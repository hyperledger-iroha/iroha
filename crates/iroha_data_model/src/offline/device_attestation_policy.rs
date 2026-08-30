/// Deterministic snapshot of Google's Android Key Attestation status list.
///
/// Governance pins the exact upstream payload digest together with its HTTP
/// freshness metadata and the canonical set of certificate serials whose
/// status is not valid. Consensus consumes this snapshot without performing
/// network I/O.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct OfflineAndroidAttestationStatusSnapshotV1 {
    /// Snapshot layout marker.
    pub version: u16,
    /// SHA-256 digest of the exact upstream response payload.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub payload_sha256: [u8; 32],
    /// Upstream HTTP `Date` value in Unix milliseconds.
    pub response_date_ms: u64,
    /// Optional upstream HTTP `Last-Modified` value in Unix milliseconds.
    pub last_modified_ms: Option<u64>,
    /// Upstream `Cache-Control: max-age` lifetime in seconds.
    pub cache_max_age_seconds: u32,
    /// Canonical lowercase hexadecimal serials whose status is not valid.
    pub non_valid_serials: Vec<String>,
}

/// Hardware security boundary reported by Android Key Attestation.
///
/// Software-backed keys never reach this type: native certificate and
/// authorization-list verification rejects them before policy evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(tag = "security_level", content = "value", rename_all = "snake_case")]
pub enum OfflineAndroidDeviceSecurityLevelV2 {
    /// A key isolated by the device trusted execution environment.
    TrustedEnvironment,
    /// A key isolated by a discrete `StrongBox` secure element.
    StrongBox,
}

/// Complete Android properties authenticated by one Key Attestation leaf.
///
/// The five build identity strings come from the `attestationId*` tags. The
/// version and patch values, root-of-trust material, and security level come
/// from the same hardware-enforced `KeyDescription`. Keeping the exact values in
/// registration state lets a later governed CVE rule re-evaluate an existing
/// device without trusting mutable application metadata.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct OfflineAndroidAttestedDevicePropertiesV2 {
    /// Property layout marker.
    pub version: u16,
    /// Attestation extension version.
    pub attestation_version: u32,
    /// KeyMint/Keymaster implementation version.
    pub keymint_version: u32,
    /// Hardware boundary protecting the attested key.
    pub security_level: OfflineAndroidDeviceSecurityLevelV2,
    /// Attested build brand.
    pub brand: String,
    /// Attested build device code name.
    pub device: String,
    /// Attested build product name.
    pub product: String,
    /// Attested device manufacturer.
    pub manufacturer: String,
    /// Attested device model.
    pub model: String,
    /// Android `osVersion` integer from Key Attestation.
    pub os_version: u32,
    /// Android `osPatchLevel` integer from Key Attestation.
    pub os_patch_level: u32,
    /// Android `vendorPatchLevel` integer from Key Attestation.
    pub vendor_patch_level: u32,
    /// Android `bootPatchLevel` integer from Key Attestation.
    pub boot_patch_level: u32,
    /// Exact verified-boot key bytes from `rootOfTrust`.
    pub verified_boot_key: Vec<u8>,
    /// Exact verified-boot hash from `rootOfTrust`.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub verified_boot_hash: [u8; 32],
}

fn canonical_android_patch_month_v2(value: u32) -> bool {
    let year = value / 100;
    let month = value % 100;
    (2_000..=9_999).contains(&year) && (1..=12).contains(&month)
}

fn canonical_android_patch_date_v2(value: u32) -> bool {
    let year = value / 10_000;
    let month = (value / 100) % 100;
    let day = value % 100;
    if !(2_000..=9_999).contains(&year) || !(1..=12).contains(&month) {
        return false;
    }
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let maximum_day = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    (1..=maximum_day).contains(&day)
}

impl OfflineAndroidAttestedDevicePropertiesV2 {
    /// Return whether every property required for testnet eligibility is
    /// present and canonically bounded.
    #[must_use]
    pub fn is_complete_v2(&self) -> bool {
        self.version == OFFLINE_ANDROID_ATTESTED_DEVICE_PROPERTIES_VERSION_V2
            && self.attestation_version > 0
            && self.keymint_version > 0
            && self.os_version > 0
            && self.os_version <= OFFLINE_ANDROID_OS_VERSION_MAX_V2
            && canonical_android_patch_month_v2(self.os_patch_level)
            && canonical_android_patch_date_v2(self.vendor_patch_level)
            && canonical_android_patch_date_v2(self.boot_patch_level)
            && !self.verified_boot_key.is_empty()
            && self.verified_boot_key.len() <= OFFLINE_ANDROID_VERIFIED_BOOT_KEY_MAX_BYTES_V2
            && self.verified_boot_hash != [0; 32]
            && [
                self.brand.as_str(),
                self.device.as_str(),
                self.product.as_str(),
                self.manufacturer.as_str(),
                self.model.as_str(),
            ]
            .into_iter()
            .all(|value| {
                !value.is_empty()
                    && value.len() <= OFFLINE_ANDROID_ATTESTED_PROPERTY_MAX_BYTES_V2
                    && value.is_ascii()
                    && !value.chars().any(char::is_control)
                    && value.trim() == value
            })
    }
}

/// One reviewed Android firmware vulnerability rule.
///
/// Rules select an exact manufacturer and may further narrow the match with an
/// exact model, build identifiers, affected OS range, or verified-boot
/// material. A matched device is safe only when it meets every configured
/// floor. A permanent block has no safe firmware floor and always yields
/// drain-only behavior.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct OfflineAndroidDeviceVulnerabilityRuleV2 {
    /// Stable governance identifier. Policy order is ascending by this field.
    pub rule_id: String,
    /// Lowercase exact attested manufacturer selector.
    pub manufacturer: String,
    /// Optional lowercase exact attested model selector.
    ///
    /// Vendor bulletins that apply across a complete Android-version family
    /// leave this absent rather than inventing an incomplete device list.
    pub model: Option<String>,
    /// Optional lowercase attested model-code family prefix.
    ///
    /// This is mutually exclusive with `model`. Prefix matching is ASCII
    /// case-insensitive because Android attestation preserves vendor casing.
    pub model_prefix: Option<String>,
    /// Optional lowercase exact attested brand selector.
    pub brand: Option<String>,
    /// Optional lowercase exact attested device selector.
    pub device: Option<String>,
    /// Optional lowercase exact attested product selector.
    pub product: Option<String>,
    /// Optional SHA-256 selector for exact verified-boot key bytes.
    pub verified_boot_key_sha256: Option<[u8; 32]>,
    /// Optional exact verified-boot hash selector.
    pub verified_boot_hash: Option<[u8; 32]>,
    /// Optional inclusive lower `osVersion` selector.
    pub affected_os_version_min: Option<u32>,
    /// Optional inclusive upper `osVersion` selector.
    pub affected_os_version_max: Option<u32>,
    /// Minimum KeyMint/Keymaster version known to contain the fix.
    pub minimum_safe_keymint_version: Option<u32>,
    /// Minimum Android `osVersion` known to contain the fix.
    pub minimum_safe_os_version: Option<u32>,
    /// Minimum Android OS patch level known to contain the fix.
    pub minimum_safe_os_patch_level: Option<u32>,
    /// Minimum Android vendor patch level known to contain the fix.
    pub minimum_safe_vendor_patch_level: Option<u32>,
    /// Minimum Android boot patch level known to contain the fix.
    pub minimum_safe_boot_patch_level: Option<u32>,
    /// Permanently block every matching firmware from new offline activity.
    pub permanently_blocked: bool,
    /// Sorted unique vendor advisory or review source identifiers.
    pub source_ids: Vec<String>,
    /// Sorted unique canonical CVE identifiers.
    pub cve_ids: Vec<String>,
}

impl OfflineAndroidDeviceVulnerabilityRuleV2 {
    fn matches_v2(&self, properties: &OfflineAndroidAttestedDevicePropertiesV2) -> bool {
        fn exact(candidate: &str, expected: &str) -> bool {
            candidate.eq_ignore_ascii_case(expected)
        }
        fn prefix(candidate: &str, expected: &str) -> bool {
            candidate
                .get(..expected.len())
                .is_some_and(|candidate_prefix| candidate_prefix.eq_ignore_ascii_case(expected))
        }
        if !exact(&properties.manufacturer, &self.manufacturer)
            || self
                .model
                .as_deref()
                .is_some_and(|expected| !exact(&properties.model, expected))
            || self
                .model_prefix
                .as_deref()
                .is_some_and(|expected| !prefix(&properties.model, expected))
            || self
                .brand
                .as_deref()
                .is_some_and(|expected| !exact(&properties.brand, expected))
            || self
                .device
                .as_deref()
                .is_some_and(|expected| !exact(&properties.device, expected))
            || self
                .product
                .as_deref()
                .is_some_and(|expected| !exact(&properties.product, expected))
            || self
                .verified_boot_hash
                .is_some_and(|expected| expected != properties.verified_boot_hash)
            || self
                .affected_os_version_min
                .is_some_and(|minimum| properties.os_version < minimum)
            || self
                .affected_os_version_max
                .is_some_and(|maximum| properties.os_version > maximum)
        {
            return false;
        }
        if let Some(expected) = self.verified_boot_key_sha256 {
            let actual: [u8; 32] = Sha256::digest(&properties.verified_boot_key).into();
            if actual != expected {
                return false;
            }
        }
        true
    }

    fn firmware_is_safe_v2(&self, properties: &OfflineAndroidAttestedDevicePropertiesV2) -> bool {
        !self.permanently_blocked
            && self
                .minimum_safe_keymint_version
                .is_none_or(|floor| properties.keymint_version >= floor)
            && self
                .minimum_safe_os_version
                .is_none_or(|floor| properties.os_version >= floor)
            && self
                .minimum_safe_os_patch_level
                .is_none_or(|floor| properties.os_patch_level >= floor)
            && self
                .minimum_safe_vendor_patch_level
                .is_none_or(|floor| properties.vendor_patch_level >= floor)
            && self
                .minimum_safe_boot_patch_level
                .is_none_or(|floor| properties.boot_patch_level >= floor)
    }
}

/// Closed device-policy outcome used by native and mobile state machines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(tag = "outcome", content = "value", rename_all = "snake_case")]
pub enum OfflineDeviceEligibilityOutcomeV1 {
    /// New top-up, receive, and peer-spend operations are permitted.
    Eligible,
    /// Existing cash is retained, but no new offline value may be accepted or spent.
    DrainOnly,
    /// The cryptographic attestation boundary failed and no device state is trusted.
    CryptographicallyRejected,
}

/// Stable reason associated with a device-policy outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(tag = "reason", content = "value", rename_all = "snake_case")]
pub enum OfflineDeviceEligibilityReasonV1 {
    /// Complete hardware evidence satisfies current policy.
    PolicySatisfied,
    /// Native certificate, signature, key security, or verified-boot checks failed.
    CryptographicAttestationRejected,
    /// The finalized policy or its authenticated status snapshot is stale or rolled back.
    PolicyNotFresh,
    /// Required attested device properties are incomplete.
    IncompleteAttestedProperties,
    /// A pre-Android-12 key is TEE-backed instead of StrongBox-backed.
    UnsupportedPreAndroid12Tee,
    /// A reviewed vulnerability rule matched firmware below a safe floor.
    VulnerableFirmware,
    /// A reviewed vulnerability rule permanently blocks the exact device identity.
    PermanentlyBlockedDevice,
}

/// Maximum UTF-8 byte length of the authenticated human detail retained for one
/// committed device-eligibility rejection.
pub const OFFLINE_DEVICE_ELIGIBILITY_REJECTION_DETAIL_MAX_BYTES_V1: usize = 1_024;

/// Deterministic result of evaluating one device against finalized policy.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct OfflineDeviceEligibilityDecisionV1 {
    /// Closed eligibility state.
    pub outcome: OfflineDeviceEligibilityOutcomeV1,
    /// Stable reason for the state.
    pub reason: OfflineDeviceEligibilityReasonV1,
    /// Sorted rule identifiers that caused drain-only behavior.
    pub matched_rule_ids: Vec<String>,
}

/// Shape error for a typed offline-device eligibility decision or rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum OfflineDeviceEligibilityDecisionV1Error {
    /// The closed outcome, reason, and matched-rule combination is invalid.
    #[error("Offline device eligibility decision has an invalid shape")]
    InvalidDecision,
    /// A committed rejection attempted to carry an eligible decision.
    #[error("Offline device eligibility rejection must not carry an eligible decision")]
    EligibleRejection,
    /// The authenticated rejection detail is not canonical bounded UTF-8 text.
    #[error("Offline device eligibility rejection detail is invalid")]
    InvalidDetail,
}

impl OfflineDeviceEligibilityDecisionV1 {
    /// Validate the closed outcome/reason relation and the canonical matched-rule set.
    ///
    /// # Errors
    ///
    /// Returns [`OfflineDeviceEligibilityDecisionV1Error::InvalidDecision`] when
    /// the reason does not belong to the outcome or the rule identifiers are not
    /// sorted, unique, printable, and bounded.
    pub fn validate_v1(&self) -> Result<(), OfflineDeviceEligibilityDecisionV1Error> {
        let rules = &self.matched_rule_ids;
        if rules.len() > OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_VULNERABILITY_RULES_V2
            || rules.windows(2).any(|pair| pair[0] >= pair[1])
            || rules.iter().any(|rule| {
                rule.is_empty()
                    || rule.len() > OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_RULE_ID_BYTES_V2
                    || !rule.is_ascii()
                    || rule.trim() != rule
                    || rule.chars().any(char::is_control)
            })
        {
            return Err(OfflineDeviceEligibilityDecisionV1Error::InvalidDecision);
        }
        let valid = matches!(
            (self.outcome, self.reason, rules.is_empty()),
            (
                OfflineDeviceEligibilityOutcomeV1::Eligible,
                OfflineDeviceEligibilityReasonV1::PolicySatisfied,
                true
            ) | (
                OfflineDeviceEligibilityOutcomeV1::CryptographicallyRejected,
                OfflineDeviceEligibilityReasonV1::CryptographicAttestationRejected,
                true
            ) | (
                OfflineDeviceEligibilityOutcomeV1::DrainOnly,
                OfflineDeviceEligibilityReasonV1::PolicyNotFresh
                    | OfflineDeviceEligibilityReasonV1::IncompleteAttestedProperties
                    | OfflineDeviceEligibilityReasonV1::UnsupportedPreAndroid12Tee,
                true
            ) | (
                OfflineDeviceEligibilityOutcomeV1::DrainOnly,
                OfflineDeviceEligibilityReasonV1::VulnerableFirmware
                    | OfflineDeviceEligibilityReasonV1::PermanentlyBlockedDevice,
                false
            )
        );
        if !valid {
            return Err(OfflineDeviceEligibilityDecisionV1Error::InvalidDecision);
        }
        Ok(())
    }
}

