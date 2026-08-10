//! Bridge proof ingestion instructions.

use super::*;

/// Activation update for one exact governed SCCP route.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct SccpSetRouteActivationV1 {
    /// Exact route to update.
    pub key: crate::bridge::SccpRouteKeyV1,
    /// Activation state that must still be current when governance executes.
    pub expected_current: crate::bridge::SccpRouteActivationV1,
    /// Legal replacement activation state.
    pub next: crate::bridge::SccpRouteActivationV1,
    /// Required authenticated delayed-claim cutoff when `next` is terminal;
    /// absent for every nonterminal transition.
    pub inbound_finality_cutoff: Option<crate::bridge::SccpInboundFinalityCutoffV1>,
}

/// Append-only native trust-anchor update for one exact governed SCCP lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct SccpAdvanceLaneTrustAnchorV1 {
    /// Exact lane whose current checkpoint advances.
    pub lane_id: crate::bridge::SccpLaneIdV1,
    /// Checkpoint that must still be current when governance executes.
    pub expected_current: crate::bridge::SccpNativeTrustAnchorV1,
    /// New family-tagged native checkpoint appended to retained history.
    pub next: crate::bridge::SccpNativeTrustAnchorV1,
}

/// First native trust-anchor installation for one exact governed SCCP lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct SccpInitializeLaneTrustAnchorV1 {
    /// Exact lane whose absent checkpoint is initialized.
    pub lane_id: crate::bridge::SccpLaneIdV1,
    /// Checkpoint state that must still be absent when governance executes.
    pub expected_current: Option<crate::bridge::SccpNativeTrustAnchorV1>,
    /// First valid family-tagged native checkpoint.
    pub initial: crate::bridge::SccpNativeTrustAnchorV1,
}

/// Atomic registration input for one exact staged route.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct SccpRegisterRouteV1 {
    /// Complete immutable route, necessarily staged at registration.
    pub route: crate::bridge::SccpGovernedRouteV1,
    /// Optional initial lane anchor, or the exact current value when joining a lane.
    pub native_trust_anchor: Option<crate::bridge::SccpNativeTrustAnchorV1>,
}

/// Atomic cutover from one immutable route revision to its staged successor.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct SccpSwitchRouteRevisionV1 {
    /// Currently selected immutable revision.
    pub previous_key: crate::bridge::SccpRouteKeyV1,
    /// Activation state that the previous revision must still have.
    pub expected_previous: crate::bridge::SccpRouteActivationV1,
    /// Inbound-draining or emergency-paused state assigned to the previous revision.
    pub previous_next: crate::bridge::SccpRouteActivationV1,
    /// Required authenticated delayed-claim cutoff when the previous revision
    /// becomes terminal in this atomic cutover; otherwise absent.
    pub previous_inbound_finality_cutoff: Option<crate::bridge::SccpInboundFinalityCutoffV1>,
    /// Already-registered staged monotonic successor.
    pub successor_key: crate::bridge::SccpRouteKeyV1,
    /// Bidirectional state assigned to the successor.
    pub successor_next: crate::bridge::SccpRouteActivationV1,
}

/// Closed atomic SCCP route-governance action.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
#[norito(tag = "action", content = "route")]
#[expect(
    clippy::large_enum_variant,
    reason = "boxing a governance action would change its canonical Norito wire shape"
)]
pub enum SccpRouteGovernanceActionV1 {
    /// Register one complete immutable route in staged state.
    #[codec(index = 0)]
    Register(SccpRegisterRouteV1),
    /// Change only the route's directional activation state.
    #[codec(index = 1)]
    SetActivation(SccpSetRouteActivationV1),
    /// Atomically stop one revision and enable its staged successor.
    #[codec(index = 2)]
    SwitchRevision(SccpSwitchRouteRevisionV1),
    /// Install the first native source trust anchor using a `None` compare-and-swap.
    #[codec(index = 3)]
    InitializeTrustAnchor(SccpInitializeLaneTrustAnchorV1),
    /// Append and select the next native source trust anchor without deleting history.
    #[codec(index = 4)]
    AdvanceTrustAnchor(SccpAdvanceLaneTrustAnchorV1),
    /// Remove a never-used staged route.
    #[codec(index = 5)]
    Remove(crate::bridge::SccpRouteKeyV1),
}

