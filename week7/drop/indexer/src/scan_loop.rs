//! Lightwalletd scan wiring: compact block txids -> full tx decrypt -> engine.
//!
//! `scan_once` is the reusable A1 library entry point. It deliberately accepts a
//! block range so callers can use it both for manual live checks and, later, for
//! cursor-based TEE operation.

use anyhow::{anyhow, Context, Result};
use zcash_protocol::consensus::Network;

use crate::detect::detect_incoming;
use crate::engine::{Engine, Note, PaymentDispatch};
use crate::lightwalletd::LightwalletdClient;
use crate::memo::decode_memo;
use crate::{Bucket, Catalog};

/// Counters and dispatch metadata produced by one range scan.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ScanSummary {
    pub blocks_fetched: usize,
    pub compact_txs: usize,
    pub full_txs_fetched: usize,
    pub incoming_notes: usize,
    pub notes_without_memo: usize,
    pub decoded_memos: usize,
    pub undecodable_memos: usize,
    pub dispatches: Vec<PaymentDispatch>,
}

impl ScanSummary {
    fn merge(&mut self, other: ScanSummary) {
        self.incoming_notes += other.incoming_notes;
        self.notes_without_memo += other.notes_without_memo;
        self.decoded_memos += other.decoded_memos;
        self.undecodable_memos += other.undecodable_memos;
        self.dispatches.extend(other.dispatches);
    }
}

/// Scan an inclusive block range once.
///
/// Flow: `GetBlockRange` compact txids -> `GetTransaction` full bytes ->
/// `detect_incoming` -> `decode_memo` (`raw40` or `A1B64:`) ->
/// `Engine::on_note`.
pub async fn scan_once<C, K, B>(
    client: &C,
    ufvk: &str,
    network: &Network,
    start: u64,
    end: u64,
    engine: &mut Engine<K, B>,
) -> Result<ScanSummary>
where
    C: LightwalletdClient + ?Sized,
    K: Catalog,
    B: Bucket,
{
    if start > end {
        return Err(anyhow!(
            "scan start height {start} is greater than end height {end}"
        ));
    }

    let blocks = client
        .fetch_block_range(start, end)
        .await
        .with_context(|| format!("fetch compact block range {start}..={end}"))?;

    let mut summary = ScanSummary {
        blocks_fetched: blocks.len(),
        ..ScanSummary::default()
    };

    for block in blocks {
        for compact_tx in &block.vtx {
            summary.compact_txs += 1;

            let txid: [u8; 32] =
                compact_tx.txid.as_slice().try_into().map_err(|_| {
                    anyhow!("compact txid at height {} is not 32 bytes", block.height)
                })?;

            let raw_tx = client
                .fetch_transaction(&txid)
                .await
                .with_context(|| format!("fetch full tx {}", hex::encode(txid)))?;
            summary.full_txs_fetched += 1;
            if raw_tx.is_empty() {
                continue;
            }

            let incoming = detect_incoming(ufvk, &raw_tx, network, block.height as u32)
                .with_context(|| format!("decrypt full tx at height {}", block.height))?;
            let per_tx = process_incoming_notes(&txid, incoming, engine).await?;
            summary.merge(per_tx);
        }
    }

    Ok(summary)
}

