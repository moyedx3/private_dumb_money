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
        // POST /Info with empty body.
        // Response shape: { "app_id": "...", "tcb_info": "{\"mrtd\":\"<hex>\", ...}", ... }
        // `tcb_info` is a JSON-encoded STRING (not an object), so we parse it in two steps.
        let resp: serde_json::Value = post_uds_json(
            &self.socket_path,
            "/Info",
            &serde_json::json!({}),
        ).await?;

        // The dstack /Info endpoint returns `tcb_info` as a JSON-encoded STRING that
        // contains the `mrtd` field. We parse the string into a nested Value, then
        // pull `mrtd` out of it.
        let tcb_info_str = resp.get("tcb_info").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("dstack info: missing 'tcb_info' field"))?;
        let tcb_info: serde_json::Value = serde_json::from_str(tcb_info_str)
            .map_err(|e| anyhow::anyhow!("dstack info: tcb_info is not valid JSON: {e}"))?;
        let mrtd = tcb_info.get("mrtd").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("dstack info: missing 'mrtd' inside tcb_info"))?;

        let normalized = if mrtd.starts_with("0x") {
            mrtd.to_lowercase()
        } else {
            format!("0x{}", mrtd.to_lowercase())
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

    // Parse the HTTP status line (first line of headers)
    let header_bytes = &result[..split];
    let status_line = header_bytes.split(|&b| b == b'\n').next().unwrap_or(&[]);
    let status_line_str = std::str::from_utf8(status_line).unwrap_or("").trim();
    // Format: "HTTP/1.1 200 OK"
    let status_code: u16 = status_line_str
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow::anyhow!("dstack response: malformed status line: {status_line_str:?}"))?;
    if !(200..300).contains(&status_code) {
        let body_str = String::from_utf8_lossy(body_bytes);
        return Err(anyhow::anyhow!(
            "dstack HTTP {status_code} from {endpoint}: {body_str}"
        ));
    }

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

    #[tokio::test]
    #[ignore]
    async fn live_simulator_info_returns_mrtd() {
        let socket = "/home/kkang/.phala-cloud/simulator/0.5.3/dstack.sock";
        let a = DstackAttestor::new(socket);
        let info = a.info().await.expect("info() should succeed against simulator");
        assert!(info.code_measurement.starts_with("0x"));
        assert_eq!(info.code_measurement.len(), 98, "0x + 96 hex chars");
        println!("simulator code measurement: {}", info.code_measurement);
    }

    #[tokio::test]
    #[ignore]
    async fn live_simulator_get_quote_returns_quote_hex() {
        let socket = "/home/kkang/.phala-cloud/simulator/0.5.3/dstack.sock";
        let a = DstackAttestor::new(socket);
        let report_data = [0u8; 32];
        let q = a.get_quote(&report_data).await.expect("get_quote() should succeed");
        assert!(!q.quote_hex.is_empty(), "quote_hex should be non-empty");
        println!("quote_hex prefix: {}", &q.quote_hex[..64]);
    }
}