/// Complete action-bound preimage approved by SCCP route governance.
///
/// The exact genesis-derived network identity is part of the canonical
/// preimage, so an approval cannot be replayed on a network with the same
/// display label or rebound to a different registry action.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpRouteGovernanceAnchorV1 {
    /// Exact genesis-derived network identity on which this action may execute.
    pub network_id: NetworkId,
    /// Complete closed registry action approved by the referendum.
    pub action: SccpRouteGovernanceActionV1,
}

impl SccpRouteGovernanceActionV1 {
    /// Validate invariants that do not require current world state.
    ///
    /// # Errors
    ///
    /// Returns [`crate::bridge::SccpRouteValidationError`] when the action's
    /// route, activation transition, revision, or trust-anchor update is invalid.
    #[expect(
        clippy::too_many_lines,
        reason = "one exhaustive match keeps all closed governance variants visibly fail-closed"
    )]
    pub fn validate_static(&self) -> Result<(), crate::bridge::SccpRouteValidationError> {
        use crate::bridge::SccpRouteValidationError;

        match self {
            Self::Register(registration) => {
                registration.route.validate_registration()?;
                registration
                    .route
                    .validate_with_anchor(registration.native_trust_anchor)
            }
            Self::SetActivation(update) => {
                update.key.validate()?;
                let cutoff_valid = if update.next.is_terminal() {
                    update
                        .inbound_finality_cutoff
                        .is_some_and(crate::bridge::SccpInboundFinalityCutoffV1::is_well_formed)
                } else {
                    update.inbound_finality_cutoff.is_none()
                };
                if !update.expected_current.can_transition_to(update.next) || !cutoff_valid {
                    return Err(SccpRouteValidationError::InvalidActivationTransition);
                }
                Ok(())
            }
            Self::SwitchRevision(update) => {
                update.previous_key.validate()?;
                update.successor_key.validate()?;
                let same_lineage = update.previous_key.lane_id == update.successor_key.lane_id
                    && update.previous_key.route_id == update.successor_key.route_id
                    && update.previous_key.asset_key == update.successor_key.asset_key;
                let terminal_cutover = update.previous_next.is_terminal();
                let previous_transition_valid = if terminal_cutover {
                    matches!(
                        update.expected_previous,
                        crate::bridge::SccpRouteActivationV1::Bidirectional
                            | crate::bridge::SccpRouteActivationV1::InboundOnly
                            | crate::bridge::SccpRouteActivationV1::Paused
                    )
                } else {
                    update
                        .expected_previous
                        .can_transition_to(update.previous_next)
                };
                let cutoff_valid = if terminal_cutover {
                    update
                        .previous_inbound_finality_cutoff
                        .is_some_and(crate::bridge::SccpInboundFinalityCutoffV1::is_well_formed)
                } else {
                    update.previous_inbound_finality_cutoff.is_none()
                };
                if !same_lineage
                    || update.successor_key.revision
                        != update
                            .previous_key
                            .revision
                            .checked_add(1)
                            .ok_or(SccpRouteValidationError::InvalidRouteRevision)?
                    || !previous_transition_valid
                    || !cutoff_valid
                    || !matches!(
                        update.previous_next,
                        crate::bridge::SccpRouteActivationV1::InboundOnly
                            | crate::bridge::SccpRouteActivationV1::Paused
                            | crate::bridge::SccpRouteActivationV1::Retired
                    )
                    || !crate::bridge::SccpRouteActivationV1::Staged
                        .can_transition_to(update.successor_next)
                    || update.successor_next != crate::bridge::SccpRouteActivationV1::Bidirectional
                {
                    return Err(SccpRouteValidationError::InvalidActivationTransition);
                }
                Ok(())
            }
            Self::InitializeTrustAnchor(update) => {
                if !update.lane_id.is_well_formed()
                    || !update.lane_id.source.is_external()
                    || !update.lane_id.target.is_sora()
                    || update.expected_current.is_some()
                    || !update.initial.is_well_formed()
                    || !update
                        .initial
                        .backend
                        .supports_source_network(update.lane_id.source)
                {
                    return Err(SccpRouteValidationError::InvalidTrustAnchorInitialize);
                }
                Ok(())
            }
            Self::AdvanceTrustAnchor(update) => {
                if !update.lane_id.is_well_formed()
                    || !update.lane_id.source.is_external()
                    || !update.lane_id.target.is_sora()
                    || !update.expected_current.is_well_formed()
                    || !update.next.is_well_formed()
                    || update.expected_current.backend != update.next.backend
                    || !update
                        .next
                        .backend
                        .supports_source_network(update.lane_id.source)
                    || update.expected_current.anchor_hash == update.next.anchor_hash
                    || update.next.checkpoint_height <= update.expected_current.checkpoint_height
                {
                    return Err(SccpRouteValidationError::InvalidTrustAnchorAdvance);
                }
                Ok(())
            }
            Self::Remove(key) => key.validate(),
        }
    }
}

