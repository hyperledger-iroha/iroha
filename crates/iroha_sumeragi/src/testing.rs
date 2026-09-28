//! Deterministic fake cryptography and harness keys for tests and the simulator (spec §13.1).
//!
//! - [`FakeCrypto`]: SHA-256 as `H`; signatures are `SIGNATURE_LEN` bytes derived from the key
//!   and message; aggregates are limb-wise sums modulo `2^256`, so they are order-independent
//!   like BLS and verifiable from the keys and messages alone.
//! - [`FakeSigner`] records the provenance `(key, preimage, signature)` of every signature it
//!   produces in an optional shared [`SignLog`]; [`FakeCrypto`] records the parts of every
//!   aggregate it forms. Oracles (O-SIGN, O-CERT) check certificates against this log, so a
//!   signature that verifies but was never produced by its key's signer is detected.
//! - [`FakeValidators`]: a committee of harness-held keys with helpers that sign votes,
//!   timeouts, proposals and build certificates (including deliberately malformed ones).
//! - [`FakeAttestor`] and [`FakeVerifier`]: the commit-attestation extension (§3.7) with a keyed
//!   MAC per member key and height standing in for the application's signature, so a forged,
//!   stripped or replayed attestation is told apart from a genuine one.

/// Explicit stationary scheduling context for protocol-unit fixtures.
/// Rotation harnesses replace bounds and identity from their declared schedule.
pub const TEST_EPOCH: crate::types::EpochConfig = crate::types::EpochConfig {
    id: crate::types::EpochId {
        epoch: 0,
        context: crate::types::Hash32([0xE0; 32]),
    },
    authority_generation: crate::types::Hash32([0xE1; 32]),
    first_height: 0,
    last_height: u64::MAX,
    leader_seed: crate::types::Hash32([0xE2; 32]),
};

/// Explicit successor identity for test schedules; no production caller derives authority here.
pub fn scheduled_epoch(
    epoch: u64,
    first_height: u64,
    last_height: u64,
) -> crate::types::EpochConfig {
    let mut context = TEST_EPOCH;
    context.id.epoch = epoch;
    context.first_height = first_height;
    context.last_height = last_height;
    if epoch != 0 || first_height != 0 || last_height != u64::MAX {
        let mut bytes = b"sumeragi/test-epoch".to_vec();
        bytes.extend_from_slice(&epoch.to_be_bytes());
        bytes.extend_from_slice(&first_height.to_be_bytes());
        bytes.extend_from_slice(&last_height.to_be_bytes());
        context.id.context = crate::types::Hash32(sha256(&bytes));
        context.authority_generation = context.id.context;
        bytes.push(1);
        context.leader_seed = crate::types::Hash32(sha256(&bytes));
    }
    context
}

/// Window slot after an applied cut, with no guessed next-epoch roster.
pub fn window_slot(
    active: &crate::types::HeightConfig,
    height: u64,
    config: crate::types::HeightConfig,
) -> crate::types::ConfigSlot {
    if active.epoch.contains(height) {
        crate::types::ConfigSlot::Ready(config)
    } else {
        crate::types::ConfigSlot::PendingBoundary {
            boundary_height: active.epoch.last_height,
            predecessor: active.epoch.id,
        }
    }
}

/// The atomic application result supplied by a simulated original execution.
pub fn applied_config(
    height: u64,
    current: &crate::types::HeightConfig,
    next: crate::types::HeightConfig,
    after_next: crate::types::HeightConfig,
) -> crate::types::AppliedConfig {
    if height == current.epoch.last_height {
        crate::types::AppliedConfig::Boundary { next, after_next }
    } else {
        crate::types::AppliedConfig::Continuation {
            after_next: window_slot(current, height + 2, after_next),
        }
    }
}

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use crate::{
    crypto::{AttestOutcome, Attestation, AttestationVerifier, Attestor, Crypto, Signer},
    message::{BlockHeader, Proposal, Qc, TcEntry, TimeoutCert, TimeoutVote, Vote, VoteKind},
    preimage,
    types::{
        AggregateSignature, Bitmap, Committee, Hash32, PublicKey, SIGNATURE_LEN, Signature,
        ValidatorIndex, usize_of,
    },
};

