//! Frozen validator elections and per-seat preparation evidence.
//!
//! A preparation selects an immutable committee two scheduling epochs ahead.
//! Its terminal disposition is authenticated by incumbent boundary finality.

use crate::{
    NetworkId,
    block::{
        BlockHeader,
        consensus_v2::{DualQuorum, ValidatorPower},
    },
    consensus::GlobalThresholdBeaconPartialSignatureV1,
    isi::kagemusha_v1::{
        BeaconEpochBindingV1, InstalledBeaconEpochBindingV1,
        KagemushaMintFinalityAuthorityGenerationV1, KagemushaMintFinalityEpochAuthorizationV1,
        KagemushaMintFinalityEpochDecisionV1, KagemushaMintFinalityPairedPossessionProofV1,
        KagemushaMintFinalitySeatReadinessContextV1, KagemushaMintFinalityValidatorKeysV1,
    },
    parameter::{CustomParameter, CustomParameterId},
};
use iroha_crypto::{Hash, HashOf, SignatureOf};
use iroha_primitives::json::Json;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

/// Candidate-owned public keys for one network and signing generation.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::nexus::ValidatorCandidateKeysV1")]
#[norito(deny_unknown_fields)]
pub struct ValidatorCandidateKeysV1 {
    /// Exact genesis-derived network identity.
    pub network_id: NetworkId,
    /// Generation to which both public keys belong.
    pub generation: u64,
    /// Consensus peer and both generation-derived Pasta public keys.
    pub keys: KagemushaMintFinalityValidatorKeysV1,
    /// Proof of actual possession under the candidate-publication challenge.
    pub possession: KagemushaMintFinalityPairedPossessionProofV1,
    /// Consent signed by the exact consensus peer over the complete publication challenge.
    pub peer_signature: SignatureOf<ValidatorCandidateKeyAuthorizationV1>,
}

/// Domain-separated consensus-peer consent to one exact candidate key publication.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::nexus::ValidatorCandidateKeyAuthorizationV1")]
#[norito(deny_unknown_fields)]
pub struct ValidatorCandidateKeyAuthorizationV1 {
    /// Fixed protocol domain; consumers reconstruct this challenge from the candidate.
    domain: String,
    /// Exact network whose future authority may consume these keys.
    pub network_id: NetworkId,
    /// Authenticated signing generation, independent of scheduling epoch.
    pub generation: u64,
    /// Exact consensus identity and paired Pasta public keys.
    pub keys: KagemushaMintFinalityValidatorKeysV1,
    /// Actual possession of both Pasta signing keys.
    pub possession: KagemushaMintFinalityPairedPossessionProofV1,
}

impl ValidatorCandidateKeyAuthorizationV1 {
    /// Build the only message accepted as candidate key consent.
    #[must_use]
    pub fn new(
        network_id: NetworkId,
        generation: u64,
        keys: KagemushaMintFinalityValidatorKeysV1,
        possession: KagemushaMintFinalityPairedPossessionProofV1,
    ) -> Self {
        Self {
            domain: "iroha:validator-candidate-key-consent:v1".to_owned(),
            network_id,
            generation,
            keys,
            possession,
        }
    }
}

impl ValidatorCandidateKeysV1 {
    /// Reconstruct the exact signed peer-consent challenge.
    #[must_use]
    pub fn authorization(&self) -> ValidatorCandidateKeyAuthorizationV1 {
        ValidatorCandidateKeyAuthorizationV1::new(
            self.network_id,
            self.generation,
            self.keys.clone(),
            self.possession,
        )
    }
    /// Canonical store identity; public keys cannot change the one publication slot.
    #[must_use]
    pub fn key_id(
        network_id: NetworkId,
        generation: u64,
        peer: &iroha_model_base::peer::PeerId,
    ) -> [u8; 32] {
        Hash::new_from_chunks(&[
            b"iroha:validator-candidate-key-slot:v1",
            network_id.as_bytes(),
            &generation.to_le_bytes(),
            &peer.encode(),
        ])
        .into()
    }

