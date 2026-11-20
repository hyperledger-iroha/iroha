//! Production funding constructors for an enrolled hardware wallet.
//!
//! `NativeEngine::bootstrap_from_enrollment` derives the exact zero state of an
//! authenticated enrollment credential. `NativeEngine::load_from_finality`
//! derives an ordinary Load from the released predecessor's private opening and
//! genesis-rooted finality. Neither accepts a caller-selected private witness,
//! a signing key or a fixture issuer: every state, statement and receipt field
//! is recomputed here from authenticated originals, and the wallet-owned map
//! store supplies openings only.
//!
//! The returned witness confers no authority. `Engine::prepare` still checks
//! the complete relation, and the canonical Advance provider still selects,
//! obtains the hardware signature and releases.
//!
//! State blinding and retry. The successor nonce (Core field 29) is fresh
//! nonzero OS randomness on every construction; no dedicated protocol state
//! salt exists. The operation identity does not depend on it: Bootstrap's is
//! `P(kgwopid1, wallet, 1, enrollment id)` and Load's is
//! `P(kgwopid1, wallet, 2, receipt digest)`. A repeated fresh construction
//! therefore yields the same operation identity with another successor head and
//! capsule. Custody freezes the exact bytes it selects.
use super::*;
use crate::{
    bootstrap_context as context, certificate, credential,
    fs::ProofEntropy,
    full, indexed_map,
    ipa::Gens,
    load, monetary_state,
    statement::{self, Kind},
};
use ff::{Field, PrimeField};
use iroha_data_model::{
    isi::kagemusha_wallet::load_finality::KagemushaWalletLoadReceiptV1,
    kagemusha::{
        KagemushaWalletCredentialBodyV1, KagemushaWalletIndexedInsertV1,
        KagemushaWalletLoadFinalityV1,
    },
    sumeragi_finality::SumeragiCommitVerifierV1,
};
use iroha_pasta::{Eq as Vesta, poseidon};

/// Poseidon domain of every operation identity.
const OPERATION_DOMAIN: u64 = u64::from_le_bytes(*b"kgwopid1");

/// The installed trust a funding constructor reads. Production borrows it from
/// authenticated parameters and the engine's protected epoch selection only.
#[derive(Clone, Copy)]
struct Installed<'a> {
    id: &'a [u8; 32],
    manifest_digest: &'a [u8; 32],
    scheme: &'a [u8; 32],
    relation: &'a [u8; 32],
    root: &'a p256::Point,
    table: &'a Gens<Vesta>,
    finality: &'a SumeragiCommitVerifierV1,
}
impl<'a> Installed<'a> {
    fn of(parameters: &'a AuthenticatedParameters) -> Self {
        let inner = &parameters.inner;
        Self {
            id: &parameters.id,
            manifest_digest: &parameters.manifest_digest,
            scheme: &inner.scheme,
            relation: &inner.relation,
            root: &inner.root,
            table: &inner.generators.p,
            finality: &inner.finality,
        }
    }
}

impl NativeEngine {
    /// Exact unsigned Bootstrap of an enrolled hardware credential.
    ///
    /// `credential_original` is the 596-byte credential (body ‖ signature) and
    /// `issuer_original` its 172-byte Enrollment-role issuer certificate. Both
    /// are authenticated under the installed scheme root, with the certified
    /// payment key table recomputed on the installed generators, and the
    /// credential must pass the wallet protocol's enrollment-field validation.
    ///
    /// `enrollment_marker` is the digest of the provider's generation-0
    /// enrollment marker. The Bootstrap statement binds it, and it is not
    /// derivable from the credential (it covers the enrollment challenge). Pass
    /// the same value to [`NativeEngine::enrollment_origin`].
    ///
    /// The zero state, statement and receipt body follow from those originals
    /// and the installed relation. The receipt is unsigned and its proof,
    /// capsule and Payment digests are zero until `Engine::prepare` freezes it.
    ///
    /// # Errors
    /// Rejects malformed or unauthenticated originals, a foreign scheme, a
    /// zero marker and unavailable OS entropy.
    pub fn bootstrap_from_enrollment(
        &self,
        credential_original: &[u8],
        issuer_original: &[u8],
        enrollment_marker: [u8; 32],
        parameters: &AuthenticatedParameters,
    ) -> Result<ExactOperationWitness> {
        let installed = Installed::of(parameters);
        let (credential, issuer, fields) =
            enrollment_originals(&installed, credential_original, issuer_original)?;
        bootstrap_witness(
            &installed,
            credential,
            issuer,
            &fields,
            &enrollment_marker,
            fresh_state_nonce(None)?,
        )
    }

    /// Exact unsigned ordinary Load that follows `released`.
    ///
    /// `released` is the frozen preparation of the wallet's current released
    /// operation, of any kind, and `verified` the verified head it released.
    /// The predecessor state is `released`'s own successor opening, checked
    /// against `verified`'s public head, lineage and credential. A head already
    /// bound to custody ([`NativeEngine::bind_released_custody`],
    /// `Engine::finalize`) must be bound to exactly this preparation's capsule.
    ///
    /// `receipt_original` and `finality_original` are the exact canonical
    /// ledger originals. The receipt must name this wallet and its next Load
    /// ordinal, and the finality must verify under the installed reader, at the
    /// engine's protected epoch selection when it holds one.
    ///
    /// `recovery` is asked once, only after finality has verified, for the
    /// insertion of this Load into the wallet-owned load/redeem recovery map.
    /// Its arguments are the committed recovery root, the map key and the leaf
    /// value, each a canonical field encoding derived here. A
    /// [`SpartanWalletMaps`] store answers it by staging
    /// `insert(SpartanWalletMap::Recovery, root, key, value)`. The answer is
    /// verified against that committed root before anything is returned, so the
    /// store supplies map openings only.
    ///
    /// # Errors
    /// Rejects a foreign parameter scope, a head that is not `released`'s, a
    /// wrong opening, unauthenticated originals, a repeated or out-of-order
    /// ordinal, arithmetic overflow, a wrong map insertion and unavailable OS
    /// entropy. An error of `recovery` is returned unchanged.
    pub fn load_from_finality<R>(
        &self,
        released: &PreparedOperation,
        verified: &VerifiedWalletHead,
        receipt_original: &[u8],
        finality_original: &[u8],
        recovery: R,
        parameters: &AuthenticatedParameters,
    ) -> Result<ExactOperationWitness>
    where
        R: FnOnce(&[u8; 32], &[u8; 32], &[u8; 32]) -> Result<KagemushaWalletIndexedInsertV1>,
    {
        let mut installed = Installed::of(parameters);
        installed.finality = self.funding_commit_reader(parameters);
        load_witness(
            &installed,
            released,
            verified,
            receipt_original,
            finality_original,
            recovery,
            |previous| fresh_state_nonce(Some(previous)),
        )
    }
}

/// Fresh nonzero state nonce from the operating system, different from the
/// predecessor's.
pub(super) fn fresh_state_nonce(previous: Option<Fp>) -> Result<Fp> {
    for _ in 0..4 {
        let entropy = ProofEntropy::fresh().map_err(|_| Error::Unavailable("OS state entropy"))?;
        let nonce = entropy.stream(b"monetary state nonce").scalar::<Fp>();
        if nonce != Fp::ZERO && Some(nonce) != previous {
            return Ok(nonce);
        }
    }
    Err(Error::Unavailable("OS state entropy"))
}

/// A 32-byte identity as its two little-endian 128-bit limbs.
pub(super) fn limbs(bytes: &[u8; 32]) -> [Fp; 2] {
    let (low, high) = bytes.split_at(16);
    [low, high].map(|half| {
        Fp::from_u128(u128::from_le_bytes(
            half.try_into().expect("sixteen-byte limb"),
        ))
    })
}

