//! Historical receipt proofs from the original archived writes and native finality.
//!
//! An archive is untrusted until its exact carrier and complete ordinary-write root agree
//! with the authenticated native chain. Current World values cannot substitute for it.
//! Ordinary writes retain exact original-pool allocation custody through historical proof
//! construction; compact casting leaves retain their exact array charge. TODO(S8): admit the
//! remaining native proof graph and
//! receipt/tree scratch before allocation; this scoped write owner does not fund those graphs.

mod ordinary_writes;

use crate::{
    query::native_context_archive::NativeContextArchive, state::StateReadOnly,
    sumeragi::finality::build_proof,
};
use iroha_data_model::{
    block::{consensus::ExecWitness, decode_versioned_signed_block},
    isi::kagemusha_v1::{
        KAGEMUSHA_CHAIN_VERSION_V1, KagemushaOperationFinalityV1, KagemushaOperationKindV1,
        KagemushaReserveReceiptWitnessV1, KagemushaTopUpMembershipWitnessV1,
    },
    sumeragi_amx::{
        AmxCertifiedBlockV1, AmxRecordKind, AmxRecordProofV1, AmxRecordV1, amx_record_witness_key,
    },
    sumeragi_finality::{NativeLaneStateProof, SumeragiFinalityProof},
    validation_fee::ValidationFeePolicyWitnessProofV1,
};

type CastingBinding =
    iroha_data_model::parliament_casting::ParliamentTimedOvnCastingContextBindingV1;

struct OriginalReceiptSource {
    finality: SumeragiFinalityProof,
    witness: iroha_allocation::RetainedPayload<ExecWitness>,
    casting_bindings: ordinary_writes::CastingOwner,
}

fn original_source(
    view: &impl StateReadOnly,
    height: u64,
) -> Result<OriginalReceiptSource, String> {
    // Genesis R is unsigned. Historical execution capabilities require a successor;
    // these operation endpoints serve exact committed non-genesis operations only.
    if height < 2 {
        return Err("historical receipt requires a certified non-genesis execution".into());
    }
    let finality = build_proof(view, height).map_err(|error| error.to_string())?;
    let decoded = finality
        .decode_checked()
        .map_err(|error| error.to_string())?;
    let budget = view.prepared_contract_cache().execution_budget().clone();
    let archive = NativeContextArchive::open_existing(
        view.kura(),
        budget.clone(),
        view.kura().native_context_archive_max_bytes(),
    )
    .map_err(|error| error.to_string())?;
    let bytes = archive
        .read_exact(height, finality.block_header.hash())
        .map_err(|error| error.to_string())?;
    let projection =
        ordinary_writes::decode_projection(&bytes, &budget).map_err(|error| error.to_string())?;
    if projection.carrier_height != height
        || projection.carrier_hash != finality.block_header.hash()
    {
        return Err("original write archive names another certified carrier".into());
    }
    if !projection.casting_bindings.belongs_to(&budget) {
        return Err("original casting array belongs to another allocation pool".into());
    }
    let witness = projection.witness;
    let path = NativeLaneStateProof::from_witness(witness.get(), &budget)
        .map_err(|error| error.to_string())?;
    if !path.verify(
        *view.network_id(),
        height,
        decoded.execution().ordinary_writes_root,
    ) || !path
        .matches_state_payload(*view.network_id(), height, projection.lane_payload)
        .map_err(|error| error.to_string())?
    {
        return Err("archived original writes differ from certified native execution".into());
    }
    archive
        .recheck_namespace()
        .map_err(|error| error.to_string())?;
    Ok(OriginalReceiptSource {
        finality,
        witness,
        casting_bindings: projection.casting_bindings,
    })
}

