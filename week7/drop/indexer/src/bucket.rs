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

#[async_trait::async_trait]
impl Bucket for FsBucket {
    async fn put(&self, key: &str, bytes: &[u8]) -> anyhow::Result<()> {
        tokio::fs::write(self.dir.join(key), bytes).await?;
        Ok(())
    }
    async fn get(&self, key: &str) -> anyhow::Result<Option<Vec<u8>>> {
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

        b.put("k1", b"hello").await.unwrap();
        assert_eq!(b.get("k1").await.unwrap().as_deref(), Some(&b"hello"[..]));
        assert_eq!(b.get("missing").await.unwrap(), None);
        assert_eq!(b.list().await.unwrap(), vec!["k1".to_string()]);
    }
}
