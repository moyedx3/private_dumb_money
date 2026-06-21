//! Payment engine: validated incoming note -> sealed dispatch blob -> bucket.
//!
//! This module intentionally does not decrypt Zcash transactions. `detect.rs`
//! owns IVK/UFVK note detection; `memo.rs` owns the on-chain memo payload.  The
//! engine receives an already-decoded payment note and performs the A1 business
//! checks: replay guard, catalog lookup, price check, dispatch wrapping, and
//! bucket publication.

use std::collections::HashSet;

use crate::dispatch::{blob_key, wrap_k_drop, EPHEMERAL_PUBLIC_KEY_LEN};
use crate::{Bucket, Catalog};

/// Decoded payment candidate produced from an incoming shielded note memo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub drop_id: u64,
    pub e_pub: [u8; EPHEMERAL_PUBLIC_KEY_LEN],
    pub value_zat: u64,
    pub txid: [u8; 32],
}

/// Metadata for a dispatch blob published by the engine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaymentDispatch {
    pub drop_id: u64,
    pub txid: [u8; 32],
    pub value_zat: u64,
    pub bucket_key: String,
}

/// In-memory replay guard for demo/test scope.
///
/// Production must persist this set (or an equivalent nullifier/txid ledger) so
/// a restart cannot re-dispatch an already-processed payment. Tracked as the A1
/// replay-window open question.
#[derive(Default, Debug)]
pub struct SeenTxids(HashSet<[u8; 32]>);

impl SeenTxids {
    /// Returns true the first time a txid is seen; false on replay.
    pub fn first_time(&mut self, txid: &[u8; 32]) -> bool {
        self.0.insert(*txid)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A1 payment engine over mockable catalog and bucket boundaries.
pub struct Engine<C: Catalog, B: Bucket> {
    cat: C,
    bucket: B,
    seen: SeenTxids,
}

impl<C: Catalog, B: Bucket> Engine<C, B> {
    pub fn new(cat: C, bucket: B) -> Self {
        Self {
            cat,
            bucket,
            seen: SeenTxids::default(),
        }
    }

    pub fn seen(&self) -> &SeenTxids {
        &self.seen
    }

    /// Process one detected incoming note.
    ///
    /// Publishes a dispatch blob iff the payment is fresh, the drop exists, and
    /// `value_zat >= price_zat`. The method is idempotent on txid.
    pub async fn on_note(&mut self, n: &Note) -> anyhow::Result<Option<PaymentDispatch>> {
        if !self.seen.first_time(&n.txid) {
            return Ok(None);
        }

        let Some(cfg) = self.cat.lookup(n.drop_id) else {
            tracing::warn!(
                drop_id = n.drop_id,
                "drop config not found; skipping payment"
            );
            return Ok(None);
        };

        if n.value_zat < cfg.price_zat {
            tracing::warn!(
                drop_id = n.drop_id,
                paid_zat = n.value_zat,
                price_zat = cfg.price_zat,
                "underpaid shielded note; skipping dispatch"
            );
            return Ok(None);
        }

        let blob = wrap_k_drop(&cfg.k_drop, &n.e_pub)?;
        let key = blob_key(&blob[..EPHEMERAL_PUBLIC_KEY_LEN], &n.txid);
        self.bucket.put(&key, &blob).await?;

        Ok(Some(PaymentDispatch {
            drop_id: n.drop_id,
            txid: n.txid,
            value_zat: n.value_zat,
            bucket_key: key,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::DISPATCH_BLOB_LEN;
    use crate::DropConfig;
    use dryoc::keypair::StackKeyPair;
    use dryoc::types::ByteArray;
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

        fn entries(&self) -> Vec<(String, Vec<u8>)> {
            self.puts.lock().unwrap().clone()
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

    fn buyer_epub() -> [u8; EPHEMERAL_PUBLIC_KEY_LEN] {
        let buyer = StackKeyPair::gen();
        *buyer.public_key.as_array()
    }

    #[test]
    fn rejects_duplicate_txid() {
        let mut seen = SeenTxids::default();
        let id = [1u8; 32];

        assert!(seen.first_time(&id));
        assert!(!seen.first_time(&id));
        assert_eq!(seen.len(), 1);
    }

    #[tokio::test]
    async fn valid_payment_publishes_one_blob_underpay_none() {
        let cat = MockCatalog {
            drop_id: 1,
            price_zat: 10_000,
            k_drop: [9u8; 32],
        };
        let bucket = MockBucket::default();
        let mut eng = Engine::new(cat, bucket.clone());
        let e_pub = buyer_epub();

        let dispatched = eng
            .on_note(&Note {
                drop_id: 1,
                e_pub,
                value_zat: 10_000,
                txid: [1u8; 32],
            })
            .await
            .unwrap();
        assert!(dispatched.is_some());
        assert_eq!(bucket.count(), 1);
        let entries = bucket.entries();
        assert_eq!(entries[0].0.len(), 64);
        assert_eq!(entries[0].1.len(), DISPATCH_BLOB_LEN);

        let underpaid = eng
            .on_note(&Note {
                drop_id: 1,
                e_pub,
                value_zat: 9_999,
                txid: [2u8; 32],
            })
            .await
            .unwrap();
        assert!(underpaid.is_none());
        assert_eq!(bucket.count(), 1);
    }

    #[tokio::test]
    async fn duplicate_txid_does_not_republish() {
        let cat = MockCatalog {
            drop_id: 1,
            price_zat: 10_000,
            k_drop: [9u8; 32],
        };
        let bucket = MockBucket::default();
        let mut eng = Engine::new(cat, bucket.clone());
        let note = Note {
            drop_id: 1,
            e_pub: buyer_epub(),
            value_zat: 10_000,
            txid: [3u8; 32],
        };

        assert!(eng.on_note(&note).await.unwrap().is_some());
        assert!(eng.on_note(&note).await.unwrap().is_none());
        assert_eq!(bucket.count(), 1);
    }

    #[tokio::test]
    async fn unknown_drop_does_not_publish() {
        let cat = MockCatalog {
            drop_id: 1,
            price_zat: 10_000,
            k_drop: [9u8; 32],
        };
        let bucket = MockBucket::default();
        let mut eng = Engine::new(cat, bucket.clone());

        let result = eng
            .on_note(&Note {
                drop_id: 404,
                e_pub: buyer_epub(),
                value_zat: 10_000,
                txid: [4u8; 32],
            })
            .await
            .unwrap();

        assert!(result.is_none());
        assert_eq!(bucket.count(), 0);
    }
}
