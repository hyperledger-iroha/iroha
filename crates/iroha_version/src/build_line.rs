//! Build-line selectors used to distinguish Iroha 2 vs Iroha 3 binaries.

use core::fmt::{self, Display, Formatter};

/// Release line for the compiled binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildLine {
    /// Iroha 2 (self-hosted deployments without Nexus/Sora features).
    Iroha2,
    /// Iroha 3 / Nexus (multilane, `SoraFS`, `SoraNet`).
    Iroha3,
}

impl BuildLine {
    /// Derive the build line from the binary name. Any binary starting with
    /// `iroha2` is treated as an Iroha 2 artefact; everything else defaults to
    /// the Iroha 3 line.
    #[must_use]
    pub fn from_bin_name(bin_name: &str) -> Self {
        let lowered = bin_name.trim().to_ascii_lowercase();
        if lowered.starts_with("iroha2") {
            Self::Iroha2
        } else {
            Self::Iroha3
        }
    }

    /// Canonical string identifier for the build line.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Iroha2 => "iroha2",
            Self::Iroha3 => "iroha3",
        }
    }

    /// Default CLI binary name associated with this line.
    #[must_use]
    pub const fn cli_bin(self) -> &'static str {
        match self {
            Self::Iroha2 => "iroha2",
            Self::Iroha3 => "iroha3",
        }
    }

    /// Default daemon binary name associated with this line.
    #[must_use]
    pub const fn daemon_bin(self) -> &'static str {
        match self {
            Self::Iroha2 => "iroha2d",
            Self::Iroha3 => "iroha3d",
        }
    }

    /// Whether this selector targets the Iroha 2 line.
    #[must_use]
    pub const fn is_iroha2(self) -> bool {
        matches!(self, Self::Iroha2)
    }

    /// Whether this selector targets the Iroha 3 line.
    #[must_use]
    pub const fn is_iroha3(self) -> bool {
        matches!(self, Self::Iroha3)
    }
}

impl Display for BuildLine {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::BuildLine;

    #[test]
    fn detects_iroha2_bins() {
        assert_eq!(BuildLine::from_bin_name("iroha2"), BuildLine::Iroha2);
        assert_eq!(BuildLine::from_bin_name("iroha2d"), BuildLine::Iroha2);
        assert_eq!(BuildLine::from_bin_name("IROHA2-cli"), BuildLine::Iroha2);
    }

    #[test]
    fn defaults_to_iroha3() {
        assert_eq!(BuildLine::from_bin_name("iroha3d"), BuildLine::Iroha3);
        assert_eq!(BuildLine::from_bin_name("iroha"), BuildLine::Iroha3);
        assert_eq!(BuildLine::from_bin_name("custom"), BuildLine::Iroha3);
    }

    #[test]
    fn string_helpers_match() {
        let i2 = BuildLine::Iroha2;
        let i3 = BuildLine::Iroha3;

        assert_eq!(i2.as_str(), "iroha2");
        assert_eq!(i3.as_str(), "iroha3");
        assert_eq!(i2.cli_bin(), "iroha2");
        assert_eq!(i3.cli_bin(), "iroha3");
        assert_eq!(i2.daemon_bin(), "iroha2d");
        assert_eq!(i3.daemon_bin(), "iroha3d");
        assert!(i2.is_iroha2());
        assert!(i3.is_iroha3());
    }
}
