//! Attestation bundle verification (spec §5.1.2, §5.1.3, §5.1.8, §7.1 step 6).
//!
//! A wallet never trusts Torii: before it pays for a destination transaction it recomputes,
//! from the bundle's fields alone, everything the destination contract will check, against the
//! roster state it read from the destination itself:
//!
//! 1. the statement's §3.6.1 invariants and its EIP-712 digest under the Taira `NetworkId`
//!    (a bundle for another Taira network never matches);
//! 2. the roster digest, recomputed with the `n`, `t` and ordering checks of §3.7, equal to the
//!    view's digest and to the statement's `rosterDigest`;
//! 3. acceptance of that roster by the destination (§5.1.2): the current roster until its
//!    `validUntilMs`, or the previous one until its grace-capped `prevValidUntilMs`;
//! 4. the §3.8 signature set: every set bit addresses a nonzero member that the signature
//!    recovers to, bits `≥ n` are clear, and at least `t` bits are set;
//! 5. the Merkle path of the transfer leaf, computed from the payload and the destination's own
//!    destination word, directly against the attested block or through the history root;
//! 6. the payload rules of the destination: a Taira → destination payload for its route id and
//!    revision, a recipient valid for its codec that is not the contract itself, and the
//!    deadline (`now_ms ≤ deadline_ms` to finalize, `now_ms > deadline_ms` to void).
//!
//! The verified result carries the signature set trimmed to exactly `t` signatures (the lowest
//! set bits), which is what the calldata builders in [`super::evm`] encode.

use core::fmt;

use iroha_data_model::sccp::attestation::SccpAttestationStatementV1;
use iroha_sccp::{
    api::{ApiShapeError, SccpMessageProofBundleV1, SccpRosterViewV1, SccpSignatureSetV1},
    v1::{
        constants::SIGNATURE_BYTES,
        eip712::{AttestationFieldsV1, AttestationInvariantError},
        evm_abi::AttestedV1,
        proof::{
            DestinationV1, HistoryProofV1, MessageProofV1, ProofError, VerifiedTransferV1,
            verify_transfer_direct, verify_transfer_historical,
        },
        roster::{RosterError, RosterStateV1, RosterV1},
        signature::{SignatureError, SignatureSetV1},
    },
};

/// Everything the wallet read from one destination deployment before verifying a bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DestinationContextV1 {
    /// Taira `NetworkId` bytes: the EIP-712 salt and the Taira identity word.
    pub taira_network_id: [u8; 32],
    /// The deployment: network, route revision and destination word.
    pub destination: DestinationV1,
    /// The destination's `rosterState()`.
    pub roster_state: RosterStateV1,
    /// Destination time the transaction is expected to execute at (`block.timestamp × 1000`).
    pub now_ms: u64,
}

/// Which destination roster accepts an attestation (§5.1.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AcceptedRosterV1 {
    /// The current roster, before its `validUntilMs`.
    Current,
    /// The previous roster, within its grace-capped `prevValidUntilMs`.
    Previous,
}

/// What a message bundle is verified for: the two sides of the deadline (§3.2, §5.1.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BundlePurposeV1 {
    /// `finalizeFromTaira*`: minting requires `now_ms ≤ deadline_ms`.
    Finalize,
    /// `voidExpired*`: voiding requires `now_ms > deadline_ms`.
    VoidExpired,
}

