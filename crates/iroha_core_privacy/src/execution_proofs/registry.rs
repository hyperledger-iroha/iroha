//! Extensible catalog and dispatch for immutable, compiled execution adapters.
//!
//! This module is not part of any adapter's source commitment. Add later relations here with an exact descriptor. Registration resolves this
//! closed catalog; neither callers nor registrants may upload code, replace parameters or alias IDs.

use super::{error::ExecutionProofErrorV1, proof};
use iroha_crypto::Hash;
use iroha_data_model::{
    NetworkId,
    execution_proofs::{ExecutionProofEnvelopeV1, ExecutionProofProfileV1},
    game::{
        GameCheckpointV1, GameForcedBatchV1, GameManifestV1, GameOutcomeV1, GameParticipantV1,
        GameTranscriptAnchorV1,
    },
};

#[derive(Clone, Copy)]
enum CompiledAdapterV1 {
    RaceV1,
}
const COMPILED_ADAPTERS_V1: &[CompiledAdapterV1] = &[CompiledAdapterV1::RaceV1];

impl CompiledAdapterV1 {
    fn profile(self) -> ExecutionProofProfileV1 {
        match self {
            Self::RaceV1 => proof::compiled_race_profile_v1(),
        }
    }
    fn resolve(profile_id: &Hash) -> Option<Self> {
        COMPILED_ADAPTERS_V1
            .iter()
            .copied()
            .find(|adapter| adapter.profile().profile_id == *profile_id)
    }
    fn for_manifest(manifest: &GameManifestV1) -> Result<Self, ExecutionProofErrorV1> {
        Self::resolve(&manifest.profile_id).ok_or(ExecutionProofErrorV1::Statement)
    }
    fn for_envelope(envelope: &ExecutionProofEnvelopeV1) -> Result<Self, ExecutionProofErrorV1> {
        Self::resolve(&envelope.profile_id).ok_or(ExecutionProofErrorV1::Envelope)
    }
}

/// Resolve an exact compiled descriptor; registration cannot install code or relax its parameters.
#[must_use]
pub fn compiled_execution_profile_v1(profile: &Hash) -> Option<ExecutionProofProfileV1> {
    CompiledAdapterV1::resolve(profile).map(CompiledAdapterV1::profile)
}

/// Enumerate every compiled descriptor for application-neutral capability discovery.
#[must_use]
pub fn compiled_execution_profiles_v1() -> Vec<ExecutionProofProfileV1> {
    COMPILED_ADAPTERS_V1
        .iter()
        .map(|adapter| adapter.profile())
        .collect()
}

/// Validate application-neutral manifest bounds using its exact immutable compiled adapter.
pub fn validate_game_manifest_v1(manifest: &GameManifestV1) -> Result<(), ExecutionProofErrorV1> {
    match CompiledAdapterV1::for_manifest(manifest)? {
        CompiledAdapterV1::RaceV1 => proof::validate_race_manifest_v1(manifest),
    }
}

/// Validate opaque participant data using the manifest's compiled adapter.
pub fn validate_game_participant_v1(
    manifest: &GameManifestV1,
    data: &[u8],
) -> Result<(), ExecutionProofErrorV1> {
    match CompiledAdapterV1::for_manifest(manifest)? {
        CompiledAdapterV1::RaceV1 => proof::validate_race_participant_v1(manifest, data),
    }
}

/// Return the exact immutable equipment requirements of the compiled adapter.
/// This only describes requirements; the typed wallet clauses must independently authorize them.
pub fn game_resource_requirements_v1(
    manifest: &GameManifestV1,
    data: &[u8],
) -> Result<Vec<iroha_data_model::game::GameResourceRequirementV1>, ExecutionProofErrorV1> {
    match CompiledAdapterV1::for_manifest(manifest)? {
        CompiledAdapterV1::RaceV1 => {
            proof::validate_race_participant_v1(manifest, data)?;
            Ok(Vec::new())
        }
    }
}

/// Validate opaque input batches before accepting their commitments or reveals.
pub fn validate_game_input_v1(
    manifest: &GameManifestV1,
    input: &[u8],
) -> Result<(), ExecutionProofErrorV1> {
    match CompiledAdapterV1::for_manifest(manifest)? {
        CompiledAdapterV1::RaceV1 => proof::validate_race_input_v1(manifest, input),
    }
}

/// Compute the initial state commitment prescribed by the exact compiled application.
pub fn initial_game_state_root_v1(
    network: &NetworkId,
    manifest: &GameManifestV1,
    player_count: u8,
) -> Result<Hash, ExecutionProofErrorV1> {
    match CompiledAdapterV1::for_manifest(manifest)? {
        CompiledAdapterV1::RaceV1 => {
            proof::initial_race_manifest_state_root_v1(network, manifest, player_count)
        }
    }
}

/// Verify execution and return only the adapter-authenticated generic outcome.
pub fn verify_execution_proof_v1(
    envelope: &ExecutionProofEnvelopeV1,
) -> Result<GameOutcomeV1, ExecutionProofErrorV1> {
    match CompiledAdapterV1::for_envelope(envelope)? {
        CompiledAdapterV1::RaceV1 => proof::verify_race_outcome_v1(envelope),
    }
}