isi! {
    /// Submit a bridge proof artifact for verification and registry retention.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(deny_unknown_fields)]
    pub struct SubmitBridgeProof {
        /// Typed bridge proof payload and its payload-owned verifier binding.
        pub proof: crate::bridge::BridgeProof,
    }
}

impl crate::seal::Instruction for SubmitBridgeProof {}

impl SubmitBridgeProof {
    /// Construct a new submission wrapping the provided proof.
    pub fn new(proof: crate::bridge::BridgeProof) -> Self {
        Self { proof }
    }
}

isi! {
    /// Record a bridge receipt and emit a typed bridge event.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(deny_unknown_fields)]
    pub struct RecordBridgeReceipt {
        /// Bridge receipt payload to record.
        pub receipt: crate::bridge::BridgeReceipt,
    }
}

impl crate::seal::Instruction for RecordBridgeReceipt {}

impl RecordBridgeReceipt {
    /// Construct a new record instruction for the provided receipt.
    pub fn new(receipt: crate::bridge::BridgeReceipt) -> Self {
        Self { receipt }
    }
}

isi! {
    /// Rejected direct SCCP route-governance mutation token.
    ///
    /// Core and the default executor reject this instruction unconditionally;
    /// route actions execute only through `EnactSccpRouteGovernance`.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(deny_unknown_fields)]
    pub struct ApplySccpRouteGovernance {
        /// Complete closed governance action.
        pub action: SccpRouteGovernanceActionV1,
    }
}

impl crate::seal::Instruction for ApplySccpRouteGovernance {}

impl ApplySccpRouteGovernance {
    /// Construct a direct mutation token that admission will reject.
    pub fn new(action: SccpRouteGovernanceActionV1) -> Self {
        Self { action }
    }
}

isi! {
    /// Fund one exact route-scoped SCCP protocol escrow.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(deny_unknown_fields)]
    pub struct FundSccpRouteEscrow {
        /// Immutable governed route whose escrow receives the funds.
        pub route_key: crate::bridge::SccpRouteKeyV1,
        /// Exact governed settlement asset, repeated to bind signed intent.
        pub asset_definition_id: AssetDefinitionId,
        /// Positive amount transferred from the route's exact custody owner.
        pub amount: Quantity,
    }
}

impl crate::seal::Instruction for FundSccpRouteEscrow {}

isi! {
    /// Refund one exact inactive route-scoped SCCP protocol escrow.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(deny_unknown_fields)]
    pub struct RefundSccpRouteEscrow {
        /// Immutable governed route whose escrow is debited.
        pub route_key: crate::bridge::SccpRouteKeyV1,
        /// Exact governed settlement asset, repeated to bind signed intent.
        pub asset_definition_id: AssetDefinitionId,
        /// Positive amount returned only to the route's exact custody owner.
        pub amount: Quantity,
    }
}

impl crate::seal::Instruction for RefundSccpRouteEscrow {}

fn bridge_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}