/// Typed decision retained in a committed result when fresh device registration
/// cannot create protected state.
///
/// The exact submitted attestation remains in signed transaction history. This
/// type carries only the deterministic policy outcome, public rule identifiers,
/// and a bounded authenticated explanation; it is never active registration
/// state and cannot be exchanged for an eligibility credential.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema, thiserror::Error,
)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[error("offline_reason::device_eligibility:{detail}")]
pub struct OfflineDeviceEligibilityRejectionV1 {
    /// Closed non-eligible outcome committed by consensus.
    pub decision: OfflineDeviceEligibilityDecisionV1,
    /// Canonical bounded human explanation authenticated by the result hash.
    pub detail: String,
}

impl OfflineDeviceEligibilityRejectionV1 {
    /// Construct and validate one committed typed registration rejection.
    ///
    /// # Errors
    ///
    /// Returns an error for an eligible or malformed decision, or non-canonical
    /// rejection text.
    pub fn new_v1(
        decision: OfflineDeviceEligibilityDecisionV1,
        detail: impl Into<String>,
    ) -> Result<Self, OfflineDeviceEligibilityDecisionV1Error> {
        let rejection = Self {
            decision,
            detail: detail.into(),
        };
        rejection.validate_v1()?;
        Ok(rejection)
    }

    /// Validate the canonical committed rejection shape.
    ///
    /// # Errors
    ///
    /// Returns an error for an eligible or malformed decision, or non-canonical
    /// rejection text.
    pub fn validate_v1(&self) -> Result<(), OfflineDeviceEligibilityDecisionV1Error> {
        self.decision.validate_v1()?;
        if self.decision.outcome == OfflineDeviceEligibilityOutcomeV1::Eligible {
            return Err(OfflineDeviceEligibilityDecisionV1Error::EligibleRejection);
        }
        if self.detail.is_empty()
            || self.detail.len() > OFFLINE_DEVICE_ELIGIBILITY_REJECTION_DETAIL_MAX_BYTES_V1
            || self.detail.trim() != self.detail
            || self.detail.chars().any(char::is_control)
        {
            return Err(OfflineDeviceEligibilityDecisionV1Error::InvalidDetail);
        }
        Ok(())
    }
}

/// Shape error for governed device-policy V2 rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum OfflineDeviceAttestationPolicyV2Error {
    /// A bounded or canonical policy invariant is not satisfied.
    #[error("{0}")]
    Invalid(&'static str),
}

fn canonical_nonempty_printable_ascii(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value.is_ascii()
        && !value.chars().any(char::is_control)
        && value.trim() == value
}

fn canonical_lowercase_selector(value: &str) -> bool {
    canonical_nonempty_printable_ascii(value, OFFLINE_ANDROID_ATTESTED_PROPERTY_MAX_BYTES_V2)
        && value.bytes().all(|byte| !byte.is_ascii_uppercase())
}

fn canonical_cve_id(value: &str) -> bool {
    let mut components = value.split('-');
    let prefix = components.next();
    let year = components.next();
    let sequence = components.next();
    components.next().is_none()
        && prefix == Some("CVE")
        && year.is_some_and(|value| {
            value.len() == 4 && value.bytes().all(|byte| byte.is_ascii_digit())
        })
        && sequence.is_some_and(|value| {
            (4..=19).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_digit())
        })
}

const OFFLINE_SAMSUNG_J3_TOP_UPDATE_HISTORY_SOURCE_V2: &str =
    "https://doc.samsungmobile.com/SM-J337W/002040181023/eng.html";
const OFFLINE_SAMSUNG_NOTE9_UPDATE_HISTORY_SOURCE_V2: &str =
    "https://doc.samsungmobile.com/SM-N960F/001244180820/eng.html";
const OFFLINE_SAMSUNG_S10_LITE_UPDATE_HISTORY_SOURCE_V2: &str =
    "https://doc.samsungmobile.com/SM-G770F/011910200316/eng.html";
const OFFLINE_SAMSUNG_S20_FE_UPDATE_HISTORY_SOURCE_V2: &str =
    "https://doc.samsungmobile.com/SM-G780F/014801201001/eng.html";
const OFFLINE_SAMSUNG_S20_FE_5G_UPDATE_HISTORY_SOURCE_V2: &str =
    "https://doc.samsungmobile.com/SM-G781B/014790201001/eng.html";

fn reviewed_samsung_2021_rule_v2(
    rule_stem: &str,
    model: (&str, &str, Option<&str>),
    affected_os_version_min: u32,
    affected_os_version_max: u32,
    minimum_safe_os_patch_level: u32,
    bulletin_source: &str,
    cve_id: &str,
) -> OfflineAndroidDeviceVulnerabilityRuleV2 {
    let (model_prefix, model_source, update_history_source) = model;
    let mut source_ids = vec![
        model_source.to_owned(),
        bulletin_source.to_owned(),
        OFFLINE_SAMSUNG_FIRMWARE_UPDATES_2021_ARCHIVE_SOURCE_V2.to_owned(),
        OFFLINE_SAMSUNG_KEYMASTER_USENIX_2022_SOURCE_V2.to_owned(),
    ];
    if let Some(update_history_source) = update_history_source {
        source_ids.push(update_history_source.to_owned());
    }
    source_ids.sort();
    OfflineAndroidDeviceVulnerabilityRuleV2 {
        rule_id: format!("{rule_stem}-{model_prefix}"),
        manufacturer: "samsung".to_owned(),
        model: None,
        model_prefix: Some(model_prefix.to_owned()),
        brand: None,
        device: None,
        product: None,
        verified_boot_key_sha256: None,
        verified_boot_hash: None,
        affected_os_version_min: Some(affected_os_version_min),
        affected_os_version_max: Some(affected_os_version_max),
        minimum_safe_keymint_version: None,
        minimum_safe_os_version: None,
        minimum_safe_os_patch_level: Some(minimum_safe_os_patch_level),
        minimum_safe_vendor_patch_level: None,
        minimum_safe_boot_patch_level: None,
        permanently_blocked: false,
        source_ids,
        cve_ids: vec![cve_id.to_owned()],
    }
}

/// Return the reviewed Samsung Keymaster rules safe to ship with policy V2.
///
/// These rules are candidates for a signed governance transaction; merely
/// upgrading node software does not mutate chain policy. CVE-2026-21046 is
/// scoped to Samsung and the inclusive Android versions named by its vendor
/// bulletin, so it is not a generic patch-age cutoff. Samsung publishes the fix
/// as an SMR month, which maps to the canonical Key Attestation `osPatchLevel`
/// encoding (`YYYYMM`); it does not establish a `vendorPatchLevel` day.
///
/// CVE-2021-25444 and CVE-2021-25490 are limited to exact Samsung model-code
/// family prefixes cross-checked against Samsung product material and the
/// USENIX disclosure. The first family covers every model family in Samsung's
/// disclosed August 2021 patch list (S9/S9+, Note9, J3 Top, J7 Top, J7 Duo,
/// Tab S4, Tab A S Lite, A6+, and A9S) on Android O/P/Q. Samsung's CVE row
/// labels Android O as 8.1, while the official J3 Top update history records
/// Android 8.0 for that disclosed family. The exact-model rules conservatively
/// retain Android 8.0 as the lower bound and pin that history as evidence. The
/// second covers the enumerated S10/S20/S21 families, including S10 Lite and
/// both S20 FE variants, on Android P/Q/R through the October 2021 SMR floor.
/// Galaxy S8 and genuinely unlisted model codes deliberately remain outside
/// both scopes.
#[must_use]
pub fn reviewed_samsung_android_vulnerability_rules_v2()
-> Vec<OfflineAndroidDeviceVulnerabilityRuleV2> {
    let mut rules = Vec::with_capacity(28);
    for (model_prefix, model_source, update_history_source) in [
        ("sm-a605", OFFLINE_SAMSUNG_MODEL_CODE_MAP_SOURCE_V2, None),
        ("sm-a920", OFFLINE_SAMSUNG_MODEL_CODE_MAP_SOURCE_V2, None),
        ("sm-g960", OFFLINE_SAMSUNG_MODEL_CODE_MAP_SOURCE_V2, None),
        ("sm-g965", OFFLINE_SAMSUNG_MODEL_CODE_MAP_SOURCE_V2, None),
        (
            "sm-j337",
            OFFLINE_SAMSUNG_J3_2018_MODEL_SOURCE_V2,
            Some(OFFLINE_SAMSUNG_J3_TOP_UPDATE_HISTORY_SOURCE_V2),
        ),
        ("sm-j720", OFFLINE_SAMSUNG_J7_DUO_MODEL_SOURCE_V2, None),
        ("sm-j737", OFFLINE_SAMSUNG_J7_2018_MODEL_SOURCE_V2, None),
        (
            "sm-n960",
            OFFLINE_SAMSUNG_MODEL_CODE_MAP_SOURCE_V2,
            Some(OFFLINE_SAMSUNG_NOTE9_UPDATE_HISTORY_SOURCE_V2),
        ),
        ("sm-t387", OFFLINE_SAMSUNG_TAB_A_T387_MODEL_SOURCE_V2, None),
        ("sm-t830", OFFLINE_SAMSUNG_MODEL_CODE_MAP_SOURCE_V2, None),
        ("sm-t835", OFFLINE_SAMSUNG_MODEL_CODE_MAP_SOURCE_V2, None),
        ("sm-t837", OFFLINE_SAMSUNG_TAB_S4_T837_MODEL_SOURCE_V2, None),
    ] {
        rules.push(reviewed_samsung_2021_rule_v2(
            "samsung-cve-2021-25444-keymaster-iv-reuse",
            (model_prefix, model_source, update_history_source),
            80_000,
            109_999,
            202_108,
            OFFLINE_SAMSUNG_SMR_AUGUST_2021_SOURCE_V2,
            "CVE-2021-25444",
        ));
    }
    for (model_prefix, update_history_source) in [
        (
            "sm-g770",
            Some(OFFLINE_SAMSUNG_S10_LITE_UPDATE_HISTORY_SOURCE_V2),
        ),
        (
            "sm-g780",
            Some(OFFLINE_SAMSUNG_S20_FE_UPDATE_HISTORY_SOURCE_V2),
        ),
        (
            "sm-g781",
            Some(OFFLINE_SAMSUNG_S20_FE_5G_UPDATE_HISTORY_SOURCE_V2),
        ),
        ("sm-g970", None),
        ("sm-g973", None),
        ("sm-g975", None),
        ("sm-g977", None),
        ("sm-g980", None),
        ("sm-g981", None),
        ("sm-g985", None),
        ("sm-g986", None),
        ("sm-g988", None),
        ("sm-g991", None),
        ("sm-g996", None),
        ("sm-g998", None),
    ] {
        rules.push(reviewed_samsung_2021_rule_v2(
            "samsung-cve-2021-25490-keymaster-downgrade",
            (
                model_prefix,
                OFFLINE_SAMSUNG_MODEL_CODE_MAP_SOURCE_V2,
                update_history_source,
            ),
            90_000,
            119_999,
            202_110,
            OFFLINE_SAMSUNG_SMR_OCTOBER_2021_SOURCE_V2,
            "CVE-2021-25490",
        ));
    }
    rules.push(OfflineAndroidDeviceVulnerabilityRuleV2 {
        rule_id: "samsung-cve-2026-21046-fabric-keymaster-toctou".to_owned(),
        manufacturer: "samsung".to_owned(),
        model: None,
        model_prefix: None,
        brand: None,
        device: None,
        product: None,
        verified_boot_key_sha256: None,
        verified_boot_hash: None,
        affected_os_version_min: Some(140_000),
        affected_os_version_max: Some(169_999),
        minimum_safe_keymint_version: None,
        minimum_safe_os_version: None,
        minimum_safe_os_patch_level: Some(202_607),
        minimum_safe_vendor_patch_level: None,
        minimum_safe_boot_patch_level: None,
        permanently_blocked: false,
        source_ids: vec![
            OFFLINE_SAMSUNG_SMR_JULY_2026_SOURCE_V2.to_owned(),
            OFFLINE_SAMSUNG_FIRMWARE_UPDATES_2026_ARCHIVE_SOURCE_V2.to_owned(),
        ],
        cve_ids: vec!["CVE-2026-21046".to_owned()],
    });
    rules.sort_by(|left, right| left.rule_id.cmp(&right.rule_id));
    rules
}