/// Why a bundle, a statement or its signatures failed verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BundleError {
    /// A field exceeds a protocol bound.
    Shape(ApiShapeError),
    /// The statement breaks a §3.6.1 invariant.
    Statement(AttestationInvariantError),
    /// The claimed EIP-712 digest differs from the digest of the statement's fields under the
    /// Taira `NetworkId`.
    DigestMismatch,
    /// The roster breaks a §3.7 rule (`n`, `t`, generation or ordering).
    Roster(RosterError),
    /// The roster's claimed digest differs from its recomputed digest.
    RosterDigestMismatch,
    /// The roster is not the one the statement names in `rosterDigest`.
    StatementRosterMismatch,
    /// The destination does not accept the roster at `now_ms` (unknown or expired).
    RosterNotAccepted,
    /// A signature or the bitmap is invalid.
    Signatures(SignatureError),
    /// Fewer than `t` valid signatures.
    TooFewSignatures {
        /// Valid signatures supplied (at most 31).
        count: u8,
        /// Required threshold `t` (at most 21).
        threshold: u8,
    },
    /// The bundle's `message_count` differs from the leaf count of the block it proves against.
    MessageCountMismatch,
    /// The payload, leaf or Merkle path fails the destination checks.
    Proof(ProofError),
    /// The bundle's `message_id` differs from the payload's message id.
    MessageIdMismatch,
    /// The bundle's `deadline_ms` differs from the payload's.
    DeadlineMismatch,
    /// `now_ms > deadline_ms`: the message can no longer be minted.
    DeadlinePassed,
    /// `now_ms ≤ deadline_ms`: the message cannot be voided yet.
    DeadlineNotReached,
}

impl fmt::Display for BundleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Shape(error) => write!(formatter, "bundle shape: {error}"),
            Self::Statement(error) => write!(formatter, "attestation statement: {error}"),
            Self::DigestMismatch => formatter.write_str(
                "attestation digest differs from the statement under this Taira network id",
            ),
            Self::Roster(error) => write!(formatter, "roster: {error}"),
            Self::RosterDigestMismatch => {
                formatter.write_str("roster digest differs from the recomputed roster digest")
            }
            Self::StatementRosterMismatch => {
                formatter.write_str("roster is not the roster named by the statement")
            }
            Self::RosterNotAccepted => {
                formatter.write_str("the destination does not accept this roster now")
            }
            Self::Signatures(error) => write!(formatter, "signature set: {error}"),
            Self::TooFewSignatures { count, threshold } => write!(
                formatter,
                "signature set has {count} valid signatures; the threshold is {threshold}"
            ),
            Self::MessageCountMismatch => {
                formatter.write_str("message_count differs from the leaf count of the proven block")
            }
            Self::Proof(error) => write!(formatter, "message proof: {error}"),
            Self::MessageIdMismatch => {
                formatter.write_str("message_id differs from the payload's message id")
            }
            Self::DeadlineMismatch => {
                formatter.write_str("deadline_ms differs from the payload's deadline")
            }
            Self::DeadlinePassed => formatter.write_str("the mint deadline has passed"),
            Self::DeadlineNotReached => formatter.write_str("the mint deadline has not passed"),
        }
    }
}

impl std::error::Error for BundleError {}

impl From<ApiShapeError> for BundleError {
    fn from(error: ApiShapeError) -> Self {
        Self::Shape(error)
    }
}

impl From<RosterError> for BundleError {
    fn from(error: RosterError) -> Self {
        Self::Roster(error)
    }
}

impl From<ProofError> for BundleError {
    fn from(error: ProofError) -> Self {
        Self::Proof(error)
    }
}

/// A statement whose digest, roster and signature quorum were verified, independent of any
/// destination state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedAttestationV1 {
    /// The contract-visible attestation.
    pub attestation: AttestationFieldsV1,
    /// Its EIP-712 digest.
    pub digest: [u8; 32],
    /// The roster that signed it.
    pub roster: RosterV1,
    /// Exactly `t` of its valid signatures (the lowest set bits).
    pub signatures: SignatureSetV1,
}

/// A signed attestation that the destination accepts at `now_ms`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedAttestationV1 {
    /// The verified statement, roster and trimmed signatures.
    pub signed: SignedAttestationV1,
    /// Which destination roster accepts it.
    pub accepted_as: AcceptedRosterV1,
}

