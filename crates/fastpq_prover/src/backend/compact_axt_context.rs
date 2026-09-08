//! Public AXT transfer context for the unadmitted compact protocol prototype.
//!
//! The complete canonical binding, exact pre-proof metadata mirrors and remote
//! spend preimages wrap the typed public-transfer statement before challenges.
//! Legacy and compact callers share the same public-fact acceptance predicates.
//! This module accepts no legacy batch, private SMT witness, or arbitrary context.
//!
//! `AxtProofEnvelope::amount_commitment` is deliberately a post-proof mirror:
//! `ivm_abi::axt` derives it from the completed envelope with that field cleared,
//! including its proof bytes. Including it here would make proving circular.
//! The outer ABI must still validate that commitment, exact clear/hidden amount
//! resolution, expiry at use, handle signatures/replay, and source finality.
//! Committed-amount validation here preserves the existing scalar metadata rule;
//! it does not invent a conversion or aggregation of public transfer quantities.
//!
//! TODO: Wire these bytes into a distinct reviewed compact AXT relation and the
//! authenticated caller API. Encoding does not admit a proof or remove replay,
//! and touched-balance roots do not establish finalized source-state authority.

use iroha_data_model::nexus::{AxtFastpqBinding, AxtRemoteSpendClaimV1};
#[cfg(test)]
use norito::{NoritoSerialize, codec::Encode};

use super::compact_value_domain::CompactTransferValue;
#[cfg(test)]
use super::{compact_protocol::FixedAir, compact_public_transfer::PublicTransferAir};
use crate::{
    Error, Result, VerifyLimits,
    axt_binding::AxtPublicMetadataBytes,
    gadgets::public_transfer_statement::{PreparedPublicTransfers, PublicTransferLimits},
};

#[cfg(test)]
use crate::{
    axt_binding::{
        AxtProofContextMirrors, validate_axt_public_metadata, validate_axt_public_transfer_facts,
    },
    proof::PublicIO,
};

#[derive(NoritoSerialize)]
#[norito(schema_name = "fastpq_prover::compact_prototype::AxtTransferContextV1")]
#[cfg(test)]
struct BoundContext {
    version: u16,
    public_transfer_context: Vec<u8>,
    binding: AxtFastpqBinding,
    metadata: AxtProofContextMirrors,
    remote_spend_claims: Option<Vec<AxtRemoteSpendClaimV1>>,
}

/// Encode a bounded, canonical public statement without accepting any proof.
///
/// `expected` and the outer binding/mirrors must come from the authenticated
/// surrounding caller. Values copied from an untrusted proof do not authorize a
/// source root or spend. The prepared table must select AXT transfer semantics
/// and contain exactly one delta; its original public claims and all seven
/// PublicIO fields are bound by `PublicTransferAir` before this wrapper is built.
#[cfg(test)]
pub(super) fn encode_context<V: CompactTransferValue>(
    prepared: &PreparedPublicTransfers<'_, V>,
    expected: &PublicIO,
    binding: &AxtFastpqBinding,
    metadata: AxtPublicMetadataBytes<'_>,
    outer: AxtProofContextMirrors,
    remote_spend_claims: Option<&[AxtRemoteSpendClaimV1]>,
) -> Result<Vec<u8>> {
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let mut public_bytes = preflight_context(prepared, binding, metadata, remote_spend_claims)?;
    let max_bytes = VerifyLimits::default().max_batch_bytes;
    let transfer = PublicTransferAir::new(prepared, expected)?;
    public_bytes = checked_sum(public_bytes, transfer.statement_bytes().len())?;
    check_limit(public_bytes, max_bytes)?;
    validate_axt_public_transfer_facts(binding, metadata, prepared, remote_spend_claims)?;
    validate_axt_public_metadata(binding, metadata, outer)?;
    let context = BoundContext {
        version: 1,
        public_transfer_context: transfer.statement_bytes().to_vec(),
        binding: binding.clone(),
        metadata: outer,
        remote_spend_claims: remote_spend_claims.map(<[AxtRemoteSpendClaimV1]>::to_vec),
    };
    check_limit(norito::core::encoded_frame_len(&context)?, max_bytes)?;
    Ok(context.encode())
}

