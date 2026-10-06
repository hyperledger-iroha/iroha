//! Transitive Payment hashing from the same constrained component tapes.
//!
//! This layer binds the compact Payment transcript to its Request, credential,
//! Send statement, receipt and proof digest. It returns a total predicate;
//! recursive proof decisions and signature authorizations are separate,
//! mandatory inputs to the owning Receive or Archive operation.

use ff::Field;
use iroha_pasta::Fp;
use iroha_plonk::frontend::{Error, Region};
use iroha_plonk_gadgets::{
    Bit, UintChip, Word, WordHasher,
    bytes::{
        PBytes, chunk_segments,
        tape::{ByteRun, SegmentSpec},
    },
};

use super::{
    ObjectKind, SignedObjectCells,
    credential::CredentialCells,
    decode_atom,
    predicates::{all, equal, is_constant},
    receipt::{self, ReceiptContext},
    request::RequestCells,
    schema::Atom,
};
use crate::operation_relation::incoming_statement::StatementView;

const SCHEMA: [Atom; 5] = [
    Atom::Integer(2),
    Atom::Field,
    Atom::Key,
    Atom::Field,
    Atom::Field,
];

/// Same-tape components of a Payment's Send package.
///
/// The proof digest must be recomputed from exact `LE32 Ω.len || Ω ||
/// LE32 σ.len || σ` bytes under `kgwprf_1`, tied to the corresponding Q
/// slots. The owning relation separately authenticates the payer credential,
/// receipt signature and incoming predecessor, and closes every proof obligation.
pub struct PaymentInputs<'a, S: StatementView> {
    /// Exact signed Request carried by this Payment.
    pub request: &'a RequestCells,
    /// Payer credential from the retained Offer.
    pub payer: &'a CredentialCells,
    /// Send statement verified by the incoming sigma slot.
    pub statement: &'a S,
    /// Payer's Send receipt, verified under its payment key.
    pub receipt: &'a SignedObjectCells,
    /// Pinned Advance provider contract identity.
    pub provider: &'a [Word<Fp>; 2],
    /// Digest of the two exact proof encodings, including their lengths.
    pub proof_digest: &'a Word<Fp>,
}

/// Exact compact Payment transcript, recomputed package digest and total validity.
/// This is a constrained binding, not a proof or signature authorization.
#[derive(Clone, Debug)]
pub struct PaymentCells {
    digest: Word<Fp>,
    package: Word<Fp>,
    valid: Bit<Fp>,
}
impl PaymentCells {
    /// Fixed `LE16 version || request || SEC1 key || credential || package` size.
    pub const BYTES: usize = 163;

    /// `P_bytes` chunks spanning the entire exact transcript.
    pub fn primary_segments() -> Vec<usize> {
        chunk_segments(0, Self::BYTES)
    }

    /// Integer, canonical field and big-endian key limb views of those bytes.
    pub fn secondary_segments() -> Vec<SegmentSpec> {
        let mut out = vec![
            SegmentSpec::little(0, 2),
            SegmentSpec::little(2, 16),
            SegmentSpec::little(18, 16),
            SegmentSpec::little(34, 1),
        ];
        for i in 0..4 {
            out.push(SegmentSpec::big(35 + i * 16, 16));
        }
        for offset in [99, 131] {
            out.push(SegmentSpec::little(offset, 16));
            out.push(SegmentSpec::little(offset + 16, 16));
        }
        out
    }

    /// Decode and bind an incoming Payment without replacing malformed bytes.
    ///
    /// The digest always hashes the original transcript. A noncanonical field,
    /// wrong SEC1 prefix, wrong version, invalid source body or mismatched binding
    /// gives a false predicate, allowing the corrected-claim burn/no-op branch.
    ///
    /// # Errors
    /// Wrong fixed length/segments, a non-Send statement, wrong receipt class or
    /// layout failure. Semantic mismatches return a constrained false verdict.
    pub fn from_run<S: StatementView>(
        uint: &mut UintChip<'_, Fp>,
        hash: &mut impl WordHasher<Fp>,
        region: &mut Region<'_, Fp>,
        run: &ByteRun<Fp>,
        inputs: &PaymentInputs<'_, S>,
    ) -> Result<Self, Error> {
        if run.len() != Self::BYTES || inputs.receipt.kind() != ObjectKind::Receipt {
            return Err(Error::Synthesis);
        }
        let mut offset = 0;
        let mut fields = Vec::new();
        let mut checks = Vec::new();
        for atom in SCHEMA {
            let (field, valid) = decode_atom(uint, region, run, offset, atom)?;
            fields.push(field);
            checks.push(valid);
            offset += atom.len();
        }
        checks.push(is_constant(uint.glue(), region, &fields[0][0], 1)?);
        let package = hash.hash_words(
            region,
            u64::from_le_bytes(*b"kgwpkg_1"),
            &[
                inputs.statement.digest().clone(),
                inputs.proof_digest.clone(),
                inputs.receipt.digest().clone(),
            ],
        )?;
        for (actual, expected) in [
            (&fields[1][0], inputs.request.object().digest()),
            (&fields[3][0], inputs.payer.object().digest()),
            (&fields[4][0], &package),
        ] {
            checks.push(uint.glue().is_equal(region, actual, expected)?);
        }
        checks.push(equal(
            uint.glue(),
            region,
            &fields[2],
            inputs.payer.payment_key()?,
        )?);
        checks.push(
            inputs
                .request
                .bind_send(uint, region, inputs.statement, inputs.payer)?,
        );
        let zero = uint.glue().constant(region, Fp::ZERO)?;
        checks.push(receipt::bind(
            uint,
            hash,
            region,
            inputs.receipt,
            &ReceiptContext {
                wallet: inputs.payer.object().identifier(3)?,
                provider: inputs.provider,
                statement: inputs.statement,
                proof_digest: inputs.proof_digest,
                payment_digest: &zero,
            },
        )?);
        let mut tape = PBytes::new();
        for segment in run.primary() {
            tape.push_bounded(segment.bounded().ok_or(Error::Synthesis)?)?;
        }
        if tape.len() != Self::BYTES {
            return Err(Error::Synthesis);
        }
        let digest = tape.digest(uint.glue(), hash, region, u64::from_le_bytes(*b"kgwpay_1"))?;
        let valid = all(uint.glue(), region, &checks)?;
        Ok(Self {
            digest,
            package,
            valid,
        })
    }
    /// Payment digest of the original exact 163-byte transcript.
    pub const fn digest(&self) -> &Word<Fp> {
        &self.digest
    }
    /// Recomputed package digest, never the unchecked carried value.
    pub const fn package_digest(&self) -> &Word<Fp> {
        &self.package
    }
    /// Binding/body verdict that must join every incoming verification predicate.
    pub const fn valid(&self) -> &Bit<Fp> {
        &self.valid
    }
}
