//! Wire contract between the PQ-CAS worker and the containerised TLS prober adapters.
//!
//! Every adapter image (openssl/boringssl/wolfssl/go) exposes the same three endpoints:
//!
//! * `GET  /healthz`       -> 200 when the process is alive
//! * `GET  /capabilities`  -> [`Capabilities`], used to decide `unsupported` vs `disabled`
//! * `POST /probe`         -> [`ProbeRequest`] in, [`ProbeResponse`] out
//!
//! The types live in their own crate so the Rust adapters, the worker and the API all
//! serialise exactly the same JSON; the Go adapter mirrors these structs by hand and is
//! kept honest by the conformance tests.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const HEADER_REQUEST_ID: &str = "x-pqcas-request-id";

/// Where the handshake should be aimed.
#[derive(Clone, Debug, Default, Deserialize, Serialize, ToSchema)]
pub struct Target {
    pub host: String,
    pub port: u16,
    /// SNI to present; defaults to `host` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sni: Option<String>,
    /// Request path for the optional post-handshake `GET`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_path: Option<String>,
}

/// What evidence the caller wants retained. Raw record capture is expensive, so the
/// worker only asks for it on failing subtasks and on explicit re-runs.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, ToSchema)]
pub struct CaptureOptions {
    #[serde(default)]
    pub raw_records: bool,
    #[serde(default)]
    pub keylog: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct ProbeRequest {
    pub target: Target,
    /// TLS `key_share` / `supported_groups` entries to offer, in preference order.
    #[serde(default)]
    pub kem_groups: Vec<String>,
    /// `signature_algorithms` entries to offer.
    #[serde(default)]
    pub sig_algs: Vec<String>,
    #[serde(default = "default_tls_versions")]
    pub tls_versions: Vec<String>,
    #[serde(default)]
    pub cipher_suites: Vec<String>,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub capture: CaptureOptions,
}

fn default_tls_versions() -> Vec<String> {
    vec!["TLSv1.3".to_string(), "TLSv1.2".to_string()]
}

fn default_timeout_ms() -> u64 {
    15_000
}

impl ProbeRequest {
    pub fn new(target: Target) -> Self {
        Self {
            target,
            kem_groups: Vec::new(),
            sig_algs: Vec::new(),
            tls_versions: default_tls_versions(),
            cipher_suites: Vec::new(),
            timeout_ms: default_timeout_ms(),
            capture: CaptureOptions::default(),
        }
    }
}

/// The stage a probe got to before it stopped. Mirrors the `Failed Stage` row in the UI.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Dns,
    Tcp,
    TlsHandshake,
    HttpRequest,
    Complete,
}

impl Stage {
    pub fn as_str(self) -> &'static str {
        match self {
            Stage::Dns => "dns",
            Stage::Tcp => "tcp",
            Stage::TlsHandshake => "tls_handshake",
            Stage::HttpRequest => "http_request",
            Stage::Complete => "complete",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Client -> Server
    Outbound,
    /// Client <- Server
    Inbound,
}

/// One handshake record rendered as a row of the subtask trace.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct HandshakeMessage {
    pub index: u32,
    pub direction: Direction,
    /// `ClientHello`, `ServerHello`, `Alert`, `EncryptedExtensions`, ...
    pub kind: String,
    /// Ordered label/value pairs shown underneath the message chip.
    #[serde(default)]
    pub fields: Vec<MessageField>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct MessageField {
    pub label: String,
    pub values: Vec<String>,
}

impl MessageField {
    pub fn new(label: impl Into<String>, values: Vec<String>) -> Self {
        Self {
            label: label.into(),
            values,
        }
    }

    pub fn single(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            values: vec![value.into()],
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct Alert {
    pub level: String,
    pub description: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, ToSchema)]
pub struct Negotiated {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cipher_suite: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cert_verify_sig_alg: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct Certificate {
    pub subject: String,
    pub issuer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature_algorithm: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_key_algorithm: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256_fingerprint: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct HttpOutcome {
    pub status: u16,
    #[serde(default)]
    pub headers: Vec<MessageField>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_excerpt: Option<String>,
}

/// Raw bytes kept for the `UDS Dump / Receiver Evidence` panel.
#[derive(Clone, Debug, Default, Deserialize, Serialize, ToSchema)]
pub struct Evidence {
    #[serde(default)]
    pub bytes_sent: u64,
    #[serde(default)]
    pub bytes_received: u64,
    /// base64 of the raw record capture, only when `capture.raw_records` was set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uds_dump_b64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keylog: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct ProbeResponse {
    pub adapter: String,
    pub handshake_success: bool,
    #[serde(default)]
    pub negotiated: Negotiated,
    #[serde(default)]
    pub messages: Vec<HandshakeMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alert: Option<Alert>,
    #[serde(default)]
    pub cert_chain: Vec<Certificate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http: Option<HttpOutcome>,
    pub failed_stage: Stage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_summary: Option<String>,
    /// Free-form adapter internals surfaced in the `Diagnostics` collapsible.
    #[serde(default)]
    pub diagnostics: serde_json::Value,
    #[serde(default)]
    pub evidence: Evidence,
    pub duration_ms: u64,
}

impl ProbeResponse {
    /// A response for something that never reached the wire (adapter refused the request,
    /// container unreachable, timeout in the worker).
    pub fn failure(adapter: impl Into<String>, stage: Stage, error: impl Into<String>) -> Self {
        Self {
            adapter: adapter.into(),
            handshake_success: false,
            negotiated: Negotiated::default(),
            messages: Vec::new(),
            alert: None,
            cert_chain: Vec::new(),
            http: None,
            failed_stage: stage,
            error_summary: Some(error.into()),
            diagnostics: serde_json::Value::Null,
            evidence: Evidence::default(),
            duration_ms: 0,
        }
    }
}

/// Advertised by each adapter so the UI can distinguish "this stack cannot do that
/// algorithm" (`unsupported`) from "we did not build that adapter" (`disabled`).
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct Capabilities {
    pub adapter: String,
    pub version: String,
    #[serde(default)]
    pub kem_groups: Vec<String>,
    #[serde(default)]
    pub sig_algs: Vec<String>,
    #[serde(default)]
    pub tls_versions: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_request_defaults_are_applied() {
        let req: ProbeRequest =
            serde_json::from_str(r#"{"target":{"host":"example.test","port":443}}"#)
                .expect("minimal request parses");
        assert_eq!(req.timeout_ms, 15_000);
        assert_eq!(req.tls_versions, vec!["TLSv1.3", "TLSv1.2"]);
        assert!(!req.capture.raw_records);
    }

    #[test]
    fn stage_round_trips_as_snake_case() {
        let json = serde_json::to_string(&Stage::TlsHandshake).unwrap();
        assert_eq!(json, "\"tls_handshake\"");
        assert_eq!(Stage::TlsHandshake.as_str(), "tls_handshake");
    }
}