const K: [u32; 64] = [
    0x428a_2f98,
    0x7137_4491,
    0xb5c0_fbcf,
    0xe9b5_dba5,
    0x3956_c25b,
    0x59f1_11f1,
    0x923f_82a4,
    0xab1c_5ed5,
    0xd807_aa98,
    0x1283_5b01,
    0x2431_85be,
    0x550c_7dc3,
    0x72be_5d74,
    0x80de_b1fe,
    0x9bdc_06a7,
    0xc19b_f174,
    0xe49b_69c1,
    0xefbe_4786,
    0x0fc1_9dc6,
    0x240c_a1cc,
    0x2de9_2c6f,
    0x4a74_84aa,
    0x5cb0_a9dc,
    0x76f9_88da,
    0x983e_5152,
    0xa831_c66d,
    0xb003_27c8,
    0xbf59_7fc7,
    0xc6e0_0bf3,
    0xd5a7_9147,
    0x06ca_6351,
    0x1429_2967,
    0x27b7_0a85,
    0x2e1b_2138,
    0x4d2c_6dfc,
    0x5338_0d13,
    0x650a_7354,
    0x766a_0abb,
    0x81c2_c92e,
    0x9272_2c85,
    0xa2bf_e8a1,
    0xa81a_664b,
    0xc24b_8b70,
    0xc76c_51a3,
    0xd192_e819,
    0xd699_0624,
    0xf40e_3585,
    0x106a_a070,
    0x19a4_c116,
    0x1e37_6c08,
    0x2748_774c,
    0x34b0_bcb5,
    0x391c_0cb3,
    0x4ed8_aa4a,
    0x5b9c_ca4f,
    0x682e_6ff3,
    0x748f_82ee,
    0x78a5_636f,
    0x84c8_7814,
    0x8cc7_0208,
    0x90be_fffa,
    0xa450_6ceb,
    0xbef9_a3f7,
    0xc671_78f2,
];

/// SHA-256 (FIPS 180-4), the fake scheme's `H`.
#[allow(clippy::many_single_char_names)] // FIPS 180-4 names
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut state: [u32; 8] = [
        0x6a09_e667,
        0xbb67_ae85,
        0x3c6e_f372,
        0xa54f_f53a,
        0x510e_527f,
        0x9b05_688c,
        0x1f83_d9ab,
        0x5be0_cd19,
    ];
    let bit_len = u64::try_from(data.len())
        .unwrap_or(u64::MAX)
        .wrapping_mul(8);
    let mut message = data.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());
    for block in message.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (word, bytes) in w.iter_mut().zip(block.chunks_exact(4)) {
            *word = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for (k, word) in K.iter().zip(w.iter()) {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(*k)
                .wrapping_add(*word);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (slot, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(value);
        }
    }
    let mut out = [0u8; 32];
    for (chunk, word) in out.chunks_exact_mut(4).zip(state) {
        chunk.copy_from_slice(&word.to_be_bytes());
    }
    out
}

const TAG_FAKE_SIG: &[u8] = b"sumeragi/fake-sig";
const TAG_FAKE_PK: &[u8] = b"sumeragi/fake-pk";
const TAG_FAKE_ATTEST: &[u8] = b"sumeragi/fake-attest";

/// The fake attestation (§3.7) of `statement` by member `key` at `height`:
/// `SHA-256("sumeragi/fake-attest" ‖ kb(key) ‖ be64(height) ‖ statement)`, a keyed MAC standing
/// in for the application's signature under the member's key of that height.
pub fn fake_attestation(
    key: &PublicKey,
    height: u64,
    statement: &[u8],
) -> crate::message::CommitAttestation {
    let mut input = TAG_FAKE_ATTEST.to_vec();
    input.extend_from_slice(&preimage::kb(key));
    input.extend_from_slice(&height.to_be_bytes());
    input.extend_from_slice(statement);
    crate::message::CommitAttestation {
        witness: crate::message::ResultWitness::from_untrusted(statement.to_vec())
            .expect("nonempty bounded test statement"),
        signature: crate::message::AttestationSignature::try_from_slice(&sha256(&input)).unwrap(),
    }
}

/// The statements `(height, att_preimage(height, bh, R))` of the blocks a node executed to
/// `Valid(R)`, shared between its executor and an execution-gated [`FakeAttestor`]: like a
/// KAGEMUSHA authority, which needs `R`'s preimage from its own execution, it attests only
/// these (§3.7 A2).
#[derive(Clone, Debug, Default)]
pub struct Executed(Arc<Mutex<Statements>>);

/// `(height, statement)` pairs of [`Executed`].
type Statements = BTreeSet<(u64, Vec<u8>)>;