/// Count the complete public AXT inputs before cloning or commitment validation.
///
/// This is only the shared resource preflight. Each typed constructor still
/// validates the complete prepared public facts and exact outer mirrors itself.
pub(super) fn preflight_context<V: CompactTransferValue>(
    prepared: &PreparedPublicTransfers<'_, V>,
    binding: &AxtFastpqBinding,
    metadata: AxtPublicMetadataBytes<'_>,
    remote_spend_claims: Option<&[AxtRemoteSpendClaimV1]>,
) -> Result<usize> {
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let limits = PublicTransferLimits::default();
    let max_bytes = VerifyLimits::default().max_batch_bytes;
    check_limit(prepared.work().public_bytes, limits.max_public_bytes)?;
    if let Some(claims) = remote_spend_claims {
        if claims.len() > limits.max_deltas {
            return Err(Error::VerifierLimitExceeded {
                limit: "max_compact_axt_remote_claims",
                actual: claims.len(),
                max: limits.max_deltas,
            });
        }
    }
    // Bound variable-count containers in O(1) before the counting serializer
    // walks their elements. These are conservative raw payload lower bounds;
    // the exact canonical framed count remains mandatory below.
    check_limit(binding.target_dsids.len().saturating_mul(8), max_bytes)?;
    check_limit(
        binding
            .remote_spend_intent_commitments
            .len()
            .saturating_mul(32),
        max_bytes,
    )?;
    // Count borrowed values before canonicalization clones binding strings or
    // commitment validation hashes remote preimages. Only public bytes enter
    // these lengths; no metadata map or witness-bearing transcript is accepted.
    let mut public_bytes = norito::core::encoded_frame_len(binding)?;
    check_limit(public_bytes, max_bytes)?;
    for claim in remote_spend_claims.into_iter().flatten() {
        public_bytes = checked_sum(public_bytes, norito::core::encoded_frame_len(claim)?)?;
        check_limit(public_bytes, max_bytes)?;
    }
    for bytes in [
        metadata.parameter.as_bytes(),
        metadata.entry_hash,
        metadata.committed_amount.unwrap_or(&[]),
        metadata.expiry_slot,
        metadata.manifest_root,
        metadata.da_commitment,
    ] {
        public_bytes = checked_sum(public_bytes, bytes.len())?;
        check_limit(public_bytes, max_bytes)?;
    }
    Ok(public_bytes)
}

fn checked_sum(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right).ok_or(Error::VerifierLimitExceeded {
        limit: "max_compact_axt_context_bytes",
        actual: usize::MAX,
        max: VerifyLimits::default().max_batch_bytes,
    })
}

