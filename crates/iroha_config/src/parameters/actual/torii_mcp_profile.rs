/// Native MCP tool profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToriiMcpProfile {
    /// Read-only profile for AI agents (status/query/list/get style tools).
    #[default]
    ReadOnly,
    /// Writer profile (includes non-operator mutating tools).
    Writer,
    /// Operator profile (includes operator-only routes when exposed).
    Operator,
}
impl ToriiMcpProfile {
    /// Parse a user-provided profile label.
    pub fn parse(label: &str) -> Option<Self> {
        match label.trim().to_ascii_lowercase().as_str() {
            "read_only" | "readonly" | "read-only" => Some(Self::ReadOnly),
            "writer" | "write" => Some(Self::Writer),
            "operator" | "ops" => Some(Self::Operator),
            _ => None,
        }
    }
    /// Canonical label for configuration dumps.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::Writer => "writer",
            Self::Operator => "operator",
        }
    }
}
