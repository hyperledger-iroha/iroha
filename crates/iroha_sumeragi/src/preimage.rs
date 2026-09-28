//! Signing and hash preimages (spec §2.1, §3.1–§3.3, §1.8): fixed byte layouts built by hand,
//! independent of any codec. Every layout starts with a domain tag; instance, scheduling epoch/context, height and view are
//! in every consensus signing preimage, block hash, result and the block's attestation flag in
//! every vote. The probe echo (kind `0x05`, §7.4 R2) is the only other signed object; the commit
//! statement (kind `0x06`, §3.7) is attested by the application, not signed by consensus keys.

use crate::{
    crypto::Crypto,
    message::{Block, BlockHeader, Qc, TimeoutCert, VoteKind},
    types::{Committee, EpochConfig, EpochId, Hash32, PublicKey},
};

/// Domain tag of every signing preimage.
pub const TAG_SIG: &[u8] = b"sumeragi/sig";
/// Domain tag of `block_hash`.
pub const TAG_BLOCK: &[u8] = b"sumeragi/block";
/// Domain tag of `payload_hash`.
pub const TAG_PAY: &[u8] = b"sumeragi/payload";
/// Domain tag of `att_digest`.
pub const TAG_ATT: &[u8] = b"sumeragi/attach";
/// Domain tag of `qc_digest`.
pub const TAG_QC: &[u8] = b"sumeragi/qc";
/// Domain tag of `tc_digest`.
pub const TAG_TC: &[u8] = b"sumeragi/tc";
/// Domain tag of `committee_digest`.
pub const TAG_COMMITTEE: &[u8] = b"sumeragi/committee";
/// Domain tag of the topology seed.
pub const TAG_TOPOLOGY: &[u8] = b"sumeragi/topology";
/// Domain tag of the (application-side) instance-id derivation (§1.8).
pub const TAG_INSTANCE: &[u8] = b"sumeragi/instance";
/// Kind byte of a proposal signature.
pub const KIND_PROPOSAL: u8 = 0x01;
/// Kind byte of a Prepare vote.
pub const KIND_PREPARE: u8 = 0x02;
/// Kind byte of a Commit vote.
pub const KIND_COMMIT: u8 = 0x03;
/// Kind byte of a timeout vote.
pub const KIND_TIMEOUT: u8 = 0x04;
/// Kind byte of a probe echo (§3.3, §7.4 R2).
pub const KIND_ECHO: u8 = 0x05;
/// Kind byte of the commit statement an application attests (§3.3, §3.7).
pub const KIND_ATTEST: u8 = 0x06;

/// `kb(pk) = be16(len(raw)) ‖ raw`.
pub fn kb(pk: &PublicKey) -> Vec<u8> {
    let mut out = Vec::with_capacity(pk.as_bytes().len() + 2);
    put_kb(&mut out, pk);
    out
}

fn put_kb(out: &mut Vec<u8>, pk: &PublicKey) {
    let raw = pk.as_bytes();
    // Keys longer than `MAX_PUBLIC_KEY_LEN` (≪ 65 535) are rejected at every intake.
    let len = u16::try_from(raw.len()).unwrap_or(u16::MAX);
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(raw);
}

/// `keys(l) = be32(len(l)) ‖ kb(l[0]) ‖ … ‖ kb(l[len(l)−1])`.
pub fn keys(list: &[PublicKey]) -> Vec<u8> {
    let mut out = Vec::new();
    put_keys(&mut out, list);
    out
}

fn put_keys(out: &mut Vec<u8>, list: &[PublicKey]) {
    put_len32(out, list.len());
    for pk in list {
        put_kb(out, pk);
    }
}

fn put_len32(out: &mut Vec<u8>, len: usize) {
    // Every list reaching a preimage is bounded far below 2^32 by the intake limits.
    out.extend_from_slice(&u32::try_from(len).unwrap_or(u32::MAX).to_be_bytes());
}

/// `bit(b) = 0x01 if b else 0x00` (§3.1).
pub fn bit(flag: bool) -> u8 {
    u8::from(flag)
}

/// `blobs(l) = be32(len(l)) ‖ [be32(len(x)) ‖ x] for x in l` (§3.1).
pub fn put_blobs(out: &mut Vec<u8>, list: &[Vec<u8>]) {
    put_len32(out, list.len());
    for blob in list {
        put_len32(out, blob.len());
        out.extend_from_slice(blob);
    }
}

/// `enc(None) = 0x00 ; enc(Some(w)) = 0x01 ‖ be64(w)` (optional view).
pub fn enc_view(out: &mut Vec<u8>, view: Option<u64>) {
    match view {
        None => out.push(0x00),
        Some(w) => {
            out.push(0x01);
            out.extend_from_slice(&w.to_be_bytes());
        }
    }
}

/// `opt(None) = 0x00 ; opt(Some(x)) = 0x01 ‖ x` (optional digest).
pub fn opt_digest(out: &mut Vec<u8>, digest: Option<&Hash32>) {
    match digest {
        None => out.push(0x00),
        Some(d) => {
            out.push(0x01);
            out.extend_from_slice(d.as_bytes());
        }
    }
}

fn put_epoch(out: &mut Vec<u8>, epoch: &EpochId) {
    // MS43: signatures cease binding the scheduling epoch and its complete context.
    if !cfg!(sumeragi_mutation = "MS43") {
        out.extend_from_slice(&epoch.epoch.to_be_bytes());
        out.extend_from_slice(epoch.context.as_bytes());
    }
}

fn put_round(out: &mut Vec<u8>, instance: &Hash32, epoch: &EpochId, height: u64, view: u64) {
    out.extend_from_slice(instance.as_bytes());
    put_epoch(out, epoch);
    out.extend_from_slice(&height.to_be_bytes());
    out.extend_from_slice(&view.to_be_bytes());
}