fn check_limit(actual: usize, max: usize) -> Result<()> {
    if actual > max {
        Err(Error::VerifierLimitExceeded {
            limit: "max_compact_axt_context_bytes",
            actual,
            max,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::{
        OperationKind, ProofSemantics, PublicInputs, StateTransition,
        axt_binding::DEFAULT_PARAMETER,
        gadgets::public_transfer_statement::{
            PublicTransferDelta, PublicTransferTranscript, prepare_public_transfers,
        },
    };
    use iroha_crypto::Hash;
    use iroha_data_model::{
        DataSpaceId, DomainId,
        asset::id::AssetDefinitionId,
        nexus::{
            AxtEffectBinding, AxtHandleIssuerContextV1, AxtHandleReplayKey, LaneId,
            compute_remote_spend_claim_commitment_v1,
        },
    };
    use iroha_primitives::numeric::Quantity;
    use iroha_test_samples::{ALICE_ID, BOB_ID};
    use iroha_zkp_halo2::poseidon::PoseidonByteHasher;

    #[derive(Clone)]
    pub(in crate::backend) struct Fixture {
        rows: Vec<StateTransition>,
        claims: Vec<PublicTransferTranscript>,
        inputs: PublicInputs,
        pub(in crate::backend) binding: AxtFastpqBinding,
        entry: [u8; 32],
        amount: Option<[u8; 16]>,
        expiry: [u8; 8],
        manifest: [u8; 32],
        da: [u8; 33],
        pub(in crate::backend) outer: AxtProofContextMirrors,
        pub(in crate::backend) remote: Option<Vec<AxtRemoteSpendClaimV1>>,
    }

    impl Fixture {
        pub(in crate::backend) fn new(remote: bool) -> Self {
            let delta = PublicTransferDelta {
                from_account: (*ALICE_ID).clone(),
                to_account: (*BOB_ID).clone(),
                asset_definition: AssetDefinitionId::derive_from_components(
                    DomainId::try_new("wonderland", "universal").unwrap(),
                    "rose".parse().unwrap(),
                ),
                amount: Quantity::from(35_u64),
                from_balance_before: Quantity::from(100_u64),
                from_balance_after: Quantity::from(65_u64),
                to_balance_before: Quantity::from(200_u64),
                to_balance_after: Quantity::from(235_u64),
            };
            let batch_hash = Hash::new(b"compact AXT public entry");
            // Fixture digest uses only its original public preimage fields.
            let mut hasher = PoseidonByteHasher::new();
            delta.from_account.encode_to(&mut hasher);
            delta.to_account.encode_to(&mut hasher);
            delta.asset_definition.encode_to(&mut hasher);
            delta.amount.encode_to(&mut hasher);
            hasher.update(batch_hash.as_ref());
            let transcript = PublicTransferTranscript {
                batch_hash,
                authority_digest: Hash::new(b"caller-authenticated execution authority"),
                poseidon_preimage_digest: Some(Hash::prehashed(hasher.finalize())),
                deltas: vec![delta.clone()],
            };
            let mut rows: Vec<_> = [
                (&delta.from_account, 100_u64, 65_u64),
                (&delta.to_account, 200_u64, 235_u64),
            ]
            .into_iter()
            .map(|(account, before, after)| {
                StateTransition::new(
                    iroha_data_model::fastpq::transfer_balance_key(
                        &delta.asset_definition,
                        account,
                    )
                    .unwrap(),
                    before.to_le_bytes().to_vec(),
                    after.to_le_bytes().to_vec(),
                    OperationKind::Transfer,
                )
            })
            .collect();
            rows.sort_by(|left, right| left.key.cmp(&right.key));
            let mut dsid = [0; 16];
            dsid[..8].copy_from_slice(&7_u64.to_le_bytes());
            let binding = AxtFastpqBinding {
                parameter: DEFAULT_PARAMETER.into(),
                source_dsid: 7,
                source_dataspace: "source".into(),
                source_receipt_id: "receipt-1".into(),
                source_tx_commitment: hex::encode(batch_hash.as_ref()),
                claim_type: "tx_predicate".into(),
                claim_digest: hex::encode([2; 32]),
                witness_commitment: hex::encode([3; 32]),
                policy_commitment: hex::encode([4; 32]),
                verified_effect_type: "transfer".into(),
                corridor: "corridor".into(),
                verifier_id: "fastpq".into(),
                verifier_version: "v1".into(),
                target_dsids: vec![9],
                effect_binding: Some(AxtEffectBinding {
                    destination_domain: None,
                    destination_account_id: None,
                    vault_account_id: None,
                    issuance_account_id: None,
                    source_asset_definition_id: Some(delta.asset_definition.to_string()),
                    destination_asset_definition_id: None,
                    source_amount_i64: None,
                    destination_amount_i64: None,
                }),
                remote_spend_intent_commitments: vec![],
            };
            let mut fixture = Self {
                rows,
                claims: vec![transcript],
                inputs: PublicInputs {
                    dsid,
                    slot: 123,
                    old_root: Hash::new(b"caller expected old touched-balance root").into(),
                    new_root: Hash::new(b"caller expected new touched-balance root").into(),
                    perm_root: Hash::new(b"caller expected permission context").into(),
                    tx_set_hash: Hash::new(b"caller expected transaction set").into(),
                },
                binding,
                entry: batch_hash.into(),
                amount: Some(35_u128.to_le_bytes()),
                expiry: 456_u64.to_le_bytes(),
                manifest: [5; 32],
                da: core::array::from_fn(|index| if index == 0 { 1 } else { 6 }),
                outer: AxtProofContextMirrors {
                    dsid: DataSpaceId::new(7),
                    manifest_root: [5; 32],
                    da_commitment: Some([6; 32]),
                    committed_amount: Some(35),
                    expiry_slot: Some(456),
                },
                remote: None,
            };
            if remote {
                fixture.remote = Some(vec![AxtRemoteSpendClaimV1::new(
                    AxtHandleReplayKey::from_parts(
                        DataSpaceId::new(7),
                        AxtHandleIssuerContextV1::default().asset_definition_incarnation,
                        [8; 32],
                        1,
                        1,
                        LaneId::new(0),
                    ),
                    delta.asset_definition,
                    "transfer",
                    delta.from_account.to_string(),
                    delta.to_account.to_string(),
                    delta.amount,
                )]);
                fixture.recommit_remote();
            }
            fixture
        }

        /// Build repeated public transfer occurrences with complete remote facts.
        ///
        /// All transcript entries bind the same source transaction, while each
        /// debit/credit pair has its actual chronological balances. The remote
        /// transfer facts are equal but have distinct handle replay keys, making
        /// omission and duplicate-occurrence accounting observable in tests.
        pub(in crate::backend) fn multiple(count: usize, remote: bool) -> Self {
            assert!((1..=3).contains(&count));
            let mut fixture = Self::new(false);
            let original = fixture.claims.pop().unwrap();
            let template = original.deltas[0].clone();
            fixture.rows.clear();
            let mut remote_claims = Vec::new();
            for ordinal in 0..count {
                let mut delta = template.clone();
                let from_before = 100 - 5 * ordinal as u64;
                let to_before = 200 + 5 * ordinal as u64;
                delta.amount = Quantity::from(5_u64);
                delta.from_balance_before = Quantity::from(from_before);
                delta.from_balance_after = Quantity::from(from_before - 5);
                delta.to_balance_before = Quantity::from(to_before);
                delta.to_balance_after = Quantity::from(to_before + 5);
                let mut hasher = PoseidonByteHasher::new();
                delta.from_account.encode_to(&mut hasher);
                delta.to_account.encode_to(&mut hasher);
                delta.asset_definition.encode_to(&mut hasher);
                delta.amount.encode_to(&mut hasher);
                hasher.update(original.batch_hash.as_ref());
                for (account, before, after) in [
                    (&delta.from_account, from_before, from_before - 5),
                    (&delta.to_account, to_before, to_before + 5),
                ] {
                    fixture.rows.push(StateTransition::new(
                        iroha_data_model::fastpq::transfer_balance_key(
                            &delta.asset_definition,
                            account,
                        )
                        .unwrap(),
                        before.to_le_bytes().to_vec(),
                        after.to_le_bytes().to_vec(),
                        OperationKind::Transfer,
                    ));
                }
                if remote {
                    remote_claims.push(AxtRemoteSpendClaimV1::new(
                        AxtHandleReplayKey::from_parts(
                            DataSpaceId::new(7),
                            AxtHandleIssuerContextV1::default().asset_definition_incarnation,
                            [8; 32],
                            ordinal as u64 + 1,
                            1,
                            LaneId::new(0),
                        ),
                        delta.asset_definition.clone(),
                        "transfer",
                        delta.from_account.to_string(),
                        delta.to_account.to_string(),
                        delta.amount.clone(),
                    ));
                }
                fixture.claims.push(PublicTransferTranscript {
                    batch_hash: original.batch_hash,
                    authority_digest: original.authority_digest,
                    deltas: vec![delta],
                    poseidon_preimage_digest: Some(Hash::prehashed(hasher.finalize())),
                });
            }
            fixture.rows.sort_by(|left, right| left.key.cmp(&right.key));
            fixture.amount = Some((count as u128 * 5).to_le_bytes());
            fixture.outer.committed_amount = Some(count as u128 * 5);
            if remote {
                fixture.remote = Some(remote_claims);
                fixture.recommit_remote();
            }
            fixture
        }

        /// Replace only the deterministic test fixture's public touched roots.
        ///
        /// Full proof diagnostics compute these from genuine prover witnesses;
        /// this test helper does not grant source-state authority or finality.
        pub(in crate::backend) fn set_touched_roots(
            &mut self,
            old_root: [u8; 32],
            new_root: [u8; 32],
        ) {
            self.inputs.old_root = old_root;
            self.inputs.new_root = new_root;
        }

        pub(in crate::backend) fn metadata(&self) -> AxtPublicMetadataBytes<'_> {
            AxtPublicMetadataBytes {
                parameter: DEFAULT_PARAMETER,
                entry_hash: &self.entry,
                committed_amount: self.amount.as_ref().map(|bytes| &bytes[..]),
                expiry_slot: &self.expiry,
                manifest_root: &self.manifest,
                da_commitment: &self.da,
            }
        }

        pub(in crate::backend) fn prepare(
            &self,
            semantics: ProofSemantics,
        ) -> PreparedPublicTransfers<'_> {
            prepare_public_transfers(
                &self.rows,
                &self.claims,
                self.inputs.clone(),
                semantics,
                PublicTransferLimits::default(),
            )
            .unwrap()
        }

        pub(in crate::backend) fn expected(
            &self,
            prepared: &PreparedPublicTransfers<'_>,
        ) -> PublicIO {
            PublicIO {
                dsid: self.inputs.dsid,
                slot: self.inputs.slot,
                old_root: self.inputs.old_root,
                new_root: self.inputs.new_root,
                perm_root: self.inputs.perm_root,
                tx_set_hash: self.inputs.tx_set_hash,
                ordering_hash: prepared.ordering_hash().into(),
            }
        }

        fn encode(&self) -> Result<Vec<u8>> {
            let prepared = self.prepare(ProofSemantics::AxtTransferClaim);
            encode_context(
                &prepared,
                &self.expected(&prepared),
                &self.binding,
                self.metadata(),
                self.outer,
                self.remote.as_deref(),
            )
        }

        fn recommit_remote(&mut self) {
            if let Some(claims) = &mut self.remote {
                claims.sort_by_key(compute_remote_spend_claim_commitment_v1);
                self.binding.remote_spend_intent_commitments = claims
                    .iter()
                    .map(compute_remote_spend_claim_commitment_v1)
                    .collect();
            }
        }
    }

    #[test]
    fn extracted_preflight_preserves_the_original_one_delta_axt_envelope() {
        for remote in [false, true] {
            let fixture = Fixture::new(remote);
            let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
            let _flags =
                norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
            let mut original_count = norito::core::encoded_frame_len(&fixture.binding).unwrap();
            for claim in fixture.remote.iter().flatten() {
                original_count += norito::core::encoded_frame_len(claim).unwrap();
            }
            let metadata = fixture.metadata();
            for bytes in [
                metadata.parameter.as_bytes(),
                metadata.entry_hash,
                metadata.committed_amount.unwrap_or(&[]),
                metadata.expiry_slot,
                metadata.manifest_root,
                metadata.da_commitment,
            ] {
                original_count += bytes.len();
            }
            assert_eq!(
                preflight_context(
                    &prepared,
                    &fixture.binding,
                    metadata,
                    fixture.remote.as_deref()
                )
                .unwrap(),
                original_count
            );
            let transfer = PublicTransferAir::new(&prepared, &fixture.expected(&prepared)).unwrap();
            let original = BoundContext {
                version: 1,
                public_transfer_context: transfer.statement_bytes().to_vec(),
                binding: fixture.binding.clone(),
                metadata: fixture.outer,
                remote_spend_claims: fixture.remote.clone(),
            }
            .encode();
            assert_eq!(fixture.encode().unwrap(), original);
        }
    }

    #[test]
    fn context_is_canonical_bounded_and_ambient_codec_independent() {
        let fixture = Fixture::new(true);
        let bytes = fixture.encode().unwrap();
        assert!(bytes.len() < VerifyLimits::default().max_batch_bytes);
        let flags = norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(fixture.encode().unwrap(), bytes);
        let without_remote = Fixture::new(false).encode().unwrap();
        assert_ne!(without_remote, bytes);
        assert_eq!(checked_sum(4, 5).unwrap(), 9);
        assert!(checked_sum(usize::MAX, 1).is_err());
        assert!(check_limit(9, 9).is_ok());
        assert!(check_limit(10, 9).is_err());
    }

    #[test]
    fn every_binding_field_is_bound_or_rejected() {
        let fixture = Fixture::new(false);
        let expected = fixture.encode().unwrap();
        for field in 0..23 {
            let mut changed = fixture.clone();
            let binding = &mut changed.binding;
            match field {
                0 => binding.parameter.push('x'),
                1 => binding.source_dsid += 1,
                2 => binding.source_dataspace.push('x'),
                3 => binding.source_receipt_id.push('x'),
                4 => binding.source_tx_commitment = hex::encode([99; 32]),
                5 => binding.claim_type = "value_conservation".into(),
                6 => binding.claim_digest = hex::encode([99; 32]),
                7 => binding.witness_commitment = hex::encode([99; 32]),
                8 => binding.policy_commitment = hex::encode([99; 32]),
                9 => binding.verified_effect_type.push('x'),
                10 => binding.corridor.push('x'),
                11 => binding.verifier_id.push('x'),
                12 => binding.verifier_version.push('x'),
                13 => binding.target_dsids.push(10),
                14 => binding.effect_binding.as_mut().unwrap().source_amount_i64 = Some(35),
                15 => {
                    binding
                        .effect_binding
                        .as_mut()
                        .unwrap()
                        .destination_amount_i64 = Some(35)
                }
                16 => {
                    binding.effect_binding.as_mut().unwrap().destination_domain =
                        Some("other".into())
                }
                17 => binding.remote_spend_intent_commitments = vec![[42; 32]],
                18 => {
                    binding
                        .effect_binding
                        .as_mut()
                        .unwrap()
                        .destination_account_id = Some("destination".into())
                }
                19 => {
                    binding.effect_binding.as_mut().unwrap().vault_account_id = Some("vault".into())
                }
                20 => {
                    binding.effect_binding.as_mut().unwrap().issuance_account_id =
                        Some("issuer".into())
                }
                21 => {
                    binding
                        .effect_binding
                        .as_mut()
                        .unwrap()
                        .source_asset_definition_id = Some("source-asset".into())
                }
                22 => {
                    binding
                        .effect_binding
                        .as_mut()
                        .unwrap()
                        .destination_asset_definition_id = Some("destination-asset".into())
                }
                _ => unreachable!(),
            }
            if let Ok(actual) = changed.encode() {
                assert_ne!(actual, expected, "unbound binding field {field}");
            }
        }
        let mut noncanonical = fixture.clone();
        noncanonical.binding.source_dataspace.push(' ');
        assert!(noncanonical.encode().is_err());
        let mut opaque = fixture;
        opaque.binding.claim_type = "authorization".into();
        assert!(matches!(
            opaque.encode(),
            Err(Error::InvalidProofSemantics { .. })
        ));
    }

    #[test]
    fn matched_outer_mirror_changes_change_the_bound_context() {
        let fixture = Fixture::new(false);
        let bytes = fixture.encode().unwrap();
        for field in 0..5 {
            let mut changed = fixture.clone();
            match field {
                0 => {
                    changed.binding.source_dsid = 8;
                    changed.inputs.dsid[..8].copy_from_slice(&8_u64.to_le_bytes());
                    changed.outer.dsid = DataSpaceId::new(8);
                }
                1 => {
                    changed.manifest[0] ^= 1;
                    changed.outer.manifest_root = changed.manifest;
                }
                2 => {
                    changed.da = [0; 33];
                    changed.outer.da_commitment = None;
                }
                3 => {
                    changed.amount = None;
                    changed.outer.committed_amount = None;
                }
                4 => {
                    changed.expiry = u64::MAX.to_le_bytes();
                    changed.outer.expiry_slot = Some(u64::MAX);
                }
                _ => unreachable!(),
            }
            assert_ne!(
                changed.encode().unwrap(),
                bytes,
                "unbound outer field {field}"
            );
        }
    }

    #[test]
    fn public_io_profile_and_entry_are_exact() {
        let fixture = Fixture::new(false);
        let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
        for field in 0..7 {
            let mut expected = fixture.expected(&prepared);
            match field {
                0 => expected.dsid[15] ^= 1,
                1 => expected.slot ^= 1,
                2 => expected.old_root[0] ^= 1,
                3 => expected.new_root[0] ^= 1,
                4 => expected.perm_root[0] ^= 1,
                5 => expected.tx_set_hash[0] ^= 1,
                6 => expected.ordering_hash[0] ^= 1,
                _ => unreachable!(),
            }
            assert!(matches!(
                encode_context(
                    &prepared,
                    &expected,
                    &fixture.binding,
                    fixture.metadata(),
                    fixture.outer,
                    None
                ),
                Err(Error::PublicIoMismatch { .. })
            ));
        }
        let generic = fixture.prepare(ProofSemantics::StateTransition);
        assert!(matches!(
            encode_context(
                &generic,
                &fixture.expected(&generic),
                &fixture.binding,
                fixture.metadata(),
                fixture.outer,
                None
            ),
            Err(Error::InvalidProofSemantics { .. })
        ));
        let mut changed = fixture.clone();
        changed.entry[0] ^= 1;
        assert!(changed.encode().is_err());
        let mut changed = fixture;
        changed.inputs.dsid[15] = 1;
        assert!(changed.encode().is_err());
    }

    #[test]
    fn metadata_mirrors_boundaries_and_encoding_are_exact() {
        let fixture = Fixture::new(false);
        for field in 0..5 {
            let mut changed = fixture.clone();
            match field {
                0 => changed.outer.dsid = DataSpaceId::new(8),
                1 => changed.outer.manifest_root[0] ^= 1,
                2 => changed.outer.da_commitment = None,
                3 => changed.outer.committed_amount = None,
                4 => changed.outer.expiry_slot = None,
                _ => unreachable!(),
            }
            assert!(changed.encode().is_err());
        }
        for value in [1, u128::MAX] {
            let mut changed = fixture.clone();
            changed.amount = Some(value.to_le_bytes());
            changed.outer.committed_amount = Some(value);
            assert!(changed.encode().is_ok());
        }
        let mut absent = fixture.clone();
        absent.amount = None;
        absent.outer.committed_amount = None;
        absent.expiry = [0; 8];
        absent.outer.expiry_slot = None;
        absent.da = [0; 33];
        absent.outer.da_commitment = None;
        assert!(absent.encode().is_ok());
        absent.outer.expiry_slot = Some(0);
        assert!(absent.encode().is_err());
        let mut zero = fixture.clone();
        zero.amount = Some([0; 16]);
        zero.outer.committed_amount = Some(0);
        assert!(zero.encode().is_err());
        zero = fixture.clone();
        zero.manifest = [0; 32];
        zero.outer.manifest_root = [0; 32];
        assert!(zero.encode().is_err());
        for tag in [0, 2, 255] {
            let mut changed = fixture.clone();
            changed.da[0] = tag;
            assert!(changed.encode().is_err());
        }
        for field in 0..4 {
            let mut metadata = fixture.metadata();
            match field {
                0 => metadata.committed_amount = Some(&[0; 15]),
                1 => metadata.expiry_slot = &[0; 7],
                2 => metadata.manifest_root = &[0; 31],
                3 => metadata.da_commitment = &[0; 32],
                _ => unreachable!(),
            }
            assert!(matches!(
                validate_axt_public_metadata(&fixture.binding, metadata, fixture.outer),
                Err(Error::MetadataLength { .. })
            ));
        }
    }

    #[test]
    fn remote_preimages_match_exact_accounts_asset_amount_and_occurrence() {
        let fixture = Fixture::new(true);
        fixture.encode().unwrap();
        for field in 0..7 {
            let mut changed = fixture.clone();
            let claim = &mut changed.remote.as_mut().unwrap()[0];
            match field {
                0 => claim.effective_amount = Quantity::from(34_u64),
                1 => claim.from = (*BOB_ID).to_string(),
                2 => claim.to = (*ALICE_ID).to_string(),
                3 => claim.kind = "mint".into(),
                4 => claim.from.push(' '),
                5 => claim.handle_replay_key.asset_dsid = DataSpaceId::new(8),
                6 => {
                    claim.asset_definition_id = AssetDefinitionId::derive_from_components(
                        DomainId::try_new("wonderland", "universal").unwrap(),
                        "lily".parse().unwrap(),
                    )
                }
                _ => unreachable!(),
            }
            changed.recommit_remote();
            assert!(changed.encode().is_err(), "remote mutation {field}");
        }
        let mut duplicate = fixture.clone();
        let mut second = duplicate.remote.as_ref().unwrap()[0].clone();
        second.handle_replay_key = AxtHandleReplayKey::from_parts(
            DataSpaceId::new(7),
            AxtHandleIssuerContextV1::default().asset_definition_incarnation,
            [9; 32],
            1,
            1,
            LaneId::new(0),
        );
        duplicate.remote.as_mut().unwrap().push(second);
        duplicate.recommit_remote();
        assert!(duplicate.encode().is_err());
        let mut missing = fixture.clone();
        missing.remote = None;
        assert!(matches!(
            missing.encode(),
            Err(Error::MissingMetadata { .. })
        ));
        let mut stale = fixture;
        stale.remote.as_mut().unwrap()[0].effective_amount = Quantity::from(34_u64);
        assert!(stale.encode().is_err());
        let mut empty = Fixture::new(false);
        empty.remote = Some(vec![]);
        assert!(empty.encode().is_err());
    }

    #[test]
    fn resource_preflight_precedes_canonicalization_and_preimage_hashing() {
        for target_dsids in [true, false] {
            let mut oversized = Fixture::new(false);
            if target_dsids {
                oversized.binding.target_dsids =
                    vec![9; VerifyLimits::default().max_batch_bytes / 8 + 1];
            } else {
                oversized.binding.remote_spend_intent_commitments =
                    vec![[0; 32]; VerifyLimits::default().max_batch_bytes / 32 + 1];
            }
            // Both vectors deliberately violate canonical binding rules too;
            // raw-count admission must win before walking or validating them.
            assert!(matches!(
                oversized.encode(),
                Err(Error::VerifierLimitExceeded {
                    limit: "max_compact_axt_context_bytes",
                    ..
                })
            ));
        }
        let mut oversized = Fixture::new(false);
        oversized.binding.source_dataspace =
            " ".repeat(VerifyLimits::default().max_batch_bytes + 1);
        assert!(matches!(
            oversized.encode(),
            Err(Error::VerifierLimitExceeded { .. })
        ));
        let mut oversized = Fixture::new(true);
        let remote = oversized.remote.as_ref().unwrap()[0].clone();
        oversized.remote = Some(vec![remote; PublicTransferLimits::default().max_deltas + 1]);
        assert!(matches!(
            oversized.encode(),
            Err(Error::VerifierLimitExceeded {
                limit: "max_compact_axt_remote_claims",
                ..
            })
        ));
    }
}