/// Verify execution and its binding to every retained checkpoint, forced input and removal.
pub fn verify_game_proof_for_history_v1(
    envelope: &ExecutionProofEnvelopeV1,
    manifest: &GameManifestV1,
    outcome: &GameOutcomeV1,
    checkpoint: Option<&GameCheckpointV1>,
    anchors: &[GameTranscriptAnchorV1],
    batches: &[GameForcedBatchV1],
    participants: &[GameParticipantV1],
    epoch: u64,
) -> Result<(), ExecutionProofErrorV1> {
    match CompiledAdapterV1::for_envelope(envelope)? {
        CompiledAdapterV1::RaceV1 => proof::verify_race_proof_for_history_v1(
            envelope,
            manifest,
            outcome,
            checkpoint,
            anchors,
            batches,
            participants,
            epoch,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::HashOf;
    use iroha_data_model::{
        block::BlockHeader,
        execution_proofs::{RaceReplayV1, RaceResultV1, RaceStateV1, RaceTrackV1},
        game::{GameAccessV1, GamePayoutPolicyV1},
    };
    use norito::codec::Encode;

    #[test]
    fn catalog_retains_exact_unique_descriptors_and_rejects_uncompiled_ids() {
        let profiles = compiled_execution_profiles_v1();
        assert!(!profiles.is_empty());
        let mut identifiers = std::collections::BTreeSet::new();
        for profile in profiles {
            assert!(identifiers.insert(profile.profile_id));
            assert_eq!(
                compiled_execution_profile_v1(&profile.profile_id),
                Some(profile)
            );
        }
        assert_eq!(
            compiled_execution_profile_v1(&proof::race_profile_id_v1()),
            Some(proof::compiled_race_profile_v1())
        );
        assert_eq!(
            identifiers.len(),
            1,
            "first release exposes one compiled stock relation"
        );
        assert!(!proof::compiled_race_profile_v1().qualified);
        assert!(
            compiled_execution_profile_v1(&Hash::new(b"uncompiled-execution-profile")).is_none()
        );
    }

    #[test]
    fn compiled_catalog_binds_rules_and_rejects_unknown_manifest_ids() {
        let network = NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::new(b"generic-execution-registry-parity-network"),
        ));
        let mut manifest = GameManifestV1 {
            version: 1,
            application_id: Hash::new(b"application-independent-registry-fixture"),
            profile_id: proof::race_profile_id_v1(),
            application_parameters: RaceTrackV1::NeonTokyo.encode(),
            min_participants: 2,
            max_participants: 8,
            batch_ticks: 6,
            max_ticks: 5400,
            max_input_bytes: 12,
            max_participant_data_bytes: 1,
            access: GameAccessV1::Public,
            payout_policy: GamePayoutPolicyV1::NoPayout,
        };
        let mut genesis = None;
        for profile in compiled_execution_profiles_v1() {
            manifest.profile_id = profile.profile_id;
            validate_game_manifest_v1(&manifest).unwrap();
            assert_eq!(profile.version, 1, "typed profile schema remains V1");
            assert_eq!(profile.rules_hash, proof::race_rules_hash_v1());
            for slot in 0..6 {
                validate_game_participant_v1(&manifest, &[slot]).unwrap();
            }
            assert!(validate_game_participant_v1(&manifest, &[6]).is_err());
            validate_game_input_v1(&manifest, &[0; 12]).unwrap();
            assert!(validate_game_input_v1(&manifest, &[255; 12]).is_err());
            let root = initial_game_state_root_v1(&network, &manifest, 8).unwrap();
            assert_eq!(*genesis.get_or_insert(root), root);
        }
        manifest.profile_id = Hash::new(b"caller-supplied-unknown-profile");
        assert!(validate_game_manifest_v1(&manifest).is_err());
        assert!(validate_game_input_v1(&manifest, &[0; 12]).is_err());
        assert!(initial_game_state_root_v1(&network, &manifest, 8).is_err());
    }

    #[test]
    fn browser_race_adapter_vectors_match_native_codec_and_transcript_commitments() {
        use norito::json::{self, JsonDeserialize, Value};

        fn hex(bytes: &[u8]) -> String {
            bytes.iter().map(|byte| format!("{byte:02X}")).collect()
        }
        fn check<T: Encode + JsonDeserialize>(row: &Value) -> T {
            let value: T = json::from_value(row["value"].clone()).unwrap();
            assert_eq!(hex(&value.encode()), row["encoded_hex"].as_str().unwrap());
            value
        }
        let fixture: Value = json::from_str(include_str!(
            "../../../../javascript/iroha_js/test/fixtures/race-v1-codec.json"
        ))
        .unwrap();
        let network: NetworkId = json::from_value(fixture["network_id"].clone()).unwrap();
        let rows = fixture["vectors"].as_array().unwrap();
        assert_eq!(rows.len(), 4);
        for row in rows {
            let digest = match row["name"].as_str().unwrap() {
                "RaceTrackV1" => {
                    check::<RaceTrackV1>(row);
                    None
                }
                "RaceStateV1" => Some(proof::race_state_root_v1(
                    &network,
                    &check::<RaceStateV1>(row),
                )),
                "RaceReplayV1" => Some(proof::race_transcript_root_v1(
                    &network,
                    &check::<RaceReplayV1>(row),
                )),
                "RaceResultV1" => {
                    check::<RaceResultV1>(row);
                    None
                }
                unexpected => panic!("unknown native adapter fixture {unexpected}"),
            };
            if let Some(digest) = digest {
                assert_eq!(
                    hex(digest.as_ref()),
                    row["gameplay_digest_hex"].as_str().unwrap()
                );
            }
        }
    }
}
