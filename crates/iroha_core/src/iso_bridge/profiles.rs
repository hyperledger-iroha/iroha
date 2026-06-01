//! Rail-specific ISO 20022 profile catalog for the bridge.
//!
//! The profile data is embedded as Norito JSON so all nodes start from the same
//! deterministic baseline without runtime network fetches. Operators can layer
//! configuration overrides in Torii, but the defaults here remain the source of
//! truth for generic ISO, CBPR+, Fedwire, SEPA SCT Inst, and securities CSD
//! validation policy.

use std::collections::BTreeMap;

use norito::json::{self, Value};

/// Default profile used when Torii configuration does not select a rail.
pub const DEFAULT_PROFILE_ID: &str = "generic-iso20022";

const DEFAULT_PROFILES_JSON: &str = r#"
[
  {
    "id": "generic-iso20022",
    "rail": "generic-iso20022",
    "embedded_signature_policy": "record-only",
    "required_reference_datasets": [],
    "message_profiles": [
      {
        "message_type": "pacs.008",
        "direction": "inbound",
        "versions": ["pacs.008", "pacs.008.001.08", "pacs.008.001.10"],
        "business_services": [],
        "require_app_header": false,
        "require_business_service": false,
        "require_uetr": false,
        "structured_address_mode": "permissive",
        "supplementary_data_max_bytes": 16384,
        "amount_minor_units": [
          {"currency": "BHD", "minor_units": 3},
          {"currency": "CLF", "minor_units": 4},
          {"currency": "EUR", "minor_units": 2},
          {"currency": "GBP", "minor_units": 2},
          {"currency": "JPY", "minor_units": 0},
          {"currency": "KWD", "minor_units": 3},
          {"currency": "USD", "minor_units": 2}
        ]
      },
      {
        "message_type": "pacs.009",
        "direction": "inbound",
        "versions": ["pacs.009", "pacs.009.001.08", "pacs.009.001.10"],
        "business_services": [],
        "require_app_header": false,
        "require_business_service": false,
        "require_uetr": false,
        "structured_address_mode": "permissive",
        "supplementary_data_max_bytes": 16384,
        "amount_minor_units": [
          {"currency": "BHD", "minor_units": 3},
          {"currency": "CLF", "minor_units": 4},
          {"currency": "EUR", "minor_units": 2},
          {"currency": "GBP", "minor_units": 2},
          {"currency": "JPY", "minor_units": 0},
          {"currency": "KWD", "minor_units": 3},
          {"currency": "USD", "minor_units": 2}
        ]
      },
      {
        "message_type": "pacs.002",
        "direction": "inbound",
        "versions": ["pacs.002", "pacs.002.001.10", "pacs.002.001.12"],
        "business_services": [],
        "require_app_header": false,
        "require_business_service": false,
        "require_uetr": false,
        "structured_address_mode": "permissive",
        "supplementary_data_max_bytes": 16384,
        "amount_minor_units": []
      },
      {
        "message_type": "pacs.004",
        "direction": "inbound",
        "versions": ["pacs.004", "pacs.004.001.09", "pacs.004.001.10"],
        "business_services": [],
        "require_app_header": false,
        "require_business_service": false,
        "require_uetr": false,
        "structured_address_mode": "permissive",
        "supplementary_data_max_bytes": 16384,
        "amount_minor_units": []
      },
      {
        "message_type": "camt.056",
        "direction": "inbound",
        "versions": ["camt.056", "camt.056.001.08", "camt.056.001.09"],
        "business_services": [],
        "require_app_header": false,
        "require_business_service": false,
        "require_uetr": false,
        "structured_address_mode": "permissive",
        "supplementary_data_max_bytes": 16384,
        "amount_minor_units": []
      },
      {
        "message_type": "sese.023",
        "direction": "inbound",
        "versions": ["sese.023", "sese.023.001.09", "sese.023.001.11"],
        "business_services": [],
        "require_app_header": false,
        "require_business_service": false,
        "require_uetr": false,
        "structured_address_mode": "permissive",
        "supplementary_data_max_bytes": 16384,
        "amount_minor_units": []
      },
      {
        "message_type": "sese.024",
        "direction": "inbound",
        "versions": ["sese.024", "sese.024.001.09", "sese.024.001.10"],
        "business_services": [],
        "require_app_header": false,
        "require_business_service": false,
        "require_uetr": false,
        "structured_address_mode": "permissive",
        "supplementary_data_max_bytes": 16384,
        "amount_minor_units": []
      },
      {
        "message_type": "sese.025",
        "direction": "inbound",
        "versions": ["sese.025", "sese.025.001.08", "sese.025.001.10"],
        "business_services": [],
        "require_app_header": false,
        "require_business_service": false,
        "require_uetr": false,
        "structured_address_mode": "permissive",
        "supplementary_data_max_bytes": 16384,
        "amount_minor_units": []
      }
    ]
  },
  {
    "id": "swift-cbpr-plus",
    "rail": "swift-cbpr-plus",
    "embedded_signature_policy": "reject-unsupported",
    "required_reference_datasets": ["bic-lei"],
    "message_profiles": [
      {
        "message_type": "pacs.008",
        "direction": "inbound",
        "versions": ["pacs.008.001.08", "pacs.008.001.10"],
        "business_services": ["swift.cbprplus.02", "swift.cbprplus.03"],
        "require_app_header": true,
        "require_business_service": true,
        "require_uetr": true,
        "structured_address_mode": "require-structured",
        "supplementary_data_max_bytes": 4096,
        "amount_minor_units": [
          {"currency": "BHD", "minor_units": 3},
          {"currency": "CLF", "minor_units": 4},
          {"currency": "EUR", "minor_units": 2},
          {"currency": "GBP", "minor_units": 2},
          {"currency": "JPY", "minor_units": 0},
          {"currency": "KWD", "minor_units": 3},
          {"currency": "USD", "minor_units": 2}
        ]
      },
      {
        "message_type": "pacs.009",
        "direction": "inbound",
        "versions": ["pacs.009.001.08", "pacs.009.001.10"],
        "business_services": ["swift.cbprplus.02", "swift.cbprplus.03"],
        "require_app_header": true,
        "require_business_service": true,
        "require_uetr": false,
        "structured_address_mode": "require-structured",
        "supplementary_data_max_bytes": 4096,
        "amount_minor_units": [
          {"currency": "BHD", "minor_units": 3},
          {"currency": "CLF", "minor_units": 4},
          {"currency": "EUR", "minor_units": 2},
          {"currency": "GBP", "minor_units": 2},
          {"currency": "JPY", "minor_units": 0},
          {"currency": "KWD", "minor_units": 3},
          {"currency": "USD", "minor_units": 2}
        ]
      },
      {
        "message_type": "pacs.002",
        "direction": "inbound",
        "versions": ["pacs.002.001.10", "pacs.002.001.12"],
        "business_services": ["swift.cbprplus.02", "swift.cbprplus.03"],
        "require_app_header": true,
        "require_business_service": true,
        "require_uetr": false,
        "structured_address_mode": "require-structured",
        "supplementary_data_max_bytes": 4096,
        "amount_minor_units": []
      },
      {
        "message_type": "pacs.004",
        "direction": "inbound",
        "versions": ["pacs.004.001.09", "pacs.004.001.10"],
        "business_services": ["swift.cbprplus.02", "swift.cbprplus.03"],
        "require_app_header": true,
        "require_business_service": true,
        "require_uetr": false,
        "structured_address_mode": "require-structured",
        "supplementary_data_max_bytes": 4096,
        "amount_minor_units": []
      },
      {
        "message_type": "camt.056",
        "direction": "inbound",
        "versions": ["camt.056.001.08", "camt.056.001.09"],
        "business_services": ["swift.cbprplus.02", "swift.cbprplus.03"],
        "require_app_header": true,
        "require_business_service": true,
        "require_uetr": false,
        "structured_address_mode": "require-structured",
        "supplementary_data_max_bytes": 4096,
        "amount_minor_units": []
      }
    ]
  },
  {
    "id": "fedwire-funds",
    "rail": "fedwire-funds",
    "embedded_signature_policy": "reject-unsupported",
    "required_reference_datasets": ["bic-lei"],
    "message_profiles": [
      {
        "message_type": "pacs.008",
        "direction": "inbound",
        "versions": ["pacs.008.001.08"],
        "business_services": ["fedwire.funds.01"],
        "require_app_header": true,
        "require_business_service": true,
        "require_uetr": true,
        "structured_address_mode": "require-structured",
        "supplementary_data_max_bytes": 2048,
        "amount_minor_units": [
          {"currency": "USD", "minor_units": 2}
        ]
      },
      {
        "message_type": "pacs.009",
        "direction": "inbound",
        "versions": ["pacs.009.001.08"],
        "business_services": ["fedwire.funds.01"],
        "require_app_header": true,
        "require_business_service": true,
        "require_uetr": false,
        "structured_address_mode": "require-structured",
        "supplementary_data_max_bytes": 2048,
        "amount_minor_units": [
          {"currency": "USD", "minor_units": 2}
        ]
      },
      {
        "message_type": "pacs.002",
        "direction": "inbound",
        "versions": ["pacs.002.001.10"],
        "business_services": ["fedwire.funds.01"],
        "require_app_header": true,
        "require_business_service": true,
        "require_uetr": false,
        "structured_address_mode": "require-structured",
        "supplementary_data_max_bytes": 2048,
        "amount_minor_units": []
      },
      {
        "message_type": "pacs.004",
        "direction": "inbound",
        "versions": ["pacs.004.001.09"],
        "business_services": ["fedwire.funds.01"],
        "require_app_header": true,
        "require_business_service": true,
        "require_uetr": false,
        "structured_address_mode": "require-structured",
        "supplementary_data_max_bytes": 2048,
        "amount_minor_units": []
      },
      {
        "message_type": "camt.056",
        "direction": "inbound",
        "versions": ["camt.056.001.08"],
        "business_services": ["fedwire.funds.01"],
        "require_app_header": true,
        "require_business_service": true,
        "require_uetr": false,
        "structured_address_mode": "require-structured",
        "supplementary_data_max_bytes": 2048,
        "amount_minor_units": []
      }
    ]
  },
  {
    "id": "sepa-sct-inst",
    "rail": "sepa-sct-inst",
    "embedded_signature_policy": "reject-unsupported",
    "required_reference_datasets": ["bic-lei"],
    "message_profiles": [
      {
        "message_type": "pacs.008",
        "direction": "inbound",
        "versions": ["pacs.008.001.08", "pacs.008.001.10"],
        "business_services": ["sepa.sct.inst"],
        "require_app_header": true,
        "require_business_service": true,
        "require_uetr": true,
        "structured_address_mode": "require-structured",
        "supplementary_data_max_bytes": 2048,
        "amount_minor_units": [
          {"currency": "EUR", "minor_units": 2}
        ]
      },
      {
        "message_type": "pacs.002",
        "direction": "inbound",
        "versions": ["pacs.002.001.10", "pacs.002.001.12"],
        "business_services": ["sepa.sct.inst"],
        "require_app_header": true,
        "require_business_service": true,
        "require_uetr": false,
        "structured_address_mode": "require-structured",
        "supplementary_data_max_bytes": 2048,
        "amount_minor_units": []
      },
      {
        "message_type": "pacs.004",
        "direction": "inbound",
        "versions": ["pacs.004.001.09", "pacs.004.001.10"],
        "business_services": ["sepa.sct.inst"],
        "require_app_header": true,
        "require_business_service": true,
        "require_uetr": false,
        "structured_address_mode": "require-structured",
        "supplementary_data_max_bytes": 2048,
        "amount_minor_units": []
      },
      {
        "message_type": "camt.056",
        "direction": "inbound",
        "versions": ["camt.056.001.08", "camt.056.001.09"],
        "business_services": ["sepa.sct.inst"],
        "require_app_header": true,
        "require_business_service": true,
        "require_uetr": false,
        "structured_address_mode": "require-structured",
        "supplementary_data_max_bytes": 2048,
        "amount_minor_units": []
      }
    ]
  },
  {
    "id": "securities-csd",
    "rail": "securities-csd",
    "embedded_signature_policy": "reject-unsupported",
    "required_reference_datasets": ["bic-lei", "isin-cusip", "mic-directory"],
    "message_profiles": [
      {
        "message_type": "pacs.009",
        "direction": "inbound",
        "versions": ["pacs.009.001.08", "pacs.009.001.10"],
        "business_services": ["securities.csd.cash"],
        "require_app_header": true,
        "require_business_service": true,
        "require_uetr": false,
        "structured_address_mode": "require-structured",
        "supplementary_data_max_bytes": 4096,
        "amount_minor_units": [
          {"currency": "BHD", "minor_units": 3},
          {"currency": "CLF", "minor_units": 4},
          {"currency": "EUR", "minor_units": 2},
          {"currency": "GBP", "minor_units": 2},
          {"currency": "JPY", "minor_units": 0},
          {"currency": "KWD", "minor_units": 3},
          {"currency": "USD", "minor_units": 2}
        ]
      },
      {
        "message_type": "pacs.002",
        "direction": "inbound",
        "versions": ["pacs.002.001.10", "pacs.002.001.12"],
        "business_services": ["securities.csd.cash"],
        "require_app_header": true,
        "require_business_service": true,
        "require_uetr": false,
        "structured_address_mode": "require-structured",
        "supplementary_data_max_bytes": 4096,
        "amount_minor_units": []
      },
      {
        "message_type": "pacs.004",
        "direction": "inbound",
        "versions": ["pacs.004.001.09", "pacs.004.001.10"],
        "business_services": ["securities.csd.cash"],
        "require_app_header": true,
        "require_business_service": true,
        "require_uetr": false,
        "structured_address_mode": "require-structured",
        "supplementary_data_max_bytes": 4096,
        "amount_minor_units": []
      },
      {
        "message_type": "camt.056",
        "direction": "inbound",
        "versions": ["camt.056.001.08", "camt.056.001.09"],
        "business_services": ["securities.csd.cash"],
        "require_app_header": true,
        "require_business_service": true,
        "require_uetr": false,
        "structured_address_mode": "require-structured",
        "supplementary_data_max_bytes": 4096,
        "amount_minor_units": []
      },
      {
        "message_type": "sese.023",
        "direction": "inbound",
        "versions": ["sese.023.001.11"],
        "business_services": ["securities.csd.settlement"],
        "require_app_header": false,
        "require_business_service": false,
        "require_uetr": false,
        "structured_address_mode": "permissive",
        "supplementary_data_max_bytes": 4096,
        "amount_minor_units": []
      },
      {
        "message_type": "sese.024",
        "direction": "inbound",
        "versions": ["sese.024.001.09", "sese.024.001.10"],
        "business_services": ["securities.csd.settlement"],
        "require_app_header": false,
        "require_business_service": false,
        "require_uetr": false,
        "structured_address_mode": "permissive",
        "supplementary_data_max_bytes": 4096,
        "amount_minor_units": []
      },
      {
        "message_type": "sese.025",
        "direction": "inbound",
        "versions": ["sese.025.001.08", "sese.025.001.10"],
        "business_services": ["securities.csd.settlement"],
        "require_app_header": false,
        "require_business_service": false,
        "require_uetr": false,
        "structured_address_mode": "permissive",
        "supplementary_data_max_bytes": 4096,
        "amount_minor_units": []
      },
      {
        "message_type": "sese.025",
        "direction": "follow-up",
        "versions": ["sese.025.001.10"],
        "business_services": ["securities.csd.settlement"],
        "require_app_header": false,
        "require_business_service": false,
        "require_uetr": false,
        "structured_address_mode": "permissive",
        "supplementary_data_max_bytes": 4096,
        "amount_minor_units": []
      }
    ]
  }
]
"#;

