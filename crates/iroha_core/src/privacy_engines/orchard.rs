//! First-release Orchard V3 action-bundle verifier.
//!
//! The integration deliberately exposes no caller-selected Orchard protocol or
//! circuit version. Every bundle is reconstructed as `orchard_v3`, every proof
//! is verified with the `PostNu6_3` key, and the historical insecure and
//! compatibility circuits are therefore unrepresentable.

use std::sync::OnceLock;

use incrementalmerkletree::{Position, frontier::Frontier};
use nonempty::NonEmpty;
use orchard::{
    Action, Anchor, Bundle, Proof,
    bundle::{Authorized, BundleVersion, Flags},
    circuit::{OrchardCircuitVersion, VerifyingKey},
    note::{ExtractedNoteCommitment, Nullifier, TransmittedNoteCiphertext},
    primitives::redpallas::{self, Binding, SpendAuth},
    tree::MerkleHashOrchard,
    value::ValueCommitment,
};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

/// Maximum Orchard actions admitted by the first-release Taira profile.
pub(crate) const ORCHARD_MAX_ACTIONS_V1: usize = 2;
/// Exact pinned upstream Orchard crate version.
pub(crate) const ORCHARD_UPSTREAM_CRATE_VERSION_V1: &str = "0.15.4";
/// Exact pinned upstream source revision.
pub(crate) const ORCHARD_UPSTREAM_REVISION_V1: &str = "9d07047d32c4787e1b7964b4cf4fa0286c93824c";
/// SHA-256 of the pinned upstream Post-NU6.3 circuit description.
pub(crate) const ORCHARD_POST_NU6_3_CIRCUIT_DESCRIPTION_SHA256_V1: &str =
    "8d325ee6753c8effb7d5184bdd729255d2697dd1730c0278084cd91192020e90";
/// Magic and version for the sole first-release Orchard authorization wire.
pub(crate) const ORCHARD_AUTHORIZATION_WIRE_MAGIC_V1: [u8; 4] = *b"ORC1";
/// Complete native-engine profile descriptor.
pub(crate) const ORCHARD_COMPILED_PROFILE_DESCRIPTOR_V1: &[u8] = b"version=1|protocol=orchard-v3|pool=orchard|circuit=PostNu6_3|upstream=orchard-0.15.4@9d07047d32c4787e1b7964b4cf4fa0286c93824c|circuit_description_sha256=8d325ee6753c8effb7d5184bdd729255d2697dd1730c0278084cd91192020e90|critical_deps=halo2-proofs-0.3.4:halo2-gadgets-0.5.0:incrementalmerkletree-0.8.2:pasta-curves-0.5.2:reddsa-0.5.2|flags=spends-enabled:outputs-enabled:cross-address-disabled|actions=1..2|halo2_proof_bytes=2720+2272*actions|authorization_wire=ORC1:u8-action-count:halo2-proof:ordered-64-byte-spend-signatures:64-byte-binding-signature|sighash=sha256-framed-public-bundle-v1|legacy=unrepresentable";

const SIGHASH_DOMAIN_V1: &[u8] = b"iroha.privacy.orchard-v3.bundle-sighash.v1";
const ORCHARD_AUTHORIZATION_HEADER_BYTES_V1: usize = ORCHARD_AUTHORIZATION_WIRE_MAGIC_V1.len() + 1;
const ORCHARD_REDPALLAS_SIGNATURE_BYTES_V1: usize = 64;
const ORCHARD_TREE_DEPTH_V1: u8 = 32;

/// Exact public data for one Orchard V3 action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OrchardActionPublicV1 {
    /// Canonical Pallas-base nullifier encoding.
    pub(crate) nullifier: [u8; 32],
    /// Canonical non-identity randomized RedPallas verification key.
    pub(crate) randomized_key: [u8; 32],
    /// Canonical extracted note commitment.
    pub(crate) note_commitment: [u8; 32],
    /// Canonical non-identity ephemeral Pallas public key.
    pub(crate) ephemeral_key: [u8; 32],
    /// Exact Orchard encrypted-note ciphertext.
    pub(crate) encrypted_note: [u8; 580],
    /// Exact Orchard outgoing ciphertext.
    pub(crate) outgoing_ciphertext: [u8; 80],
    /// Canonical Pallas value commitment.
    pub(crate) value_commitment: [u8; 32],
}

/// Exact public data for one first-release Orchard V3 bundle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OrchardBundlePublicV1 {
    /// Normalized Iroha transaction-intent digest.
    pub(crate) transaction_intent_digest: [u8; 32],
    /// Canonical Orchard note-commitment-tree anchor.
    pub(crate) anchor: [u8; 32],
    /// Signed public Orchard value balance.
    pub(crate) value_balance: i64,
    /// Non-empty ordered Orchard actions.
    pub(crate) actions: Vec<OrchardActionPublicV1>,
}

/// Canonical compact representation of one Orchard note-commitment frontier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OrchardFrontierPartsV1 {
    /// Number of leaves already appended.
    pub(crate) tree_size: u64,
    /// Most recently appended leaf, absent only for the empty tree.
    pub(crate) leaf: Option<[u8; 32]>,
    /// Past subtree roots needed to continue appending.
    pub(crate) ommers: Vec<[u8; 32]>,
    /// Root derived from the complete compact frontier.
    pub(crate) root: [u8; 32],
}