    /// Check the published key and proof encodings before cryptographic verification.
    ///
    /// # Errors
    /// Rejects genesis generations, missing network identities and empty signing material.
    pub fn validate(&self) -> Result<(), String> {
        if self.network_id.as_bytes() == &[0; 32]
            || self.generation == 0
            || self.keys.eq_proof_public_key == [0; 32]
            || self.keys.ep_proof_public_key == [0; 32]
            || self.keys.validator.public_key().algorithm() != iroha_crypto::Algorithm::BlsNormal
        {
            return Err("invalid candidate generation publication".to_owned());
        }
        self.possession
            .validate()
            .map_err(|error| error.to_string())
    }
}

/// Immutable result of the election at the end of epoch E for epoch E+2.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::nexus::ValidatorCommitteePreparationV1")]
#[norito(deny_unknown_fields)]
pub struct ValidatorCommitteePreparationV1 {
    /// Sole first-release layout version.
    pub version: u16,
    /// Exact genesis-derived network identity.
    pub network_id: NetworkId,
    /// Epoch whose final pre-state supplied the election inputs.
    pub selection_epoch: u64,
    /// Boundary height whose exact current quorum freezes this election.
    pub selection_height: u64,
    /// Last committed block before the selecting boundary, never that boundary's own hash.
    pub selection_anchor: HashOf<BlockHeader>,
    /// Scheduling epoch after the complete preparation epoch.
    pub target_epoch: u64,
    /// Inclusive activation height.
    pub first_height: u64,
    /// Inclusive end of the initial target scheduling epoch.
    pub last_height: u64,
    /// Successor generation relative to the authority governing preparation.
    pub authority_generation: u64,
    /// Exact E+1 authorization, whose current quorum will decide this attempt.
    pub preparing_authorization_id: [u8; 32],
    /// Independently domain-separated election randomness from the authenticated boundary pulse.
    pub election_seed: [u8; 32],
    /// Complete ordered target committee; it cannot be shrunk or rerolled.
    pub roster: Vec<ValidatorPower>,
    /// Exact consensus-key possession proofs in target seat order.
    pub validator_set_pops: Vec<Vec<u8>>,
}

impl ValidatorCommitteePreparationV1 {
    /// Validate scheduling arithmetic and the canonical complete committee.
    ///
    /// # Errors
    /// Rejects malformed shape; election-input authentication and BLS verification belong to Core.
    pub fn validate(&self) -> Result<(), String> {
        let invalid = || "invalid frozen validator committee preparation".to_owned();
        if self.version != 1
            || self.network_id.as_bytes() == &[0; 32]
            || self.selection_anchor.as_ref() == &[0; 32]
            || self.selection_height == 0
            || self.authority_generation == 0
            || self.selection_epoch.checked_add(2) != Some(self.target_epoch)
            || self.last_height < self.first_height
            || self
                .selection_height
                .checked_add(1)
                .is_none_or(|preparing_first| self.first_height <= preparing_first)
            || self.preparing_authorization_id == [0; 32]
            || self.election_seed == [0; 32]
            || self.roster.len() < 4
            || self.roster.len() > 31
            || self
                .roster
                .windows(2)
                .any(|pair| pair[0].validator >= pair[1].validator)
            || self.roster.iter().any(|entry| entry.power != 1)
            || self.validator_set_pops.len() != self.roster.len()
            || self.validator_set_pops.iter().any(|pop| pop.len() != 96)
        {
            return Err(invalid());
        }
        DualQuorum::from_roster(&self.roster).map_err(|_| invalid())?;
        Ok(())
    }

    /// Check the complete epoch reserved for preparation against its authenticated authorization.
    ///
    /// # Errors
    /// Rejects epoch/height gaps, changed generation, or a different predecessor authorization.
    pub fn validate_against_preparing_authorization(
        &self,
        preparing: &KagemushaMintFinalityEpochAuthorizationV1,
    ) -> Result<(), String> {
        self.validate()?;
        preparing.validate().map_err(|error| error.to_string())?;
        if self.network_id != preparing.network_id
            || self.selection_epoch.checked_add(1) != Some(preparing.epoch)
            || self.selection_height.checked_add(1) != Some(preparing.first_height)
            || preparing.last_height.checked_add(1) != Some(self.first_height)
            || preparing.authority_generation.checked_add(1) != Some(self.authority_generation)
            || preparing
                .authorization_id()
                .map_err(|error| error.to_string())?
                != self.preparing_authorization_id
        {
            return Err(
                "frozen preparation differs from its exact preparing authorization".to_owned(),
            );
        }
        Ok(())
    }