/// Convert detected incoming notes into engine notes and publish dispatches.
///
/// This is separate from `scan_once` so memo/engine wiring can be unit-tested
/// without requiring a real Zcash transaction fixture.
pub async fn process_incoming_notes<K, B>(
    txid: &[u8; 32],
    notes: impl IntoIterator<Item = crate::detect::IncomingNote>,
    engine: &mut Engine<K, B>,
) -> Result<ScanSummary>
where
    K: Catalog,
    B: Bucket,
{
    let mut summary = ScanSummary::default();

    for incoming in notes {
        summary.incoming_notes += 1;

        // ZIP-302 0xF6 means "no memo". Do not run raw 40-byte decoding on it.
        if incoming.memo.first() == Some(&0xF6) {
            summary.notes_without_memo += 1;
            continue;
        }

        let Some((drop_id, e_pub)) = decode_memo(&incoming.memo) else {
            summary.undecodable_memos += 1;
            continue;
        };
        summary.decoded_memos += 1;

        if let Some(dispatch) = engine
            .on_note(&Note {
                drop_id,
                e_pub,
                value_zat: incoming.value_zat,
                txid: *txid,
            })
            .await?
        {
            summary.dispatches.push(dispatch);
        }
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::{IncomingNote, ShieldedPool};
    use crate::dispatch::DISPATCH_BLOB_LEN;
    use crate::lightwalletd::proto::compact::{CompactBlock, CompactTx};
    use crate::lightwalletd::tests::MockClient;
    use crate::memo::encode_text_memo;
    use crate::DropConfig;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct MockCatalog {
        drop_id: u64,
        price_zat: u64,
        k_drop: [u8; 32],
    }

    impl Catalog for MockCatalog {
        fn lookup(&self, drop_id: u64) -> Option<DropConfig> {
            (drop_id == self.drop_id).then(|| DropConfig {
                price_zat: self.price_zat,
                k_drop: self.k_drop,
                creator_ufvk: "uview1mock".to_string(),
            })
        }
    }

    #[derive(Clone, Default)]
    struct MockBucket {
        puts: Arc<Mutex<Vec<(String, Vec<u8>)>>>,
    }

    impl MockBucket {
        fn count(&self) -> usize {
            self.puts.lock().unwrap().len()
        }

        fn first_blob_len(&self) -> usize {
            self.puts.lock().unwrap()[0].1.len()
        }
    }

    #[async_trait::async_trait]
    impl Bucket for MockBucket {
        async fn put(&self, key: &str, bytes: &[u8]) -> anyhow::Result<()> {
            self.puts
                .lock()
                .unwrap()
                .push((key.to_string(), bytes.to_vec()));
            Ok(())
        }
    }

    fn engine_with_bucket(bucket: MockBucket) -> Engine<MockCatalog, MockBucket> {
        Engine::new(
            MockCatalog {
                drop_id: 1,
                price_zat: 10_000,
                k_drop: [9u8; 32],
            },
            bucket,
        )
    }

    #[tokio::test]
    async fn process_notes_dispatches_decoded_text_memo() {
        let bucket = MockBucket::default();
        let mut engine = engine_with_bucket(bucket.clone());
        let e_pub: [u8; 32] = core::array::from_fn(|i| i as u8);
        let txid = [7u8; 32];

        let summary = process_incoming_notes(
            &txid,
            [IncomingNote {
                pool: ShieldedPool::Orchard,
                value_zat: 10_000,
                memo: encode_text_memo(1, &e_pub).into_bytes(),
            }],
            &mut engine,
        )
        .await
        .unwrap();

        assert_eq!(summary.incoming_notes, 1);
        assert_eq!(summary.decoded_memos, 1);
        assert_eq!(summary.dispatches.len(), 1);
        assert_eq!(summary.dispatches[0].drop_id, 1);
        assert_eq!(summary.dispatches[0].value_zat, 10_000);
        assert_eq!(bucket.count(), 1);
        assert_eq!(bucket.first_blob_len(), DISPATCH_BLOB_LEN);
    }

    #[tokio::test]
    async fn process_notes_skips_no_memo_and_undecodable_memo() {
        let bucket = MockBucket::default();
        let mut engine = engine_with_bucket(bucket.clone());
        let txid = [8u8; 32];

        let summary = process_incoming_notes(
            &txid,
            [
                IncomingNote {
                    pool: ShieldedPool::Orchard,
                    value_zat: 10_000,
                    memo: vec![0xF6, 0, 0],
                },
                IncomingNote {
                    pool: ShieldedPool::Orchard,
                    value_zat: 10_000,
                    memo: b"A1B64:not valid".to_vec(),
                },
            ],
            &mut engine,
        )
        .await
        .unwrap();

        assert_eq!(summary.incoming_notes, 2);
        assert_eq!(summary.notes_without_memo, 1);
        assert_eq!(summary.undecodable_memos, 1);
        assert_eq!(summary.dispatches.len(), 0);
        assert_eq!(bucket.count(), 0);
    }

    #[tokio::test]
    async fn scan_once_fetches_full_txs_for_compact_block() {
        let txid = [3u8; 32];
        let client = MockClient::new(
            42,
            vec![CompactBlock {
                height: 42,
                vtx: vec![CompactTx {
                    txid: txid.to_vec(),
                    ..Default::default()
                }],
                ..Default::default()
            }],
        );
        let bucket = MockBucket::default();
        let mut engine = engine_with_bucket(bucket);

        // The mock returns empty raw bytes for unknown txids, so this verifies
        // the compact->full tx plumbing without requiring a Zcash fixture.
        let summary = scan_once(
            &client,
            "uview1dummy-not-used-for-empty-raw-tx",
            &Network::MainNetwork,
            42,
            42,
            &mut engine,
        )
        .await
        .unwrap();

        assert_eq!(summary.blocks_fetched, 1);
        assert_eq!(summary.compact_txs, 1);
        assert_eq!(summary.full_txs_fetched, 1);
        assert_eq!(summary.incoming_notes, 0);
    }
}