impl VerifiedAttestationV1 {
    /// The common `(attestation, roster, signatures)` prefix of every finalize, control and
    /// `voidExpired*` call.
    #[must_use]
    pub fn attested(&self) -> AttestedV1<'_> {
        AttestedV1 {
            attestation: &self.signed.attestation,
            roster: &self.signed.roster,
            signatures: &self.signed.signatures,
        }
    }
}

/// A message bundle verified for one destination and purpose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedMessageBundleV1 {
    /// The accepted attestation with trimmed signatures.
    pub attested: VerifiedAttestationV1,
    /// `MessageProofV1` for the calldata.
    pub proof: MessageProofV1,
    /// `HistoryProofV1` in historical mode.
    pub history: Option<HistoryProofV1>,
    /// Decoded payload, message id and transfer leaf.
    pub transfer: VerifiedTransferV1,
    /// The purpose the deadline was checked for.
    pub purpose: BundlePurposeV1,
}

impl VerifiedMessageBundleV1 {
    /// Whether the message is proven through the history root.
    #[must_use]
    pub fn is_historical(&self) -> bool {
        self.history.is_some()
    }

    /// The payload's destination nonce (the consumed-set bit).
    #[must_use]
    pub fn nonce(&self) -> u64 {
        self.transfer.payload.nonce
    }

    /// The payload's destination-time deadline.
    #[must_use]
    pub fn deadline_ms(&self) -> u64 {
        self.transfer.payload.deadline_ms
    }
}

/// Keep the lowest `threshold` set bits of a signature set and their signatures.
///
/// # Errors
///
/// Returns [`SignatureError::TooFewSignatures`] for fewer than `threshold` set bits and
/// [`SignatureError::BadLength`] when the signatures are not 65 bytes per set bit.
pub fn trim_to_threshold(
    set: &SignatureSetV1,
    threshold: usize,
) -> Result<SignatureSetV1, SignatureError> {
    let count = set.popcount() as usize;
    if set.signatures.len() != count * SIGNATURE_BYTES {
        return Err(SignatureError::BadLength);
    }
    if count < threshold {
        return Err(SignatureError::TooFewSignatures);
    }
    let mut bitmap = 0_u32;
    let mut signatures = Vec::with_capacity(threshold * SIGNATURE_BYTES);
    let mut chunks = set.signatures.chunks_exact(SIGNATURE_BYTES);
    for bit in 0..32_u32 {
        if bitmap.count_ones() as usize == threshold {
            break;
        }
        if set.signer_bitmap & (1 << bit) == 0 {
            continue;
        }
        let chunk = chunks.next().ok_or(SignatureError::BadLength)?;
        bitmap |= 1 << bit;
        signatures.extend_from_slice(chunk);
    }
    Ok(SignatureSetV1 {
        signer_bitmap: bitmap,
        signatures,
    })
}

/// Verify a statement, the roster that signed it and its signature quorum (steps 1, 2 and 4 of
/// the module docs), without consulting destination state.
///
/// # Errors
///
/// Returns the first failing [`BundleError`].
pub fn verify_signed(
    statement: &SccpAttestationStatementV1,
    digest: &[u8; 32],
    roster: &SccpRosterViewV1,
    signatures: &SccpSignatureSetV1,
    taira_network_id: &[u8; 32],
) -> Result<SignedAttestationV1, BundleError> {
    roster.check_shape()?;
    signatures.check_shape()?;
    let attestation = AttestationFieldsV1::from(*statement);
    attestation
        .check_invariants()
        .map_err(BundleError::Statement)?;
    if attestation.digest(taira_network_id) != *digest {
        return Err(BundleError::DigestMismatch);
    }
    let contract_roster = roster.to_roster()?;
    let roster_digest = contract_roster.digest(taira_network_id)?;
    if roster_digest != roster.digest {
        return Err(BundleError::RosterDigestMismatch);
    }
    if roster_digest != attestation.roster_digest {
        return Err(BundleError::StatementRosterMismatch);
    }
    let set = signatures.to_signature_set();
    let count = set
        .verify(digest, &contract_roster.members)
        .map_err(BundleError::Signatures)?;
    let threshold = contract_roster.threshold();
    if (count as usize) < threshold {
        // Both fit a `u8`: a verified roster has at most 31 members.
        return Err(BundleError::TooFewSignatures {
            count: u8::try_from(count).unwrap_or(u8::MAX),
            threshold: u8::try_from(threshold).unwrap_or(u8::MAX),
        });
    }
    let signatures = trim_to_threshold(&set, threshold).map_err(BundleError::Signatures)?;
    Ok(SignedAttestationV1 {
        attestation,
        digest: *digest,
        roster: contract_roster,
        signatures,
    })
}