/// Failure returned by the native first-release Orchard verifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub(crate) enum OrchardNativeErrorV1 {
    /// The normalized transaction-intent digest was the zero sentinel.
    #[error("Orchard transaction-intent digest must be non-zero")]
    ZeroTransactionIntentDigest,
    /// The action count was outside the compiled non-empty bound.
    #[error("Orchard action count {actual} is outside 1..={max}")]
    ActionCount {
        /// Supplied count.
        actual: usize,
        /// Compiled maximum.
        max: usize,
    },
    /// The proof did not have the unique canonical size for its action count.
    #[error("Orchard proof length {actual} does not equal canonical length {expected}")]
    ProofLength {
        /// Supplied proof length.
        actual: usize,
        /// Exact required proof length.
        expected: usize,
    },
    /// The proof payload does not use the sole first-release wire version.
    #[error("Orchard authorization wire magic/version is not ORC1")]
    AuthorizationWireMagic,
    /// The proof payload action count differs from the public statement.
    #[error(
        "Orchard authorization wire action count {encoded} differs from statement count {expected}"
    )]
    AuthorizationActionCount {
        /// Count encoded in the authorization wire.
        encoded: usize,
        /// Exact public statement count.
        expected: usize,
    },
    /// The anchor was not a canonical Pallas-base encoding.
    #[error("Orchard anchor is not canonical")]
    AnchorEncoding,
    /// An action nullifier was not canonical.
    #[error("Orchard action {index} nullifier is not canonical")]
    NullifierEncoding {
        /// Ordered action index.
        index: usize,
    },
    /// An action randomized verification key was not canonical.
    #[error("Orchard action {index} randomized key is not canonical")]
    RandomizedKeyEncoding {
        /// Ordered action index.
        index: usize,
    },
    /// An action extracted note commitment was not canonical.
    #[error("Orchard action {index} note commitment is not canonical")]
    NoteCommitmentEncoding {
        /// Ordered action index.
        index: usize,
    },
    /// An action value commitment was not canonical.
    #[error("Orchard action {index} value commitment is not canonical")]
    ValueCommitmentEncoding {
        /// Ordered action index.
        index: usize,
    },
    /// An action contained an identity randomized key or invalid ephemeral key.
    #[error("Orchard action {index} violates canonical action construction")]
    ActionEncoding {
        /// Ordered action index.
        index: usize,
    },
    /// The fixed V3 bundle could not be reconstructed.
    #[error("Orchard V3 bundle construction rejected canonical public data")]
    BundleEncoding,
    /// A RedPallas spend-authorization signature failed.
    #[error("Orchard action {index} spend-authorization signature is invalid")]
    SpendAuthorizationSignature {
        /// Ordered action index.
        index: usize,
    },
    /// The RedPallas binding signature failed.
    #[error("Orchard binding signature is invalid")]
    BindingSignature,
    /// The Post-NU6.3 Halo2 proof failed.
    #[error("Orchard Post-NU6.3 Halo2 action proof is invalid")]
    Halo2Proof,
}

/// Failure while restoring or advancing the authoritative Orchard frontier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub(crate) enum OrchardFrontierErrorV1 {
    /// Empty and non-empty shape fields disagree.
    #[error("Orchard frontier tree size, leaf, and ommers have an inconsistent empty shape")]
    EmptyShape,
    /// The persisted leaf is not a canonical Pallas-base encoding.
    #[error("Orchard frontier leaf is not canonical")]
    LeafEncoding,
    /// One persisted ommer is not a canonical Pallas-base encoding.
    #[error("Orchard frontier ommer {index} is not canonical")]
    OmmerEncoding {
        /// Zero-based ommer index.
        index: usize,
    },
    /// The compact parts do not describe a valid depth-32 frontier.
    #[error("Orchard compact frontier parts are inconsistent or exceed depth 32")]
    FrontierShape,
    /// The reconstructed root differs from persisted authoritative state.
    #[error("Orchard reconstructed frontier root differs from persisted root")]
    RootMismatch,
    /// An output note commitment is not a canonical Orchard leaf.
    #[error("Orchard output action {index} note commitment is not canonical")]
    NoteCommitmentEncoding {
        /// Zero-based output action index.
        index: usize,
    },
    /// Appending another action would exceed the depth-32 tree capacity.
    #[error("Orchard note-commitment tree is full")]
    TreeFull,
}

struct ParsedOrchardAuthorizationV1<'a> {
    halo2_proof: &'a [u8],
    spend_authorization_signatures: Vec<[u8; ORCHARD_REDPALLAS_SIGNATURE_BYTES_V1]>,
    binding_signature: [u8; ORCHARD_REDPALLAS_SIGNATURE_BYTES_V1],
}

fn decode_merkle_hash(
    bytes: &[u8; 32],
    error: OrchardFrontierErrorV1,
) -> Result<MerkleHashOrchard, OrchardFrontierErrorV1> {
    Option::<MerkleHashOrchard>::from(MerkleHashOrchard::from_bytes(bytes)).ok_or(error)
}