/// A committed counter or balance as its integer.
pub(super) fn uint(value: Fp) -> Result<u128> {
    let bytes = value.to_repr();
    if bytes[16..].iter().any(|byte| *byte != 0) {
        return Err(Error::Malformed("released state integer width"));
    }
    Ok(u128::from_le_bytes(
        bytes[..16].try_into().expect("sixteen-byte integer"),
    ))
}

/// Exact own-receipt body before selection. The signature and the proof,
/// capsule and Payment digests stay zero: `Engine::prepare` freezes the first
/// two, and only a Receive carries a Payment digest.
pub(super) fn unsigned_receipt(
    c: &Credential,
    fields: &[Fp; 26],
    operation: Fp,
) -> bootstrap::Receipt {
    let mut receipt = bootstrap::Receipt {
        body: [0; bootstrap::RECEIPT_BYTES],
        signature: [0; 64],
    };
    receipt.body[..2].copy_from_slice(&1u16.to_le_bytes());
    for (to, from) in [(2, 2), (34, 66), (66, 259)] {
        receipt.body[to..to + 32].copy_from_slice(&c.body[from..from + 32]);
    }
    receipt.body[98..114].copy_from_slice(&fields[9].to_repr()[..16]);
    for (at, value) in [
        (114, operation),
        (146, fields[14]),
        (178, fields[15]),
        (210, statement::digest(fields)),
    ] {
        receipt.body[at..at + 32].copy_from_slice(&value.to_repr());
    }
    receipt
}

/// Authenticate the exact enrollment originals under the installed scope and
/// read the credential's validated enrollment fields.
fn enrollment_originals(
    installed: &Installed<'_>,
    credential_original: &[u8],
    issuer_original: &[u8],
) -> Result<(
    Credential,
    certificate::Certificate,
    KagemushaWalletCredentialBodyV1,
)> {
    if credential_original.len() != credential::BODY_BYTES + 64 {
        return Err(Error::Malformed("enrollment credential length"));
    }
    let (body, signature) = credential_original.split_at(credential::BODY_BYTES);
    let c = Credential {
        body: body.try_into().expect("checked credential body length"),
        signature: signature
            .try_into()
            .expect("checked credential signature length"),
    };
    let issuer = super::engine::read_certificate(issuer_original)?;
    super::engine::authenticate_credential_under(
        installed.scheme,
        installed.root,
        installed.table,
        &c,
        &issuer,
    )?;
    let fields = c.enrollment_fields()?;
    Ok((c, issuer, fields))
}

/// The zero state of an authenticated credential and its unsigned Bootstrap.
fn bootstrap_witness(
    installed: &Installed<'_>,
    c: Credential,
    issuer: certificate::Certificate,
    fields: &KagemushaWalletCredentialBodyV1,
    enrollment_marker: &[u8; 32],
    nonce: Fp,
) -> Result<ExactOperationWitness> {
    if enrollment_marker.iter().all(|byte| *byte == 0) {
        return Err(Error::AuthorityMismatch("enrollment marker"));
    }
    if nonce == Fp::ZERO {
        return Err(Error::Unavailable("fresh state nonce"));
    }
    let mut core = [Fp::ZERO; 30];
    core[0] = Fp::ONE;
    core[1..3].copy_from_slice(&limbs(&fields.scheme_id));
    core[3..5].copy_from_slice(&limbs(&fields.asset_digest));
    core[5..7].copy_from_slice(&limbs(&fields.wallet_id));
    core[7] = credential::digest(&c);
    core[8] = bootstrap::padding_root(b"kgwpcrd1", b"kgwpcrn1", 1, 16);
    core[17..21].fill(bootstrap::empty_indexed_root());
    core[21] = bootstrap::padding_root(b"kgwquse1", b"kgwqusn1", 4, 6);
    core[25] = Fp::from(fields.regulatory_policy.time_anchor_max_response_ms);
    core[26] = Fp::from(fields.lease_expires_at_ms);
    core[29] = nonce;
    let mut rest = [Fp::ZERO; 6];
    rest[0] = Fp::from(u64::from(fields.regulatory_policy.permitted_controls));
    let mut terms = [Fp::ZERO; 26];
    terms[0] = Fp::ONE;
    terms[1..3].copy_from_slice(&limbs(installed.relation));
    terms[3..7].copy_from_slice(&core[1..5]);
    terms[7] = core[7];
    terms[8] = core[0];
    terms[15] = monetary_state::commitment(&core, &rest);
    terms[16] = Fp::from(Kind::Bootstrap.tag());
    terms[17..19].copy_from_slice(&limbs(&fields.enrollment_id));
    terms[19..21].copy_from_slice(&limbs(enrollment_marker));
    let operation = poseidon::hash_with_domain(
        OPERATION_DOMAIN,
        &[core[5], core[6], terms[16], terms[17], terms[18]],
    );
    let receipt = unsigned_receipt(&c, &terms, operation);
    Ok(ExactOperationWitness::Bootstrap(Box::new(
        BootstrapWitness {
            scheme: *installed.scheme,
            root: installed.root.clone(),
            relation: *installed.relation,
            input: bootstrap::Inputs {
                certificate: issuer,
                credential: c,
                core,
                rest,
                statement: terms,
                receipt,
            },
        },
    )))
}

/// The relation's insertion witness of a store answer already verified against
/// `old_root`.
pub(super) fn insert_witness(
    insertion: &KagemushaWalletIndexedInsertV1,
    old_root: Fp,
    key: Fp,
    value: Fp,
) -> Result<indexed_map::InsertWitness<32>> {
    fn path(slot: u32, siblings: &[[u8; 32]; 32]) -> Result<indexed_map::Path<32>> {
        let mut fields = [Fp::ZERO; 32];
        for (to, from) in fields.iter_mut().zip(siblings) {
            *to = field(from)?;
        }
        Ok(indexed_map::Path {
            index: slot,
            siblings: fields,
        })
    }
    Ok(indexed_map::InsertWitness {
        old_root,
        key,
        value,
        low: indexed_map::Leaf {
            key: field(&insertion.low.key)?,
            value: field(&insertion.low.value)?,
            next: field(&insertion.low.next_key)?,
        },
        low_path: path(insertion.low_opening.slot, &insertion.low_opening.siblings)?,
        slot: path(
            insertion.slot_opening.slot,
            &insertion.slot_opening.siblings,
        )?,
    })
}

