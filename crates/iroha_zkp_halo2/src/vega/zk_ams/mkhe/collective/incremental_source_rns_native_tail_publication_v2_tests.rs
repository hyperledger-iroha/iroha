use std::{cell::Cell, rc::Rc};

use super::super::super::super::ZkAmsMkhePartyIdV1;
use super::super::super::super::direct_object_transport::{
    ZkAmsMkheDirectObjectPublishedBindingV1, ZkAmsMkheDirectObjectSealTokenV1,
    ZkAmsMkheDirectObjectStagingTokenV1,
};
use super::super::{
    StreamingCollectiveEncryptionKeyAuthoritySealV1, ZkAmsMkheStreamingCollectiveKeyBindingV1,
};
use super::*;

const PUBLICATION_ID: [u8; 32] = [0x11; 32];
const PROVIDER_ID: [u8; 32] = [0x22; 32];
const SNAPSHOT_ID: [u8; 32] = [0x33; 32];
const STAGING_ID: [u8; 32] = [0x44; 32];
const SEAL_ID: [u8; 32] = [0x55; 32];
const PUBLISHED_ID: [u8; 32] = [0x66; 32];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CasFaultV2 {
    None,
    ShortWrite,
    PanicWrite,
    ShortPublishedRead,
    MutatePublishedRead,
    DriftPublishedRead,
}

struct StageV2 {
    token_digest: [u8; 32],
    kind: ZkAmsMkheDirectObjectKindV1,
    bytes: Vec<u8>,
    expected: u64,
}

struct SealV2 {
    token_digest: [u8; 32],
    kind: ZkAmsMkheDirectObjectKindV1,
    bytes: Vec<u8>,
}

struct ObjectV2 {
    pointer: ZkAmsMkheDirectObjectPointerV1,
    bytes: Vec<u8>,
}

struct OneObjectCasV2 {
    fault: CasFaultV2,
    provider_identity: [u8; 32],
    stage: Option<StageV2>,
    seal: Option<SealV2>,
    object: Option<ObjectV2>,
    writes: usize,
    sealed_reads: usize,
    published_reads: usize,
    begin_calls: Option<Rc<Cell<usize>>>,
}

impl OneObjectCasV2 {
    const fn new_v2(fault: CasFaultV2) -> Self {
        Self {
            fault,
            provider_identity: PROVIDER_ID,
            stage: None,
            seal: None,
            object: None,
            writes: 0,
            sealed_reads: 0,
            published_reads: 0,
            begin_calls: None,
        }
    }

    fn counted_v2(begin_calls: Rc<Cell<usize>>) -> Self {
        Self {
            begin_calls: Some(begin_calls),
            ..Self::new_v2(CasFaultV2::None)
        }
    }

    const fn invalid_v2() -> ZkAmsMkheErrorV1 {
        ZkAmsMkheErrorV1::InvalidKeyMaterial
    }
}

impl ZkAmsMkheDirectObjectReadAtProviderV1 for OneObjectCasV2 {
    fn provider_identity(&mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        Ok(self.provider_identity)
    }

    fn snapshot_identity(&mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        Ok(SNAPSHOT_ID)
    }

    fn object_len(
        &mut self,
        pointer: ZkAmsMkheDirectObjectPointerV1,
    ) -> Result<u64, ZkAmsMkheErrorV1> {
        let object = self
            .object
            .as_ref()
            .filter(|object| object.pointer == pointer)
            .ok_or_else(Self::invalid_v2)?;
        u64::try_from(object.bytes.len()).map_err(|_| Self::invalid_v2())
    }

    fn read_at(
        &mut self,
        pointer: ZkAmsMkheDirectObjectPointerV1,
        absolute_offset: u64,
        destination: &mut [u8],
    ) -> Result<usize, ZkAmsMkheErrorV1> {
        self.published_reads += 1;
        let start = usize::try_from(absolute_offset).map_err(|_| Self::invalid_v2())?;
        let copied = if self.fault == CasFaultV2::ShortPublishedRead {
            destination.len().saturating_sub(1)
        } else {
            destination.len()
        };
        let object = self
            .object
            .as_mut()
            .filter(|object| object.pointer == pointer)
            .ok_or_else(Self::invalid_v2)?;
        if self.fault == CasFaultV2::MutatePublishedRead {
            *object.bytes.get_mut(start).ok_or_else(Self::invalid_v2)? ^= 0x80;
        }
        let end = start.checked_add(copied).ok_or_else(Self::invalid_v2)?;
        destination[..copied]
            .copy_from_slice(object.bytes.get(start..end).ok_or_else(Self::invalid_v2)?);
        if self.fault == CasFaultV2::DriftPublishedRead {
            self.provider_identity = [0x77; 32];
        }
        Ok(copied)
    }
}

impl ZkAmsMkheDirectObjectCasPublicationV1 for OneObjectCasV2 {
    fn publication_identity(&mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        Ok(PUBLICATION_ID)
    }

    fn begin_staging(
        &mut self,
        kind: ZkAmsMkheDirectObjectKindV1,
        payload_bytes: u64,
    ) -> Result<ZkAmsMkheDirectObjectStagingTokenV1, ZkAmsMkheErrorV1> {
        if let Some(begin_calls) = &self.begin_calls {
            begin_calls.set(begin_calls.get() + 1);
        }
        if self.stage.is_some() || self.seal.is_some() || self.object.is_some() {
            return Err(Self::invalid_v2());
        }
        let token = ZkAmsMkheDirectObjectStagingTokenV1::new(
            PUBLICATION_ID,
            STAGING_ID,
            kind,
            payload_bytes,
        )?;
        self.stage = Some(StageV2 {
            token_digest: token.token_digest(),
            kind,
            bytes: Vec::new(),
            expected: payload_bytes,
        });
        Ok(token)
    }

    fn staged_len(
        &mut self,
        staging: &ZkAmsMkheDirectObjectStagingTokenV1,
    ) -> Result<u64, ZkAmsMkheErrorV1> {
        let stage = self.stage.as_ref().ok_or_else(Self::invalid_v2)?;
        if stage.token_digest != staging.token_digest()
            || stage.kind != staging.kind()
            || stage.expected != staging.payload_bytes()
        {
            return Err(Self::invalid_v2());
        }
        u64::try_from(stage.bytes.len()).map_err(|_| Self::invalid_v2())
    }