/// Legacy policy layout retained only for deterministic state-transition decoding.
///
/// Runtime registration and eligibility never accept this layout. Governance
/// may replace one installed V1 value with a monotonic V2 policy while
/// retaining the authenticated Android status-list anti-rollback watermark.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct OfflineDeviceAttestationPolicyV1 {
    /// Legacy format marker.
    pub version: u16,
    /// Trusted platform roots accepted by the on-chain verifier.
    pub trusted_roots: Vec<OfflineDeviceAttestationTrustedRoot>,
    /// Revoked certificate TBS hashes.
    pub revoked_certificate_tbs_sha256: Vec<Vec<u8>>,
    /// Accepted iOS application identities.
    pub ios_apps: Vec<OfflineIosAppAttestationPolicy>,
    /// Accepted Android application identities.
    pub android_apps: Vec<OfflineAndroidAppAttestationPolicy>,
    /// Governed Android attestation status snapshot.
    pub android_status_snapshot: Option<OfflineAndroidAttestationStatusSnapshotV1>,
    /// Legacy iOS enablement gate.
    pub require_ios_app_policy: bool,
    /// Legacy Android enablement gate.
    pub require_android_app_policy: bool,
}

/// Governed Offline device-attestation verifier policy.
///
/// Nodes require this policy to be installed in chain state before accepting hardware-backed
/// offline registration or transaction authorization. The first-release platform roots are accepted
/// only when included in that explicit governed policy; absence of policy state fails closed.
/// Operators can rotate roots, publish deterministic revocations, and restrict accepted app
/// identities without relying on external middleware state.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct OfflineDeviceAttestationPolicy {
    /// Policy format marker.
    pub version: u16,
    /// Monotonic governance epoch. Any non-identical update must increase it.
    pub policy_epoch: u64,
    /// Trusted platform roots accepted by the on-chain verifier.
    pub trusted_roots: Vec<OfflineDeviceAttestationTrustedRoot>,
    /// SHA-256 digests of the exact raw DER encoding of revoked `TBSCertificate` values.
    pub revoked_certificate_tbs_sha256: Vec<Vec<u8>>,
    /// Accepted iOS App Attest app identities.
    pub ios_apps: Vec<OfflineIosAppAttestationPolicy>,
    /// Accepted Android `KeyMint` app identities.
    pub android_apps: Vec<OfflineAndroidAppAttestationPolicy>,
    /// Governed Android Key Attestation status-list snapshot.
    pub android_status_snapshot: Option<OfflineAndroidAttestationStatusSnapshotV1>,
    /// Sorted, bounded, reviewed Android firmware vulnerability rules.
    pub android_vulnerability_rules: Vec<OfflineAndroidDeviceVulnerabilityRuleV2>,
    /// Explicitly enables iOS registration and online assertions when a matching
    /// entry exists in `ios_apps`.
    ///
    /// iOS App Attest is disabled when this is false; there is no implicit app
    /// identity fallback.
    pub require_ios_app_policy: bool,
    /// Explicitly enables Android registration when a matching entry exists in `android_apps`.
    ///
    /// Android `KeyMint` is disabled when this is false; there is no implicit
    /// unlisted-package or signing-certificate fallback.
    pub require_android_app_policy: bool,
}

fn validate_vulnerability_rule_identity_v2(
    rule: &OfflineAndroidDeviceVulnerabilityRuleV2,
    previous_rule_id: Option<&str>,
) -> Result<(), OfflineDeviceAttestationPolicyV2Error> {
    use OfflineDeviceAttestationPolicyV2Error::Invalid;

    if !canonical_nonempty_printable_ascii(
        &rule.rule_id,
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_RULE_ID_BYTES_V2,
    ) || previous_rule_id.is_some_and(|previous| previous >= rule.rule_id.as_str())
    {
        return Err(Invalid(
            "Offline device vulnerability rules must have sorted unique canonical identifiers",
        ));
    }
    if !canonical_lowercase_selector(&rule.manufacturer)
        || [
            rule.model.as_deref(),
            rule.model_prefix.as_deref(),
            rule.brand.as_deref(),
            rule.device.as_deref(),
            rule.product.as_deref(),
        ]
        .into_iter()
        .flatten()
        .any(|value| !canonical_lowercase_selector(value))
    {
        return Err(Invalid(
            "Offline device vulnerability selectors must be canonical lowercase printable ASCII",
        ));
    }
    if rule.model.is_some() && rule.model_prefix.is_some() {
        return Err(Invalid(
            "Offline device vulnerability exact-model and model-prefix selectors are mutually exclusive",
        ));
    }
    if rule
        .model_prefix
        .as_ref()
        .is_some_and(|prefix| prefix.len() < OFFLINE_ANDROID_ATTESTED_MODEL_PREFIX_MIN_BYTES_V2)
    {
        return Err(Invalid(
            "Offline device vulnerability model-prefix selector is too short",
        ));
    }
    match (rule.affected_os_version_min, rule.affected_os_version_max) {
        (None, None) => {}
        (Some(minimum), Some(maximum))
            if minimum > 0
                && minimum <= maximum
                && maximum <= OFFLINE_ANDROID_OS_VERSION_MAX_V2 => {}
        _ => {
            return Err(Invalid(
                "Offline device vulnerability affected OS range must be a bounded inclusive pair",
            ));
        }
    }
    if rule
        .verified_boot_key_sha256
        .is_some_and(|digest| digest == [0; 32])
        || rule
            .verified_boot_hash
            .is_some_and(|digest| digest == [0; 32])
    {
        return Err(Invalid(
            "Offline device vulnerability verified-boot selectors must be non-zero",
        ));
    }
    Ok(())
}

fn validate_vulnerability_rule_floors_v2(
    rule: &OfflineAndroidDeviceVulnerabilityRuleV2,
) -> Result<(), OfflineDeviceAttestationPolicyV2Error> {
    use OfflineDeviceAttestationPolicyV2Error::Invalid;

    let floors = [
        rule.minimum_safe_keymint_version,
        rule.minimum_safe_os_version,
        rule.minimum_safe_os_patch_level,
        rule.minimum_safe_vendor_patch_level,
        rule.minimum_safe_boot_patch_level,
    ];
    let has_floor = floors.into_iter().any(|floor| floor.is_some());
    let has_zero_floor = floors.into_iter().flatten().any(|floor| floor == 0);
    if (!rule.permanently_blocked && !has_floor) || has_zero_floor {
        return Err(Invalid(
            "Offline device vulnerability rules require non-zero safe floors or a permanent block",
        ));
    }
    if rule
        .minimum_safe_os_version
        .is_some_and(|version| version > OFFLINE_ANDROID_OS_VERSION_MAX_V2)
        || rule
            .minimum_safe_os_patch_level
            .is_some_and(|level| !canonical_android_patch_month_v2(level))
        || rule
            .minimum_safe_vendor_patch_level
            .is_some_and(|level| !canonical_android_patch_date_v2(level))
        || rule
            .minimum_safe_boot_patch_level
            .is_some_and(|level| !canonical_android_patch_date_v2(level))
    {
        return Err(Invalid(
            "Offline device vulnerability safe floors use invalid Android encodings",
        ));
    }
    Ok(())
}

fn validate_vulnerability_rule_sources_v2(
    rule: &OfflineAndroidDeviceVulnerabilityRuleV2,
) -> Result<(), OfflineDeviceAttestationPolicyV2Error> {
    use OfflineDeviceAttestationPolicyV2Error::Invalid;

    if rule.source_ids.is_empty()
        || rule.source_ids.len() > OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_RULE_SOURCES_V2
        || rule.cve_ids.len() > OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_RULE_CVES_V2
    {
        return Err(Invalid(
            "Offline device vulnerability rule source or CVE count is invalid",
        ));
    }
    let sources_are_canonical = rule.source_ids.iter().enumerate().all(|(index, source)| {
        canonical_nonempty_printable_ascii(
            source,
            OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_RULE_SOURCE_BYTES_V2,
        ) && (index == 0 || rule.source_ids[index - 1] < *source)
    });
    if !sources_are_canonical {
        return Err(Invalid(
            "Offline device vulnerability sources must be sorted unique printable ASCII",
        ));
    }
    let cves_are_canonical = rule.cve_ids.iter().enumerate().all(|(index, cve)| {
        cve.len() <= OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_RULE_CVE_BYTES_V2
            && canonical_cve_id(cve)
            && (index == 0 || rule.cve_ids[index - 1] < *cve)
    });
    if !cves_are_canonical {
        return Err(Invalid(
            "Offline device vulnerability CVEs must be sorted unique canonical identifiers",
        ));
    }
    Ok(())
}

impl OfflineDeviceAttestationPolicy {
    /// Encode this policy into its canonical, header-framed Norito representation.
    ///
    /// Decode the returned archive with [`norito::decode_canonical`]. The
    /// derived [`norito::codec::Decode`] implementation consumes the distinct
    /// headerless payload format and is not the inverse of this method.
    ///
    /// # Errors
    ///
    /// Returns a Norito error if canonical encoding fails.
    pub fn canonical_bytes_v2(&self) -> Result<Vec<u8>, norito::Error> {
        norito::encode_canonical(self)
    }

    /// Compute the existing Iroha hash identity of the canonical policy bytes.
    ///
    /// # Errors
    ///
    /// Returns a Norito error if canonical encoding fails.
    pub fn canonical_hash_v2(&self) -> Result<[u8; 32], norito::Error> {
        self.canonical_bytes_v2().map(Hash::new).map(Into::into)
    }

    /// Validate the V2 epoch and deterministic vulnerability-rule shape.
    ///
    /// Certificate, application, status-list freshness, and chain-specific
    /// transition validation remain native Core responsibilities.
    ///
    /// # Errors
    ///
    /// Returns an error when the epoch, collection bounds, ordering, selectors,
    /// floors, sources, or CVE identifiers are not canonical.
    pub fn validate_v2_rule_shape(&self) -> Result<(), OfflineDeviceAttestationPolicyV2Error> {
        use OfflineDeviceAttestationPolicyV2Error::Invalid;

        if self.version != OFFLINE_DEVICE_ATTESTATION_POLICY_VERSION_V2 {
            return Err(Invalid(
                "Offline device attestation policy must use version 2",
            ));
        }
        if self.policy_epoch == 0 {
            return Err(Invalid(
                "Offline device attestation policy epoch must be non-zero",
            ));
        }
        if self.android_vulnerability_rules.len()
            > OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_VULNERABILITY_RULES_V2
        {
            return Err(Invalid(
                "Offline device attestation policy has too many vulnerability rules",
            ));
        }

        let mut previous_rule_id = None;
        for rule in &self.android_vulnerability_rules {
            validate_vulnerability_rule_identity_v2(rule, previous_rule_id)?;
            validate_vulnerability_rule_floors_v2(rule)?;
            validate_vulnerability_rule_sources_v2(rule)?;
            previous_rule_id = Some(rule.rule_id.as_str());
        }
        Ok(())
    }

    /// Evaluate already-verified Android properties under the testnet policy.
    ///
    /// Android 12+ TEE and `StrongBox` devices receive the same eligibility.
    /// Pre-Android-12 devices require `StrongBox`. Complete unknown models are
    /// allowed; there is deliberately no generic patch-age cutoff.
    #[must_use]
    pub fn evaluate_verified_android_device_v2(
        &self,
        properties: Option<&OfflineAndroidAttestedDevicePropertiesV2>,
        policy_is_fresh: bool,
    ) -> OfflineDeviceEligibilityDecisionV1 {
        if self.validate_v2_rule_shape().is_err() || !policy_is_fresh {
            return OfflineDeviceEligibilityDecisionV1 {
                outcome: OfflineDeviceEligibilityOutcomeV1::DrainOnly,
                reason: OfflineDeviceEligibilityReasonV1::PolicyNotFresh,
                matched_rule_ids: Vec::new(),
            };
        }
        let Some(properties) = properties.filter(|properties| properties.is_complete_v2()) else {
            return OfflineDeviceEligibilityDecisionV1 {
                outcome: OfflineDeviceEligibilityOutcomeV1::DrainOnly,
                reason: OfflineDeviceEligibilityReasonV1::IncompleteAttestedProperties,
                matched_rule_ids: Vec::new(),
            };
        };
        if properties.os_version < OFFLINE_ANDROID_12_OS_VERSION_FLOOR_V2
            && properties.security_level != OfflineAndroidDeviceSecurityLevelV2::StrongBox
        {
            return OfflineDeviceEligibilityDecisionV1 {
                outcome: OfflineDeviceEligibilityOutcomeV1::DrainOnly,
                reason: OfflineDeviceEligibilityReasonV1::UnsupportedPreAndroid12Tee,
                matched_rule_ids: Vec::new(),
            };
        }
        let mut vulnerable = Vec::new();
        let mut permanently_blocked = false;
        for rule in &self.android_vulnerability_rules {
            if rule.matches_v2(properties) && !rule.firmware_is_safe_v2(properties) {
                permanently_blocked |= rule.permanently_blocked;
                vulnerable.push(rule.rule_id.clone());
            }
        }
        if !vulnerable.is_empty() {
            return OfflineDeviceEligibilityDecisionV1 {
                outcome: OfflineDeviceEligibilityOutcomeV1::DrainOnly,
                reason: if permanently_blocked {
                    OfflineDeviceEligibilityReasonV1::PermanentlyBlockedDevice
                } else {
                    OfflineDeviceEligibilityReasonV1::VulnerableFirmware
                },
                matched_rule_ids: vulnerable,
            };
        }
        OfflineDeviceEligibilityDecisionV1 {
            outcome: OfflineDeviceEligibilityOutcomeV1::Eligible,
            reason: OfflineDeviceEligibilityReasonV1::PolicySatisfied,
            matched_rule_ids: Vec::new(),
        }
    }

    /// Construct the closed outcome used when native cryptographic attestation
    /// verification rejects the device before policy evaluation.
    #[must_use]
    pub fn cryptographic_rejection_v1() -> OfflineDeviceEligibilityDecisionV1 {
        OfflineDeviceEligibilityDecisionV1 {
            outcome: OfflineDeviceEligibilityOutcomeV1::CryptographicallyRejected,
            reason: OfflineDeviceEligibilityReasonV1::CryptographicAttestationRejected,
            matched_rule_ids: Vec::new(),
        }
    }
}

