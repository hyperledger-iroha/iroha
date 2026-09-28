//! Native source-complete paired-Pasta Commit attestations and original-execution publication.
//!
//! A verifier consumes only the exact core signing statement, its canonical result preimage
//! and one compact paired signature. The result's complete epoch must hash to the context the
//! core already authenticated for that height. No local execution cache, World lookup, quorum
//! subset or obsolete height context grants verification authority.

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    block::consensus_v2::HeightContextId,
    isi::kagemusha_v1::{
        KAGEMUSHA_CHAIN_VERSION_V1, KagemushaMintFinalitySealMessageV1,
        KagemushaMintFinalityValidatorSealV1, KagemushaPastaSchnorrSignatureV1,
        kagemusha_mint_finality_root_v1,
    },
};
use iroha_sumeragi::{
    crypto::{AttestOutcome, AttestationVerifier, Attestor},
    message::{AttestationSignature, BlockHeader, CommitAttestation, ResultWitness},
    preimage::{AttestationStatement, att_preimage},
    types::{Hash32, PublicKey, ValidatorIndex},
};
use mv::allocation::{
    AllocationBudget, AllocationRefusal, ChargedBuffer, ChargedShared, PrepaidSharedError,
    RetainedPayload,
};
use sha2::{Digest as _, Sha256};
use std::sync::{Mutex, TryLockError};

use super::{
    commitment::{ExecutionResultCommitment, result_of_preimage},
    crypto::core_key,
};
use crate::zk::kagemusha_v1_recursion::{
    KagemushaMintFinalityLocalAuthorityV1, kagemusha_mint_finality_empty_root_v1,
    verify_kagemusha_mint_finality_validator_seal_v1,
};

const SUBJECT_DOMAIN: &[u8] = b"iroha:native-pasta-commit:v1";
const SEAL_BYTES: usize = 4 + 4 * 32;

/// Pure verifier pinned to the actual network and native consensus instance.
#[derive(Clone, Copy, Debug)]
pub struct NativePastaVerifier {
    instance: Hash32,
    network: NetworkId,
}
impl NativePastaVerifier {
    /// Pin the verifier to the configured instance and signed-genesis network.
    #[must_use]
    pub const fn new(instance: Hash32, network: NetworkId) -> Self {
        Self { instance, network }
    }
}
impl AttestationVerifier for NativePastaVerifier {
    fn verify(
        &self,
        height: u64,
        signer: ValidatorIndex,
        key: &PublicKey,
        statement: &[u8],
        witness: &ResultWitness,
        signature: &[u8],
    ) -> bool {
        let Some(source) = AttestationStatement::parse(statement) else {
            return false;
        };
        if source.instance != self.instance
            || source.height != height
            || result_of_preimage(witness.as_slice()) != source.result
        {
            return false;
        }
        let Ok(result) = ExecutionResultCommitment::decode(witness.as_slice()) else {
            return false;
        };
        let Ok(message) =
            message_from_result(self.instance, self.network, source, statement, &result)
        else {
            return false;
        };
        let Some(member) = result.schedule.current.committee.get(signer as usize) else {
            return false;
        };
        if core_key(member.validator.public_key()).ok().as_ref() != Some(key) {
            return false;
        }
        let Some(seal) = decode_seal(signature) else {
            return false;
        };
        seal.validator_index == signer
            && verify_kagemusha_mint_finality_validator_seal_v1(
                &result.schedule.current.authority,
                &message,
                &seal,
            )
            .is_ok()
    }
}

