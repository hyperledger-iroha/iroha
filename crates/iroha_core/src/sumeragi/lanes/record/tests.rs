//! Real BLS lane records, exact wire parity, context rejection and retained write funding.

use iroha_crypto::{Algorithm, KeyPair, bls_normal_pop_prove};
use iroha_sumeragi::{
    availability::PayloadAuthoring,
    crypto::{AttestationVerifier, Crypto, NoAttestation, Signer, Verifier},
    message::{AttestationSignature, ResultWitness},
    types::{
        AggregateSignature, Bitmap, Hash32, PublicKey, SIGNATURE_LEN, Signature, ValidatorIndex,
    },
};

use super::*;
use crate::sumeragi::{
    body_record,
    crypto::{BlsCrypto, KeyPairSigner},
};

pub(in crate::sumeragi) fn fixture(
    size: usize,
    witness_size: Option<usize>,
) -> (
    AvailableBody,
    Qc,
    AvailabilitySource,
    AllocationBudget,
    BlsCrypto,
) {
    let (mut body, mut source, budget) = body_record::tests::fixture(size);
    let crypto = BlsCrypto::new();
    let pairs: Vec<_> = (1..=4)
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
        .collect();
    let mut signers = Vec::new();
    for pair in &pairs {
        crypto
            .admit(
                pair.public_key(),
                &bls_normal_pop_prove(pair.private_key()).unwrap(),
            )
            .unwrap();
        signers.push(KeyPairSigner::new(pair).unwrap());
    }
    signers.sort_by(|a, b| a.public_key().cmp(b.public_key()));
    if witness_size.is_some() {
        let mut header = body.header().clone();
        header.attest = true;
        let authored = PayloadAuthoring::new(header, body.payload().clone())
            .complete(
                source.instance(),
                source.config(),
                &budget,
                &crypto,
                &signers[0],
            )
            .unwrap_or_else(|_| panic!("real signed flagged original body"));
        body = authored.body;
        drop(authored.codeword);
        source = AvailabilitySource::new(
            source.instance(),
            source.height(),
            body.hash(&crypto),
            source.config().clone(),
        )
        .unwrap();
    }
    let witness = witness_size.map(|size| {
        let mut bytes = ChargedBuffer::new(size, &budget).unwrap();
        for i in 0..size {
            bytes.push_reserved((i % 239) as u8);
        }
        ResultWitness::from_charged(bytes, &budget)
            .unwrap_or_else(|_| panic!("actual witness backing/control"))
    });
    let result = witness
        .as_ref()
        .map_or(Hash32([3; 32]), |w| crypto.hash(w.as_slice()));
    let mut qc = Qc {
        kind: VoteKind::Commit,
        instance: body.header().instance,
        epoch: body.header().epoch,
        height: body.header().height,
        view: body.header().origin_view,
        block_hash: body.hash(&crypto),
        result,
        attest: body.header().attest,
        signers: Bitmap::from_indices(4, [0, 1, 2]).unwrap(),
        agg_sig: AggregateSignature([0; SIGNATURE_LEN]),
        attestations: Vec::new(),
        attestation_witness: witness,
    };
    let signatures: Vec<_> = signers[..3]
        .iter()
        .map(|signer| signer.sign(&qc.preimage()))
        .collect();
    qc.agg_sig = crypto.aggregate(&signatures);
    if qc.attest {
        qc.attestations = signers[..3]
            .iter()
            .map(|signer| {
                AttestationSignature::try_from_slice(&signer.sign(&qc.statement()).0).unwrap()
            })
            .collect();
    }
    (body, qc, source, budget, crypto)
}

struct ExactAttestation<'a> {
    qc: &'a Qc,
    crypto: &'a BlsCrypto,
}
impl AttestationVerifier for ExactAttestation<'_> {
    fn verify(
        &self,
        height: u64,
        _signer: ValidatorIndex,
        key: &PublicKey,
        statement: &[u8],
        witness: &ResultWitness,
        signature: &[u8],
    ) -> bool {
        let Ok(bytes) = <[u8; SIGNATURE_LEN]>::try_from(signature) else {
            return false;
        };
        height == self.qc.height
            && statement == self.qc.statement()
            && self.crypto.hash(witness.as_slice()) == self.qc.result
            && self.crypto.verify(key, statement, &Signature(bytes))
    }
}