/// Preimage of `block_hash` (§3.2):
/// `TAG_BLOCK ‖ I ‖ E ‖ be64(h) ‖ be64(origin_view) ‖ parent_hash ‖ parent_result ‖ payload_hash ‖
/// be32(payload_len) ‖ be32(proposer) ‖ keys(skipped_leaders) ‖ be32(control_len) ‖ control ‖ bit(attest)`.
pub fn block_hash_preimage(header: &BlockHeader) -> Vec<u8> {
    let mut out = Vec::with_capacity(200);
    out.extend_from_slice(TAG_BLOCK);
    put_round(
        &mut out,
        &header.instance,
        &header.epoch,
        header.height,
        header.origin_view,
    );
    out.extend_from_slice(header.parent_hash.as_bytes());
    out.extend_from_slice(header.parent_result.as_bytes());
    out.extend_from_slice(header.payload_hash.as_bytes());
    out.extend_from_slice(&header.payload_len.to_be_bytes());
    out.extend_from_slice(&header.proposer.to_be_bytes());
    put_keys(&mut out, &header.skipped_leaders);
    // MS46: control bytes disappear from every signature rooted in this header hash.
    if !cfg!(sumeragi_mutation = "MS46") {
        out.extend_from_slice(&(header.control_witness.len() as u32).to_be_bytes());
        out.extend_from_slice(header.control_witness.as_slice());
    }
    out.push(bit(header.attest));
    out
}

/// `block_hash(header)` (§3.2).
pub fn block_hash(crypto: &dyn Crypto, header: &BlockHeader) -> Hash32 {
    crypto.hash(&block_hash_preimage(header))
}

/// `payload_hash = H(TAG_PAY ‖ payload)` (§3.2).
pub fn payload_hash(crypto: &dyn Crypto, payload: &[u8]) -> Hash32 {
    let mut out = Vec::with_capacity(TAG_PAY.len() + payload.len());
    out.extend_from_slice(TAG_PAY);
    out.extend_from_slice(payload);
    crypto.hash(&out)
}

/// `body_ok(b) := 0 < len(b.payload) == b.header.payload_len ∧ H(TAG_PAY ‖ b.payload) ==
/// b.header.payload_hash` (§3.2).
pub fn body_ok(crypto: &dyn Crypto, block: &Block) -> bool {
    cfg!(sumeragi_mutation = "MS20")
        || (!block.payload.is_empty()
            && u32::try_from(block.payload.len()).is_ok_and(|len| len == block.header.payload_len)
            && payload_hash(crypto, &block.payload) == block.header.payload_hash)
}

/// `prop_preimage(h, v, bh, ad) = TAG_SIG ‖ 0x01 ‖ I ‖ E ‖ be64(h) ‖ be64(v) ‖ bh ‖ ad` (§3.3).
pub fn prop_preimage(
    instance: &Hash32,
    epoch: &EpochId,
    height: u64,
    view: u64,
    bh: &Hash32,
    ad: &Hash32,
) -> Vec<u8> {
    #[cfg(sumeragi_mutation = "MS16")]
    let instance = &Hash32::ZERO;
    let mut out = Vec::with_capacity(TAG_SIG.len() + 1 + 32 + 16 + 64);
    out.extend_from_slice(TAG_SIG);
    out.push(KIND_PROPOSAL);
    put_round(&mut out, instance, epoch, height, view);
    out.extend_from_slice(bh.as_bytes());
    out.extend_from_slice(ad.as_bytes());
    out
}

/// `vote_preimage(kind, h, v, bh, R, a) = TAG_SIG ‖ kind ‖ I ‖ E ‖ be64(h) ‖ be64(v) ‖ bh ‖ R ‖
/// bit(a)` (§3.3), with `a` the block's attestation flag (§3.7).
pub fn vote_preimage(
    kind: VoteKind,
    instance: &Hash32,
    epoch: &EpochId,
    height: u64,
    view: u64,
    bh: &Hash32,
    result: &Hash32,
    attest: bool,
) -> Vec<u8> {
    #[cfg(sumeragi_mutation = "MS16")]
    let instance = &Hash32::ZERO;
    let mut out = Vec::with_capacity(TAG_SIG.len() + 1 + 32 + 16 + 65);
    out.extend_from_slice(TAG_SIG);
    out.push(kind.byte());
    put_round(&mut out, instance, epoch, height, view);
    out.extend_from_slice(bh.as_bytes());
    #[cfg(not(sumeragi_mutation = "MS17"))]
    out.extend_from_slice(result.as_bytes());
    // MA6: the flag is not signed.
    #[cfg(not(sumeragi_mutation = "MA6"))]
    out.push(bit(attest));
    #[cfg(sumeragi_mutation = "MA6")]
    let _ = attest;
    out
}

/// `att_preimage(h, bh, R) = TAG_SIG ‖ 0x06 ‖ I ‖ E ‖ be64(h) ‖ bh ‖ R` (§3.3): the commit statement
/// an application attests (§3.7). It binds the instance, scheduling epoch/context, height, block hash and result, not the
/// view, so an attestation stays valid in every view of its block.
pub fn att_preimage(
    instance: &Hash32,
    epoch: &EpochId,
    height: u64,
    bh: &Hash32,
    result: &Hash32,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(TAG_SIG.len() + 1 + 32 + 8 + 64);
    out.extend_from_slice(TAG_SIG);
    out.push(KIND_ATTEST);
    out.extend_from_slice(instance.as_bytes());
    put_epoch(&mut out, epoch);
    #[cfg(not(sumeragi_mutation = "MA4"))]
    out.extend_from_slice(&height.to_be_bytes());
    #[cfg(sumeragi_mutation = "MA4")]
    let _ = height;
    out.extend_from_slice(bh.as_bytes());
    #[cfg(not(sumeragi_mutation = "MA3"))]
    out.extend_from_slice(result.as_bytes());
    #[cfg(sumeragi_mutation = "MA3")]
    let _ = result;
    out
}