/// Why an original execution cannot produce a native Pasta receipt.
#[derive(Debug, thiserror::Error)]
pub enum NativeAttestationError {
    /// Original result, preimage, immutable witness or shared control has another source.
    #[error("native attestation differs from its original execution source")]
    Source,
    /// The exact native header or execution projection does not define one valid statement.
    #[error("invalid native attestation statement: {0}")]
    Statement(String),
    /// The provisioned private custody does not own this exact authenticated generation seat.
    #[error("native Pasta custody cannot sign this generation: {0}")]
    Custody(String),
    /// Original pool refused the actual mailbox control allocation.
    #[error(transparent)]
    Admission(AllocationRefusal),
    /// The admitted mailbox control allocation was physically refused.
    #[error(transparent)]
    Allocator(PrepaidSharedError),
}

/// Reconstruct the unique recursive-mint message from one source-complete native statement.
/// This performs no finality check: callers must authenticate the enclosing exact-quorum
/// certificate and epoch separately. It never signs or consults local state.
///
/// # Errors
/// Rejects malformed canonical R, another instance/network/epoch/height, changed result bytes
/// or an inconsistent top-up projection. A flagged rejected top-up has the canonical empty root.
pub fn native_seal_message(
    instance: Hash32,
    network: NetworkId,
    statement: &[u8],
    result_preimage: &[u8],
) -> Result<KagemushaMintFinalitySealMessageV1, NativeAttestationError> {
    let source = AttestationStatement::parse(statement).ok_or(NativeAttestationError::Source)?;
    if result_of_preimage(result_preimage) != source.result {
        return Err(NativeAttestationError::Source);
    }
    let result = ExecutionResultCommitment::decode(result_preimage)
        .map_err(|error| NativeAttestationError::Statement(error.to_string()))?;
    message_from_result(instance, network, source, statement, &result)
}

fn message_from_result(
    instance: Hash32,
    network: NetworkId,
    source: AttestationStatement,
    statement: &[u8],
    result: &ExecutionResultCommitment,
) -> Result<KagemushaMintFinalitySealMessageV1, NativeAttestationError> {
    let invalid = |reason: String| NativeAttestationError::Statement(reason);
    result
        .validate()
        .map_err(|error| invalid(error.to_string()))?;
    let current = &result.schedule.current;
    if source.instance != instance
        || source.height != result.height
        || current.network_id != network
        || source.epoch.epoch != current.authorization.epoch
        || source.epoch.context.0 != current.context_id().map_err(invalid)?
    {
        return Err(NativeAttestationError::Source);
    }
    let next = result
        .schedule
        .boundary
        .as_ref()
        .map(|boundary| boundary.next.authorization);
    let (count, root) = (
        result.execution.kagemusha_top_up_count,
        result.execution.kagemusha_top_up_root,
    );
    let root = match (count, root) {
        (0, None) => kagemusha_mint_finality_root_v1(
            kagemusha_mint_finality_empty_root_v1().map_err(|error| invalid(error.to_string()))?,
        ),
        (count, Some(root)) if count > 0 => root,
        _ => {
            return Err(invalid(
                "required commit has inconsistent top-up count and root".into(),
            ));
        }
    };
    let mut digest = Sha256::new();
    digest.update(SUBJECT_DOMAIN);
    digest.update([0]);
    digest.update(statement);
    let message = KagemushaMintFinalitySealMessageV1 {
        version: KAGEMUSHA_CHAIN_VERSION_V1,
        epoch_authorization: current.authorization,
        validator_count: u32::try_from(current.authority.validators.len())
            .map_err(|_| NativeAttestationError::Source)?,
        network_id: network,
        block_height: source.height,
        height_context_id: HeightContextId(HashOf::from_untyped_unchecked(Hash::prehashed(
            source.epoch.context.0,
        ))),
        subject_digest: digest.finalize().into(),
        execution_commitment_digest: source.result.0,
        kagemusha_top_up_root: root,
        kagemusha_top_up_count: count,
        next_epoch_authorization: next,
    };
    message
        .validate()
        .map_err(|error| invalid(error.to_string()))?;
    Ok(message)
}