impl Executed {
    /// An empty set.
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, Statements> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Record execution of `bh` under its authenticated epoch to `Valid(result)`.
    pub fn record(
        &self,
        instance: &Hash32,
        epoch: &crate::types::EpochId,
        height: u64,
        bh: &Hash32,
        result: &Hash32,
    ) {
        let statement = preimage::att_preimage(instance, epoch, height, bh, result);
        self.lock().insert((height, statement));
    }

    /// Whether `statement` at `height` was recorded.
    pub fn contains(&self, height: u64, statement: &[u8]) -> bool {
        self.lock().contains(&(height, statement.to_vec()))
    }

    /// Forget the executions at heights `≤ height` (applied: no Commit vote needs them).
    pub fn prune_through(&self, height: u64) {
        self.lock().retain(|(h, _)| *h > height);
    }

    /// Forget every execution (the node crashed).
    pub fn clear(&self) {
        self.lock().clear();
    }

    /// Number of recorded executions.
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    /// Whether nothing is recorded.
    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }
}

/// A fake application authority (§3.7): it attests with [`fake_attestation`] for every member
/// key except those it holds no authority for, or forges (attestations that never verify); an
/// execution-gated one answers `Pending` for a block its node has not executed.
#[derive(Clone, Debug, Default)]
pub struct FakeAttestor {
    without: BTreeSet<PublicKey>,
    forge: bool,
    executed: Option<Executed>,
}

impl FakeAttestor {
    /// An authority for every key.
    pub fn new() -> Self {
        Self::default()
    }

    /// An authority for every key except `keys` (`attest` answers `NoAuthority` for them).
    pub fn without_authority(keys: impl IntoIterator<Item = PublicKey>) -> Self {
        Self {
            without: keys.into_iter().collect(),
            ..Self::default()
        }
    }

    /// A Byzantine authority whose attestations never verify.
    pub fn forging() -> Self {
        Self {
            forge: true,
            ..Self::default()
        }
    }

    /// This authority, answering `Pending` for every statement not in `executed`.
    #[must_use]
    pub fn after_execution(self, executed: Executed) -> Self {
        Self {
            executed: Some(executed),
            ..self
        }
    }
}

impl Attestor for FakeAttestor {
    fn attest(&self, height: u64, key: &PublicKey, statement: &[u8]) -> AttestOutcome {
        if self.without.contains(key) {
            return AttestOutcome::NoAuthority;
        }
        if (self.executed.as_ref()).is_some_and(|executed| !executed.contains(height, statement)) {
            return AttestOutcome::Pending;
        }
        let mut attestation = fake_attestation(key, height, statement);
        if self.forge {
            let mut bytes = attestation.signature.as_slice().to_vec();
            bytes[0] ^= 0xff;
            attestation.signature =
                crate::message::AttestationSignature::try_from_slice(&bytes).unwrap();
        }
        AttestOutcome::Attested(attestation)
    }
}

/// The fake attestation verifier (§3.7): an attestation verifies iff it is
/// [`fake_attestation`] of the statement by the member key at the height.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FakeVerifier;

impl AttestationVerifier for FakeVerifier {
    fn verify(
        &self,
        height: u64,
        _signer: ValidatorIndex,
        key: &PublicKey,
        statement: &[u8],
        witness: &crate::message::ResultWitness,
        attestation: &[u8],
    ) -> bool {
        witness.as_slice() == statement
            && fake_attestation(key, height, statement)
                .signature
                .as_slice()
                == attestation
    }
}

/// The fake commit-attestation extension: `attestor` with [`FakeVerifier`].
pub fn fake_attestation_ext(attestor: FakeAttestor) -> Attestation {
    Attestation::new(Box::new(attestor), Box::new(FakeVerifier))
}
const LIMBS: usize = SIGNATURE_LEN / 32;

/// The fake signature of `key` over `msg`: limb `i` is
/// `SHA-256("sumeragi/fake-sig" ‖ i ‖ kb(key) ‖ msg)`.
pub fn fake_sig(key: &PublicKey, msg: &[u8]) -> Signature {
    let mut out = [0u8; SIGNATURE_LEN];
    let kb = preimage::kb(key);
    for (limb, chunk) in out.chunks_exact_mut(32).enumerate() {
        let mut input = Vec::with_capacity(TAG_FAKE_SIG.len() + 1 + kb.len() + msg.len());
        input.extend_from_slice(TAG_FAKE_SIG);
        input.push(u8::try_from(limb).unwrap_or(u8::MAX));
        input.extend_from_slice(&kb);
        input.extend_from_slice(msg);
        chunk.copy_from_slice(&sha256(&input));
    }
    Signature(out)
}