    /// Commit to every frozen input, without a certificate or a self-referential context hash.
    ///
    /// # Errors
    /// Returns an error when the preparation is not canonical.
    pub fn transition_id(&self) -> Result<[u8; 32], String> {
        self.validate()?;
        let mut bytes = b"iroha:validator-committee-preparation:v1".to_vec();
        bytes.extend(self.encode());
        Ok(Hash::new(bytes).into())
    }

    /// Unique DKG session for this immutable preparation attempt.
    ///
    /// # Errors
    /// Rejects a malformed preparation before deriving its separate session domain.
    pub fn beacon_session_id(&self) -> Result<[u8; 32], String> {
        Ok(Hash::new_from_chunks(&[
            b"iroha:validator-committee-beacon-session:v1",
            &self.transition_id()?,
        ])
        .into())
    }
}

/// Complete public credentials prepared for the immutable target committee.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::nexus::ValidatorCommitteeCredentialsV1")]
#[norito(deny_unknown_fields)]
pub struct ValidatorCommitteeCredentialsV1 {
    /// Complete generation, built only from authenticated candidate key publications.
    pub authority: KagemushaMintFinalityAuthorityGenerationV1,
    /// Exact finalized target DKG transcript, retained separately from the incumbent session.
    pub beacon: InstalledBeaconEpochBindingV1,
}

/// Actual possession evidence for every signing role of one target seat.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::nexus::ValidatorCommitteeSeatReadinessV1")]
#[norito(deny_unknown_fields)]
pub struct ValidatorCommitteeSeatReadinessV1 {
    /// Zero-based index in the immutable ordered target roster.
    pub validator_index: u32,
    /// Possession of both Pasta generation keys under the exact attempt context.
    pub pasta: KagemushaMintFinalityPairedPossessionProofV1,
    /// Adaptive threshold proof of the exact beacon share under a separate readiness domain.
    pub beacon: GlobalThresholdBeaconPartialSignatureV1,
}

/// Retained preparation progress and its incumbent-certified terminal disposition.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::nexus::ValidatorCommitteeTransitionV1")]
#[norito(deny_unknown_fields)]
pub struct ValidatorCommitteeTransitionV1 {
    /// Immutable authenticated election; no update may change this value.
    pub preparation: ValidatorCommitteePreparationV1,
    /// Once installed, the exact credentials cannot be replaced within this attempt.
    pub credentials: Option<ValidatorCommitteeCredentialsV1>,
    /// Strictly ordered evidence for distinct target seats; activation requires every seat.
    pub readiness: Vec<ValidatorCommitteeSeatReadinessV1>,
    /// Authorization body whose boundary certificate activated or cancelled this attempt.
    /// This body alone is never accepted as proof of finality.
    pub outcome: Option<KagemushaMintFinalityEpochAuthorizationV1>,
}

/// Install one complete public credential set for an already frozen attempt.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
pub struct PrepareValidatorCommitteeCredentialsV1 {
    /// Exact immutable attempt; a cancelled attempt cannot be reused.
    pub transition_id: [u8; 32],
    /// Target scheduling epoch identifying the retained preparation.
    pub target_epoch: u64,
    /// Complete target generation and finalized beacon transcript.
    pub credentials: ValidatorCommitteeCredentialsV1,
}

/// Admit one target seat's possession evidence under its frozen attempt.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
pub struct AdmitValidatorCommitteeSeatV1 {
    /// Exact immutable attempt, including its authenticated selecting boundary.
    pub transition_id: [u8; 32],
    /// Target scheduling epoch identifying the retained preparation.
    pub target_epoch: u64,
    /// Exact seat, paired-Pasta possession and adaptive beacon-share possession.
    pub readiness: ValidatorCommitteeSeatReadinessV1,
}

