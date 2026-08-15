use super::super::{
    DirectRkgOnePublicationOwnerV1, DirectRkgOnePublicationScopeV1,
    RKG_ONE_POLYNOMIAL_BYTES_V1, validate_publication_pair_v1,
};
use crate::vega::{
    sponge::Keccak256,
    zk_ams::mkhe::{
        MKHE_VERSION_V1, ZkAmsMkheErrorV1,
        active_exact_binding::SealedDirectRkgOneProofOwnerV1,
        direct_object_transport::{
            ZK_AMS_MKHE_DIRECT_OBJECT_POINTER_BYTES_V1, ZkAmsMkheDirectObjectKindV1,
            ZkAmsMkheDirectObjectPointerV1, ZkAmsMkheDirectObjectPublicationReceiptV1,
        },
    },
};

const STORAGE_KEY_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-rkg-one-orphan-transaction";
const RECORD_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v1.mkhe.direct-rkg-one-lifecycle-record.v2";
const RECEIPT_SET_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-rkg-one-publication-receipt-set.v2";
const RECORD_TAG_V2: [u8; 4] = *b"R1L2";
const SCHEMA_VERSION_V2: u8 = 2;
const RELINEARIZATION_TARGET_TAG_V1: u8 = 0;
const FRESH_TAG_V2: u8 = 0;
const PUBLISHED_UNBOUND_TAG_V2: u8 = 1;
const PROOF_PUBLISHED_UNVERIFIED_TAG_V2: u8 = 2;
const VERIFIED_BOUND_RESERVED_TAG_V2: u8 = 3;

pub(super) const RECORD_BYTES_V2: usize = 640;
pub(super) const LEGACY_RECORD_BYTES_V1: usize = 334;
pub(super) const RECORD_PREFIX_BYTES_V2: usize = 608;
const TRANSACTION_ID_RANGE_V2: core::ops::Range<usize> = 16..48;
const CONTEXT_DIGEST_RANGE_V2: core::ops::Range<usize> = 48..80;
const PARTY_ID_RANGE_V2: core::ops::Range<usize> = 82..114;
const PUBLICATION_IDENTITY_RANGE_V2: core::ops::Range<usize> = 114..146;
const H0_POINTER_RANGE_V2: core::ops::Range<usize> = 146..224;
const H1_POINTER_RANGE_V2: core::ops::Range<usize> = 224..302;
const RECEIPT_SET_DIGEST_RANGE_V2: core::ops::Range<usize> = 302..334;
const PROVIDER_IDENTITY_RANGE_V2: core::ops::Range<usize> = 334..366;
const SNAPSHOT_IDENTITY_RANGE_V2: core::ops::Range<usize> = 366..398;
const PROOF_PUBLICATION_IDENTITY_RANGE_V2: core::ops::Range<usize> = 398..430;
const PROOF_POINTER_RANGE_V2: core::ops::Range<usize> = 430..508;
const PROOF_RECEIPT_DIGEST_RANGE_V2: core::ops::Range<usize> = 508..540;
const VERIFIER_RECEIPT_DIGEST_RANGE_V2: core::ops::Range<usize> = 540..572;
const BINDING_IDENTITY_RANGE_V2: core::ops::Range<usize> = 572..604;
const RECORD_DIGEST_RANGE_V2: core::ops::Range<usize> = 608..640;

const _: () = {
    assert!(ZK_AMS_MKHE_DIRECT_OBJECT_POINTER_BYTES_V1 == 78);
    assert!(RECORD_PREFIX_BYTES_V2 + 32 == RECORD_BYTES_V2);
    assert!(2 * RECORD_BYTES_V2 == 1_280);
    assert!(8 * 38 * RECORD_BYTES_V2 == 194_560);
};