/// Limb-wise sum modulo `2^256` of `SIGNATURE_LEN`-byte big-endian values.
fn add_into(acc: &mut [u8; SIGNATURE_LEN], value: &[u8; SIGNATURE_LEN]) {
    for limb in 0..LIMBS {
        let mut carry = 0u16;
        for i in (limb * 32..(limb + 1) * 32).rev() {
            let sum = u16::from(acc[i]) + u16::from(value[i]) + carry;
            acc[i] = sum.to_be_bytes()[1];
            carry = sum >> 8;
        }
    }
}

/// Deterministic fake [`Crypto`] with optional aggregate provenance recording.
#[derive(Clone, Debug, Default)]
pub struct FakeCrypto {
    log: Option<SignLog>,
}

impl FakeCrypto {
    /// A fake scheme that records nothing.
    pub fn new() -> Self {
        Self { log: None }
    }

    /// A fake scheme that records the parts of every aggregate in `log`.
    pub fn with_log(log: SignLog) -> Self {
        Self { log: Some(log) }
    }

    /// The provenance log, if any.
    pub fn log(&self) -> Option<&SignLog> {
        self.log.as_ref()
    }

    fn sum(sigs: &[Signature]) -> AggregateSignature {
        let mut acc = [0u8; SIGNATURE_LEN];
        for sig in sigs {
            add_into(&mut acc, &sig.0);
        }
        AggregateSignature(acc)
    }
}

impl Crypto for FakeCrypto {
    fn hash(&self, bytes: &[u8]) -> Hash32 {
        Hash32(sha256(bytes))
    }

    fn verify(&self, pk: &PublicKey, msg: &[u8], sig: &Signature) -> bool {
        fake_sig(pk, msg) == *sig
    }

    fn aggregate(&self, sigs: &[Signature]) -> AggregateSignature {
        let agg = Self::sum(sigs);
        if let Some(log) = &self.log {
            log.record_aggregate(agg, sigs.to_vec());
        }
        agg
    }

    fn verify_aggregate(&self, pks: &[&PublicKey], msg: &[u8], agg: &AggregateSignature) -> bool {
        if pks.is_empty() {
            return false;
        }
        let expected: Vec<Signature> = pks.iter().map(|pk| fake_sig(pk, msg)).collect();
        Self::sum(&expected) == *agg
    }

    fn verify_aggregate_multi(
        &self,
        groups: &[(Vec<&PublicKey>, Vec<u8>)],
        agg: &AggregateSignature,
    ) -> bool {
        if groups.is_empty() || groups.iter().any(|(pks, _)| pks.is_empty()) {
            return false;
        }
        let expected: Vec<Signature> = groups
            .iter()
            .flat_map(|(pks, msg)| pks.iter().map(move |pk| fake_sig(pk, msg)))
            .collect();
        Self::sum(&expected) == *agg
    }
}

/// One signature produced by a [`FakeSigner`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignRecord {
    /// Signing key.
    pub key: PublicKey,
    /// Signed preimage.
    pub preimage: Vec<u8>,
    /// Produced signature.
    pub sig: Signature,
}

#[derive(Debug, Default)]
struct LogInner {
    signatures: Vec<SignRecord>,
    signed: BTreeSet<(PublicKey, Vec<u8>)>,
    aggregates: BTreeMap<AggregateSignature, Vec<Signature>>,
}

/// Shared provenance log of every genuine signature and every aggregate formed.
#[derive(Clone, Debug, Default)]
pub struct SignLog {
    inner: Arc<Mutex<LogInner>>,
}

impl SignLog {
    /// An empty log.
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, LogInner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Record a genuine signature.
    pub fn record_signature(&self, key: &PublicKey, preimage: &[u8], sig: Signature) {
        let mut inner = self.lock();
        inner.signed.insert((key.clone(), preimage.to_vec()));
        inner.signatures.push(SignRecord {
            key: key.clone(),
            preimage: preimage.to_vec(),
            sig,
        });
    }

    /// Record the parts of an aggregate.
    pub fn record_aggregate(&self, agg: AggregateSignature, parts: Vec<Signature>) {
        self.lock().aggregates.insert(agg, parts);
    }

    /// Every recorded signature in production order.
    pub fn signatures(&self) -> Vec<SignRecord> {
        self.lock().signatures.clone()
    }

