use axum::{
    extract::{rejection::JsonRejection, DefaultBodyLimit, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};

use crate::attest::{Attestor, Quote};
use crate::artifact::{artifact_hash_bytes, ScreeningArtifact};
use crate::lightwalletd::LightwalletdClient;
use crate::policy::{DepositIntent, Policy};
use crate::scan::{scan_and_screen, ScanError, ScreenRequest};

#[derive(Clone)]
pub struct AppState {
    pub client: Arc<dyn LightwalletdClient>,
    pub attestor: Arc<dyn Attestor>,
    pub scanner_code_measurement: String,
    pub scanner_network: String,
    pub scan_lock: Arc<Mutex<()>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenInput {
    pub ufvk: String,
    pub policy: Policy,
    #[serde(rename = "depositIntent")] pub deposit_intent: DepositIntent,
}

#[derive(Serialize)]
pub struct ScreenOutput {
    pub artifact: ScreeningArtifact,
    pub quote: SerializableQuote,
}

#[derive(Serialize)]
pub struct SerializableQuote {
    pub quote_hex: String,
    pub event_log: serde_json::Value,
    pub vm_config: serde_json::Value,
}

impl From<Quote> for SerializableQuote {
    fn from(q: Quote) -> Self {
        SerializableQuote {
            quote_hex: q.quote_hex,
            event_log: q.event_log,
            vm_config: q.vm_config,
        }
    }
}

#[derive(Serialize)]
struct AttestationResponse {
    code_measurement: String,
    quote: SerializableQuote,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

fn err(code: StatusCode, msg: &str) -> Response {
    (code, Json(ErrorBody { error: msg.into() })).into_response()
}

pub fn router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);
    Router::new()
        .route("/health", get(health))
        .route("/attestation", get(attestation))
        .route("/screen", post(screen))
        .layer(DefaultBodyLimit::max(16 * 1024))
        .layer(cors)
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}

async fn attestation(State(s): State<AppState>) -> Response {
    let report_data = [0u8; 32];
    match s.attestor.get_quote(&report_data).await {
        Ok(q) => Json(AttestationResponse {
            code_measurement: s.scanner_code_measurement.clone(),
            quote: SerializableQuote::from(q),
        }).into_response(),
        Err(_) => err(
            StatusCode::SERVICE_UNAVAILABLE,
            "Attestation hardware unavailable, retry.",
        ),
    }
}

async fn screen(
    State(s): State<AppState>,
    body: Result<Json<ScreenInput>, JsonRejection>,
) -> Response {
    let Json(input) = match body {
        Ok(j) => j,
        Err(_) => return err(StatusCode::BAD_REQUEST, "Policy or deposit intent is malformed."),
    };

    let guard = match s.scan_lock.try_lock() {
        Ok(g) => g,
        Err(_) => {
            return err(
                StatusCode::TOO_MANY_REQUESTS,
                "Scanner busy, retry in a moment.",
            )
        }
    };

    let req = ScreenRequest {
        ufvk_str: &input.ufvk,
        policy: &input.policy,
        deposit_intent: &input.deposit_intent,
        scanner_code_measurement: &s.scanner_code_measurement,
        scanner_network: &s.scanner_network,
    };

    let art = match scan_and_screen(req, s.client.as_ref()).await {
        Ok(a) => a,
        Err(ScanError::NetworkMismatch { .. }) => {
            return err(
                StatusCode::BAD_REQUEST,
                "Scanner network does not match the policy network.",
            )
        }
        Err(ScanError::RangeTooLarge { .. }) => {
            return err(
                StatusCode::BAD_REQUEST,
                "Scan range too large for this scanner.",
            )
        }
        Err(ScanError::RangeAboveTip { .. }) => {
            return err(
                StatusCode::BAD_REQUEST,
                "Audit range exceeds current chain tip.",
            )
        }
        Err(ScanError::IntentExpired) => {
            return err(StatusCode::BAD_REQUEST, "Deposit intent has expired.")
        }
        Err(ScanError::InvalidUfvk(_)) => {
            return err(StatusCode::BAD_REQUEST, "Viewing key could not be parsed.")
        }
        Err(ScanError::Lightwalletd(_)) => {
            return err(
                StatusCode::SERVICE_UNAVAILABLE,
                "Block source unreachable, retry.",
            )
        }
        Err(ScanError::Internal(_)) => {
            return err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal scanner error.",
            )
        }
    };

    let hash = match artifact_hash_bytes(&art) {
        Ok(h) => h,
        Err(_) => {
            return err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to canonicalize artifact.",
            )
        }
    };
    let quote = match s.attestor.get_quote(&hash).await {
        Ok(q) => q,
        Err(_) => {
            return err(
                StatusCode::SERVICE_UNAVAILABLE,
                "Attestation hardware unavailable, retry.",
            )
        }
    };

    drop(guard);
    Json(ScreenOutput {
        artifact: art,
        quote: quote.into(),
    })
    .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attest::tests::MockAttestor;
    use crate::lightwalletd::tests::MockClient;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use tower::ServiceExt;

    fn state_with_mocks() -> AppState {
        AppState {
            client: Arc::new(MockClient {
                tip: 1_000_000,
                blocks: vec![],
                raw_txs: vec![],
            }),
            attestor: Arc::new(MockAttestor {
                code_measurement: format!("0x{}", "b".repeat(96)),
                quote_hex: "QUOTE".into(),
            }),
            scanner_code_measurement: format!("0x{}", "b".repeat(96)),
            scanner_network: "testnet".into(),
            scan_lock: Arc::new(Mutex::new(())),
        }
    }

    fn screen_body(ufvk: &str, network: &str, end: u64, expiry: u64) -> String {
        serde_json::json!({
            "ufvk": ufvk,
            "policy": {
                "policyName": "demo-v1",
                "policyVersion": 1,
                "network": network,
                "auditStartHeight": 10,
                "auditEndHeight": end,
                "sanctionedAddressHashes": [],
                "expectedScannerCodeMeasurement": format!("0x{}", "b".repeat(96)),
                "createdAtUnix": 1
            },
            "depositIntent": {
                "exchangeName": "demo",
                "exchangeDepositAddress": "ztestsapling1xyz",
                "depositAmountZat": "1",
                "nonce": format!("0x{}", "0".repeat(32)),
                "expiryUnix": expiry
            }
        })
        .to_string()
    }

    fn ufvk() -> String {
        "uviewtest1".to_string() + &"a".repeat(80)
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let app = router(state_with_mocks());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn attestation_returns_quote() {
        let app = router(state_with_mocks());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/attestation")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn screen_happy_path_returns_pass_artifact_with_quote() {
        let app = router(state_with_mocks());
        let body = screen_body(&ufvk(), "testnet", 20, u64::MAX);
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/screen")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["artifact"]["result"], "PASS");
        assert!(v["quote"]["quote_hex"]
            .as_str()
            .unwrap()
            .starts_with("QUOTE"));
    }

    #[tokio::test]
    async fn screen_rejects_mainnet_policy() {
        let app = router(state_with_mocks());
        let body = screen_body(&ufvk(), "mainnet", 20, u64::MAX);
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/screen")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 400);
    }

    #[tokio::test]
    async fn screen_rejects_expired_intent() {
        let app = router(state_with_mocks());
        let body = screen_body(&ufvk(), "testnet", 20, 0);
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/screen")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 400);
    }

    #[tokio::test]
    async fn screen_rejects_range_above_tip() {
        let app = router(state_with_mocks());
        let body = screen_body(&ufvk(), "testnet", 1_000_000_000, u64::MAX);
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/screen")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 400);
    }

    #[tokio::test]
    async fn screen_rejects_malformed_json() {
        let app = router(state_with_mocks());
        let resp = app.oneshot(Request::builder()
            .method("POST").uri("/screen")
            .header("content-type", "application/json")
            .body(Body::from("{this is not json}")).unwrap()).await.unwrap();
        assert_eq!(resp.status(), 400);
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["error"], "Policy or deposit intent is malformed.");
    }
}