/// High-level rail family represented by an ISO bridge profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TradfiRail {
    /// Baseline ISO 20022 validation without a live payment-network profile.
    GenericIso20022,
    /// SWIFT CBPR+ cross-border payment profile.
    SwiftCbprPlus,
    /// Fedwire Funds Service ISO 20022 profile.
    FedwireFunds,
    /// SEPA Instant Credit Transfer profile.
    SepaSctInst,
    /// Securities central securities depository settlement profile.
    SecuritiesCsd,
}

impl TradfiRail {
    /// Parse a stable rail identifier.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "generic-iso20022" => Some(Self::GenericIso20022),
            "swift-cbpr-plus" => Some(Self::SwiftCbprPlus),
            "fedwire-funds" => Some(Self::FedwireFunds),
            "sepa-sct-inst" => Some(Self::SepaSctInst),
            "securities-csd" => Some(Self::SecuritiesCsd),
            _ => None,
        }
    }

    /// Stable rail identifier.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GenericIso20022 => "generic-iso20022",
            Self::SwiftCbprPlus => "swift-cbpr-plus",
            Self::FedwireFunds => "fedwire-funds",
            Self::SepaSctInst => "sepa-sct-inst",
            Self::SecuritiesCsd => "securities-csd",
        }
    }
}