    fn write_staged_at(
        &mut self,
        staging: &ZkAmsMkheDirectObjectStagingTokenV1,
        absolute_offset: u64,
        source: &[u8],
    ) -> Result<usize, ZkAmsMkheErrorV1> {
        self.writes += 1;
        if self.fault == CasFaultV2::PanicWrite {
            panic!("tail publication write panic");
        }
        let stage = self.stage.as_mut().ok_or_else(Self::invalid_v2)?;
        if stage.token_digest != staging.token_digest()
            || usize::try_from(absolute_offset).ok() != Some(stage.bytes.len())
        {
            return Err(Self::invalid_v2());
        }
        let copied = if self.fault == CasFaultV2::ShortWrite {
            source.len().saturating_sub(1)
        } else {
            source.len()
        };
        stage.bytes.extend_from_slice(&source[..copied]);
        Ok(copied)
    }

    fn seal_staged(
        &mut self,
        staging: ZkAmsMkheDirectObjectStagingTokenV1,
    ) -> Result<ZkAmsMkheDirectObjectSealTokenV1, ZkAmsMkheErrorV1> {
        let stage = self.stage.take().ok_or_else(Self::invalid_v2)?;
        if stage.token_digest != staging.token_digest()
            || u64::try_from(stage.bytes.len()).ok() != Some(stage.expected)
        {
            return Err(Self::invalid_v2());
        }
        let seal = ZkAmsMkheDirectObjectSealTokenV1::from_staging(staging, SEAL_ID)?;
        self.seal = Some(SealV2 {
            token_digest: seal.token_digest(),
            kind: stage.kind,
            bytes: stage.bytes,
        });
        Ok(seal)
    }

    fn sealed_len(
        &mut self,
        seal: &ZkAmsMkheDirectObjectSealTokenV1,
    ) -> Result<u64, ZkAmsMkheErrorV1> {
        let stored = self.seal.as_ref().ok_or_else(Self::invalid_v2)?;
        if stored.token_digest != seal.token_digest() {
            return Err(Self::invalid_v2());
        }
        u64::try_from(stored.bytes.len()).map_err(|_| Self::invalid_v2())
    }

    fn read_sealed_at(
        &mut self,
        seal: &ZkAmsMkheDirectObjectSealTokenV1,
        absolute_offset: u64,
        destination: &mut [u8],
    ) -> Result<usize, ZkAmsMkheErrorV1> {
        self.sealed_reads += 1;
        let stored = self.seal.as_ref().ok_or_else(Self::invalid_v2)?;
        if stored.token_digest != seal.token_digest() {
            return Err(Self::invalid_v2());
        }
        let start = usize::try_from(absolute_offset).map_err(|_| Self::invalid_v2())?;
        let end = start
            .checked_add(destination.len())
            .ok_or_else(Self::invalid_v2)?;
        destination.copy_from_slice(stored.bytes.get(start..end).ok_or_else(Self::invalid_v2)?);
        Ok(destination.len())
    }

    fn publish_sealed_by_pointer(
        &mut self,
        seal: &ZkAmsMkheDirectObjectSealTokenV1,
        pointer: ZkAmsMkheDirectObjectPointerV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let stored = self.seal.as_ref().ok_or_else(Self::invalid_v2)?;
        if stored.token_digest != seal.token_digest()
            || ZkAmsMkheDirectObjectPointerV1::from_payload(stored.kind, &stored.bytes)? != pointer
        {
            return Err(Self::invalid_v2());
        }
        self.object = Some(ObjectV2 {
            pointer,
            bytes: stored.bytes.clone(),
        });
        Ok(())
    }

    fn lookup_published_pointer(
        &mut self,
        pointer: ZkAmsMkheDirectObjectPointerV1,
    ) -> Result<Option<ZkAmsMkheDirectObjectPublishedBindingV1>, ZkAmsMkheErrorV1> {
        if self
            .object
            .as_ref()
            .is_none_or(|object| object.pointer != pointer)
        {
            return Ok(None);
        }
        Ok(Some(ZkAmsMkheDirectObjectPublishedBindingV1::new(
            PUBLICATION_ID,
            PUBLISHED_ID,
            pointer,
        )?))
    }
}

fn zero_tail_coefficients_v2() -> Vec<u64> {
    vec![0_u64; ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1]
}