/// Which roster of `state` accepts `roster_digest` at `now_ms` (§5.1.2), if any.
#[must_use]
pub fn accepted_roster(
    state: &RosterStateV1,
    roster_digest: &[u8; 32],
    now_ms: u64,
) -> Option<AcceptedRosterV1> {
    if !state.accepts(roster_digest, now_ms) {
        return None;
    }
    if *roster_digest == state.digest && now_ms <= state.valid_until_ms {
        Some(AcceptedRosterV1::Current)
    } else {
        Some(AcceptedRosterV1::Previous)
    }
}

/// [`verify_signed`] plus acceptance of the roster by the destination (§5.1.2).
///
/// # Errors
///
/// Returns the first failing [`BundleError`].
pub fn verify_attested(
    statement: &SccpAttestationStatementV1,
    digest: &[u8; 32],
    roster: &SccpRosterViewV1,
    signatures: &SccpSignatureSetV1,
    context: &DestinationContextV1,
) -> Result<VerifiedAttestationV1, BundleError> {
    let signed = verify_signed(
        statement,
        digest,
        roster,
        signatures,
        &context.taira_network_id,
    )?;
    let accepted_as = accepted_roster(
        &context.roster_state,
        &signed.attestation.roster_digest,
        context.now_ms,
    )
    .ok_or(BundleError::RosterNotAccepted)?;
    Ok(VerifiedAttestationV1 {
        signed,
        accepted_as,
    })
}

/// Check the deadline side of `purpose` (§3.2, §5.1.3 step 4, §5.1.8).
///
/// # Errors
///
/// Returns [`BundleError::DeadlinePassed`] or [`BundleError::DeadlineNotReached`].
pub fn check_deadline(
    purpose: BundlePurposeV1,
    deadline_ms: u64,
    now_ms: u64,
) -> Result<(), BundleError> {
    match purpose {
        BundlePurposeV1::Finalize if now_ms > deadline_ms => Err(BundleError::DeadlinePassed),
        BundlePurposeV1::VoidExpired if now_ms <= deadline_ms => {
            Err(BundleError::DeadlineNotReached)
        }
        _ => Ok(()),
    }
}

/// Verify a message proof bundle for `purpose` against the destination (all six steps of the
/// module docs) and return the pieces of the destination call.
///
/// # Errors
///
/// Returns the first failing [`BundleError`].
pub fn verify_message_bundle(
    bundle: &SccpMessageProofBundleV1,
    context: &DestinationContextV1,
    purpose: BundlePurposeV1,
) -> Result<VerifiedMessageBundleV1, BundleError> {
    bundle.check_shape()?;
    let attested = verify_attested(
        &bundle.statement,
        &bundle.digest,
        &bundle.roster,
        &bundle.signatures,
        context,
    )?;
    let attestation = &attested.signed.attestation;
    let proof = bundle.message_proof();
    let history = bundle.history_proof();
    let transfer = match &history {
        None => {
            if bundle.message_count != attestation.message_count {
                return Err(BundleError::MessageCountMismatch);
            }
            verify_transfer_direct(
                attestation,
                &proof,
                &context.taira_network_id,
                &context.destination,
            )?
        }
        Some(history) => {
            if bundle.message_count != history.block.message_count {
                return Err(BundleError::MessageCountMismatch);
            }
            verify_transfer_historical(
                attestation,
                history,
                &proof,
                &context.taira_network_id,
                &context.destination,
            )?
        }
    };
    if transfer.message_id != bundle.message_id {
        return Err(BundleError::MessageIdMismatch);
    }
    if transfer.payload.deadline_ms != bundle.deadline_ms {
        return Err(BundleError::DeadlineMismatch);
    }
    check_deadline(purpose, transfer.payload.deadline_ms, context.now_ms)?;
    Ok(VerifiedMessageBundleV1 {
        attested,
        proof,
        history,
        transfer,
        purpose,
    })
}