/// Direction in which a profile permits a message to move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MessageDirection {
    /// Inbound network message submitted to Torii.
    Inbound,
    /// Outbound status or lifecycle message generated by Torii.
    Outbound,
    /// Follow-up message that updates the lifecycle of a prior instruction.
    FollowUp,
}

impl MessageDirection {
    /// Parse a stable direction identifier.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "inbound" => Some(Self::Inbound),
            "outbound" => Some(Self::Outbound),
            "follow-up" | "followup" => Some(Self::FollowUp),
            _ => None,
        }
    }

    /// Stable direction identifier.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Inbound => "inbound",
            Self::Outbound => "outbound",
            Self::FollowUp => "follow-up",
        }
    }
}

/// Reference dataset required by a rail profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReferenceDatasetRequirement {
    /// BIC to LEI mapping snapshot.
    BicLei,
    /// ISIN to CUSIP instrument crosswalk snapshot.
    IsinCusip,
    /// MIC directory snapshot.
    MicDirectory,
}

impl ReferenceDatasetRequirement {
    /// Parse a stable dataset identifier.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "bic-lei" | "bic_lei" => Some(Self::BicLei),
            "isin-cusip" | "isin_cusip" => Some(Self::IsinCusip),
            "mic-directory" | "mic_directory" => Some(Self::MicDirectory),
            _ => None,
        }
    }

    /// Stable dataset identifier.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BicLei => "bic-lei",
            Self::IsinCusip => "isin-cusip",
            Self::MicDirectory => "mic-directory",
        }
    }
}