    /// Every recorded signature of `key` in production order.
    pub fn signatures_by(&self, key: &PublicKey) -> Vec<SignRecord> {
        self.lock()
            .signatures
            .iter()
            .filter(|record| &record.key == key)
            .cloned()
            .collect()
    }

    /// Number of recorded signatures.
    pub fn len(&self) -> usize {
        self.lock().signatures.len()
    }

    /// Whether no signature was recorded.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether `key`'s signer genuinely signed `preimage`.
    pub fn was_signed(&self, key: &PublicKey, preimage: &[u8]) -> bool {
        self.lock()
            .signed
            .contains(&(key.clone(), preimage.to_vec()))
    }

    /// The parts of an aggregate formed by a logging [`FakeCrypto`].
    pub fn aggregate_parts(&self, agg: &AggregateSignature) -> Option<Vec<Signature>> {
        self.lock().aggregates.get(agg).cloned()
    }

    /// O-CERT for a QC: every signer of `committee = C_{qc.height}` in the bitmap genuinely
    /// signed the vote preimage.
    pub fn qc_provenance_ok(&self, committee: &Committee, qc: &Qc) -> bool {
        let preimage = qc.preimage();
        committee
            .keys_of(&qc.signers)
            .is_some_and(|keys| keys.iter().all(|key| self.was_signed(key, &preimage)))
    }

    /// O-CERT for a TC: every entry's signer genuinely signed its timeout preimage, and the
    /// attached `PrepareQC` (if any) passes [`SignLog::qc_provenance_ok`].
    pub fn tc_provenance_ok(&self, committee: &Committee, tc: &TimeoutCert) -> bool {
        let entries_ok = tc.entries.iter().all(|entry| {
            committee.get(entry.signer).is_some_and(|key| {
                self.was_signed(
                    key,
                    &preimage::tmo_preimage(&tc.instance, &tc.epoch, tc.height, tc.view, entry.hq),
                )
            })
        });
        entries_ok
            && tc
                .high_pqc
                .as_ref()
                .is_none_or(|qc| self.qc_provenance_ok(committee, qc))
    }
}

/// A deterministic fake signing key that records its signatures.
#[derive(Clone, Debug)]
pub struct FakeSigner {
    key: PublicKey,
    log: Option<SignLog>,
}

impl FakeSigner {
    /// The signer whose 32-byte key is `SHA-256("sumeragi/fake-pk" ‖ seed)`.
    pub fn from_seed(seed: &[u8], log: Option<SignLog>) -> Self {
        let mut input = TAG_FAKE_PK.to_vec();
        input.extend_from_slice(seed);
        let key = PublicKey::new(sha256(&input).to_vec()).expect("a 32-byte key is well formed");
        Self { key, log }
    }

    /// A signer for an explicit key.
    pub fn with_key(key: PublicKey, log: Option<SignLog>) -> Self {
        Self { key, log }
    }
}

impl Signer for FakeSigner {
    fn public_key(&self) -> &PublicKey {
        &self.key
    }

    fn sign(&self, preimage: &[u8]) -> Signature {
        let sig = fake_sig(&self.key, preimage);
        if let Some(log) = &self.log {
            log.record_signature(&self.key, preimage, sig);
        }
        sig
    }
}

/// A committee of harness-held fake keys (canonical order) with signing helpers.
#[derive(Clone, Debug)]
pub struct FakeValidators {
    /// Fake crypto (logging into the same log as the signers, if any).
    pub crypto: FakeCrypto,
    /// The committee.
    pub committee: Committee,
    signers: Vec<FakeSigner>,
}

// Harness helpers mirror the protocol's tuples `(kind, I, h, v, bh, R)`.
#[allow(clippy::too_many_arguments)]
impl FakeValidators {
    /// `n` keys derived from `(seed, i)`; `log` records every signature and aggregate.
    pub fn new(n: usize, seed: u64, log: Option<SignLog>) -> Self {
        let mut signers: Vec<FakeSigner> = (0..n)
            .map(|i| {
                let mut s = seed.to_be_bytes().to_vec();
                s.extend_from_slice(&u64::try_from(i).unwrap_or(u64::MAX).to_be_bytes());
                FakeSigner::from_seed(&s, log.clone())
            })
            .collect();
        signers.sort_by(|a, b| a.key.cmp(&b.key));
        let committee = Committee::new(signers.iter().map(|s| s.key.clone()).collect())
            .unwrap_or_else(|e| panic!("fake committee: {e}"));
        let crypto = log.map_or_else(FakeCrypto::new, FakeCrypto::with_log);
        Self {
            crypto,
            committee,
            signers,
        }
    }