impl<'a> norito::core::DecodeFromSlice<'a> for SubmitBridgeProof {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = bridge_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let proof = super::decode_aos_canonical_field::<crate::bridge::BridgeProof>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { proof }, offset))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for RecordBridgeReceipt {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = bridge_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let receipt = super::decode_aos_canonical_field::<crate::bridge::BridgeReceipt>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { receipt }, offset))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for ApplySccpRouteGovernance {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = bridge_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let action = super::decode_aos_canonical_field::<SccpRouteGovernanceActionV1>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { action }, offset))
    }
}

macro_rules! impl_sccp_route_escrow_decode_from_slice {
    ($ty:ty) => {
        impl<'a> norito::core::DecodeFromSlice<'a> for $ty {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let flags = bridge_decode_flags();
                if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
                    return super::decode_packed_instruction_payload::<Self>(bytes);
                }

                let mut offset = 0usize;
                let route_key = super::decode_aos_canonical_field::<crate::bridge::SccpRouteKeyV1>(
                    super::read_aos_field(bytes, &mut offset, flags)?,
                    flags,
                )?;
                let asset_definition_id = super::decode_aos_canonical_field::<AssetDefinitionId>(
                    super::read_aos_field(bytes, &mut offset, flags)?,
                    flags,
                )?;
                let amount = super::decode_aos_canonical_field::<Quantity>(
                    super::read_aos_field(bytes, &mut offset, flags)?,
                    flags,
                )?;
                if offset != bytes.len() {
                    return Err(norito::core::Error::LengthMismatch);
                }
                norito::core::note_payload_access(bytes, offset);
                Ok((
                    Self {
                        route_key,
                        asset_definition_id,
                        amount,
                    },
                    offset,
                ))
            }
        }
    };
}

impl_sccp_route_escrow_decode_from_slice!(FundSccpRouteEscrow);
impl_sccp_route_escrow_decode_from_slice!(RefundSccpRouteEscrow);

isi! {
    /// Record an SCCP message payload for block-level commitment anchoring.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(deny_unknown_fields)]
    pub struct RecordSccpMessage {
        /// Exact governed lane and destination binding for this outbound message.
        pub context: crate::bridge::SccpOutboundMessageContextV1,
        /// Canonical SCCP payload bytes.
        pub payload_bytes: Vec<u8>,
    }
}

impl crate::seal::Instruction for RecordSccpMessage {}

impl RecordSccpMessage {
    /// Construct an SCCP message record instruction for the exact governed context.
    pub fn new(
        context: crate::bridge::SccpOutboundMessageContextV1,
        payload_bytes: Vec<u8>,
    ) -> Self {
        Self {
            context,
            payload_bytes,
        }
    }
}

#[cfg(test)]
mod sccp_governance_tests {
    use super::*;
    use crate::bridge::{
        BridgeNativeProofBackendV1, SccpLaneIdV1, SccpNativeTrustAnchorV1, SccpNetworkV1,
        SccpRouteActivationV1, SccpRouteKeyV1, SccpRouteValidationError,
    };

    fn lane() -> SccpLaneIdV1 {
        SccpLaneIdV1 {
            source: SccpNetworkV1::EthereumMainnet,
            target: SccpNetworkV1::SoraTaira,
        }
    }

    fn anchor(hash: u8, height: u64) -> SccpNativeTrustAnchorV1 {
        SccpNativeTrustAnchorV1 {
            backend: BridgeNativeProofBackendV1::EthereumBeacon,
            anchor_hash: [hash; 32],
            checkpoint_height: height,
        }
    }

    fn key(revision: u32) -> SccpRouteKeyV1 {
        SccpRouteKeyV1 {
            lane_id: lane(),
            route_id: "taira_eth_xor".to_owned(),
            asset_key: "xor".to_owned(),
            revision,
        }
    }