/// The ordinary Load that follows `released`. `nonce` receives the predecessor
/// nonce and returns the successor's; production samples the operating system.
fn load_witness<R, N>(
    installed: &Installed<'_>,
    released: &PreparedOperation,
    verified: &VerifiedWalletHead,
    receipt_original: &[u8],
    finality_original: &[u8],
    recovery: R,
    nonce: N,
) -> Result<ExactOperationWitness>
where
    R: FnOnce(&[u8; 32], &[u8; 32], &[u8; 32]) -> Result<KagemushaWalletIndexedInsertV1>,
    N: FnOnce(Fp) -> Result<Fp>,
{
    // Installed scope of both capabilities.
    if &released.parameters_id != installed.id || &verified.parameters_id != installed.id {
        return Err(Error::AuthorityMismatch("funding parameter identity"));
    }
    if &released.manifest_digest != installed.manifest_digest
        || &verified.manifest_digest != installed.manifest_digest
    {
        return Err(Error::AuthorityMismatch("released producer manifest"));
    }
    // `verified` is a released recursive head, never an enrollment origin, and
    // custody already bound to it is this preparation's.
    if verified.enrollment_marker.is_some()
        || verified.pair.is_empty()
        || (verified.custody_digest != [0; 32]
            && &verified.custody_digest != released.capsule_digest())
    {
        return Err(Error::AuthorityMismatch("released predecessor custody"));
    }
    let c = released.credential();
    if c.body != verified.credential.body
        || c.signature != verified.credential.signature
        || c.body[2..34] != installed.scheme[..]
    {
        return Err(Error::AuthorityMismatch("released credential"));
    }
    // The private opening is the one the verified public head commits to.
    let (core, rest) = released.successor_opening();
    let head = monetary_state::commitment(core, rest);
    if head != verified.profile[full::HEAD_IO]
        || head.to_repr() != *released.capsule.successor()
        || verified.profile[context::LINEAGE_IO + context::LIFECYCLE] != core[0]
        || verified.profile[context::LINEAGE_IO + context::POLICY_EPOCH] != core[27]
        || verified.profile[context::LINEAGE_IO + context::ENABLED_CONTROLS] != core[22]
        || core[1..3] != limbs(&c.body[2..34].try_into().expect("credential scheme"))
        || core[3..5] != limbs(&c.body[34..66].try_into().expect("credential asset"))
        || core[5..7] != limbs(&c.body[66..98].try_into().expect("credential wallet"))
        || core[7] != credential::digest(c)
    {
        return Err(Error::AuthorityMismatch("released successor opening"));
    }
    // Exact canonical originals.
    let receipt = KagemushaWalletLoadReceiptV1::decode_canonical(receipt_original)
        .map_err(|e| Error::Rejected(e.to_string()))?;
    let finality = KagemushaWalletLoadFinalityV1::decode_canonical(finality_original)
        .map_err(|e| Error::Rejected(e.to_string()))?;
    if receipt.scheme_id != c.body[2..34]
        || receipt.asset_digest != c.body[34..66]
        || receipt.wallet_id != c.body[66..98]
    {
        return Err(Error::AuthorityMismatch("Load receipt wallet scope"));
    }
    // Exactly the next ordinal, and integer successors.
    let next_load = uint(core[13])?;
    if receipt.ordinal < next_load {
        return Err(Error::AuthorityMismatch("Load ordinal already consumed"));
    }
    if receipt.ordinal > next_load {
        return Err(Error::AuthorityMismatch(
            "Load ordinal is ahead of the next ordinal",
        ));
    }
    let balance = uint(core[9])?
        .checked_add(receipt.amount)
        .ok_or(Error::Malformed("Load balance overflow"))?;
    let sequence = uint(core[11])?
        .checked_add(1)
        .ok_or(Error::Malformed("Load sequence overflow"))?;
    let following = receipt
        .ordinal
        .checked_add(1)
        .ok_or(Error::Malformed("Load ordinal overflow"))?;
    // Concrete signed finality and counted event inclusion, installed reader.
    finality
        .verify_with(&mut installed.finality.clone(), &receipt)
        .map_err(|e| Error::Rejected(e.to_string()))?;
    let ordinary = receipt
        .transcript()
        .map_err(|e| Error::Rejected(e.to_string()))?;
    let digest = load::ordinary_digest(&ordinary);
    if digest.to_repr() != finality.receipt_digest {
        return Err(Error::AuthorityMismatch("Load receipt digest"));
    }
    // Recovery entry `(Load, ordinal)` against the committed root.
    let ordinal = Fp::from_u128(receipt.ordinal);
    let amount = Fp::from_u128(receipt.amount);
    let key = Fp::from_u128(1u128 << 127).double() + ordinal;
    let value = poseidon::hash_with_domain(load::RECOVERY_DOMAIN, &[ordinal, digest, amount]);
    let root = core[19];
    let (root_bytes, key_bytes, value_bytes) = (root.to_repr(), key.to_repr(), value.to_repr());
    let insertion = recovery(&root_bytes, &key_bytes, &value_bytes)?;
    let inserted = insertion
        .verify(&root_bytes, &key_bytes, &value_bytes)
        .map_err(|_| Error::AuthorityMismatch("Load recovery insertion"))?;
    let new_root = field(&inserted)?;
    let opening = insert_witness(&insertion, root, key, value)?;
    let leaf = indexed_map::Leaf {
        key,
        value,
        next: opening.low.next,
    };
    if indexed_map::path_root(indexed_map::leaf_hash(leaf), &opening.slot) != new_root {
        return Err(Error::AuthorityMismatch("Load recovery insertion"));
    }
    let nonce = nonce(core[29])?;
    if nonce == Fp::ZERO || nonce == core[29] {
        return Err(Error::Unavailable("fresh state nonce"));
    }
    let mut after = zeroize::Zeroizing::new(*core);
    after[9] = Fp::from_u128(balance);
    after[11] = Fp::from_u128(sequence);
    after[13] = Fp::from_u128(following);
    after[19] = new_root;
    after[29] = nonce;
    let mut terms = zeroize::Zeroizing::new([Fp::ZERO; 26]);
    terms[0] = Fp::ONE;
    terms[1..3].copy_from_slice(&limbs(installed.relation));
    terms[3..7].copy_from_slice(&after[1..5]);
    terms[7] = after[7];
    terms[8] = after[0];
    terms[9] = after[11];
    terms[10] = after[13];
    terms[11] = core[22];
    terms[14] = head;
    terms[15] = monetary_state::commitment(&after, rest);
    terms[16] = Fp::from(Kind::Load.tag());
    terms[17] = digest;
    terms[18] = ordinal;
    terms[19] = amount;
    terms[20] = Fp::from_u128(receipt.online_charge);
    let operation = poseidon::hash_with_domain(
        OPERATION_DOMAIN,
        &[after[5], after[6], terms[16], terms[17]],
    );
    Ok(ExactOperationWitness::Load {
        step: Box::new(load::Witness {
            before_core: *core,
            before_rest: *rest,
            after_core: *after,
            after_rest: *rest,
            credential: c.clone(),
            ordinary,
            statement: *terms,
            receipt: unsigned_receipt(c, &terms, operation),
            recovery: opening,
        }),
        receipt_original: receipt_original.into(),
        finality_original: finality_original.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation_auth::tests as harness;
    use iroha_data_model::{
        kagemusha::{
            KagemushaWalletIndexedLeafV1, KagemushaWalletIndexedOpeningV1,
            KagemushaWalletIndexedTreeV1, KagemushaWalletLoadLeafV1,
        },
        sumeragi_finality::test_fixtures::NativeFinalityFixture,
    };

    /// Enrollment marker of `bootstrap::fixture`'s statement.
    const MARKER: [u8; 32] = [0x44; 32];
    /// Public secret of the fixture Enrollment issuer.
    const ISSUER_SECRET: [u8; 32] = [0x26; 32];

    /// The isolated fixture scheme's installed trust, without its programs.
    struct Fixture {
        generators: Gens<Vesta>,
        chain: crate::chain_fixture::Chain,
        finality: SumeragiCommitVerifierV1,
        manifest: [u8; 32],
    }
    impl Fixture {
        fn new() -> Self {
            let generators = Gens::<Vesta>::new(4096);
            let chain = device_fixture::chain(&generators);
            let native = NativeFinalityFixture::new_with_explicit_parameters();
            Self {
                finality: SumeragiCommitVerifierV1::new(&native.verifier()).unwrap(),
                generators,
                chain,
                manifest: [0x6d; 32],
            }
        }
        fn installed(&self) -> Installed<'_> {
            let context = &self.chain.sender_context;
            Installed {
                id: &context.relation,
                manifest_digest: &self.manifest,
                scheme: &context.root.scheme,
                relation: &context.relation,
                root: &context.root.root,
                table: &self.generators,
                finality: &self.finality,
            }
        }
        /// The independent fixture Bootstrap of the sender wallet.
        fn bootstrap(&self) -> ExactOperationWitness {
            let context = &self.chain.sender_context;
            ExactOperationWitness::Bootstrap(Box::new(BootstrapWitness {
                scheme: context.root.scheme,
                root: context.root.root.clone(),
                relation: context.relation,
                input: self.chain.sender_bootstrap.clone(),
            }))
        }
        fn bootstrap_profile(&self) -> [Fp; 64] {
            bootstrap::public_profile(&self.chain.sender_context, &self.chain.sender_bootstrap)
        }
    }

    /// Wallet-owned store stand-in holding only the recovery map. It answers
    /// for its current root; `checks_root = false` and `forge` model a store
    /// that answers wrongly.
    struct RecoveryStore {
        map: KagemushaWalletIndexedTreeV1,
        checks_root: bool,
        forge: Option<fn(&mut KagemushaWalletIndexedInsertV1)>,
        asked: usize,
    }
    impl RecoveryStore {
        fn of(map: &KagemushaWalletIndexedTreeV1) -> Self {
            Self {
                map: map.clone(),
                checks_root: true,
                forge: None,
                asked: 0,
            }
        }
    }
    impl SpartanWalletMaps for RecoveryStore {
        fn member_or_low(
            &mut self,
            _: SpartanWalletMap,
            _: [u8; 32],
            _: [u8; 32],
        ) -> Result<(
            KagemushaWalletIndexedLeafV1,
            KagemushaWalletIndexedOpeningV1,
        )> {
            Err(Error::Unavailable("a Load performs no map search"))
        }
        fn insert(
            &mut self,
            which: SpartanWalletMap,
            root: [u8; 32],
            key: [u8; 32],
            value: [u8; 32],
        ) -> Result<KagemushaWalletIndexedInsertV1> {
            self.asked += 1;
            if which != SpartanWalletMap::Recovery || (self.checks_root && self.map.root() != root)
            {
                return Err(Error::AuthorityMismatch("test store recovery root"));
            }
            let mut insertion = self
                .map
                .insert(key, value)
                .map_err(|e| Error::Rejected(e.to_string()))?;
            if let Some(forge) = self.forge {
                forge(&mut insertion);
            }
            Ok(insertion)
        }
    }
    /// A store that cannot answer.
    struct Offline;
    impl SpartanWalletMaps for Offline {
        fn member_or_low(
            &mut self,
            _: SpartanWalletMap,
            _: [u8; 32],
            _: [u8; 32],
        ) -> Result<(
            KagemushaWalletIndexedLeafV1,
            KagemushaWalletIndexedOpeningV1,
        )> {
            Err(Error::Unavailable("test recovery map"))
        }
        fn insert(
            &mut self,
            _: SpartanWalletMap,
            _: [u8; 32],
            _: [u8; 32],
            _: [u8; 32],
        ) -> Result<KagemushaWalletIndexedInsertV1> {
            Err(Error::Unavailable("test recovery map"))
        }
    }
    /// The adapter a provider passes: its map store staging the Recovery
    /// insertion for the committed root, key and value.
    fn staging(
        store: &mut dyn SpartanWalletMaps,
    ) -> impl FnOnce(&[u8; 32], &[u8; 32], &[u8; 32]) -> Result<KagemushaWalletIndexedInsertV1>
    {
        move |root, key, value| store.insert(SpartanWalletMap::Recovery, *root, *key, *value)
    }
    fn forge_slot_sibling(insertion: &mut KagemushaWalletIndexedInsertV1) {
        insertion.slot_opening.siblings[3][0] ^= 1;
    }
    fn forge_low_leaf(insertion: &mut KagemushaWalletIndexedInsertV1) {
        insertion.low.value[0] ^= 1;
    }
    fn forge_occupied_slot(insertion: &mut KagemushaWalletIndexedInsertV1) {
        insertion.slot_opening.slot = insertion.low_opening.slot;
    }

    fn originals(input: &bootstrap::Inputs) -> (Vec<u8>, Vec<u8>) {
        (
            credential_bytes(&input.credential),
            [
                input.certificate.body.as_slice(),
                input.certificate.signature.as_slice(),
            ]
            .concat(),
        )
    }

    fn credential_bytes(c: &Credential) -> Vec<u8> {
        [c.body.as_slice(), c.signature.as_slice()].concat()
    }

    /// Fixture issuer signature over a changed credential body.
    fn reissue(c: &mut Credential) {
        c.signature = bootstrap::sign(
            credential::message(c),
            &BigUint::from_bytes_be(&ISSUER_SECRET),
            "funding test issuer",
        );
    }

    /// The two receipt fields `Engine::prepare` freezes before its relation check.
    fn freeze(receipt: &mut bootstrap::Receipt) {
        receipt.body[242..274].copy_from_slice(&Fp::from(987).to_repr());
        receipt.body[274..306].fill(0x55);
    }

    /// Whether the unsigned Bootstrap relation `Engine::prepare` checks holds.
    fn bootstrap_relation_holds(f: &Fixture, input: &bootstrap::Inputs) -> bool {
        let mut input = input.clone();
        freeze(&mut input.receipt);
        let prepared = bootstrap::prepare_unsigned(&f.chain.sender_context, &input).unwrap();
        harness::holds(&harness::finish(prepared.wiring, 16))
    }

    /// Whether the Control preparation relation `Engine::prepare` checks holds
    /// for this Load after the predecessor with `profile`.
    fn load_relation_holds(f: &Fixture, step: &load::Witness, profile: [Fp; 64]) -> bool {
        let mut input = step.clone();
        freeze(&mut input.receipt);
        let mut w = full::Wiring::default();
        let family = w.family(0, 1);
        let prior = harness::predecessor(&mut w, family, profile);
        let prepared = crate::control::append_preparation(
            &mut w,
            family,
            &f.chain.sender_context.root.root,
            &crate::control::Witness::load(input),
            &prior,
        )
        .unwrap();
        harness::expose(
            &mut w,
            family,
            4,
            &[prepared.head(), prepared.body().message()],
        );
        harness::holds(&harness::finish(w, 16))
    }

    fn own_receipt(witness: &mut ExactOperationWitness) -> &mut bootstrap::Receipt {
        match witness {
            ExactOperationWitness::Bootstrap(w) => &mut w.input.receipt,
            ExactOperationWitness::Load { step, .. } => &mut step.receipt,
            _ => unreachable!("funding tests release only Bootstrap and Load"),
        }
    }

    /// What the engine and provider hold once `witness` is released: its frozen
    /// preparation and the custody-bound head with public `profile`.
    fn release(
        f: &Fixture,
        mut witness: ExactOperationWitness,
        profile: [Fp; 64],
    ) -> (PreparedOperation, VerifiedWalletHead) {
        let installed = f.installed();
        let enrollment = matches!(witness, ExactOperationWitness::Bootstrap(_));
        let credential = witness.credential().clone();
        let receipt = own_receipt(&mut witness);
        receipt.signature = [0; 64];
        receipt.body[242..274].copy_from_slice(&Fp::from(987).to_repr());
        receipt.body[274..306].fill(0);
        let body = receipt.body;
        let capsule = RecoveryCapsule {
            frame: CapsuleFrame {
                version: RECOVERY_CAPSULE_VERSION,
                parameters_id: *installed.id,
                manifest_digest: *installed.manifest_digest,
                scheme: credential.body[2..34].try_into().unwrap(),
                asset: credential.body[34..66].try_into().unwrap(),
                wallet: credential.body[66..98].try_into().unwrap(),
                payment_key: credential.body[130..195].to_vec(),
                kind: if enrollment { 1 } else { 2 },
                sequence: u128::from_le_bytes(body[98..114].try_into().unwrap()),
                predecessor: body[146..178].try_into().unwrap(),
                predecessor_capsule_digest: if enrollment { [0; 32] } else { [0x21; 32] },
                successor: body[178..210].try_into().unwrap(),
                operation_id: body[114..146].try_into().unwrap(),
                proof_digest: body[242..274].try_into().unwrap(),
                enrollment_marker: enrollment.then_some(MARKER),
                receipt_body: body.to_vec(),
                recovery: vec![1],
            },
        };
        let digest = RecoveryCapsule::frame_digest(&capsule.encode().unwrap());
        own_receipt(&mut witness).body[274..306].copy_from_slice(&digest);
        let body = own_receipt(&mut witness).body;
        let head = VerifiedWalletHead {
            parameters_id: *installed.id,
            manifest_digest: *installed.manifest_digest,
            profile,
            pair: Arc::from(vec![1u8]),
            credential,
            certificate: f.chain.sender_bootstrap.certificate.clone(),
            custody_digest: digest,
            enrollment_marker: None,
        };
        (
            PreparedOperation {
                manifest_digest: *installed.manifest_digest,
                parameters_id: *installed.id,
                capsule,
                witness: Arc::new(witness),
                receipt_body: body,
                receive: None,
                predecessor: head.clone(),
            },
            head,
        )
    }

    /// Public profile of the head a Load releases after `prior`.
    fn after_load_profile(prior: &[Fp; 64], step: &load::Witness) -> [Fp; 64] {
        crate::chain_fixture::public_after(
            prior,
            &step.statement,
            &step.receipt,
            prior[32..38].try_into().unwrap(),
            full::CONTROL,
        )
    }

    fn load_receipt(c: &Credential, ordinal: u128, amount: u128) -> KagemushaWalletLoadReceiptV1 {
        let mut request_id = [0x45; 32];
        let mut transaction_hash = [0x56; 32];
        request_id[31] ^= ordinal.to_le_bytes()[0];
        transaction_hash[31] ^= ordinal.to_le_bytes()[0];
        request_id[30] ^= amount.to_le_bytes()[0];
        transaction_hash[30] ^= amount.to_le_bytes()[0];
        KagemushaWalletLoadReceiptV1 {
            version: 1,
            scheme_id: c.body[2..34].try_into().unwrap(),
            asset_digest: c.body[34..66].try_into().unwrap(),
            wallet_id: c.body[66..98].try_into().unwrap(),
            request_id,
            ordinal,
            amount,
            online_charge: 0,
            charge_quote: [0; 32],
            transaction_hash,
            // Replaced by the certified block's height.
            block_height: 2,
            payer_account_digest: c.body[98..130].try_into().unwrap(),
        }
    }

    /// Genuine fixture-genesis finality for every receipt, in one certified
    /// block, as `native_device_fixture` certifies its Loads. Returns the exact
    /// canonical receipt and finality originals.
    fn certify(mut receipts: Vec<KagemushaWalletLoadReceiptV1>) -> Vec<(Vec<u8>, Vec<u8>)> {
        use iroha_data_model::{
            events::{
                EventBox,
                data::{DataEvent, kagemusha::KagemushaLoadCommittedV1},
            },
            prelude::{HashOf, MerkleTree},
            sumeragi_finality::SumeragiCommitCertificateV1,
        };
        let mut consensus =
            NativeFinalityFixture::start_with_explicit_parameters("portable-native-fixture");
        let header = consensus.next_header();
        let height = header.height.get();
        for receipt in &mut receipts {
            receipt.block_height = height;
        }
        let events: Vec<EventBox> = receipts
            .iter()
            .map(|receipt| {
                let event = KagemushaLoadCommittedV1::from_receipt(receipt)
                    .map_err(|e| e.to_string())
                    .unwrap();
                EventBox::Data(DataEvent::KagemushaLoadCommitted(event).into())
            })
            .collect();
        let tree: MerkleTree<EventBox> = events.iter().map(HashOf::new).collect();
        let block = consensus.block_with_submitted_work(header);
        let proof = consensus.certify_with_events(block, &events);
        let certificate = SumeragiCommitCertificateV1::from_proof(&proof)
            .map_err(|e| e.to_string())
            .unwrap();
        receipts
            .iter()
            .enumerate()
            .map(|(i, receipt)| {
                let finality = KagemushaWalletLoadFinalityV1 {
                    version: 1,
                    receipt_digest: receipt.receipt_digest().map_err(|e| e.to_string()).unwrap(),
                    certificate: certificate.clone(),
                    event_proof: tree.get_proof(i as u32).unwrap(),
                };
                (
                    receipt
                        .to_canonical_bytes()
                        .map_err(|e| e.to_string())
                        .unwrap(),
                    finality
                        .to_canonical_bytes()
                        .map_err(|e| e.to_string())
                        .unwrap(),
                )
            })
            .collect()
    }

    fn assert_same_insertion(
        actual: &indexed_map::InsertWitness<32>,
        expected: &indexed_map::InsertWitness<32>,
    ) {
        assert_eq!(
            (actual.old_root, actual.key, actual.value),
            (expected.old_root, expected.key, expected.value)
        );
        assert_eq!(
            (actual.low.key, actual.low.value, actual.low.next),
            (expected.low.key, expected.low.value, expected.low.next)
        );
        assert_eq!(
            (actual.low_path.index, actual.low_path.siblings),
            (expected.low_path.index, expected.low_path.siblings)
        );
        assert_eq!(
            (actual.slot.index, actual.slot.siblings),
            (expected.slot.index, expected.slot.siblings)
        );
    }

    fn assert_canonical_recovery_roundtrip(witness: &ExactOperationWitness) {
        let bytes = witness.encode().unwrap();
        let restored = ExactOperationWitness::decode(&bytes).unwrap();
        assert_eq!(restored.encode().unwrap(), bytes);
    }

    #[test]
    fn state_nonce_is_fresh_nonzero_os_randomness() {
        let first = fresh_state_nonce(None).unwrap();
        let second = fresh_state_nonce(Some(first)).unwrap();
        assert_ne!(first, Fp::ZERO);
        assert_ne!(second, Fp::ZERO);
        assert_ne!(first, second);
    }

    #[test]
    fn bootstrap_matches_the_fixture_except_independent_state_blinding() {
        let f = Fixture::new();
        let installed = f.installed();
        let expected = &f.chain.sender_bootstrap;
        let (credential, issuer) = originals(expected);
        let build = |nonce: Fp| {
            let (c, cert, fields) = enrollment_originals(&installed, &credential, &issuer).unwrap();
            bootstrap_witness(&installed, c, cert, &fields, &MARKER, nonce).unwrap()
        };

        // The fixture's public parameters and its nonce: the same Bootstrap.
        let same = build(expected.core[29]);
        let ExactOperationWitness::Bootstrap(same) = &same else {
            unreachable!()
        };
        assert_eq!(same.scheme, f.chain.sender_context.root.scheme);
        assert_eq!(same.relation, f.chain.sender_context.relation);
        assert!(same.root == f.chain.sender_context.root.root);
        assert_eq!(same.input.credential.body, expected.credential.body);
        assert_eq!(
            same.input.credential.signature,
            expected.credential.signature
        );
        assert_eq!(same.input.certificate.body, expected.certificate.body);
        assert_eq!(
            same.input.certificate.signature,
            expected.certificate.signature
        );
        assert_eq!(same.input.core, expected.core);
        assert_eq!(same.input.rest, expected.rest);
        assert_eq!(same.input.statement, expected.statement);
        // The fixture signs and fills the two fields the engine freezes.
        assert_eq!(same.input.receipt.body[..242], expected.receipt.body[..242]);
        assert_eq!(same.input.receipt.body[306..], expected.receipt.body[306..]);
        assert!(same.input.receipt.body[242..306].iter().all(|b| *b == 0));
        assert!(same.input.receipt.signature.iter().all(|b| *b == 0));

        // Independent blinding moves only the nonce, the head and what signs it.
        let nonce = fresh_state_nonce(Some(expected.core[29])).unwrap();
        let fresh = build(nonce);
        assert_canonical_recovery_roundtrip(&fresh);
        let ExactOperationWitness::Bootstrap(fresh) = &fresh else {
            unreachable!()
        };
        assert_eq!(fresh.input.core[..29], expected.core[..29]);
        assert_eq!(fresh.input.core[29], nonce);
        assert_eq!(fresh.input.rest, expected.rest);
        let head = monetary_state::commitment(&fresh.input.core, &fresh.input.rest);
        assert_ne!(head, expected.statement[15]);
        assert_eq!(fresh.input.statement[..15], expected.statement[..15]);
        assert_eq!(fresh.input.statement[15], head);
        assert_eq!(fresh.input.statement[16..], expected.statement[16..]);
        let body = &fresh.input.receipt.body;
        // The operation identity (114..146) is durable across fresh blinding.
        assert_eq!(body[..178], expected.receipt.body[..178]);
        assert_eq!(body[178..210], head.to_repr());
        assert_eq!(
            body[210..242],
            statement::digest(&fresh.input.statement).to_repr()
        );
        assert!(body[242..].iter().all(|b| *b == 0));

        // The engine's unsigned Bootstrap preparation accepts it and still
        // refuses a state the credential does not determine.
        assert!(bootstrap_relation_holds(&f, &fresh.input));
        let mut funded = fresh.input.clone();
        funded.core[9] = Fp::ONE;
        funded.statement[15] = monetary_state::commitment(&funded.core, &funded.rest);
        funded.receipt.body[178..210].copy_from_slice(&funded.statement[15].to_repr());
        funded.receipt.body[210..242]
            .copy_from_slice(&statement::digest(&funded.statement).to_repr());
        assert!(!bootstrap_relation_holds(&f, &funded));
    }

    #[test]
    fn bootstrap_refuses_unauthenticated_or_foreign_enrollment_originals() {
        let f = Fixture::new();
        let installed = f.installed();
        let input = &f.chain.sender_bootstrap;
        let (credential, issuer) = originals(input);
        let admit = |scope: &Installed<'_>, credential: &[u8], issuer: &[u8]| {
            enrollment_originals(scope, credential, issuer).map(drop)
        };
        admit(&installed, &credential, &issuer).unwrap();

        // Exact lengths.
        let mut long = credential.clone();
        long.push(0);
        for c in [&credential[..595], long.as_slice()] {
            assert!(matches!(
                admit(&installed, c, &issuer),
                Err(Error::Malformed("enrollment credential length"))
            ));
        }
        assert!(matches!(
            admit(&installed, &credential, &issuer[..171]),
            Err(Error::Malformed(_))
        ));

        // Every credential and certificate byte is authenticated.
        for at in [0, 40, 100, 140, 200, 300, 450, 510, 540, 590] {
            let mut changed = credential.clone();
            changed[at] ^= 1;
            assert!(
                admit(&installed, &changed, &issuer).is_err(),
                "credential byte {at}"
            );
        }
        for at in [0, 10, 40, 80, 120, 170] {
            let mut changed = issuer.clone();
            changed[at] ^= 1;
            assert!(
                admit(&installed, &credential, &changed).is_err(),
                "issuer certificate byte {at}"
            );
        }

        // Another installed scheme or root.
        let mut other_scheme = *installed.scheme;
        other_scheme[0] ^= 1;
        let mut foreign = installed;
        foreign.scheme = &other_scheme;
        assert!(matches!(
            admit(&foreign, &credential, &issuer),
            Err(Error::AuthorityMismatch(_))
        ));
        let other_root = p256::mul(&p256::generator(), &BigUint::from(777u64)).unwrap();
        let mut foreign = installed;
        foreign.root = &other_root;
        assert!(matches!(
            admit(&foreign, &credential, &issuer),
            Err(Error::AuthorityMismatch(_))
        ));

        // A root-certified key of another role cannot issue credentials.
        let issuer_key =
            p256::mul(&p256::generator(), &BigUint::from_bytes_be(&ISSUER_SECRET)).unwrap();
        let artifact = certificate::issue_fixture(
            installed.scheme,
            certificate::Role::Artifact as u8,
            &issuer_key,
            7,
            &BigUint::from_bytes_be(&super::super::engine::TEST_ROOT_SECRET),
        );
        let mut wrong_role = input.credential.clone();
        wrong_role.body[500..532]
            .copy_from_slice(&certificate::certificate_digest(&artifact).to_repr());
        reissue(&mut wrong_role);
        assert!(matches!(
            admit(
                &installed,
                &credential_bytes(&wrong_role),
                &[artifact.body.as_slice(), artifact.signature.as_slice()].concat()
            ),
            Err(Error::AuthorityMismatch(_))
        ));

        // A correctly signed credential whose key table is another key's.
        let mut wrong_table = input.credential.clone();
        wrong_table.body[195..259]
            .copy_from_slice(&f.chain.receiver_bootstrap.credential.body[195..259]);
        reissue(&mut wrong_table);
        assert!(matches!(
            admit(&installed, &credential_bytes(&wrong_table), &issuer),
            Err(Error::AuthorityMismatch("certified wallet table"))
        ));

        // The relation's credential predicate alone admits this credential;
        // the enrollment-field validation of a new Bootstrap does not.
        let mut weak = input.credential.clone();
        weak.body[292..324].fill(0);
        reissue(&mut weak);
        assert!(
            super::super::engine::authenticate_credential_under(
                installed.scheme,
                installed.root,
                installed.table,
                &weak,
                &input.certificate
            )
            .is_ok()
        );
        assert!(admit(&installed, &credential_bytes(&weak), &issuer).is_err());

        // The marker and the blinder are mandatory.
        let (c, cert, fields) = enrollment_originals(&installed, &credential, &issuer).unwrap();
        assert!(matches!(
            bootstrap_witness(
                &installed,
                c.clone(),
                cert.clone(),
                &fields,
                &[0; 32],
                Fp::ONE
            ),
            Err(Error::AuthorityMismatch("enrollment marker"))
        ));
        assert!(matches!(
            bootstrap_witness(&installed, c, cert, &fields, &MARKER, Fp::ZERO),
            Err(Error::Unavailable("fresh state nonce"))
        ));
    }

    #[test]
    fn first_load_after_bootstrap_matches_the_fixture_and_the_load_relation() {
        let f = Fixture::new();
        let installed = f.installed();
        let fixture = device_fixture::load100(&f.bootstrap()).unwrap();
        let ExactOperationWitness::Load {
            step: expected,
            receipt_original,
            finality_original,
        } = &fixture
        else {
            unreachable!()
        };
        let profile = f.bootstrap_profile();
        let (released, verified) = release(&f, f.bootstrap(), profile);
        let mut store = RecoveryStore::of(&KagemushaWalletIndexedTreeV1::new());
        // The fixture's blinding is its predecessor nonce plus one.
        let built = load_witness(
            &installed,
            &released,
            &verified,
            receipt_original,
            finality_original,
            staging(&mut store),
            |previous| Ok(previous + Fp::ONE),
        )
        .unwrap();
        assert_eq!(store.asked, 1);
        assert_canonical_recovery_roundtrip(&built);
        let ExactOperationWitness::Load {
            step,
            receipt_original: kept_receipt,
            finality_original: kept_finality,
        } = &built
        else {
            unreachable!()
        };
        assert_eq!(kept_receipt[..], receipt_original[..]);
        assert_eq!(kept_finality[..], finality_original[..]);
        assert_eq!(step.before_core, expected.before_core);
        assert_eq!(step.before_rest, expected.before_rest);
        assert_eq!(step.after_core, expected.after_core);
        assert_eq!(step.after_rest, expected.after_rest);
        assert_eq!(step.credential.body, expected.credential.body);
        assert_eq!(step.credential.signature, expected.credential.signature);
        assert_eq!(step.ordinary, expected.ordinary);
        assert_eq!(step.statement, expected.statement);
        assert_eq!(step.receipt.body, expected.receipt.body);
        assert_eq!(step.receipt.signature, expected.receipt.signature);
        // The store's canonical map answers with the relation's own opening.
        assert_same_insertion(&step.recovery, &expected.recovery);
        assert_eq!(step.after_core[19].to_repr(), store.map.root());
        assert_eq!(step.after_core[9], Fp::from(100));
        assert!(load_relation_holds(&f, step, profile));
    }

    #[test]
    fn load_after_a_released_load_inserts_into_the_committed_map() {
        let f = Fixture::new();
        let installed = f.installed();
        let credential = &f.chain.sender_bootstrap.credential;
        let certified = certify(vec![
            load_receipt(credential, 0, 100),
            load_receipt(credential, 1, 7),
        ]);
        let boot_profile = f.bootstrap_profile();
        let (released, verified) = release(&f, f.bootstrap(), boot_profile);
        let empty = KagemushaWalletIndexedTreeV1::new();
        let mut store = RecoveryStore::of(&empty);
        let first = load_witness(
            &installed,
            &released,
            &verified,
            &certified[0].0,
            &certified[0].1,
            staging(&mut store),
            |previous| fresh_state_nonce(Some(previous)),
        )
        .unwrap();
        let ExactOperationWitness::Load { step, .. } = &first else {
            unreachable!()
        };
        // Independent blinding of the same Load keeps its operation identity.
        let again = load_witness(
            &installed,
            &released,
            &verified,
            &certified[0].0,
            &certified[0].1,
            staging(&mut RecoveryStore::of(&empty)),
            |previous| fresh_state_nonce(Some(previous)),
        )
        .unwrap();
        let ExactOperationWitness::Load { step: retry, .. } = &again else {
            unreachable!()
        };
        assert_ne!(retry.after_core[29], step.after_core[29]);
        assert_ne!(retry.statement[15], step.statement[15]);
        assert_eq!(retry.after_core[..29], step.after_core[..29]);
        assert_eq!(
            retry.receipt.body[114..146],
            step.receipt.body[114..146],
            "durable operation identity"
        );

        // The released Load is the predecessor of the next Load.
        let first_profile = after_load_profile(&boot_profile, step);
        let (released, verified) = release(&f, first.clone(), first_profile);
        let second = load_witness(
            &installed,
            &released,
            &verified,
            &certified[1].0,
            &certified[1].1,
            staging(&mut store),
            |previous| fresh_state_nonce(Some(previous)),
        )
        .unwrap();
        assert_canonical_recovery_roundtrip(&second);
        let ExactOperationWitness::Load { step: next, .. } = &second else {
            unreachable!()
        };
        assert_eq!(next.before_core, step.after_core);
        assert_eq!(next.before_rest, step.after_rest);
        assert_eq!(next.after_core[9], Fp::from(107));
        assert_eq!(next.after_core[11], Fp::from(2));
        assert_eq!(next.after_core[13], Fp::from(2));
        assert_eq!(next.after_core[19].to_repr(), store.map.root());
        assert_ne!(next.after_core[29], step.after_core[29]);
        assert_ne!(next.after_core[29], Fp::ZERO);
        assert_eq!(next.statement[9], Fp::from(2));
        assert_eq!(next.statement[10], Fp::from(2));
        assert_eq!(next.statement[14], step.statement[15]);
        assert_eq!(next.statement[18], Fp::ONE);
        assert_eq!(next.statement[19], Fp::from(7));
        // The relation's recovery entry is the canonical Load leaf.
        let leaf = KagemushaWalletLoadLeafV1 {
            ordinal: 1,
            receipt_digest: next.statement[17].to_repr(),
            amount: 7,
        };
        assert_eq!(next.recovery.key.to_repr(), leaf.key());
        assert_eq!(
            next.recovery.value.to_repr(),
            leaf.leaf_value().map_err(|e| e.to_string()).unwrap()
        );
        assert_eq!(next.recovery.old_root, step.after_core[19]);
        assert!(load_relation_holds(&f, next, first_profile));
    }

    #[test]
    fn load_refuses_wrong_scope_ordinal_overflow_opening_and_map_before_selection() {
        let f = Fixture::new();
        let installed = f.installed();
        let credential = &f.chain.sender_bootstrap.credential;
        let other = &f.chain.receiver_bootstrap.credential;
        let certified = certify(vec![
            load_receipt(credential, 0, 100),
            load_receipt(credential, 1, 100),
            load_receipt(other, 0, 100),
            load_receipt(credential, 1, u128::MAX),
        ]);
        let (valid, ahead, another_wallet, overflowing) =
            (&certified[0], &certified[1], &certified[2], &certified[3]);
        let profile = f.bootstrap_profile();
        let (released, verified) = release(&f, f.bootstrap(), profile);
        let empty = KagemushaWalletIndexedTreeV1::new();
        let attempt = |scope: &Installed<'_>,
                       released: &PreparedOperation,
                       verified: &VerifiedWalletHead,
                       receipt: &[u8],
                       finality: &[u8],
                       map: &KagemushaWalletIndexedTreeV1| {
            load_witness(
                scope,
                released,
                verified,
                receipt,
                finality,
                staging(&mut RecoveryStore::of(map)),
                |previous| Ok(previous + Fp::ONE),
            )
        };
        attempt(
            &installed, &released, &verified, &valid.0, &valid.1, &empty,
        )
        .unwrap();

        // Parameter scope of the installation, the head and the preparation.
        let mut other_id = *installed.id;
        other_id[0] ^= 1;
        let mut foreign = installed;
        foreign.id = &other_id;
        let mut foreign_head = verified.clone();
        foreign_head.parameters_id = other_id;
        for (scope, head) in [(&foreign, &verified), (&installed, &foreign_head)] {
            assert!(matches!(
                attempt(scope, &released, head, &valid.0, &valid.1, &empty),
                Err(Error::AuthorityMismatch("funding parameter identity"))
            ));
        }
        let other_manifest = [0x6e; 32];
        let mut reissued = installed;
        reissued.manifest_digest = &other_manifest;
        let mut reissued_head = verified.clone();
        reissued_head.manifest_digest = other_manifest;
        assert!(matches!(
            attempt(&installed, &released, &reissued_head, &valid.0, &valid.1, &empty),
            Err(Error::AuthorityMismatch("released producer manifest"))
        ));
        assert!(matches!(
            attempt(&reissued, &released, &verified, &valid.0, &valid.1, &empty),
            Err(Error::AuthorityMismatch("released producer manifest"))
        ));

        // A head not yet bound to custody is admitted; a head bound to another
        // capsule, an enrollment origin or a head without a recursive proof is
        // not this preparation's released head.
        let mut unbound = verified.clone();
        unbound.custody_digest = [0; 32];
        attempt(&installed, &released, &unbound, &valid.0, &valid.1, &empty).unwrap();
        let mut another_capsule = verified.clone();
        another_capsule.custody_digest[0] ^= 1;
        let mut origin = verified.clone();
        origin.enrollment_marker = Some(MARKER);
        let mut unproved = verified.clone();
        unproved.pair = Arc::from(Vec::new());
        for head in [&another_capsule, &origin, &unproved] {
            assert!(matches!(
                attempt(&installed, &released, head, &valid.0, &valid.1, &empty),
                Err(Error::AuthorityMismatch("released predecessor custody"))
            ));
        }
        let mut another_credential = verified.clone();
        another_credential.credential = other.clone();
        assert!(matches!(
            attempt(
                &installed,
                &released,
                &another_credential,
                &valid.0,
                &valid.1,
                &empty
            ),
            Err(Error::AuthorityMismatch("released credential"))
        ));

        // The private opening must be the verified public head's.
        for at in [
            full::HEAD_IO,
            context::LINEAGE_IO + context::LIFECYCLE,
            context::LINEAGE_IO + context::POLICY_EPOCH,
            context::LINEAGE_IO + context::ENABLED_CONTROLS,
        ] {
            let mut wrong = verified.clone();
            wrong.profile[at] += Fp::ONE;
            assert!(
                matches!(
                    attempt(&installed, &released, &wrong, &valid.0, &valid.1, &empty),
                    Err(Error::AuthorityMismatch("released successor opening"))
                ),
                "public profile word {at}"
            );
        }

        // Exact canonical originals under concrete finality.
        let mut trailing = valid.0.clone();
        trailing.push(0);
        assert!(
            attempt(
                &installed, &released, &verified, &trailing, &valid.1, &empty
            )
            .is_err()
        );
        assert!(
            attempt(
                &installed,
                &released,
                &verified,
                &valid.0[..valid.0.len() - 1],
                &valid.1,
                &empty
            )
            .is_err()
        );
        let mut trailing = valid.1.clone();
        trailing.push(0);
        assert!(
            attempt(
                &installed, &released, &verified, &valid.0, &trailing, &empty
            )
            .is_err()
        );
        // Genuine finality of another receipt; the store is never asked.
        let mut store = RecoveryStore::of(&empty);
        assert!(
            load_witness(
                &installed,
                &released,
                &verified,
                &valid.0,
                &another_wallet.1,
                staging(&mut store),
                |previous| Ok(previous + Fp::ONE),
            )
            .is_err()
        );
        assert_eq!(store.asked, 0, "the store is asked only after finality");

        // Wallet scope and ordinal order of genuinely finalized receipts.
        assert!(matches!(
            attempt(
                &installed,
                &released,
                &verified,
                &another_wallet.0,
                &another_wallet.1,
                &empty
            ),
            Err(Error::AuthorityMismatch("Load receipt wallet scope"))
        ));
        assert!(matches!(
            attempt(
                &installed, &released, &verified, &ahead.0, &ahead.1, &empty
            ),
            Err(Error::AuthorityMismatch(
                "Load ordinal is ahead of the next ordinal"
            ))
        ));

        // The store supplies openings only; they must fit the committed root.
        let mut unrelated = KagemushaWalletIndexedTreeV1::new();
        unrelated
            .insert(Fp::from(5).to_repr(), Fp::from(6).to_repr())
            .map_err(|e| e.to_string())
            .unwrap();
        let mut liar = RecoveryStore::of(&unrelated);
        liar.checks_root = false;
        assert!(matches!(
            load_witness(
                &installed,
                &released,
                &verified,
                &valid.0,
                &valid.1,
                staging(&mut liar),
                |previous| Ok(previous + Fp::ONE),
            ),
            Err(Error::AuthorityMismatch("Load recovery insertion"))
        ));
        assert_eq!(liar.asked, 1);
        for (case, forge) in [
            forge_slot_sibling as fn(&mut KagemushaWalletIndexedInsertV1),
            forge_low_leaf,
            forge_occupied_slot,
        ]
        .into_iter()
        .enumerate()
        {
            let mut forger = RecoveryStore::of(&empty);
            forger.forge = Some(forge);
            assert!(
                matches!(
                    load_witness(
                        &installed,
                        &released,
                        &verified,
                        &valid.0,
                        &valid.1,
                        staging(&mut forger),
                        |previous| Ok(previous + Fp::ONE),
                    ),
                    Err(Error::AuthorityMismatch("Load recovery insertion"))
                ),
                "forged insertion {case}"
            );
        }
        assert!(matches!(
            load_witness(
                &installed,
                &released,
                &verified,
                &valid.0,
                &valid.1,
                staging(&mut Offline),
                |previous| Ok(previous + Fp::ONE),
            ),
            Err(Error::Unavailable("test recovery map"))
        ));

        // The successor is blinded independently of its predecessor.
        for reused in [false, true] {
            assert!(matches!(
                load_witness(
                    &installed,
                    &released,
                    &verified,
                    &valid.0,
                    &valid.1,
                    staging(&mut RecoveryStore::of(&empty)),
                    |previous| Ok(if reused { previous } else { Fp::ZERO }),
                ),
                Err(Error::Unavailable("fresh state nonce"))
            ));
        }

        // After the first Load is released: no repeat, and no overflow.
        let mut store = RecoveryStore::of(&empty);
        let first = load_witness(
            &installed,
            &released,
            &verified,
            &valid.0,
            &valid.1,
            staging(&mut store),
            |previous| Ok(previous + Fp::ONE),
        )
        .unwrap();
        let ExactOperationWitness::Load { step, .. } = &first else {
            unreachable!()
        };
        let (released, verified) = release(&f, first.clone(), after_load_profile(&profile, step));
        let loaded = &store.map;
        assert!(matches!(
            attempt(
                &installed, &released, &verified, &valid.0, &valid.1, loaded
            ),
            Err(Error::AuthorityMismatch("Load ordinal already consumed"))
        ));
        assert!(matches!(
            attempt(
                &installed,
                &released,
                &verified,
                &overflowing.0,
                &overflowing.1,
                loaded
            ),
            Err(Error::Malformed("Load balance overflow"))
        ));
        attempt(
            &installed, &released, &verified, &ahead.0, &ahead.1, loaded,
        )
        .unwrap();
        // A store that did not record the released Load cannot answer.
        assert!(matches!(
            attempt(
                &installed, &released, &verified, &ahead.0, &ahead.1, &empty
            ),
            Err(Error::AuthorityMismatch("test store recovery root"))
        ));
    }

    /// Software stand-in for the fixture wallet's hardware boundary.
    fn sign(prepared: &PreparedOperation, d: &BigUint) -> HardwareAdvanceReceipt {
        let signature = bootstrap::sign(
            field(&prepared.signing_message()).unwrap(),
            d,
            "funding test software payment key",
        );
        HardwareAdvanceReceipt::verify_selected(prepared, prepared.receipt_body(), signature)
            .unwrap()
    }

    /// Both public constructors through the installed engine: Bootstrap from
    /// enrollment originals, then a Load from the resumed released capsule.
    #[test]
    #[ignore = "requires the authenticated full-backend-chain/native-device bundle"]
    fn funding_constructors_advance_through_the_installed_engine() {
        let path = std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("output/full-backend-chain/native-device")
            });
        let read = |name: &str| std::fs::read(path.join(name)).unwrap();
        let parameters = device_fixture::install(
            &read("program-bundle.norito"),
            &read("parameter-manifest.norito"),
            read("parameter-signature.bin").try_into().unwrap(),
        )
        .unwrap();
        let engine = NativeEngine::default();
        let chain = device_fixture::chain(&parameters.inner.generators.p);
        let (credential, issuer) = originals(&chain.sender_bootstrap);
        let marker = [0x57; 32];

        let boot = engine
            .bootstrap_from_enrollment(&credential, &issuer, marker, &parameters)
            .unwrap();
        let ExactOperationWitness::Bootstrap(input) = &boot else {
            unreachable!()
        };
        let origin = engine
            .enrollment_origin(input, marker, &parameters)
            .unwrap();
        let other_origin = engine.enrollment_origin(input, [0x58; 32], &parameters).unwrap();
        assert!(matches!(engine.prepare(&boot, &other_origin, &parameters), Err(Error::AuthorityMismatch("bootstrap enrollment origin"))));
        let prepared = engine.prepare(&boot, &origin, &parameters).unwrap();
        let capsule = prepared.capsule().clone();
        let signed = sign(&prepared, &chain.sender_key.d);
        let enrolled = engine.finalize(prepared, &signed, &parameters).unwrap();
        assert_eq!(
            enrolled.head().profile()[full::HEAD_IO],
            input.input.statement[15]
        );

        let released = engine.resume(&capsule, &parameters).unwrap();
        let certified = certify(vec![load_receipt(&chain.sender_bootstrap.credential, 0, 100)]);
        let mut store = RecoveryStore::of(&KagemushaWalletIndexedTreeV1::new());
        let funded = engine
            .load_from_finality(
                &released,
                enrolled.head(),
                &certified[0].0,
                &certified[0].1,
                staging(&mut store),
                &parameters,
            )
            .unwrap();
        let ExactOperationWitness::Load { step, .. } = &funded else {
            unreachable!()
        };
        let prepared = engine
            .prepare(&funded, enrolled.head(), &parameters)
            .unwrap();
        let signed = sign(&prepared, &chain.sender_key.d);
        let loaded = engine.finalize(prepared, &signed, &parameters).unwrap();
        assert_eq!(loaded.head().profile()[full::HEAD_IO], step.statement[15]);
        assert_eq!(step.after_core[9], Fp::from(100));
    }
}