    /// The signer of canonical index `index`.
    pub fn signer(&self, index: ValidatorIndex) -> &FakeSigner {
        &self.signers[usize_of(index)]
    }

    /// All signers in canonical order.
    pub fn signers(&self) -> &[FakeSigner] {
        &self.signers
    }

    /// The key of canonical index `index`.
    pub fn key(&self, index: ValidatorIndex) -> PublicKey {
        self.signer(index).key.clone()
    }

    /// A signed vote of an unflagged block.
    pub fn vote(
        &self,
        kind: VoteKind,
        signer: ValidatorIndex,
        instance: &Hash32,
        height: u64,
        view: u64,
        block_hash: &Hash32,
        result: &Hash32,
    ) -> Vote {
        self.vote_flagged(
            kind, signer, instance, height, view, block_hash, result, false,
        )
    }

    /// A signed vote with the attestation flag `attest`; a flagged Commit vote carries the
    /// signer's genuine [`fake_attestation`] (§3.7).
    pub fn vote_flagged(
        &self,
        kind: VoteKind,
        signer: ValidatorIndex,
        instance: &Hash32,
        height: u64,
        view: u64,
        block_hash: &Hash32,
        result: &Hash32,
        attest: bool,
    ) -> Vote {
        let msg = preimage::vote_preimage(
            kind,
            instance,
            &crate::testing::TEST_EPOCH.id,
            height,
            view,
            block_hash,
            result,
            attest,
        );
        let statement = preimage::att_preimage(
            instance,
            &crate::testing::TEST_EPOCH.id,
            height,
            block_hash,
            result,
        );
        Vote {
            epoch: crate::testing::TEST_EPOCH.id,
            kind,
            instance: *instance,
            height,
            view,
            block_hash: *block_hash,
            result: *result,
            attest,
            signer,
            sig: self.signer(signer).sign(&msg),
            attestation: (kind == VoteKind::Commit && attest)
                .then(|| fake_attestation(&self.key(signer), height, &statement)),
        }
    }

    /// A certificate of an unflagged block signed by exactly `signers` (any number, even below
    /// quorum).
    pub fn qc(
        &self,
        kind: VoteKind,
        instance: &Hash32,
        height: u64,
        view: u64,
        block_hash: &Hash32,
        result: &Hash32,
        signers: &[ValidatorIndex],
    ) -> Qc {
        self.qc_flagged(
            kind, instance, height, view, block_hash, result, signers, false,
        )
    }

    /// A certificate with the attestation flag `attest` signed by exactly `signers`; a flagged
    /// `CommitQC` carries the signers' genuine [`fake_attestation`]s in signer order (§3.7 A4).
    pub fn qc_flagged(
        &self,
        kind: VoteKind,
        instance: &Hash32,
        height: u64,
        view: u64,
        block_hash: &Hash32,
        result: &Hash32,
        signers: &[ValidatorIndex],
        attest: bool,
    ) -> Qc {
        let msg = preimage::vote_preimage(
            kind,
            instance,
            &crate::testing::TEST_EPOCH.id,
            height,
            view,
            block_hash,
            result,
            attest,
        );
        let statement = preimage::att_preimage(
            instance,
            &crate::testing::TEST_EPOCH.id,
            height,
            block_hash,
            result,
        );
        let mut sorted = signers.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        let sigs: Vec<Signature> = sorted
            .iter()
            .map(|index| self.signer(*index).sign(&msg))
            .collect();
        let attestations = if kind == VoteKind::Commit && attest {
            sorted
                .iter()
                .map(|index| fake_attestation(&self.key(*index), height, &statement).signature)
                .collect()
        } else {
            Vec::new()
        };
        Qc {
            attestation_witness: (kind == VoteKind::Commit && attest)
                .then(|| crate::message::ResultWitness::from_untrusted(statement.clone()).unwrap()),
            epoch: crate::testing::TEST_EPOCH.id,
            kind,
            instance: *instance,
            height,
            view,
            block_hash: *block_hash,
            result: *result,
            attest,
            signers: Bitmap::from_indices(self.committee.n(), sorted.iter().copied())
                .unwrap_or_else(|| Bitmap::new(self.committee.n())),
            agg_sig: self.crypto.aggregate(&sigs),
            attestations,
        }
    }