    #[test]
    fn trust_anchor_initialize_is_strict_none_to_some_cas() {
        let valid =
            SccpRouteGovernanceActionV1::InitializeTrustAnchor(SccpInitializeLaneTrustAnchorV1 {
                lane_id: lane(),
                expected_current: None,
                initial: anchor(1, 100),
            });
        assert!(valid.validate_static().is_ok());

        let stale =
            SccpRouteGovernanceActionV1::InitializeTrustAnchor(SccpInitializeLaneTrustAnchorV1 {
                lane_id: lane(),
                expected_current: Some(anchor(1, 100)),
                initial: anchor(2, 101),
            });
        assert_eq!(
            stale.validate_static(),
            Err(SccpRouteValidationError::InvalidTrustAnchorInitialize)
        );

        let zero =
            SccpRouteGovernanceActionV1::InitializeTrustAnchor(SccpInitializeLaneTrustAnchorV1 {
                lane_id: lane(),
                expected_current: None,
                initial: SccpNativeTrustAnchorV1 {
                    anchor_hash: [0; 32],
                    ..anchor(1, 100)
                },
            });
        assert_eq!(
            zero.validate_static(),
            Err(SccpRouteValidationError::InvalidTrustAnchorInitialize)
        );
    }

    #[test]
    fn trust_anchor_advance_rejects_same_rollback_and_cross_family() {
        let valid = SccpRouteGovernanceActionV1::AdvanceTrustAnchor(SccpAdvanceLaneTrustAnchorV1 {
            lane_id: lane(),
            expected_current: anchor(1, 100),
            next: anchor(2, 101),
        });
        assert!(valid.validate_static().is_ok());

        for next in [anchor(1, 101), anchor(2, 100), anchor(2, 99)] {
            let action =
                SccpRouteGovernanceActionV1::AdvanceTrustAnchor(SccpAdvanceLaneTrustAnchorV1 {
                    lane_id: lane(),
                    expected_current: anchor(1, 100),
                    next,
                });
            assert_eq!(
                action.validate_static(),
                Err(SccpRouteValidationError::InvalidTrustAnchorAdvance)
            );
        }

        let cross_family =
            SccpRouteGovernanceActionV1::AdvanceTrustAnchor(SccpAdvanceLaneTrustAnchorV1 {
                lane_id: lane(),
                expected_current: anchor(1, 100),
                next: SccpNativeTrustAnchorV1 {
                    backend: BridgeNativeProofBackendV1::TronDpos,
                    anchor_hash: [2; 32],
                    checkpoint_height: 101,
                },
            });
        assert_eq!(
            cross_family.validate_static(),
            Err(SccpRouteValidationError::InvalidTrustAnchorAdvance)
        );
    }

    #[test]
    fn revision_switch_requires_checked_successor_and_draining_previous() {
        let valid = SccpRouteGovernanceActionV1::SwitchRevision(SccpSwitchRouteRevisionV1 {
            previous_key: key(1),
            expected_previous: SccpRouteActivationV1::Bidirectional,
            previous_next: SccpRouteActivationV1::InboundOnly,
            previous_inbound_finality_cutoff: None,
            successor_key: key(2),
            successor_next: SccpRouteActivationV1::Bidirectional,
        });
        assert!(valid.validate_static().is_ok());

        let overflow = SccpRouteGovernanceActionV1::SwitchRevision(SccpSwitchRouteRevisionV1 {
            previous_key: key(u32::MAX),
            expected_previous: SccpRouteActivationV1::Bidirectional,
            previous_next: SccpRouteActivationV1::InboundOnly,
            previous_inbound_finality_cutoff: None,
            successor_key: key(u32::MAX),
            successor_next: SccpRouteActivationV1::Bidirectional,
        });
        assert_eq!(
            overflow.validate_static(),
            Err(SccpRouteValidationError::InvalidRouteRevision)
        );

        let unsafe_retire =
            SccpRouteGovernanceActionV1::SwitchRevision(SccpSwitchRouteRevisionV1 {
                previous_key: key(1),
                expected_previous: SccpRouteActivationV1::Bidirectional,
                previous_next: SccpRouteActivationV1::Retired,
                previous_inbound_finality_cutoff: None,
                successor_key: key(2),
                successor_next: SccpRouteActivationV1::Bidirectional,
            });
        assert_eq!(
            unsafe_retire.validate_static(),
            Err(SccpRouteValidationError::InvalidActivationTransition)
        );

        let safe_retire = SccpRouteGovernanceActionV1::SwitchRevision(SccpSwitchRouteRevisionV1 {
            previous_key: key(1),
            expected_previous: SccpRouteActivationV1::Bidirectional,
            previous_next: SccpRouteActivationV1::Retired,
            previous_inbound_finality_cutoff: Some(crate::bridge::SccpInboundFinalityCutoffV1 {
                trust_anchor_hash: [0x91; 32],
                max_anchor_interval_height: 101,
            }),
            successor_key: key(2),
            successor_next: SccpRouteActivationV1::Bidirectional,
        });
        assert!(safe_retire.validate_static().is_ok());
    }