#[cfg(test)]
mod tests {
    use iroha_data_model::bridge::SccpNetworkV1;
    use iroha_sccp::v1::{
        hashes::{keccak256, word_address},
        merkle::PromoteOddTree,
        payload::SccpTransferPayloadV1,
        signature::{address_of_secret, sign_digest},
    };

    use super::*;

    const TAIRA: [u8; 32] = [0x11; 32];
    const T0: u64 = 1_800_000_000_000;
    const DAY: u64 = 86_400_000;

    fn secret(index: u8) -> [u8; 32] {
        keccak256(&[b"SCCP/WALLET/BUNDLE/TEST", &[index]])
    }

    fn roster() -> RosterV1 {
        let mut members: Vec<[u8; 20]> = (0..4)
            .map(|index| address_of_secret(&secret(index)).expect("secret"))
            .collect();
        members.sort_unstable();
        RosterV1 {
            generation: 3,
            valid_from_ms: T0,
            valid_until_ms: T0 + 14 * DAY,
            members,
        }
    }

    fn destination() -> DestinationV1 {
        DestinationV1 {
            network: SccpNetworkV1::EthereumMainnet,
            route_revision: 1,
            destination_word: word_address(&[0x22; 20]),
        }
    }

    fn signed_set(roster: &RosterV1, digest: &[u8; 32], keys: &[u8]) -> SccpSignatureSetV1 {
        let entries: Vec<(usize, [u8; 65])> = keys
            .iter()
            .map(|key| {
                let address = address_of_secret(&secret(*key)).expect("secret");
                let index = roster
                    .members
                    .iter()
                    .position(|member| *member == address)
                    .expect("member");
                (index, sign_digest(&secret(*key), digest).expect("sign"))
            })
            .collect();
        let set = SignatureSetV1::from_signers(roster.n(), &entries).expect("set");
        SccpSignatureSetV1::try_from(&set).expect("view")
    }

    fn make_bundle(keys: &[u8]) -> (SccpMessageProofBundleV1, DestinationContextV1) {
        let roster = roster();
        let payload = SccpTransferPayloadV1::outbound(
            SccpNetworkV1::EthereumMainnet,
            5,
            1,
            T0 + DAY,
            1_000_000_000,
            vec![0x02, 0x01, 0x20, 0x33],
            vec![0x44; 20],
        )
        .expect("payload");
        let message_id = payload.message_id(&TAIRA).expect("id");
        let leaf =
            iroha_sccp::v1::hashes::transfer_leaf(&message_id, &destination().destination_word);
        let tree = PromoteOddTree::block(&[[0x01; 32], leaf, [0x03; 32]]).expect("tree");
        let statement = SccpAttestationStatementV1 {
            height: 10,
            epoch: 1,
            timestamp_ms: T0 + 1_000,
            block_hash: [0xbb; 32],
            sccp_root: tree.root(),
            message_count: 3,
            history_root: [0xcc; 32],
            history_size: 1,
            roster_digest: roster.digest(&TAIRA).expect("digest"),
            next_roster_digest: [0; 32],
        };
        let digest = AttestationFieldsV1::from(statement).digest(&TAIRA);
        let bundle = SccpMessageProofBundleV1 {
            message_id,
            payload: payload.encode().expect("payload"),
            deadline_ms: T0 + DAY,
            leaf_index: 1,
            message_count: 3,
            path: tree.path(1).expect("path"),
            statement,
            digest,
            signatures: signed_set(&roster, &digest, keys),
            roster: SccpRosterViewV1::from_roster(&roster, &TAIRA).expect("view"),
            history: None,
        };
        let context = DestinationContextV1 {
            taira_network_id: TAIRA,
            destination: destination(),
            roster_state: RosterStateV1::initial(&roster, &TAIRA, T0).expect("state"),
            now_ms: T0 + 2_000,
        };
        (bundle, context)
    }

