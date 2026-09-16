//! Status vocabularies. These are Postgres enums as well as API values, so the SQL type
//! names below must match `services/api/migrations/0001_init.sql`.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

macro_rules! pg_enum {
    ($(#[$meta:meta])* $name:ident, $sql:literal, { $($variant:ident => $wire:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, ToSchema, sqlx::Type)]
        #[sqlx(type_name = $sql, rename_all = "snake_case")]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire),+
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl std::str::FromStr for $name {
            type Err = String;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($wire => Ok(Self::$variant),)+
                    other => Err(format!("unknown {}: {other}", stringify!($name))),
                }
            }
        }
    };
}

pg_enum!(
    /// Lifecycle of a whole test task (the chip next to the host:port heading).
    TaskStatus, "task_status", {
        Pending => "pending",
        Running => "running",
        Finished => "finished",
        Failed => "failed",
        Cancelled => "cancelled",
    }
);

pg_enum!(
    /// Did the subtask get to run at all.
    ExecStatus, "exec_status", {
        Queued => "queued",
        Running => "running",
        Finished => "finished",
        Error => "error",
    }
);

pg_enum!(
    /// Did the thing under test behave. Drives the matrix cell colour.
    ResultStatus, "result_status", {
        Pending => "pending",
        Passed => "passed",
        Failed => "failed",
        Unsupported => "unsupported",
        Disabled => "disabled",
    }
);

pg_enum!(
    /// Whether a full report blob was captured for the subtask.
    ReportStatus, "report_status", {
        Missing => "missing",
        Partial => "partial",
        Complete => "complete",
    }
);

pg_enum!(
    /// Which flavour of KEM DEMO run this is.
    KemRunMode, "kem_run_mode", {
        LibraryCompat => "library_compat",
        GroupCompat => "group_compat",
        Priority => "priority",
    }
);

pg_enum!(
    /// ACVP capability under exercise in the CAVP module.
    Capability, "cavp_capability", {
        KeyGen => "key_gen",
        SigGen => "sig_gen",
        SigVer => "sig_ver",
    }
);

impl Capability {
    /// The spelling ACVP itself uses in `prompt.json`.
    pub fn acvp_mode(self) -> &'static str {
        match self {
            Capability::KeyGen => "keyGen",
            Capability::SigGen => "sigGen",
            Capability::SigVer => "sigVer",
        }
    }

    pub fn from_acvp_mode(mode: &str) -> Option<Self> {
        match mode {
            "keyGen" => Some(Capability::KeyGen),
            "sigGen" => Some(Capability::SigGen),
            "sigVer" => Some(Capability::SigVer),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_maps_to_acvp_spelling() {
        assert_eq!(Capability::SigGen.acvp_mode(), "sigGen");
        assert_eq!(
            Capability::from_acvp_mode("sigVer"),
            Some(Capability::SigVer)
        );
        assert_eq!(Capability::from_acvp_mode("nope"), None);
    }

    #[test]
    fn statuses_round_trip_through_strings() {
        for status in [
            ResultStatus::Passed,
            ResultStatus::Unsupported,
            ResultStatus::Disabled,
        ] {
            assert_eq!(status.as_str().parse::<ResultStatus>().unwrap(), status);
        }
    }
}