fn restore_orchard_frontier_v1(
    tree_size: u64,
    leaf: Option<[u8; 32]>,
    ommers: &[[u8; 32]],
) -> Result<Frontier<MerkleHashOrchard, ORCHARD_TREE_DEPTH_V1>, OrchardFrontierErrorV1> {
    if tree_size == 0 {
        if leaf.is_some() || !ommers.is_empty() {
            return Err(OrchardFrontierErrorV1::EmptyShape);
        }
        return Ok(Frontier::empty());
    }
    let leaf = leaf.ok_or(OrchardFrontierErrorV1::EmptyShape)?;
    let leaf = decode_merkle_hash(&leaf, OrchardFrontierErrorV1::LeafEncoding)?;
    let ommers = ommers
        .iter()
        .enumerate()
        .map(|(index, bytes)| {
            decode_merkle_hash(bytes, OrchardFrontierErrorV1::OmmerEncoding { index })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Frontier::from_parts(Position::from(tree_size - 1), leaf, ommers)
        .map_err(|_| OrchardFrontierErrorV1::FrontierShape)
}

fn orchard_frontier_parts_v1(
    frontier: Frontier<MerkleHashOrchard, ORCHARD_TREE_DEPTH_V1>,
) -> OrchardFrontierPartsV1 {
    let root = frontier.root().to_bytes();
    let tree_size = frontier.tree_size();
    let (leaf, ommers) = frontier.take().map_or((None, Vec::new()), |frontier| {
        let (_, leaf, ommers) = frontier.into_parts();
        (
            Some(leaf.to_bytes()),
            ommers.into_iter().map(|ommer| ommer.to_bytes()).collect(),
        )
    });
    OrchardFrontierPartsV1 {
        tree_size,
        leaf,
        ommers,
        root,
    }
}

/// Return the unique pinned Orchard V3 empty-tree root.
#[must_use]
pub(crate) fn orchard_empty_root_v1() -> [u8; 32] {
    Frontier::<MerkleHashOrchard, ORCHARD_TREE_DEPTH_V1>::empty()
        .root()
        .to_bytes()
}

/// Return whether bytes are one canonical Orchard nullifier encoding.
#[must_use]
pub(crate) fn is_canonical_orchard_nullifier_v1(bytes: &[u8; 32]) -> bool {
    bool::from(Nullifier::from_bytes(bytes).is_some())
}

/// Reconstruct and validate one persisted compact Orchard frontier.
///
/// # Errors
///
/// Rejects inconsistent empty/non-empty shape, non-canonical field values,
/// impossible ommer structure or depth, and a root mismatch.
pub(crate) fn validate_orchard_frontier_v1(
    tree_size: u64,
    leaf: Option<[u8; 32]>,
    ommers: &[[u8; 32]],
    expected_root: [u8; 32],
) -> Result<(), OrchardFrontierErrorV1> {
    let frontier = restore_orchard_frontier_v1(tree_size, leaf, ommers)?;
    if frontier.root().to_bytes() != expected_root {
        return Err(OrchardFrontierErrorV1::RootMismatch);
    }
    Ok(())
}

/// Append ordered output commitments and return the complete successor parts.
///
/// # Errors
///
/// Rejects malformed persisted state, non-canonical commitments, or a full
/// depth-32 tree.
pub(crate) fn append_orchard_commitments_v1(
    tree_size: u64,
    leaf: Option<[u8; 32]>,
    ommers: &[[u8; 32]],
    expected_root: [u8; 32],
    output_commitments: &[[u8; 32]],
) -> Result<OrchardFrontierPartsV1, OrchardFrontierErrorV1> {
    let mut frontier = restore_orchard_frontier_v1(tree_size, leaf, ommers)?;
    if frontier.root().to_bytes() != expected_root {
        return Err(OrchardFrontierErrorV1::RootMismatch);
    }
    for (index, commitment) in output_commitments.iter().enumerate() {
        let commitment = Option::<ExtractedNoteCommitment>::from(
            ExtractedNoteCommitment::from_bytes(commitment),
        )
        .ok_or(OrchardFrontierErrorV1::NoteCommitmentEncoding { index })?;
        if !frontier.append(MerkleHashOrchard::from_cmx(&commitment)) {
            return Err(OrchardFrontierErrorV1::TreeFull);
        }
    }
    Ok(orchard_frontier_parts_v1(frontier))
}

/// Return the unique first-release authorization-wire size for `action_count`.
#[must_use]
pub(crate) fn orchard_authorization_wire_size_v1(action_count: usize) -> Option<usize> {
    let halo2_proof = Proof::expected_proof_size(action_count);
    ORCHARD_AUTHORIZATION_HEADER_BYTES_V1
        .checked_add(halo2_proof)?
        .checked_add(action_count.checked_mul(ORCHARD_REDPALLAS_SIGNATURE_BYTES_V1)?)?
        .checked_add(ORCHARD_REDPALLAS_SIGNATURE_BYTES_V1)
}

fn decode_authorization_wire_v1(
    proof_bytes: &[u8],
    action_count: usize,
) -> Result<ParsedOrchardAuthorizationV1<'_>, OrchardNativeErrorV1> {
    let expected = orchard_authorization_wire_size_v1(action_count).ok_or(
        OrchardNativeErrorV1::ProofLength {
            actual: proof_bytes.len(),
            expected: usize::MAX,
        },
    )?;
    if proof_bytes.len() != expected {
        return Err(OrchardNativeErrorV1::ProofLength {
            actual: proof_bytes.len(),
            expected,
        });
    }
    if proof_bytes[..ORCHARD_AUTHORIZATION_WIRE_MAGIC_V1.len()]
        != ORCHARD_AUTHORIZATION_WIRE_MAGIC_V1
    {
        return Err(OrchardNativeErrorV1::AuthorizationWireMagic);
    }
    let encoded_action_count = usize::from(proof_bytes[ORCHARD_AUTHORIZATION_WIRE_MAGIC_V1.len()]);
    if encoded_action_count != action_count {
        return Err(OrchardNativeErrorV1::AuthorizationActionCount {
            encoded: encoded_action_count,
            expected: action_count,
        });
    }

    let halo2_len = Proof::expected_proof_size(action_count);
    let halo2_start = ORCHARD_AUTHORIZATION_HEADER_BYTES_V1;
    let halo2_end = halo2_start + halo2_len;
    let mut cursor = halo2_end;
    let mut spend_authorization_signatures = Vec::with_capacity(action_count);
    for _ in 0..action_count {
        let end = cursor + ORCHARD_REDPALLAS_SIGNATURE_BYTES_V1;
        let mut signature = [0; ORCHARD_REDPALLAS_SIGNATURE_BYTES_V1];
        signature.copy_from_slice(&proof_bytes[cursor..end]);
        spend_authorization_signatures.push(signature);
        cursor = end;
    }
    let mut binding_signature = [0; ORCHARD_REDPALLAS_SIGNATURE_BYTES_V1];
    binding_signature.copy_from_slice(&proof_bytes[cursor..]);
    Ok(ParsedOrchardAuthorizationV1 {
        halo2_proof: &proof_bytes[halo2_start..halo2_end],
        spend_authorization_signatures,
        binding_signature,
    })
}

fn append_field(hasher: &mut Sha256, field: &[u8]) {
    hasher.update(
        u64::try_from(field.len())
            .expect("compiled Orchard field length fits u64")
            .to_be_bytes(),
    );
    hasher.update(field);
}

/// Derive the sole message signed by every action and the bundle binding key.
///
/// The normalized transaction-intent digest binds Iroha-level fields. The
/// remaining framing independently binds every Orchard public action byte and
/// its order, including ciphertexts that are not Halo2 public inputs.
#[must_use]
pub(crate) fn derive_orchard_bundle_sighash_v1(bundle: &OrchardBundlePublicV1) -> [u8; 32] {
    let mut hasher = Sha256::new();
    append_field(&mut hasher, SIGHASH_DOMAIN_V1);
    append_field(&mut hasher, &bundle.transaction_intent_digest);
    append_field(&mut hasher, &bundle.anchor);
    append_field(&mut hasher, &bundle.value_balance.to_be_bytes());
    append_field(
        &mut hasher,
        &u64::try_from(bundle.actions.len())
            .expect("bounded Orchard action count fits u64")
            .to_be_bytes(),
    );
    for action in &bundle.actions {
        append_field(&mut hasher, &action.nullifier);
        append_field(&mut hasher, &action.randomized_key);
        append_field(&mut hasher, &action.note_commitment);
        append_field(&mut hasher, &action.ephemeral_key);
        append_field(&mut hasher, &action.encrypted_note);
        append_field(&mut hasher, &action.outgoing_ciphertext);
        append_field(&mut hasher, &action.value_commitment);
    }
    hasher.finalize().into()
}

fn orchard_v3_verifying_key() -> &'static VerifyingKey {
    static VERIFYING_KEY: OnceLock<VerifyingKey> = OnceLock::new();
    VERIFYING_KEY.get_or_init(|| VerifyingKey::build(OrchardCircuitVersion::PostNu6_3))
}

