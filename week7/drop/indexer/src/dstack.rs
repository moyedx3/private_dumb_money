//! dstack UDS client — talks to the dstack guest agent over a unix socket.
//! Ports the HTTP-over-UDS pattern from clean-wallet `attest.rs` (/GetQuote, /Info).

use anyhow::{anyhow, Result};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream as StdUnixStream;

/// Pack a 32-byte report_data into the 64-byte TDX quote slot (zero-padded).
pub fn pad_report_data(rd: &[u8; 32]) -> [u8; 64] {
    let mut p = [0u8; 64];
    p[..32].copy_from_slice(rd);
    p
}

/// Extract the `quote` hex field from a dstack `/GetQuote` JSON response.
fn parse_quote(resp: &serde_json::Value) -> Result<String> {
    resp.get("quote")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("dstack /GetQuote: missing 'quote' field"))
}

/// Thin client over the dstack guest-agent unix socket.
pub struct Dstack {
    pub socket: String,
}

impl Dstack {
    pub fn new(socket: impl Into<String>) -> Self {
        Self { socket: socket.into() }
    }

    /// Request a TDX quote binding `report_data` (zero-padded to 64 bytes). Returns the quote hex.
    pub async fn get_quote(&self, report_data: &[u8; 32]) -> Result<String> {
        let body = serde_json::json!({ "report_data": hex::encode(pad_report_data(report_data)) });
        let resp = post_uds_json(&self.socket, "/GetQuote", &body).await?;
        parse_quote(&resp)
    }

    /// Read the enclave's code measurement (MRTD) from `/Info`.
    pub async fn info_mrtd(&self) -> Result<String> {
        let resp = post_uds_json(&self.socket, "/Info", &serde_json::json!({})).await?;
        // dstack quirk: `tcb_info` is a JSON-encoded *string*, not an object.
        let tcb = resp.get("tcb_info").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("dstack /Info: missing 'tcb_info'"))?;
        let tcb: serde_json::Value = serde_json::from_str(tcb)?;
        tcb.get("mrtd").and_then(|v| v.as_str()).map(|s| s.to_string())
            .ok_or_else(|| anyhow!("dstack /Info: missing 'mrtd'"))
    }

    /// Derive a stable, app-bound 32-byte secret from the dstack KMS (`/GetKey`; confirmed
    /// against the v0.5.3 simulator — response `{ key: <64-hex>, signature_chain: [...] }`).
    /// Stable per measurement → used as the provisioning keypair seed. Changes on rebuild (C4).
    pub async fn get_key(&self, path: &str) -> Result<[u8; 32]> {
        let resp = post_uds_json(&self.socket, "/GetKey", &serde_json::json!({ "path": path })).await?;
        let hexk = resp.get("key").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("dstack /GetKey: missing 'key'"))?;
        let raw = hex::decode(hexk)?;
        raw.as_slice().try_into()
            .map_err(|_| anyhow!("dstack key not 32 bytes (got {})", raw.len()))
    }
}

/// Minimal HTTP/1.1 POST with a JSON body over a unix socket; parse the JSON response.
async fn post_uds_json(socket: &str, path: &str, body: &serde_json::Value) -> Result<serde_json::Value> {
    let socket = socket.to_string();
    let path = path.to_string();
    let body = body.to_string();
    tokio::task::spawn_blocking(move || -> Result<serde_json::Value> {
        let mut s = StdUnixStream::connect(&socket)?;
        let req = format!(
            "POST {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        s.write_all(req.as_bytes())?;
        let mut buf = String::new();
        s.read_to_string(&mut buf)?;
        let start = buf.find("\r\n\r\n").ok_or_else(|| anyhow!("no body in dstack response"))? + 4;
        Ok(serde_json::from_str(&buf[start..])?)
    })
    .await?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_data_is_zero_padded_to_64() {
        let rd = [9u8; 32];
        let padded = pad_report_data(&rd);
        assert_eq!(padded.len(), 64);
        assert_eq!(&padded[..32], &rd);
        assert_eq!(&padded[32..], &[0u8; 32]);
    }

    #[test]
    fn parse_quote_extracts_field_or_errors() {
        let ok = serde_json::json!({ "quote": "deadbeef", "x": 1 });
        assert_eq!(parse_quote(&ok).unwrap(), "deadbeef");
        let bad = serde_json::json!({ "nope": 1 });
        assert!(parse_quote(&bad).is_err());
    }

    /// Live check against the local dstack simulator. Run with:
    ///   DSTACK_SOCKET=~/.phala-cloud/simulator/<ver>/dstack.sock \
    ///   cargo test -p drop-indexer -- --ignored dstack::tests::live
    #[tokio::test]
    #[ignore]
    async fn live_simulator_returns_quote() {
        let sock = std::env::var("DSTACK_SOCKET").expect("set DSTACK_SOCKET to the simulator socket");
        let ds = Dstack::new(sock);
        let q = ds.get_quote(&[1u8; 32]).await.unwrap();
        assert!(!q.is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn live_simulator_derives_stable_key() {
        let sock = std::env::var("DSTACK_SOCKET").expect("set DSTACK_SOCKET to the simulator socket");
        let ds = Dstack::new(sock);
        let a = ds.get_key("drop/provisioning").await.unwrap();
        let b = ds.get_key("drop/provisioning").await.unwrap();
        assert_eq!(a, b); // stable per measurement
        assert_ne!(a, [0u8; 32]); // actually derived
    }
}
