//! Per-stack command construction and output parsing.

use std::process::Stdio;
use std::time::{Duration, Instant};

use adapter_proto::{
    Alert, Certificate, Direction, Evidence, HandshakeMessage, HttpOutcome, MessageField,
    Negotiated, ProbeRequest, ProbeResponse, Stage,
};
use base64::Engine;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::Config;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Backend {
    OpenSsl,
    BoringSsl,
    WolfSsl,
    Mock,
}

impl Backend {
    pub fn parse(name: &str) -> anyhow::Result<Self> {
        match name {
            "openssl" => Ok(Backend::OpenSsl),
            "boringssl" => Ok(Backend::BoringSsl),
            "wolfssl" => Ok(Backend::WolfSsl),
            "mock" => Ok(Backend::Mock),
            other => Err(anyhow::anyhow!("unknown backend {other}")),
        }
    }
}

pub async fn run(config: &Config, backend: Backend, request: &ProbeRequest) -> ProbeResponse {
    let started = Instant::now();
    let adapter = config.adapter_name();

    if backend == Backend::Mock {
        let mut response = mock(config, request);
        response.duration_ms = started.elapsed().as_millis() as u64;
        return response;
    }

    let args = match backend {
        Backend::OpenSsl => openssl_args(request),
        Backend::BoringSsl => boringssl_args(request),
        Backend::WolfSsl => wolfssl_args(request),
        Backend::Mock => unreachable!(),
    };

    let http_request = http_request_line(request);

    let mut child = match Command::new(&config.client_bin)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(child) => child,
        Err(err) => {
            return ProbeResponse::failure(
                adapter,
                Stage::Tcp,
                format!("cannot launch {}: {err}", config.client_bin),
            )
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        // Best effort: a stack that failed the handshake closes stdin under us, and that is
        // not the error worth reporting.
        let _ = stdin.write_all(http_request.as_bytes()).await;
        let _ = stdin.shutdown().await;
    }

    let timeout = Duration::from_millis(request.timeout_ms);
    let output = match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Err(_) => {
            return ProbeResponse::failure(
                adapter,
                Stage::TlsHandshake,
                format!("timed out after {} ms", request.timeout_ms),
            )
        }
        Ok(Err(err)) => {
            return ProbeResponse::failure(adapter, Stage::Tcp, format!("client failed: {err}"))
        }
        Ok(Ok(output)) => output,
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    let mut response = match backend {
        Backend::OpenSsl => parse_openssl(&adapter, request, &stdout, &stderr),
        Backend::BoringSsl => parse_boringssl(&adapter, request, &stdout, &stderr),
        Backend::WolfSsl => parse_wolfssl(&adapter, request, &stdout, &stderr),
        Backend::Mock => unreachable!(),
    };

    if request.capture.raw_records {
        let transcript = format!("$ {} {}\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}", config.client_bin, args.join(" "));
        response.evidence.uds_dump_b64 =
            Some(base64::engine::general_purpose::STANDARD.encode(transcript));
    }
    response.duration_ms = started.elapsed().as_millis() as u64;
    response
}