#[test]
fn resource_ledger_and_fail_closed_flags_are_exact() {
    let ledger = &RNS_NATIVE_TAIL_PUBLICATION_RESOURCE_LEDGER_V2;
    assert_eq!(
        PUBLICATION_RECEIPT_OWNER_BYTES_V2,
        core::mem::size_of::<ZkAmsMkheDirectObjectPublicationReceiptV1>()
    );
    assert_eq!(
        READ_RECEIPT_OWNER_BYTES_V2,
        core::mem::size_of::<ZkAmsMkheDirectObjectReadReceiptV1>()
    );
    assert_eq!(ledger.tail_objects, 176);
    assert_eq!(ledger.tail_coefficients, 23_068_672);
    assert_eq!(ledger.tail_canonical_bytes, 184_550_080);
    assert_eq!(ledger.tail_coefficient_chunks, 22_528);
    assert_eq!(ledger.tail_publication_writes, 22_704);
    assert_eq!(ledger.tail_transport_operations, 68_112);
    assert_eq!(ledger.tail_authenticated_transfer_bytes, 553_650_240);
    assert_eq!(ledger.tail_work_units, 576_718_912);
    assert_eq!(ledger.tail_pointer_frame_bytes, 13_728);
    assert_eq!(ledger.tail_publication_receipt_bytes, 123_904);
    assert_eq!(ledger.all_publication_receipt_bytes, 2_478_080);
    assert_eq!(ledger.retained_v1_read_receipt_bytes, 1_620_928);
    assert_eq!(ledger.tail_plus_reader_io_bytes, 4_244_651_840);
    assert_eq!(ledger.tail_plus_reader_work_units, 9_349_571_552);
<<<<<<< HEAD
    assert!(RNS_NATIVE_TAIL_CAS_CONTRACT_IMPLEMENTED_V2);
    assert!(RNS_NATIVE_TAIL_LIFECYCLE_CONTRACT_IMPLEMENTED_V2);
    assert!(RNS_NATIVE_WHOLE_V1_OWNER_CONTRACT_IMPLEMENTED_V2);
    assert!(RNS_NATIVE_COMPOSITE_PROVIDER_CONTRACT_IMPLEMENTED_V2);
    assert!(RNS_NATIVE_EXISTING_READER_BRIDGE_CONTRACT_IMPLEMENTED_V2);
    assert!(RNS_NATIVE_SINGLE_QPCS_BATCH_CONTRACT_IMPLEMENTED_V2);
    assert!(RNS_NATIVE_V1_TAIL_COORDINATOR_IMPLEMENTED_V2);
    assert!(RNS_NATIVE_TAIL_V1_CALLBACK_INTEGRATED_V2);
    assert!(!RNS_NATIVE_TAIL_PHASE23_OWNER_AVAILABLE_V2);
    assert!(!RNS_NATIVE_KEY_TAIL_CAS_OWNER_AVAILABLE_V2);
    assert!(!RNS_NATIVE_TAIL_PRODUCTION_OWNER_AVAILABLE_V2);
    assert!(!RNS_NATIVE_TAIL_PRODUCTION_ADAPTER_AVAILABLE_V2);
    assert!(!RNS_NATIVE_COMPOSITE_PROVIDER_INTEGRATED_V2);
    assert!(!RNS_NATIVE_EXISTING_READER_INTEGRATED_V2);
    assert!(!RNS_NATIVE_SINGLE_QPCS_BATCH_INTEGRATED_V2);
    assert!(!RNS_NATIVE_READER_PROVIDER_RETURN_INTEGRATED_V2);
    assert!(!RNS_NATIVE_TAIL_RESOURCE_EVIDENCE_QUALIFIED_V2);
    assert!(!RNS_NATIVE_TAIL_DEVICE_EVIDENCE_QUALIFIED_V2);
    assert!(!RNS_NATIVE_TAIL_READINESS_V2);
    assert!(!RNS_NATIVE_TAIL_RELEASE_AUTHORIZED_V2);
=======
    const {
        assert!(RNS_NATIVE_TAIL_CAS_CONTRACT_IMPLEMENTED_V2);
        assert!(RNS_NATIVE_TAIL_LIFECYCLE_CONTRACT_IMPLEMENTED_V2);
        assert!(RNS_NATIVE_WHOLE_V1_OWNER_CONTRACT_IMPLEMENTED_V2);
        assert!(RNS_NATIVE_COMPOSITE_PROVIDER_CONTRACT_IMPLEMENTED_V2);
        assert!(RNS_NATIVE_EXISTING_READER_BRIDGE_CONTRACT_IMPLEMENTED_V2);
        assert!(RNS_NATIVE_SINGLE_QPCS_BATCH_CONTRACT_IMPLEMENTED_V2);
        assert!(!RNS_NATIVE_TAIL_V1_CALLBACK_INTEGRATED_V2);
        assert!(!RNS_NATIVE_TAIL_PHASE23_OWNER_AVAILABLE_V2);
        assert!(!RNS_NATIVE_KEY_TAIL_CAS_OWNER_AVAILABLE_V2);
        assert!(!RNS_NATIVE_TAIL_PRODUCTION_OWNER_AVAILABLE_V2);
        assert!(!RNS_NATIVE_TAIL_PRODUCTION_ADAPTER_AVAILABLE_V2);
        assert!(!RNS_NATIVE_COMPOSITE_PROVIDER_INTEGRATED_V2);
        assert!(!RNS_NATIVE_EXISTING_READER_INTEGRATED_V2);
        assert!(!RNS_NATIVE_SINGLE_QPCS_BATCH_INTEGRATED_V2);
        assert!(!RNS_NATIVE_READER_PROVIDER_RETURN_INTEGRATED_V2);
        assert!(!RNS_NATIVE_TAIL_RESOURCE_EVIDENCE_QUALIFIED_V2);
        assert!(!RNS_NATIVE_TAIL_DEVICE_EVIDENCE_QUALIFIED_V2);
        assert!(!RNS_NATIVE_TAIL_READINESS_V2);
        assert!(!RNS_NATIVE_TAIL_RELEASE_AUTHORIZED_V2);
    }
>>>>>>> origin/optimizations
    assert_eq!(RNS_NATIVE_TAIL_PUBLICATION_BLOCKERS_V2.len(), 5);
    assert_eq!(
        RNS_NATIVE_TAIL_PUBLICATION_BLOCKERS_V2[0].code,
        "LIVE_CPK_KEY_CAS_OWNER"
    );
}

fn malformed_streaming_authority_v2() -> ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1 {
    ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1 {
        _seal: StreamingCollectiveEncryptionKeyAuthoritySealV1,
        binding: ZkAmsMkheStreamingCollectiveKeyBindingV1 {
            version: 0,
            profile_digest: [0x10; 32],
            security_certificate_digest: [0x20; 32],
            roster_digest: [0x30; 32],
            key_material_digest: [0x40; 32],
            epoch: 1,
            transcript_digest: [0x50; 32],
            parties: core::array::from_fn(|index| {
                ZkAmsMkhePartyIdV1::new([index as u8 + 1; 32]).unwrap()
            }),
            share_digests: [[0x60; 32]; 8],
            key_digest: [0x70; 32],
            public_a_limb_pointers: Vec::new(),
            public_b_limb_pointers: Vec::new(),
            binding_digest: [0x80; 32],
        },
        public_a_publication_receipts: Vec::new(),
        public_b_publication_receipts: Vec::new(),
        authority_digest: [0x90; 32],
        next_sample_index: 0,
        failed: false,
    }
}

#[test]
fn malformed_authority_rejects_before_first_key_tail_cas() {
    let begin_calls = Rc::new(Cell::new(0));
    let result = RnsNativeV1TailPublicationCoordinatorV2::begin_v2(
        malformed_streaming_authority_v2(),
        RnsNativeCollectiveKeyTailOwnerV2::malformed_test_owner_v2(),
        OneObjectCasV2::counted_v2(Rc::clone(&begin_calls)),
        OneObjectCasV2::new_v2(CasFaultV2::None),
    );
    assert!(matches!(
        result,
        Err(RnsNativeV1TailCoordinatorErrorV2::Encryption(_))
    ));
    assert_eq!(begin_calls.get(), 0);
}

