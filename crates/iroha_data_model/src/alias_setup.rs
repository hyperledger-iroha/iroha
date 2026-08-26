//! Declarative alias/SNS setup names, intents, plans, and diagnostics.
//!
//! Textual names in this module are catalog-free.  Resolution pins the canonical
//! text to the numeric dataspace identifier that consensus must revalidate when
//! executing an alias instruction.
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    NetworkId,
    account::{
        AccountId,
        rekey::{AccountAlias, AccountAliasDomain},
    },
    asset::AssetDefinitionId,
    domain::DomainId,
    error::ParseError,
    name::{self, Name},
    nexus::{DataSpaceCatalog, DataSpaceId},
};
use core::{fmt, str::FromStr};
use iroha_crypto::{Hash, HashOf};
use iroha_primitives::numeric::Quantity;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use std::{string::String, vec::Vec};
/// Domain separator for [`AliasTransactionPlanBodyV1`] commitments.
pub const ALIAS_TRANSACTION_PLAN_HASH_DOMAIN_V1: &[u8] = b"iroha:alias-transaction-plan-body:v1\0";
/// Domain separator for [`AliasLifecycleTransactionPlanBodyV1`] commitments.
pub const ALIAS_LIFECYCLE_TRANSACTION_PLAN_HASH_DOMAIN_V1: &[u8] =
    b"iroha:alias-lifecycle-transaction-plan-body:v1\0";