pub(in crate::sumeragi) fn verify(source: &AvailabilitySource, qc: &Qc, crypto: &BlsCrypto) {
    let verifier = Verifier::new(
        crypto,
        &qc.instance,
        &source.config().epoch.id,
        &source.config().committee,
    );
    if qc.attest {
        verifier
            .verify_qc(&ExactAttestation { qc, crypto }, qc)
            .unwrap();
    } else {
        verifier.verify_qc(&NoAttestation, qc).unwrap();
    }
}

pub(super) fn raw_job(
    size: usize,
    witness_size: Option<usize>,
) -> (
    LaneRecordDecode,
    AvailabilitySource,
    AllocationBudget,
    BlsCrypto,
) {
    let (body, qc, source, budget, crypto) = fixture(size, witness_size);
    verify(&source, &qc, &crypto);
    let mut prepared = PreparedLaneWrite::new(body, qc);
    prepared.check_context(&source, &crypto).unwrap();
    let bytes = prepared.prepare(&budget).unwrap();
    let mut raw = ChargedBuffer::new(bytes.len(), &budget).unwrap();
    raw.append(bytes).unwrap();
    drop(prepared);
    (LaneRecordDecode::new(raw), source, budget, crypto)
}

#[test]
fn exact_owned_borrowed_wire_roundtrip_preserves_real_bls_body_and_qc() {
    for witness_size in [None, Some(1025)] {
        let (body, qc, source, budget, crypto) = fixture(23572, witness_size);
        verify(&source, &qc, &crypto);
        let expected = norito::encode_canonical(&LaneRecord {
            header: body.header().clone(),
            availability: body.availability().clone(),
            payload: body.payload().clone(),
            commit_qc: qc.clone(),
        })
        .unwrap();
        assert_eq!(
            norito::encode_canonical(&QcRef::from(&qc)).unwrap(),
            norito::encode_canonical(&qc).unwrap()
        );
        let mut prepared = PreparedLaneWrite::new(body, qc);
        prepared.check_context(&source, &crypto).unwrap();
        let encoded = prepared.prepare(&budget).unwrap();
        assert_eq!(encoded, expected);
        let mut raw = ChargedBuffer::new(encoded.len(), &budget).unwrap();
        raw.append(encoded).unwrap();
        let record = LaneRecordDecode::new(raw)
            .complete(&budget)
            .unwrap_or_else(|_| panic!("funded canonical decode"));
        record.check_context(&source, &crypto).unwrap();
        assert_eq!(record.header(), prepared.body().header());
        assert_eq!(record.availability(), prepared.body().availability());
        assert_eq!(record.commit_qc(), prepared.commit_qc());
        verify(&source, record.commit_qc(), &crypto);
        let (restoration, qc) = record.into_restoration(source);
        let restored = restoration
            .complete(&budget, &crypto)
            .unwrap_or_else(|_| panic!("actual full-codeword restoration"));
        assert_eq!(&restored, prepared.body());
        assert_eq!(&qc, prepared.commit_qc());
    }
}