fn parse_action(
    index: usize,
    action: &OrchardActionPublicV1,
    spend_authorization_signature: [u8; ORCHARD_REDPALLAS_SIGNATURE_BYTES_V1],
) -> Result<Action<redpallas::Signature<SpendAuth>>, OrchardNativeErrorV1> {
    let nullifier = Option::<Nullifier>::from(Nullifier::from_bytes(&action.nullifier))
        .ok_or(OrchardNativeErrorV1::NullifierEncoding { index })?;
    let randomized_key = redpallas::VerificationKey::<SpendAuth>::try_from(action.randomized_key)
        .map_err(|_| OrchardNativeErrorV1::RandomizedKeyEncoding { index })?;
    let note_commitment = Option::<ExtractedNoteCommitment>::from(
        ExtractedNoteCommitment::from_bytes(&action.note_commitment),
    )
    .ok_or(OrchardNativeErrorV1::NoteCommitmentEncoding { index })?;
    let value_commitment =
        Option::<ValueCommitment>::from(ValueCommitment::from_bytes(&action.value_commitment))
            .ok_or(OrchardNativeErrorV1::ValueCommitmentEncoding { index })?;
    Action::from_parts(
        nullifier,
        randomized_key,
        note_commitment,
        TransmittedNoteCiphertext {
            epk_bytes: action.ephemeral_key,
            enc_ciphertext: action.encrypted_note,
            out_ciphertext: action.outgoing_ciphertext,
        },
        value_commitment,
        redpallas::Signature::<SpendAuth>::from(spend_authorization_signature),
    )
    .map_err(|_| OrchardNativeErrorV1::ActionEncoding { index })
}