/// Policy for embedded XMLDSig/XAdES blocks inside ISO envelopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddedSignaturePolicy {
    /// Preserve an audit marker but do not verify the embedded signature.
    RecordOnly,
    /// Reject messages carrying embedded signatures until verification is wired.
    RejectUnsupported,
    /// Require a verified signature before accepting the message.
    RequireVerified,
}

impl EmbeddedSignaturePolicy {
    /// Parse a stable policy identifier.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "record-only" | "record_only" => Some(Self::RecordOnly),
            "reject-unsupported" | "reject_unsupported" => Some(Self::RejectUnsupported),
            "require-verified" | "require_verified" => Some(Self::RequireVerified),
            _ => None,
        }
    }

    /// Stable policy identifier.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RecordOnly => "record-only",
            Self::RejectUnsupported => "reject-unsupported",
            Self::RequireVerified => "require-verified",
        }
    }
}

/// Structured postal-address policy applied by a rail profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuredAddressMode {
    /// Do not require structured address fields.
    Permissive,
    /// Reject unstructured-only postal address payloads for the profile.
    RequireStructured,
    /// Reject any unstructured postal address field.
    ForbidUnstructured,
}

impl StructuredAddressMode {
    /// Parse a stable structured-address mode.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "permissive" => Some(Self::Permissive),
            "require-structured" | "require_structured" => Some(Self::RequireStructured),
            "forbid-unstructured" | "forbid_unstructured" => Some(Self::ForbidUnstructured),
            _ => None,
        }
    }

    /// Stable structured-address mode.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Permissive => "permissive",
            Self::RequireStructured => "require-structured",
            Self::ForbidUnstructured => "forbid-unstructured",
        }
    }
}