pub(super) type RecordV2 = [u8; RECORD_BYTES_V2];

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct PublishedAxesV2 {
    pub(super) publication_identity: [u8; 32],
    pub(super) h0: ZkAmsMkheDirectObjectPointerV1,
    pub(super) h1: ZkAmsMkheDirectObjectPointerV1,
    pub(super) receipt_set_digest: [u8; 32],
    pub(super) provider_identity: [u8; 32],
    pub(super) snapshot_identity: [u8; 32],
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct ProofAxesV2 {
    pub(super) publication_identity: [u8; 32],
    pub(super) pointer: ZkAmsMkheDirectObjectPointerV1,
    pub(super) receipt_digest: [u8; 32],
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum DecodedStateV2 {
    Fresh,
    PublishedUnbound(PublishedAxesV2),
    ProofPublishedUnverified(PublishedAxesV2, ProofAxesV2),
}

pub(super) fn stable_storage_key_v2(
    scope: DirectRkgOnePublicationScopeV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    validate_scope_v2(scope)?;
    let mut hash = Keccak256::new();
    hash.update(STORAGE_KEY_DOMAIN_V1);
    hash.update(&[MKHE_VERSION_V1]);
    hash.update(&scope.context_digest);
    hash.update(&[
        RELINEARIZATION_TARGET_TAG_V1,
        scope.digit_index,
        scope.party_index,
    ]);
    hash.update(&scope.party.to_bytes());
    Ok(hash.finalize())
}

pub(super) fn published_axes_v2(
    publication: &DirectRkgOnePublicationOwnerV1,
) -> Result<PublishedAxesV2, ZkAmsMkheErrorV1> {
    validate_publication_pair_v1(publication)?;
    let h0 = &publication.h0.publication;
    let h1 = &publication.h1.publication;
    let h0_snapshot = h0.post_publish_read_receipt().snapshot();
    let h1_snapshot = h1.post_publish_read_receipt().snapshot();
    let mut hash = Keccak256::new();
    hash.update(RECEIPT_SET_DOMAIN_V2);
    hash.update(&[MKHE_VERSION_V1, SCHEMA_VERSION_V2]);
    hash.update(&h0.receipt_digest());
    hash.update(&h1.receipt_digest());
    let axes = PublishedAxesV2 {
        publication_identity: h0.publication_identity(),
        h0: h0.pointer(),
        h1: h1.pointer(),
        receipt_set_digest: hash.finalize(),
        provider_identity: h0_snapshot.provider_identity(),
        snapshot_identity: h0_snapshot.snapshot_identity(),
    };
    if h1.publication_identity() != axes.publication_identity
        || h1_snapshot.provider_identity() != axes.provider_identity
        || h1_snapshot.snapshot_identity() != axes.snapshot_identity
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    validate_published_axes_v2(axes)?;
    Ok(axes)
}

pub(super) fn proof_axes_v2(
    published: PublishedAxesV2,
    receipt: &ZkAmsMkheDirectObjectPublicationReceiptV1,
) -> Result<ProofAxesV2, ZkAmsMkheErrorV1> {
    let pointer = receipt.pointer();
    let snapshot = receipt.post_publish_read_receipt().snapshot();
    let axes = ProofAxesV2 {
        publication_identity: receipt.publication_identity(),
        pointer,
        receipt_digest: receipt.receipt_digest(),
    };
    if axes.publication_identity != published.publication_identity
        || pointer.kind() != ZkAmsMkheDirectObjectKindV1::ProofEnvelope
        || pointer.payload_bytes()
            != SealedDirectRkgOneProofOwnerV1::CANONICAL_PROOF_BYTES_V1
        || receipt.post_publish_read_receipt().canonical_bytes() != pointer.payload_bytes()
        || snapshot.pointer() != pointer
        || snapshot.provider_identity() != published.provider_identity
        || snapshot.snapshot_identity() != published.snapshot_identity
        || axes.receipt_digest == [0; 32]
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(axes)
}

pub(super) fn encode_fresh_v2(
    scope: DirectRkgOnePublicationScopeV1,
    id: [u8; 32],
    record: &mut RecordV2,
) -> Result<(), ZkAmsMkheErrorV1> {
    encode_base_v2(scope, id, FRESH_TAG_V2, record)
}

pub(super) fn encode_published_v2(
    scope: DirectRkgOnePublicationScopeV1,
    id: [u8; 32],
    axes: PublishedAxesV2,
    record: &mut RecordV2,
) -> Result<(), ZkAmsMkheErrorV1> {
    validate_published_axes_v2(axes)?;
    encode_base_v2(scope, id, PUBLISHED_UNBOUND_TAG_V2, record)?;
    write_published_axes_v2(axes, record);
    finish_record_v2(record);
    Ok(())
}

pub(super) fn encode_proof_v2(
    scope: DirectRkgOnePublicationScopeV1,
    id: [u8; 32],
    published: PublishedAxesV2,
    proof: ProofAxesV2,
    record: &mut RecordV2,
) -> Result<(), ZkAmsMkheErrorV1> {
    validate_published_axes_v2(published)?;
    validate_proof_axes_v2(proof)?;
    encode_base_v2(scope, id, PROOF_PUBLISHED_UNVERIFIED_TAG_V2, record)?;
    write_published_axes_v2(published, record);
    record[PROOF_PUBLICATION_IDENTITY_RANGE_V2].copy_from_slice(&proof.publication_identity);
    record[PROOF_POINTER_RANGE_V2].copy_from_slice(&proof.pointer.encode());
    record[PROOF_RECEIPT_DIGEST_RANGE_V2].copy_from_slice(&proof.receipt_digest);
    finish_record_v2(record);
    Ok(())
}

pub(super) fn decode_record_v2(
    scope: DirectRkgOnePublicationScopeV1,
    id: [u8; 32],
    record: &RecordV2,
) -> Result<DecodedStateV2, ZkAmsMkheErrorV1> {
    validate_scope_v2(scope)?;
    if stable_storage_key_v2(scope)? != id
        || record[..4] != RECORD_TAG_V2
        || record[4] != MKHE_VERSION_V1
        || record[5] != SCHEMA_VERSION_V2
        || record[7] != 0
        || record[TRANSACTION_ID_RANGE_V2] != id
        || record[CONTEXT_DIGEST_RANGE_V2] != scope.context_digest
        || record[80] != scope.party_index
        || record[81] != scope.digit_index
        || record[PARTY_ID_RANGE_V2] != scope.party.to_bytes()
        || record[604..608] != [0; 4]
        || record[RECORD_DIGEST_RANGE_V2] != record_digest_v2(record)
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let generation = u64::from_be_bytes(
        record[8..16]
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
    );
    match record[6] {
        FRESH_TAG_V2 if generation == 0 && record[114..608] == [0; 494] => {
            Ok(DecodedStateV2::Fresh)
        }
        PUBLISHED_UNBOUND_TAG_V2 if generation == 1 && record[398..608] == [0; 210] => {
            Ok(DecodedStateV2::PublishedUnbound(
                decode_published_axes_v2(record)?,
            ))
        }
        PROOF_PUBLISHED_UNVERIFIED_TAG_V2
            if generation == 2 && record[540..608] == [0; 68] =>
        {
            let published = decode_published_axes_v2(record)?;
            let proof = ProofAxesV2 {
                publication_identity: array_at_v2(record, PROOF_PUBLICATION_IDENTITY_RANGE_V2)?,
                pointer: ZkAmsMkheDirectObjectPointerV1::decode_exact(
                    ZkAmsMkheDirectObjectKindV1::ProofEnvelope,
                    &record[PROOF_POINTER_RANGE_V2],
                )?,
                receipt_digest: array_at_v2(record, PROOF_RECEIPT_DIGEST_RANGE_V2)?,
            };
            validate_proof_axes_v2(proof)?;
            if proof.publication_identity != published.publication_identity {
                return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
            }
            Ok(DecodedStateV2::ProofPublishedUnverified(
                published, proof,
            ))
        }
        VERIFIED_BOUND_RESERVED_TAG_V2 => Err(ZkAmsMkheErrorV1::InvalidWireEncoding),
        _ => Err(ZkAmsMkheErrorV1::InvalidWireEncoding),
    }
}

fn encode_base_v2(
    scope: DirectRkgOnePublicationScopeV1,
    id: [u8; 32],
    state: u8,
    record: &mut RecordV2,
) -> Result<(), ZkAmsMkheErrorV1> {
    validate_scope_v2(scope)?;
    if stable_storage_key_v2(scope)? != id || state > PROOF_PUBLISHED_UNVERIFIED_TAG_V2 {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    record.fill(0);
    record[..4].copy_from_slice(&RECORD_TAG_V2);
    record[4] = MKHE_VERSION_V1;
    record[5] = SCHEMA_VERSION_V2;
    record[6] = state;
    record[8..16].copy_from_slice(&u64::from(state).to_be_bytes());
    record[TRANSACTION_ID_RANGE_V2].copy_from_slice(&id);
    record[CONTEXT_DIGEST_RANGE_V2].copy_from_slice(&scope.context_digest);
    record[80] = scope.party_index;
    record[81] = scope.digit_index;
    record[PARTY_ID_RANGE_V2].copy_from_slice(&scope.party.to_bytes());
    finish_record_v2(record);
    Ok(())
}

fn write_published_axes_v2(axes: PublishedAxesV2, record: &mut RecordV2) {
    record[PUBLICATION_IDENTITY_RANGE_V2].copy_from_slice(&axes.publication_identity);
    record[H0_POINTER_RANGE_V2].copy_from_slice(&axes.h0.encode());
    record[H1_POINTER_RANGE_V2].copy_from_slice(&axes.h1.encode());
    record[RECEIPT_SET_DIGEST_RANGE_V2].copy_from_slice(&axes.receipt_set_digest);
    record[PROVIDER_IDENTITY_RANGE_V2].copy_from_slice(&axes.provider_identity);
    record[SNAPSHOT_IDENTITY_RANGE_V2].copy_from_slice(&axes.snapshot_identity);
}

fn decode_published_axes_v2(record: &RecordV2) -> Result<PublishedAxesV2, ZkAmsMkheErrorV1> {
    let axes = PublishedAxesV2 {
        publication_identity: array_at_v2(record, PUBLICATION_IDENTITY_RANGE_V2)?,
        h0: ZkAmsMkheDirectObjectPointerV1::decode_exact(
            ZkAmsMkheDirectObjectKindV1::RkgH0,
            &record[H0_POINTER_RANGE_V2],
        )?,
        h1: ZkAmsMkheDirectObjectPointerV1::decode_exact(
            ZkAmsMkheDirectObjectKindV1::RkgH1,
            &record[H1_POINTER_RANGE_V2],
        )?,
        receipt_set_digest: array_at_v2(record, RECEIPT_SET_DIGEST_RANGE_V2)?,
        provider_identity: array_at_v2(record, PROVIDER_IDENTITY_RANGE_V2)?,
        snapshot_identity: array_at_v2(record, SNAPSHOT_IDENTITY_RANGE_V2)?,
    };
    validate_published_axes_v2(axes)?;
    Ok(axes)
}

fn validate_scope_v2(scope: DirectRkgOnePublicationScopeV1) -> Result<(), ZkAmsMkheErrorV1> {
    if scope.context_digest == [0; 32]
        || usize::from(scope.party_index) >= 8
        || usize::from(scope.digit_index) >= 38
        || scope.party.to_bytes() == [0; 32]
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}

fn validate_published_axes_v2(axes: PublishedAxesV2) -> Result<(), ZkAmsMkheErrorV1> {
    if axes.publication_identity == [0; 32]
        || axes.receipt_set_digest == [0; 32]
        || axes.provider_identity == [0; 32]
        || axes.snapshot_identity == [0; 32]
        || axes.h0.kind() != ZkAmsMkheDirectObjectKindV1::RkgH0
        || axes.h1.kind() != ZkAmsMkheDirectObjectKindV1::RkgH1
        || axes.h0.payload_bytes() != RKG_ONE_POLYNOMIAL_BYTES_V1
        || axes.h1.payload_bytes() != RKG_ONE_POLYNOMIAL_BYTES_V1
        || axes.h0 == axes.h1
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(())
}

fn validate_proof_axes_v2(axes: ProofAxesV2) -> Result<(), ZkAmsMkheErrorV1> {
    if axes.publication_identity == [0; 32]
        || axes.receipt_digest == [0; 32]
        || axes.pointer.kind() != ZkAmsMkheDirectObjectKindV1::ProofEnvelope
        || axes.pointer.payload_bytes()
            != SealedDirectRkgOneProofOwnerV1::CANONICAL_PROOF_BYTES_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(())
}

fn finish_record_v2(record: &mut RecordV2) {
    let digest = record_digest_v2(record);
    record[RECORD_DIGEST_RANGE_V2].copy_from_slice(&digest);
}

pub(super) fn record_digest_v2(record: &RecordV2) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(RECORD_DOMAIN_V2);
    hash.update(&record[..RECORD_PREFIX_BYTES_V2]);
    hash.finalize()
}

fn array_at_v2(
    record: &RecordV2,
    range: core::ops::Range<usize>,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    record[range]
        .try_into()
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)
}

#[cfg(test)]
pub(super) fn encode_reserved_verified_for_test_v2(
    scope: DirectRkgOnePublicationScopeV1,
    id: [u8; 32],
    published: PublishedAxesV2,
    proof: ProofAxesV2,
    verifier_receipt_digest: [u8; 32],
    binding_identity: [u8; 32],
    record: &mut RecordV2,
) {
    encode_proof_v2(scope, id, published, proof, record).expect("valid test axes");
    record[6] = VERIFIED_BOUND_RESERVED_TAG_V2;
    record[8..16].copy_from_slice(&3_u64.to_be_bytes());
    record[VERIFIER_RECEIPT_DIGEST_RANGE_V2].copy_from_slice(&verifier_receipt_digest);
    record[BINDING_IDENTITY_RANGE_V2].copy_from_slice(&binding_identity);
    finish_record_v2(record);
}
