//! Lightwalletd scan wiring: compact block txids -> full tx decrypt -> engine.
//!
//! `scan_once` is the reusable A1 library entry point. It deliberately accepts a
//! block range so callers can use it both for manual live checks and, later, for
//! cursor-based TEE operation.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use zcash_protocol::consensus::Network;

use crate::bucket::FsBucket;
use crate::catalog::CatalogStore;
use crate::detect::detect_incoming;
use crate::detect::infer_network_from_ufvk;
use crate::engine::{Engine, Note, PaymentDispatch};
use crate::lightwalletd::LightwalletdClient;
use crate::memo::decode_memo;
use crate::state::{MemoryScanState, ScanState};
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

/// Runtime scanner options used by the A2 server integration.
#[derive(Clone, Debug)]
pub struct RuntimeScanConfig {
    /// Sleep between polling passes.
    pub poll_interval: Duration,
    /// Maximum block count per UFVK per pass.
    pub batch_size: u64,
    /// Optional first height. If unset, a newly seen UFVK starts at current tip.
    pub start_height: Option<u64>,
}

impl Default for RuntimeScanConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(30),
            batch_size: 10,
            start_height: None,
        }
    }
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

/// Scan an inclusive block range once, using persistent state for replay
/// suppression and cursor advancement.
///
/// `state` is consulted before full transaction fetches. A txid is marked seen
/// only after this pass produces at least one dispatch for it; the block cursor
/// is advanced only after the full range completes without error.
pub async fn scan_once_with_state<C, K, B, S>(
    client: &C,
    ufvk: &str,
    network: &Network,
    start: u64,
    end: u64,
    state: &mut S,
    engine: &mut Engine<K, B>,
) -> Result<ScanSummary>
where
    C: LightwalletdClient + ?Sized,
    K: Catalog,
    B: Bucket,
    S: ScanState,
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
    let mut max_height = None;

    for block in blocks {
        max_height = Some(max_height.map_or(block.height, |h: u64| h.max(block.height)));

        for compact_tx in &block.vtx {
            summary.compact_txs += 1;

            let txid: [u8; 32] =
                compact_tx.txid.as_slice().try_into().map_err(|_| {
                    anyhow!("compact txid at height {} is not 32 bytes", block.height)
                })?;
            if state.has_seen_txid(&txid) {
                continue;
            }

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
            let per_tx = process_incoming_notes_with_state(&txid, incoming, state, engine).await?;
            summary.merge(per_tx);
        }
    }

    if let Some(height) = max_height {
        state.set_last_scanned_height(height);
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

/// State-aware note processing used by persistent scanner loops.
pub async fn process_incoming_notes_with_state<K, B, S>(
    txid: &[u8; 32],
    notes: impl IntoIterator<Item = crate::detect::IncomingNote>,
    state: &mut S,
    engine: &mut Engine<K, B>,
) -> Result<ScanSummary>
where
    K: Catalog,
    B: Bucket,
    S: ScanState,
{
    if state.has_seen_txid(txid) {
        return Ok(ScanSummary::default());
    }

    let summary = process_incoming_notes(txid, notes, engine).await?;
    if !summary.dispatches.is_empty() {
        state.mark_seen_txid(*txid);
    }
    Ok(summary)
}

/// One A1 scanner pass over all provisioned drops in the A2 catalog.
///
/// Multiple drops can share the same creator UFVK, so this scans each unique UFVK
/// once and lets the shared `CatalogStore` resolve individual memo `drop_id`s
/// inside `Engine::on_note`.
pub async fn scan_catalog_once<C>(
    client: &C,
    catalog: CatalogStore,
    dispatch: FsBucket,
    states: &mut HashMap<String, MemoryScanState>,
    cfg: &RuntimeScanConfig,
) -> Result<Vec<ScanSummary>>
where
    C: LightwalletdClient + ?Sized,
{
    let mut seen_ufvks = HashSet::new();
    let mut out = Vec::new();

    for (_, drop_cfg) in catalog.configs() {
        if drop_cfg.creator_ufvk.trim().is_empty()
            || !seen_ufvks.insert(drop_cfg.creator_ufvk.clone())
        {
            continue;
        }

        let ufvk = drop_cfg.creator_ufvk;
        let network = infer_network_from_ufvk(&ufvk)
            .with_context(|| "infer network from provisioned creator UFVK")?;
        let tip = client.current_chain_tip().await.context("get chain tip")?;
        let state = states.entry(ufvk.clone()).or_default();
        let start = state
            .last_scanned_height()
            .and_then(|h| h.checked_add(1))
            .or(cfg.start_height)
            .unwrap_or(tip);
        if start > tip {
            continue;
        }
        let end = start
            .saturating_add(cfg.batch_size.saturating_sub(1))
            .min(tip);

        let mut engine = Engine::new(catalog.clone(), dispatch.clone());
        let summary =
            scan_once_with_state(client, &ufvk, &network, start, end, state, &mut engine).await?;
        out.push(summary);
    }

    Ok(out)
}

/// Long-running A1 scanner task mounted by the A2 binary.
///
/// State is in-memory for this demo integration; production should replace it
/// with `EncryptedFileScanState` backed by a TEE sealing key before relying on
/// restart-safe replay suppression.
pub async fn run_catalog_loop<C>(
    client: C,
    catalog: CatalogStore,
    dispatch: FsBucket,
    cfg: RuntimeScanConfig,
) -> Result<()>
where
    C: LightwalletdClient,
{
    let mut states = HashMap::new();
    loop {
        match scan_catalog_once(
            &client,
            catalog.clone(),
            dispatch.clone(),
            &mut states,
            &cfg,
        )
        .await
        {
            Ok(summaries) => {
                for summary in summaries {
                    if !summary.dispatches.is_empty() {
                        tracing::info!(
                            dispatches = summary.dispatches.len(),
                            decoded_memos = summary.decoded_memos,
                            incoming_notes = summary.incoming_notes,
                            "A1 scanner published dispatches"
                        );
                    }
                }
            }
            Err(err) => {
                tracing::warn!(?err, "A1 scanner pass failed; will retry");
            }
        }
        tokio::time::sleep(cfg.poll_interval).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::{IncomingNote, ShieldedPool};
    use crate::dispatch::DISPATCH_BLOB_LEN;
    use crate::lightwalletd::proto::compact::{CompactBlock, CompactTx};
    use crate::lightwalletd::tests::MockClient;
    use crate::memo::encode_text_memo;
    use crate::state::{MemoryScanState, ScanState};
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
                h_content: "abc123".to_string(),
                deposit_addr: "u1mockshielded".to_string(),
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

        async fn get(&self, key: &str) -> anyhow::Result<Option<Vec<u8>>> {
            Ok(self
                .puts
                .lock()
                .unwrap()
                .iter()
                .find(|(stored_key, _)| stored_key == key)
                .map(|(_, bytes)| bytes.clone()))
        }

        async fn list(&self) -> anyhow::Result<Vec<String>> {
            Ok(self
                .puts
                .lock()
                .unwrap()
                .iter()
                .map(|(key, _)| key.clone())
                .collect())
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

    #[tokio::test]
    async fn state_aware_scan_skips_already_seen_txids_and_advances_cursor() {
        let txid = [4u8; 32];
        let client = MockClient::new(
            50,
            vec![CompactBlock {
                height: 50,
                vtx: vec![CompactTx {
                    txid: txid.to_vec(),
                    ..Default::default()
                }],
                ..Default::default()
            }],
        );
        let mut state = MemoryScanState::new();
        state.mark_seen_txid(txid);
        let mut engine = engine_with_bucket(MockBucket::default());

        let summary = scan_once_with_state(
            &client,
            "uview1dummy-not-used-for-skipped-tx",
            &Network::MainNetwork,
            50,
            50,
            &mut state,
            &mut engine,
        )
        .await
        .unwrap();

        assert_eq!(summary.blocks_fetched, 1);
        assert_eq!(summary.compact_txs, 1);
        assert_eq!(summary.full_txs_fetched, 0);
        assert_eq!(state.last_scanned_height(), Some(50));
    }

    #[tokio::test]
    async fn state_aware_note_processing_marks_dispatched_txid() {
        let bucket = MockBucket::default();
        let mut engine = engine_with_bucket(bucket);
        let mut state = MemoryScanState::new();
        let e_pub: [u8; 32] = core::array::from_fn(|i| i as u8);
        let txid = [5u8; 32];

        let summary = process_incoming_notes_with_state(
            &txid,
            [IncomingNote {
                pool: ShieldedPool::Orchard,
                value_zat: 10_000,
                memo: encode_text_memo(1, &e_pub).into_bytes(),
            }],
            &mut state,
            &mut engine,
        )
        .await
        .unwrap();

        assert_eq!(summary.dispatches.len(), 1);
        assert!(state.has_seen_txid(&txid));

        let duplicate = process_incoming_notes_with_state(
            &txid,
            [IncomingNote {
                pool: ShieldedPool::Orchard,
                value_zat: 10_000,
                memo: encode_text_memo(1, &e_pub).into_bytes(),
            }],
            &mut state,
            &mut engine,
        )
        .await
        .unwrap();

        assert_eq!(duplicate.incoming_notes, 0);
        assert_eq!(duplicate.dispatches.len(), 0);
    }
}