/// Message-specific validation profile nested under a rail profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageProfile {
    /// Canonical message family such as `pacs.008`.
    pub message_type: String,
    /// Direction allowed for this profile entry.
    pub direction: MessageDirection,
    /// Exact ISO message definition identifiers accepted by this profile.
    pub versions: Vec<String>,
    /// Business service identifiers accepted by this profile.
    pub business_services: Vec<String>,
    /// Whether the Business Application Header must be present.
    pub require_app_header: bool,
    /// Whether `BizSvc` must be present and match [`Self::business_services`].
    pub require_business_service: bool,
    /// Whether the message must carry a UETR.
    pub require_uetr: bool,
    /// Structured postal-address policy.
    pub structured_address_mode: StructuredAddressMode,
    /// Maximum accepted serialized supplementary-data bytes.
    pub supplementary_data_max_bytes: usize,
    /// Currency-specific minor-unit overrides.
    pub amount_minor_units: BTreeMap<String, u8>,
}

impl MessageProfile {
    /// Returns `true` if `version` is accepted by this profile.
    #[must_use]
    pub fn allows_version(&self, version: &str) -> bool {
        self.versions
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(version))
    }

    /// Returns `true` if `business_service` is accepted by this profile.
    #[must_use]
    pub fn allows_business_service(&self, business_service: &str) -> bool {
        self.business_services.is_empty()
            || self
                .business_services
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(business_service))
    }

    /// Minor units allowed for the currency. Unlisted currencies default to two.
    #[must_use]
    pub fn minor_units_for(&self, currency: &str) -> u8 {
        let key = currency.trim().to_ascii_uppercase();
        self.amount_minor_units.get(&key).copied().unwrap_or(2)
    }
}