/// Prove one AMX record from its original persisted native execution.
///
/// An absent record returns `None` only after the complete archived write set and its carrier
/// have been authenticated. Missing or corrupt history never becomes record absence, and
/// current World values cannot replace records pruned after the AMX deadline. The caller's
/// immutable State view pins the history cut throughout construction.
///
/// Archive input and decoded ordinary writes use the original execution pool. As for the
/// other native receipt readers, portable proof graphs and downstream tree scratch still
/// require complete resource qualification; this API does not grant monetary authority.
///
/// # Errors
/// Uncertified height, missing or corrupt native history, original-pool refusal, inconsistent
/// archive identity/root, or a malformed record.
pub fn amx_record_proof(
    view: &impl StateReadOnly,
    height: u64,
    kind: AmxRecordKind,
    transaction: [u8; 32],
) -> Result<Option<AmxRecordProofV1>, String> {
    let source = original_source(view, height)?;
    let key = amx_record_witness_key(kind, transaction);
    let Some(written) = source
        .witness
        .get()
        .writes
        .iter()
        .rev()
        .find(|write| write.key == key)
    else {
        return Ok(None);
    };
    let record = AmxRecordV1::from_witness(&written.key, &written.value)
        .map_err(|error| error.to_string())?;
    // Decode the exact portable carrier already authenticated by original_source, never a
    // second Kura read which could substitute certificate bytes after source verification.
    let block = norito::core::with_decode_limits_scope(
        norito::canonical_decode_limits(source.finality.block_wire.len()),
        || decode_versioned_signed_block(&source.finality.block_wire),
    )
    .map_err(|error| error.to_string())?;
    let certificate = block
        .commit_certificate()
        .ok_or("authenticated AMX carrier has no native certificate")?;
    AmxRecordProofV1::from_writes(
        AmxCertifiedBlockV1::from_certificate(certificate),
        source
            .witness
            .get()
            .writes
            .iter()
            .map(|write| (write.key.as_slice(), write.value.as_slice())),
        record,
    )
    .map(Some)
    .map_err(|error| error.to_string())
}

/// Derive the exact fee policy opening from a certified historical execution.
///
/// # Errors
/// Missing or corrupt original archive, invalid native finality, or inconsistent policy write.
pub fn validation_fee_policy_witness(
    view: &impl StateReadOnly,
    height: u64,
) -> Result<ValidationFeePolicyWitnessProofV1, String> {
    let source = original_source(view, height)?;
    let (proof, root) =
        crate::receiver_snapshot::validation_fee_policy_witness_proof_v1(source.witness.get())?;
    let execution = source
        .finality
        .decode_checked()
        .map_err(|error| error.to_string())?;
    if root != execution.execution().ordinary_writes_root || !proof.verify(root) {
        return Err("fee policy opening differs from the certified ordinary-write root".into());
    }
    Ok(proof)
}

/// Derive one reserve receipt and its exact top-up path from original native execution.
///
/// Portable finality grants no offline mint authority. The release-pinned recursive proof
/// verifier and device/profile authorization remain required for monetary mint execution.
///
/// # Errors
/// Invalid finality, missing original history, inconsistent receipt, or invalid top-up tree.
pub fn kagemusha_operation_finality(
    view: &impl StateReadOnly,
    height: u64,
    operation_id: [u8; 32],
) -> Result<Option<KagemushaOperationFinalityV1>, String> {
    kagemusha_finality_source(view, height)?.operation_finality(operation_id)
}

/// One authenticated original height, retaining one native proof and one complete receipt tree.
///
/// This is historical source material, not recursive monetary proof authority. Operation
/// results are assembled one at a time so a height never clones its block frame per receipt.
pub(crate) struct KagemushaFinalitySource {
    finality: SumeragiFinalityProof,
    receipts: Vec<KagemushaReserveReceiptWitnessV1>,
    top_ups: Option<crate::zk::kagemusha_v1_recursion::KagemushaMintFinalityTreeV1>,
}

impl KagemushaFinalitySource {
    /// Original native decision authenticated independently from the write archive.
    pub(crate) fn finality_proof(&self) -> &SumeragiFinalityProof {
        &self.finality
    }

    /// Exact operation-id ordered reserve receipts from this original execution.
    pub(crate) fn receipts(&self) -> &[KagemushaReserveReceiptWitnessV1] {
        &self.receipts
    }