/// Finalized block identity bound to one policy view and eligibility credential.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct OfflineDevicePolicyFinalityBindingV1 {
    /// Binding layout marker.
    pub version: u16,
    /// Exact Iroha network identity.
    pub network_id: NetworkId,
    /// Finalized block height containing or following the policy transaction.
    pub finalized_block_height: u64,
    /// Exact finalized block hash.
    pub finalized_block_hash: Hash,
    /// Finalized block timestamp in Unix milliseconds.
    pub finalized_block_timestamp_ms: u64,
    /// Hash of the portable finality evidence supplied with the policy view.
    pub finality_evidence_hash: Hash,
}

impl OfflineDevicePolicyFinalityBindingV1 {
    fn validate_v1(&self) -> bool {
        self.version == OFFLINE_DEVICE_POLICY_FINALITY_BINDING_VERSION_V1
            && self.network_id.as_bytes().iter().any(|byte| *byte != 0)
            && self.finalized_block_height > 0
            && self.finalized_block_timestamp_ms > 0
            && self
                .finalized_block_hash
                .as_ref()
                .iter()
                .any(|byte| *byte != 0)
            && self
                .finality_evidence_hash
                .as_ref()
                .iter()
                .any(|byte| *byte != 0)
    }
}

/// Typed finalized query result for the governed device-attestation policy.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct OfflineDeviceAttestationPolicyViewV1 {
    /// View layout marker.
    pub version: u16,
    /// Exact canonical policy bytes read from finalized chain state.
    pub canonical_policy_bytes: Vec<u8>,
    /// Existing Iroha hash bytes of `canonical_policy_bytes`.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub policy_hash: [u8; 32],
    /// Monotonic epoch decoded from the canonical policy.
    pub policy_epoch: u64,
    /// Exclusive wall-clock deadline for use of this cached finalized policy.
    pub freshness_deadline_ms: u64,
    /// Exact canonical `BridgeFinalityProof` bytes retained for native verification.
    pub finality_evidence_bytes: Vec<u8>,
    /// Finalized network/block binding for the state read.
    pub finality: OfflineDevicePolicyFinalityBindingV1,
}

fn validated_android_status_freshness_deadline_v1(
    policy: &OfflineDeviceAttestationPolicy,
) -> Result<u64, OfflineDevicePolicyViewErrorV1> {
    let snapshot = policy
        .android_status_snapshot
        .as_ref()
        .ok_or(OfflineDevicePolicyViewErrorV1::InvalidPolicy)?;
    if snapshot.version != OFFLINE_ANDROID_ATTESTATION_STATUS_SNAPSHOT_VERSION_V1
        || snapshot.payload_sha256 == [0; 32]
        || snapshot.response_date_ms == 0
        || !snapshot.response_date_ms.is_multiple_of(1_000)
        || snapshot.cache_max_age_seconds == 0
        || snapshot.cache_max_age_seconds
            > OFFLINE_ANDROID_ATTESTATION_STATUS_MAX_CACHE_AGE_SECONDS_V1
        || snapshot.last_modified_ms.is_some_and(|last_modified_ms| {
            last_modified_ms == 0
                || !last_modified_ms.is_multiple_of(1_000)
                || last_modified_ms > snapshot.response_date_ms
        })
        || snapshot.non_valid_serials.len()
            > OFFLINE_ANDROID_ATTESTATION_STATUS_MAX_NON_VALID_SERIALS_V1
    {
        return Err(OfflineDevicePolicyViewErrorV1::InvalidPolicy);
    }
    let mut previous_serial: Option<&str> = None;
    for serial in &snapshot.non_valid_serials {
        if serial.is_empty()
            || serial.len() > OFFLINE_ANDROID_ATTESTATION_STATUS_MAX_SERIAL_HEX_BYTES_V1
            || !serial
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
            || (serial.len() > 1 && serial.starts_with('0'))
            || previous_serial.is_some_and(|previous| previous >= serial.as_str())
        {
            return Err(OfflineDevicePolicyViewErrorV1::InvalidPolicy);
        }
        previous_serial = Some(serial);
    }
    snapshot
        .response_date_ms
        .checked_add(u64::from(snapshot.cache_max_age_seconds) * 1_000)
        .ok_or(OfflineDevicePolicyViewErrorV1::InvalidPolicy)
}

impl OfflineDeviceAttestationPolicyViewV1 {
    /// Build a self-consistent finalized policy view.
    ///
    /// # Errors
    ///
    /// Returns an error when the policy, freshness deadline, evidence bytes,
    /// or finality binding is invalid or cannot be canonically encoded.
    pub fn new_v1(
        policy: &OfflineDeviceAttestationPolicy,
        freshness_deadline_ms: u64,
        finality_evidence_bytes: Vec<u8>,
        finality: OfflineDevicePolicyFinalityBindingV1,
    ) -> Result<Self, OfflineDevicePolicyViewErrorV1> {
        policy
            .validate_v2_rule_shape()
            .map_err(|_| OfflineDevicePolicyViewErrorV1::InvalidPolicy)?;
        let expected_freshness_deadline_ms =
            validated_android_status_freshness_deadline_v1(policy)?;
        if !finality.validate_v1()
            || freshness_deadline_ms != expected_freshness_deadline_ms
            || finality_evidence_bytes.is_empty()
            || finality_evidence_bytes.len() > OFFLINE_DEVICE_POLICY_FINALITY_EVIDENCE_MAX_BYTES_V1
            || Hash::new(&finality_evidence_bytes) != finality.finality_evidence_hash
        {
            return Err(OfflineDevicePolicyViewErrorV1::InvalidFinality);
        }
        let canonical_policy_bytes = policy
            .canonical_bytes_v2()
            .map_err(|_| OfflineDevicePolicyViewErrorV1::CanonicalEncoding)?;
        if canonical_policy_bytes.len() > OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_CANONICAL_BYTES_V1 {
            return Err(OfflineDevicePolicyViewErrorV1::InvalidPolicy);
        }
        let policy_hash: [u8; 32] = Hash::new(&canonical_policy_bytes).into();
        Ok(Self {
            version: OFFLINE_DEVICE_ATTESTATION_POLICY_VIEW_VERSION_V1,
            canonical_policy_bytes,
            policy_hash,
            policy_epoch: policy.policy_epoch,
            freshness_deadline_ms,
            finality_evidence_bytes,
            finality,
        })
    }

    /// Decode and validate the exact policy carried by this finalized view.
    ///
    /// # Errors
    ///
    /// Returns an error if the view is stale at `evaluation_time_ms`, its
    /// canonical bytes/hash/epoch disagree, its evidence bytes/hash disagree,
    /// or its finality binding is invalid.
    pub fn validated_policy_v1(
        &self,
        evaluation_time_ms: u64,
    ) -> Result<OfflineDeviceAttestationPolicy, OfflineDevicePolicyViewErrorV1> {
        let policy = self.validated_policy_binding_v1(evaluation_time_ms)?;
        let response_date_ms = policy
            .android_status_snapshot
            .as_ref()
            .ok_or(OfflineDevicePolicyViewErrorV1::InvalidPolicy)?
            .response_date_ms;
        if evaluation_time_ms < response_date_ms || evaluation_time_ms >= self.freshness_deadline_ms
        {
            return Err(OfflineDevicePolicyViewErrorV1::InvalidFinality);
        }
        Ok(policy)
    }

    /// Decode and authenticate the exact policy/finality binding without
    /// requiring the cached policy to still be fresh at `evaluation_time_ms`.
    ///
    /// This is the fail-closed validation path for typed drain-only policy
    /// synchronization. It still rejects evaluation before the finalized block,
    /// malformed snapshot metadata, a non-canonical policy, any hash/epoch or
    /// freshness-deadline mismatch, and invalid finality evidence binding.
    /// Eligible credential validation must continue to call
    /// [`Self::validated_policy_v1`].
    ///
    /// # Errors
    ///
    /// Returns an error when any structural, canonical policy, time-origin,
    /// evidence-hash, or finality binding invariant is invalid.
    pub fn validated_policy_binding_v1(
        &self,
        evaluation_time_ms: u64,
    ) -> Result<OfflineDeviceAttestationPolicy, OfflineDevicePolicyViewErrorV1> {
        if self.version != OFFLINE_DEVICE_ATTESTATION_POLICY_VIEW_VERSION_V1
            || !self.finality.validate_v1()
            || evaluation_time_ms < self.finality.finalized_block_timestamp_ms
            || self.finality_evidence_bytes.is_empty()
            || self.finality_evidence_bytes.len()
                > OFFLINE_DEVICE_POLICY_FINALITY_EVIDENCE_MAX_BYTES_V1
            || Hash::new(&self.finality_evidence_bytes) != self.finality.finality_evidence_hash
            || self.canonical_policy_bytes.len()
                > OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_CANONICAL_BYTES_V1
            || <[u8; 32]>::from(Hash::new(&self.canonical_policy_bytes)) != self.policy_hash
        {
            return Err(OfflineDevicePolicyViewErrorV1::InvalidFinality);
        }
        let policy: OfflineDeviceAttestationPolicy =
            norito::decode_canonical(&self.canonical_policy_bytes)
                .map_err(|_| OfflineDevicePolicyViewErrorV1::CanonicalEncoding)?;
        policy
            .validate_v2_rule_shape()
            .map_err(|_| OfflineDevicePolicyViewErrorV1::InvalidPolicy)?;
        let canonical = policy
            .canonical_bytes_v2()
            .map_err(|_| OfflineDevicePolicyViewErrorV1::CanonicalEncoding)?;
        let expected_freshness_deadline_ms =
            validated_android_status_freshness_deadline_v1(&policy)?;
        if canonical != self.canonical_policy_bytes
            || policy.policy_epoch != self.policy_epoch
            || expected_freshness_deadline_ms != self.freshness_deadline_ms
        {
            return Err(OfflineDevicePolicyViewErrorV1::InvalidPolicy);
        }
        Ok(policy)
    }
}

/// Validation error for finalized policy views.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum OfflineDevicePolicyViewErrorV1 {
    /// The carried policy does not satisfy V2 invariants.
    #[error("invalid Offline device-attestation policy")]
    InvalidPolicy,
    /// Canonical policy encoding or decoding failed.
    #[error("invalid canonical Offline device-attestation policy bytes")]
    CanonicalEncoding,
    /// Finality, time, hash, or freshness binding is invalid.
    #[error("invalid or stale Offline device-attestation policy finality binding")]
    InvalidFinality,
}

/// Issuer-signed claims for one short-lived device eligibility credential.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct OfflineDeviceEligibilityCredentialPayloadV1 {
    /// Credential layout marker.
    pub version: u16,
    /// Exact Iroha network identity.
    pub network_id: NetworkId,
    /// Account controlling the attested offline cash.
    pub account_id: AccountId,
    /// Platform device identifier.
    pub device_id: String,
    /// Issuer-scoped attestation key identifier.
    pub attestation_key_id: String,
    /// Exact registered device key.
    pub device_public_key: KagemushaDevicePublicKeyV2,
    /// Exact platform assertion public key bytes.
    pub assertion_public_key: Vec<u8>,
    /// Canonical registration hash admitted by consensus.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub registration_hash: [u8; 32],
    /// Only `Eligible` may be issued as a spend credential.
    pub eligibility: OfflineDeviceEligibilityOutcomeV1,
    /// Monotonic governed policy epoch.
    pub policy_epoch: u64,
    /// Canonical governed policy hash.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub policy_hash: [u8; 32],
    /// Exact finalized binding copied from the policy view.
    pub policy_finality: OfflineDevicePolicyFinalityBindingV1,
    /// Exclusive cached-policy freshness deadline.
    pub policy_freshness_deadline_ms: u64,
    /// Credential issue time in Unix milliseconds.
    pub issued_at_ms: u64,
    /// Exclusive credential expiry in Unix milliseconds.
    pub expires_at_ms: u64,
}