    /// A signed timeout vote carrying `high_pqc`.
    pub fn timeout(
        &self,
        signer: ValidatorIndex,
        instance: &Hash32,
        height: u64,
        view: u64,
        high_pqc: Option<Qc>,
    ) -> TimeoutVote {
        let hq = high_pqc.as_ref().map(|qc| qc.view);
        let msg =
            preimage::tmo_preimage(instance, &crate::testing::TEST_EPOCH.id, height, view, hq);
        TimeoutVote {
            epoch: crate::testing::TEST_EPOCH.id,
            instance: *instance,
            height,
            view,
            high_pqc,
            signer,
            sig: self.signer(signer).sign(&msg),
        }
    }

    /// A TC from exactly `entries` (signer, carried `PrepareQC`), any number: entries sorted by
    /// index, `high_pqc` = the `PrepareQC` of the maximal `hq` (lowest index among equals).
    pub fn tc(
        &self,
        instance: &Hash32,
        height: u64,
        view: u64,
        entries: &[(ValidatorIndex, Option<Qc>)],
    ) -> TimeoutCert {
        let mut timeouts: Vec<TimeoutVote> = entries
            .iter()
            .map(|(signer, qc)| self.timeout(*signer, instance, height, view, qc.clone()))
            .collect();
        timeouts.sort_by_key(|t| t.signer);
        let high_pqc = timeouts
            .iter()
            .filter(|t| t.high_pqc.is_some())
            .max_by(|a, b| a.hq().cmp(&b.hq()).then(b.signer.cmp(&a.signer)))
            .and_then(|t| t.high_pqc.clone());
        let sigs: Vec<Signature> = timeouts.iter().map(|t| t.sig).collect();
        TimeoutCert {
            epoch: crate::testing::TEST_EPOCH.id,
            instance: *instance,
            height,
            view,
            entries: timeouts
                .iter()
                .map(|t| TcEntry {
                    signer: t.signer,
                    hq: t.hq(),
                })
                .collect(),
            agg_sig: self.crypto.aggregate(&sigs),
            high_pqc,
        }
    }