    /// Select one real membership for a boundary rotation, or none for an empty tree.
    pub(crate) fn first_top_up_membership(
        &self,
    ) -> Result<Option<KagemushaTopUpMembershipWitnessV1>, String> {
        let Some(receipt) = self
            .receipts
            .iter()
            .find(|proof| proof.receipt.kind == KagemushaOperationKindV1::TopUp)
        else {
            return Ok(None);
        };
        self.top_up_membership(receipt.receipt.operation_id)
            .map(Some)
    }

    fn top_up_membership(
        &self,
        operation_id: [u8; 32],
    ) -> Result<KagemushaTopUpMembershipWitnessV1, String> {
        let tree = self
            .top_ups
            .as_ref()
            .ok_or("original top-up tree is absent")?;
        let proof = tree
            .witness(operation_id)
            .map_err(|error| error.to_string())?;
        crate::zk::kagemusha_v1_recursion::verify_kagemusha_top_up_membership_v1(
            &proof,
            tree.leaf_count(),
        )
        .map_err(|error| error.to_string())?;
        Ok(proof)
    }

    /// Assemble one receipt opening, preserving the exact originally certified block frame.
    pub(crate) fn operation_finality(
        &self,
        operation_id: [u8; 32],
    ) -> Result<Option<KagemushaOperationFinalityV1>, String> {
        let Ok(index) = self
            .receipts
            .binary_search_by_key(&operation_id, |proof| proof.receipt.operation_id)
        else {
            return Ok(None);
        };
        let receipt = &self.receipts[index];
        let top_up_membership_witness = if receipt.receipt.kind == KagemushaOperationKindV1::TopUp {
            Some(self.top_up_membership(operation_id)?)
        } else {
            None
        };
        Ok(Some(KagemushaOperationFinalityV1 {
            version: KAGEMUSHA_CHAIN_VERSION_V1,
            network_id: receipt.receipt.network_id,
            finality_proof: self.finality.clone(),
            reserve_receipt_witness: receipt.clone(),
            top_up_membership_witness,
        }))
    }
}

/// Authenticate all original reserve receipts and their complete certified top-up projection.
/// An absent requested operation cannot mask a malformed root/count or another receipt.
pub(crate) fn kagemusha_finality_source(
    view: &impl StateReadOnly,
    height: u64,
) -> Result<KagemushaFinalitySource, String> {
    use crate::zk::kagemusha_v1_recursion::{
        KagemushaMintFinalityTreeV1, kagemusha_top_up_leaf_from_receipt_v1,
    };
    let source = original_source(view, height)?;
    let (receipts, root) =
        crate::receiver_snapshot::kagemusha_reserve_receipt_witnesses_v1(source.witness.get())?;
    let decoded = source
        .finality
        .decode_checked()
        .map_err(|error| error.to_string())?;
    let execution = decoded.execution();
    if root != execution.ordinary_writes_root
        || receipts
            .iter()
            .any(|proof| proof.receipt.network_id != *view.network_id())
    {
        return Err("reserve receipts differ from the certified native execution".into());
    }
    let leaves = receipts
        .iter()
        .filter(|proof| proof.receipt.kind == KagemushaOperationKindV1::TopUp)
        .map(|proof| kagemusha_top_up_leaf_from_receipt_v1(&proof.receipt))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let top_ups = if leaves.is_empty() {
        if execution.kagemusha_top_up_count != 0 || execution.kagemusha_top_up_root.is_some() {
            return Err("absent original top-ups differ from certified native execution".into());
        }
        None
    } else {
        let tree = KagemushaMintFinalityTreeV1::new(leaves).map_err(|error| error.to_string())?;
        if tree.leaf_count() != execution.kagemusha_top_up_count
            || Some(tree.execution_root()) != execution.kagemusha_top_up_root
        {
            return Err("original top-up tree differs from certified native execution".into());
        }
        Some(tree)
    };
    drop(decoded);
    Ok(KagemushaFinalitySource {
        finality: source.finality,
        receipts,
        top_ups,
    })
}