/// Exact parsed source of an application commit statement. The parser accepts only the
/// fixed signing layout emitted by [`att_preimage`], without allocation or trailing bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttestationStatement {
    /// Network and lane instance.
    pub instance: Hash32,
    /// Scheduling epoch and complete authenticated context identity.
    pub epoch: EpochId,
    /// Exact block height.
    pub height: u64,
    /// Hash of the complete signed native header.
    pub block_hash: Hash32,
    /// Exact application execution result.
    pub result: Hash32,
}
impl AttestationStatement {
    /// Parse the sole fixed signing format, rejecting alternate domains, truncation and suffixes.
    #[must_use]
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        const TAIL: usize = 32 + 8 + 32 + 8 + 32 + 32;
        let mut rest = bytes.strip_prefix(TAG_SIG)?.strip_prefix(&[KIND_ATTEST])?;
        if rest.len() != TAIL {
            return None;
        }
        fn take<const N: usize>(rest: &mut &[u8]) -> Option<[u8; N]> {
            let (value, tail) = rest.split_at_checked(N)?;
            *rest = tail;
            value.try_into().ok()
        }
        Some(Self {
            instance: Hash32(take(&mut rest)?),
            epoch: EpochId {
                epoch: u64::from_be_bytes(take(&mut rest)?),
                context: Hash32(take(&mut rest)?),
            },
            height: u64::from_be_bytes(take(&mut rest)?),
            block_hash: Hash32(take(&mut rest)?),
            result: Hash32(take(&mut rest)?),
        })
    }
}

/// `tmo_preimage(h, v, hq) = TAG_SIG ‖ 0x04 ‖ I ‖ E ‖ be64(h) ‖ be64(v) ‖ enc(hq)` (§3.3).
pub fn tmo_preimage(
    instance: &Hash32,
    epoch: &EpochId,
    height: u64,
    view: u64,
    hq: Option<u64>,
) -> Vec<u8> {
    #[cfg(sumeragi_mutation = "MS16")]
    let instance = &Hash32::ZERO;
    let mut out = Vec::with_capacity(TAG_SIG.len() + 1 + 32 + 16 + 9);
    out.extend_from_slice(TAG_SIG);
    out.push(KIND_TIMEOUT);
    put_round(&mut out, instance, epoch, height, view);
    enc_view(&mut out, hq);
    out
}

/// `echo_preimage(nonce, height) = TAG_SIG ‖ 0x05 ‖ I ‖ E ‖ be64(nonce) ‖ be64(height)` (§3.3): the
/// signed answer to a probe (§7.4 R2). It binds the prober's nonce and the replier's reported
/// height; its kind byte keeps it from verifying as a proposal, vote or timeout.
pub fn echo_preimage(instance: &Hash32, epoch: &EpochId, nonce: u64, height: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(TAG_SIG.len() + 1 + 32 + 16);
    out.extend_from_slice(TAG_SIG);
    out.push(KIND_ECHO);
    out.extend_from_slice(instance.as_bytes());
    put_epoch(&mut out, epoch);
    out.extend_from_slice(&nonce.to_be_bytes());
    out.extend_from_slice(&height.to_be_bytes());
    out
}

/// Preimage of `qc_digest(c)` (§3.3): `TAG_QC ‖ c.kind ‖ I ‖ E ‖ be64(c.height) ‖ be64(c.view) ‖
/// c.block_hash ‖ c.result ‖ be32(len(c.signers)) ‖ c.signers ‖ c.agg_sig ‖ bit(c.attest) ‖
/// blobs(c.attestations)`.
// SPEC: `I` in qc_digest / tc_digest is taken from the certificate's own `instance` field (every
// verified certificate has `instance == I`) (Appendix E, E13).
pub fn qc_digest_preimage(qc: &Qc) -> Vec<u8> {
    let mut out = Vec::with_capacity(TAG_QC.len() + 1 + 32 + 16 + 64 + 4 + 8 + 96);
    out.extend_from_slice(TAG_QC);
    out.push(qc.kind.byte());
    put_round(&mut out, &qc.instance, &qc.epoch, qc.height, qc.view);
    out.extend_from_slice(qc.block_hash.as_bytes());
    out.extend_from_slice(qc.result.as_bytes());
    put_len32(&mut out, qc.signers.as_bytes().len());
    out.extend_from_slice(qc.signers.as_bytes());
    out.extend_from_slice(&qc.agg_sig.0);
    out.push(bit(qc.attest));
    match &qc.attestation_witness {
        Some(witness) => {
            out.push(1);
            put_len32(&mut out, witness.as_slice().len());
            out.extend_from_slice(witness.as_slice());
        }
        None => out.push(0),
    }
    put_len32(&mut out, qc.attestations.len());
    for signature in &qc.attestations {
        put_len32(&mut out, signature.len());
        out.extend_from_slice(signature.as_slice());
    }
    out
}

/// `qc_digest(c)` (§3.3).
pub fn qc_digest(crypto: &dyn Crypto, qc: &Qc) -> Hash32 {
    crypto.hash(&qc_digest_preimage(qc))
}

/// Preimage of `tc_digest(t)` (§3.3): `TAG_TC ‖ I ‖ E ‖ be64(t.height) ‖ be64(t.view) ‖
/// be32(len(t.entries)) ‖ [be32(idx) ‖ enc(hq)]… ‖ t.agg_sig ‖ opt(t.high_pqc.map(qc_digest))`.
pub fn tc_digest_preimage(crypto: &dyn Crypto, tc: &TimeoutCert) -> Vec<u8> {
    let mut out = Vec::with_capacity(TAG_TC.len() + 32 + 16 + 4 + tc.entries.len() * 13 + 96 + 33);
    out.extend_from_slice(TAG_TC);
    put_round(&mut out, &tc.instance, &tc.epoch, tc.height, tc.view);
    put_len32(&mut out, tc.entries.len());
    for entry in &tc.entries {
        out.extend_from_slice(&entry.signer.to_be_bytes());
        enc_view(&mut out, entry.hq);
    }
    out.extend_from_slice(&tc.agg_sig.0);
    let high = tc.high_pqc.as_ref().map(|qc| qc_digest(crypto, qc));
    opt_digest(&mut out, high.as_ref());
    out
}

/// `tc_digest(t)` (§3.3).
pub fn tc_digest(crypto: &dyn Crypto, tc: &TimeoutCert) -> Hash32 {
    crypto.hash(&tc_digest_preimage(crypto, tc))
}