#[test]
fn retained_write_refusal_and_foreign_pool_preserve_original_pointers() {
    let (body, qc, source, budget, crypto) = fixture(1025, Some(65536));
    let pointers = (
        body.payload().as_slice().as_ptr(),
        body.availability().as_slice().as_ptr(),
        qc.attestation_witness.as_ref().unwrap().as_slice().as_ptr(),
    );
    let mut prepared = PreparedLaneWrite::new(body, qc);
    let retained = budget.reserved_bytes();
    budget.set_limit_bytes(retained);
    assert!(matches!(
        prepared.prepare(&budget),
        Err(LaneRecordError::Allocation(_))
    ));
    assert_eq!(budget.reserved_bytes(), retained);
    let foreign = AllocationBudget::new(1 << 25);
    assert!(matches!(
        prepared.prepare(&foreign),
        Err(LaneRecordError::ForeignBudget)
    ));
    assert_eq!(foreign.reserved_bytes(), 0);
    budget.set_limit_bytes(1 << 25);
    prepared.check_context(&source, &crypto).unwrap();
    let pointer = prepared.prepare(&budget).unwrap().as_ptr();
    let charged = budget.reserved_bytes();
    for _ in 0..3 {
        assert_eq!(prepared.prepare(&budget).unwrap().as_ptr(), pointer);
        assert_eq!(budget.reserved_bytes(), charged);
    }
    assert_eq!(
        pointers,
        (
            prepared.body().payload().as_slice().as_ptr(),
            prepared.body().availability().as_slice().as_ptr(),
            prepared
                .commit_qc()
                .attestation_witness
                .as_ref()
                .unwrap()
                .as_slice()
                .as_ptr()
        )
    );
    drop(prepared);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn context_mismatches_and_foreign_witness_never_prepare_an_output() {
    for changed in 0..5 {
        let (body, mut qc, _, budget, _) = fixture(1, None);
        match changed {
            0 => qc.kind = VoteKind::Prepare,
            1 => qc.instance = Hash32([9; 32]),
            2 => qc.epoch.context = Hash32([9; 32]),
            3 => qc.height += 1,
            _ => qc.attest = true,
        }
        let before = budget.reserved_bytes();
        let mut prepared = PreparedLaneWrite::new(body, qc);
        assert!(matches!(
            prepared.prepare(&budget),
            Err(LaneRecordError::Context)
        ));
        assert!(prepared.bytes.is_none());
        assert_eq!(budget.reserved_bytes(), before);
    }
    let (body, mut qc, source, budget, crypto) = fixture(1, Some(9));
    let foreign = AllocationBudget::new(1024);
    let mut buffer = ChargedBuffer::new(9, &foreign).unwrap();
    buffer.append(&[7; 9]).unwrap();
    qc.attestation_witness = Some(
        ResultWitness::from_charged(buffer, &foreign).unwrap_or_else(|_| panic!("foreign fixture")),
    );
    let mut prepared = PreparedLaneWrite::new(body, qc);
    assert!(matches!(
        prepared.prepare(&budget),
        Err(LaneRecordError::ForeignBudget)
    ));
    assert!(prepared.bytes.is_none());
    let wrong = AvailabilitySource::new(
        source.instance(),
        source.height(),
        Hash32([9; 32]),
        source.config().clone(),
    )
    .unwrap();
    assert!(matches!(
        prepared.check_context(&wrong, &crypto),
        Err(LaneRecordError::Context)
    ));
}

#[test]
fn prepared_lane_parts_move_original_metadata_and_bulk_owners_without_readmission() {
    let (body, qc, _source, budget, _crypto) = fixture(1025, Some(257));
    let payload = body.payload().as_slice().as_ptr();
    let availability = body.availability().as_slice().as_ptr();
    let epoch = std::ptr::from_ref(&*body.source().config().epoch);
    let attestations = qc.attestations.as_ptr();
    let witness = qc.attestation_witness.as_ref().unwrap().as_slice().as_ptr();
    let mut prepared = PreparedLaneWrite::new(body, qc);
    let retained = budget.reserved_bytes();
    let output_len = prepared.prepare(&budget).unwrap().len();
    assert_eq!(budget.reserved_bytes(), retained + output_len);
    budget.set_limit_bytes(0);
    let (body, qc) = prepared.into_parts();
    assert_eq!(body.payload().as_slice().as_ptr(), payload);
    assert_eq!(body.availability().as_slice().as_ptr(), availability);
    assert_eq!(std::ptr::from_ref(&*body.source().config().epoch), epoch);
    assert_eq!(qc.attestations.as_ptr(), attestations);
    assert_eq!(
        qc.attestation_witness.as_ref().unwrap().as_slice().as_ptr(),
        witness
    );
    assert!(body.admitted_to(&budget));
    assert_eq!(budget.reserved_bytes(), retained);
    drop(body);
    drop(qc);
    assert_eq!(budget.reserved_bytes(), 0);
}
