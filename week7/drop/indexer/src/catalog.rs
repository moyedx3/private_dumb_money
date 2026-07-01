//! Catalog store (interface I3) — holds internal `DropConfig`s (read by A1 via the `Catalog`
//! trait) and serves the public catalog JSON (browsed by Lane B). Secrets never leave here.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::{Catalog, CatalogEntry, DropConfig};

#[derive(Debug, PartialEq, Eq)]
pub enum CatalogError {
    OwnershipMismatch,
}

/// In-memory catalog (demo scope). Production must persist this — a restart otherwise loses
/// every provisioned drop, forcing creators to re-provision.
#[derive(Clone, Default)]
pub struct CatalogStore {
    inner: Arc<RwLock<HashMap<u64, (DropConfig, String)>>>, // drop_id -> (config, title)
}

impl CatalogStore {
    /// Store a provisioned drop's config + display title.
    ///
    /// The first creator UFVK to claim a `drop_id` owns it. Re-provisioning by
    /// the same creator is allowed, but a different creator cannot overwrite an
    /// existing drop and redirect buyer payments.
    pub fn insert(&self, drop_id: u64, cfg: DropConfig, title: String) -> Result<(), CatalogError> {
        let mut inner = self.inner.write().unwrap();
        if let Some((existing, _)) = inner.get(&drop_id) {
            if existing.creator_ufvk != cfg.creator_ufvk {
                return Err(CatalogError::OwnershipMismatch);
            }
        }
        inner.insert(drop_id, (cfg, title));
        Ok(())
    }

    /// Snapshot internal configs for A1's scanner. This is intentionally crate-public
    /// runtime plumbing: secrets stay in-process inside the enclave boundary and are
    /// never serialized through HTTP.
    pub fn configs(&self) -> Vec<(u64, DropConfig)> {
        self.inner
            .read()
            .unwrap()
            .iter()
            .map(|(id, (cfg, _))| (*id, cfg.clone()))
            .collect()
    }

    /// Public catalog entries (interface I3-a) — no secrets (no `k_drop`, no `creator_ufvk`).
    pub fn public_entries(&self) -> Vec<CatalogEntry> {
        self.inner
            .read()
            .unwrap()
            .iter()
            .map(|(id, (c, title))| CatalogEntry {
                drop_id: *id,
                price_zec: zat_to_zec_string(c.price_zat),
                h_content: c.h_content.clone(),
                title: title.clone(),
                deposit_addr: c.deposit_addr.clone(),
            })
            .collect()
    }
}

fn zat_to_zec_string(zat: u64) -> String {
    let whole = zat / 100_000_000;
    let frac = zat % 100_000_000;
    if frac == 0 {
        return whole.to_string();
    }
    let mut frac_s = format!("{frac:08}");
    while frac_s.ends_with('0') {
        frac_s.pop();
    }
    format!("{whole}.{frac_s}")
}

impl Catalog for CatalogStore {
    fn lookup(&self, drop_id: u64) -> Option<DropConfig> {
        self.inner
            .read()
            .unwrap()
            .get(&drop_id)
            .map(|(c, _)| c.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Catalog, DropConfig};

    #[test]
    fn lookup_after_insert_and_public_hides_secrets() {
        let store = CatalogStore::default();
        store
            .insert(
                1,
                DropConfig {
                    price_zat: 500,
                    k_drop: [1u8; 32],
                    creator_ufvk: "uview1secret".into(),
                    h_content: "h1".into(),
                    deposit_addr: "u1demo".into(),
                },
                "Cat photo".into(),
            )
            .unwrap();

        assert_eq!(store.lookup(1).unwrap().price_zat, 500);
        assert!(store.lookup(2).is_none());
        assert_eq!(store.configs().len(), 1);
        assert_eq!(store.configs()[0].0, 1);

        let public = store.public_entries();
        assert_eq!(public.len(), 1);
        assert_eq!(public[0].h_content, "h1");
        assert_eq!(public[0].price_zec, "0.000005");
        assert_eq!(public[0].deposit_addr, "u1demo");
        // the public JSON must not leak the viewing key (or any secret)
        let json = serde_json::to_string(&public).unwrap();
        assert!(!json.contains("uview1secret"));
    }

    #[test]
    fn public_catalog_formats_price_zec_for_interfaces_i3a() {
        assert_eq!(zat_to_zec_string(0), "0");
        assert_eq!(zat_to_zec_string(1), "0.00000001");
        assert_eq!(zat_to_zec_string(1_000_000), "0.01");
        assert_eq!(zat_to_zec_string(100_000_000), "1");
        assert_eq!(zat_to_zec_string(123_456_789), "1.23456789");
    }
    #[test]
    fn same_creator_can_reprovision_but_other_creator_cannot_hijack_drop_id() {
        let store = CatalogStore::default();
        let mut cfg = DropConfig {
            price_zat: 500,
            k_drop: [1u8; 32],
            creator_ufvk: "uview1creator-a".into(),
            h_content: "h1".into(),
            deposit_addr: "u1creator-a".into(),
        };

        store.insert(1, cfg.clone(), "Cat photo".into()).unwrap();

        cfg.price_zat = 700;
        cfg.deposit_addr = "u1creator-a-new".into();
        store.insert(1, cfg.clone(), "Cat photo v2".into()).unwrap();
        assert_eq!(store.lookup(1).unwrap().price_zat, 700);
        assert_eq!(store.public_entries()[0].deposit_addr, "u1creator-a-new");

        let attacker = DropConfig {
            price_zat: 1,
            k_drop: [2u8; 32],
            creator_ufvk: "uview1creator-b".into(),
            h_content: "evil".into(),
            deposit_addr: "u1attacker".into(),
        };
        assert_eq!(
            store.insert(1, attacker, "evil".into()),
            Err(CatalogError::OwnershipMismatch)
        );

        let public = store.public_entries();
        assert_eq!(public.len(), 1);
        assert_eq!(public[0].deposit_addr, "u1creator-a-new");
        assert_eq!(store.lookup(1).unwrap().creator_ufvk, "uview1creator-a");
    }
}