// The sole compact share encoding is index_be32 followed by Eq nonce/response and Ep
// nonce/response. Canonical scalar/point validation belongs to the genuine Pasta verifier.
/// Encode the sole compact native paired seal, including its exact canonical seat index.
#[must_use]
pub fn encode_native_seal(seal: KagemushaMintFinalityValidatorSealV1) -> AttestationSignature {
    let mut bytes = [0; SEAL_BYTES];
    bytes[..4].copy_from_slice(&seal.validator_index.to_be_bytes());
    for (chunk, value) in bytes[4..].chunks_exact_mut(32).zip([
        seal.eq_proof_signature.nonce_commitment,
        seal.eq_proof_signature.response,
        seal.ep_proof_signature.nonce_commitment,
        seal.ep_proof_signature.response,
    ]) {
        chunk.copy_from_slice(&value);
    }
    AttestationSignature::try_from_slice(&bytes)
        .expect("fixed paired signature fits protocol bound")
}
fn decode_seal(bytes: &[u8]) -> Option<KagemushaMintFinalityValidatorSealV1> {
    if bytes.len() != SEAL_BYTES {
        return None;
    }
    let mut fields = bytes[4..].chunks_exact(32);
    Some(KagemushaMintFinalityValidatorSealV1 {
        validator_index: u32::from_be_bytes(bytes[..4].try_into().ok()?),
        eq_proof_signature: KagemushaPastaSchnorrSignatureV1 {
            nonce_commitment: fields.next()?.try_into().ok()?,
            response: fields.next()?.try_into().ok()?,
        },
        ep_proof_signature: KagemushaPastaSchnorrSignatureV1 {
            nonce_commitment: fields.next()?.try_into().ok()?,
            response: fields.next()?.try_into().ok()?,
        },
    })
}

/// Move-only proof that the native worker signed its exact retained original execution.
/// Private fields prevent a decoded or remote frame from minting a local execution receipt.
pub(crate) struct LocalCommitAttestation {
    source: AttestationStatement,
    key: [u8; 48],
    attestation: CommitAttestation,
}

impl LocalCommitAttestation {
    /// Borrow the witness already retained by a genuine original receipt; grants no capability.
    #[cfg(test)]
    pub(crate) fn witness_for_test(&self) -> &ResultWitness {
        &self.attestation.witness
    }
}