impl OfflineDeviceEligibilityCredentialPayloadV1 {
    fn validate_shape_v1(&self) -> Result<(), OfflineDeviceEligibilityCredentialErrorV1> {
        if self.version != OFFLINE_DEVICE_ELIGIBILITY_CREDENTIAL_VERSION_V1
            || self.device_id.is_empty()
            || self.device_id.len() > OFFLINE_DEVICE_ATTESTATION_DEVICE_ID_MAX_BYTES_V1
            || self.device_id.chars().any(char::is_control)
            || self.device_id.trim() != self.device_id
            || self.attestation_key_id.is_empty()
            || self.attestation_key_id.len() > OFFLINE_DEVICE_ATTESTATION_KEY_ID_MAX_BYTES_V1
            || self.attestation_key_id.chars().any(char::is_control)
            || self.attestation_key_id.trim() != self.attestation_key_id
            || self.device_public_key.validate().is_err()
            || KagemushaDevicePublicKeyV2::from_sec1_bytes(&self.assertion_public_key).is_err()
            || self.registration_hash == [0; 32]
            || self.policy_epoch == 0
            || self.policy_hash == [0; 32]
            || self.eligibility != OfflineDeviceEligibilityOutcomeV1::Eligible
            || !self.policy_finality.validate_v1()
            || self.network_id != self.policy_finality.network_id
            || self.issued_at_ms < self.policy_finality.finalized_block_timestamp_ms
            || self.expires_at_ms <= self.issued_at_ms
            || self.expires_at_ms > self.policy_freshness_deadline_ms
            || self.expires_at_ms.saturating_sub(self.issued_at_ms)
                > OFFLINE_DEVICE_ELIGIBILITY_CREDENTIAL_MAX_TTL_MS_V1
        {
            return Err(OfflineDeviceEligibilityCredentialErrorV1::InvalidClaims);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct OfflineDeviceEligibilityCredentialSigningPreimageV1 {
    domain: String,
    payload: OfflineDeviceEligibilityCredentialPayloadV1,
    issuer_public_key: PublicKey,
}

/// A short-lived issuer-signed device credential suitable for offline handoff.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct OfflineDeviceEligibilityCredentialV1 {
    /// Exact eligibility claims.
    pub payload: OfflineDeviceEligibilityCredentialPayloadV1,
    /// Governed credential issuer key.
    pub issuer_public_key: PublicKey,
    /// Issuer signature over the domain-separated canonical claims and key.
    pub issuer_signature: Signature,
}

impl OfflineDeviceEligibilityCredentialV1 {
    const SIGNING_DOMAIN_V1: &'static str =
        "iroha:kagemusha:offline-device-eligibility-credential:v1";

    fn signing_preimage_v1(
        payload: &OfflineDeviceEligibilityCredentialPayloadV1,
        issuer_public_key: &PublicKey,
    ) -> Result<Vec<u8>, OfflineDeviceEligibilityCredentialErrorV1> {
        norito::encode_canonical(&OfflineDeviceEligibilityCredentialSigningPreimageV1 {
            domain: Self::SIGNING_DOMAIN_V1.to_owned(),
            payload: payload.clone(),
            issuer_public_key: issuer_public_key.clone(),
        })
        .map_err(|_| OfflineDeviceEligibilityCredentialErrorV1::CanonicalEncoding)
    }

    /// Sign one already-finalized eligible-device payload.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid claims, canonical encoding failure, signing
    /// failure, or an issuer public key that does not verify the new signature.
    pub fn sign_v1(
        payload: OfflineDeviceEligibilityCredentialPayloadV1,
        issuer_public_key: PublicKey,
        issuer_private_key: &PrivateKey,
    ) -> Result<Self, OfflineDeviceEligibilityCredentialErrorV1> {
        payload.validate_shape_v1()?;
        let preimage = Self::signing_preimage_v1(&payload, &issuer_public_key)?;
        let issuer_signature = Signature::try_new(issuer_private_key, &preimage)
            .map_err(|_| OfflineDeviceEligibilityCredentialErrorV1::Signing)?;
        let credential = Self {
            payload,
            issuer_public_key,
            issuer_signature,
        };
        credential.verify_signature_v1()?;
        Ok(credential)
    }

    fn verify_signature_v1(&self) -> Result<(), OfflineDeviceEligibilityCredentialErrorV1> {
        let preimage = Self::signing_preimage_v1(&self.payload, &self.issuer_public_key)?;
        self.issuer_signature
            .verify(&self.issuer_public_key, &preimage)
            .map_err(|_| OfflineDeviceEligibilityCredentialErrorV1::InvalidSignature)
    }

    /// Verify issuer, policy, finality, network, TTL, and wall-clock validity.
    ///
    /// # Errors
    ///
    /// Returns an error if any credential claim differs from the cached
    /// finalized policy view, the credential is not currently live, or the
    /// issuer signature is invalid.
    pub fn verify_against_policy_view_v1(
        &self,
        expected_issuer: &PublicKey,
        policy_view: &OfflineDeviceAttestationPolicyViewV1,
        evaluation_time_ms: u64,
    ) -> Result<(), OfflineDeviceEligibilityCredentialErrorV1> {
        self.payload.validate_shape_v1()?;
        policy_view
            .validated_policy_v1(evaluation_time_ms)
            .map_err(|_| OfflineDeviceEligibilityCredentialErrorV1::PolicyBinding)?;
        if &self.issuer_public_key != expected_issuer
            || self.payload.network_id != policy_view.finality.network_id
            || self.payload.policy_epoch != policy_view.policy_epoch
            || self.payload.policy_hash != policy_view.policy_hash
            || self.payload.policy_finality != policy_view.finality
            || self.payload.policy_freshness_deadline_ms != policy_view.freshness_deadline_ms
            || evaluation_time_ms < self.payload.issued_at_ms
            || evaluation_time_ms >= self.payload.expires_at_ms
        {
            return Err(OfflineDeviceEligibilityCredentialErrorV1::PolicyBinding);
        }
        self.verify_signature_v1()
    }
}

/// Validation error for one Offline device eligibility credential.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum OfflineDeviceEligibilityCredentialErrorV1 {
    /// One or more bounded or temporal claims are invalid.
    #[error("invalid Offline device eligibility credential claims")]
    InvalidClaims,
    /// Canonical signing-preimage encoding failed.
    #[error("failed to encode Offline device eligibility credential")]
    CanonicalEncoding,
    /// The supplied private key could not create a signature.
    #[error("failed to sign Offline device eligibility credential")]
    Signing,
    /// The issuer signature is invalid.
    #[error("invalid Offline device eligibility credential signature")]
    InvalidSignature,
    /// The credential does not bind the expected finalized policy view.
    #[error("Offline device eligibility credential policy binding mismatch")]
    PolicyBinding,
}

/// Trusted platform root certificate for Offline device attestation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct OfflineDeviceAttestationTrustedRoot {
    /// Platform class, for example `ios-appattest` or `android-keymint`.
    pub platform: String,
    /// Root certificate DER bytes.
    pub der: Vec<u8>,
    /// Optional governance activation time in Unix milliseconds.
    pub not_before_ms: Option<u64>,
    /// Optional governance expiry time in Unix milliseconds.
    pub not_after_ms: Option<u64>,
}

/// Allowed iOS App Attest app identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct OfflineIosAppAttestationPolicy {
    /// Apple App ID prefix (normally the Apple Developer Team ID).
    pub team_id: String,
    /// iOS bundle identifier.
    pub bundle_id: String,
    /// App Attest environment, either `production` or `development`.
    pub environment: String,
    /// Allowed Apple validation categories from extension-bearing App Attest data.
    pub allowed_validation_categories: Vec<u32>,
    /// Allowed application bundle versions from extension-bearing App Attest data.
    pub allowed_bundle_versions: Vec<String>,
}

/// Allowed Android `KeyMint` app identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct OfflineAndroidAppAttestationPolicy {
    /// Android package name.
    pub package_name: String,
    /// Allowed Android signing certificate SHA-256 digests.
    pub signing_certificate_sha256: Vec<Vec<u8>>,
}

#[cfg(test)]
mod device_attestation_policy_tests {
    use super::*;
    use norito::codec::DecodeAll as _;
    use p256::elliptic_curve::sec1::ToEncodedPoint as _;

    fn snapshot() -> OfflineAndroidAttestationStatusSnapshotV1 {
        OfflineAndroidAttestationStatusSnapshotV1 {
            version: OFFLINE_ANDROID_ATTESTATION_STATUS_SNAPSHOT_VERSION_V1,
            payload_sha256: [0x5a; 32],
            response_date_ms: 1_800_000_000_000,
            last_modified_ms: Some(1_799_999_000_000),
            cache_max_age_seconds: 3_600,
            non_valid_serials: vec!["1ab".to_owned(), "fe10".to_owned()],
        }
    }

    fn vulnerability_rule() -> OfflineAndroidDeviceVulnerabilityRuleV2 {
        OfflineAndroidDeviceVulnerabilityRuleV2 {
            rule_id: "samsung-test-floor".to_owned(),
            manufacturer: "samsung".to_owned(),
            model: Some("sm-test".to_owned()),
            model_prefix: None,
            brand: Some("samsung".to_owned()),
            device: None,
            product: None,
            verified_boot_key_sha256: None,
            verified_boot_hash: None,
            affected_os_version_min: None,
            affected_os_version_max: None,
            minimum_safe_keymint_version: Some(4),
            minimum_safe_os_version: None,
            minimum_safe_os_patch_level: None,
            minimum_safe_vendor_patch_level: Some(20_260_701),
            minimum_safe_boot_patch_level: None,
            permanently_blocked: false,
            source_ids: vec!["https://security.example.test/advisory".to_owned()],
            cve_ids: vec!["CVE-2026-21046".to_owned()],
        }
    }

    fn policy() -> OfflineDeviceAttestationPolicy {
        OfflineDeviceAttestationPolicy {
            version: OFFLINE_DEVICE_ATTESTATION_POLICY_VERSION_V2,
            policy_epoch: 7,
            trusted_roots: Vec::new(),
            revoked_certificate_tbs_sha256: Vec::new(),
            ios_apps: Vec::new(),
            android_apps: Vec::new(),
            android_status_snapshot: None,
            android_vulnerability_rules: vec![vulnerability_rule()],
            require_ios_app_policy: false,
            require_android_app_policy: false,
        }
    }

    fn properties(
        security_level: OfflineAndroidDeviceSecurityLevelV2,
        os_version: u32,
        vendor_patch_level: u32,
    ) -> OfflineAndroidAttestedDevicePropertiesV2 {
        OfflineAndroidAttestedDevicePropertiesV2 {
            version: OFFLINE_ANDROID_ATTESTED_DEVICE_PROPERTIES_VERSION_V2,
            attestation_version: 4,
            keymint_version: 4,
            security_level,
            brand: "Samsung".to_owned(),
            device: "test-device".to_owned(),
            product: "test-product".to_owned(),
            manufacturer: "Samsung".to_owned(),
            model: "SM-TEST".to_owned(),
            os_version,
            os_patch_level: 202_607,
            vendor_patch_level,
            boot_patch_level: 20_260_701,
            verified_boot_key: vec![0x42; 32],
            verified_boot_hash: [0x51; 32],
        }
    }

    #[test]
    fn android_status_snapshot_norito_roundtrip() {
        let expected = snapshot();
        let encoded = expected.encode();
        let decoded =
            OfflineAndroidAttestationStatusSnapshotV1::decode_all(&mut encoded.as_slice())
                .expect("decode Android attestation status snapshot");
        assert_eq!(decoded, expected);
    }

    #[test]
    fn typed_eligibility_rejection_has_canonical_roundtrip_and_closed_shape() {
        let rejection = OfflineDeviceEligibilityRejectionV1::new_v1(
            OfflineDeviceEligibilityDecisionV1 {
                outcome: OfflineDeviceEligibilityOutcomeV1::DrainOnly,
                reason: OfflineDeviceEligibilityReasonV1::VulnerableFirmware,
                matched_rule_ids: vec![
                    "samsung-2021-cve-2021-25490-sm-g770".to_owned(),
                    "samsung-2026-cve-2026-21046".to_owned(),
                ],
            },
            "reviewed firmware floor is not satisfied",
        )
        .expect("valid typed rejection");
        let encoded = norito::encode_canonical(&rejection).expect("encode rejection");
        let decoded: OfflineDeviceEligibilityRejectionV1 =
            norito::decode_canonical(&encoded).expect("decode rejection");
        assert_eq!(decoded, rejection);
        decoded.validate_v1().expect("decoded rejection shape");

        assert!(matches!(
            OfflineDeviceEligibilityRejectionV1::new_v1(
                OfflineDeviceEligibilityDecisionV1 {
                    outcome: OfflineDeviceEligibilityOutcomeV1::Eligible,
                    reason: OfflineDeviceEligibilityReasonV1::PolicySatisfied,
                    matched_rule_ids: Vec::new(),
                },
                "eligible cannot be a rejection",
            ),
            Err(OfflineDeviceEligibilityDecisionV1Error::EligibleRejection)
        ));
        assert!(matches!(
            OfflineDeviceEligibilityRejectionV1::new_v1(
                OfflineDeviceEligibilityDecisionV1 {
                    outcome: OfflineDeviceEligibilityOutcomeV1::DrainOnly,
                    reason: OfflineDeviceEligibilityReasonV1::VulnerableFirmware,
                    matched_rule_ids: vec!["rule-b".to_owned(), "rule-a".to_owned()],
                },
                "rules must be sorted",
            ),
            Err(OfflineDeviceEligibilityDecisionV1Error::InvalidDecision)
        ));
        assert!(matches!(
            OfflineDeviceEligibilityRejectionV1::new_v1(
                OfflineDeviceAttestationPolicy::cryptographic_rejection_v1(),
                " rejected ",
            ),
            Err(OfflineDeviceEligibilityDecisionV1Error::InvalidDetail)
        ));
        assert!(matches!(
            OfflineDeviceEligibilityRejectionV1::new_v1(
                OfflineDeviceAttestationPolicy::cryptographic_rejection_v1(),
                "x".repeat(OFFLINE_DEVICE_ELIGIBILITY_REJECTION_DETAIL_MAX_BYTES_V1 + 1),
            ),
            Err(OfflineDeviceEligibilityDecisionV1Error::InvalidDetail)
        ));
    }

    #[cfg(feature = "json")]
    #[test]
    fn android_status_snapshot_json_shape_and_roundtrip() {
        let expected = snapshot();
        let json = norito::json::to_json(&expected)
            .expect("serialize Android attestation status snapshot JSON");
        assert!(json.contains("\"payload_sha256\":[90,90,90"));
        assert!(json.contains("\"last_modified_ms\":1799999000000"));
        assert!(json.contains("\"non_valid_serials\":[\"1ab\",\"fe10\"]"));
        let decoded: OfflineAndroidAttestationStatusSnapshotV1 =
            norito::json::from_str(&json).expect("decode Android attestation status snapshot JSON");
        assert_eq!(decoded, expected);
    }

