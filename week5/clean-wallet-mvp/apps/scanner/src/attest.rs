use anyhow::Result;
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct Quote {
    pub quote_hex: String,
    pub event_log: serde_json::Value,
    pub vm_config: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct Info {
    /// Hex-encoded with "0x" prefix; the TDX MRTD ("code measurement").
    pub code_measurement: String,
}

#[async_trait]
pub trait Attestor: Send + Sync {
    async fn get_quote(&self, report_data: &[u8; 32]) -> Result<Quote>;
    async fn info(&self) -> Result<Info>;
}

pub struct DstackAttestor {
    socket_path: String,
}

impl DstackAttestor {
    pub fn new(socket_path: impl Into<String>) -> Self {
        Self { socket_path: socket_path.into() }
    }
}

#[async_trait]
impl Attestor for DstackAttestor {
    async fn get_quote(&self, report_data: &[u8; 32]) -> Result<Quote> {
        // Pack the 32-byte hash into the 64-byte reportData slot, zero-padded.
        let mut padded = [0u8; 64];
        padded[..32].copy_from_slice(report_data);

        // Direct unix-socket HTTP POST to /GetQuote
        // Body: { "report_data": "<128 hex chars>" }
        // Response: { "quote": "<hex string>", "event_log": <json>, "vm_config": <json object> }
        post_uds_json(
            &self.socket_path,
            "/GetQuote",
            &serde_json::json!({ "report_data": hex::encode(padded) }),
        )
        .await
        .and_then(|resp: serde_json::Value| {
            let quote_hex = resp.get("quote")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("dstack getQuote: missing 'quote' field"))?
                .to_string();
            let event_log = resp.get("event_log").cloned().unwrap_or(serde_json::json!([]));
            let vm_config = resp.get("vm_config").cloned().unwrap_or(serde_json::json!({}));
            Ok(Quote { quote_hex, event_log, vm_config })
        })
    }

    async fn info(&self) -> Result<Info> {
        // POST /Info with empty body. Response includes "mrtd" hex string.
        let resp: serde_json::Value = post_uds_json(
            &self.socket_path,
            "/Info",
            &serde_json::json!({}),
        ).await?;

        let mrtd = resp.get("mrtd").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("dstack info: missing 'mrtd' field"))?;

        let normalized = if mrtd.starts_with("0x") {
            mrtd.to_string()
        } else {
            format!("0x{}", mrtd)
        };
        Ok(Info { code_measurement: normalized })
    }
}

/// Send a JSON POST to the dstack unix socket and parse the JSON response.
async fn post_uds_json(
    socket_path: &str,
    endpoint: &str,
    body: &serde_json::Value,
) -> Result<serde_json::Value> {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream as StdUnixStream;
    // Using std UnixStream synchronously inside an async fn is acceptable here because
    // dstack calls are infrequent (twice per /screen request) and the request is small.
    let body_bytes = serde_json::to_vec(body)?;
    let request = format!(
        "POST {} HTTP/1.1\r\nHost: dstack\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        endpoint,
        body_bytes.len()
    );

    let socket_path_owned = socket_path.to_string();
    let result = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
        let mut stream = StdUnixStream::connect(&socket_path_owned)?;
        stream.write_all(request.as_bytes())?;
        stream.write_all(&body_bytes)?;
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf)?;
        Ok(buf)
    }).await??;

    // Split headers from body: find "\r\n\r\n"
    let sep = b"\r\n\r\n";
    let split = result.windows(sep.len()).position(|w| w == sep)
        .ok_or_else(|| anyhow::anyhow!("dstack response: missing header/body separator"))?;
    let body_start = split + sep.len();
    let body_bytes = &result[body_start..];

    let parsed: serde_json::Value = serde_json::from_slice(body_bytes)
        .map_err(|e| anyhow::anyhow!("dstack response not JSON: {e}; raw body: {:?}", String::from_utf8_lossy(body_bytes)))?;
    Ok(parsed)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub struct MockAttestor {
        pub code_measurement: String,
        pub quote_hex: String,
    }

    #[async_trait]
    impl Attestor for MockAttestor {
        async fn get_quote(&self, report_data: &[u8; 32]) -> Result<Quote> {
            Ok(Quote {
                quote_hex: format!("{}-{}", self.quote_hex, hex::encode(report_data)),
                event_log: serde_json::json!([]),
                vm_config: serde_json::json!({"measurement": self.code_measurement}),
            })
        }
        async fn info(&self) -> Result<Info> {
            Ok(Info { code_measurement: self.code_measurement.clone() })
        }
    }

    #[tokio::test]
    async fn mock_packs_report_data_into_quote() {
        let a = MockAttestor {
            code_measurement: format!("0x{}", "b".repeat(96)),
            quote_hex: "QUOTE".into(),
        };
        let report_data = [42u8; 32];
        let q = a.get_quote(&report_data).await.unwrap();
        assert!(q.quote_hex.contains(&hex::encode(report_data)));
    }

    #[tokio::test]
    async fn mock_info_returns_provided_measurement() {
        let a = MockAttestor {
            code_measurement: format!("0x{}", "c".repeat(96)),
            quote_hex: "X".into(),
        };
        let info = a.info().await.unwrap();
        assert_eq!(info.code_measurement, format!("0x{}", "c".repeat(96)));
    }
}
