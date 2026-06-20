//! Filesystem-backed bucket — hash-addressed blob storage (demo scope; production swaps in
//! S3/Blossom). A1 writes dispatch blobs, C writes content blobs, B reads/lists them.
//! Keys are hex (safe as filenames).

use std::path::PathBuf;

use crate::Bucket;

/// Filesystem bucket. Keys must be safe filenames (our keys are hex).
pub struct FsBucket {
    dir: PathBuf,
}

impl FsBucket {
    pub fn new(dir: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }
}

/// Blob keys are hash hex (dispatch-blob / content-blob design). Reject anything else so a
/// request can never address a path outside the bucket dir — path-traversal hardening.
/// (axum percent-decodes `:key` *after* route matching, so `%2e%2e%2f` would otherwise reach
/// the FS as `../`.) Hex-only also bounds the length and charset.
fn valid_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= 128 && key.bytes().all(|b| b.is_ascii_hexdigit())
}

#[async_trait::async_trait]
impl Bucket for FsBucket {
    async fn put(&self, key: &str, bytes: &[u8]) -> anyhow::Result<()> {
        if !valid_key(key) {
            anyhow::bail!("invalid bucket key");
        }
        tokio::fs::write(self.dir.join(key), bytes).await?;
        Ok(())
    }
    async fn get(&self, key: &str) -> anyhow::Result<Option<Vec<u8>>> {
        if !valid_key(key) {
            return Ok(None);
        }
        match tokio::fs::read(self.dir.join(key)).await {
            Ok(b) => Ok(Some(b)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    async fn list(&self) -> anyhow::Result<Vec<String>> {
        let mut out = Vec::new();
        let mut rd = tokio::fs::read_dir(&self.dir).await?;
        while let Some(e) = rd.next_entry().await? {
            out.push(e.file_name().to_string_lossy().into_owned());
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Bucket;

    #[tokio::test]
    async fn bucket_put_get_list_roundtrip() {
        let dir = std::env::temp_dir().join("drop-bucket-test-a2");
        let _ = std::fs::remove_dir_all(&dir);
        let b = FsBucket::new(&dir).unwrap();

        b.put("deadbeef", b"hello").await.unwrap(); // keys are hex blob hashes
        assert_eq!(b.get("deadbeef").await.unwrap().as_deref(), Some(&b"hello"[..]));
        assert_eq!(b.get("cafe").await.unwrap(), None); // valid hex, not present
        assert_eq!(b.list().await.unwrap(), vec!["deadbeef".to_string()]);
    }

    #[tokio::test]
    async fn bucket_rejects_path_traversal_and_non_hex_keys() {
        // Bucket lives at <root>/bucket so "../escape" would land at <root>/escape.
        let root = std::env::temp_dir().join("drop-bucket-traversal");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let b = FsBucket::new(root.join("bucket")).unwrap();

        // keys are hex (blob hashes). Anything else — traversal, slashes, dots — must be refused
        // and must never touch the filesystem outside the bucket dir.
        for bad in ["../escape", "..", "a/b", "k1.txt", "/etc/passwd", ""] {
            assert!(b.put(bad, b"x").await.is_err(), "put({bad:?}) should be rejected");
            assert_eq!(b.get(bad).await.unwrap(), None, "get({bad:?}) should be None");
        }
        // proof the rejected "../escape" never wrote outside the bucket dir
        assert!(!root.join("escape").exists());

        // a valid hex key still works
        b.put("deadbeef", b"ok").await.unwrap();
        assert_eq!(b.get("deadbeef").await.unwrap().as_deref(), Some(&b"ok"[..]));
    }
}