    /// A proposal signed by `leader` over `prop_preimage(height, view, bh, ad)`.
    pub fn proposal(
        &self,
        leader: ValidatorIndex,
        instance: &Hash32,
        height: u64,
        view: u64,
        header: BlockHeader,
        justify: Option<TimeoutCert>,
        parent_qc: Option<Qc>,
        payload: Option<Vec<u8>>,
    ) -> Proposal {
        let bh = preimage::block_hash(&self.crypto, &header);
        let ad = preimage::att_digest(&self.crypto, justify.as_ref(), parent_qc.as_ref());
        let msg = preimage::prop_preimage(
            instance,
            &crate::testing::TEST_EPOCH.id,
            height,
            view,
            &bh,
            &ad,
        );
        Proposal {
            instance: *instance,
            height,
            view,
            header,
            justify,
            parent_qc,
            payload,
            sig: self.signer(leader).sign(&msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        use core::fmt::Write as _;
        bytes.iter().fold(String::new(), |mut out, b| {
            let _ = write!(out, "{b:02x}");
            out
        })
    }

    #[test]
    fn sha256_vectors() {
        assert_eq!(
            hex(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hex(&sha256(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        let million = vec![b'a'; 1_000_000];
        assert_eq!(
            hex(&sha256(&million)),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
        // Block-boundary lengths.
        for len in [55usize, 56, 63, 64, 65, 119, 120] {
            assert_eq!(sha256(&vec![0x61; len]).len(), 32);
        }
    }

    #[test]
    fn fake_signatures_and_aggregates() {
        let crypto = FakeCrypto::new();
        let a = FakeSigner::from_seed(b"a", None);
        let b = FakeSigner::from_seed(b"b", None);
        assert_eq!(a.public_key().as_bytes().len(), 32);
        let sig_a = a.sign(b"m");
        let sig_b = b.sign(b"m");
        assert!(crypto.verify(a.public_key(), b"m", &sig_a));
        assert!(!crypto.verify(b.public_key(), b"m", &sig_a));
        assert!(!crypto.verify(a.public_key(), b"n", &sig_a));
        let agg = crypto.aggregate(&[sig_a, sig_b]);
        assert_eq!(agg, crypto.aggregate(&[sig_b, sig_a]), "order independent");
        assert!(crypto.verify_aggregate(&[a.public_key(), b.public_key()], b"m", &agg));
        assert!(!crypto.verify_aggregate(&[a.public_key()], b"m", &agg));
        assert!(!crypto.verify_aggregate(&[], b"m", &crypto.aggregate(&[])));
        let sig_c = b.sign(b"other");
        let multi = crypto.aggregate(&[sig_a, sig_c]);
        assert!(crypto.verify_aggregate_multi(
            &[
                (vec![a.public_key()], b"m".to_vec()),
                (vec![b.public_key()], b"other".to_vec())
            ],
            &multi
        ));
        assert!(!crypto.verify_aggregate_multi(
            &[
                (vec![a.public_key()], b"m".to_vec()),
                (vec![b.public_key()], b"m".to_vec())
            ],
            &multi
        ));
        assert!(!crypto.verify_aggregate_multi(&[], &multi));
        assert!(!crypto.verify_aggregate_multi(&[(vec![], b"m".to_vec())], &multi));
        // Carries propagate within a limb.
        let mut acc = [0xffu8; SIGNATURE_LEN];
        let mut one = [0u8; SIGNATURE_LEN];
        one[31] = 1;
        add_into(&mut acc, &one);
        assert!(acc[..32].iter().all(|b| *b == 0));
        assert!(acc[32..].iter().all(|b| *b == 0xff));
    }

    #[test]
    fn provenance_log() {
        let log = SignLog::new();
        assert!(log.is_empty());
        let signer = FakeSigner::from_seed(b"x", Some(log.clone()));
        let crypto = FakeCrypto::with_log(log.clone());
        assert!(crypto.log().is_some());
        let sig = signer.sign(b"hello");
        assert_eq!(log.len(), 1);
        assert!(log.was_signed(signer.public_key(), b"hello"));
        assert!(!log.was_signed(signer.public_key(), b"bye"));
        // A forged-but-verifying signature (computed without the signer) is not in the log.
        let forged = fake_sig(signer.public_key(), b"bye");
        assert!(crypto.verify(signer.public_key(), b"bye", &forged));
        assert!(!log.was_signed(signer.public_key(), b"bye"));
        let agg = crypto.aggregate(&[sig]);
        assert_eq!(log.aggregate_parts(&agg), Some(vec![sig]));
        assert_eq!(log.signatures_by(signer.public_key()).len(), 1);
        assert_eq!(log.signatures()[0].preimage, b"hello".to_vec());
        let other = FakeSigner::with_key(PublicKey::new(vec![1; 32]).unwrap(), None);
        other.sign(b"unlogged");
        assert_eq!(log.len(), 1);
    }

    #[test]
    fn harness_certificates_and_provenance() {
        let log = SignLog::new();
        let v = FakeValidators::new(4, 7, Some(log.clone()));
        let i = Hash32([1; 32]);
        for index in 0..4 {
            assert_eq!(v.committee.index_of(&v.key(index)), Some(index));
        }
        assert_eq!(v.signers().len(), 4);
        let qc = v.qc(
            VoteKind::Prepare,
            &i,
            3,
            1,
            &Hash32([2; 32]),
            &Hash32([3; 32]),
            &[2, 0, 2],
        );
        assert_eq!(qc.signers.ones().collect::<Vec<_>>(), vec![0, 2]);
        assert!(log.qc_provenance_ok(&v.committee, &qc));
        // A certificate whose aggregate was computed without the signers fails provenance.
        let unlogged = FakeValidators::new(4, 7, None);
        let forged = unlogged.qc(
            VoteKind::Commit,
            &i,
            3,
            1,
            &Hash32([2; 32]),
            &Hash32([3; 32]),
            &[1, 3],
        );
        assert!(!log.qc_provenance_ok(&v.committee, &forged));
        let tc = v.tc(&i, 3, 2, &[(3, Some(qc.clone())), (1, None)]);
        assert_eq!(tc.entries[0].signer, 1);
        assert_eq!(tc.high_pqc, Some(qc));
        assert!(log.tc_provenance_ok(&v.committee, &tc));
        let forged_tc = unlogged.tc(&i, 3, 2, &[(0, None)]);
        assert!(!log.tc_provenance_ok(&v.committee, &forged_tc));
        let bad_index = TimeoutCert {
            entries: vec![TcEntry {
                signer: 9,
                hq: None,
            }],
            ..forged_tc
        };
        assert!(!log.tc_provenance_ok(&v.committee, &bad_index));
    }
}