/// Preimage of `att_digest(justify, parent_qc)`:
/// `TAG_ATT ‖ opt(justify.map(tc_digest)) ‖ opt(parent_qc.map(qc_digest))` (§3.3).
pub fn att_digest_preimage(
    crypto: &dyn Crypto,
    justify: Option<&TimeoutCert>,
    parent_qc: Option<&Qc>,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(TAG_ATT.len() + 66);
    out.extend_from_slice(TAG_ATT);
    let tc = justify.map(|tc| tc_digest(crypto, tc));
    opt_digest(&mut out, tc.as_ref());
    let qc = parent_qc.map(|qc| qc_digest(crypto, qc));
    opt_digest(&mut out, qc.as_ref());
    out
}

/// `ad = att_digest(justify, parent_qc)` (§3.3).
pub fn att_digest(
    crypto: &dyn Crypto,
    justify: Option<&TimeoutCert>,
    parent_qc: Option<&Qc>,
) -> Hash32 {
    crypto.hash(&att_digest_preimage(crypto, justify, parent_qc))
}

/// Preimage of `committee_digest(C) = H(TAG_COMMITTEE ‖ be32(n) ‖ kb(C[0]) ‖ … ‖ kb(C[n−1]))`.
pub fn committee_digest_preimage(committee: &Committee) -> Vec<u8> {
    let mut out = Vec::with_capacity(TAG_COMMITTEE.len() + 4 + committee.n() * 50);
    out.extend_from_slice(TAG_COMMITTEE);
    put_keys(&mut out, committee.members());
    out
}

/// `committee_digest(C)` (§2.1, exported for light clients, §11).
pub fn committee_digest(crypto: &dyn Crypto, committee: &Committee) -> Hash32 {
    crypto.hash(&committee_digest_preimage(committee))
}

/// `seed_C = H(TAG_TOPOLOGY ‖ I ‖ E ‖ leader_seed ‖ committee_digest(C))` (§2.1).
pub fn topology_seed(
    crypto: &dyn Crypto,
    instance: &Hash32,
    epoch: &EpochConfig,
    committee: &Committee,
) -> Hash32 {
    let mut out = Vec::with_capacity(TAG_TOPOLOGY.len() + 64);
    out.extend_from_slice(TAG_TOPOLOGY);
    out.extend_from_slice(instance.as_bytes());
    put_epoch(&mut out, &epoch.id);
    out.extend_from_slice(epoch.leader_seed.as_bytes());
    out.extend_from_slice(committee_digest(crypto, committee).as_bytes());
    crypto.hash(&out)
}

/// Kind of a consensus instance (§1.8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstanceKind {
    /// The global (Nexus) chain.
    Global,
    /// A dataspace chain.
    Dataspace,
    /// A lane chain.
    Lane,
}

impl InstanceKind {
    /// Kind byte: 0 = global, 1 = dataspace, 2 = lane.
    pub const fn byte(self) -> u8 {
        match self {
            Self::Global => 0,
            Self::Dataspace => 1,
            Self::Lane => 2,
        }
    }
}

/// Application-side instance-id derivation (§1.8):
/// `I = H("sumeragi/instance" ‖ genesis_hash ‖ chain_id_bytes ‖ kind:u8 ‖ be32(index))`.
/// The core itself only ever sees the resulting 32 opaque bytes.
pub fn instance_id(
    crypto: &dyn Crypto,
    genesis_hash: &Hash32,
    chain_id: &[u8],
    kind: InstanceKind,
    index: u32,
) -> Hash32 {
    let mut out = Vec::with_capacity(TAG_INSTANCE.len() + 32 + chain_id.len() + 5);
    out.extend_from_slice(TAG_INSTANCE);
    out.extend_from_slice(genesis_hash.as_bytes());
    out.extend_from_slice(chain_id);
    out.push(kind.byte());
    out.extend_from_slice(&index.to_be_bytes());
    crypto.hash(&out)
}

#[cfg(test)]
mod tests {
    //! Golden vectors. Expected bytes and digests were produced by an independent Python
    //! implementation of the §3 layouts (SHA-256 as `H`, which is also the fake scheme's hash).
    use super::*;
    use crate::{
        message::TcEntry,
        testing::FakeCrypto,
        types::{AggregateSignature, Bitmap, SIGNATURE_LEN},
    };

    fn hex(bytes: &[u8]) -> String {
        use core::fmt::Write as _;
        bytes.iter().fold(String::new(), |mut out, b| {
            let _ = write!(out, "{b:02x}");
            out
        })
    }

    fn h(byte: u8) -> Hash32 {
        Hash32([byte; 32])
    }

    fn key(byte: u8, len: usize) -> PublicKey {
        PublicKey::new(vec![byte; len]).unwrap()
    }

    fn golden_qc() -> Qc {
        Qc {
            attestation_witness: None,
            epoch: crate::testing::TEST_EPOCH.id,
            kind: VoteKind::Prepare,
            instance: h(0x11),
            height: 7,
            view: 1,
            block_hash: h(0x22),
            result: h(0x33),
            signers: Bitmap::from_indices(4, [0, 1, 3]).unwrap(),
            agg_sig: AggregateSignature([0x55; SIGNATURE_LEN]),
            attest: false,
            attestations: Vec::new(),
        }
    }

    fn golden_tc() -> TimeoutCert {
        TimeoutCert {
            epoch: crate::testing::TEST_EPOCH.id,
            instance: h(0x11),
            height: 7,
            view: 2,
            entries: vec![
                TcEntry {
                    signer: 0,
                    hq: None,
                },
                TcEntry {
                    signer: 1,
                    hq: Some(1),
                },
                TcEntry {
                    signer: 3,
                    hq: Some(1),
                },
            ],
            agg_sig: AggregateSignature([0x66; SIGNATURE_LEN]),
            high_pqc: Some(golden_qc()),
        }
    }