/// Sign only the exact original retained result and separately admitted identical witness.
/// The caller retains every owner across local refusal, then publishes this receipt before Valid.
///
/// # Errors
/// Returns the same witness owner on every source, statement or custody failure.
pub(crate) fn attest_original(
    custody: &KagemushaMintFinalityLocalAuthorityV1,
    verifier: NativePastaVerifier,
    header: &BlockHeader,
    result: &RetainedPayload<ExecutionResultCommitment>,
    preimage: &ChargedBuffer<u8>,
    witness: ResultWitness,
    budget: &AllocationBudget,
) -> Result<LocalCommitAttestation, (ResultWitness, NativeAttestationError)> {
    let checked = (|| {
        if !header.attest
            || !result.belongs_to(budget)
            || !preimage.belongs_to(budget)
            || !witness.admitted_to(budget)
            || preimage.as_slice() != witness.as_slice()
        {
            return Err(NativeAttestationError::Source);
        }
        let source = AttestationStatement {
            instance: header.instance,
            epoch: header.epoch,
            height: header.height,
            block_hash: header.hash(&super::crypto::BlsCrypto::new()),
            result: result_of_preimage(preimage.as_slice()),
        };
        // The private retained execution owner must be the exact canonical preimage being signed.
        // Count/stream compare through a nonallocating writer rather than cloning/encoding R.
        let mut compare = CompareWriter {
            expected: preimage.as_slice(),
            used: 0,
        };
        norito::core::write_canonical_to_writer(result.get(), &mut compare)
            .map_err(|error| NativeAttestationError::Statement(error.to_string()))?;
        if compare.used != preimage.as_slice().len() {
            return Err(NativeAttestationError::Source);
        }
        let statement = att_preimage(
            &source.instance,
            &source.epoch,
            source.height,
            &source.block_hash,
            &source.result,
        );
        let message = message_from_result(
            verifier.instance,
            verifier.network,
            source,
            &statement,
            result.get(),
        )?;
        let signer = custody
            .signer_for_authority(&result.get().schedule.current.authority)
            .map_err(|error| NativeAttestationError::Custody(error.to_string()))?;
        let member = result
            .get()
            .schedule
            .current
            .committee
            .get(signer.validator_index() as usize)
            .ok_or(NativeAttestationError::Source)?;
        let key =
            core_key(member.validator.public_key()).map_err(|_| NativeAttestationError::Source)?;
        let key: [u8; 48] = key
            .as_bytes()
            .try_into()
            .map_err(|_| NativeAttestationError::Source)?;
        let signature = encode_native_seal(
            signer
                .sign(&message)
                .map_err(|error| NativeAttestationError::Custody(error.to_string()))?,
        );
        Ok((source, key, signature))
    })();
    match checked {
        Ok((source, key, signature)) => Ok(LocalCommitAttestation {
            source,
            key,
            attestation: CommitAttestation { witness, signature },
        }),
        Err(error) => Err((witness, error)),
    }
}
struct CompareWriter<'a> {
    expected: &'a [u8],
    used: usize,
}
impl std::io::Write for CompareWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let end = self
            .used
            .checked_add(bytes.len())
            .ok_or(std::io::ErrorKind::InvalidData)?;
        if self.expected.get(self.used..end) != Some(bytes) {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        self.used = end;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct Mailbox {
    instance: Hash32,
    key: [u8; 48],
    provisioned: bool,
    budget: AllocationBudget,
    receipt: Mutex<Option<LocalCommitAttestation>>,
}
/// Nonblocking core adapter. It only borrows already-signed original execution receipts.
pub(crate) struct NativePastaAttestor {
    mailbox: ChargedShared<Mailbox>,
}
/// Sole serialized native worker publisher, sharing the original funded mailbox control.
pub(crate) struct NativeAttestationPublisher {
    mailbox: ChargedShared<Mailbox>,
}

/// Admit one process-lived bounded mailbox from the original State pool.
///
/// # Errors
/// Rejects a malformed consensus key or original-pool control allocation refusal.
pub(crate) fn channel(
    instance: Hash32,
    key: &PublicKey,
    provisioned: bool,
    budget: &AllocationBudget,
) -> Result<(NativePastaAttestor, NativeAttestationPublisher), NativeAttestationError> {
    let key = key
        .as_bytes()
        .try_into()
        .map_err(|_| NativeAttestationError::Source)?;
    let mut reservation = budget
        .try_reserve(ChargedShared::<Mailbox>::allocation_layout())
        .map_err(NativeAttestationError::Admission)?;
    let mailbox = ChargedShared::from_reservation(
        Mailbox {
            instance,
            key,
            provisioned,
            budget: budget.clone(),
            receipt: Mutex::new(None),
        },
        &mut reservation,
    )
    .map_err(|(_, error)| NativeAttestationError::Allocator(error))?;
    Ok((
        NativePastaAttestor {
            mailbox: mailbox.clone(),
        },
        NativeAttestationPublisher { mailbox },
    ))
}
/// Publication refusal separates transient contention from terminal source/poison failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AttestationPublishError {
    /// The exact original receipt does not belong to this mailbox.
    Source,
    /// A short concurrent nonblocking read currently owns the mailbox lock.
    Busy,
    /// A previous panic poisoned the publication owner; recovery is required.
    Poisoned,
}
impl NativeAttestationPublisher {
    /// Exact local consensus key; observers must skip signing outside their authenticated seat.
    pub(crate) fn key(&self) -> &[u8; 48] {
        &self.mailbox.key
    }