    #[test]
    fn trim_keeps_the_lowest_set_bits() {
        let set = SignatureSetV1 {
            signer_bitmap: 0b1011_0100,
            signatures: (0..4_u8).flat_map(|index| [index; 65]).collect(),
        };
        let trimmed = trim_to_threshold(&set, 3).expect("trim");
        assert_eq!(trimmed.signer_bitmap, 0b0011_0100);
        assert_eq!(trimmed.signatures.len(), 3 * 65);
        assert_eq!(trimmed.signatures[0], 0);
        assert_eq!(trimmed.signatures[2 * 65], 2);
        assert_eq!(trim_to_threshold(&set, 4).expect("all"), set);
        assert_eq!(
            trim_to_threshold(&set, 5),
            Err(SignatureError::TooFewSignatures)
        );
        let ragged = SignatureSetV1 {
            signer_bitmap: 0b11,
            signatures: vec![0; 65],
        };
        assert_eq!(
            trim_to_threshold(&ragged, 1),
            Err(SignatureError::BadLength)
        );
    }

    #[test]
    fn verify_signed_checks_digest_roster_and_quorum() {
        let (bundle, _) = make_bundle(&[0, 1, 2, 3]);
        let signed = verify_signed(
            &bundle.statement,
            &bundle.digest,
            &bundle.roster,
            &bundle.signatures,
            &TAIRA,
        )
        .expect("verifies");
        assert_eq!(signed.signatures.popcount(), 3, "trimmed to t = 3");
        assert_eq!(signed.digest, bundle.digest);
        assert_eq!(
            verify_signed(
                &bundle.statement,
                &bundle.digest,
                &bundle.roster,
                &bundle.signatures,
                &[0x12; 32],
            ),
            Err(BundleError::DigestMismatch)
        );
        let (few, _) = make_bundle(&[0, 1]);
        assert_eq!(
            verify_signed(
                &few.statement,
                &few.digest,
                &few.roster,
                &few.signatures,
                &TAIRA
            ),
            Err(BundleError::TooFewSignatures {
                count: 2,
                threshold: 3
            })
        );
        let mut statement = bundle.statement;
        statement.sccp_root = [0; 32];
        assert_eq!(
            verify_signed(
                &statement,
                &bundle.digest,
                &bundle.roster,
                &bundle.signatures,
                &TAIRA
            ),
            Err(BundleError::Statement(
                AttestationInvariantError::RootCountMismatch
            ))
        );
    }

    #[test]
    fn accepted_roster_distinguishes_current_and_previous() {
        let state = RosterStateV1 {
            digest: [1; 32],
            generation: 2,
            valid_until_ms: 100,
            prev_digest: [2; 32],
            prev_valid_until_ms: 50,
        };
        assert_eq!(
            accepted_roster(&state, &[1; 32], 100),
            Some(AcceptedRosterV1::Current)
        );
        assert_eq!(accepted_roster(&state, &[1; 32], 101), None);
        assert_eq!(
            accepted_roster(&state, &[2; 32], 50),
            Some(AcceptedRosterV1::Previous)
        );
        assert_eq!(accepted_roster(&state, &[2; 32], 51), None);
        assert_eq!(accepted_roster(&state, &[0; 32], 0), None);
        assert_eq!(accepted_roster(&state, &[3; 32], 0), None);
    }