    fn golden_header() -> BlockHeader {
        BlockHeader {
            control_witness: crate::types::ControlWitness::empty(),
            epoch: crate::testing::TEST_EPOCH.id,
            instance: h(0x11),
            height: 7,
            origin_view: 2,
            parent_hash: h(0x44),
            parent_result: h(0x45),
            payload_hash: h(0x46),
            payload_len: 3,
            proposer: 2,
            skipped_leaders: vec![key(0xa1, 32), key(0xa2, 48)],
            attest: false,
        }
    }

    #[test]
    fn tag_constants() {
        assert_eq!(TAG_SIG, b"sumeragi/sig");
        assert_eq!(TAG_BLOCK, b"sumeragi/block");
        assert_eq!(TAG_PAY, b"sumeragi/payload");
        assert_eq!(TAG_ATT, b"sumeragi/attach");
        assert_eq!(TAG_QC, b"sumeragi/qc");
        assert_eq!(TAG_TC, b"sumeragi/tc");
        assert_eq!(TAG_COMMITTEE, b"sumeragi/committee");
        assert_eq!(TAG_TOPOLOGY, b"sumeragi/topology");
        assert_eq!(
            [
                KIND_PROPOSAL,
                KIND_PREPARE,
                KIND_COMMIT,
                KIND_TIMEOUT,
                KIND_ECHO,
                KIND_ATTEST
            ],
            [1, 2, 3, 4, 5, 6]
        );
    }

    #[test]
    fn primitive_encodings() {
        assert_eq!(hex(&kb(&key(0xab, 3))), "0003ababab");
        assert_eq!(
            hex(&keys(&[key(1, 1), key(2, 2)])),
            "0000000200010100020202"
        );
        assert_eq!(hex(&keys(&[])), "00000000");
        let mut out = vec![];
        enc_view(&mut out, None);
        enc_view(&mut out, Some(0x0102));
        assert_eq!(hex(&out), "00010000000000000102");
        let mut out = vec![];
        opt_digest(&mut out, None);
        opt_digest(&mut out, Some(&h(0xee)));
        assert_eq!(out.len(), 1 + 1 + 32);
        assert_eq!(out[..2], [0, 1]);
        assert!(out[2..].iter().all(|b| *b == 0xee));
        assert_eq!((bit(false), bit(true)), (0, 1));
        let mut out = vec![];
        put_blobs(&mut out, &[vec![0xaa; 3], vec![]]);
        assert_eq!(hex(&out), "0000000200000003aaaaaa00000000");
    }

    #[test]
    fn golden_signing_preimages() {
        let prop = prop_preimage(
            &h(0x11),
            &crate::testing::TEST_EPOCH.id,
            7,
            2,
            &h(0x22),
            &h(0x44),
        );
        assert_eq!(
            hex(&prop),
            format!(
                "{}01{}{:016x}{:016x}{}{}",
                hex(TAG_SIG),
                format!("{}0000000000000000{}", "11".repeat(32), "e0".repeat(32)),
                7,
                2,
                "22".repeat(32),
                "44".repeat(32)
            )
        );
        let prepare = vote_preimage(
            VoteKind::Prepare,
            &h(0x11),
            &crate::testing::TEST_EPOCH.id,
            7,
            2,
            &h(0x22),
            &h(0x33),
            false,
        );
        let commit = vote_preimage(
            VoteKind::Commit,
            &h(0x11),
            &crate::testing::TEST_EPOCH.id,
            7,
            2,
            &h(0x22),
            &h(0x33),
            false,
        );
        let flagged = vote_preimage(
            VoteKind::Commit,
            &h(0x11),
            &crate::testing::TEST_EPOCH.id,
            7,
            2,
            &h(0x22),
            &h(0x33),
            true,
        );
        let expected_tail = format!(
            "{}{:016x}{:016x}{}{}",
            format!("{}0000000000000000{}", "11".repeat(32), "e0".repeat(32)),
            7,
            2,
            "22".repeat(32),
            "33".repeat(32)
        );
        assert_eq!(
            hex(&prepare),
            format!("{}02{expected_tail}00", hex(TAG_SIG))
        );
        assert_eq!(hex(&commit), format!("{}03{expected_tail}00", hex(TAG_SIG)));
        assert_eq!(
            hex(&flagged),
            format!("{}03{expected_tail}01", hex(TAG_SIG))
        );
        assert_eq!(
            hex(&tmo_preimage(
                &h(0x11),
                &crate::testing::TEST_EPOCH.id,
                7,
                2,
                None
            )),
            format!(
                "{}04{}{:016x}{:016x}00",
                hex(TAG_SIG),
                format!("{}0000000000000000{}", "11".repeat(32), "e0".repeat(32)),
                7,
                2
            )
        );
        assert_eq!(
            hex(&tmo_preimage(
                &h(0x11),
                &crate::testing::TEST_EPOCH.id,
                7,
                2,
                Some(1)
            )),
            format!(
                "{}04{}{:016x}{:016x}01{:016x}",
                hex(TAG_SIG),
                format!("{}0000000000000000{}", "11".repeat(32), "e0".repeat(32)),
                7,
                2,
                1
            )
        );
        // SHA-256 digests of the preimages (independent Python implementation).
        let crypto = FakeCrypto::new();
        assert_eq!(
            crypto.hash(&prop).to_string(),
            "74387e4970874078d8b5999dca1c03dd03e8c4f24a6fe37e7b5eca6a28da8713"
        );
        assert_eq!(
            crypto.hash(&prepare).to_string(),
            "dc21a6f4d17467ab1c009ecf93cd074ed3857c3d22980d40a80e145c6d206c27"
        );
        assert_eq!(
            crypto.hash(&commit).to_string(),
            "7c955480b0110f9eaa247962f7ceee9dd3e0b49c58685734bdd9aa02c7c132f8"
        );
        assert_eq!(
            crypto.hash(&flagged).to_string(),
            "81933fd9209a684f2a70dd1f419e041a0269260a443c54d6fc861094d6ddbc70"
        );
        assert_eq!(
            crypto
                .hash(&tmo_preimage(
                    &h(0x11),
                    &crate::testing::TEST_EPOCH.id,
                    7,
                    2,
                    Some(1)
                ))
                .to_string(),
            "62b8c5fa3f8a852a7de9f3df85e826ff5fa5fee776e0c87ae7e556f76dfb6937"
        );
    }