/// Complete rail profile used by the ISO bridge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradfiRailProfile {
    /// Stable profile identifier.
    pub id: String,
    /// High-level rail family.
    pub rail: TradfiRail,
    /// Policy for embedded XMLDSig/XAdES blocks.
    pub embedded_signature_policy: EmbeddedSignaturePolicy,
    /// Accepted SHA-256 pins of raw XMLDSig P-256 public keys.
    pub trusted_public_key_sha256: Vec<String>,
    /// Accepted SHA-256 pins of DER-encoded XMLDSig X.509 certificates.
    pub trusted_certificate_sha256: Vec<String>,
    /// Datasets that must be loaded before accepting live-profile messages.
    pub required_reference_datasets: Vec<ReferenceDatasetRequirement>,
    /// Message profiles supported by this rail profile.
    pub message_profiles: Vec<MessageProfile>,
}

impl TradfiRailProfile {
    /// Find a message profile by type and direction.
    #[must_use]
    pub fn message_profile(
        &self,
        message_type: &str,
        direction: MessageDirection,
    ) -> Option<&MessageProfile> {
        self.message_profiles.iter().find(|profile| {
            profile.direction == direction
                && profile.message_type.eq_ignore_ascii_case(message_type)
        })
    }

    /// Returns `true` if the profile requires the dataset.
    #[must_use]
    pub fn requires_dataset(&self, dataset: ReferenceDatasetRequirement) -> bool {
        self.required_reference_datasets.contains(&dataset)
    }

    /// Returns `true` when this profile has at least one configured XMLDSig trust pin.
    #[must_use]
    pub fn has_xml_signature_trust_anchors(&self) -> bool {
        !self.trusted_public_key_sha256.is_empty() || !self.trusted_certificate_sha256.is_empty()
    }

    /// Returns `true` when the verified XMLDSig key material matches this profile's pins.
    #[must_use]
    pub fn accepts_xml_signature_key(
        &self,
        public_key_sha256: &str,
        certificate_sha256: &[String],
    ) -> bool {
        self.trusted_public_key_sha256
            .iter()
            .any(|pin| pin == public_key_sha256)
            || certificate_sha256.iter().any(|digest| {
                self.trusted_certificate_sha256
                    .iter()
                    .any(|pin| pin == digest)
            })
    }
}

/// Parse the embedded Norito JSON profile catalog.
///
/// # Panics
/// Panics only if the embedded static catalog is malformed.
#[must_use]
pub fn default_profiles() -> Vec<TradfiRailProfile> {
    let value: Value = json::from_json(DEFAULT_PROFILES_JSON)
        .expect("embedded ISO bridge profile catalog must be valid Norito JSON");
    profiles_from_value(&value).expect("embedded ISO bridge profile catalog must be valid")
}

/// Return the embedded catalog keyed by profile identifier.
///
/// # Panics
/// Panics only if the embedded static catalog is malformed.
#[must_use]
pub fn default_profile_catalog() -> BTreeMap<String, TradfiRailProfile> {
    default_profiles()
        .into_iter()
        .map(|profile| (profile.id.clone(), profile))
        .collect()
}

/// Return one embedded profile by identifier.
#[must_use]
pub fn default_profile(id: &str) -> Option<TradfiRailProfile> {
    default_profiles()
        .into_iter()
        .find(|profile| profile.id.eq_ignore_ascii_case(id))
}

fn profiles_from_value(value: &Value) -> Result<Vec<TradfiRailProfile>, String> {
    let profiles = value
        .as_array()
        .ok_or_else(|| "profile catalog must be an array".to_owned())?;
    profiles.iter().map(profile_from_value).collect()
}