    #[test]
    fn policy_v2_roundtrip_hash_and_rule_order_are_exact() {
        let expected = policy();
        expected
            .validate_v2_rule_shape()
            .expect("canonical V2 rules");
        let encoded = expected.canonical_bytes_v2().expect("encode V2 policy");
        let decoded: OfflineDeviceAttestationPolicy =
            norito::decode_canonical(&encoded).expect("decode canonical framed V2 policy");
        assert_eq!(decoded, expected);
        assert_eq!(
            expected.canonical_hash_v2().expect("hash V2 policy"),
            <[u8; 32]>::from(Hash::new(&encoded))
        );

        // The derived codec is the intentionally headerless pairing used by
        // internal callers.  Keep that boundary distinct from the framed,
        // checksummed canonical archive above so a field-codec regression
        // cannot be mistaken for a framing mismatch.
        let bare = expected.encode();
        let decoded_bare = OfflineDeviceAttestationPolicy::decode_all(&mut bare.as_slice())
            .expect("decode bare derived V2 policy");
        assert_eq!(decoded_bare, expected);
        assert_ne!(encoded, bare);
        assert!(
            OfflineDeviceAttestationPolicy::decode_all(&mut encoded.as_slice()).is_err(),
            "DecodeAll must not be used as the decoder for a canonical framed archive"
        );
        assert!(
            norito::decode_canonical::<OfflineDeviceAttestationPolicy>(&bare).is_err(),
            "the canonical decoder must reject a bare derived payload"
        );

        let mut duplicate = expected.clone();
        duplicate
            .android_vulnerability_rules
            .push(vulnerability_rule());
        assert!(duplicate.validate_v2_rule_shape().is_err());
        let mut noncanonical_cve = expected;
        noncanonical_cve.android_vulnerability_rules[0].cve_ids = vec!["cve-2026-21046".to_owned()];
        assert!(noncanonical_cve.validate_v2_rule_shape().is_err());

        let mut unbounded_range = policy();
        unbounded_range.android_vulnerability_rules[0].affected_os_version_min = Some(90_000);
        assert!(unbounded_range.validate_v2_rule_shape().is_err());

        let mut prefix_rule = policy();
        prefix_rule.android_vulnerability_rules[0].model = None;
        prefix_rule.android_vulnerability_rules[0].model_prefix = Some("sm-test".to_owned());
        prefix_rule
            .validate_v2_rule_shape()
            .expect("bounded lowercase model prefix is canonical");
        let mut exact_and_prefix = prefix_rule.clone();
        exact_and_prefix.android_vulnerability_rules[0].model = Some("sm-test".to_owned());
        assert!(exact_and_prefix.validate_v2_rule_shape().is_err());
        let mut uppercase_prefix = prefix_rule.clone();
        uppercase_prefix.android_vulnerability_rules[0].model_prefix = Some("SM-TEST".to_owned());
        assert!(uppercase_prefix.validate_v2_rule_shape().is_err());
        let mut broad_prefix = prefix_rule;
        broad_prefix.android_vulnerability_rules[0].model_prefix = Some("sm-g9".to_owned());
        assert!(broad_prefix.validate_v2_rule_shape().is_err());
    }

    #[test]
    fn android_patch_month_and_date_fields_are_not_interchangeable() {
        assert!(canonical_android_patch_month_v2(202_607));
        assert!(!canonical_android_patch_month_v2(20_260_701));
        assert!(canonical_android_patch_date_v2(20_240_229));
        assert!(!canonical_android_patch_date_v2(20_230_229));
        assert!(!canonical_android_patch_date_v2(202_607));

        let complete = properties(
            OfflineAndroidDeviceSecurityLevelV2::TrustedEnvironment,
            OFFLINE_ANDROID_12_OS_VERSION_FLOOR_V2,
            20_260_701,
        );
        assert!(complete.is_complete_v2());
        let mut malformed = complete.clone();
        malformed.os_patch_level = 202_613;
        assert!(!malformed.is_complete_v2());
        malformed = complete.clone();
        malformed.vendor_patch_level = 202_607;
        assert!(!malformed.is_complete_v2());
        malformed = complete;
        malformed.boot_patch_level = 20_260_231;
        assert!(!malformed.is_complete_v2());

        let mut malformed_floor = policy();
        malformed_floor.android_vulnerability_rules[0].minimum_safe_vendor_patch_level =
            Some(202_607);
        assert!(malformed_floor.validate_v2_rule_shape().is_err());
        let mut malformed_floor = policy();
        malformed_floor.android_vulnerability_rules[0].minimum_safe_os_patch_level = Some(202_613);
        assert!(malformed_floor.validate_v2_rule_shape().is_err());
    }

    #[test]
    fn reviewed_samsung_seed_is_canonical_and_preserves_2026_rule() {
        let mut policy = policy();
        policy.android_vulnerability_rules = reviewed_samsung_android_vulnerability_rules_v2();
        policy
            .validate_v2_rule_shape()
            .expect("reviewed Samsung rules are canonical");
        assert_eq!(policy.android_vulnerability_rules.len(), 28);
        assert!(
            policy
                .android_vulnerability_rules
                .windows(2)
                .all(|window| window[0].rule_id < window[1].rule_id)
        );
        let rule = policy
            .android_vulnerability_rules
            .iter()
            .find(|rule| rule.cve_ids.len() == 1 && rule.cve_ids[0].as_str() == "CVE-2026-21046")
            .expect("reviewed 2026 Samsung rule");
        assert_eq!(
            rule.rule_id,
            "samsung-cve-2026-21046-fabric-keymaster-toctou"
        );
        assert_eq!(rule.manufacturer, "samsung");
        assert_eq!(rule.model, None);
        assert_eq!(rule.model_prefix, None);
        assert_eq!(rule.affected_os_version_min, Some(140_000));
        assert_eq!(rule.affected_os_version_max, Some(169_999));
        assert_eq!(rule.minimum_safe_keymint_version, None);
        assert_eq!(rule.minimum_safe_os_version, None);
        assert_eq!(rule.minimum_safe_os_patch_level, Some(202_607));
        assert_eq!(rule.minimum_safe_vendor_patch_level, None);
        assert_eq!(rule.minimum_safe_boot_patch_level, None);
        assert!(!rule.permanently_blocked);
        assert_eq!(
            rule.source_ids,
            vec![
                OFFLINE_SAMSUNG_SMR_JULY_2026_SOURCE_V2.to_owned(),
                OFFLINE_SAMSUNG_FIRMWARE_UPDATES_2026_ARCHIVE_SOURCE_V2.to_owned(),
            ]
        );
        assert_eq!(rule.cve_ids, vec!["CVE-2026-21046"]);

        let mut vulnerable_android_14 = properties(
            OfflineAndroidDeviceSecurityLevelV2::TrustedEnvironment,
            140_000,
            20_260_101,
        );
        vulnerable_android_14.os_patch_level = 202_606;
        assert_eq!(
            policy
                .evaluate_verified_android_device_v2(Some(&vulnerable_android_14), true)
                .matched_rule_ids,
            vec!["samsung-cve-2026-21046-fabric-keymaster-toctou"]
        );
        vulnerable_android_14.model = "Unknown-1".to_owned();
        assert_eq!(
            policy
                .evaluate_verified_android_device_v2(Some(&vulnerable_android_14), true)
                .matched_rule_ids,
            vec!["samsung-cve-2026-21046-fabric-keymaster-toctou"],
            "the unqualified Samsung Android 14/15/16 scope does not require an invented model map",
        );

        let mut non_samsung = vulnerable_android_14.clone();
        non_samsung.manufacturer = "Other".to_owned();
        non_samsung.brand = "Other".to_owned();
        assert_eq!(
            policy
                .evaluate_verified_android_device_v2(Some(&non_samsung), true)
                .outcome,
            OfflineDeviceEligibilityOutcomeV1::Eligible,
            "a vendor-scoped rule must not become a generic patch-age cutoff",
        );

        let mut unknown_model_out_of_range = vulnerable_android_14.clone();
        unknown_model_out_of_range.model = "Unknown-1".to_owned();
        unknown_model_out_of_range.os_version = 130_000;
        assert_eq!(
            policy
                .evaluate_verified_android_device_v2(Some(&unknown_model_out_of_range), true)
                .outcome,
            OfflineDeviceEligibilityOutcomeV1::Eligible,
            "a complete unknown model outside the published Android range remains eligible",
        );

        vulnerable_android_14.os_patch_level = 202_607;
        assert_eq!(
            policy
                .evaluate_verified_android_device_v2(Some(&vulnerable_android_14), true)
                .outcome,
            OfflineDeviceEligibilityOutcomeV1::Eligible,
            "the published SMR month is an osPatchLevel floor, not a synthesized vendorPatchLevel day",
        );
    }

    fn reviewed_samsung_policy() -> OfflineDeviceAttestationPolicy {
        let rules = reviewed_samsung_android_vulnerability_rules_v2();
        assert_eq!(rules.len(), 28, "the reviewed seed has a closed rule set");
        let mut policy = policy();
        policy.android_vulnerability_rules = rules;
        policy
            .validate_v2_rule_shape()
            .expect("reviewed Samsung rules are canonical");
        policy
    }

    fn samsung_decision(
        policy: &OfflineDeviceAttestationPolicy,
        model: &str,
        os_version: u32,
        os_patch_level: u32,
        security_level: OfflineAndroidDeviceSecurityLevelV2,
    ) -> OfflineDeviceEligibilityDecisionV1 {
        let mut candidate = properties(security_level, os_version, 20_211_001);
        candidate.model = model.to_owned();
        candidate.os_patch_level = os_patch_level;
        policy.evaluate_verified_android_device_v2(Some(&candidate), true)
    }

    fn assert_reviewed_samsung_iv_reuse_rules(policy: &OfflineDeviceAttestationPolicy) {
        for (model_prefix, model_source, update_history_source) in [
            ("sm-a605", OFFLINE_SAMSUNG_MODEL_CODE_MAP_SOURCE_V2, None),
            ("sm-a920", OFFLINE_SAMSUNG_MODEL_CODE_MAP_SOURCE_V2, None),
            ("sm-g960", OFFLINE_SAMSUNG_MODEL_CODE_MAP_SOURCE_V2, None),
            ("sm-g965", OFFLINE_SAMSUNG_MODEL_CODE_MAP_SOURCE_V2, None),
            (
                "sm-j337",
                OFFLINE_SAMSUNG_J3_2018_MODEL_SOURCE_V2,
                Some(OFFLINE_SAMSUNG_J3_TOP_UPDATE_HISTORY_SOURCE_V2),
            ),
            ("sm-j720", OFFLINE_SAMSUNG_J7_DUO_MODEL_SOURCE_V2, None),
            ("sm-j737", OFFLINE_SAMSUNG_J7_2018_MODEL_SOURCE_V2, None),
            (
                "sm-n960",
                OFFLINE_SAMSUNG_MODEL_CODE_MAP_SOURCE_V2,
                Some(OFFLINE_SAMSUNG_NOTE9_UPDATE_HISTORY_SOURCE_V2),
            ),
            ("sm-t387", OFFLINE_SAMSUNG_TAB_A_T387_MODEL_SOURCE_V2, None),
            ("sm-t830", OFFLINE_SAMSUNG_MODEL_CODE_MAP_SOURCE_V2, None),
            ("sm-t835", OFFLINE_SAMSUNG_MODEL_CODE_MAP_SOURCE_V2, None),
            ("sm-t837", OFFLINE_SAMSUNG_TAB_S4_T837_MODEL_SOURCE_V2, None),
        ] {
            let attested_model = format!("{}F", model_prefix.to_ascii_uppercase());
            let expected_rule = format!("samsung-cve-2021-25444-keymaster-iv-reuse-{model_prefix}");
            let rule = policy
                .android_vulnerability_rules
                .iter()
                .find(|rule| rule.rule_id == expected_rule)
                .expect("reviewed August 2021 patched-device rule");
            assert_eq!(rule.model, None);
            assert_eq!(rule.model_prefix.as_deref(), Some(model_prefix));
            assert_eq!(rule.affected_os_version_min, Some(80_000));
            assert_eq!(rule.affected_os_version_max, Some(109_999));
            assert_eq!(rule.minimum_safe_os_patch_level, Some(202_108));
            assert_eq!(rule.cve_ids, vec!["CVE-2021-25444"]);
            assert!(rule.source_ids.iter().any(|source| source == model_source));
            if let Some(update_history_source) = update_history_source {
                assert!(
                    rule.source_ids
                        .iter()
                        .any(|source| source == update_history_source),
                    "{expected_rule} must retain its exact Samsung update-history evidence",
                );
            }
            let vulnerable = samsung_decision(
                policy,
                &attested_model,
                100_000,
                202_107,
                OfflineAndroidDeviceSecurityLevelV2::StrongBox,
            );
            assert_eq!(
                vulnerable.reason,
                OfflineDeviceEligibilityReasonV1::VulnerableFirmware
            );
            assert_eq!(vulnerable.matched_rule_ids, vec![expected_rule]);
            assert_eq!(
                samsung_decision(
                    policy,
                    &attested_model,
                    100_000,
                    202_108,
                    OfflineAndroidDeviceSecurityLevelV2::StrongBox,
                )
                .outcome,
                OfflineDeviceEligibilityOutcomeV1::Eligible,
                "the August 2021 SMR month is the disclosed device-family safe floor",
            );
        }

        assert_eq!(
            samsung_decision(
                policy,
                "SM-J337W",
                80_000,
                202_107,
                OfflineAndroidDeviceSecurityLevelV2::StrongBox,
            )
            .reason,
            OfflineDeviceEligibilityReasonV1::VulnerableFirmware,
            "the official J3 Top history requires retaining Android 8.0 in the conservative affected range",
        );
        assert_eq!(
            samsung_decision(
                policy,
                "SM-J337W",
                80_000,
                202_108,
                OfflineAndroidDeviceSecurityLevelV2::StrongBox,
            )
            .outcome,
            OfflineDeviceEligibilityOutcomeV1::Eligible,
            "the August 2021 SMR remains the J3 Top safe floor on Android 8.0",
        );
    }