    #[test]
    fn golden_echo_preimage() {
        // `TAG_SIG ‖ 0x05 ‖ I ‖ E ‖ be64(nonce) ‖ be64(height)`; digest from an independent Python
        // implementation (SHA-256).
        let echo = echo_preimage(
            &h(0x11),
            &crate::testing::TEST_EPOCH.id,
            0x0102_0304_0506_0708,
            7,
        );
        assert_eq!(
            hex(&echo),
            "73756d65726167692f7369670511111111111111111111111111111111111111111111111111111111111111110000000000000000e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e001020304050607080000000000000007"
        );
        assert_eq!(
            FakeCrypto::new().hash(&echo).to_string(),
            "73d599df3c33ad5de89a43074a7eb5568dc591954bdaf1c3cf42f6587852789d"
        );
        // Never equal to a consensus preimage of the same numbers (kind byte).
        assert_ne!(
            echo[..13],
            tmo_preimage(&h(0x11), &crate::testing::TEST_EPOCH.id, 1, 7, None)[..13]
        );
        assert_ne!(
            echo_preimage(&h(0x11), &crate::testing::TEST_EPOCH.id, 1, 7),
            echo_preimage(&h(0x12), &crate::testing::TEST_EPOCH.id, 1, 7),
            "instance bound"
        );
        assert_ne!(
            echo_preimage(&h(0x11), &crate::testing::TEST_EPOCH.id, 1, 7),
            echo_preimage(&h(0x11), &crate::testing::TEST_EPOCH.id, 2, 7),
            "nonce bound"
        );
        assert_ne!(
            echo_preimage(&h(0x11), &crate::testing::TEST_EPOCH.id, 1, 7),
            echo_preimage(&h(0x11), &crate::testing::TEST_EPOCH.id, 1, 8),
            "height bound"
        );
    }