    /// Hold the actual mailbox lock while a regression exercises nonblocking publication.
    /// This changes no outcome and injects no receipt; callers must retry after the guard drops.
    #[cfg(test)]
    pub(crate) fn with_locked_receipt_for_test<T>(&self, action: impl FnOnce() -> T) -> T {
        let _guard = self
            .mailbox
            .receipt
            .lock()
            .expect("fixture mailbox is not poisoned");
        action()
    }

    /// Publish the same original source, preserving the receipt on every typed refusal.
    pub(crate) fn publish(
        &self,
        receipt: LocalCommitAttestation,
    ) -> Result<(), (LocalCommitAttestation, AttestationPublishError)> {
        if receipt.source.instance != self.mailbox.instance
            || receipt.key != self.mailbox.key
            || !receipt
                .attestation
                .witness
                .admitted_to(&self.mailbox.budget)
        {
            return Err((receipt, AttestationPublishError::Source));
        }
        let mut slot = match self.mailbox.receipt.try_lock() {
            Ok(slot) => slot,
            Err(TryLockError::WouldBlock) => return Err((receipt, AttestationPublishError::Busy)),
            Err(TryLockError::Poisoned(_)) => {
                return Err((receipt, AttestationPublishError::Poisoned));
            }
        };
        *slot = Some(receipt);
        Ok(())
    }
    /// Discard a receipt before dropping its corresponding unretained original execution.
    pub(crate) fn discard(&self, height: u64, keep: &[Hash32]) -> bool {
        let Ok(mut slot) = self.mailbox.receipt.lock() else {
            return false;
        };
        if slot.as_ref().is_some_and(|receipt| {
            receipt.source.height != height || !keep.contains(&receipt.source.block_hash)
        }) {
            *slot = None;
        }
        true
    }
}
impl NativePastaAttestor {
    /// Hold the actual shared mailbox lock while the attached worker retries publication.
    #[cfg(test)]
    pub(crate) fn with_locked_receipt_for_test<T>(&self, action: impl FnOnce() -> T) -> T {
        let _guard = self
            .mailbox
            .receipt
            .lock()
            .expect("fixture mailbox is not poisoned");
        action()
    }
}
impl Attestor for NativePastaAttestor {
    fn attest(&self, height: u64, key: &PublicKey, statement: &[u8]) -> AttestOutcome {
        if !self.mailbox.provisioned || key.as_bytes() != self.mailbox.key {
            return AttestOutcome::NoAuthority;
        }
        let Some(source) = AttestationStatement::parse(statement) else {
            return AttestOutcome::NoAuthority;
        };
        if source.instance != self.mailbox.instance || source.height != height {
            return AttestOutcome::NoAuthority;
        }
        let slot = match self.mailbox.receipt.try_lock() {
            Ok(slot) => slot,
            Err(TryLockError::WouldBlock) => return AttestOutcome::Pending,
            Err(TryLockError::Poisoned(_)) => return AttestOutcome::NoAuthority,
        };
        match slot.as_ref().filter(|receipt| receipt.source == source) {
            Some(receipt) => AttestOutcome::Attested(receipt.attestation.clone()),
            None => AttestOutcome::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sumeragi::{
        certified_chain::CertifiedChain,
        test_chain::{CertifiedTestChain, Signers},
    };
    use iroha_sumeragi::crypto::verify_qc;

    #[test]
    fn actual_empty_boundary_pasta_is_source_complete_and_subset_independent() {
        let mut chain = CertifiedTestChain::npos_boundary_fixture();
        chain.commit(Vec::new());
        let view = chain.state().view();
        let reader = CertifiedChain::new(&view).unwrap();
        let certified = reader.certified(10).unwrap();
        let header = certified.header().unwrap();
        assert!(header.attest);
        assert_eq!(header.payload_len, 0);
        let qc = certified.commit_qc().unwrap();
        let current = &certified.commitment().schedule.current;
        let config = super::super::schedule::ScheduledConfig {
            height: 10,
            epoch: current.clone(),
            params: certified.commitment().schedule.next.params().clone(),
        }
        .height_config()
        .unwrap();
        let crypto = crate::sumeragi::crypto::BlsCrypto::new();
        crypto
            .admit_committee(
                current
                    .committee
                    .iter()
                    .map(|m| (m.validator.public_key(), m.proof_of_possession.as_slice())),
            )
            .unwrap();
        let verifier = NativePastaVerifier::new(chain.instance(), chain.network_id());
        verify_qc(
            &crypto,
            &verifier,
            &chain.instance(),
            &header.epoch,
            &config.committee,
            qc,
        )
        .unwrap();
        let other = chain.commit_qc(
            10,
            certified.core_hash(),
            certified.result(),
            true,
            Signers::LastThree,
        );
        verify_qc(
            &crypto,
            &verifier,
            &chain.instance(),
            &header.epoch,
            &config.committee,
            &other,
        )
        .unwrap();
        assert_ne!(qc.signers, other.signers);
        assert_eq!(qc.attestation_witness, other.attestation_witness);
        let witness = qc.attestation_witness.as_ref().unwrap();
        let statement = qc.statement();
        let key = core_key(current.committee[0].validator.public_key()).unwrap();
        let signature = qc.attestations[0].as_slice();
        assert!(verifier.verify(10, 0, &key, &statement, witness, signature));
        assert!(!verifier.verify(11, 0, &key, &statement, witness, signature));
        assert!(!verifier.verify(10, 1, &key, &statement, witness, signature));
        let wrong_key = core_key(current.committee[1].validator.public_key()).unwrap();
        assert!(!verifier.verify(10, 0, &wrong_key, &statement, witness, signature));
        for index in [0, 4, 36, 68, 100] {
            let mut changed = signature.to_vec();
            changed[index] ^= 1;
            assert!(!verifier.verify(10, 0, &key, &statement, witness, &changed));
        }
        let mut changed = witness.as_slice().to_vec();
        let last = changed.len() - 1;
        changed[last] ^= 1;
        let changed = ResultWitness::from_untrusted(changed).unwrap();
        assert!(!verifier.verify(10, 0, &key, &statement, &changed, signature));
        let mut source = AttestationStatement::parse(&statement).unwrap();
        source.epoch.epoch += 1;
        let changed = att_preimage(
            &source.instance,
            &source.epoch,
            source.height,
            &source.block_hash,
            &source.result,
        );
        assert!(!verifier.verify(10, 0, &key, &changed, witness, signature));
        let foreign = NativePastaVerifier::new(Hash32([0x77; 32]), chain.network_id());
        assert!(!foreign.verify(10, 0, &key, &statement, witness, signature));
    }

    #[test]
    fn rejected_flagged_top_up_commits_empty_result_but_cannot_authorize_mint() {
        use crate::state::World;
        use crate::sumeragi::test_chain::TestChainConfig;
        use iroha_crypto::{Algorithm, KeyPair};
        use iroha_data_model::{
            account::AccountId,
            isi::kagemusha_v1::{KagemushaTopUpLeafV1, TopUpKagemushaV1},
        };
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
        let stranger = KeyPair::from_seed(vec![0xF1; 32], Algorithm::Ed25519);
        let request = crate::queue::kagemusha_top_up_admission_tests::fixture_top_up_request(
            AccountId::new(stranger.public_key().clone()),
            chain.network_id(),
        );
        // This request has no release authority and its payer is unregistered. Its static
        // instruction still mandates Pasta attestations, although execution creates no leaf.
        let transaction = chain.sign(
            &stranger,
            [TopUpKagemushaV1::new(request).unwrap().into()],
            1_001,
        );
        assert_eq!(chain.commit(vec![transaction]), [false]);
        let view = chain.state().view();
        let reader = CertifiedChain::new(&view).unwrap();
        let certified = reader.certified(2).unwrap();
        assert!(certified.header().unwrap().attest);
        assert_eq!(certified.commitment().execution.kagemusha_top_up_count, 0);
        assert!(
            certified
                .commitment()
                .execution
                .kagemusha_top_up_root
                .is_none()
        );
        assert!(certified.commitment().schedule.boundary.is_none());
        let qc = certified.commit_qc().unwrap();
        let message = native_seal_message(
            chain.instance(),
            chain.network_id(),
            &qc.statement(),
            qc.attestation_witness.as_ref().unwrap().as_slice(),
        )
        .unwrap();
        assert_eq!(message.kagemusha_top_up_count, 0);
        assert_eq!(
            message.kagemusha_top_up_root,
            kagemusha_mint_finality_root_v1(kagemusha_mint_finality_empty_root_v1().unwrap())
        );
        let leaf = KagemushaTopUpLeafV1 {
            version: KAGEMUSHA_CHAIN_VERSION_V1,
            operation_id: [1; 32],
            reserve_receipt_digest: [2; 32],
            statement_digest: [3; 32],
            amount: 1,
        };
        let tree =
            crate::zk::kagemusha_v1_recursion::KagemushaMintFinalityTreeV1::new(vec![leaf.clone()])
                .unwrap();
        let witness = tree.witness(leaf.operation_id).unwrap();
        assert!(
            crate::zk::kagemusha_v1_recursion::verify_kagemusha_top_up_membership_v1(&witness, 1)
                .is_ok()
        );
        assert!(
            crate::zk::kagemusha_v1_recursion::verify_kagemusha_top_up_membership_v1(
                &witness,
                message.kagemusha_top_up_count
            )
            .is_err()
        );
        assert!(witness.validate_against(&message).is_err());
    }

    #[test]
    fn actual_mailbox_contention_retains_original_receipt_and_source() {
        let budget = AllocationBudget::new(1 << 20);
        let key = PublicKey::new(vec![7; 48]).unwrap();
        let source = AttestationStatement {
            instance: Hash32([1; 32]),
            epoch: iroha_sumeragi::testing::TEST_EPOCH.id,
            height: 10,
            block_hash: Hash32([2; 32]),
            result: Hash32([3; 32]),
        };
        let (attestor, publisher) = channel(source.instance, &key, true, &budget).unwrap();
        let mut backing = ChargedBuffer::new(4, &budget).unwrap();
        backing.append(&[1, 2, 3, 4]).unwrap();
        let pointer = backing.as_slice().as_ptr();
        let witness = ResultWitness::from_charged(backing, &budget).unwrap();
        let receipt = LocalCommitAttestation {
            source,
            key: [7; 48],
            attestation: CommitAttestation {
                witness,
                signature: AttestationSignature::try_from_slice(&[9]).unwrap(),
            },
        };
        let (receipt, error) = attestor
            .with_locked_receipt_for_test(|| publisher.publish(receipt))
            .err()
            .unwrap();
        assert_eq!(error, AttestationPublishError::Busy);
        assert_eq!(receipt.attestation.witness.as_slice().as_ptr(), pointer);
        assert!(publisher.publish(receipt).is_ok());
        let statement = att_preimage(
            &source.instance,
            &source.epoch,
            source.height,
            &source.block_hash,
            &source.result,
        );
        let AttestOutcome::Attested(share) = attestor.attest(10, &key, &statement) else {
            panic!("original receipt")
        };
        assert_eq!(share.witness.as_slice().as_ptr(), pointer);
        assert!(share.witness.admitted_to(&budget));
        assert!(publisher.discard(11, &[]));
        assert!(matches!(
            attestor.attest(10, &key, &statement),
            AttestOutcome::Pending
        ));
        assert_eq!(
            share.witness.as_slice(),
            &[1, 2, 3, 4],
            "retained vote still owns its real backing"
        );
    }
}