/// Deterministic duration of one SNS lease year in milliseconds (365 days).
pub const ALIAS_LEASE_YEAR_MS: u64 = 31_536_000_000;
/// Catalog-free textual account alias.
///
/// `merchant@banka.paynet` has label `merchant`, domain `banka`, and
/// dataspace `paynet`; `merchant@paynet` has no domain segment.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
#[norito(decode_from_slice)]
pub struct AccountAliasName {
    /// Canonical alias label.
    pub label: Name,
    /// Optional canonical domain label inside the dataspace.
    #[norito(required)]
    pub domain: Option<Name>,
    /// Canonical textual dataspace name.
    pub dataspace: Name,
}
impl AccountAliasName {
    /// Parse canonical alias components without consulting a dataspace catalog.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if any segment is empty or invalid.
    pub fn try_new(
        label: impl AsRef<str>,
        domain: Option<impl AsRef<str>>,
        dataspace: impl AsRef<str>,
    ) -> Result<Self, ParseError> {
        let label = canonical_alias_segment(label.as_ref(), AliasSegment::Label)?;
        let domain = domain
            .map(|value| canonical_alias_segment(value.as_ref(), AliasSegment::Domain))
            .transpose()?;
        let dataspace = canonical_alias_segment(dataspace.as_ref(), AliasSegment::Dataspace)?;
        Ok(Self {
            label,
            domain,
            dataspace,
        })
    }
    /// Return the canonical alias literal.
    #[must_use]
    pub fn canonical_text(&self) -> String {
        self.to_string()
    }
    /// Resolve the optional domain portion into its fully qualified textual ID.
    #[must_use]
    pub fn domain_id(&self) -> Option<DomainId> {
        self.domain
            .as_ref()
            .and_then(|domain| DomainId::try_new(domain, &self.dataspace).ok())
    }
    /// Check that directly decoded or constructed fields are in canonical form.
    #[must_use]
    pub fn is_canonical(&self) -> bool {
        self.to_string()
            .parse::<Self>()
            .is_ok_and(|value| value == *self)
    }
}
impl fmt::Display for AccountAliasName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@", self.label)?;
        if let Some(domain) = &self.domain {
            write!(f, "{domain}.")?;
        }
        write!(f, "{}", self.dataspace)
    }
}
impl FromStr for AccountAliasName {
    type Err = ParseError;
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if input.is_empty() {
            return Err(ParseError::new("account alias must not be empty"));
        }
        if input.trim() != input {
            return Err(ParseError::new(
                "account alias must not contain leading or trailing whitespace",
            ));
        }
        if input.chars().any(char::is_control) {
            return Err(ParseError::new(
                "account alias must not contain control characters",
            ));
        }
        let (label, scope) = input.split_once('@').ok_or_else(|| {
            ParseError::new(
                "account alias must use `name@domain.dataspace` or `name@dataspace` format",
            )
        })?;
        if scope.contains('@') {
            return Err(ParseError::new(
                "account alias must contain exactly one `@` separator",
            ));
        }
        if label.is_empty() {
            return Err(ParseError::new(
                "account alias label segment must not be empty",
            ));
        }
        if scope.is_empty() {
            return Err(ParseError::new(
                "account alias dataspace segment must not be empty",
            ));
        }
        match scope.bytes().filter(|byte| *byte == b'.').count() {
            0 => Self::try_new(label, Option::<&str>::None, scope),
            1 => {
                let (domain, dataspace) = scope
                    .split_once('.')
                    .expect("validated alias scope contains one dot");
                if domain.is_empty() || dataspace.is_empty() {
                    return Err(ParseError::new(
                        "account alias domain and dataspace segments must not be empty",
                    ));
                }
                Self::try_new(label, Some(domain), dataspace)
            }
            _ => Err(ParseError::new(
                "account alias must contain at most one `.` after `@`",
            )),
        }
    }
}
#[derive(Clone, Copy)]
enum AliasSegment {
    Label,
    Domain,
    Dataspace,
}
fn canonical_alias_segment(raw: &str, segment: AliasSegment) -> Result<Name, ParseError> {
    if raw.is_empty() || raw.contains('.') {
        return Err(match segment {
            AliasSegment::Label => ParseError::new("account alias label segment is invalid"),
            AliasSegment::Domain => ParseError::new("account alias domain segment is invalid"),
            AliasSegment::Dataspace => {
                ParseError::new("account alias dataspace segment is invalid")
            }
        });
    }
    let canonical = name::canonicalize_domain_label(raw).map_err(|_| match segment {
        AliasSegment::Label => ParseError::new("account alias label segment is invalid"),
        AliasSegment::Domain => ParseError::new("account alias domain segment is invalid"),
        AliasSegment::Dataspace => ParseError::new("account alias dataspace segment is invalid"),
    })?;
    canonical.parse().map_err(|_| match segment {
        AliasSegment::Label => ParseError::new("account alias label segment is invalid"),
        AliasSegment::Domain => ParseError::new("account alias domain segment is invalid"),
        AliasSegment::Dataspace => ParseError::new("account alias dataspace segment is invalid"),
    })
}
/// Canonical dataspace text paired with the numeric ID expected by the caller.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct ResolvedDataSpaceV1 {
    /// Canonical textual dataspace name.
    pub canonical_name: Name,
    /// Numeric dataspace ID that the text must resolve to at execution.
    pub dataspace_id: DataSpaceId,
}
impl ResolvedDataSpaceV1 {
    /// Construct a resolved dataspace pair.
    #[must_use]
    pub const fn new(canonical_name: Name, dataspace_id: DataSpaceId) -> Self {
        Self {
            canonical_name,
            dataspace_id,
        }
    }
    /// Resolve a textual dataspace using a static catalog.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if the name is malformed or absent from the catalog.
    pub fn resolve_catalog(input: &str, catalog: &DataSpaceCatalog) -> Result<Self, ParseError> {
        let canonical = canonical_alias_segment(input, AliasSegment::Dataspace)?;
        let dataspace_id = catalog
            .by_alias(canonical.as_ref())
            .map(|entry| entry.id)
            .ok_or_else(|| ParseError::new("unknown dataspace alias"))?;
        Ok(Self::new(canonical, dataspace_id))
    }
    /// Return whether the textual name still maps to the pinned numeric ID.
    #[must_use]
    pub fn matches_catalog(&self, catalog: &DataSpaceCatalog) -> bool {
        catalog
            .by_alias(self.canonical_name.as_ref())
            .is_some_and(|entry| entry.id == self.dataspace_id)
    }
    /// Return the canonical textual form.
    #[must_use]
    pub fn canonical_text(&self) -> String {
        self.canonical_name.to_string()
    }
}
impl fmt::Display for ResolvedDataSpaceV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.canonical_name.fmt(f)
    }
}
/// Canonical fully qualified domain paired with its expected numeric dataspace ID.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct ResolvedDomainV1 {
    /// Canonical `domain.dataspace` text.
    pub canonical_name: DomainId,
    /// Numeric parent dataspace ID that the text must resolve to at execution.
    pub dataspace_id: DataSpaceId,
}
impl ResolvedDomainV1 {
    /// Construct a resolved domain pair.
    #[must_use]
    pub const fn new(canonical_name: DomainId, dataspace_id: DataSpaceId) -> Self {
        Self {
            canonical_name,
            dataspace_id,
        }
    }
    /// Resolve a fully qualified domain using a static catalog.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if the domain is malformed or its dataspace is unknown.
    pub fn resolve_catalog(input: &str, catalog: &DataSpaceCatalog) -> Result<Self, ParseError> {
        let canonical_name = DomainId::parse_fully_qualified(input)?;
        let dataspace_id = catalog
            .by_alias(canonical_name.dataspace().as_ref())
            .map(|entry| entry.id)
            .ok_or_else(|| ParseError::new("unknown dataspace alias in domain"))?;
        Ok(Self::new(canonical_name, dataspace_id))
    }
    /// Return whether the textual parent still maps to the pinned numeric ID.
    #[must_use]
    pub fn matches_catalog(&self, catalog: &DataSpaceCatalog) -> bool {
        catalog
            .by_alias(self.canonical_name.dataspace().as_ref())
            .is_some_and(|entry| entry.id == self.dataspace_id)
    }
    /// Return the canonical textual form.
    #[must_use]
    pub fn canonical_text(&self) -> String {
        self.canonical_name.to_string()
    }
    /// Return the resolved parent dataspace.
    #[must_use]
    pub fn parent_dataspace(&self) -> ResolvedDataSpaceV1 {
        ResolvedDataSpaceV1::new(self.canonical_name.dataspace().clone(), self.dataspace_id)
    }
}
impl fmt::Display for ResolvedDomainV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.canonical_name.fmt(f)
    }
}
/// Canonical account-alias text paired with its expected numeric dataspace ID.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct ResolvedAccountAliasV1 {
    /// Canonical account alias text.
    pub canonical_name: AccountAliasName,
    /// Numeric parent dataspace ID that the text must resolve to at execution.
    pub dataspace_id: DataSpaceId,
}
impl ResolvedAccountAliasV1 {
    /// Construct a resolved account alias pair.
    #[must_use]
    pub const fn new(canonical_name: AccountAliasName, dataspace_id: DataSpaceId) -> Self {
        Self {
            canonical_name,
            dataspace_id,
        }
    }
    /// Resolve an account alias using a static catalog.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if the alias is malformed or its dataspace is unknown.
    pub fn resolve_catalog(input: &str, catalog: &DataSpaceCatalog) -> Result<Self, ParseError> {
        let canonical_name = input.parse::<AccountAliasName>()?;
        let dataspace_id = catalog
            .by_alias(canonical_name.dataspace.as_ref())
            .map(|entry| entry.id)
            .ok_or_else(|| ParseError::new("unknown dataspace alias in account alias"))?;
        Ok(Self::new(canonical_name, dataspace_id))
    }
    /// Return whether the textual parent still maps to the pinned numeric ID.
    #[must_use]
    pub fn matches_catalog(&self, catalog: &DataSpaceCatalog) -> bool {
        catalog
            .by_alias(self.canonical_name.dataspace.as_ref())
            .is_some_and(|entry| entry.id == self.dataspace_id)
    }
    /// Return the canonical textual form.
    #[must_use]
    pub fn canonical_text(&self) -> String {
        self.canonical_name.to_string()
    }
    /// Convert to the numeric on-chain account-alias key.
    #[must_use]
    pub fn account_alias(&self) -> AccountAlias {
        AccountAlias::new(
            self.canonical_name.label.clone(),
            self.canonical_name
                .domain
                .clone()
                .map(AccountAliasDomain::new),
            self.dataspace_id,
        )
    }
    /// Return the optional resolved domain parent.
    #[must_use]
    pub fn parent_domain(&self) -> Option<ResolvedDomainV1> {
        self.canonical_name
            .domain_id()
            .map(|canonical_name| ResolvedDomainV1::new(canonical_name, self.dataspace_id))
    }
}
impl fmt::Display for ResolvedAccountAliasV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.canonical_name.fmt(f)
    }
}
impl From<&ResolvedAccountAliasV1> for AccountAlias {
    fn from(value: &ResolvedAccountAliasV1) -> Self {
        value.account_alias()
    }
}
impl From<ResolvedAccountAliasV1> for AccountAlias {
    fn from(value: ResolvedAccountAliasV1) -> Self {
        value.account_alias()
    }
}
/// Account provisioning behavior requested by an account-alias intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(
        tag = "kind",
        content = "value",
        rename_all = "snake_case",
        deny_unknown_fields
    )
)]
#[repr(u8)]
pub enum AccountProvisionV1 {
    /// The target account must already exist.
    #[codec(index = 0)]
    Existing,
    /// Create the target account if it is absent.
    #[codec(index = 1)]
    Create,
}
/// Whether an account alias should be primary or additional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(
        tag = "kind",
        content = "value",
        rename_all = "snake_case",
        deny_unknown_fields
    )
)]
#[repr(u8)]
pub enum AccountAliasRoleV1 {
    /// Make the exact alias the account's primary alias.
    #[codec(index = 0)]
    Primary,
    /// Bind the exact alias without changing the primary alias.
    #[codec(index = 1)]
    Additional,
}
/// Desired state for one dataspace alias.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct AliasDataSpaceIntentV1 {
    /// Resolved dataspace name.
    pub dataspace: ResolvedDataSpaceV1,
    /// Exact desired owner.
    pub owner: AccountId,
}
/// Desired state for one domain.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct AliasDomainIntentV1 {
    /// Resolved domain name, including its parent dataspace.
    pub domain: ResolvedDomainV1,
    /// Exact desired owner.
    pub owner: AccountId,
}
/// Desired state for one account alias.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct AliasAccountIntentV1 {
    /// Resolved account alias.
    pub alias: ResolvedAccountAliasV1,
    /// Exact canonical target account.
    pub target_account: AccountId,
    /// Whether the target must exist or may be created.
    pub provision: AccountProvisionV1,
    /// Whether this is the primary or an additional alias.
    pub role: AccountAliasRoleV1,
}
/// Declarative desired state for one alias/SNS resource.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(
        tag = "kind",
        content = "intent",
        rename_all = "snake_case",
        deny_unknown_fields
    )
)]
pub enum AliasIntentV1 {
    /// Ensure a dataspace alias and its exact owner.
    #[codec(index = 0)]
    Dataspace(AliasDataSpaceIntentV1),
    /// Ensure a domain and its exact owner.
    #[codec(index = 1)]
    Domain(AliasDomainIntentV1),
    /// Ensure an account alias, target account, and primary role.
    #[codec(index = 2)]
    AccountAlias(AliasAccountIntentV1),
}
impl AliasIntentV1 {
    /// Return the resource targeted by the intent.
    #[must_use]
    pub fn target(&self) -> AliasTargetV1 {
        match self {
            Self::Dataspace(intent) => AliasTargetV1::Dataspace(intent.dataspace.clone()),
            Self::Domain(intent) => AliasTargetV1::Domain(intent.domain.clone()),
            Self::AccountAlias(intent) => AliasTargetV1::AccountAlias(intent.alias.clone()),
        }
    }
}
/// Lease terms used only when setup classifies a resource as absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct AliasLeaseAcquisitionV1 {
    /// Requested lease term in whole years.
    pub term_years: u8,
    /// Optional pricing class expected by the caller.
    ///
    /// The slot is always encoded in V1 JSON; `None` means that the active
    /// policy may select the pricing class.
    #[norito(required)]
    pub pricing_class_hint: Option<u8>,
}
impl AliasLeaseAcquisitionV1 {
    /// Construct acquisition terms.
    #[must_use]
    pub const fn new(term_years: u8, pricing_class_hint: Option<u8>) -> Self {
        Self {
            term_years,
            pricing_class_hint,
        }
    }
}
/// Guard binding a lease operation to an exact policy and bounded quote.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct AliasQuoteGuardV1 {
    /// Policy version that consensus must observe.
    pub expected_policy_version: u16,
    /// Payment asset that consensus must observe.
    pub expected_payment_asset: AssetDefinitionId,
    /// Maximum amount authorized by the payer.
    pub max_amount: Quantity,
    /// Last block timestamp at which the quote may be used.
    pub valid_until_ms: u64,
}
/// Canonical signed request body for planning one atomic alias setup transaction.
///
/// Each entry is the exact [`EnsureAlias`](crate::isi::alias_setup::EnsureAlias)
/// instruction the client expects to submit. The planner reorders entries into dependency order,
/// reclassifies them against live state, and returns the canonical framed instruction vector.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct AliasSetupPlanRequestV1 {
    /// Request layout version. The only supported value is [`Self::VERSION`].
    pub schema_version: u8,
    /// Per-resource ensure instructions forming one indivisible setup intent.
    pub intents: Vec<crate::isi::alias_setup::EnsureAlias>,
}
impl AliasSetupPlanRequestV1 {
    /// Current signed planner request layout version.
    pub const VERSION: u8 = 1;
    /// Construct a versioned planner request.
    #[must_use]
    pub const fn new(intents: Vec<crate::isi::alias_setup::EnsureAlias>) -> Self {
        Self {
            schema_version: Self::VERSION,
            intents,
        }
    }
}
/// Resolved resource supported by setup, renewal, and auto-renew operations.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(
        tag = "kind",
        content = "resource",
        rename_all = "snake_case",
        deny_unknown_fields
    )
)]
pub enum AliasTargetV1 {
    /// Dataspace alias resource.
    #[codec(index = 0)]
    Dataspace(ResolvedDataSpaceV1),
    /// Domain resource.
    #[codec(index = 1)]
    Domain(ResolvedDomainV1),
    /// Account-alias resource.
    #[codec(index = 2)]
    AccountAlias(ResolvedAccountAliasV1),
}
impl AliasTargetV1 {
    /// Return the pinned numeric dataspace for deterministic routing and revalidation.
    #[must_use]
    pub const fn dataspace_id(&self) -> DataSpaceId {
        match self {
            Self::Dataspace(value) => value.dataspace_id,
            Self::Domain(value) => value.dataspace_id,
            Self::AccountAlias(value) => value.dataspace_id,
        }
    }
}
impl fmt::Display for AliasTargetV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dataspace(value) => write!(f, "dataspace:{value}"),
            Self::Domain(value) => write!(f, "domain:{value}"),
            Self::AccountAlias(value) => write!(f, "account_alias:{value}"),
        }
    }
}
/// Owner-configured deterministic auto-renew policy.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct AliasAutoRenewConfigV1 {
    /// Renewal term in whole years.
    pub term_years: u8,
    /// SNS policy version accepted by the owner.
    pub policy_version: u16,
    /// Payment asset accepted by the owner.
    pub payment_asset: AssetDefinitionId,
    /// Maximum exact renewal charge authorized by the owner.
    pub max_amount: Quantity,
    /// Time before expiry at which renewal attempts begin.
    pub renew_before_expiry_ms: u64,
    /// Deterministic delay between failed attempts.
    pub retry_backoff_ms: u64,
    /// Number of failures after which auto-renew is suspended.
    pub max_failures: u32,
}
/// Persisted compare-and-set state for native deterministic alias auto-renew.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct AliasAutoRenewStateV1 {
    /// State layout version, currently `1`.
    pub version: u8,
    /// Exact lease target governed by this record.
    pub target: AliasTargetV1,
    /// Resource owner whose balance native renewal may debit.
    pub owner: AccountId,
    /// Monotonic configuration/runtime revision used for compare-and-set.
    pub revision: u64,
    /// Enabled configuration, or `None` when explicitly disabled.
    #[norito(default)]
    pub config: Option<AliasAutoRenewConfigV1>,
    /// Consecutive deterministic renewal failures.
    pub failure_count: u32,
    /// Earliest block timestamp for the next retry, when scheduled.
    #[norito(default)]
    pub next_retry_at_ms: Option<u64>,
    /// Stable suspension code for policy/asset drift or repeated failures.
    #[norito(default)]
    pub suspended_reason: Option<String>,
}
impl AliasAutoRenewStateV1 {
    /// Current persisted layout version.
    pub const VERSION: u8 = 1;
    /// Construct a freshly configured or disabled record.
    #[must_use]
    pub const fn new(
        target: AliasTargetV1,
        owner: AccountId,
        revision: u64,
        config: Option<AliasAutoRenewConfigV1>,
    ) -> Self {
        Self {
            version: Self::VERSION,
            target,
            owner,
            revision,
            config,
            failure_count: 0,
            next_retry_at_ms: None,
            suspended_reason: None,
        }
    }
}
/// Canonical signed request body for planning one lease renewal.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct AliasLeaseRenewPlanRequestV1 {
    /// Request layout version. The only supported value is [`Self::VERSION`].
    pub schema_version: u8,
    /// Exact absolute-expiry CAS renewal the client intends to submit.
    pub renewal: crate::isi::alias_setup::RenewAliasLease,
}
impl AliasLeaseRenewPlanRequestV1 {
    /// Current signed renewal planner request layout version.
    pub const VERSION: u8 = 1;
    /// Construct a versioned renewal planner request.
    #[must_use]
    pub const fn new(renewal: crate::isi::alias_setup::RenewAliasLease) -> Self {
        Self {
            schema_version: Self::VERSION,
            renewal,
        }
    }
}
/// Canonical signed request body for planning one auto-renew configuration CAS.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct AliasAutoRenewPlanRequestV1 {
    /// Request layout version. The only supported value is [`Self::VERSION`].
    pub schema_version: u8,
    /// Exact owner-only configuration CAS the client intends to submit.
    pub configuration: crate::isi::alias_setup::ConfigureAliasAutoRenew,
}
impl AliasAutoRenewPlanRequestV1 {
    /// Current signed auto-renew planner request layout version.
    pub const VERSION: u8 = 1;
    /// Construct a versioned auto-renew planner request.
    #[must_use]
    pub const fn new(configuration: crate::isi::alias_setup::ConfigureAliasAutoRenew) -> Self {
        Self {
            schema_version: Self::VERSION,
            configuration,
        }
    }
}
/// Exact lifecycle operation committed by a lifecycle transaction plan.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "operation", rename_all = "snake_case")
)]
pub enum AliasLifecycleOperationV1 {
    /// Absolute-expiry lease renewal with an expected-current-expiry CAS.
    #[codec(index = 0)]
    RenewLease(crate::isi::alias_setup::RenewAliasLease),
    /// Enable, replace, or disable deterministic native auto-renew.
    #[codec(index = 1)]
    ConfigureAutoRenew(crate::isi::alias_setup::ConfigureAliasAutoRenew),
}
impl AliasLifecycleOperationV1 {
    /// Return the resolved resource targeted by this lifecycle operation.
    #[must_use]
    pub fn target(&self) -> &AliasTargetV1 {
        match self {
            Self::RenewLease(operation) => &operation.target,
            Self::ConfigureAutoRenew(operation) => &operation.target,
        }
    }
}
/// Planner classification for one lifecycle operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", rename_all = "snake_case")
)]
#[repr(u8)]
pub enum AliasLifecyclePlanDispositionV1 {
    /// Exact desired configuration already exists; no instruction or charge is required.
    #[codec(index = 0)]
    NoOp,
    /// The exact framed lifecycle instruction may be submitted as one normal transaction.
    #[codec(index = 1)]
    Apply,
}
/// Planner classification for one resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(
        tag = "kind",
        content = "value",
        rename_all = "snake_case",
        deny_unknown_fields
    )
)]
#[repr(u8)]
pub enum AliasPlanDispositionV1 {
    /// Exact desired state already exists and incurs no charge.
    #[codec(index = 0)]
    NoOp,
    /// Only derived state is missing and can be restored without a lease charge.
    #[codec(index = 1)]
    Repair,
    /// The resource is absent and requires one lease acquisition charge.
    #[codec(index = 2)]
    Create,
    /// Existing authoritative state differs and must not be overwritten.
    #[codec(index = 3)]
    Conflict,
}
/// Exact lease quote attached to a create or renewal plan resource.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct AliasLeaseQuoteV1 {
    /// Resource to which the quote applies.
    pub target: AliasTargetV1,
    /// Pricing class selected by the active policy.
    pub pricing_class: u8,
    /// Exact amount consensus will charge.
    pub exact_amount: Quantity,
    /// Client guard, including policy, asset, cap, and deadline.
    pub guard: AliasQuoteGuardV1,
    /// Resulting paid-term expiry.
    pub expires_at_ms: u64,
    /// Resulting grace-period expiry.
    pub grace_expires_at_ms: u64,
    /// Resulting redemption-period expiry.
    pub redemption_expires_at_ms: u64,
}
/// Planner result for one ordered resource intent.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct AliasPlanResourceV1 {
    /// Canonically resolved desired state.
    pub intent: AliasIntentV1,
    /// Planner classification.
    pub disposition: AliasPlanDispositionV1,
    /// Exact quote when this resource requires acquisition or renewal.
    ///
    /// The slot is always encoded in V1 JSON; `None` is explicit for a
    /// resource that does not require a quote.
    #[norito(required)]
    pub quote: Option<AliasLeaseQuoteV1>,
    /// Index of the matching executable instruction, if any.
    ///
    /// The slot is always encoded in V1 JSON; `None` is explicit when no
    /// instruction is emitted.
    #[norito(required)]
    pub instruction_index: Option<u32>,
}
/// Exact framed Norito instruction returned by the planner.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct AliasFramedInstructionV1 {
    /// Stable instruction wire identifier.
    pub wire_id: String,
    /// Exact Norito frame that clients must decode and re-encode.
    pub framed_payload: Vec<u8>,
}
/// Exact total charge for one payment asset.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct AliasAssetTotalV1 {
    /// Payment asset.
    pub payment_asset: AssetDefinitionId,
    /// Exact aggregate amount.
    pub amount: Quantity,
}
/// Overall setup/readiness state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "status", content = "value", rename_all = "snake_case")
)]
#[repr(u8)]
pub enum AliasSetupStatusV1 {
    /// Validation succeeded and the operation is ready.
    #[codec(index = 0)]
    Ready,
    /// Required live state is not yet available.
    #[codec(index = 1)]
    Pending,
    /// A deterministic validation or drift error blocks the operation.
    #[codec(index = 2)]
    Blocked,
}
/// Phase that produced a setup diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "phase", content = "value", rename_all = "snake_case")
)]
#[repr(u8)]
pub enum AliasSetupValidationPhaseV1 {
    /// Static configuration validation.
    #[codec(index = 0)]
    Config,
    /// Text-to-dataspace catalog validation.
    #[codec(index = 1)]
    Catalog,
    /// Genesis/bootstrap validation.
    #[codec(index = 2)]
    Bootstrap,
    /// Live world-state validation.
    #[codec(index = 3)]
    WorldState,
    /// Transaction planning validation.
    #[codec(index = 4)]
    Planning,
}
/// Severity of a setup diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "severity", content = "value", rename_all = "snake_case")
)]
#[repr(u8)]
pub enum AliasSetupSeverityV1 {
    /// Informational observation.
    #[codec(index = 0)]
    Info,
    /// Non-blocking warning.
    #[codec(index = 1)]
    Warning,
    /// Blocking error.
    #[codec(index = 2)]
    Error,
}
/// One stable, secret-free setup/readiness diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct AliasSetupDiagnosticV1 {
    /// Validation phase.
    pub phase: AliasSetupValidationPhaseV1,
    /// Stable machine-readable diagnostic code.
    pub code: String,
    /// Diagnostic severity.
    pub severity: AliasSetupSeverityV1,
    /// Canonical resource text, if one is available.
    #[norito(default)]
    pub resource: Option<String>,
    /// Configuration path associated with the diagnostic.
    #[norito(default)]
    pub config_path: Option<String>,
    /// Redacted expected value.
    #[norito(default)]
    pub expected: Option<String>,
    /// Redacted actual value.
    #[norito(default)]
    pub actual: Option<String>,
    /// Human-readable corrective action.
    pub remediation: String,
}
/// Deterministically ordered setup/readiness diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct AliasSetupReportV1 {
    /// Report layout version. The only supported value is [`Self::VERSION`].
    pub version: u8,
    /// Overall readiness state.
    pub status: AliasSetupStatusV1,
    /// Stable diagnostics sorted by their canonical field order.
    pub diagnostics: Vec<AliasSetupDiagnosticV1>,
}
impl AliasSetupReportV1 {
    /// Current report layout version.
    pub const VERSION: u8 = 1;
    /// Construct a report and sort diagnostics deterministically.
    #[must_use]
    pub fn new(status: AliasSetupStatusV1, mut diagnostics: Vec<AliasSetupDiagnosticV1>) -> Self {
        diagnostics.sort();
        Self {
            version: Self::VERSION,
            status,
            diagnostics,
        }
    }
}
/// World-state anchor used to classify an alias plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct AliasPlanAnchorV1 {
    /// Height of the anchored block.
    pub block_height: u64,
    /// Hash of the anchored block.
    pub block_hash: Hash,
}
/// Canonical body committed by an alias transaction plan hash.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct AliasTransactionPlanBodyV1 {
    /// Layout version. The only supported value is [`Self::VERSION`].
    pub version: u8,
    /// Transaction authority and lease payer.
    pub authority: AccountId,
    /// Exact genesis-derived network on which the plan was produced.
    pub network_id: NetworkId,
    /// World-state anchor used for classification.
    pub anchor: AliasPlanAnchorV1,
    /// Ordered resources in dependency order.
    pub resources: Vec<AliasPlanResourceV1>,
    /// Ordered exact framed instructions for one ordinary transaction.
    pub instructions: Vec<AliasFramedInstructionV1>,
    /// Exact totals sorted by canonical payment asset ID.
    pub totals_by_asset: Vec<AliasAssetTotalV1>,
    /// Non-blocking planner diagnostics.
    pub warnings: Vec<AliasSetupDiagnosticV1>,
    /// Blocking planner diagnostics. Executable plans must keep this empty.
    pub blockers: Vec<AliasSetupDiagnosticV1>,
    /// Last block timestamp at which the plan may be submitted.
    pub valid_until_ms: u64,
}
impl AliasTransactionPlanBodyV1 {
    /// Current canonical body layout version.
    pub const VERSION: u8 = 1;
    /// Compute the domain-separated canonical plan body hash.
    #[must_use]
    pub fn canonical_hash(&self) -> HashOf<Self> {
        let encoded = self.encode();
        HashOf::from_untyped_unchecked(Hash::new_from_chunks(&[
            ALIAS_TRANSACTION_PLAN_HASH_DOMAIN_V1,
            encoded.as_slice(),
        ]))
    }
    /// Sort fields whose wire semantics are sets while preserving resource and instruction order.
    pub fn canonicalize_unordered_fields(&mut self) {
        self.totals_by_asset.sort();
        self.warnings.sort();
        self.blockers.sort();
    }
}
/// Alias transaction plan and its canonical body commitment.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct AliasTransactionPlanV1 {
    /// Canonical plan body.
    pub body: AliasTransactionPlanBodyV1,
    /// Domain-separated hash of `body`.
    pub plan_hash: HashOf<AliasTransactionPlanBodyV1>,
}
impl AliasTransactionPlanV1 {
    /// Construct a plan after canonicalizing unordered body fields.
    #[must_use]
    pub fn new(mut body: AliasTransactionPlanBodyV1) -> Self {
        body.canonicalize_unordered_fields();
        let plan_hash = body.canonical_hash();
        Self { body, plan_hash }
    }
    /// Verify that the carried hash matches the canonical body.
    #[must_use]
    pub fn verify_hash(&self) -> bool {
        self.plan_hash == self.body.canonical_hash()
    }
}
/// Canonical body committed by an alias lifecycle transaction plan hash.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct AliasLifecycleTransactionPlanBodyV1 {
    /// Layout version. The only supported value is [`Self::VERSION`].
    pub version: u8,
    /// Transaction authority and renewal payer.
    pub authority: AccountId,
    /// Exact genesis-derived network on which the plan was produced.
    pub network_id: NetworkId,
    /// World-state anchor used for classification.
    pub anchor: AliasPlanAnchorV1,
    /// Exact operation supplied by the signed planning request.
    pub operation: AliasLifecycleOperationV1,
    /// Whether apply is required or the exact desired configuration is already present.
    pub disposition: AliasLifecyclePlanDispositionV1,
    /// Exact framed instruction for `Apply`; absent only for `NoOp`.
    #[norito(default)]
    pub instruction: Option<AliasFramedInstructionV1>,
    /// Exact renewal quote; present only for lease renewal.
    #[norito(default)]
    pub quote: Option<AliasLeaseQuoteV1>,
    /// Exact totals sorted by canonical payment asset ID.
    pub totals_by_asset: Vec<AliasAssetTotalV1>,
    /// Non-blocking planner diagnostics.
    pub warnings: Vec<AliasSetupDiagnosticV1>,
    /// Blocking planner diagnostics. Executable plans must keep this empty.
    pub blockers: Vec<AliasSetupDiagnosticV1>,
    /// Last wall-clock millisecond at which a client may submit this plan.
    pub valid_until_ms: u64,
}
impl AliasLifecycleTransactionPlanBodyV1 {
    /// Current canonical lifecycle plan body layout version.
    pub const VERSION: u8 = 1;
    /// Compute the domain-separated canonical lifecycle plan body hash.
    #[must_use]
    pub fn canonical_hash(&self) -> HashOf<Self> {
        let encoded = self.encode();
        HashOf::from_untyped_unchecked(Hash::new_from_chunks(&[
            ALIAS_LIFECYCLE_TRANSACTION_PLAN_HASH_DOMAIN_V1,
            encoded.as_slice(),
        ]))
    }
    /// Sort fields whose wire semantics are sets.
    pub fn canonicalize_unordered_fields(&mut self) {
        self.totals_by_asset.sort();
        self.warnings.sort();
        self.blockers.sort();
    }
}
/// Alias lifecycle transaction plan and its canonical body commitment.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct AliasLifecycleTransactionPlanV1 {
    /// Canonical lifecycle plan body.
    pub body: AliasLifecycleTransactionPlanBodyV1,
    /// Domain-separated hash of `body`.
    pub plan_hash: HashOf<AliasLifecycleTransactionPlanBodyV1>,
}
impl AliasLifecycleTransactionPlanV1 {
    /// Construct a plan after canonicalizing unordered body fields.
    #[must_use]
    pub fn new(mut body: AliasLifecycleTransactionPlanBodyV1) -> Self {
        body.canonicalize_unordered_fields();
        let plan_hash = body.canonical_hash();
        Self { body, plan_hash }
    }
    /// Verify that the carried hash matches the canonical body.
    #[must_use]
    pub fn verify_hash(&self) -> bool {
        self.plan_hash == self.body.canonical_hash()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::nexus::DataSpaceMetadata;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
    use iroha_primitives::numeric::Numeric;
    fn plan_network_id(seed: u8) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<crate::block::BlockHeader>::from_untyped_unchecked(
            Hash::new([seed]),
        ))
    }
    fn shared_plan_network_id() -> NetworkId {
        let mut bytes = [0_u8; Hash::LENGTH];
        hex::decode_to_slice(
            "32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149",
            &mut bytes,
        )
        .expect("shared plan NetworkId hex");
        NetworkId::from_genesis_hash(HashOf::<crate::block::BlockHeader>::from_untyped_unchecked(
            Hash::prehashed(bytes),
        ))
    }
    #[derive(crate::DeriveJsonDeserialize)]
    struct SharedAliasSetupFixture {
        schema_version: u8,
        account_alias_cases: Vec<SharedAliasNameCase>,
        resolved_name_json_vectors: SharedResolvedNameVectors,
        quote_guard_json_vector: AliasQuoteGuardV1,
        permission_scope_json_vector: norito::json::Value,
        account_onboarding_receipt_vector: SharedAccountOnboardingReceiptVector,
        plan_hash_vectors: Vec<SharedPlanHashVector>,
        instruction_frame_vectors: Vec<SharedInstructionFrameVector>,
        report_json_vector: AliasSetupReportV1,
    }
    #[derive(crate::DeriveJsonDeserialize)]
    struct SharedResolvedNameVectors {
        dataspace: ResolvedDataSpaceV1,
        domain: ResolvedDomainV1,
        account_alias: ResolvedAccountAliasV1,
    }
    #[derive(crate::DeriveJsonDeserialize)]
    struct SharedAliasNameCase {
        input: String,
        canonical: String,
        label: String,
        domain: Option<String>,
        dataspace: String,
    }
    #[derive(crate::DeriveJsonDeserialize)]
    struct SharedPlanHashVector {
        name: String,
        domain: String,
        canonical_body_norito_hex: String,
        canonical_plan_hash_hex: String,
    }
    #[derive(crate::DeriveJsonDeserialize)]
    struct SharedInstructionFrameVector {
        name: String,
        wire_id: String,
        framed_payload_hex: String,
    }
    const ACCOUNT_ONBOARDING_RECEIPT_HASH_DOMAIN_V1: &[u8] =
        b"iroha:account-onboarding-plan-receipt:v1\0";
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        Encode,
        Decode,
        crate::DeriveJsonSerialize,
        crate::DeriveJsonDeserialize,
    )]
    #[norito(deny_unknown_fields)]
    struct FixtureAccountOnboardingPlanRequestV1 {
        version: u8,
        alias: String,
        account_id: String,
        permissions: Vec<String>,
    }
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        Encode,
        Decode,
        crate::DeriveJsonSerialize,
        crate::DeriveJsonDeserialize,
    )]
    #[norito(deny_unknown_fields)]
    struct FixtureAccountOnboardingPlanBodyV1 {
        version: u8,
        request: FixtureAccountOnboardingPlanRequestV1,
        authority: AccountId,
        network_id: NetworkId,
        anchor: AliasPlanAnchorV1,
        resource: AliasPlanResourceV1,
        acquisition: AliasLeaseAcquisitionV1,
        quote_guard: AliasQuoteGuardV1,
        instructions: Vec<AliasFramedInstructionV1>,
        #[norito(required)]
        owner_auto_renew_instruction: Option<AliasFramedInstructionV1>,
        valid_until_ms: u64,
    }
    impl FixtureAccountOnboardingPlanBodyV1 {
        fn canonical_hash(&self) -> Hash {
            let encoded = self.encode();
            Hash::new_from_chunks(&[
                ACCOUNT_ONBOARDING_RECEIPT_HASH_DOMAIN_V1,
                encoded.as_slice(),
            ])
        }
    }
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        Encode,
        Decode,
        crate::DeriveJsonSerialize,
        crate::DeriveJsonDeserialize,
    )]
    #[norito(deny_unknown_fields)]
    struct FixtureAccountOnboardingPlanReceiptV1 {
        body: FixtureAccountOnboardingPlanBodyV1,
        plan_hash: Hash,
        signature: Signature,
    }
    impl FixtureAccountOnboardingPlanReceiptV1 {
        fn verify(&self) -> bool {
            self.plan_hash == self.body.canonical_hash()
                && self
                    .body
                    .authority
                    .try_signatory()
                    .is_some_and(|signatory| {
                        self.signature
                            .verify(signatory, self.plan_hash.as_ref())
                            .is_ok()
                    })
        }
    }
    #[derive(
        Debug, Clone, PartialEq, Eq, crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize,
    )]
    #[norito(deny_unknown_fields)]
    struct SharedAccountOnboardingReceiptVector {
        name: String,
        domain: String,
        canonical_body_norito_hex: String,
        canonical_plan_hash_hex: String,
        authority: String,
        signature_hex: String,
        receipt_json: FixtureAccountOnboardingPlanReceiptV1,
    }
    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive checked alias setup fixture keypair");
        AccountId::new(key_pair.public_key().clone())
    }
    fn catalog() -> DataSpaceCatalog {
        DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: DataSpaceId::new(7),
                alias: "paynet".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("dataspace catalog")
    }
    fn resolved_alias() -> ResolvedAccountAliasV1 {
        ResolvedAccountAliasV1::resolve_catalog("merchant@banka.paynet", &catalog())
            .expect("resolved alias")
    }
    fn payment_asset() -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("assets", "paynet").expect("asset domain"),
            "xor".parse().expect("asset name"),
        )
    }
    fn amount(value: u32) -> Quantity {
        Quantity::from_canonical_numeric(Numeric::new(value, 0)).expect("quantity")
    }
    macro_rules! assert_json_unknown_field_rejected {
        ($value:expr, $ty:ty, $label:literal) => {{
            let mut value =
                norito::json::to_value(&$value).expect(concat!("serialize ", $label));
            norito::json::from_value::<$ty>(value.clone())
                .expect(concat!("decode canonical ", $label));
            value
                .as_object_mut()
                .expect(concat!($label, " JSON object"))
                .insert(
                    "retired_v0".to_owned(),
                    norito::json::Value::Bool(true),
                );
            let error = norito::json::from_value::<$ty>(value)
                .expect_err(concat!($label, " must reject unknown fields"));
            assert!(
                matches!(
                    error,
                    norito::json::Error::UnknownField { ref field } if field == "retired_v0"
                ),
                "{} reported the wrong error: {error:?}",
                $label
            );
        }};
    }
    fn deterministic_account_onboarding_receipt_vector() -> SharedAccountOnboardingReceiptVector {
        let target_signer = KeyPair::try_from_seed(vec![0x22; 32], Algorithm::Ed25519)
            .expect("derive deterministic onboarding target");
        let target_account = AccountId::new(target_signer.public_key().clone());
        let onboarding_signer = KeyPair::try_from_seed(vec![0x51; 32], Algorithm::Ed25519)
            .expect("derive deterministic onboarding authority");
        let authority = AccountId::new(onboarding_signer.public_key().clone());
        let alias = resolved_alias();
        let intent = AliasIntentV1::AccountAlias(AliasAccountIntentV1 {
            alias,
            target_account: target_account.clone(),
            provision: AccountProvisionV1::Create,
            role: AccountAliasRoleV1::Primary,
        });
        let guard = AliasQuoteGuardV1 {
            expected_policy_version: 2,
            expected_payment_asset: "4rPeAP6jAjiLVZThZYwwPRBuQagt"
                .parse()
                .expect("deterministic onboarding payment asset"),
            max_amount: amount(3),
            valid_until_ms: 50_000,
        };
        let acquisition = AliasLeaseAcquisitionV1::new(1, None);
        let ensure: crate::prelude::InstructionBox =
            crate::isi::alias_setup::EnsureAlias::new(intent.clone(), acquisition, guard.clone())
                .into();
        let (wire_id, framed_payload) = crate::isi::framed_instruction_payload(&ensure)
            .expect("registered deterministic onboarding instruction");
        let body = FixtureAccountOnboardingPlanBodyV1 {
            version: 1,
            request: FixtureAccountOnboardingPlanRequestV1 {
                version: 1,
                alias: "merchant@banka.paynet".to_owned(),
                account_id: target_account.to_string(),
                permissions: Vec::new(),
            },
            authority: authority.clone(),
            network_id: plan_network_id(0xA1),
            anchor: AliasPlanAnchorV1 {
                block_height: 11,
                block_hash: Hash::new(b"account-onboarding-receipt-fixture-anchor"),
            },
            resource: AliasPlanResourceV1 {
                intent: intent.clone(),
                disposition: AliasPlanDispositionV1::Create,
                quote: Some(AliasLeaseQuoteV1 {
                    target: intent.target(),
                    pricing_class: 1,
                    exact_amount: amount(3),
                    guard: guard.clone(),
                    expires_at_ms: 1_000,
                    grace_expires_at_ms: 2_000,
                    redemption_expires_at_ms: 3_000,
                }),
                instruction_index: Some(0),
            },
            acquisition,
            quote_guard: guard,
            instructions: vec![AliasFramedInstructionV1 {
                wire_id: wire_id.to_owned(),
                framed_payload,
            }],
            owner_auto_renew_instruction: None,
            valid_until_ms: 50_000,
        };
        let body_bytes = body.encode();
        let plan_hash = body.canonical_hash();
        let signature = Signature::try_new(onboarding_signer.private_key(), plan_hash.as_ref())
            .expect("sign deterministic onboarding receipt");
        let receipt = FixtureAccountOnboardingPlanReceiptV1 {
            body,
            plan_hash,
            signature,
        };
        SharedAccountOnboardingReceiptVector {
            name: "sponsored_account_alias_create".to_owned(),
            domain: String::from_utf8(ACCOUNT_ONBOARDING_RECEIPT_HASH_DOMAIN_V1.to_vec())
                .expect("onboarding receipt hash domain is UTF-8"),
            canonical_body_norito_hex: hex::encode(body_bytes),
            canonical_plan_hash_hex: hex::encode(receipt.plan_hash.as_ref()),
            authority: authority.to_string(),
            signature_hex: hex::encode_upper(receipt.signature.payload()),
            receipt_json: receipt,
        }
    }
    #[test]
    fn account_alias_name_parses_both_supported_forms() {
        let domainful: AccountAliasName = "Merchant@Banka.Paynet".parse().expect("domain alias");
        assert_eq!(domainful.to_string(), "merchant@banka.paynet");
        assert_eq!(domainful.domain.as_ref().map(Name::as_ref), Some("banka"));
        let root: AccountAliasName = "Merchant@Paynet".parse().expect("root alias");
        assert_eq!(root.to_string(), "merchant@paynet");
        assert!(root.domain.is_none());
    }
    #[test]
    fn account_alias_name_json_requires_explicit_domain_slot() {
        let alias: AccountAliasName = "merchant@paynet".parse().expect("root alias");
        let value = norito::json::to_value(&alias).expect("serialize current account alias name");
        assert_eq!(
            value.as_object().and_then(|object| object.get("domain")),
            Some(&norito::json::Value::Null)
        );
        let mut missing = value;
        missing
            .as_object_mut()
            .expect("account alias name object")
            .remove("domain");
        assert!(
            norito::json::from_value::<AccountAliasName>(missing).is_err(),
            "current account alias name must reject an omitted domain slot"
        );
    }
    #[test]
    fn onboarding_nested_v1_json_requires_explicit_optional_slots() {
        let fixture = deterministic_account_onboarding_receipt_vector();
        let acquisition = fixture.receipt_json.body.acquisition;
        let acquisition_json = norito::json::to_value(&acquisition)
            .expect("encode onboarding acquisition as exact V1 JSON");
        assert_eq!(
            norito::json::from_value::<AliasLeaseAcquisitionV1>(acquisition_json.clone())
                .expect("round-trip exact onboarding acquisition"),
            acquisition
        );
        let mut missing_pricing_class = acquisition_json;
        assert!(
            missing_pricing_class
                .as_object_mut()
                .expect("acquisition object")
                .remove("pricing_class_hint")
                .is_some()
        );
        assert!(
            norito::json::from_value::<AliasLeaseAcquisitionV1>(missing_pricing_class).is_err(),
            "V1 acquisition must not default an omitted pricing_class_hint to None"
        );

        let resource = fixture.receipt_json.body.resource;
        let resource_json =
            norito::json::to_value(&resource).expect("encode onboarding resource as exact V1 JSON");
        assert_eq!(
            norito::json::from_value::<AliasPlanResourceV1>(resource_json.clone())
                .expect("round-trip exact onboarding resource"),
            resource
        );
        for field in ["quote", "instruction_index"] {
            let mut missing = resource_json.clone();
            assert!(
                missing
                    .as_object_mut()
                    .expect("resource object")
                    .remove(field)
                    .is_some()
            );
            assert!(
                norito::json::from_value::<AliasPlanResourceV1>(missing).is_err(),
                "V1 resource must reject an omitted {field} slot"
            );
        }
    }
    #[test]
    fn onboarding_nested_v1_records_reject_unknown_fields() {
        let fixture = deterministic_account_onboarding_receipt_vector();
        let body = &fixture.receipt_json.body;
        let resource = &body.resource;
        let AliasIntentV1::AccountAlias(account_intent) = &resource.intent else {
            panic!("onboarding fixture must contain an account-alias intent");
        };
        let quote = resource
            .quote
            .as_ref()
            .expect("onboarding fixture must contain a lease quote");

        let resolved_dataspace = ResolvedDataSpaceV1::new(
            "paynet".parse().expect("dataspace name"),
            DataSpaceId::new(7),
        );
        let resolved_domain = ResolvedDomainV1::new(
            DomainId::try_new("banka", "paynet").expect("resolved domain"),
            DataSpaceId::new(7),
        );
        let dataspace_intent = AliasDataSpaceIntentV1 {
            dataspace: resolved_dataspace.clone(),
            owner: account(0x61),
        };
        let domain_intent = AliasDomainIntentV1 {
            domain: resolved_domain.clone(),
            owner: account(0x62),
        };

        assert_json_unknown_field_rejected!(
            &account_intent.alias.canonical_name,
            AccountAliasName,
            "account alias name"
        );
        assert_json_unknown_field_rejected!(
            &resolved_dataspace,
            ResolvedDataSpaceV1,
            "resolved dataspace"
        );
        assert_json_unknown_field_rejected!(&resolved_domain, ResolvedDomainV1, "resolved domain");
        assert_json_unknown_field_rejected!(
            &account_intent.alias,
            ResolvedAccountAliasV1,
            "resolved account alias"
        );
        assert_json_unknown_field_rejected!(
            &dataspace_intent,
            AliasDataSpaceIntentV1,
            "dataspace intent"
        );
        assert_json_unknown_field_rejected!(&domain_intent, AliasDomainIntentV1, "domain intent");
        assert_json_unknown_field_rejected!(
            account_intent,
            AliasAccountIntentV1,
            "account-alias intent"
        );
        assert_json_unknown_field_rejected!(
            &body.acquisition,
            AliasLeaseAcquisitionV1,
            "lease acquisition"
        );
        assert_json_unknown_field_rejected!(&body.quote_guard, AliasQuoteGuardV1, "quote guard");
        assert_json_unknown_field_rejected!(quote, AliasLeaseQuoteV1, "lease quote");
        assert_json_unknown_field_rejected!(resource, AliasPlanResourceV1, "plan resource");
        assert_json_unknown_field_rejected!(
            &body.instructions[0],
            AliasFramedInstructionV1,
            "framed instruction"
        );
        assert_json_unknown_field_rejected!(&body.anchor, AliasPlanAnchorV1, "plan anchor");
    }
    #[test]
    fn onboarding_nested_v1_tagged_enums_reject_unknown_fields() {
        let fixture = deterministic_account_onboarding_receipt_vector();
        let resource = &fixture.receipt_json.body.resource;
        let quote = resource
            .quote
            .as_ref()
            .expect("onboarding fixture must contain a lease quote");

        assert_json_unknown_field_rejected!(
            AccountProvisionV1::Create,
            AccountProvisionV1,
            "account provision"
        );
        assert_json_unknown_field_rejected!(
            AccountAliasRoleV1::Primary,
            AccountAliasRoleV1,
            "account alias role"
        );
        assert_json_unknown_field_rejected!(&resource.intent, AliasIntentV1, "alias intent");
        assert_json_unknown_field_rejected!(&quote.target, AliasTargetV1, "alias target");
        assert_json_unknown_field_rejected!(
            resource.disposition,
            AliasPlanDispositionV1,
            "plan disposition"
        );
    }
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one shared V1 fixture keeps canonical alias names, hash frames, instruction bytes, and report projections visibly aligned"
    )]
    fn shared_alias_setup_v1_fixture_matches_names_hash_frames_and_report() {
        use crate::isi::alias_setup::{
            CompareAndSetPrimaryAccountAlias, ConfigureAliasAutoRenew, EnsureAlias,
            RebindAccountAlias, RenewAliasLease,
        };
        let fixture: SharedAliasSetupFixture = norito::json::from_str(include_str!(
            "../../../fixtures/norito_rpc/alias_setup_v1/alias_setup_v1.json"
        ))
        .expect("decode shared alias setup fixture");
        assert_eq!(fixture.schema_version, 1);
        let expected_onboarding = deterministic_account_onboarding_receipt_vector();
        assert_eq!(
            fixture.account_onboarding_receipt_vector,
            expected_onboarding
        );
        let onboarding = fixture.account_onboarding_receipt_vector;
        assert_eq!(
            onboarding.domain.as_bytes(),
            ACCOUNT_ONBOARDING_RECEIPT_HASH_DOMAIN_V1
        );
        let onboarding_body = hex::decode(&onboarding.canonical_body_norito_hex)
            .expect("fixture onboarding body hex");
        let decoded_onboarding =
            FixtureAccountOnboardingPlanBodyV1::decode(&mut onboarding_body.as_slice())
                .expect("decode fixture onboarding body");
        assert_eq!(decoded_onboarding, onboarding.receipt_json.body);
        assert_eq!(
            hex::encode(onboarding.receipt_json.body.canonical_hash().as_ref()),
            onboarding.canonical_plan_hash_hex
        );
        assert_eq!(
            onboarding.authority,
            onboarding.receipt_json.body.authority.to_string()
        );
        assert_eq!(
            onboarding.signature_hex,
            hex::encode_upper(onboarding.receipt_json.signature.payload())
        );
        assert!(onboarding.receipt_json.verify());
        for case in fixture.account_alias_cases {
            let parsed: AccountAliasName = case.input.parse().expect("fixture account alias");
            assert_eq!(parsed.to_string(), case.canonical);
            assert_eq!(parsed.label.as_ref(), case.label);
            assert_eq!(
                parsed.domain.as_ref().map(Name::as_ref),
                case.domain.as_deref()
            );
            assert_eq!(parsed.dataspace.as_ref(), case.dataspace);
        }
        let resolved_alias = resolved_alias();
        assert_eq!(
            fixture.resolved_name_json_vectors.dataspace,
            ResolvedDataSpaceV1::new(
                "paynet".parse().expect("dataspace name"),
                DataSpaceId::new(7)
            )
        );
        assert_eq!(
            fixture.resolved_name_json_vectors.domain,
            ResolvedDomainV1::new(
                DomainId::try_new("banka", "paynet").expect("domain"),
                DataSpaceId::new(7),
            )
        );
        assert_eq!(
            fixture.resolved_name_json_vectors.account_alias,
            resolved_alias.clone()
        );
        assert_eq!(
            fixture.quote_guard_json_vector,
            AliasQuoteGuardV1 {
                expected_policy_version: 2,
                expected_payment_asset: "4rPeAP6jAjiLVZThZYwwPRBuQagt"
                    .parse()
                    .expect("fixture payment asset"),
                max_amount: amount(10),
                valid_until_ms: 50_000,
            }
        );
        let permission = fixture
            .permission_scope_json_vector
            .as_object()
            .expect("alias permission scope object");
        assert_eq!(
            permission
                .get("scope")
                .and_then(norito::json::Value::as_str),
            Some("alias")
        );
        let permission_alias: ResolvedAccountAliasV1 =
            norito::json::JsonDeserialize::json_from_value(
                permission
                    .get("value")
                    .expect("alias permission scope value"),
            )
            .expect("decode exact alias permission value");
        assert_eq!(permission_alias, resolved_alias);
        let expected_hash_domains = [
            (
                "setup_account_alias_create",
                ALIAS_TRANSACTION_PLAN_HASH_DOMAIN_V1,
            ),
            (
                "renew_account_alias",
                ALIAS_LIFECYCLE_TRANSACTION_PLAN_HASH_DOMAIN_V1,
            ),
        ];
        assert_eq!(fixture.plan_hash_vectors.len(), expected_hash_domains.len());
        for (name, domain) in expected_hash_domains {
            let vector = fixture
                .plan_hash_vectors
                .iter()
                .find(|vector| vector.name == name)
                .expect("named shared plan hash vector");
            assert_eq!(vector.domain.as_bytes(), domain);
            let body =
                hex::decode(&vector.canonical_body_norito_hex).expect("fixture Norito body hex");
            let hash = Hash::new_from_chunks(&[domain, body.as_slice()]);
            assert_eq!(hex::encode(hash.as_ref()), vector.canonical_plan_hash_hex);
            match name {
                "setup_account_alias_create" => {
                    let decoded = AliasTransactionPlanBodyV1::decode(&mut body.as_slice())
                        .expect("decode shared setup-plan body");
                    assert_eq!(decoded.network_id, shared_plan_network_id());
                    assert_eq!(decoded.encode(), body);
                }
                "renew_account_alias" => {
                    let decoded = AliasLifecycleTransactionPlanBodyV1::decode(&mut body.as_slice())
                        .expect("decode shared lifecycle-plan body");
                    assert_eq!(decoded.network_id, shared_plan_network_id());
                    assert_eq!(decoded.encode(), body);
                }
                _ => unreachable!("expected plan vector names are closed"),
            }
        }
        let expected_frames = [
            ("ensure_account_alias", EnsureAlias::WIRE_ID),
            ("renew_account_alias", RenewAliasLease::WIRE_ID),
            (
                "configure_auto_renew_enable",
                ConfigureAliasAutoRenew::WIRE_ID,
            ),
            (
                "configure_auto_renew_disable",
                ConfigureAliasAutoRenew::WIRE_ID,
            ),
            ("rebind_account_alias", RebindAccountAlias::WIRE_ID),
            (
                "compare_and_set_primary_account_alias",
                CompareAndSetPrimaryAccountAlias::WIRE_ID,
            ),
        ];
        assert_eq!(
            fixture.instruction_frame_vectors.len(),
            expected_frames.len()
        );
        let registry = crate::instruction_registry::default();
        for (name, wire_id) in expected_frames {
            let vector = fixture
                .instruction_frame_vectors
                .iter()
                .find(|vector| vector.name == name)
                .expect("named shared instruction frame vector");
            assert_eq!(vector.wire_id, wire_id);
            let framed =
                hex::decode(&vector.framed_payload_hex).expect("fixture instruction frame hex");
            assert_eq!(framed.get(..4), Some(b"NRT0".as_slice()));
            let decoded = registry
                .decode(wire_id, &framed)
                .expect("registered shared alias instruction")
                .expect("decode shared alias instruction frame");
            let (reencoded_wire_id, reencoded) = crate::isi::framed_instruction_payload(&decoded)
                .expect("re-encode shared alias instruction frame");
            assert_eq!(reencoded_wire_id, wire_id);
            assert_eq!(reencoded, framed);
        }
        let expected_report = AliasSetupReportV1::new(
            AliasSetupStatusV1::Blocked,
            vec![AliasSetupDiagnosticV1 {
                phase: AliasSetupValidationPhaseV1::Catalog,
                code: "alias.catalog.mapping_conflict".to_owned(),
                severity: AliasSetupSeverityV1::Error,
                resource: Some("dataspace:paynet".to_owned()),
                config_path: None,
                expected: Some("7".to_owned()),
                actual: Some("9".to_owned()),
                remediation: "Make the static catalog and active SNS record map paynet to the same dataspace ID."
                    .to_owned(),
            }],
        );
        assert_eq!(fixture.report_json_vector, expected_report);
        let report_json =
            norito::json::to_json(&fixture.report_json_vector).expect("encode shared report JSON");
        let report: AliasSetupReportV1 =
            norito::json::from_str(&report_json).expect("decode shared report JSON");
        assert_eq!(report, fixture.report_json_vector);
    }
    #[test]
    fn account_alias_name_rejects_noncanonical_shapes() {
        for input in [
            "",
            " merchant@paynet",
            "merchant",
            "merchant@",
            "@paynet",
            "merchant@@paynet",
            "merchant@a.b.c",
            "merchant@.paynet",
        ] {
            assert!(
                input.parse::<AccountAliasName>().is_err(),
                "must fail: {input}"
            );
        }
    }
    #[test]
    fn resolved_names_pin_and_revalidate_dataspace_ids() {
        let alias = resolved_alias();
        assert_eq!(alias.dataspace_id, DataSpaceId::new(7));
        assert!(alias.matches_catalog(&catalog()));
        assert_eq!(alias.account_alias().dataspace, DataSpaceId::new(7));
        let wrong = ResolvedAccountAliasV1::new(alias.canonical_name, DataSpaceId::new(8));
        assert!(!wrong.matches_catalog(&catalog()));
    }
    #[test]
    fn resolved_names_roundtrip_through_norito_and_json() {
        let alias = resolved_alias();
        let bytes = alias.encode();
        let decoded = ResolvedAccountAliasV1::decode(&mut bytes.as_slice()).expect("Norito decode");
        assert_eq!(decoded, alias);
        let json = norito::json::to_json(&alias).expect("JSON encode");
        let decoded: ResolvedAccountAliasV1 = norito::json::from_str(&json).expect("JSON decode");
        assert_eq!(decoded, alias);
    }
    #[test]
    fn setup_dtos_roundtrip_and_plan_hash_detects_changes() {
        let alias = resolved_alias();
        let authority = account(0xA1);
        let guard = AliasQuoteGuardV1 {
            expected_policy_version: 3,
            expected_payment_asset: payment_asset(),
            max_amount: amount(5),
            valid_until_ms: 20_000,
        };
        let intent = AliasIntentV1::AccountAlias(AliasAccountIntentV1 {
            alias: alias.clone(),
            target_account: authority.clone(),
            provision: AccountProvisionV1::Create,
            role: AccountAliasRoleV1::Primary,
        });
        let request =
            AliasSetupPlanRequestV1::new(vec![crate::isi::alias_setup::EnsureAlias::new(
                intent.clone(),
                AliasLeaseAcquisitionV1::new(1, None),
                guard.clone(),
            )]);
        let encoded_request = norito::to_bytes(&request).expect("encode setup request");
        let decoded_request: AliasSetupPlanRequestV1 =
            norito::decode_from_bytes(&encoded_request).expect("decode setup request");
        assert_eq!(decoded_request, request);
        let request_json = norito::json::to_json(&request).expect("JSON encode setup request");
        let decoded_request: AliasSetupPlanRequestV1 =
            norito::json::from_str(&request_json).expect("JSON decode setup request");
        assert_eq!(decoded_request, request);
        let body = AliasTransactionPlanBodyV1 {
            version: AliasTransactionPlanBodyV1::VERSION,
            authority,
            network_id: plan_network_id(0xA1),
            anchor: AliasPlanAnchorV1 {
                block_height: 9,
                block_hash: Hash::new(b"anchor"),
            },
            resources: vec![AliasPlanResourceV1 {
                intent,
                disposition: AliasPlanDispositionV1::Create,
                quote: Some(AliasLeaseQuoteV1 {
                    target: AliasTargetV1::AccountAlias(alias),
                    pricing_class: 1,
                    exact_amount: amount(3),
                    guard,
                    expires_at_ms: 100,
                    grace_expires_at_ms: 200,
                    redemption_expires_at_ms: 300,
                }),
                instruction_index: Some(0),
            }],
            instructions: vec![AliasFramedInstructionV1 {
                wire_id: "iroha.alias.ensure".to_owned(),
                framed_payload: vec![1, 2, 3],
            }],
            totals_by_asset: vec![AliasAssetTotalV1 {
                payment_asset: payment_asset(),
                amount: amount(3),
            }],
            warnings: Vec::new(),
            blockers: Vec::new(),
            valid_until_ms: 20_000,
        };
        let plan = AliasTransactionPlanV1::new(body);
        assert!(plan.verify_hash());
        let encoded = norito::to_bytes(&plan).expect("encode plan");
        let decoded: AliasTransactionPlanV1 =
            norito::decode_from_bytes(&encoded).expect("decode plan");
        assert_eq!(decoded, plan);
        let json = norito::json::to_json(&plan).expect("JSON encode");
        let decoded: AliasTransactionPlanV1 = norito::json::from_str(&json).expect("JSON decode");
        assert_eq!(decoded, plan);
        let legacy_json = json.replacen("\"network_id\":", "\"chain_id\":", 1);
        assert_ne!(legacy_json, json, "plan JSON exposes the exact NetworkId");
        assert!(
            norito::json::from_str::<AliasTransactionPlanV1>(&legacy_json).is_err(),
            "retired chain_id plan fields must fail closed"
        );
        let mut tampered = plan;
        tampered.body.valid_until_ms += 1;
        assert!(!tampered.verify_hash());
    }
    #[test]
    fn lifecycle_planner_dtos_roundtrip_and_hash_detects_changes() {
        use crate::isi::alias_setup::{ConfigureAliasAutoRenew, RenewAliasLease};
        let authority = account(0xA3);
        let target = AliasTargetV1::AccountAlias(resolved_alias());
        let guard = AliasQuoteGuardV1 {
            expected_policy_version: 3,
            expected_payment_asset: payment_asset(),
            max_amount: amount(5),
            valid_until_ms: 20_000,
        };
        let renewal = RenewAliasLease::new(target.clone(), 1_000, 2_000, guard.clone());
        let renewal_request = AliasLeaseRenewPlanRequestV1::new(renewal.clone());
        let encoded = norito::to_bytes(&renewal_request).expect("encode renewal request");
        let decoded: AliasLeaseRenewPlanRequestV1 =
            norito::decode_from_bytes(&encoded).expect("decode renewal request");
        assert_eq!(decoded, renewal_request);
        let json = norito::json::to_json(&renewal_request).expect("JSON encode renewal request");
        let decoded: AliasLeaseRenewPlanRequestV1 =
            norito::json::from_str(&json).expect("JSON decode renewal request");
        assert_eq!(decoded, renewal_request);
        let configuration = ConfigureAliasAutoRenew::new(
            target.clone(),
            4,
            Some(AliasAutoRenewConfigV1 {
                term_years: 1,
                policy_version: 3,
                payment_asset: payment_asset(),
                max_amount: amount(5),
                renew_before_expiry_ms: 100,
                retry_backoff_ms: 50,
                max_failures: 5,
            }),
        );
        let configuration_request = AliasAutoRenewPlanRequestV1::new(configuration);
        let encoded = norito::to_bytes(&configuration_request).expect("encode auto-renew request");
        let decoded: AliasAutoRenewPlanRequestV1 =
            norito::decode_from_bytes(&encoded).expect("decode auto-renew request");
        assert_eq!(decoded, configuration_request);
        let json =
            norito::json::to_json(&configuration_request).expect("JSON encode auto-renew request");
        let decoded: AliasAutoRenewPlanRequestV1 =
            norito::json::from_str(&json).expect("JSON decode auto-renew request");
        assert_eq!(decoded, configuration_request);
        let body = AliasLifecycleTransactionPlanBodyV1 {
            version: AliasLifecycleTransactionPlanBodyV1::VERSION,
            authority,
            network_id: plan_network_id(0xA2),
            anchor: AliasPlanAnchorV1 {
                block_height: 10,
                block_hash: Hash::new(b"lifecycle-anchor"),
            },
            operation: AliasLifecycleOperationV1::RenewLease(renewal),
            disposition: AliasLifecyclePlanDispositionV1::Apply,
            instruction: Some(AliasFramedInstructionV1 {
                wire_id: "iroha.alias.lease.renew".to_owned(),
                framed_payload: vec![4, 5, 6],
            }),
            quote: Some(AliasLeaseQuoteV1 {
                target,
                pricing_class: 1,
                exact_amount: amount(3),
                guard,
                expires_at_ms: 2_000,
                grace_expires_at_ms: 2_100,
                redemption_expires_at_ms: 2_200,
            }),
            totals_by_asset: vec![AliasAssetTotalV1 {
                payment_asset: payment_asset(),
                amount: amount(3),
            }],
            warnings: Vec::new(),
            blockers: Vec::new(),
            valid_until_ms: 20_000,
        };
        let plan = AliasLifecycleTransactionPlanV1::new(body);
        assert!(plan.verify_hash());
        let encoded = norito::to_bytes(&plan).expect("encode lifecycle plan");
        let decoded: AliasLifecycleTransactionPlanV1 =
            norito::decode_from_bytes(&encoded).expect("decode lifecycle plan");
        assert_eq!(decoded, plan);
        let json = norito::json::to_json(&plan).expect("JSON encode lifecycle plan");
        let decoded: AliasLifecycleTransactionPlanV1 =
            norito::json::from_str(&json).expect("JSON decode lifecycle plan");
        assert_eq!(decoded, plan);
        let legacy_json = json.replacen("\"network_id\":", "\"chain_id\":", 1);
        assert_ne!(legacy_json, json, "plan JSON exposes the exact NetworkId");
        assert!(
            norito::json::from_str::<AliasLifecycleTransactionPlanV1>(&legacy_json).is_err(),
            "retired chain_id lifecycle-plan fields must fail closed"
        );
        let mut tampered = plan;
        tampered.body.valid_until_ms += 1;
        assert!(!tampered.verify_hash());
    }
    #[test]
    fn auto_renew_state_roundtrips_for_enable_and_disable() {
        let alias = resolved_alias();
        let owner = account(0xA2);
        let config = AliasAutoRenewConfigV1 {
            term_years: 1,
            policy_version: 3,
            payment_asset: payment_asset(),
            max_amount: amount(5),
            renew_before_expiry_ms: 86_400_000,
            retry_backoff_ms: 3_600_000,
            max_failures: 5,
        };
        for state in [
            AliasAutoRenewStateV1::new(
                AliasTargetV1::AccountAlias(alias.clone()),
                owner.clone(),
                1,
                Some(config),
            ),
            AliasAutoRenewStateV1::new(
                AliasTargetV1::AccountAlias(alias.clone()),
                owner.clone(),
                2,
                None,
            ),
        ] {
            let encoded = norito::to_bytes(&state).expect("encode auto-renew state");
            let decoded: AliasAutoRenewStateV1 =
                norito::decode_from_bytes(&encoded).expect("decode auto-renew state");
            assert_eq!(decoded, state);
            let json = norito::json::to_json(&state).expect("JSON encode auto-renew state");
            let decoded: AliasAutoRenewStateV1 =
                norito::json::from_str(&json).expect("JSON decode auto-renew state");
            assert_eq!(decoded, state);
        }
    }
    #[test]
    fn report_diagnostics_are_sorted_deterministically() {
        let diagnostic = |code: &str| AliasSetupDiagnosticV1 {
            phase: AliasSetupValidationPhaseV1::Config,
            code: code.to_owned(),
            severity: AliasSetupSeverityV1::Error,
            resource: None,
            config_path: None,
            expected: None,
            actual: None,
            remediation: "fix configuration".to_owned(),
        };
        let report = AliasSetupReportV1::new(
            AliasSetupStatusV1::Blocked,
            vec![diagnostic("z.code"), diagnostic("a.code")],
        );
        assert_eq!(report.diagnostics[0].code, "a.code");
    }
}