    /// MA4, MA3: the commit statement of §3.3 binds `I`, `h`, `bh` and `R` (and no view) in a
    /// fixed layout that an application verifier may parse; digest from an independent Python
    /// implementation (SHA-256).
    #[test]
    fn golden_attestation_preimage() {
        let statement = att_preimage(
            &h(0x11),
            &crate::testing::TEST_EPOCH.id,
            7,
            &h(0x22),
            &h(0x33),
        );
        assert_eq!(
            hex(&statement),
            format!(
                "{}06{}{:016x}{}{}",
                hex(TAG_SIG),
                format!("{}0000000000000000{}", "11".repeat(32), "e0".repeat(32)),
                7,
                "22".repeat(32),
                "33".repeat(32)
            )
        );
        assert_eq!(
            FakeCrypto::new().hash(&statement).to_string(),
            "faff2b12f49fd1e37e9b82e33e230c5594c763956bf7c0735872eedd28c61af1"
        );
        for other in [
            att_preimage(
                &h(0x12),
                &crate::testing::TEST_EPOCH.id,
                7,
                &h(0x22),
                &h(0x33),
            ),
            att_preimage(
                &h(0x11),
                &crate::testing::TEST_EPOCH.id,
                8,
                &h(0x22),
                &h(0x33),
            ),
            att_preimage(
                &h(0x11),
                &crate::testing::TEST_EPOCH.id,
                7,
                &h(0x23),
                &h(0x33),
            ),
            att_preimage(
                &h(0x11),
                &crate::testing::TEST_EPOCH.id,
                7,
                &h(0x22),
                &h(0x34),
            ),
        ] {
            assert_ne!(
                other, statement,
                "instance, height, block and result are bound"
            );
        }
        // Never a vote preimage (kind byte), whatever the numbers.
        assert_ne!(
            statement[..13],
            vote_preimage(
                VoteKind::Commit,
                &h(0x11),
                &crate::testing::TEST_EPOCH.id,
                7,
                2,
                &h(0x22),
                &h(0x33),
                true
            )[..13]
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn golden_block_hash_and_body() {
        let crypto = FakeCrypto::new();
        let header = golden_header();
        let preimage = block_hash_preimage(&header);
        let expected = format!(
            "{}{}{:016x}{:016x}{}{}{}{:08x}{:08x}00000002{}{}{}{}0000000000",
            hex(TAG_BLOCK),
            format!("{}0000000000000000{}", "11".repeat(32), "e0".repeat(32)),
            7,
            2,
            "44".repeat(32),
            "45".repeat(32),
            "46".repeat(32),
            3,
            2,
            "0020",
            "a1".repeat(32),
            "0030",
            "a2".repeat(48)
        );
        assert_eq!(hex(&preimage), expected);
        assert_eq!(
            block_hash(&crypto, &header).to_string(),
            "ca85008c0cd159d57365ea1ca3b2e942e7b78e41423523d47111b57c3ff3820a"
        );
        assert_eq!(
            block_hash(
                &crypto,
                &BlockHeader {
                    attest: true,
                    ..header.clone()
                }
            )
            .to_string(),
            "0c96df0264b9faa573517be2193b53d5fcaf77daca648a6aaac9dbe0b6eeddbd"
        );
        assert_eq!(
            payload_hash(&crypto, &[1, 2, 3]).to_string(),
            "a0a0dfa08688a53aaa799ab662a06c9efe6a0c41050e6b023b1a6b6cb9ff320c"
        );
        let good = BlockHeader {
            payload_hash: payload_hash(&crypto, &[1, 2, 3]),
            ..header
        };
        let block = Block {
            header: good.clone(),
            payload: vec![1, 2, 3],
        };
        assert!(body_ok(&crypto, &block));
        let tampered = Block {
            payload: vec![1, 2, 4],
            ..block.clone()
        };
        assert!(!body_ok(&crypto, &tampered));
        let truncated = Block {
            payload: vec![1, 2],
            ..block.clone()
        };
        assert!(!body_ok(&crypto, &truncated));
        let wrong_len = Block {
            header: BlockHeader {
                payload_len: 4,
                ..good
            },
            payload: vec![1, 2, 3],
        };
        assert!(!body_ok(&crypto, &wrong_len));
        // Every header field is bound by the hash.
        let base = block_hash(&crypto, &golden_header());
        let variants = [
            BlockHeader {
                instance: h(0x12),
                ..golden_header()
            },
            BlockHeader {
                height: 8,
                ..golden_header()
            },
            BlockHeader {
                origin_view: 3,
                ..golden_header()
            },
            BlockHeader {
                parent_hash: h(0),
                ..golden_header()
            },
            BlockHeader {
                parent_result: h(0),
                ..golden_header()
            },
            BlockHeader {
                payload_hash: h(0),
                ..golden_header()
            },
            BlockHeader {
                payload_len: 0,
                ..golden_header()
            },
            BlockHeader {
                proposer: 3,
                ..golden_header()
            },
            BlockHeader {
                skipped_leaders: vec![key(0xa1, 32)],
                ..golden_header()
            },
            BlockHeader {
                attest: true,
                ..golden_header()
            },
        ];
        for variant in variants {
            assert_ne!(block_hash(&crypto, &variant), base, "{variant:?}");
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn golden_certificate_digests() {
        let crypto = FakeCrypto::new();
        let qc = golden_qc();
        let qc_pre = qc_digest_preimage(&qc);
        assert_eq!(hex(&qc_pre), GOLDEN_QC_PREIMAGE);
        assert_eq!(
            qc_digest(&crypto, &qc).to_string(),
            "9f167dc4fae6a2f5754af5ec6a2fa40391f31a15006e3d0ca7a993a7b9134def"
        );
        let flagged = Qc {
            kind: VoteKind::Commit,
            attest: true,
            attestations: [&[0xaa; 3][..], &[0xbb; 2][..], &[][..]]
                .map(|b| crate::message::AttestationSignature::try_from_slice(b).unwrap())
                .to_vec(),
            ..golden_qc()
        };
        assert_eq!(
            qc_digest(&crypto, &flagged).to_string(),
            "a23c431cc30a432dbe17959ec135d8348a46b6f168ff244ee57c6b8d76dc89fa"
        );

        let tc = golden_tc();
        let tc_pre = tc_digest_preimage(&crypto, &tc);
        assert_eq!(hex(&tc_pre), GOLDEN_TC_PREIMAGE);
        assert_eq!(
            tc_digest(&crypto, &tc).to_string(),
            "3ead8d746b846639424db413cdef71dad86a3ec636ba4040d17d5590a63146f1"
        );
        let tc_none = TimeoutCert {
            high_pqc: None,
            ..golden_tc()
        };
        assert_eq!(*tc_digest_preimage(&crypto, &tc_none).last().unwrap(), 0);

        let att = att_digest_preimage(&crypto, Some(&tc), Some(&qc));
        assert_eq!(
            hex(&att),
            format!(
                "{}01{}01{}",
                hex(TAG_ATT),
                tc_digest(&crypto, &tc),
                qc_digest(&crypto, &qc)
            )
        );
        assert_eq!(
            hex(&att_digest_preimage(&crypto, None, None)),
            format!("{}0000", hex(TAG_ATT))
        );
        assert_eq!(
            att_digest(&crypto, Some(&tc), Some(&qc)).to_string(),
            "5b2b29cdd4281dc4200e5e38bbefd3d578919e3d16202d4aae7da0a1a6781b85"
        );
        assert_eq!(
            att_digest(&crypto, None, None).to_string(),
            "21f1533cc89e45310d5124187eef8e5ff7dbeb6da41314863ecae13dd75a61a9"
        );
        // Every certificate field is bound.
        let base = qc_digest(&crypto, &qc);
        for variant in [
            Qc {
                kind: VoteKind::Commit,
                ..golden_qc()
            },
            Qc {
                instance: h(0),
                ..golden_qc()
            },
            Qc {
                height: 8,
                ..golden_qc()
            },
            Qc {
                view: 2,
                ..golden_qc()
            },
            Qc {
                block_hash: h(0),
                ..golden_qc()
            },
            Qc {
                result: h(0),
                ..golden_qc()
            },
            Qc {
                signers: Bitmap::from_indices(4, [0, 1, 2]).unwrap(),
                ..golden_qc()
            },
            Qc {
                agg_sig: AggregateSignature([0; SIGNATURE_LEN]),
                ..golden_qc()
            },
            Qc {
                attest: true,
                ..golden_qc()
            },
            Qc {
                attestations: vec![crate::message::AttestationSignature::empty()],
                ..golden_qc()
            },
        ] {
            assert_ne!(qc_digest(&crypto, &variant), base);
        }
        // Attestations are delimited: moving a byte between two of them changes the digest.
        assert_ne!(
            qc_digest(
                &crypto,
                &Qc {
                    attestations: [&[1, 2][..], &[3][..]]
                        .map(|b| crate::message::AttestationSignature::try_from_slice(b).unwrap())
                        .to_vec(),
                    ..flagged.clone()
                }
            ),
            qc_digest(
                &crypto,
                &Qc {
                    attestations: [&[1][..], &[2, 3][..]]
                        .map(|b| crate::message::AttestationSignature::try_from_slice(b).unwrap())
                        .to_vec(),
                    ..flagged
                }
            )
        );
        let base = tc_digest(&crypto, &tc);
        for variant in [
            TimeoutCert {
                view: 3,
                ..golden_tc()
            },
            TimeoutCert {
                entries: golden_tc().entries[..2].to_vec(),
                ..golden_tc()
            },
            TimeoutCert {
                high_pqc: None,
                ..golden_tc()
            },
            TimeoutCert {
                agg_sig: AggregateSignature([0; SIGNATURE_LEN]),
                ..golden_tc()
            },
        ] {
            assert_ne!(tc_digest(&crypto, &variant), base);
        }
    }

    #[test]
    fn golden_committee_digest_seed_and_instance() {
        let crypto = FakeCrypto::new();
        let committee =
            Committee::new(vec![key(3, 32), key(1, 32), key(2, 32), key(4, 32)]).unwrap();
        let pre = committee_digest_preimage(&committee);
        assert_eq!(
            hex(&pre),
            format!(
                "{}00000004{}",
                hex(TAG_COMMITTEE),
                (1..=4u8).fold(String::new(), |mut out, b| {
                    out.push_str("0020");
                    out.push_str(&format!("{b:02x}").repeat(32));
                    out
                })
            )
        );
        assert_eq!(
            committee_digest(&crypto, &committee).to_string(),
            "3677e41d43338fa446798e0bf05dc89cff145f2d9e95ded090523820f756e42d"
        );
        assert_eq!(
            topology_seed(&crypto, &h(0x11), &crate::testing::TEST_EPOCH, &committee).to_string(),
            "de56c6059900ec08726ebd96a8db6f60a7a7c047286414d483186797706fed11"
        );
        assert_eq!(
            instance_id(&crypto, &h(0x77), b"taira", InstanceKind::Dataspace, 5).to_string(),
            "a64b7c6d3c7ab3f01588d915f24332a748953cafc6b7450e8cb00c92c9b98d54"
        );
        assert_ne!(
            instance_id(&crypto, &h(0x77), b"taira", InstanceKind::Global, 5),
            instance_id(&crypto, &h(0x77), b"taira", InstanceKind::Lane, 5)
        );
        assert_eq!(
            [
                InstanceKind::Global.byte(),
                InstanceKind::Dataspace.byte(),
                InstanceKind::Lane.byte()
            ],
            [0, 1, 2]
        );
    }

    #[test]
    fn instance_height_view_separate_domains() {
        // SR16/SR17: instance, height, view, kind and result change every signing preimage.
        let base = vote_preimage(
            VoteKind::Prepare,
            &h(1),
            &crate::testing::TEST_EPOCH.id,
            1,
            1,
            &h(2),
            &h(3),
            false,
        );
        assert_ne!(
            base,
            vote_preimage(
                VoteKind::Prepare,
                &h(9),
                &crate::testing::TEST_EPOCH.id,
                1,
                1,
                &h(2),
                &h(3),
                false
            )
        );
        assert_ne!(
            base,
            vote_preimage(
                VoteKind::Prepare,
                &h(1),
                &crate::testing::TEST_EPOCH.id,
                2,
                1,
                &h(2),
                &h(3),
                false
            )
        );
        assert_ne!(
            base,
            vote_preimage(
                VoteKind::Prepare,
                &h(1),
                &crate::testing::TEST_EPOCH.id,
                1,
                2,
                &h(2),
                &h(3),
                false
            )
        );
        assert_ne!(
            base,
            vote_preimage(
                VoteKind::Commit,
                &h(1),
                &crate::testing::TEST_EPOCH.id,
                1,
                1,
                &h(2),
                &h(3),
                false
            )
        );
        assert_ne!(
            base,
            vote_preimage(
                VoteKind::Prepare,
                &h(1),
                &crate::testing::TEST_EPOCH.id,
                1,
                1,
                &h(2),
                &h(4),
                false
            )
        );
        assert_ne!(
            base,
            vote_preimage(
                VoteKind::Prepare,
                &h(1),
                &crate::testing::TEST_EPOCH.id,
                1,
                1,
                &h(2),
                &h(3),
                true
            ),
            "the attestation flag is signed (SR39)"
        );
        assert_ne!(
            tmo_preimage(&h(1), &crate::testing::TEST_EPOCH.id, 1, 1, None),
            tmo_preimage(&h(9), &crate::testing::TEST_EPOCH.id, 1, 1, None)
        );
        assert_ne!(
            prop_preimage(&h(1), &crate::testing::TEST_EPOCH.id, 1, 1, &h(2), &h(3)),
            prop_preimage(&h(9), &crate::testing::TEST_EPOCH.id, 1, 1, &h(2), &h(3))
        );
        // Kind bytes keep proposals, votes and timeouts apart.
        assert_ne!(
            prop_preimage(&h(1), &crate::testing::TEST_EPOCH.id, 1, 1, &h(2), &h(3))[..13],
            vote_preimage(
                VoteKind::Prepare,
                &h(1),
                &crate::testing::TEST_EPOCH.id,
                1,
                1,
                &h(2),
                &h(3),
                false
            )[..13]
        );
    }

    const GOLDEN_QC_PREIMAGE: &str = "73756d65726167692f71630211111111111111111111111111111111111111111111111111111111111111110000000000000000e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e00000000000000007000000000000000122222222222222222222222222222222222222222222222222222222222222223333333333333333333333333333333333333333333333333333333333333333000000010b555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555000000000000";
    const GOLDEN_TC_PREIMAGE: &str = "73756d65726167692f746311111111111111111111111111111111111111111111111111111111111111110000000000000000e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0000000000000000700000000000000020000000300000000000000000101000000000000000100000003010000000000000001666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666666019f167dc4fae6a2f5754af5ec6a2fa40391f31a15006e3d0ca7a993a7b9134def";
}

#[cfg(test)]
mod attestation_statement_tests {
    use super::*;
    #[test]
    fn exact_source_parser_consumes_domain_epoch_height_and_both_digests() {
        let source = AttestationStatement {
            instance: Hash32([1; 32]),
            epoch: crate::testing::TEST_EPOCH.id,
            height: 19,
            block_hash: Hash32([2; 32]),
            result: Hash32([3; 32]),
        };
        let bytes = att_preimage(
            &source.instance,
            &source.epoch,
            source.height,
            &source.block_hash,
            &source.result,
        );
        assert_eq!(AttestationStatement::parse(&bytes), Some(source));
        for length in 0..bytes.len() {
            assert!(AttestationStatement::parse(&bytes[..length]).is_none());
        }
        let mut suffix = bytes.clone();
        suffix.push(0);
        assert!(AttestationStatement::parse(&suffix).is_none());
        let mut domain = bytes;
        domain[0] ^= 1;
        assert!(AttestationStatement::parse(&domain).is_none());
    }
}
