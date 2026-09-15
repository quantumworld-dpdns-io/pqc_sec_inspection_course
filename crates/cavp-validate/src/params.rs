//! FIPS 204 (ML-DSA) parameter-set constants.
//!
//! Sizes are fixed by the standard, so they are the cheapest possible check on a
//! submitted answer: a wrong-length `pk`/`sk`/`signature` is reported before we even look
//! at the bytes, which gives students a far more useful error than "mismatch".

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ParameterSet {
    pub name: &'static str,
    pub pk_len: usize,
    pub sk_len: usize,
    pub sig_len: usize,
    /// Seed length for `keyGen` (xi).
    pub seed_len: usize,
}

pub const ML_DSA_44: ParameterSet = ParameterSet {
    name: "ML-DSA-44",
    pk_len: 1312,
    sk_len: 2560,
    sig_len: 2420,
    seed_len: 32,
};

pub const ML_DSA_65: ParameterSet = ParameterSet {
    name: "ML-DSA-65",
    pk_len: 1952,
    sk_len: 4032,
    sig_len: 3309,
    seed_len: 32,
};

pub const ML_DSA_87: ParameterSet = ParameterSet {
    name: "ML-DSA-87",
    pk_len: 2592,
    sk_len: 4896,
    sig_len: 4627,
    seed_len: 32,
};

pub const ALL: [ParameterSet; 3] = [ML_DSA_44, ML_DSA_65, ML_DSA_87];

pub fn lookup(name: &str) -> Option<ParameterSet> {
    ALL.into_iter().find(|p| p.name.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_match_fips204() {
        assert_eq!(lookup("ML-DSA-65").unwrap().pk_len, 1952);
        assert_eq!(lookup("ml-dsa-87").unwrap().sig_len, 4627);
        assert!(lookup("ML-KEM-768").is_none());
    }
}