#[test]
fn physical_176_order_is_exact_complete_and_bijective() {
    let positions = (0..TAIL_OBJECTS_V2)
        .map(|physical| physical_tail_position_v2(physical).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(positions[0].role_v2(), RnsNativeTailObjectRoleV2::PublicA);
    assert_eq!(positions[0].limb_v2(), 38);
    assert_eq!(positions[1].limb_v2(), 39);
    assert_eq!(
        positions[2].role_v2(),
        RnsNativeTailObjectRoleV2::CollectivePublicB
    );
    assert_eq!(positions[3].limb_v2(), 39);
    assert_eq!(
        positions[4].role_v2(),
        RnsNativeTailObjectRoleV2::CiphertextC0
    );
    assert_eq!(positions[4].record_ordinal_v2(), Some(0));
    assert_eq!(positions[4].limb_v2(), 38);
    assert_eq!(
        positions[6].role_v2(),
        RnsNativeTailObjectRoleV2::CiphertextC1
    );
    assert_eq!(positions[8].record_ordinal_v2(), Some(1));
    assert_eq!(
        positions[175].role_v2(),
        RnsNativeTailObjectRoleV2::CiphertextC1
    );
    assert_eq!(positions[175].record_ordinal_v2(), Some(42));
    assert_eq!(positions[175].limb_v2(), 39);
    let ordinals = positions
        .iter()
        .map(|position| position.tail_ordinal_v2().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(ordinals, (0..TAIL_OBJECTS_V2).collect());
    assert_eq!(
        physical_tail_position_v2(TAIL_OBJECTS_V2),
        Err(RnsNativeTailPublicationErrorV2::InvalidPosition)
    );
}

#[test]
fn one_real_full_size_tail_object_owns_the_actual_receipt() {
    let coefficients = zero_tail_coefficients_v2();
    let position = physical_tail_position_v2(0).unwrap();
    let mut cas = OneObjectCasV2::new_v2(CasFaultV2::None);
    let object = publish_tail_object_v2(position, &coefficients, &mut cas).unwrap();
    object.validate_v2().unwrap();
    assert_eq!(object.position_v2(), position);
    assert_eq!(object.pointer_v2().payload_bytes(), OBJECT_BYTES_V2 as u64);
    assert_eq!(
        object.pointer_v2().kind(),
        ZkAmsMkheDirectObjectKindV1::CollectivePublicA
    );
    assert_ne!(object.encoded_integrity_digest_v2(), [0; 32]);
    let mut encoded_hash = Keccak256::new();
    encoded_hash.update(&cas.object.as_ref().unwrap().bytes);
    assert_eq!(
        object.encoded_integrity_digest_v2(),
        encoded_hash.finalize()
    );
    assert_ne!(object.publication_receipt_v2().receipt_digest(), [0; 32]);
    assert_eq!(cas.writes, WRITES_PER_OBJECT_V2);
    assert_eq!(cas.sealed_reads, 129);
    assert_eq!(cas.published_reads, 129);
}

#[test]
fn real_object_publication_rejects_transport_and_residue_mutations() {
    let coefficients = zero_tail_coefficients_v2();
    let position = physical_tail_position_v2(0).unwrap();
    for fault in [
        CasFaultV2::ShortWrite,
        CasFaultV2::ShortPublishedRead,
        CasFaultV2::MutatePublishedRead,
        CasFaultV2::DriftPublishedRead,
    ] {
        let mut cas = OneObjectCasV2::new_v2(fault);
        assert!(publish_tail_object_v2(position, &coefficients, &mut cas).is_err());
    }
    let mut invalid = coefficients;
    invalid[17] = u64::MAX;
    let mut cas = OneObjectCasV2::new_v2(CasFaultV2::None);
    assert!(matches!(
        publish_tail_object_v2(position, &invalid, &mut cas),
        Err(RnsNativeTailPublicationErrorV2::Basis)
    ));
}

#[test]
fn visitor_poison_and_order_mismatch_are_terminal() {
    let coefficients = zero_tail_coefficients_v2();
    let expected = physical_tail_position_v2(0).unwrap();
    let wrong = physical_tail_position_v2(1).unwrap();
    let mut cas = OneObjectCasV2::new_v2(CasFaultV2::None);
    let mut visitor =
        RnsNativeTailCasVisitorV2::new_v2(&mut cas, vec![expected].into_boxed_slice()).unwrap();
    assert_eq!(
        visitor.visit_tail_coefficients_v2(wrong, &coefficients),
        Err(RnsNativeBasisExtensionErrorV2::VisitorRejected)
    );
    assert_eq!(
        visitor.visit_tail_coefficients_v2(expected, &coefficients),
        Err(RnsNativeBasisExtensionErrorV2::Poisoned)
    );
    assert!(matches!(
        visitor.finish_v2(),
        Err(RnsNativeTailPublicationErrorV2::InvalidOrder)
    ));
}

#[test]
fn caught_backend_unwind_leaves_the_visitor_poisoned() {
    let coefficients = zero_tail_coefficients_v2();
    let expected = physical_tail_position_v2(0).unwrap();
    let mut cas = OneObjectCasV2::new_v2(CasFaultV2::PanicWrite);
    let mut visitor =
        RnsNativeTailCasVisitorV2::new_v2(&mut cas, vec![expected].into_boxed_slice()).unwrap();
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = visitor.visit_tail_coefficients_v2(expected, &coefficients);
    }));
    assert!(unwind.is_err());
    assert_eq!(
        visitor.visit_tail_coefficients_v2(expected, &coefficients),
        Err(RnsNativeBasisExtensionErrorV2::Poisoned)
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RoutingFaultV2 {
    None,
    WrongLength,
    ShortRead,
    DriftAfterRead,
}

struct RoutingProviderV2 {
    identity: [u8; 32],
    snapshot: [u8; 32],
    fault: RoutingFaultV2,
    reads: Rc<Cell<usize>>,
    drops: Rc<Cell<usize>>,
}

impl Drop for RoutingProviderV2 {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

impl ZkAmsMkheDirectObjectReadAtProviderV1 for RoutingProviderV2 {
    fn provider_identity(&mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        Ok(self.identity)
    }

    fn snapshot_identity(&mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        Ok(self.snapshot)
    }

    fn object_len(
        &mut self,
        pointer: ZkAmsMkheDirectObjectPointerV1,
    ) -> Result<u64, ZkAmsMkheErrorV1> {
        if self.fault == RoutingFaultV2::WrongLength {
            Ok(pointer.payload_bytes() - 1)
        } else {
            Ok(pointer.payload_bytes())
        }
    }

    fn read_at(
        &mut self,
        _pointer: ZkAmsMkheDirectObjectPointerV1,
        _absolute_offset: u64,
        destination: &mut [u8],
    ) -> Result<usize, ZkAmsMkheErrorV1> {
        self.reads.set(self.reads.get() + 1);
        destination.fill(0x5a);
        if self.fault == RoutingFaultV2::DriftAfterRead {
            self.identity[0] ^= 1;
        }
        if self.fault == RoutingFaultV2::ShortRead {
            Ok(destination.len() - 1)
        } else {
            Ok(destination.len())
        }
    }
}

fn synthetic_pointer_v2(
    ordinal: usize,
    kind: ZkAmsMkheDirectObjectKindV1,
) -> ZkAmsMkheDirectObjectPointerV1 {
    let mut payload_blake3 = [0_u8; 32];
    payload_blake3[..8].copy_from_slice(&(ordinal as u64 + 1).to_be_bytes());
    payload_blake3[8] = kind as u8;
    ZkAmsMkheDirectObjectPointerV1::new(kind, OBJECT_BYTES_V2 as u64, payload_blake3).unwrap()
}

fn synthetic_allowlist_v2() -> Box<[RnsNativeAllowlistedPointerV2]> {
    let mut entries = Vec::with_capacity(FULL_OBJECTS_V2);
    let mut ordinal = 0;
    for (kind, count, route) in [
        (
            ZkAmsMkheDirectObjectKindV1::CollectivePublicA,
            TARGET_LIMBS_V2,
            RnsNativeCompositeRouteV2::Key,
        ),
        (
            ZkAmsMkheDirectObjectKindV1::CollectivePublicB,
            TARGET_LIMBS_V2,
            RnsNativeCompositeRouteV2::Key,
        ),
        (
            ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC0,
            RECORDS_V2 * TARGET_LIMBS_V2,
            RnsNativeCompositeRouteV2::Ciphertext,
        ),
        (
            ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC1,
            RECORDS_V2 * TARGET_LIMBS_V2,
            RnsNativeCompositeRouteV2::Ciphertext,
        ),
    ] {
        for _ in 0..count {
            entries.push(RnsNativeAllowlistedPointerV2 {
                pointer: synthetic_pointer_v2(ordinal, kind),
                route,
            });
            ordinal += 1;
        }
    }
    entries.into_boxed_slice()
}

fn bounded_allowlist_v2() -> Box<[RnsNativeAllowlistedPointerV2]> {
    [
        (
            ZkAmsMkheDirectObjectKindV1::CollectivePublicA,
            RnsNativeCompositeRouteV2::Key,
        ),
        (
            ZkAmsMkheDirectObjectKindV1::CollectivePublicB,
            RnsNativeCompositeRouteV2::Key,
        ),
        (
            ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC0,
            RnsNativeCompositeRouteV2::Ciphertext,
        ),
        (
            ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC1,
            RnsNativeCompositeRouteV2::Ciphertext,
        ),
    ]
    .into_iter()
    .enumerate()
    .map(|(ordinal, (kind, route))| RnsNativeAllowlistedPointerV2 {
        pointer: synthetic_pointer_v2(ordinal, kind),
        route,
    })
    .collect::<Vec<_>>()
    .into_boxed_slice()
}

fn bounded_composite_v2(
    key: RoutingProviderV2,
    ciphertext: RoutingProviderV2,
    entries: Box<[RnsNativeAllowlistedPointerV2]>,
) -> Result<
    RnsNativeTailCompositeProviderV2<RoutingProviderV2, RoutingProviderV2>,
    RnsNativeTailPublicationErrorV2,
> {
    let expected_pointer_count = entries.len();
    RnsNativeTailCompositeProviderV2::new_bounded_test_v2(
        key,
        ciphertext,
        entries,
        expected_pointer_count,
    )
}

fn routing_provider_v2(
    identity: u8,
    snapshot: u8,
    fault: RoutingFaultV2,
    reads: Rc<Cell<usize>>,
    drops: Rc<Cell<usize>>,
) -> RoutingProviderV2 {
    RoutingProviderV2 {
        identity: [identity; 32],
        snapshot: [snapshot; 32],
        fault,
        reads,
        drops,
    }
}

#[test]
fn composite_provider_routes_only_allowlisted_typed_pointers() {
    let entries = bounded_allowlist_v2();
    let key_pointer = entries[0].pointer;
    let ciphertext_pointer = entries[2].pointer;
    let key_reads = Rc::new(Cell::new(0));
    let ciphertext_reads = Rc::new(Cell::new(0));
    let drops = Rc::new(Cell::new(0));
    let key = routing_provider_v2(
        0x10,
        0x20,
        RoutingFaultV2::None,
        Rc::clone(&key_reads),
        Rc::clone(&drops),
    );
    let ciphertext = routing_provider_v2(
        0x30,
        0x40,
        RoutingFaultV2::None,
        Rc::clone(&ciphertext_reads),
        Rc::clone(&drops),
    );
    let mut composite = bounded_composite_v2(key, ciphertext, entries).unwrap();
    assert_ne!(
        composite.provider_identity().unwrap(),
        composite.snapshot_identity().unwrap()
    );
    assert_eq!(
        composite.object_len(key_pointer).unwrap(),
        OBJECT_BYTES_V2 as u64
    );
    assert_eq!(
        composite.object_len(ciphertext_pointer).unwrap(),
        OBJECT_BYTES_V2 as u64
    );
    let mut bytes = [0_u8; 17];
    assert_eq!(composite.read_at(key_pointer, 9, &mut bytes).unwrap(), 17);
    assert_eq!(key_reads.get(), 1);
    assert_eq!(ciphertext_reads.get(), 0);
    assert_eq!(
        composite
            .read_at(ciphertext_pointer, 11, &mut bytes)
            .unwrap(),
        17
    );
    assert_eq!(ciphertext_reads.get(), 1);
    let unknown = synthetic_pointer_v2(
        FULL_OBJECTS_V2 + 9,
        ZkAmsMkheDirectObjectKindV1::CollectivePublicA,
    );
    assert!(composite.object_len(unknown).is_err());
    drop(composite);
    assert_eq!(drops.get(), 2);
}

#[test]
fn composite_provider_rejects_wrong_route_duplicates_length_short_read_and_drift() {
    let drops = Rc::new(Cell::new(0));
    let reads = Rc::new(Cell::new(0));
    let mut wrong_route = bounded_allowlist_v2();
    wrong_route[0].route = RnsNativeCompositeRouteV2::Ciphertext;
    let result = bounded_composite_v2(
        routing_provider_v2(
            1,
            2,
            RoutingFaultV2::None,
            Rc::clone(&reads),
            Rc::clone(&drops),
        ),
        routing_provider_v2(
            3,
            4,
            RoutingFaultV2::None,
            Rc::clone(&reads),
            Rc::clone(&drops),
        ),
        wrong_route,
    );
    assert!(result.is_err());
    assert_eq!(drops.get(), 2);

    let mut duplicate = bounded_allowlist_v2();
    duplicate[1] = duplicate[0];
    let result = bounded_composite_v2(
        routing_provider_v2(
            5,
            6,
            RoutingFaultV2::None,
            Rc::clone(&reads),
            Rc::clone(&drops),
        ),
        routing_provider_v2(
            7,
            8,
            RoutingFaultV2::None,
            Rc::clone(&reads),
            Rc::clone(&drops),
        ),
        duplicate,
    );
    assert!(result.is_err());
    assert_eq!(drops.get(), 4);

    let mut length_mutation = bounded_allowlist_v2();
    let original = length_mutation[0].pointer;
    length_mutation[0].pointer = ZkAmsMkheDirectObjectPointerV1::new(
        original.kind(),
        (OBJECT_BYTES_V2 - 1) as u64,
        original.payload_blake3(),
    )
    .unwrap();
    let result = bounded_composite_v2(
        routing_provider_v2(
            13,
            14,
            RoutingFaultV2::None,
            Rc::clone(&reads),
            Rc::clone(&drops),
        ),
        routing_provider_v2(
            15,
            16,
            RoutingFaultV2::None,
            Rc::clone(&reads),
            Rc::clone(&drops),
        ),
        length_mutation,
    );
    assert!(matches!(
        result,
        Err(RnsNativeTailPublicationErrorV2::LengthMutation)
    ));
    assert_eq!(drops.get(), 6);

    for (fault, use_key) in [
        (RoutingFaultV2::WrongLength, true),
        (RoutingFaultV2::ShortRead, false),
        (RoutingFaultV2::DriftAfterRead, true),
    ] {
        let entries = bounded_allowlist_v2();
        let pointer = if use_key {
            entries[0].pointer
        } else {
            entries[2].pointer
        };
        let key_fault = if use_key { fault } else { RoutingFaultV2::None };
        let ciphertext_fault = if use_key { RoutingFaultV2::None } else { fault };
        let mut composite = bounded_composite_v2(
            routing_provider_v2(9, 10, key_fault, Rc::clone(&reads), Rc::clone(&drops)),
            routing_provider_v2(
                11,
                12,
                ciphertext_fault,
                Rc::clone(&reads),
                Rc::clone(&drops),
            ),
            entries,
        )
        .unwrap();
        if fault == RoutingFaultV2::WrongLength {
            assert!(composite.object_len(pointer).is_err());
        } else {
            let mut destination = [0_u8; 8];
            assert!(composite.read_at(pointer, 0, &mut destination).is_err());
        }
    }
    assert_eq!(drops.get(), 12);
}

#[test]
fn source_has_no_live_authority_shortcut() {
    let source = include_str!("incremental_source_rns_native_tail_publication_v2.rs");
    assert!(!source.contains("cfg(any())"));
    assert!(!source.contains("SOURCE_ADAPTER_AVAILABLE_V2: bool = true"));
    assert!(!source.contains("PRODUCTION_OWNER_AVAILABLE_V2: bool = true"));
    assert!(!source.contains("RESOURCE_EVIDENCE_QUALIFIED_V2: bool = true"));
    assert!(!source.contains("READINESS_V2: bool = true"));
    assert!(!source.contains("RELEASE_AUTHORIZED_V2: bool = true"));
    assert!(source.contains("RnsNativeV1TailPublicationCoordinatorV2"));
    assert!(!source.contains("RnsNativeWholeV1ProductionAdapterV2"));
    assert!(!source.contains("never: Infallible"));
    assert!(source.contains("RnsNativePublicPolynomialDescriptorV1::new"));
    assert!(source.contains("RnsNativePublicPolynomialManifestV1::new"));
    assert!(source.contains("RnsNativePublicPolynomialReaderV1::new"));
}

#[test]
fn pretranscript_child_is_private_once_and_every_live_gate_stays_closed() {
    let parent = include_str!("incremental_source_rns_native_tail_publication_v2.rs");
    let path =
        "incremental_source_rns_native_tail_publication_v2/pretranscript_public_statement_v2.rs";
    let declaration = "mod pretranscript_public_statement_v2;";
    assert_eq!(parent.matches(path).count(), 1);
    assert_eq!(parent.matches(declaration).count(), 1);
    assert!(!parent.contains("pub mod pretranscript_public_statement_v2;"));
    assert!(!parent.contains("pub(crate) mod pretranscript_public_statement_v2;"));
    assert!(!parent.contains("pub(super) mod pretranscript_public_statement_v2;"));

    let child = include_str!(
        "incremental_source_rns_native_tail_publication_v2/pretranscript_public_statement_v2.rs"
    );
    for closed in [
        "LIVE_CORRESPONDENCE_AVAILABLE_V2: bool = false",
        "REPEAT_READ_CONFORMANCE_QUALIFIED_V2: bool = false",
        "SOURCE_PREFLIGHT_INTEGRATED_V2: bool = false",
        "RESOURCE_EVIDENCE_QUALIFIED_V2: bool = false",
        "READINESS_V2: bool = false",
        "RELEASE_AUTHORIZED_V2: bool = false",
    ] {
        assert!(child.contains(closed), "missing closed gate: {closed}");
    }
    assert!(child.contains("never: Infallible"));
    assert!(child.contains("bridge: RnsNativeExistingReaderBridgeV2<K, P>,"));
    assert!(!child.contains("pub(super) struct RnsNativeWholePublicationOwnersV2"));
}

#[test]
fn parent_declaration_is_exactly_once_and_private() {
    let parent = include_str!("incremental_source.rs");
    let name = "incremental_source_rns_native_tail_publication_v2";
    let mentions = parent
        .lines()
        .filter(|line| line.contains(name))
        .collect::<Vec<_>>();
    assert_eq!(
        mentions.as_slice(),
        &[
            "#[path = \"incremental_source_rns_native_tail_publication_v2.rs\"]",
            "mod incremental_source_rns_native_tail_publication_v2;",
            "pub(in crate::vega::zk_ams::mkhe) use incremental_source_rns_native_tail_publication_v2::RnsNativeClaimedDirectNumericOriginV2;",
        ]
    );
    assert_eq!(
        parent
            .matches("mod incremental_source_rns_native_tail_publication_v2;")
            .count(),
        1
    );
    assert!(!parent.contains("pub mod incremental_source_rns_native_tail_publication_v2"));
    assert!(!parent.contains("pub(crate) mod incremental_source_rns_native_tail_publication_v2"));
    assert!(!parent.contains("pub(super) mod incremental_source_rns_native_tail_publication_v2"));
    assert_eq!(
        parent
            .matches("use incremental_source_rns_native_tail_publication_v2")
            .count(),
        1
    );
    assert!(!parent.contains("pub(crate) use incremental_source_rns_native_tail_publication_v2"));
    assert!(!parent.contains("pub use incremental_source_rns_native_tail_publication_v2"));
}

#[test]
fn paired_whole_owner_shape_and_sample_terminal_are_source_pinned() {
    let source = include_str!("incremental_source_rns_native_tail_publication_v2.rs");
    let ordered = [
        "pub(super) struct RnsNativeWholeKeyAndTailPublicationOwnerV2 {",
        "key_authority: ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1,",
        "key_tail: RnsNativePublishedCollectiveKeyTailOwnerV2,",
        "tail_objects: Box<[RnsNativePublishedTailObjectV2]>,",
        "pub(super) struct RnsNativeWholeRecordAndTailPublicationOwnerV2 {",
        "record_ordinal: u8,",
        "ciphertext: ZkAmsMkheStreamingCollectiveCiphertextV1,",
        "tail: RnsNativePublishedTailRecordV2,",
        "pub(super) struct RnsNativeWholePublicationOwnersV2 {",
        "key: RnsNativeWholeKeyAndTailPublicationOwnerV2,",
        "aggregate: RnsNativeCiphertextTailAggregateChecksumV2,",
        "records: Box<[RnsNativeWholeRecordAndTailPublicationOwnerV2]>,",
        "pub(super) struct RnsNativeCompletedQpcsSourceReadV2 {",
        "owners: RnsNativeWholePublicationOwnersV2,",
        "schedule: RnsNativeQpcsRelationScheduleV1,",
        "evaluations: Box<[RnsNativePublicPolynomialEvaluationV1]>,",
        "read_receipt: RnsNativePublicPolynomialReadReceiptV1,",
    ];
    let mut cursor = 0;
    for fragment in ordered {
        let relative = source[cursor..]
            .find(fragment)
            .unwrap_or_else(|| panic!("missing ordered owner fragment: {fragment}"));
        cursor += relative + fragment.len();
    }
    assert!(source.contains("assert!(KEY_TAIL_OBJECTS_V2 == 4);"));
    assert!(source.contains("assert!(RECORDS_V2 == 43);"));
    assert!(source.contains("assert!(TAIL_OBJECTS_PER_RECORD_V2 == 4);"));
    assert!(source.contains("self.key.key_authority.next_sample_index() != RECORDS_V2 as u64"));
    assert!(source.contains("self.key.tail_objects.len() != KEY_TAIL_OBJECTS_V2"));
    assert!(source.contains("self.records.len() != RECORDS_V2"));
    assert!(source.contains("owner.ciphertext.sample_index() != record as u64"));
    assert!(source.contains("owner.tail.objects.len() != TAIL_OBJECTS_PER_RECORD_V2"));
}

#[test]
fn compiled_v1_tail_coordinator_pins_one_owner_one_publisher_and_exact_order() {
    let source = include_str!("incremental_source_rns_native_tail_publication_v2.rs");
    let coordinator = source
        .split_once("pub(super) struct RnsNativeV1TailPublicationCoordinatorV2")
        .and_then(|(_, suffix)| {
            suffix
                .split_once("/// Exact future production shape")
                .map(|(coordinator, _)| coordinator)
        })
        .expect("compiled V1/tail coordinator boundary");
    for required in [
        "authority: ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1",
        "tails: RnsNativeTailPublicationLifecycleV2<K, P>",
        "records: Vec<RnsNativeWholeV1RecordPublicationOwnerV2>",
        "pub(super) fn begin_v2(",
        "pub(super) fn encrypt_next_with_confidential_sink_v2",
        "pub(super) fn finish_v2(",
        "records.try_reserve_exact(RECORDS_V2)",
        "validate_release_v1()",
        "validate_v1_authority_binding_v2(&authority)",
        "RnsNativeCiphertextTailWorkspaceOwnerV2::allocate_workspace_v2()",
        "RnsNativePreparedTailRecordPublicationV2::new_before_entropy_v2(",
        "publish_next_record_from_v1_callback_parts_v2(",
        "RnsNativeWholeV1PublicationOwnersV2 {",
        ".into_existing_reader_bridge_v2(v1)",
    ] {
        assert!(
            coordinator.contains(required),
            "missing coordinator pin: {required}"
        );
    }
    let validate_authority = coordinator.find("validate_release_v1()").unwrap();
    let validate_tail_axes = coordinator
        .find("validate_v1_authority_binding_v2(&authority)")
        .unwrap();
    let begin_tail_cas = coordinator
        .find("RnsNativeTailPublicationLifecycleV2::begin_v2(")
        .unwrap();
    assert!(validate_authority < validate_tail_axes);
    assert!(validate_tail_axes < begin_tail_cas);
    let poison = coordinator.find("self.poisoned = true;").unwrap();
    let workspace = coordinator
        .find("RnsNativeCiphertextTailWorkspaceOwnerV2::allocate_workspace_v2()")
        .unwrap();
    let prepared_publication = coordinator
        .find("RnsNativePreparedTailRecordPublicationV2::new_before_entropy_v2(")
        .unwrap();
    let prepare_sink = coordinator.find("prepare_sink()").unwrap();
    let sink = coordinator.find("if let Err(error) = sink(").unwrap();
    let tail = coordinator
        .find("publish_next_record_from_v1_callback_parts_v2(")
        .unwrap();
    let manifest = coordinator
        .find("self.records.push(RnsNativeWholeV1RecordPublicationOwnerV2")
        .unwrap();
    let clear = coordinator.rfind("self.poisoned = false;").unwrap();
    assert!(poison < workspace);
    assert!(workspace < prepared_publication);
    assert!(prepared_publication < prepare_sink);
    assert!(prepare_sink < sink);
    assert!(sink < tail);
    assert!(tail < manifest);
    assert!(manifest < clear);
    assert_eq!(coordinator.matches("self.poisoned = false;").count(), 1);
    assert_eq!(coordinator.matches("ciphertext_publisher: P").count(), 1);
    assert!(!coordinator.contains("impl Clone for RnsNativeV1TailPublicationCoordinatorV2"));
    assert!(!coordinator.contains("impl Copy for RnsNativeV1TailPublicationCoordinatorV2"));
    assert!(!coordinator.contains("into_parts"));
    assert!(!coordinator.contains("authority(&self)"));
    assert!(!coordinator.contains("provider(&self)"));

    let callback_parts = source
        .split_once("fn publish_next_record_from_v1_callback_parts_v2")
        .and_then(|(_, suffix)| {
            suffix
                .split_once("impl<K, P> RnsNativeTailPublicationLifecycleV2")
                .map(|(callback, _)| callback)
        })
        .expect("allocation-free callback parts");
    assert!(callback_parts.contains("prepared_publication.expected"));
    assert!(callback_parts.contains("prepared_publication.objects"));
    assert!(!callback_parts.contains("Vec::new"));
    assert!(!callback_parts.contains("try_reserve"));
    assert!(!callback_parts.contains(".collect::<"));
}

#[test]
fn coordinator_preserves_callback_error_origin_and_every_live_gate_remains_closed() {
    let source = include_str!("incremental_source_rns_native_tail_publication_v2.rs");
    for required in [
        "ConfidentialSink(ZkAmsMkheErrorV1)",
        "Tail(RnsNativeTailPublicationErrorV2)",
        "RnsNativeV1TailCoordinatorErrorV2::Encryption(encryption_error)",
        "RnsNativeV1TailCoordinatorErrorV2::ConfidentialSink(error)",
        "RnsNativeV1TailCoordinatorErrorV2::Tail(error)",
        "RNS_NATIVE_V1_TAIL_COORDINATOR_IMPLEMENTED_V2: bool = true",
        "RNS_NATIVE_TAIL_V1_CALLBACK_INTEGRATED_V2: bool = true",
    ] {
        assert!(
            source.contains(required),
            "missing error/contract pin: {required}"
        );
    }
    for closed in [
        "RNS_NATIVE_TAIL_PHASE23_OWNER_AVAILABLE_V2: bool = false",
        "RNS_NATIVE_KEY_TAIL_CAS_OWNER_AVAILABLE_V2: bool = false",
        "RNS_NATIVE_TAIL_PRODUCTION_OWNER_AVAILABLE_V2: bool = false",
        "RNS_NATIVE_TAIL_PRODUCTION_ADAPTER_AVAILABLE_V2: bool = false",
        "RNS_NATIVE_COMPOSITE_PROVIDER_INTEGRATED_V2: bool = false",
        "RNS_NATIVE_EXISTING_READER_INTEGRATED_V2: bool = false",
        "RNS_NATIVE_SINGLE_QPCS_BATCH_INTEGRATED_V2: bool = false",
        "RNS_NATIVE_READER_PROVIDER_RETURN_INTEGRATED_V2: bool = false",
        "RNS_NATIVE_TAIL_RESOURCE_EVIDENCE_QUALIFIED_V2: bool = false",
        "RNS_NATIVE_TAIL_DEVICE_EVIDENCE_QUALIFIED_V2: bool = false",
        "RNS_NATIVE_TAIL_READINESS_V2: bool = false",
        "RNS_NATIVE_TAIL_RELEASE_AUTHORIZED_V2: bool = false",
    ] {
        assert!(source.contains(closed), "opened live gate: {closed}");
    }
    assert!(source.contains("code: \"LIVE_PHASE23_CONFIDENTIAL_SINK\""));
    assert!(!source.contains("RnsNativeWholeV1ProductionAdapterV2"));
    assert!(!source.contains("pub(super) fn from_raw"));
}

#[test]
#[ignore = "manual 176-object transport gate; allocates and hashes one 1,048,580-byte object per iteration"]
fn full_176_tail_transport_gate_is_manual() {
    let coefficients = zero_tail_coefficients_v2();
    for physical in 0..TAIL_OBJECTS_V2 {
        let position = physical_tail_position_v2(physical).unwrap();
        let mut cas = OneObjectCasV2::new_v2(CasFaultV2::None);
        publish_tail_object_v2(position, &coefficients, &mut cas).unwrap();
    }
}

#[test]
#[ignore = "manual full 3,520-pointer allowlist/provider gate"]
fn full_3520_composite_provider_gate_is_manual() {
    let entries = synthetic_allowlist_v2();
    let reads = Rc::new(Cell::new(0));
    let drops = Rc::new(Cell::new(0));
    let composite = RnsNativeTailCompositeProviderV2::new_v2(
        routing_provider_v2(
            0x81,
            0x82,
            RoutingFaultV2::None,
            Rc::clone(&reads),
            Rc::clone(&drops),
        ),
        routing_provider_v2(
            0x83,
            0x84,
            RoutingFaultV2::None,
            Rc::clone(&reads),
            Rc::clone(&drops),
        ),
        entries,
    )
    .unwrap();
    assert_eq!(composite.allowlist.len(), FULL_OBJECTS_V2);
}