fn http_request_line(request: &ProbeRequest) -> String {
    let path = request.target.http_path.clone().unwrap_or_else(|| "/".into());
    let host = request.target.sni.clone().unwrap_or_else(|| request.target.host.clone());
    format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: pqcas-probe\r\nConnection: close\r\nAccept: */*\r\n\r\n")
}

fn connect_arg(request: &ProbeRequest) -> String {
    format!("{}:{}", request.target.host, request.target.port)
}

fn sni(request: &ProbeRequest) -> String {
    request.target.sni.clone().unwrap_or_else(|| request.target.host.clone())
}

fn openssl_args(request: &ProbeRequest) -> Vec<String> {
    let mut args = vec![
        "s_client".into(),
        "-connect".into(),
        connect_arg(request),
        "-servername".into(),
        sni(request),
        "-showcerts".into(),
        "-verify_return_error".into(),
        "-ign_eof".into(),
    ];
    if request.tls_versions.iter().any(|v| v == "TLSv1.3") && request.tls_versions.len() == 1 {
        args.push("-tls1_3".into());
    }
    if !request.kem_groups.is_empty() {
        args.push("-groups".into());
        args.push(request.kem_groups.join(":"));
    }
    if !request.sig_algs.is_empty() {
        args.push("-sigalgs".into());
        args.push(request.sig_algs.join(":"));
    }
    args
}

fn boringssl_args(request: &ProbeRequest) -> Vec<String> {
    let mut args = vec![
        "client".into(),
        "-connect".into(),
        connect_arg(request),
        "-server-name".into(),
        sni(request),
    ];
    if !request.kem_groups.is_empty() {
        args.push("-curves".into());
        args.push(request.kem_groups.join(":"));
    }
    args
}

fn wolfssl_args(request: &ProbeRequest) -> Vec<String> {
    let mut args = vec![
        "-h".into(),
        request.target.host.clone(),
        "-p".into(),
        request.target.port.to_string(),
        "-v".into(),
        "4".into(),
        "-S".into(),
        sni(request),
        "-d".into(),
    ];
    if let Some(group) = request.kem_groups.first() {
        args.push("--pqc".into());
        args.push(group.clone());
    }
    args
}

/// Shared shape for every parser: a ClientHello row describing what we offered, followed by
/// whatever the stack told us came back.
fn client_hello(request: &ProbeRequest) -> HandshakeMessage {
    HandshakeMessage {
        index: 0,
        direction: Direction::Outbound,
        kind: "ClientHello".into(),
        fields: vec![
            MessageField::new("Offered Groups", request.kem_groups.clone()),
            MessageField::new("Signature Algorithms", request.sig_algs.clone()),
            MessageField::new("Supported Versions", request.tls_versions.clone()),
            MessageField::new(
                "Cipher Suites Offered",
                if request.cipher_suites.is_empty() {
                    default_cipher_suites()
                } else {
                    request.cipher_suites.clone()
                },
            ),
        ],
    }
}

fn default_cipher_suites() -> Vec<String> {
    [
        "TLS_AES_128_GCM_SHA256",
        "TLS_AES_256_GCM_SHA384",
        "TLS_CHACHA20_POLY1305_SHA256",
        "ECDHE-ECDSA-AES128-GCM-SHA256",
        "ECDHE-RSA-AES128-GCM-SHA256",
        "ECDHE-ECDSA-AES256-GCM-SHA384",
        "ECDHE-RSA-AES256-GCM-SHA384",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn value_after(haystack: &str, needle: &str) -> Option<String> {
    haystack.lines().find_map(|line| {
        let idx = line.find(needle)?;
        let value = line[idx + needle.len()..].trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

/// OpenSSL reports the alert numerically; students get the RFC name.
fn alert_description(number: u16) -> &'static str {
    match number {
        0 => "close_notify",
        10 => "unexpected_message",
        20 => "bad_record_mac",
        40 => "handshake_failure",
        42 => "bad_certificate",
        47 => "illegal_parameter",
        48 => "unknown_ca",
        50 => "decode_error",
        51 => "decrypt_error",
        70 => "protocol_version",
        71 => "insufficient_security",
        80 => "internal_error",
        109 => "missing_extension",
        112 => "unrecognized_name",
        116 => "certificate_required",
        120 => "no_application_protocol",
        _ => "unknown",
    }
}

fn parse_alert(text: &str) -> Option<Alert> {
    if let Some(rest) = value_after(text, "SSL alert number ") {
        let number: u16 = rest.split_whitespace().next()?.parse().ok()?;
        return Some(Alert {
            level: "fatal".into(),
            description: alert_description(number).into(),
        });
    }
    // BoringSSL and wolfSSL print the name directly.
    for name in ["handshake_failure", "bad_record_mac", "illegal_parameter", "protocol_version"] {
        if text.contains(name) {
            return Some(Alert { level: "fatal".into(), description: name.into() });
        }
    }
    None
}

fn parse_http(text: &str) -> Option<HttpOutcome> {
    let start = text.find("HTTP/1.")?;
    let block = &text[start..];
    let mut lines = block.lines();
    let status_line = lines.next()?;
    let status: u16 = status_line.split_whitespace().nth(1)?.parse().ok()?;

    let mut headers = Vec::new();
    for line in lines.by_ref() {
        if line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.push(MessageField::single(name.trim(), value.trim()));
        }
    }
    let body: String = lines.collect::<Vec<_>>().join("\n");
    let body_excerpt = (!body.trim().is_empty()).then(|| body.chars().take(512).collect());

    Some(HttpOutcome { status, headers, body_excerpt })
}

fn parse_openssl_certs(stdout: &str) -> Vec<Certificate> {
    stdout
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix(char::is_numeric)?;
            let rest = rest.trim_start();
            let subject = rest.strip_prefix("s:")?;
            let (subject, issuer) = match subject.split_once(" i:") {
                Some((s, i)) => (s.trim(), i.trim()),
                None => (subject.trim(), ""),
            };
            Some(Certificate {
                subject: subject.to_string(),
                issuer: issuer.to_string(),
                not_before: None,
                not_after: None,
                signature_algorithm: None,
                public_key_algorithm: None,
                sha256_fingerprint: None,
            })
        })
        .collect()
}

fn byte_counts(text: &str) -> (u64, u64) {
    // "SSL handshake has read 4567 bytes and written 1234 bytes"
    let Some(line) = text.lines().find(|l| l.contains("SSL handshake has read")) else {
        return (0, 0);
    };
    let numbers: Vec<u64> = line
        .split_whitespace()
        .filter_map(|token| token.parse::<u64>().ok())
        .collect();
    match numbers.as_slice() {
        [read, written, ..] => (*written, *read),
        _ => (0, 0),
    }
}

fn parse_openssl(
    adapter: &str,
    request: &ProbeRequest,
    stdout: &str,
    stderr: &str,
) -> ProbeResponse {
    let combined = format!("{stdout}\n{stderr}");
    let tls_version = value_after(stdout, "Protocol  : ")
        .or_else(|| value_after(stdout, "Protocol: "))
        .or_else(|| {
            value_after(stdout, "New, ")
                .and_then(|rest| rest.split(',').next().map(str::trim).map(str::to_string))
        });
    let cipher_suite = value_after(stdout, "Cipher    : ")
        .or_else(|| value_after(stdout, "Cipher is "))
        .filter(|c| c != "0000");
    let group = value_after(stdout, "Negotiated TLS1.3 group: ")
        .or_else(|| value_after(stdout, "Server Temp Key: "));
    let cert_verify_sig_alg = value_after(stdout, "Peer signature type: ");

    let handshake_success = tls_version.is_some() && cipher_suite.is_some();
    let alert = parse_alert(&combined);
    let http = parse_http(stdout);
    let (sent, received) = byte_counts(stdout);

    build_response(
        adapter,
        request,
        handshake_success,
        Negotiated { tls_version, cipher_suite, group, cert_verify_sig_alg },
        alert,
        parse_openssl_certs(stdout),
        http,
        Evidence { bytes_sent: sent, bytes_received: received, ..Default::default() },
        first_error_line(&combined),
    )
}

fn parse_boringssl(
    adapter: &str,
    request: &ProbeRequest,
    stdout: &str,
    stderr: &str,
) -> ProbeResponse {
    let combined = format!("{stdout}\n{stderr}");
    let tls_version = value_after(stdout, "Version: ");
    let cipher_suite = value_after(stdout, "Cipher: ");
    let group = value_after(stdout, "ECDHE curve: ").or_else(|| value_after(stdout, "Group: "));
    let cert_verify_sig_alg = value_after(stdout, "Signature algorithm: ");
    let handshake_success = stdout.contains("Connected") && cipher_suite.is_some();

    build_response(
        adapter,
        request,
        handshake_success,
        Negotiated { tls_version, cipher_suite, group, cert_verify_sig_alg },
        parse_alert(&combined),
        Vec::new(),
        parse_http(stdout),
        Evidence::default(),
        first_error_line(&combined),
    )
}

fn parse_wolfssl(
    adapter: &str,
    request: &ProbeRequest,
    stdout: &str,
    stderr: &str,
) -> ProbeResponse {
    let combined = format!("{stdout}\n{stderr}");
    let tls_version = value_after(stdout, "SSL version is ");
    let cipher_suite = value_after(stdout, "SSL cipher suite is ");
    let group = value_after(stdout, "SSL curve name is ");
    let handshake_success = cipher_suite.is_some();

    build_response(
        adapter,
        request,
        handshake_success,
        Negotiated { tls_version, cipher_suite, group, cert_verify_sig_alg: None },
        parse_alert(&combined),
        Vec::new(),
        parse_http(stdout),
        Evidence::default(),
        first_error_line(&combined),
    )
}

/// Pick the line most likely to explain a failure, so `Error Summary` is one sentence
/// rather than a wall of client output.
fn first_error_line(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find(|line| {
            let lower = line.to_ascii_lowercase();
            lower.contains("error") || lower.contains("alert") || lower.contains("failed")
        })
        .map(|line| line.chars().take(300).collect())
}

#[allow(clippy::too_many_arguments)]
fn build_response(
    adapter: &str,
    request: &ProbeRequest,
    handshake_success: bool,
    negotiated: Negotiated,
    alert: Option<Alert>,
    cert_chain: Vec<Certificate>,
    http: Option<HttpOutcome>,
    evidence: Evidence,
    error_summary: Option<String>,
) -> ProbeResponse {
    let mut messages = vec![client_hello(request)];

    if handshake_success {
        messages.push(HandshakeMessage {
            index: 1,
            direction: Direction::Inbound,
            kind: "ServerHello".into(),
            fields: vec![
                MessageField::single(
                    "Version",
                    negotiated.tls_version.clone().unwrap_or_else(|| "unknown".into()),
                ),
                MessageField::single(
                    "Cipher Suite",
                    negotiated.cipher_suite.clone().unwrap_or_else(|| "unknown".into()),
                ),
                MessageField::single(
                    "Key Exchange Group",
                    negotiated.group.clone().unwrap_or_else(|| "unknown".into()),
                ),
            ],
        });
    } else if let Some(alert) = &alert {
        messages.push(HandshakeMessage {
            index: 1,
            direction: Direction::Inbound,
            kind: "Alert".into(),
            fields: vec![
                MessageField::single("Level", alert.level.clone()),
                MessageField::single("Description", alert.description.clone()),
            ],
        });
    }

    let failed_stage = if handshake_success {
        if http.is_some() {
            Stage::Complete
        } else {
            Stage::HttpRequest
        }
    } else {
        Stage::TlsHandshake
    };

    ProbeResponse {
        adapter: adapter.to_string(),
        handshake_success,
        negotiated,
        messages,
        alert,
        cert_chain,
        http,
        failed_stage,
        error_summary: if handshake_success { None } else { error_summary },
        diagnostics: serde_json::json!({
            "offeredGroups": request.kem_groups,
            "offeredSigAlgs": request.sig_algs,
        }),
        evidence,
        duration_ms: 0,
    }
}

/// Deterministic stand-in used by CI and the Playwright suite: a group the stack advertises
/// succeeds, anything else fails the way a real server would, and a host that looks like the
/// lab's deliberately broken target always fails with `InvalidTag`.
fn mock(config: &Config, request: &ProbeRequest) -> ProbeResponse {
    let adapter = config.adapter_name();
    let known = config.kem_group_list();
    let known_sigs = config.sig_alg_list();

    let offered_group = request.kem_groups.first().cloned().unwrap_or_default();
    let offered_sig = request.sig_algs.first().cloned().unwrap_or_default();

    let vulnerable = request.target.host.contains("vuln") || request.target.port == 8888;
    let group_ok = known.is_empty() || known.iter().any(|g| g.eq_ignore_ascii_case(&offered_group));
    let sig_ok = offered_sig.is_empty()
        || known_sigs.is_empty()
        || known_sigs.iter().any(|s| s.eq_ignore_ascii_case(&offered_sig));

    if vulnerable {
        let mut response = build_response(
            &adapter,
            request,
            false,
            Negotiated::default(),
            Some(Alert { level: "fatal".into(), description: "bad_record_mac".into() }),
            Vec::new(),
            None,
            Evidence::default(),
            Some("InvalidTag:".into()),
        );
        response.error_summary = Some("InvalidTag:".into());
        return response;
    }

    if !group_ok || !sig_ok {
        return build_response(
            &adapter,
            request,
            false,
            Negotiated::default(),
            Some(Alert { level: "fatal".into(), description: "handshake_failure".into() }),
            Vec::new(),
            None,
            Evidence::default(),
            Some(format!(
                "no shared {}",
                if group_ok { "signature algorithm" } else { "group" }
            )),
        );
    }

    build_response(
        &adapter,
        request,
        true,
        Negotiated {
            tls_version: Some("TLSv1.3".into()),
            cipher_suite: Some("TLS_AES_256_GCM_SHA384".into()),
            group: Some(offered_group),
            cert_verify_sig_alg: Some(if offered_sig.is_empty() {
                "ecdsa_secp256r1_sha256".into()
            } else {
                offered_sig
            }),
        },
        None,
        vec![Certificate {
            subject: format!("CN={}", request.target.host),
            issuer: "CN=pqcas-lab-ca".into(),
            not_before: None,
            not_after: None,
            signature_algorithm: Some("mldsa65".into()),
            public_key_algorithm: Some("ML-DSA-65".into()),
            sha256_fingerprint: None,
        }],
        Some(HttpOutcome {
            status: 200,
            headers: vec![
                MessageField::single("Server", "pqcas-mock/1.0"),
                MessageField::single("Content-Type", "application/json"),
                MessageField::single("Connection", "close"),
            ],
            body_excerpt: Some("{\"mock\":true}".into()),
        }),
        Evidence { bytes_sent: 9240, bytes_received: 1658, ..Default::default() },
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(groups: &[&str], sigs: &[&str]) -> ProbeRequest {
        let mut req = ProbeRequest::new(adapter_proto::Target {
            host: "tls-good".into(),
            port: 11000,
            sni: None,
            http_path: Some("/".into()),
        });
        req.kem_groups = groups.iter().map(|s| s.to_string()).collect();
        req.sig_algs = sigs.iter().map(|s| s.to_string()).collect();
        req
    }

    fn config(groups: &str) -> Config {
        Config {
            backend: "mock".into(),
            name: Some("openssl".into()),
            bind: "0".into(),
            client_bin: "openssl".into(),
            kem_groups: groups.into(),
            sig_algs: "mldsa65".into(),
            version: "test".into(),
        }
    }

    #[test]
    fn openssl_args_pin_groups_and_sigalgs() {
        let args = openssl_args(&request(&["X25519MLKEM768"], &["mldsa65"]));
        let joined = args.join(" ");
        assert!(joined.contains("-groups X25519MLKEM768"), "{joined}");
        assert!(joined.contains("-sigalgs mldsa65"), "{joined}");
        assert!(joined.contains("-connect tls-good:11000"), "{joined}");
    }

    #[test]
    fn openssl_success_output_is_parsed() {
        let stdout = "\
CONNECTED(00000003)
Certificate chain
 0 s:CN=lab.example i:CN=pqcas-lab-ca
---
SSL handshake has read 4567 bytes and written 1234 bytes
New, TLSv1.3, Cipher is TLS_AES_256_GCM_SHA384
Negotiated TLS1.3 group: X25519MLKEM768
Peer signature type: mldsa65
SSL-Session:
    Protocol  : TLSv1.3
    Cipher    : TLS_AES_256_GCM_SHA384
HTTP/1.1 200 OK
Server: nginx/1.28.1
Content-Type: application/json

{}
";
        let response = parse_openssl("openssl", &request(&["X25519MLKEM768"], &["mldsa65"]), stdout, "");
        assert!(response.handshake_success);
        assert_eq!(response.negotiated.group.as_deref(), Some("X25519MLKEM768"));
        assert_eq!(response.negotiated.cipher_suite.as_deref(), Some("TLS_AES_256_GCM_SHA384"));
        assert_eq!(response.failed_stage, Stage::Complete);
        assert_eq!(response.http.as_ref().unwrap().status, 200);
        assert_eq!(response.evidence.bytes_sent, 1234);
        assert_eq!(response.evidence.bytes_received, 4567);
        assert_eq!(response.cert_chain.len(), 1);
    }

    #[test]
    fn openssl_alert_number_becomes_a_name() {
        let stderr = "40D0F3:error:0A000410:SSL routines:ssl3_read_bytes:sslv3 alert handshake failure:ssl/record/rec_layer_s3.c:907:SSL alert number 40";
        let response = parse_openssl("openssl", &request(&["P-256"], &["mldsa44"]), "CONNECTED(3)\n", stderr);
        assert!(!response.handshake_success);
        assert_eq!(response.alert.as_ref().unwrap().description, "handshake_failure");
        assert_eq!(response.failed_stage, Stage::TlsHandshake);
        assert_eq!(response.messages[1].kind, "Alert");
    }

    #[test]
    fn mock_matches_the_lab_targets() {
        let config = config("X25519MLKEM768,MLKEM768");
        let ok = mock(&config, &request(&["X25519MLKEM768"], &["mldsa65"]));
        assert!(ok.handshake_success);
        assert_eq!(ok.evidence.bytes_sent, 9240);

        let unknown_group = mock(&config, &request(&["mceliece460896"], &["mldsa65"]));
        assert!(!unknown_group.handshake_success);

        let mut vuln = request(&["X25519MLKEM768"], &["mldsa65"]);
        vuln.target.host = "tls-vuln".into();
        vuln.target.port = 8888;
        let broken = mock(&config, &vuln);
        assert_eq!(broken.error_summary.as_deref(), Some("InvalidTag:"));
    }

    #[test]
    fn http_block_parsing_stops_at_the_blank_line() {
        let http = parse_http("HTTP/1.1 404 Not Found\r\nServer: nginx/1.28.1\r\n\r\nbody here").unwrap();
        assert_eq!(http.status, 404);
        assert_eq!(http.headers.len(), 1);
        assert_eq!(http.body_excerpt.as_deref(), Some("body here"));
    }
}