fn profile_from_value(value: &Value) -> Result<TradfiRailProfile, String> {
    let obj = value
        .as_object()
        .ok_or_else(|| "profile entry must be an object".to_owned())?;
    let id = required_string(obj, "id")?;
    let rail = TradfiRail::parse(required_string(obj, "rail")?)
        .ok_or_else(|| format!("profile `{id}` has unknown rail"))?;
    let embedded_signature_policy =
        EmbeddedSignaturePolicy::parse(required_string(obj, "embedded_signature_policy")?)
            .ok_or_else(|| format!("profile `{id}` has unknown embedded signature policy"))?;
    let trusted_public_key_sha256 =
        canonical_sha256_pins(optional_string_array(obj, "trusted_public_key_sha256")?)?;
    let trusted_certificate_sha256 =
        canonical_sha256_pins(optional_string_array(obj, "trusted_certificate_sha256")?)?;
    let required_reference_datasets = optional_string_array(obj, "required_reference_datasets")?
        .into_iter()
        .map(|raw| {
            ReferenceDatasetRequirement::parse(&raw)
                .ok_or_else(|| format!("profile `{id}` has unknown reference dataset `{raw}`"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let message_values = obj
        .get("message_profiles")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("profile `{id}` missing message_profiles array"))?;
    let message_profiles = message_values
        .iter()
        .map(message_profile_from_value)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TradfiRailProfile {
        id: id.to_owned(),
        rail,
        embedded_signature_policy,
        trusted_public_key_sha256,
        trusted_certificate_sha256,
        required_reference_datasets,
        message_profiles,
    })
}

fn canonical_sha256_pins(values: Vec<String>) -> Result<Vec<String>, String> {
    values
        .into_iter()
        .map(|value| {
            let trimmed = value.trim();
            if trimmed.len() != 64 || !trimmed.chars().all(|ch| ch.is_ascii_hexdigit()) {
                return Err(format!(
                    "XMLDSig trust pin `{value}` must be a SHA-256 hex string"
                ));
            }
            let canonical = trimmed.to_ascii_lowercase();
            if trimmed != canonical {
                return Err(format!(
                    "XMLDSig trust pin `{value}` must use lowercase canonical hex"
                ));
            }
            if canonical.chars().all(|ch| ch == '0') {
                return Err("XMLDSig trust pin must not be all zero".to_owned());
            }
            Ok(canonical)
        })
        .collect()
}

fn message_profile_from_value(value: &Value) -> Result<MessageProfile, String> {
    let obj = value
        .as_object()
        .ok_or_else(|| "message profile must be an object".to_owned())?;
    let message_type = required_string(obj, "message_type")?.to_owned();
    let direction = MessageDirection::parse(required_string(obj, "direction")?)
        .ok_or_else(|| format!("message profile `{message_type}` has unknown direction"))?;
    let versions = optional_string_array(obj, "versions")?;
    let business_services = optional_string_array(obj, "business_services")?;
    let structured_address_mode =
        StructuredAddressMode::parse(required_string(obj, "structured_address_mode")?).ok_or_else(
            || format!("message profile `{message_type}` has unknown structured address mode"),
        )?;
    let supplementary_data_max_bytes =
        optional_usize(obj, "supplementary_data_max_bytes")?.unwrap_or(4096);
    let amount_minor_units = parse_minor_units(obj.get("amount_minor_units"), &message_type)?;
    Ok(MessageProfile {
        message_type,
        direction,
        versions,
        business_services,
        require_app_header: optional_bool(obj, "require_app_header")?.unwrap_or(false),
        require_business_service: optional_bool(obj, "require_business_service")?.unwrap_or(false),
        require_uetr: optional_bool(obj, "require_uetr")?.unwrap_or(false),
        structured_address_mode,
        supplementary_data_max_bytes,
        amount_minor_units,
    })
}

fn parse_minor_units(
    value: Option<&Value>,
    message_type: &str,
) -> Result<BTreeMap<String, u8>, String> {
    let mut out = BTreeMap::new();
    let Some(value) = value else {
        return Ok(out);
    };
    for entry in value.as_array().ok_or_else(|| {
        format!("message profile `{message_type}` amount_minor_units must be array")
    })? {
        let obj = entry
            .as_object()
            .ok_or_else(|| "amount_minor_units entry must be an object".to_owned())?;
        let currency = required_string(obj, "currency")?
            .trim()
            .to_ascii_uppercase();
        if currency.len() != 3 || !currency.chars().all(|c| c.is_ascii_uppercase()) {
            return Err(format!(
                "invalid currency `{currency}` in amount minor-unit profile"
            ));
        }
        let units = optional_usize(obj, "minor_units")?
            .ok_or_else(|| format!("currency `{currency}` missing minor_units"))?;
        let units = u8::try_from(units)
            .map_err(|_| format!("currency `{currency}` minor_units is too large"))?;
        out.insert(currency, units);
    }
    Ok(out)
}

fn required_string<'a>(obj: &'a BTreeMap<String, Value>, key: &str) -> Result<&'a str, String> {
    obj.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing string field `{key}`"))
}