    fn assert_reviewed_samsung_downgrade_rules(policy: &OfflineDeviceAttestationPolicy) {
        for (model_prefix, update_history_source) in [
            (
                "sm-g770",
                Some(OFFLINE_SAMSUNG_S10_LITE_UPDATE_HISTORY_SOURCE_V2),
            ),
            (
                "sm-g780",
                Some(OFFLINE_SAMSUNG_S20_FE_UPDATE_HISTORY_SOURCE_V2),
            ),
            (
                "sm-g781",
                Some(OFFLINE_SAMSUNG_S20_FE_5G_UPDATE_HISTORY_SOURCE_V2),
            ),
            ("sm-g970", None),
            ("sm-g973", None),
            ("sm-g975", None),
            ("sm-g977", None),
            ("sm-g980", None),
            ("sm-g981", None),
            ("sm-g985", None),
            ("sm-g986", None),
            ("sm-g988", None),
            ("sm-g991", None),
            ("sm-g996", None),
            ("sm-g998", None),
        ] {
            let attested_model = format!("{}B", model_prefix.to_ascii_uppercase());
            let expected_rule =
                format!("samsung-cve-2021-25490-keymaster-downgrade-{model_prefix}");
            let rule = policy
                .android_vulnerability_rules
                .iter()
                .find(|rule| rule.rule_id == expected_rule)
                .expect("reviewed S10/S20/S21 rule");
            assert_eq!(rule.model, None);
            assert_eq!(rule.model_prefix.as_deref(), Some(model_prefix));
            assert_eq!(rule.affected_os_version_min, Some(90_000));
            assert_eq!(rule.affected_os_version_max, Some(119_999));
            assert_eq!(rule.minimum_safe_os_patch_level, Some(202_110));
            assert_eq!(rule.cve_ids, vec!["CVE-2021-25490"]);
            assert!(
                rule.source_ids
                    .iter()
                    .any(|source| source == OFFLINE_SAMSUNG_MODEL_CODE_MAP_SOURCE_V2)
            );
            if let Some(update_history_source) = update_history_source {
                assert!(
                    rule.source_ids
                        .iter()
                        .any(|source| source == update_history_source),
                    "{expected_rule} must retain its exact Samsung update-history evidence",
                );
            }
            let vulnerable = samsung_decision(
                policy,
                &attested_model,
                110_000,
                202_109,
                OfflineAndroidDeviceSecurityLevelV2::StrongBox,
            );
            assert_eq!(
                vulnerable.reason,
                OfflineDeviceEligibilityReasonV1::VulnerableFirmware
            );
            assert_eq!(vulnerable.matched_rule_ids, vec![expected_rule]);
            assert_eq!(
                samsung_decision(
                    policy,
                    &attested_model,
                    110_000,
                    202_110,
                    OfflineAndroidDeviceSecurityLevelV2::StrongBox,
                )
                .outcome,
                OfflineDeviceEligibilityOutcomeV1::Eligible,
                "the October 2021 SMR month is the S10/S20/S21 safe floor",
            );
        }
    }

    fn assert_reviewed_samsung_model_scope(policy: &OfflineDeviceAttestationPolicy) {
        assert_eq!(
            samsung_decision(
                policy,
                "SM-G950F",
                100_000,
                202_101,
                OfflineAndroidDeviceSecurityLevelV2::StrongBox,
            )
            .outcome,
            OfflineDeviceEligibilityOutcomeV1::Eligible,
            "Galaxy S8 is excluded by the reviewed model-code map",
        );
        assert_eq!(
            samsung_decision(
                policy,
                "Unknown-Complete-Model",
                120_000,
                202_101,
                OfflineAndroidDeviceSecurityLevelV2::TrustedEnvironment,
            )
            .outcome,
            OfflineDeviceEligibilityOutcomeV1::Eligible,
            "a complete unknown Android 12+ model remains eligible",
        );
        assert_eq!(
            samsung_decision(
                policy,
                "SM-G960F",
                110_000,
                202_101,
                OfflineAndroidDeviceSecurityLevelV2::StrongBox,
            )
            .outcome,
            OfflineDeviceEligibilityOutcomeV1::Eligible,
            "the S9 rule does not extend beyond Android O/P/Q",
        );
        assert_eq!(
            samsung_decision(
                policy,
                "SM-G998B",
                120_000,
                202_101,
                OfflineAndroidDeviceSecurityLevelV2::TrustedEnvironment,
            )
            .outcome,
            OfflineDeviceEligibilityOutcomeV1::Eligible,
            "the S10/S20/S21 rule does not extend beyond Android P/Q/R",
        );
    }

    fn assert_reviewed_samsung_rule_sources(policy: &OfflineDeviceAttestationPolicy) {
        for rule in &policy.android_vulnerability_rules {
            assert!(
                rule.source_ids
                    .windows(2)
                    .all(|window| window[0] < window[1])
            );
            if rule
                .cve_ids
                .iter()
                .any(|cve| cve == "CVE-2021-25444" || cve == "CVE-2021-25490")
            {
                assert!(rule.source_ids.iter().any(|source| {
                    [
                        OFFLINE_SAMSUNG_MODEL_CODE_MAP_SOURCE_V2,
                        OFFLINE_SAMSUNG_J3_2018_MODEL_SOURCE_V2,
                        OFFLINE_SAMSUNG_J7_2018_MODEL_SOURCE_V2,
                        OFFLINE_SAMSUNG_J7_DUO_MODEL_SOURCE_V2,
                        OFFLINE_SAMSUNG_TAB_S4_T837_MODEL_SOURCE_V2,
                        OFFLINE_SAMSUNG_TAB_A_T387_MODEL_SOURCE_V2,
                    ]
                    .contains(&source.as_str())
                }));
                assert!(
                    rule.source_ids
                        .iter()
                        .any(|source| source == OFFLINE_SAMSUNG_KEYMASTER_USENIX_2022_SOURCE_V2)
                );
            }
        }
        assert_eq!(
            OFFLINE_SAMSUNG_SMR_AUGUST_2021_SOURCE_V2,
            "https://security.samsungmobile.com/securityUpdate.smsb?month=8&year=2021"
        );
        assert_eq!(
            OFFLINE_SAMSUNG_SMR_OCTOBER_2021_SOURCE_V2,
            "https://security.samsungmobile.com/securityUpdate.smsb?month=10&year=2021"
        );
        assert_eq!(
            OFFLINE_SAMSUNG_FIRMWARE_UPDATES_2021_ARCHIVE_SOURCE_V2,
            "https://security.samsungmobile.com/securityUpdate.smsb?month=5&year=2021"
        );
        assert_eq!(
            OFFLINE_SAMSUNG_KEYMASTER_USENIX_2022_SOURCE_V2,
            "https://www.usenix.org/conference/usenixsecurity22/presentation/shakevsky"
        );
        assert_eq!(
            OFFLINE_SAMSUNG_MODEL_CODE_MAP_SOURCE_V2,
            "https://images.samsung.com/is/content/samsung/assets/ch/support/Samsung_SEAS_PowerUp_Modelle.pdf"
        );
        assert_eq!(
            OFFLINE_SAMSUNG_J3_2018_MODEL_SOURCE_V2,
            "https://www.samsung.com/us/business/support/owners/product/galaxy-j3-2018-unlocked/"
        );
        assert_eq!(
            OFFLINE_SAMSUNG_J7_2018_MODEL_SOURCE_V2,
            "https://www.samsung.com/us/business/support/owners/product/galaxy-j7-2018-unlocked/"
        );
        assert_eq!(
            OFFLINE_SAMSUNG_J7_DUO_MODEL_SOURCE_V2,
            "https://www.samsung.com/n_africa/support/model/SM-J720FZVDMWD/"
        );
        assert_eq!(
            OFFLINE_SAMSUNG_TAB_S4_T837_MODEL_SOURCE_V2,
            "https://www.samsung.com/us/business/support/owners/product/galaxy-tab-s4-10-5-with-s-pen-att/"
        );
        assert_eq!(
            OFFLINE_SAMSUNG_TAB_A_T387_MODEL_SOURCE_V2,
            "https://image-us.samsung.com/SamsungUS/samsungbusiness/products/mobile/tablets/galaxy-tab-a/sm-t387-2019/pdf/TAB-SPR-GALAXYTABADSHT-NOV18T-Final-11-26-18.pdf"
        );
        assert_eq!(
            OFFLINE_SAMSUNG_J3_TOP_UPDATE_HISTORY_SOURCE_V2,
            "https://doc.samsungmobile.com/SM-J337W/002040181023/eng.html"
        );
        assert_eq!(
            OFFLINE_SAMSUNG_NOTE9_UPDATE_HISTORY_SOURCE_V2,
            "https://doc.samsungmobile.com/SM-N960F/001244180820/eng.html"
        );
        assert_eq!(
            OFFLINE_SAMSUNG_S10_LITE_UPDATE_HISTORY_SOURCE_V2,
            "https://doc.samsungmobile.com/SM-G770F/011910200316/eng.html"
        );
        assert_eq!(
            OFFLINE_SAMSUNG_S20_FE_UPDATE_HISTORY_SOURCE_V2,
            "https://doc.samsungmobile.com/SM-G780F/014801201001/eng.html"
        );
        assert_eq!(
            OFFLINE_SAMSUNG_S20_FE_5G_UPDATE_HISTORY_SOURCE_V2,
            "https://doc.samsungmobile.com/SM-G781B/014790201001/eng.html"
        );
    }

    #[test]
    fn reviewed_samsung_seed_enforces_exact_2021_model_prefixes_and_smr_floors() {
        let policy = reviewed_samsung_policy();
        assert_reviewed_samsung_iv_reuse_rules(&policy);
        assert_reviewed_samsung_downgrade_rules(&policy);
        assert_reviewed_samsung_model_scope(&policy);
        assert_reviewed_samsung_rule_sources(&policy);
    }

    #[test]
    fn verified_boot_key_selector_uses_sha256() {
        let mut policy = policy();
        let properties = properties(
            OfflineAndroidDeviceSecurityLevelV2::StrongBox,
            OFFLINE_ANDROID_12_OS_VERSION_FLOOR_V2,
            20_260_701,
        );
        let sha256: [u8; 32] = Sha256::digest(&properties.verified_boot_key).into();
        assert_ne!(
            sha256,
            <[u8; 32]>::from(Hash::new(&properties.verified_boot_key))
        );
        let rule = &mut policy.android_vulnerability_rules[0];
        rule.verified_boot_key_sha256 = Some(sha256);
        rule.permanently_blocked = true;
        assert_eq!(
            policy
                .evaluate_verified_android_device_v2(Some(&properties), true)
                .reason,
            OfflineDeviceEligibilityReasonV1::PermanentlyBlockedDevice,
        );
    }

    #[cfg(feature = "json")]
    #[test]
    fn policy_v2_closed_enums_have_tagged_json_roundtrips() {
        let properties = properties(
            OfflineAndroidDeviceSecurityLevelV2::StrongBox,
            OFFLINE_ANDROID_12_OS_VERSION_FLOOR_V2,
            20_260_701,
        );
        let properties_json =
            norito::json::to_json(&properties).expect("serialize attested properties JSON");
        assert!(properties_json.contains("strong_box"));
        let decoded_properties: OfflineAndroidAttestedDevicePropertiesV2 =
            norito::json::from_str(&properties_json).expect("decode attested properties JSON");
        assert_eq!(decoded_properties, properties);

        let decision = OfflineDeviceEligibilityDecisionV1 {
            outcome: OfflineDeviceEligibilityOutcomeV1::DrainOnly,
            reason: OfflineDeviceEligibilityReasonV1::VulnerableFirmware,
            matched_rule_ids: vec!["samsung-test-floor".to_owned()],
        };
        let decision_json =
            norito::json::to_json(&decision).expect("serialize eligibility decision JSON");
        assert!(decision_json.contains("drain_only"));
        assert!(decision_json.contains("vulnerable_firmware"));
        let decoded_decision: OfflineDeviceEligibilityDecisionV1 =
            norito::json::from_str(&decision_json).expect("decode eligibility decision JSON");
        assert_eq!(decoded_decision, decision);
    }

    #[test]
    fn android_policy_allows_unknown_complete_models_and_drains_vulnerable_profiles() {
        let policy = policy();
        let vulnerable = properties(
            OfflineAndroidDeviceSecurityLevelV2::TrustedEnvironment,
            OFFLINE_ANDROID_12_OS_VERSION_FLOOR_V2,
            20_260_601,
        );
        let decision = policy.evaluate_verified_android_device_v2(Some(&vulnerable), true);
        assert_eq!(
            decision.outcome,
            OfflineDeviceEligibilityOutcomeV1::DrainOnly
        );
        assert_eq!(
            decision.reason,
            OfflineDeviceEligibilityReasonV1::VulnerableFirmware
        );
        assert_eq!(decision.matched_rule_ids, vec!["samsung-test-floor"]);

        let updated = properties(
            OfflineAndroidDeviceSecurityLevelV2::TrustedEnvironment,
            OFFLINE_ANDROID_12_OS_VERSION_FLOOR_V2,
            20_260_701,
        );
        assert_eq!(
            policy
                .evaluate_verified_android_device_v2(Some(&updated), true)
                .outcome,
            OfflineDeviceEligibilityOutcomeV1::Eligible
        );
        let mut unknown = updated.clone();
        unknown.model = "Unknown-1".to_owned();
        assert_eq!(
            policy
                .evaluate_verified_android_device_v2(Some(&unknown), true)
                .outcome,
            OfflineDeviceEligibilityOutcomeV1::Eligible
        );

        let pre_android_12_tee = properties(
            OfflineAndroidDeviceSecurityLevelV2::TrustedEnvironment,
            OFFLINE_ANDROID_12_OS_VERSION_FLOOR_V2 - 1,
            20_260_701,
        );
        assert_eq!(
            policy
                .evaluate_verified_android_device_v2(Some(&pre_android_12_tee), true)
                .reason,
            OfflineDeviceEligibilityReasonV1::UnsupportedPreAndroid12Tee
        );
        let mut pre_android_12_strongbox = pre_android_12_tee;
        pre_android_12_strongbox.security_level = OfflineAndroidDeviceSecurityLevelV2::StrongBox;
        assert_eq!(
            policy
                .evaluate_verified_android_device_v2(Some(&pre_android_12_strongbox), true)
                .outcome,
            OfflineDeviceEligibilityOutcomeV1::Eligible
        );
        assert_eq!(
            policy
                .evaluate_verified_android_device_v2(None, true)
                .reason,
            OfflineDeviceEligibilityReasonV1::IncompleteAttestedProperties
        );
        assert_eq!(
            policy
                .evaluate_verified_android_device_v2(Some(&updated), false)
                .reason,
            OfflineDeviceEligibilityReasonV1::PolicyNotFresh
        );
    }