/// Verify one complete first-release Orchard V3 bundle.
///
/// # Errors
///
/// Returns a typed failure for malformed encodings, count/size violations,
/// invalid RedPallas signatures, or an invalid Post-NU6.3 Halo2 proof.
pub(crate) fn verify_orchard_bundle_v1(
    public: &OrchardBundlePublicV1,
    proof_bytes: &[u8],
) -> Result<(), OrchardNativeErrorV1> {
    if public.transaction_intent_digest == [0; 32] {
        return Err(OrchardNativeErrorV1::ZeroTransactionIntentDigest);
    }
    if public.actions.is_empty() || public.actions.len() > ORCHARD_MAX_ACTIONS_V1 {
        return Err(OrchardNativeErrorV1::ActionCount {
            actual: public.actions.len(),
            max: ORCHARD_MAX_ACTIONS_V1,
        });
    }
    let authorization = decode_authorization_wire_v1(proof_bytes, public.actions.len())?;
    let anchor = Option::<Anchor>::from(Anchor::from_bytes(public.anchor))
        .ok_or(OrchardNativeErrorV1::AnchorEncoding)?;
    let actions = public
        .actions
        .iter()
        .zip(authorization.spend_authorization_signatures)
        .enumerate()
        .map(|(index, (action, signature))| parse_action(index, action, signature))
        .collect::<Result<Vec<_>, _>>()?;
    let actions = NonEmpty::from_vec(actions).ok_or(OrchardNativeErrorV1::ActionCount {
        actual: 0,
        max: ORCHARD_MAX_ACTIONS_V1,
    })?;
    let authorization = Authorized::from_parts(
        Proof::new(authorization.halo2_proof.to_vec()),
        redpallas::Signature::<Binding>::from(authorization.binding_signature),
    );
    let bundle = Bundle::try_from_parts(
        actions,
        Flags::CROSS_ADDRESS_DISABLED,
        public.value_balance,
        anchor,
        authorization,
        BundleVersion::orchard_v3(),
    )
    .map_err(|_| OrchardNativeErrorV1::BundleEncoding)?;
    let sighash = derive_orchard_bundle_sighash_v1(public);
    for (index, action) in bundle.actions().iter().enumerate() {
        action
            .rk()
            .verify(&sighash, action.authorization())
            .map_err(|_| OrchardNativeErrorV1::SpendAuthorizationSignature { index })?;
    }
    bundle
        .binding_validating_key()
        .verify(&sighash, bundle.authorization().binding_signature())
        .map_err(|_| OrchardNativeErrorV1::BindingSignature)?;
    bundle
        .verify_proof(orchard_v3_verifying_key())
        .map_err(|_| OrchardNativeErrorV1::Halo2Proof)
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::OnceLock;

    use orchard::{
        Anchor, Bundle,
        builder::{Builder, BundleType},
        bundle::{Authorization, Authorized, BundleVersion},
        circuit::{OrchardCircuitVersion, ProvingKey},
    };
    use rand_08::{SeedableRng as _, rngs::StdRng};

    use super::*;

    fn proving_key() -> &'static ProvingKey {
        static PROVING_KEY: OnceLock<ProvingKey> = OnceLock::new();
        PROVING_KEY.get_or_init(|| ProvingKey::build(OrchardCircuitVersion::PostNu6_3))
    }

    fn unsigned_public<T: Authorization>(
        bundle: &Bundle<T, i64>,
        transaction_intent_digest: [u8; 32],
    ) -> OrchardBundlePublicV1 {
        assert_eq!(bundle.flags(), &Flags::CROSS_ADDRESS_DISABLED);
        let actions = bundle
            .actions()
            .iter()
            .map(|action| OrchardActionPublicV1 {
                nullifier: action.nullifier().to_bytes(),
                randomized_key: <[u8; 32]>::from(action.rk()),
                note_commitment: action.cmx().to_bytes(),
                ephemeral_key: action.encrypted_note().epk_bytes,
                encrypted_note: action.encrypted_note().enc_ciphertext,
                outgoing_ciphertext: action.encrypted_note().out_ciphertext,
                value_commitment: action.cv_net().to_bytes(),
            })
            .collect();
        OrchardBundlePublicV1 {
            transaction_intent_digest,
            anchor: bundle.anchor().to_bytes(),
            value_balance: *bundle.value_balance(),
            actions,
        }
    }

    fn raw_bundle(
        bundle: &Bundle<Authorized, i64>,
        transaction_intent_digest: [u8; 32],
    ) -> (OrchardBundlePublicV1, Vec<u8>) {
        let public = unsigned_public(bundle, transaction_intent_digest);
        let mut authorization = Vec::with_capacity(
            orchard_authorization_wire_size_v1(public.actions.len()).expect("bounded wire size"),
        );
        authorization.extend_from_slice(&ORCHARD_AUTHORIZATION_WIRE_MAGIC_V1);
        authorization.push(u8::try_from(public.actions.len()).expect("bounded action count"));
        authorization.extend_from_slice(bundle.authorization().proof().as_ref());
        for action in bundle.actions().iter() {
            authorization.extend_from_slice(&<[u8; 64]>::from(action.authorization()));
        }
        authorization.extend_from_slice(&<[u8; 64]>::from(
            bundle.authorization().binding_signature(),
        ));
        assert_eq!(
            authorization.len(),
            orchard_authorization_wire_size_v1(public.actions.len()).expect("bounded wire size")
        );
        (public, authorization)
    }

    pub(crate) fn build_fixture(
        action_count: u8,
        rng_seed: [u8; 32],
        transaction_intent_digest: [u8; 32],
    ) -> (OrchardBundlePublicV1, Vec<u8>) {
        let version = BundleVersion::orchard_v3();
        let mut rng = StdRng::from_seed(rng_seed);
        let builder = Builder::new(
            BundleType::Transactional {
                bundle_required: true,
                pad_to_minimum: Some(action_count),
            },
            version,
            version.default_flags(),
            Anchor::empty_tree(),
        )
        .expect("pinned Orchard V3 builder");
        let unsigned = builder
            .build::<i64>(&mut rng)
            .expect("build dummy action")
            .expect("bundle required")
            .0;
        let proven = unsigned
            .create_proof(proving_key(), &mut rng)
            .expect("create Post-NU6.3 proof");

        // Sign the exact Iroha framing rather than an unbound caller
        // message. Signatures are applied after deriving it from the
        // proof-independent public action bytes.
        let raw = unsigned_public(&proven, transaction_intent_digest);
        let sighash = derive_orchard_bundle_sighash_v1(&raw);
        let authorized = proven
            .apply_signatures(&mut rng, sighash, &[])
            .expect("apply canonical Orchard signatures");
        raw_bundle(&authorized, transaction_intent_digest)
    }

    pub(crate) fn fixture() -> &'static (OrchardBundlePublicV1, Vec<u8>) {
        static FIXTURE: OnceLock<(OrchardBundlePublicV1, Vec<u8>)> = OnceLock::new();
        FIXTURE.get_or_init(|| build_fixture(1, [0xA7; 32], [0x44; 32]))
    }

    fn two_action_fixture() -> &'static (OrchardBundlePublicV1, Vec<u8>) {
        static FIXTURE: OnceLock<(OrchardBundlePublicV1, Vec<u8>)> = OnceLock::new();
        FIXTURE.get_or_init(|| build_fixture(2, [0xB8; 32], [0x55; 32]))
    }

    fn sha256_hex(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }

    fn public_bytes_for_kat(public: &OrchardBundlePublicV1) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&public.transaction_intent_digest);
        bytes.extend_from_slice(&public.anchor);
        bytes.extend_from_slice(&public.value_balance.to_be_bytes());
        bytes.push(u8::try_from(public.actions.len()).expect("bounded action count"));
        for action in &public.actions {
            bytes.extend_from_slice(&action.nullifier);
            bytes.extend_from_slice(&action.randomized_key);
            bytes.extend_from_slice(&action.note_commitment);
            bytes.extend_from_slice(&action.ephemeral_key);
            bytes.extend_from_slice(&action.encrypted_note);
            bytes.extend_from_slice(&action.outgoing_ciphertext);
            bytes.extend_from_slice(&action.value_commitment);
        }
        bytes
    }

    #[test]
    fn deterministic_public_and_authorization_known_answers_are_stable() {
        let (public, authorization) = fixture();
        assert_eq!(
            (
                hex::encode(derive_orchard_bundle_sighash_v1(public)),
                sha256_hex(&public_bytes_for_kat(public)),
                sha256_hex(authorization),
            ),
            (
                "26608cec06e9f35580e5cf54eccbab1572b817d39aba56416e5f3bc690970528".to_owned(),
                "dcc1e9b3328009a55a8b6b0ba07cc63342a80e0f6c6e09128f91daba029d65b8".to_owned(),
                "d5468c1031aee6e4692cd046a408fd18c0f283829eb40be657f513d8c4b2cc30".to_owned(),
            )
        );
    }

    #[test]
    fn maximum_two_action_bundle_round_trips_and_order_is_bound() {
        let (public, proof) = two_action_fixture();
        assert_eq!(public.actions.len(), ORCHARD_MAX_ACTIONS_V1);
        assert_eq!(
            proof.len(),
            orchard_authorization_wire_size_v1(ORCHARD_MAX_ACTIONS_V1)
                .expect("canonical maximum-action wire")
        );
        verify_orchard_bundle_v1(public, proof).expect("maximum-action Orchard V3 bundle verifies");

        let mut reordered = public.clone();
        reordered.actions.swap(0, 1);
        assert!(
            verify_orchard_bundle_v1(&reordered, proof).is_err(),
            "action order must be bound by proof and signatures"
        );
    }

    #[test]
    fn only_post_nu6_3_profile_is_constructed() {
        assert_eq!(
            orchard_v3_verifying_key().circuit_version(),
            OrchardCircuitVersion::PostNu6_3
        );
        assert!(orchard_v3_verifying_key().supports_cross_address_restriction());
        assert!(
            ORCHARD_COMPILED_PROFILE_DESCRIPTOR_V1
                .windows(b"legacy=unrepresentable".len())
                .any(|window| window == b"legacy=unrepresentable")
        );
    }

    #[test]
    fn empty_frontier_root_matches_upstream_vector_and_shape_is_fail_closed() {
        assert_eq!(
            orchard_empty_root_v1(),
            [
                0xae, 0x29, 0x35, 0xf1, 0xdf, 0xd8, 0xa2, 0x4a, 0xed, 0x7c, 0x70, 0xdf, 0x7d, 0xe3,
                0xa6, 0x68, 0xeb, 0x7a, 0x49, 0xb1, 0x31, 0x98, 0x80, 0xdd, 0xe2, 0xbb, 0xd9, 0x03,
                0x1a, 0xe5, 0xd8, 0x2f,
            ],
            "the node-derived origin must match the pinned upstream depth-32 vector"
        );
        validate_orchard_frontier_v1(0, None, &[], orchard_empty_root_v1())
            .expect("canonical empty frontier");
        assert_eq!(
            validate_orchard_frontier_v1(0, Some([0; 32]), &[], orchard_empty_root_v1()),
            Err(OrchardFrontierErrorV1::EmptyShape)
        );
        assert_eq!(
            validate_orchard_frontier_v1(0, None, &[[0; 32]], orchard_empty_root_v1()),
            Err(OrchardFrontierErrorV1::EmptyShape)
        );
        let mut wrong_root = orchard_empty_root_v1();
        wrong_root[0] ^= 1;
        assert_eq!(
            validate_orchard_frontier_v1(0, None, &[], wrong_root),
            Err(OrchardFrontierErrorV1::RootMismatch)
        );
    }

    #[test]
    fn compact_frontier_round_trips_appends_and_rejects_adversarial_parts() {
        let successor = append_orchard_commitments_v1(
            0,
            None,
            &[],
            orchard_empty_root_v1(),
            &[[0; 32], [1; 32]],
        )
        .expect("append two canonical commitments");
        assert_eq!(successor.tree_size, 2);
        assert!(successor.leaf.is_some());
        assert_eq!(successor.ommers.len(), 1);
        validate_orchard_frontier_v1(
            successor.tree_size,
            successor.leaf,
            &successor.ommers,
            successor.root,
        )
        .expect("persisted compact successor reconstructs exactly");

        assert_eq!(
            append_orchard_commitments_v1(0, None, &[], orchard_empty_root_v1(), &[[u8::MAX; 32]],),
            Err(OrchardFrontierErrorV1::NoteCommitmentEncoding { index: 0 })
        );
        assert_eq!(
            validate_orchard_frontier_v1(1, Some([u8::MAX; 32]), &[], successor.root),
            Err(OrchardFrontierErrorV1::LeafEncoding)
        );
        assert_eq!(
            validate_orchard_frontier_v1(2, Some([0; 32]), &[[u8::MAX; 32]], successor.root,),
            Err(OrchardFrontierErrorV1::OmmerEncoding { index: 0 })
        );
        assert_eq!(
            validate_orchard_frontier_v1(2, Some([0; 32]), &[], successor.root),
            Err(OrchardFrontierErrorV1::FrontierShape)
        );

        let full_ommers = vec![[0; 32]; usize::from(ORCHARD_TREE_DEPTH_V1)];
        let full = restore_orchard_frontier_v1(
            1_u64 << ORCHARD_TREE_DEPTH_V1,
            Some([0; 32]),
            &full_ommers,
        )
        .expect("synthetic full-depth canonical frontier");
        let full_root = full.root().to_bytes();
        assert_eq!(
            append_orchard_commitments_v1(
                1_u64 << ORCHARD_TREE_DEPTH_V1,
                Some([0; 32]),
                &full_ommers,
                full_root,
                &[[1; 32]],
            ),
            Err(OrchardFrontierErrorV1::TreeFull)
        );
    }

    #[test]
    fn complete_orchard_v3_bundle_round_trips() {
        let (public, proof) = fixture();
        assert_eq!(
            proof.len(),
            orchard_authorization_wire_size_v1(1).expect("canonical one-action wire")
        );
        verify_orchard_bundle_v1(public, proof).expect("complete Orchard V3 bundle verifies");
    }

    #[test]
    fn strict_counts_proof_size_and_canonical_encodings_fail_closed() {
        let (public, proof) = fixture();
        let mut changed = public.clone();
        changed.transaction_intent_digest = [0; 32];
        assert_eq!(
            verify_orchard_bundle_v1(&changed, proof),
            Err(OrchardNativeErrorV1::ZeroTransactionIntentDigest)
        );

        changed = public.clone();
        changed.actions.clear();
        assert!(matches!(
            verify_orchard_bundle_v1(&changed, proof),
            Err(OrchardNativeErrorV1::ActionCount { actual: 0, .. })
        ));

        changed = public.clone();
        changed.actions = vec![changed.actions[0].clone(); ORCHARD_MAX_ACTIONS_V1 + 1];
        assert!(matches!(
            verify_orchard_bundle_v1(&changed, proof),
            Err(OrchardNativeErrorV1::ActionCount { .. })
        ));

        let malformed = [
            proof[..proof.len() - 1].to_vec(),
            [proof.as_slice(), &[0]].concat(),
            Vec::new(),
        ];
        for malformed in malformed {
            assert!(matches!(
                verify_orchard_bundle_v1(public, &malformed),
                Err(OrchardNativeErrorV1::ProofLength { .. })
            ));
        }

        let mut changed_proof = proof.clone();
        changed_proof[0] ^= 1;
        assert_eq!(
            verify_orchard_bundle_v1(public, &changed_proof),
            Err(OrchardNativeErrorV1::AuthorizationWireMagic)
        );

        changed_proof = proof.clone();
        changed_proof[ORCHARD_AUTHORIZATION_WIRE_MAGIC_V1.len()] = 2;
        assert_eq!(
            verify_orchard_bundle_v1(public, &changed_proof),
            Err(OrchardNativeErrorV1::AuthorizationActionCount {
                encoded: 2,
                expected: 1
            })
        );

        changed = public.clone();
        changed.anchor = [0xFF; 32];
        assert_eq!(
            verify_orchard_bundle_v1(&changed, proof),
            Err(OrchardNativeErrorV1::AnchorEncoding)
        );

        changed = public.clone();
        changed.actions[0].nullifier = [0xFF; 32];
        assert!(matches!(
            verify_orchard_bundle_v1(&changed, proof),
            Err(OrchardNativeErrorV1::NullifierEncoding { index: 0 })
        ));

        changed = public.clone();
        changed.actions[0].randomized_key = [0; 32];
        assert!(matches!(
            verify_orchard_bundle_v1(&changed, proof),
            Err(OrchardNativeErrorV1::RandomizedKeyEncoding { index: 0 })
                | Err(OrchardNativeErrorV1::ActionEncoding { index: 0 })
        ));

        changed = public.clone();
        changed.actions[0].note_commitment = [0xFF; 32];
        assert!(matches!(
            verify_orchard_bundle_v1(&changed, proof),
            Err(OrchardNativeErrorV1::NoteCommitmentEncoding { index: 0 })
        ));

        changed = public.clone();
        changed.actions[0].ephemeral_key = [0; 32];
        assert!(matches!(
            verify_orchard_bundle_v1(&changed, proof),
            Err(OrchardNativeErrorV1::ActionEncoding { index: 0 })
        ));

        changed = public.clone();
        changed.actions[0].value_commitment = [0xFF; 32];
        assert!(matches!(
            verify_orchard_bundle_v1(&changed, proof),
            Err(OrchardNativeErrorV1::ValueCommitmentEncoding { index: 0 })
        ));
    }

    #[test]
    fn every_signed_public_component_and_authorization_rejects_mutation() {
        let (public, proof) = fixture();
        let mutations: [fn(&mut OrchardBundlePublicV1); 10] = [
            |value| value.transaction_intent_digest[0] ^= 1,
            |value| value.anchor[0] ^= 1,
            |value| value.value_balance ^= 1,
            |value| value.actions[0].nullifier[0] ^= 1,
            |value| value.actions[0].randomized_key[0] ^= 1,
            |value| value.actions[0].note_commitment[0] ^= 1,
            |value| value.actions[0].ephemeral_key[0] ^= 1,
            |value| value.actions[0].encrypted_note[0] ^= 1,
            |value| value.actions[0].outgoing_ciphertext[0] ^= 1,
            |value| value.actions[0].value_commitment[0] ^= 1,
        ];
        for mutate in mutations {
            let mut changed = public.clone();
            mutate(&mut changed);
            assert!(verify_orchard_bundle_v1(&changed, proof).is_err());
        }

        let halo2_len = Proof::expected_proof_size(public.actions.len());
        let spend_signature_offset = ORCHARD_AUTHORIZATION_HEADER_BYTES_V1 + halo2_len;
        let mut changed_proof = proof.clone();
        changed_proof[spend_signature_offset] ^= 1;
        assert_eq!(
            verify_orchard_bundle_v1(public, &changed_proof),
            Err(OrchardNativeErrorV1::SpendAuthorizationSignature { index: 0 })
        );

        changed_proof = proof.clone();
        let last = changed_proof.len() - 1;
        changed_proof[last] ^= 1;
        assert_eq!(
            verify_orchard_bundle_v1(public, &changed_proof),
            Err(OrchardNativeErrorV1::BindingSignature)
        );

        let samples = 32usize.min(halo2_len);
        for sample in 0..samples {
            let offset = ORCHARD_AUTHORIZATION_HEADER_BYTES_V1 + sample * halo2_len / samples;
            let mut corrupted = proof.clone();
            corrupted[offset] ^= 1 << (sample % 8);
            assert_eq!(
                verify_orchard_bundle_v1(public, &corrupted),
                Err(OrchardNativeErrorV1::Halo2Proof)
            );
        }
    }
}