    #[test]
    fn terminal_activation_requires_one_well_formed_inbound_finality_cutoff() {
        let cutoff = crate::bridge::SccpInboundFinalityCutoffV1 {
            trust_anchor_hash: anchor(1, 100).anchor_hash,
            max_anchor_interval_height: 150,
        };
        let valid = SccpRouteGovernanceActionV1::SetActivation(SccpSetRouteActivationV1 {
            key: key(1),
            expected_current: SccpRouteActivationV1::InboundOnly,
            next: SccpRouteActivationV1::Retired,
            inbound_finality_cutoff: Some(cutoff),
        });
        assert!(valid.validate_static().is_ok());

        for inbound_finality_cutoff in [
            None,
            Some(crate::bridge::SccpInboundFinalityCutoffV1 {
                trust_anchor_hash: [0; 32],
                ..cutoff
            }),
            Some(crate::bridge::SccpInboundFinalityCutoffV1 {
                max_anchor_interval_height: 0,
                ..cutoff
            }),
        ] {
            let invalid = SccpRouteGovernanceActionV1::SetActivation(SccpSetRouteActivationV1 {
                key: key(1),
                expected_current: SccpRouteActivationV1::InboundOnly,
                next: SccpRouteActivationV1::Retired,
                inbound_finality_cutoff,
            });
            assert_eq!(
                invalid.validate_static(),
                Err(SccpRouteValidationError::InvalidActivationTransition)
            );
        }

        let cutoff_on_live = SccpRouteGovernanceActionV1::SetActivation(SccpSetRouteActivationV1 {
            key: key(1),
            expected_current: SccpRouteActivationV1::Paused,
            next: SccpRouteActivationV1::InboundOnly,
            inbound_finality_cutoff: Some(cutoff),
        });
        assert_eq!(
            cutoff_on_live.validate_static(),
            Err(SccpRouteValidationError::InvalidActivationTransition)
        );
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for RecordSccpMessage {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let context = super::decode_aos_slice_field::<crate::bridge::SccpOutboundMessageContextV1>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let payload_bytes = super::decode_aos_slice_field::<Vec<u8>>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                context,
                payload_bytes,
            },
            offset,
        ))
    }
}

#[cfg(test)]
mod tests {
    use norito::core::DecodeFromSlice;

    use super::*;
    use crate::{
        bridge::{
            BridgeProof, BridgeProofPayload, BridgeProofRange, BridgeReceipt,
            BridgeTransparentProof, SccpLaneIdV1, SccpNetworkV1, SccpOutboundMessageContextV1,
        },
        nexus::LaneId,
        proof::ProofBox,
    };

    fn proof() -> BridgeProof {
        BridgeProof {
            range: BridgeProofRange {
                start_height: 7,
                end_height: 9,
            },
            payload: BridgeProofPayload::TransparentZk(BridgeTransparentProof {
                verifier_manifest_hash: [0xAB; 32],
                proof: ProofBox::new("halo2/mock".into(), vec![0xDE, 0xAD, 0xBE, 0xEF]),
                recursion_depth: Some(2),
            }),
        }
    }

    fn receipt() -> BridgeReceipt {
        BridgeReceipt {
            lane: LaneId::from(1),
            direction: b"mint".to_vec(),
            source_tx: [0x11; 32],
            dest_tx: Some([0x22; 32]),
            proof_hash: [0x33; 32],
            amount: 42_u64.into(),
            asset_id: b"wBTC#btc".to_vec(),
            recipient: b"alice@main".to_vec(),
        }
    }