    #[test]
    fn android_policy_distinguishes_permanent_blocks_and_cryptographic_rejection() {
        let mut policy = policy();
        let rule = &mut policy.android_vulnerability_rules[0];
        rule.permanently_blocked = true;
        rule.minimum_safe_keymint_version = None;
        rule.minimum_safe_vendor_patch_level = None;
        policy
            .validate_v2_rule_shape()
            .expect("permanent rule is canonical without a safe floor");

        let decision = policy.evaluate_verified_android_device_v2(
            Some(&properties(
                OfflineAndroidDeviceSecurityLevelV2::StrongBox,
                OFFLINE_ANDROID_12_OS_VERSION_FLOOR_V2,
                20_260_701,
            )),
            true,
        );
        assert_eq!(
            decision.outcome,
            OfflineDeviceEligibilityOutcomeV1::DrainOnly
        );
        assert_eq!(
            decision.reason,
            OfflineDeviceEligibilityReasonV1::PermanentlyBlockedDevice
        );

        assert_eq!(
            OfflineDeviceAttestationPolicy::cryptographic_rejection_v1(),
            OfflineDeviceEligibilityDecisionV1 {
                outcome: OfflineDeviceEligibilityOutcomeV1::CryptographicallyRejected,
                reason: OfflineDeviceEligibilityReasonV1::CryptographicAttestationRejected,
                matched_rule_ids: Vec::new(),
            }
        );
    }

    struct FinalizedPolicyFixture {
        policy: OfflineDeviceAttestationPolicy,
        network_id: NetworkId,
        finalized_at_ms: u64,
        freshness_deadline_ms: u64,
        finality: OfflineDevicePolicyFinalityBindingV1,
        view: OfflineDeviceAttestationPolicyViewV1,
    }

    fn finalized_policy_fixture() -> FinalizedPolicyFixture {
        let mut policy = policy();
        policy.android_status_snapshot = Some(snapshot());
        let network_id = kagemusha_test_network_id("eligibility-credential-network");
        let finalized_at_ms = 1_800_000_000_000;
        let freshness_deadline_ms = finalized_at_ms + 60 * 60 * 1_000;
        let finality_evidence_bytes = b"eligibility finality evidence".to_vec();
        let finality = OfflineDevicePolicyFinalityBindingV1 {
            version: OFFLINE_DEVICE_POLICY_FINALITY_BINDING_VERSION_V1,
            network_id,
            finalized_block_height: 44,
            finalized_block_hash: Hash::new(b"eligibility finalized block"),
            finalized_block_timestamp_ms: finalized_at_ms,
            finality_evidence_hash: Hash::new(&finality_evidence_bytes),
        };
        let view = OfflineDeviceAttestationPolicyViewV1::new_v1(
            &policy,
            freshness_deadline_ms,
            finality_evidence_bytes,
            finality,
        )
        .expect("construct finalized policy view");
        FinalizedPolicyFixture {
            policy,
            network_id,
            finalized_at_ms,
            freshness_deadline_ms,
            finality,
            view,
        }
    }

    struct EligibilityCredentialFixture {
        finalized: FinalizedPolicyFixture,
        issuer: KeyPair,
        payload: OfflineDeviceEligibilityCredentialPayloadV1,
        credential: OfflineDeviceEligibilityCredentialV1,
    }

    fn eligibility_credential_fixture() -> EligibilityCredentialFixture {
        let finalized = finalized_policy_fixture();
        let issuer = KeyPair::try_from_seed(vec![0x73; 32], Algorithm::Ed25519)
            .expect("derive credential issuer");
        let device_secret =
            p256::SecretKey::from_slice(&[0x25; 32]).expect("derive device fixture key");
        let assertion_public_key = device_secret
            .public_key()
            .to_encoded_point(false)
            .as_bytes()
            .to_vec();
        let device_public_key = KagemushaDevicePublicKeyV2::from_sec1_bytes(&assertion_public_key)
            .expect("canonical device fixture key");
        let issued_at_ms = finalized.finalized_at_ms + 1_000;
        let payload = OfflineDeviceEligibilityCredentialPayloadV1 {
            version: OFFLINE_DEVICE_ELIGIBILITY_CREDENTIAL_VERSION_V1,
            network_id: finalized.network_id,
            account_id: AccountId::new(issuer.public_key().clone()),
            device_id: "android-credential-device".to_owned(),
            attestation_key_id: "attestation-key-1".to_owned(),
            device_public_key,
            assertion_public_key,
            registration_hash: [0x81; 32],
            eligibility: OfflineDeviceEligibilityOutcomeV1::Eligible,
            policy_epoch: finalized.view.policy_epoch,
            policy_hash: finalized.view.policy_hash,
            policy_finality: finalized.finality,
            policy_freshness_deadline_ms: finalized.view.freshness_deadline_ms,
            issued_at_ms,
            expires_at_ms: issued_at_ms + 30 * 60 * 1_000,
        };
        let credential = OfflineDeviceEligibilityCredentialV1::sign_v1(
            payload.clone(),
            issuer.public_key().clone(),
            issuer.private_key(),
        )
        .expect("sign eligibility credential");
        EligibilityCredentialFixture {
            finalized,
            issuer,
            payload,
            credential,
        }
    }

    #[test]
    fn finalized_policy_view_binds_policy_network_and_freshness() {
        let fixture = finalized_policy_fixture();
        let policy = fixture.policy;
        let finalized_at_ms = fixture.finalized_at_ms;
        let freshness_deadline_ms = fixture.freshness_deadline_ms;
        let finality = fixture.finality;
        let view = fixture.view;
        assert_eq!(
            view.validated_policy_v1(finalized_at_ms)
                .expect("validate view"),
            policy
        );
        let encoded_view = norito::encode_canonical(&view).expect("encode finalized policy view");
        let decoded_view: OfflineDeviceAttestationPolicyViewV1 =
            norito::decode_canonical(&encoded_view).expect("decode finalized policy view");
        assert_eq!(decoded_view, view);
        assert_eq!(
            view.validated_policy_binding_v1(freshness_deadline_ms)
                .expect("stale view remains structurally and finality bound"),
            policy
        );
        assert_eq!(
            view.validated_policy_v1(freshness_deadline_ms),
            Err(OfflineDevicePolicyViewErrorV1::InvalidFinality),
            "fresh-only validation must still reject at the exclusive deadline"
        );
        let mut future_dated_policy = policy.clone();
        let future_response_date_ms = finalized_at_ms + 1_000;
        future_dated_policy
            .android_status_snapshot
            .as_mut()
            .expect("fixture status snapshot")
            .response_date_ms = future_response_date_ms;
        let future_dated_view = OfflineDeviceAttestationPolicyViewV1::new_v1(
            &future_dated_policy,
            future_response_date_ms + 60 * 60 * 1_000,
            view.finality_evidence_bytes.clone(),
            finality,
        )
        .expect("a future-dated snapshot remains representable for drain-only synchronization");
        future_dated_view
            .validated_policy_binding_v1(finalized_at_ms)
            .expect("the future-dated view remains structurally authenticated");
        assert_eq!(
            future_dated_view.validated_policy_v1(finalized_at_ms),
            Err(OfflineDevicePolicyViewErrorV1::InvalidFinality),
            "future-dated status metadata must never authorize an eligible credential",
        );
        let mut tampered_view = view.clone();
        tampered_view.finality_evidence_bytes[0] ^= 0x80;
        assert_eq!(
            tampered_view.validated_policy_v1(finalized_at_ms),
            Err(OfflineDevicePolicyViewErrorV1::InvalidFinality)
        );
        let mut tampered_hash = view.clone();
        tampered_hash.policy_hash[0] ^= 0x80;
        assert_eq!(
            tampered_hash.validated_policy_v1(finalized_at_ms),
            Err(OfflineDevicePolicyViewErrorV1::InvalidFinality)
        );
        let mut tampered_epoch = view.clone();
        tampered_epoch.policy_epoch += 1;
        assert_eq!(
            tampered_epoch.validated_policy_v1(finalized_at_ms),
            Err(OfflineDevicePolicyViewErrorV1::InvalidPolicy)
        );
        let mut extended_freshness = view.clone();
        extended_freshness.freshness_deadline_ms += 1_000;
        assert_eq!(
            extended_freshness.validated_policy_binding_v1(finalized_at_ms),
            Err(OfflineDevicePolicyViewErrorV1::InvalidPolicy),
            "the view cannot extend freshness beyond authenticated snapshot metadata"
        );
        let mut post_expiry_finality = finality;
        post_expiry_finality.finalized_block_timestamp_ms = freshness_deadline_ms + 1_000;
        let post_expiry_view = OfflineDeviceAttestationPolicyViewV1::new_v1(
            &policy,
            freshness_deadline_ms,
            view.finality_evidence_bytes.clone(),
            post_expiry_finality,
        )
        .expect("a finalized stale policy remains available for drain-only synchronization");
        post_expiry_view
            .validated_policy_binding_v1(post_expiry_finality.finalized_block_timestamp_ms)
            .expect("post-expiry finality binding remains structurally valid");
        assert_eq!(
            post_expiry_view.validated_policy_v1(post_expiry_finality.finalized_block_timestamp_ms),
            Err(OfflineDevicePolicyViewErrorV1::InvalidFinality)
        );
    }

    #[test]
    fn eligibility_credential_binds_policy_network_keys_and_roundtrips() {
        let fixture = eligibility_credential_fixture();
        let issuer = &fixture.issuer;
        let view = &fixture.finalized.view;
        let issued_at_ms = fixture.payload.issued_at_ms;
        let credential = fixture.credential;
        credential
            .verify_against_policy_view_v1(issuer.public_key(), view, issued_at_ms)
            .expect("verify eligibility credential");

        let credential_bytes =
            norito::encode_canonical(&credential).expect("encode eligibility credential");
        let decoded_credential: OfflineDeviceEligibilityCredentialV1 =
            norito::decode_canonical(&credential_bytes).expect("decode eligibility credential");
        assert_eq!(decoded_credential, credential);
        decoded_credential
            .verify_against_policy_view_v1(issuer.public_key(), view, issued_at_ms)
            .expect("round-tripped eligibility credential remains policy-bound");

        let mut policy_mutation = credential.clone();
        policy_mutation.payload.policy_epoch += 1;
        assert!(
            policy_mutation
                .verify_against_policy_view_v1(issuer.public_key(), view, issued_at_ms)
                .is_err(),
            "the issuer signature and finalized view jointly bind the policy epoch",
        );
        let mut finality_mutation = credential.clone();
        finality_mutation
            .payload
            .policy_finality
            .finalized_block_height += 1;
        assert!(
            finality_mutation
                .verify_against_policy_view_v1(issuer.public_key(), view, issued_at_ms)
                .is_err(),
            "the issuer signature and finalized view jointly bind finality",
        );
    }

    #[test]
    fn eligibility_credential_rejects_invalid_claims_and_overlong_ttl() {
        let fixture = eligibility_credential_fixture();
        let issuer = &fixture.issuer;
        let finalized_at_ms = fixture.finalized.finalized_at_ms;
        let payload = fixture.payload;
        let mut invalid_identifier = payload.clone();
        invalid_identifier.device_id = " android-credential-device".to_owned();
        assert!(matches!(
            OfflineDeviceEligibilityCredentialV1::sign_v1(
                invalid_identifier,
                issuer.public_key().clone(),
                issuer.private_key(),
            ),
            Err(OfflineDeviceEligibilityCredentialErrorV1::InvalidClaims)
        ));
        let mut invalid_identifier = payload.clone();
        invalid_identifier.attestation_key_id = "attestation-key-1\n".to_owned();
        assert!(matches!(
            OfflineDeviceEligibilityCredentialV1::sign_v1(
                invalid_identifier,
                issuer.public_key().clone(),
                issuer.private_key(),
            ),
            Err(OfflineDeviceEligibilityCredentialErrorV1::InvalidClaims)
        ));
        let mut invalid_assertion_key = payload.clone();
        invalid_assertion_key.assertion_public_key =
            vec![0; KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V2];
        invalid_assertion_key.assertion_public_key[0] = 0x04;
        assert!(matches!(
            OfflineDeviceEligibilityCredentialV1::sign_v1(
                invalid_assertion_key,
                issuer.public_key().clone(),
                issuer.private_key(),
            ),
            Err(OfflineDeviceEligibilityCredentialErrorV1::InvalidClaims)
        ));

        let mut maximum_ttl = payload.clone();
        maximum_ttl.issued_at_ms = finalized_at_ms;
        maximum_ttl.expires_at_ms =
            finalized_at_ms + OFFLINE_DEVICE_ELIGIBILITY_CREDENTIAL_MAX_TTL_MS_V1;
        maximum_ttl.policy_freshness_deadline_ms = maximum_ttl.expires_at_ms;
        OfflineDeviceEligibilityCredentialV1::sign_v1(
            maximum_ttl.clone(),
            issuer.public_key().clone(),
            issuer.private_key(),
        )
        .expect("the exact 24-hour eligibility lifetime is accepted");
        maximum_ttl.expires_at_ms += 1;
        maximum_ttl.policy_freshness_deadline_ms += 1;
        assert!(matches!(
            OfflineDeviceEligibilityCredentialV1::sign_v1(
                maximum_ttl,
                issuer.public_key().clone(),
                issuer.private_key(),
            ),
            Err(OfflineDeviceEligibilityCredentialErrorV1::InvalidClaims)
        ));

        let mut overlong = payload;
        overlong.expires_at_ms =
            overlong.issued_at_ms + OFFLINE_DEVICE_ELIGIBILITY_CREDENTIAL_MAX_TTL_MS_V1 + 1;
        assert!(
            OfflineDeviceEligibilityCredentialV1::sign_v1(
                overlong,
                issuer.public_key().clone(),
                issuer.private_key(),
            )
            .is_err()
        );
    }
}