/// Construct a historical casting membership proof from original execution leaves.
///
/// # Errors
/// Missing original source, mismatched root/count, invalid ordering or unavailable membership.
pub fn parliament_casting_proof(
    view: &impl StateReadOnly,
    height: u64,
    ballot_attempt_id: iroha_data_model::parliament_types::BallotAttemptId,
) -> Result<
    Option<iroha_data_model::parliament_casting::ParliamentTimedOvnFinalizedCastingProofV1>,
    String,
> {
    use iroha_crypto::{HashOf, MerkleTree};
    use iroha_data_model::parliament_casting::{
        ParliamentTimedOvnCastingContextMembershipProofV1,
        ParliamentTimedOvnCastingSnapshotCommitmentV1, ParliamentTimedOvnFinalizedCastingProofV1,
    };
    let source = original_source(view, height)?;
    let (snapshot_witness, root) =
        crate::receiver_snapshot::parliament_timed_ovn_casting_witness_proof_v1(
            source.witness.get(),
        )?;
    let decoded = source
        .finality
        .decode_checked()
        .map_err(|error| error.to_string())?;
    let expected = ParliamentTimedOvnCastingSnapshotCommitmentV1::from_ordered_bindings(
        height,
        source.casting_bindings.as_slice(),
    )?;
    if root != decoded.execution().ordinary_writes_root
        || !snapshot_witness.verify(root)
        || snapshot_witness.commitment()? != expected
        || source
            .casting_bindings
            .as_slice()
            .iter()
            .any(|binding| binding.network_id != *view.network_id().as_bytes())
    {
        return Err("casting leaves differ from certified native snapshot".into());
    }
    let Ok(index) = source
        .casting_bindings
        .as_slice()
        .binary_search_by_key(&ballot_attempt_id, |binding| binding.ballot_attempt_id)
    else {
        return Ok(None);
    };
    let tree = MerkleTree::<CastingBinding>::from_iter(
        source.casting_bindings.as_slice().iter().map(HashOf::new),
    );
    let leaf_index = u32::try_from(index).map_err(|_| "casting leaf index exceeds u32")?;
    let membership_proof = ParliamentTimedOvnCastingContextMembershipProofV1::new(
        tree.get_proof(leaf_index)
            .ok_or("casting membership proof is absent")?,
    );
    let proof = ParliamentTimedOvnFinalizedCastingProofV1 {
        snapshot_witness,
        // This fixed-size leaf has no nested allocation; the original array stays charged.
        binding: source.casting_bindings.as_slice()[index].clone(),
        membership_proof,
    };
    if !proof.verify(root) {
        return Err("casting membership differs from certified native write root".into());
    }
    Ok(Some(proof))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sumeragi::test_chain::{CertifiedTestChain, Signers, TestChainConfig};

    #[test]
    fn actual_native_receipt_source_rejects_absent_and_subquorum_history() {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(crate::state::World::new(), 1_000))
                .unwrap();
        assert!(original_source(&chain.state().view(), 1).is_err());
        chain.commit(Vec::new());
        let view = chain.state().view();
        let source = original_source(&view, 2).unwrap();
        assert!(!source.witness.get().writes.is_empty());
        assert!(validation_fee_policy_witness(&view, 2).is_ok());
        assert!(
            parliament_casting_proof(
                &view,
                2,
                iroha_data_model::parliament_types::BallotAttemptId::new([7; 32])
            )
            .unwrap()
            .is_none()
        );
        assert!(
            kagemusha_operation_finality(&view, 2, [7; 32])
                .unwrap()
                .is_none()
        );
        let mint_source = kagemusha_finality_source(&view, 2).unwrap();
        assert_eq!(mint_source.finality_proof(), &source.finality);
        assert!(mint_source.receipts().is_empty());
        assert!(mint_source.first_top_up_membership().unwrap().is_none());
        assert!(mint_source.operation_finality([9; 32]).unwrap().is_none());
        assert!(mint_source.top_up_membership([9; 32]).is_err());
        assert!(kagemusha_finality_source(&view, 1).is_err());
        assert!(kagemusha_finality_source(&view, 3).is_err());
        assert!(original_source(&view, 3).is_err());
        drop(view);
        chain.commit(Vec::new());
        chain.corrupt_local_quorum_for_test(3, Signers::BelowQuorum);
        assert!(original_source(&chain.state().view(), 3).is_err());
    }

    #[test]
    fn original_archive_corruption_never_falls_back_to_current_world() {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(crate::state::World::new(), 1_000))
                .unwrap();
        chain.commit(Vec::new());
        assert!(original_source(&chain.state().view(), 2).is_ok());
        let directory = chain.kura().store_root().join("native-contexts");
        let mut changed = 0;
        for entry in std::fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            if entry
                .file_name()
                .to_string_lossy()
                .starts_with("00000000000000000002-")
            {
                std::fs::write(entry.path(), b"corrupt original archive").unwrap();
                changed += 1;
            }
        }
        assert_eq!(changed, 1);
        assert!(original_source(&chain.state().view(), 2).is_err());
        assert!(kagemusha_finality_source(&chain.state().view(), 2).is_err());
    }

    #[test]
    fn canonical_archive_lane_substitutions_fail_against_actual_native_result() {
        use crate::state::NativeExecutionProjectionV1;
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(crate::state::World::new(), 1_000))
                .unwrap();
        chain.commit(Vec::new());
        assert!(original_source(&chain.state().view(), 2).is_ok());
        let records = std::fs::read_dir(chain.kura().store_root().join("native-contexts"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("00000000000000000002-")
            })
            .collect::<Vec<_>>();
        assert_eq!(records.len(), 1);
        let path = &records[0];
        let original = std::fs::read(path).unwrap();
        let projection: NativeExecutionProjectionV1 = norito::decode_canonical_with_limits(
            &original,
            norito::canonical_decode_limits(original.len()),
        )
        .unwrap();
        for change in 0..4 {
            let mut changed = projection.clone();
            match change {
                0 => changed.lanes.incarnations += 1,
                1 => changed.lanes.last_transition += 1,
                2 => changed.carrier_height = 3,
                _ => {
                    changed.carrier_hash = iroha_crypto::HashOf::from_untyped_unchecked(
                        iroha_crypto::Hash::new(b"foreign carrier"),
                    )
                }
            }
            std::fs::write(path, norito::encode_canonical(&changed).unwrap()).unwrap();
            assert!(
                original_source(&chain.state().view(), 2).is_err(),
                "changed canonical field {change}"
            );
        }
        // Keep the complete canonical outer frame/checksum but replace the lane field with
        // malformed payload bytes. Framing cannot authenticate a raw lane claim.
        let view = norito::core::from_bytes_view(&original).unwrap();
        for malformed in [&[][..], &[0xFF][..]] {
            let mut payload = view.as_bytes();
            let mut replaced = Vec::new();
            for field_index in 0..5 {
                let (length, prefix) =
                    norito::core::read_len_from_slice_with_flags(payload, view.flags()).unwrap();
                let value = &payload[prefix..prefix + length];
                payload = &payload[prefix + length..];
                let value = if field_index == 2 { malformed } else { value };
                norito::core::write_len_with_flags(&mut replaced, value.len() as u64, view.flags())
                    .unwrap();
                replaced.extend_from_slice(value);
            }
            assert!(payload.is_empty());
            let frame = norito::core::frame_bare_with_header_flags::<NativeExecutionProjectionV1>(
                &replaced,
                view.flags(),
            )
            .unwrap();
            std::fs::write(path, frame).unwrap();
            assert!(original_source(&chain.state().view(), 2).is_err());
            assert!(kagemusha_finality_source(&chain.state().view(), 2).is_err());
        }
        std::fs::write(path, original).unwrap();
        assert!(
            original_source(&chain.state().view(), 2).is_ok(),
            "only the original native-bound bytes restore the source"
        );
    }
}
