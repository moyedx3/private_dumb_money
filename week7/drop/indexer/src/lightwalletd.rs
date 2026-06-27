use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tonic::transport::{Channel, ClientTlsConfig};

pub mod proto {
    #[allow(clippy::all, clippy::pedantic, clippy::nursery)]
    pub mod compact {
        tonic::include_proto!("cash.z.wallet.sdk.rpc");
    }
}

pub use proto::compact::compact_tx_streamer_client::CompactTxStreamerClient;
pub use proto::compact::{BlockId, BlockRange, CompactBlock, RawTransaction, TxFilter};

#[async_trait]
pub trait LightwalletdClient: Send + Sync {
    async fn current_chain_tip(&self) -> Result<u64>;
    async fn fetch_block_range(&self, start: u64, end: u64) -> Result<Vec<CompactBlock>>;
    /// Fetch the full serialized transaction bytes for the given txid.
    ///
    /// `txid` is the 32-byte little-endian transaction identifier as found in
    /// `CompactTx::hash`.  Returns the raw transaction bytes suitable for
    /// `zcash_primitives::transaction::Transaction::read`.
    async fn fetch_transaction(&self, txid: &[u8; 32]) -> Result<Vec<u8>>;
}

pub struct GrpcClient {
    primary: String,
    backup: Option<String>,
}

impl GrpcClient {
    pub fn new(primary: impl Into<String>, backup: Option<String>) -> Self {
        Self {
            primary: primary.into(),
            backup,
        }
    }

    async fn connect(&self, url: &str) -> Result<CompactTxStreamerClient<Channel>> {
        let tls = ClientTlsConfig::new();
        let channel = Channel::from_shared(url.to_string())?
            .tls_config(tls)?
            .connect()
            .await?;
        Ok(CompactTxStreamerClient::new(channel))
    }

    async fn with_failover<F, Fut, T>(&self, f: F) -> Result<T>
    where
        F: Fn(CompactTxStreamerClient<Channel>) -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        match self.connect(&self.primary).await {
            Ok(c) => match f(c).await {
                Ok(v) => Ok(v),
                Err(primary_err) => {
                    tracing::warn!(?primary_err, "primary lightwalletd failed, trying backup");
                    if let Some(backup) = &self.backup {
                        let c = self.connect(backup).await?;
                        f(c).await
                    } else {
                        Err(primary_err)
                    }
                }
            },
            Err(connect_err) => {
                tracing::warn!(
                    ?connect_err,
                    "primary lightwalletd unreachable, trying backup"
                );
                if let Some(backup) = &self.backup {
                    let c = self.connect(backup).await?;
                    f(c).await
                } else {
                    Err(connect_err)
                }
            }
        }
    }
}

#[async_trait]
impl LightwalletdClient for GrpcClient {
    async fn current_chain_tip(&self) -> Result<u64> {
        self.with_failover(|mut c| async move {
            let resp = c.get_latest_block(proto::compact::ChainSpec {}).await?;
            Ok(resp.into_inner().height)
        })
        .await
    }

    async fn fetch_block_range(&self, start: u64, end: u64) -> Result<Vec<CompactBlock>> {
        if start > end {
            return Err(anyhow!("start > end"));
        }
        self.with_failover(|mut c| async move {
            let req = BlockRange {
                start: Some(BlockId {
                    height: start,
                    hash: vec![],
                }),
                end: Some(BlockId {
                    height: end,
                    hash: vec![],
                }),
                pool_types: vec![],
            };
            let mut stream = c.get_block_range(req).await?.into_inner();
            let mut blocks = Vec::with_capacity((end - start + 1) as usize);
            while let Some(b) = stream.message().await? {
                blocks.push(b);
            }
            Ok(blocks)
        })
        .await
    }

    async fn fetch_transaction(&self, txid: &[u8; 32]) -> Result<Vec<u8>> {
        let hash = txid.to_vec();
        self.with_failover(|mut c| {
            let hash = hash.clone();
            async move {
                let req = TxFilter {
                    block: None,
                    index: 0,
                    hash,
                };
                let resp = c.get_transaction(req).await?;
                Ok(resp.into_inner().data)
            }
        })
        .await
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub struct MockClient {
        pub tip: u64,
        pub blocks: Vec<CompactBlock>,
        /// Pre-canned raw transaction bytes keyed by txid (32-byte array).
        /// If a txid is not found, `fetch_transaction` returns an empty vec.
        pub raw_txs: Vec<([u8; 32], Vec<u8>)>,
    }

    impl MockClient {
        pub fn new(tip: u64, blocks: Vec<CompactBlock>) -> Self {
            Self {
                tip,
                blocks,
                raw_txs: vec![],
            }
        }
    }

    #[async_trait]
    impl LightwalletdClient for MockClient {
        async fn current_chain_tip(&self) -> Result<u64> {
            Ok(self.tip)
        }
        async fn fetch_block_range(&self, start: u64, end: u64) -> Result<Vec<CompactBlock>> {
            Ok(self
                .blocks
                .iter()
                .filter(|b| b.height >= start && b.height <= end)
                .cloned()
                .collect())
        }
        async fn fetch_transaction(&self, txid: &[u8; 32]) -> Result<Vec<u8>> {
            for (id, bytes) in &self.raw_txs {
                if id == txid {
                    return Ok(bytes.clone());
                }
            }
            // Return empty vec — caller treats empty as "no shielded outputs"
            Ok(vec![])
        }
    }

    #[tokio::test]
    async fn mock_returns_filtered_blocks() {
        let mock = MockClient::new(
            100,
            vec![
                CompactBlock {
                    height: 10,
                    ..Default::default()
                },
                CompactBlock {
                    height: 20,
                    ..Default::default()
                },
                CompactBlock {
                    height: 30,
                    ..Default::default()
                },
            ],
        );
        let got = mock.fetch_block_range(15, 25).await.unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].height, 20);
    }

    #[tokio::test]
    #[ignore]
    async fn live_testnet_returns_a_tip() {
        let c = GrpcClient::new("https://testnet.zec.rocks:443", None);
        let tip = c.current_chain_tip().await.unwrap();
        assert!(tip > 1_000_000);
    }
}