/// Owner-authorized preparation commands accepted through `SetParameter`.
///
/// These commands never activate a committee. Only the incumbent's boundary finality
/// certifies activation or cancellation. Execution consumes the command into the dedicated
/// World stores; the command is not persisted as a mutable network parameter.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::nexus::ValidatorCommitteeOperationV1")]
#[norito(tag = "kind", content = "value", deny_unknown_fields)]
pub enum ValidatorCommitteeOperationV1 {
    /// Publish generation keys after proving possession of both keys.
    PublishCandidate(ValidatorCandidateKeysV1),
    /// Bind every target's published keys to the immutable target DKG transcript.
    PrepareCredentials(PrepareValidatorCommitteeCredentialsV1),
    /// Record actual possession for one exact target seat.
    AdmitSeat(AdmitValidatorCommitteeSeatV1),
}

impl ValidatorCommitteeOperationV1 {
    /// Reserved first-release operation envelope identifier.
    #[must_use]
    pub fn parameter_id() -> CustomParameterId {
        "validator_committee_operation_v1"
            .parse()
            .expect("static committee operation identifier")
    }

    /// Produce the exact command envelope to inspect and sign.
    #[must_use]
    pub fn into_custom_parameter(self) -> CustomParameter {
        CustomParameter::new(Self::parameter_id(), Json::new(self))
    }

    /// Decode only the canonical reserved operation envelope.
    ///
    /// # Errors
    /// Rejects malformed matching payloads and unknown variants or fields.
    pub fn from_custom_parameter(
        parameter: &CustomParameter,
    ) -> Result<Option<Self>, norito::json::Error> {
        if parameter.id() != &Self::parameter_id() {
            return Ok(None);
        }
        norito::json::from_str(parameter.payload().get()).map(Some)
    }
}

impl ValidatorCommitteeTransitionV1 {
    fn validated_credentials(&self) -> Result<&ValidatorCommitteeCredentialsV1, String> {
        self.preparation.validate()?;
        let credentials = self
            .credentials
            .as_ref()
            .ok_or("committee credentials are not prepared")?;
        let preparation = &self.preparation;
        credentials
            .authority
            .validate()
            .map_err(|error| error.to_string())?;
        if credentials.authority.network_id != preparation.network_id
            || credentials.authority.generation != preparation.authority_generation
            || credentials.authority.validators.len() != preparation.roster.len()
            || credentials
                .authority
                .validators
                .iter()
                .zip(&preparation.roster)
                .any(|(keys, voter)| keys.validator != voter.validator)
            || credentials.beacon.session_id != preparation.beacon_session_id()?
            || credentials.beacon.transcript_hash == [0; 32]
        {
            return Err("prepared credentials differ from the immutable target".to_owned());
        }
        Ok(credentials)
    }

    /// Derive the complete, replay-bound possession challenge for one target seat.
    ///
    /// # Errors
    /// Rejects missing credentials, malformed preparation or an out-of-range seat.
    pub fn readiness_context(
        &self,
        validator_index: u32,
    ) -> Result<KagemushaMintFinalitySeatReadinessContextV1, String> {
        let credentials = self.validated_credentials()?;
        if usize::try_from(validator_index)
            .ok()
            .is_none_or(|index| index >= self.preparation.roster.len())
        {
            return Err("target validator seat is out of range".to_owned());
        }
        let context = KagemushaMintFinalitySeatReadinessContextV1 {
            version: 1,
            network_id: self.preparation.network_id,
            transition_id: self.preparation.transition_id()?,
            target_epoch: self.preparation.target_epoch,
            authority_generation: self.preparation.authority_generation,
            authority_id: credentials
                .authority
                .authority_id()
                .map_err(|error| error.to_string())?,
            first_height: self.preparation.first_height,
            last_height: self.preparation.last_height,
            validator_index,
            beacon: BeaconEpochBindingV1::Installed(credentials.beacon),
        };
        context.validate().map_err(|error| error.to_string())?;
        Ok(context)
    }