    #[test]
    fn check_deadline_splits_finalize_and_void() {
        assert_eq!(check_deadline(BundlePurposeV1::Finalize, 10, 10), Ok(()));
        assert_eq!(
            check_deadline(BundlePurposeV1::Finalize, 10, 11),
            Err(BundleError::DeadlinePassed)
        );
        assert_eq!(
            check_deadline(BundlePurposeV1::VoidExpired, 10, 10),
            Err(BundleError::DeadlineNotReached)
        );
        assert_eq!(check_deadline(BundlePurposeV1::VoidExpired, 10, 11), Ok(()));
    }

    #[test]
    fn verify_message_bundle_accepts_a_good_bundle_and_rejects_tampering() {
        let (good, context) = make_bundle(&[0, 1, 2, 3]);
        let verified =
            verify_message_bundle(&good, &context, BundlePurposeV1::Finalize).expect("verifies");
        assert!(!verified.is_historical());
        assert_eq!(verified.nonce(), 5);
        assert_eq!(verified.deadline_ms(), T0 + DAY);
        assert_eq!(verified.attested.accepted_as, AcceptedRosterV1::Current);
        assert_eq!(verified.attested.attested().signatures.popcount(), 3);

        let mut wrong_id = good.clone();
        wrong_id.message_id[0] ^= 1;
        assert_eq!(
            verify_message_bundle(&wrong_id, &context, BundlePurposeV1::Finalize),
            Err(BundleError::MessageIdMismatch)
        );
        let mut wrong_deadline = good.clone();
        wrong_deadline.deadline_ms += 1;
        assert_eq!(
            verify_message_bundle(&wrong_deadline, &context, BundlePurposeV1::Finalize),
            Err(BundleError::DeadlineMismatch)
        );
        let mut wrong_count = good.clone();
        wrong_count.message_count = 4;
        assert_eq!(
            verify_message_bundle(&wrong_count, &context, BundlePurposeV1::Finalize),
            Err(BundleError::MessageCountMismatch)
        );
        let mut wrong_path = good.clone();
        wrong_path.path[0][0] ^= 1;
        assert_eq!(
            verify_message_bundle(&wrong_path, &context, BundlePurposeV1::Finalize),
            Err(BundleError::Proof(ProofError::BadBlockPath))
        );
        let late = DestinationContextV1 {
            now_ms: T0 + DAY + 1,
            ..context
        };
        assert_eq!(
            verify_message_bundle(&good, &late, BundlePurposeV1::Finalize),
            Err(BundleError::DeadlinePassed)
        );
        assert!(verify_message_bundle(&good, &late, BundlePurposeV1::VoidExpired).is_ok());
        assert_eq!(
            verify_message_bundle(&good, &context, BundlePurposeV1::VoidExpired),
            Err(BundleError::DeadlineNotReached)
        );
        let expired = DestinationContextV1 {
            now_ms: T0 + 14 * DAY + 1,
            ..context
        };
        assert_eq!(
            verify_message_bundle(&good, &expired, BundlePurposeV1::VoidExpired),
            Err(BundleError::RosterNotAccepted)
        );
    }

    #[test]
    fn bundle_errors_display() {
        assert!(
            BundleError::TooFewSignatures {
                count: 2,
                threshold: 3
            }
            .to_string()
            .contains("threshold is 3")
        );
        assert!(
            BundleError::Proof(ProofError::BadBlockPath)
                .to_string()
                .starts_with("message proof")
        );
        let error: Box<dyn std::error::Error> = Box::new(BundleError::RosterNotAccepted);
        assert!(error.to_string().contains("does not accept"));
        assert_eq!(
            BundleError::from(ApiShapeError::PayloadTooLong),
            BundleError::Shape(ApiShapeError::PayloadTooLong)
        );
        assert_eq!(
            BundleError::from(RosterError::BadOrder),
            BundleError::Roster(RosterError::BadOrder)
        );
    }
}
