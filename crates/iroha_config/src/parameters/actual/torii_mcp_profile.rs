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
/// Native MCP configuration exposed by Torii.
#[derive(Debug, Clone)]
pub struct ToriiMcp {
    /// Master enable switch for `/v1/mcp`.
    pub enabled: bool,
    /// Maximum accepted request payload size in bytes.
    pub max_request_bytes: usize,
    /// Maximum number of tools emitted in one `tools/list` response page.
    pub max_tools_per_list: usize,
    /// Maximum number of MCP tool dispatches executing concurrently.
    pub max_inflight_dispatches: NonZeroUsize,
    /// MCP tool profile.
    pub profile: ToriiMcpProfile,
    /// Expose operator-only routes in the MCP tool registry.
    pub expose_operator_routes: bool,
    /// Additional allow-list prefixes for tool names (empty => profile-only).
    pub allow_tool_prefixes: Vec<String>,
    /// Additional deny-list prefixes for tool names.
    pub deny_tool_prefixes: Vec<String>,
    /// Optional steady-state MCP request budget (requests per minute).
    pub rate_per_minute: Option<NonZeroU32>,
    /// Optional MCP burst budget.
    pub burst: Option<NonZeroU32>,
}
/// Torii CORS response-header policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToriiCors {
    /// Enable CORS response headers.
    pub enabled: bool,
    /// Explicit browser origins allowed to make cross-origin requests.
    pub allowed_origins: Vec<String>,
    /// Explicit HTTP methods allowed in CORS preflight responses.
    pub allowed_methods: Vec<String>,
    /// Explicit request headers allowed in CORS preflight responses.
    pub allowed_headers: Vec<String>,
    /// Explicit response headers exposed to browser clients.
    pub exposed_headers: Vec<String>,
    /// Maximum preflight cache age in seconds.
    pub max_age_secs: u64,
}
impl Default for ToriiCors {
    fn default() -> Self {
        Self {
            enabled: defaults::torii::cors::ENABLED,
            allowed_origins: defaults::torii::cors::allowed_origins(),
            allowed_methods: defaults::torii::cors::allowed_methods(),
            allowed_headers: defaults::torii::cors::allowed_headers(),
            exposed_headers: defaults::torii::cors::exposed_headers(),
            max_age_secs: defaults::torii::cors::MAX_AGE_SECS,
        }
    }
}
impl From<user::ToriiCors> for ToriiCors {
    fn from(value: user::ToriiCors) -> Self {
        Self {
            enabled: value.enabled,
            allowed_origins: value.allowed_origins,
            allowed_methods: value.allowed_methods,
            allowed_headers: value.allowed_headers,
            exposed_headers: value.exposed_headers,
            max_age_secs: value.max_age_secs,
        }
    }
}