    fn outbound_context() -> SccpOutboundMessageContextV1 {
        SccpOutboundMessageContextV1::new(
            SccpLaneIdV1 {
                source: SccpNetworkV1::SoraTaira,
                target: SccpNetworkV1::BscTestnet,
            },
            [0x44; 32],
            [0x45; 32],
        )
        .expect("valid outbound context")
    }

    fn route_governance_action() -> SccpRouteGovernanceActionV1 {
        SccpRouteGovernanceActionV1::Remove(crate::bridge::SccpRouteKeyV1 {
            lane_id: SccpLaneIdV1 {
                source: SccpNetworkV1::BscTestnet,
                target: SccpNetworkV1::SoraTaira,
            },
            route_id: "taira_bsc_xor".to_owned(),
            asset_key: "xor".to_owned(),
            revision: 1,
        })
    }

    fn assert_slice_roundtrip<T>(value: T)
    where
        T: Clone + PartialEq + core::fmt::Debug + norito::codec::Encode,
        for<'a> T: DecodeFromSlice<'a>,
    {
        let bytes = value.encode();
        let (decoded, used) = T::decode_from_slice(&bytes).expect("decode from slice");
        assert_eq!(used, bytes.len());
        assert_eq!(decoded, value);
    }

    fn assert_registry_decodes<T>(registry: &crate::isi::InstructionRegistry, value: T)
    where
        T: crate::isi::Instruction
            + norito::codec::Encode
            + 'static
            + norito::core::NoritoSerialize,
        for<'de> T: norito::core::NoritoDeserialize<'de>,
    {
        let wire_id = std::any::type_name::<T>();
        let (payload, flags) = norito::codec::encode_with_header_flags(&value);
        let framed =
            norito::core::frame_bare_with_header_flags::<T>(&payload, flags).expect("frame");
        let decoded = crate::isi::InstructionRegistry::decode(registry, wire_id, &framed)
            .expect("registered")
            .expect("decode");
        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
    }

    #[test]
    fn bridge_decode_from_slice_roundtrips() {
        assert_slice_roundtrip(SubmitBridgeProof::new(proof()));
        assert_slice_roundtrip(RecordBridgeReceipt::new(receipt()));
        assert_slice_roundtrip(ApplySccpRouteGovernance::new(route_governance_action()));
        assert_slice_roundtrip(RecordSccpMessage::new(outbound_context(), vec![0xCA, 0xFE]));
    }

    #[test]
    fn bridge_registry_decodes_type_names() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_slice::<SubmitBridgeProof>()
            .register_slice::<RecordBridgeReceipt>()
            .register_slice::<ApplySccpRouteGovernance>()
            .register_slice::<RecordSccpMessage>();

        assert_registry_decodes(&registry, SubmitBridgeProof::new(proof()));
        assert_registry_decodes(&registry, RecordBridgeReceipt::new(receipt()));
        assert_registry_decodes(
            &registry,
            ApplySccpRouteGovernance::new(route_governance_action()),
        );
        assert_registry_decodes(
            &registry,
            RecordSccpMessage::new(outbound_context(), vec![0xCA, 0xFE]),
        );
    }

    #[cfg(feature = "json")]
    #[test]
    fn record_bridge_receipt_json_rejects_unknown_instruction_fields() {
        let instruction = RecordBridgeReceipt::new(receipt());
        let canonical =
            norito::json::to_json(&instruction).expect("serialize bridge receipt instruction JSON");
        assert_eq!(
            norito::json::from_json::<RecordBridgeReceipt>(&canonical)
                .expect("canonical bridge receipt instruction JSON decodes"),
            instruction
        );

        let hostile = canonical.replacen('{', "{\"adversarial_extension\":null,", 1);
        assert_ne!(hostile, canonical);
        assert!(
            norito::json::from_json::<RecordBridgeReceipt>(&hostile).is_err(),
            "signed receipt instruction JSON must reject unknown fields"
        );
    }
}