fn optional_string_array(obj: &BTreeMap<String, Value>, key: &str) -> Result<Vec<String>, String> {
    let Some(value) = obj.get(key) else {
        return Ok(Vec::new());
    };
    let array = value
        .as_array()
        .ok_or_else(|| format!("field `{key}` must be an array"))?;
    array
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| format!("field `{key}` entries must be strings"))
        })
        .collect()
}

fn optional_bool(obj: &BTreeMap<String, Value>, key: &str) -> Result<Option<bool>, String> {
    obj.get(key)
        .map(|value| {
            value
                .as_bool()
                .ok_or_else(|| format!("field `{key}` must be a boolean"))
        })
        .transpose()
}

fn optional_usize(obj: &BTreeMap<String, Value>, key: &str) -> Result<Option<usize>, String> {
    obj.get(key)
        .map(|value| {
            value
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .ok_or_else(|| format!("field `{key}` must be an unsigned integer"))
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_catalog_contains_expected_profiles() {
        let catalog = default_profile_catalog();
        for id in [
            "generic-iso20022",
            "swift-cbpr-plus",
            "fedwire-funds",
            "sepa-sct-inst",
            "securities-csd",
        ] {
            assert!(catalog.contains_key(id), "missing profile {id}");
        }
    }

    #[test]
    fn live_profiles_reject_unsupported_embedded_signatures() {
        let catalog = default_profile_catalog();
        assert_eq!(
            catalog["generic-iso20022"].embedded_signature_policy,
            EmbeddedSignaturePolicy::RecordOnly
        );
        assert_eq!(
            catalog["swift-cbpr-plus"].embedded_signature_policy,
            EmbeddedSignaturePolicy::RejectUnsupported
        );
        assert_eq!(
            catalog["fedwire-funds"].embedded_signature_policy,
            EmbeddedSignaturePolicy::RejectUnsupported
        );
        assert!(
            !catalog["swift-cbpr-plus"].has_xml_signature_trust_anchors(),
            "default live profiles must not silently trust XMLDSig keys"
        );
    }

    #[test]
    fn xml_signature_key_pins_accept_any_verified_certificate_digest() {
        let mut profile = default_profile("generic-iso20022").expect("profile");
        profile.trusted_public_key_sha256 = vec!["aa".repeat(32)];
        profile.trusted_certificate_sha256 = vec!["bb".repeat(32)];

        assert!(profile.accepts_xml_signature_key(&"aa".repeat(32), &[]));
        assert!(
            profile
                .accepts_xml_signature_key(&"11".repeat(32), &["cc".repeat(32), "bb".repeat(32)])
        );
        assert!(!profile.accepts_xml_signature_key(&"11".repeat(32), &["cc".repeat(32)]));
    }

    #[test]
    fn required_reference_data_is_profile_specific() {
        let catalog = default_profile_catalog();
        assert!(
            catalog["generic-iso20022"]
                .required_reference_datasets
                .is_empty()
        );
        assert!(catalog["swift-cbpr-plus"].requires_dataset(ReferenceDatasetRequirement::BicLei));
        assert!(catalog["securities-csd"].requires_dataset(ReferenceDatasetRequirement::IsinCusip));
        assert!(
            catalog["securities-csd"].requires_dataset(ReferenceDatasetRequirement::MicDirectory)
        );
    }

    #[test]
    fn minor_units_default_to_two_with_overrides() {
        let catalog = default_profile_catalog();
        let profile = catalog["swift-cbpr-plus"]
            .message_profile("pacs.008", MessageDirection::Inbound)
            .expect("pacs.008 profile");
        assert_eq!(profile.minor_units_for("USD"), 2);
        assert_eq!(profile.minor_units_for("JPY"), 0);
        assert_eq!(profile.minor_units_for("KWD"), 3);
        assert_eq!(profile.minor_units_for("XAU"), 2);
    }

    #[test]
    fn default_catalog_exposes_inbound_lifecycle_messages() {
        let catalog = default_profile_catalog();
        let generic = &catalog["generic-iso20022"];
        for message_type in [
            "pacs.002", "pacs.004", "camt.056", "sese.023", "sese.024", "sese.025",
        ] {
            assert!(
                generic
                    .message_profile(message_type, MessageDirection::Inbound)
                    .is_some(),
                "generic profile missing {message_type}"
            );
        }
        let securities = &catalog["securities-csd"];
        assert!(
            securities
                .message_profile("sese.024", MessageDirection::Inbound)
                .is_some()
        );
        assert!(
            securities
                .message_profile("sese.025", MessageDirection::Inbound)
                .is_some()
        );
    }
}