    /// Validate structural consistency before cryptographic and finality verification.
    ///
    /// # Errors
    /// Rejects replaced credentials, repeated seats, inconsistent terminal decisions or incomplete activation.
    pub fn validate(&self) -> Result<(), String> {
        self.preparation.validate()?;
        let invalid = || "invalid validator committee transition progress".to_owned();
        let preparation = &self.preparation;
        if self.readiness.len() > preparation.roster.len()
            || self
                .readiness
                .windows(2)
                .any(|pair| pair[0].validator_index >= pair[1].validator_index)
        {
            return Err(invalid());
        }
        if self.credentials.is_some() {
            let credentials = self.validated_credentials()?;
            for ready in &self.readiness {
                self.readiness_context(ready.validator_index)?;
                ready.pasta.validate().map_err(|_| invalid())?;
                if ready.beacon.session_id != credentials.beacon.session_id
                    || u32::from(ready.beacon.signer_index) != ready.validator_index + 1
                {
                    return Err(invalid());
                }
            }
        } else if !self.readiness.is_empty() {
            return Err(invalid());
        }
        if let Some(outcome) = &self.outcome {
            outcome.validate().map_err(|_| invalid())?;
            if outcome.network_id != preparation.network_id
                || outcome.epoch != preparation.target_epoch
                || outcome.first_height != preparation.first_height
                || outcome.last_height != preparation.last_height
                || outcome.previous_authorization_id != preparation.preparing_authorization_id
                || outcome.transition_id != preparation.transition_id()?
            {
                return Err(invalid());
            }
            match outcome.decision {
                KagemushaMintFinalityEpochDecisionV1::Activate => {
                    let credentials = self.credentials.as_ref().ok_or_else(invalid)?;
                    if self.readiness.len() != preparation.roster.len()
                        || outcome.beacon != BeaconEpochBindingV1::Installed(credentials.beacon)
                        || outcome
                            .validate_against_authority(&credentials.authority)
                            .is_err()
                    {
                        return Err(invalid());
                    }
                }
                KagemushaMintFinalityEpochDecisionV1::RetainAndCancel => {
                    if outcome.authority_generation.checked_add(1)
                        != Some(preparation.authority_generation)
                    {
                        return Err(invalid());
                    }
                }
                _ => return Err(invalid()),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        consensus::GlobalThresholdBeaconPartialSignatureProofV1,
        isi::kagemusha_v1::KagemushaPastaSchnorrSignatureV1,
    };
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_model_base::peer::PeerId;

    fn authority(generation: u64) -> KagemushaMintFinalityAuthorityGenerationV1 {
        let mut validators = (1_u8..=4)
            .map(|seed| KagemushaMintFinalityValidatorKeysV1 {
                validator: PeerId::new(
                    KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal)
                        .public_key()
                        .clone(),
                ),
                eq_proof_public_key: [seed; 32],
                ep_proof_public_key: [seed + 16; 32],
            })
            .collect::<Vec<_>>();
        validators.sort_by(|a, b| a.validator.cmp(&b.validator));
        KagemushaMintFinalityAuthorityGenerationV1 {
            version: 1,
            network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
                b"committee-model-test",
            ))),
            generation,
            validators,
        }
    }

    fn preparing() -> KagemushaMintFinalityEpochAuthorizationV1 {
        let authority = authority(0);
        KagemushaMintFinalityEpochAuthorizationV1 {
            version: 1,
            network_id: authority.network_id,
            epoch: 1,
            first_height: 11,
            last_height: 20,
            authority_generation: 0,
            authority_id: authority.authority_id().unwrap(),
            beacon: BeaconEpochBindingV1::Installed(InstalledBeaconEpochBindingV1 {
                session_id: [7; 32],
                transcript_hash: [8; 32],
            }),
            previous_authorization_id: [9; 32],
            transition_id: [0; 32],
            decision: KagemushaMintFinalityEpochDecisionV1::Retain,
        }
    }

    fn preparation() -> ValidatorCommitteePreparationV1 {
        let authorization = preparing();
        ValidatorCommitteePreparationV1 {
            version: 1,
            network_id: authorization.network_id,
            selection_epoch: 0,
            selection_height: 10,
            selection_anchor: HashOf::from_untyped_unchecked(Hash::new(b"height-nine")),
            target_epoch: 2,
            first_height: 21,
            last_height: 30,
            authority_generation: 1,
            preparing_authorization_id: authorization.authorization_id().unwrap(),
            election_seed: [4; 32],
            roster: authority(1)
                .validators
                .into_iter()
                .map(|keys| ValidatorPower {
                    validator: keys.validator,
                    power: 1,
                })
                .collect(),
            validator_set_pops: vec![vec![1; 96]; 4],
        }
    }

    // These are structural codec fixtures. Core tests generate and verify actual curve proofs.
    fn possession() -> KagemushaMintFinalityPairedPossessionProofV1 {
        KagemushaMintFinalityPairedPossessionProofV1 {
            eq_proof_signature: KagemushaPastaSchnorrSignatureV1 {
                nonce_commitment: [1; 32],
                response: [2; 32],
            },
            ep_proof_signature: KagemushaPastaSchnorrSignatureV1 {
                nonce_commitment: [3; 32],
                response: [4; 32],
            },
        }
    }

    fn transition() -> ValidatorCommitteeTransitionV1 {
        let credentials = ValidatorCommitteeCredentialsV1 {
            authority: authority(1),
            beacon: InstalledBeaconEpochBindingV1 {
                session_id: preparation().beacon_session_id().unwrap(),
                transcript_hash: [6; 32],
            },
        };
        ValidatorCommitteeTransitionV1 {
            preparation: preparation(),
            readiness: (0..4)
                .map(|index| ValidatorCommitteeSeatReadinessV1 {
                    validator_index: index,
                    pasta: possession(),
                    beacon: GlobalThresholdBeaconPartialSignatureV1 {
                        session_id: credentials.beacon.session_id,
                        signer_index: index as u16 + 1,
                        signature_share: [1; 48],
                        proof: GlobalThresholdBeaconPartialSignatureProofV1 {
                            x: [1; 96],
                            y: [2; 48],
                            z_s: [3; 32],
                            z_r: [4; 32],
                            z_u: [5; 32],
                        },
                    },
                })
                .collect(),
            credentials: Some(credentials),
            outcome: None,
        }
    }

    #[test]
    fn committee_preparation_reserves_a_complete_epoch_and_exact_parent() {
        let preparation = preparation();
        preparation
            .validate_against_preparing_authorization(&preparing())
            .unwrap();
        for mutation in 0..5 {
            let mut wrong = preparation.clone();
            match mutation {
                0 => wrong.target_epoch -= 1,
                1 => wrong.first_height -= 1,
                2 => wrong.authority_generation += 1,
                3 => wrong.preparing_authorization_id[0] ^= 1,
                _ => wrong.selection_height += 1,
            }
            assert!(
                wrong
                    .validate_against_preparing_authorization(&preparing())
                    .is_err()
            );
        }
        let mut another_attempt = preparation.clone();
        another_attempt.selection_anchor =
            HashOf::from_untyped_unchecked(Hash::new(b"another-anchor"));
        assert_ne!(
            preparation.transition_id().unwrap(),
            another_attempt.transition_id().unwrap()
        );
        let mut partial = preparation;
        partial.roster.pop();
        partial.validator_set_pops.pop();
        assert!(partial.validate().is_err());
    }

    #[test]
    fn committee_preparation_binds_unequal_preparing_and_target_intervals() {
        let preparing = preparing();
        let mut target = preparation();
        target.last_height = 45;
        target
            .validate_against_preparing_authorization(&preparing)
            .unwrap();
        let id = target.transition_id().unwrap();
        let mut changed = target.clone();
        changed.last_height += 1;
        assert_ne!(id, changed.transition_id().unwrap());
        target.first_height += 1;
        assert!(
            target
                .validate_against_preparing_authorization(&preparing)
                .is_err()
        );
        target.first_height = target.selection_height + 1;
        assert!(
            target.validate().is_err(),
            "preparation must reserve a complete nonempty epoch"
        );
    }

    #[test]
    fn committee_activation_requires_every_distinct_target_seat() {
        let mut transition = transition();
        let credentials = transition.credentials.as_ref().unwrap();
        transition.outcome = Some(KagemushaMintFinalityEpochAuthorizationV1 {
            epoch: 2,
            first_height: 21,
            last_height: 30,
            authority_generation: 1,
            authority_id: credentials.authority.authority_id().unwrap(),
            beacon: BeaconEpochBindingV1::Installed(credentials.beacon),
            previous_authorization_id: preparing().authorization_id().unwrap(),
            transition_id: transition.preparation.transition_id().unwrap(),
            decision: KagemushaMintFinalityEpochDecisionV1::Activate,
            ..preparing()
        });
        transition.validate().unwrap();
        transition
            .outcome
            .as_ref()
            .unwrap()
            .validate_successor(&preparing())
            .unwrap();
        let mut missing = transition.clone();
        missing.readiness.pop();
        assert!(
            missing.validate().is_err(),
            "an exact quorum of target seats is insufficient"
        );
        let mut duplicate = transition.clone();
        duplicate.readiness[3] = duplicate.readiness[2].clone();
        assert!(duplicate.validate().is_err());
        let mut wrong_session = transition;
        wrong_session.readiness[0].beacon.session_id[0] ^= 1;
        assert!(wrong_session.validate().is_err());
    }

    #[test]
    fn committee_cancellation_retains_the_current_generation_without_new_keys() {
        let mut transition = ValidatorCommitteeTransitionV1 {
            preparation: preparation(),
            credentials: None,
            readiness: Vec::new(),
            outcome: None,
        };
        let current = preparing();
        transition.outcome = Some(KagemushaMintFinalityEpochAuthorizationV1 {
            epoch: 2,
            first_height: 21,
            last_height: 30,
            previous_authorization_id: current.authorization_id().unwrap(),
            transition_id: transition.preparation.transition_id().unwrap(),
            decision: KagemushaMintFinalityEpochDecisionV1::RetainAndCancel,
            ..current
        });
        transition.validate().unwrap();
        transition
            .outcome
            .as_ref()
            .unwrap()
            .validate_successor(&current)
            .unwrap();
        transition.outcome.as_mut().unwrap().transition_id[0] ^= 1;
        assert!(transition.validate().is_err());
    }

    #[test]
    fn committee_readiness_context_binds_attempt_seat_and_credentials() {
        let transition = transition();
        let first = transition.readiness_context(0).unwrap();
        assert_ne!(first, transition.readiness_context(1).unwrap());
        assert!(transition.readiness_context(4).is_err());
        let mut changed = transition.clone();
        changed.preparation.election_seed[0] ^= 1;
        assert!(changed.readiness_context(0).is_err());
        changed
            .credentials
            .as_mut()
            .unwrap()
            .authority
            .validators
            .swap(0, 1);
        assert!(changed.readiness_context(0).is_err());
    }

    #[test]
    fn committee_candidate_and_transition_roundtrip_canonical_codecs() {
        let authority = authority(1);
        let keys = authority.validators[0].clone();
        let signer = (1_u8..=4)
            .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
            .find(|pair| pair.public_key() == keys.validator.public_key())
            .unwrap();
        let authorization = ValidatorCandidateKeyAuthorizationV1::new(
            authority.network_id,
            authority.generation,
            keys.clone(),
            possession(),
        );
        let candidate = ValidatorCandidateKeysV1 {
            network_id: authority.network_id,
            generation: authority.generation,
            keys,
            possession: possession(),
            peer_signature: SignatureOf::new(signer.private_key(), &authorization),
        };
        candidate.validate().unwrap();
        candidate
            .peer_signature
            .verify(
                candidate.keys.validator.public_key(),
                &candidate.authorization(),
            )
            .unwrap();
        let key_id = ValidatorCandidateKeysV1::key_id(
            candidate.network_id,
            candidate.generation,
            &candidate.keys.validator,
        );
        assert_ne!(
            key_id,
            ValidatorCandidateKeysV1::key_id(
                candidate.network_id,
                candidate.generation + 1,
                &candidate.keys.validator
            )
        );
        let mut invalid = candidate.clone();
        invalid.generation = 0;
        assert!(invalid.validate().is_err());
        assert_eq!(
            candidate,
            ValidatorCandidateKeysV1::decode(&mut candidate.encode().as_slice()).unwrap()
        );
        let transition = transition();
        transition.validate().unwrap();
        assert_eq!(
            transition,
            ValidatorCommitteeTransitionV1::decode(&mut transition.encode().as_slice()).unwrap()
        );
        let json = norito::json::to_json(&transition).unwrap();
        assert_eq!(transition, norito::json::from_json(&json).unwrap());
        let operation = ValidatorCommitteeOperationV1::PublishCandidate(candidate);
        let parameter = operation.clone().into_custom_parameter();
        assert_eq!(
            Some(operation),
            ValidatorCommitteeOperationV1::from_custom_parameter(&parameter).unwrap()
        );
    }
}
