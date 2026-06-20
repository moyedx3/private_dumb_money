//! Catalog store (interface I3) — holds internal `DropConfig`s (read by A1 via the `Catalog`
//! trait) and serves the public catalog JSON (browsed by Lane B). Secrets never leave here.

use std::collections::HashMap;
use std::sync::RwLock;

use crate::{Catalog, CatalogEntry, DropConfig};

/// In-memory catalog (demo scope). Production must persist this — a restart otherwise loses
/// every provisioned drop, forcing creators to re-provision.
#[derive(Default)]
pub struct CatalogStore {
    inner: RwLock<HashMap<u64, (DropConfig, String)>>, // drop_id -> (config, title)
}

impl CatalogStore {
    /// Store (or overwrite) a provisioned drop's config + display title.
    pub fn insert(&self, drop_id: u64, cfg: DropConfig, title: String) {
        self.inner.write().unwrap().insert(drop_id, (cfg, title));
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
        self.inner.read().unwrap().get(&drop_id).map(|(c, _)| c.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Catalog, DropConfig};

    #[test]
    fn lookup_after_insert_and_public_hides_secrets() {
        let store = CatalogStore::default();
        store.insert(
            1,
            DropConfig {
                price_zat: 500,
                k_drop: [1u8; 32],
                creator_ufvk: "uview1secret".into(),
                h_content: "h1".into(),
            },
            "Cat photo".into(),
        );

        assert_eq!(store.lookup(1).unwrap().price_zat, 500);
        assert!(store.lookup(2).is_none());

        let public = store.public_entries();
        assert_eq!(public.len(), 1);
        assert_eq!(public[0].h_content, "h1");
        assert_eq!(public[0].price_zec, "0.000005");
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
}
