//! Types shared by the PQ-CAS API, the worker and the tests.

pub mod jobs;
pub mod models;
pub mod status;

pub use adapter_proto as proto;
pub use jobs::*;
pub use models::*;
pub use status::*;

/// Map an adapter probe result onto the status vocabulary the matrix paints with.
///
/// `unsupported` is deliberately distinct from `failed`: a stack that never offered the
/// algorithm has not told us anything about the target, and the UI greys it out instead of
/// showing a red cross.
pub fn classify(
    response: &adapter_proto::ProbeResponse,
    offered: &[String],
    capabilities: &adapter_proto::Capabilities,
) -> ResultStatus {
    let unsupported = offered
        .iter()
        .all(|alg| !capabilities.kem_groups.contains(alg) && !capabilities.sig_algs.contains(alg));
    if unsupported {
        return ResultStatus::Unsupported;
    }
    if response.handshake_success {
        ResultStatus::Passed
    } else {
        ResultStatus::Failed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adapter_proto::{Capabilities, ProbeResponse, Stage};

    fn caps() -> Capabilities {
        Capabilities {
            adapter: "openssl".into(),
            version: "3.5.0".into(),
            kem_groups: vec!["X25519MLKEM768".into()],
            sig_algs: vec!["mldsa65".into()],
            tls_versions: vec!["TLSv1.3".into()],
        }
    }

    #[test]
    fn algorithms_the_adapter_never_heard_of_are_unsupported() {
        let response = ProbeResponse::failure("openssl", Stage::TlsHandshake, "no shared group");
        let status = classify(&response, &["mceliece460896".to_string()], &caps());
        assert_eq!(status, ResultStatus::Unsupported);
    }

    #[test]
    fn known_algorithm_that_fails_is_a_real_failure() {
        let response = ProbeResponse::failure("openssl", Stage::TlsHandshake, "handshake_failure");
        let status = classify(&response, &["X25519MLKEM768".to_string()], &caps());
        assert_eq!(status, ResultStatus::Failed);
    }
}
