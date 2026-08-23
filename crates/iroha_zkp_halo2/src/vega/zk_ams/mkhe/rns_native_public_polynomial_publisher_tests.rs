use super::super::{
    ZkAmsMkheErrorV1,
    direct_object_transport::{
        ZkAmsMkheDirectObjectPublishedBindingV1, ZkAmsMkheDirectObjectSealTokenV1,
        ZkAmsMkheDirectObjectStagingTokenV1, validate_zk_ams_mkhe_direct_object_v1,
    },
};
use super::*;

const FIXTURE_SOURCE_ID_V1: [u8; 32] = [0x41; 32];
const FIXTURE_DRIFTED_SOURCE_ID_V1: [u8; 32] = [0x42; 32];
const FIXTURE_PUBLICATION_ID_V1: [u8; 32] = [0x51; 32];
const FIXTURE_PROVIDER_ID_V1: [u8; 32] = [0x61; 32];
const FIXTURE_DRIFTED_PROVIDER_ID_V1: [u8; 32] = [0x62; 32];
const FIXTURE_SNAPSHOT_ID_V1: [u8; 32] = [0x71; 32];
const FIXTURE_DRIFTED_SNAPSHOT_ID_V1: [u8; 32] = [0x72; 32];
const FIXTURE_STAGING_ID_V1: [u8; 32] = [0x81; 32];
const FIXTURE_SEAL_ID_V1: [u8; 32] = [0x91; 32];
const FIXTURE_PUBLISHED_ID_V1: [u8; 32] = [0xa1; 32];

const _: () = {
    assert!(AUTHENTICATED_TRANSFER_BYTES_V1 < ZK_AMS_MKHE_RNS_NATIVE_IO_MAX_BYTES_V1);
    assert!(COARSE_WORK_UNITS_V1 < ZK_AMS_MKHE_RNS_NATIVE_WORK_MAX_V1);
};

const _: () = {
    assert!(RNS_NATIVE_PUBLIC_POLYNOMIAL_PUBLISHER_DECLARED_V1);
    assert!(!RNS_NATIVE_PUBLIC_POLYNOMIAL_UPSTREAM_40_LIMB_OWNER_AVAILABLE_V1);
    assert!(!RNS_NATIVE_PUBLIC_POLYNOMIAL_PRODUCTION_ADAPTER_INHABITED_V1);
    assert!(!RNS_NATIVE_PUBLIC_POLYNOMIAL_PUBLISHER_INTEGRATED_V1);
    assert!(!RNS_NATIVE_PUBLIC_POLYNOMIAL_PUBLISHER_READINESS_V1);
    assert!(!RNS_NATIVE_PUBLIC_POLYNOMIAL_PUBLISHER_RELEASE_GATE_V1);
    assert!(!RNS_NATIVE_PUBLIC_POLYNOMIAL_LATER_READER_RESOURCE_EVIDENCE_INCLUDED_V1);
    assert!(!RNS_NATIVE_PUBLIC_POLYNOMIAL_MEASURED_RSS_EVIDENCE_INCLUDED_V1);
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FixtureSourceFaultV1 {
    None,
    LeaveOneUnfilled,
    PanicFill,
}

struct FixtureCoefficientSourceV1 {
    identity_calls: usize,
    drift_at_identity_call: Option<usize>,
    next_chunk: usize,
    expected_chunks: usize,
    expected_position: RnsNativePublicPolynomialPositionV1,
    fault: FixtureSourceFaultV1,
}

impl FixtureCoefficientSourceV1 {
    fn new_v1(
        position: RnsNativePublicPolynomialPositionV1,
        chunks: usize,
        fault: FixtureSourceFaultV1,
    ) -> Self {
        Self {
            identity_calls: 0,
            drift_at_identity_call: None,
            next_chunk: 0,
            expected_chunks: chunks,
            expected_position: position,
            fault,
        }
    }
}

impl RnsNativePublicPolynomialCoefficientSourceV1 for FixtureCoefficientSourceV1 {
    fn source_identity_v1(
        &mut self,
    ) -> Result<[u8; DIGEST_BYTES_V1], RnsNativePublicPolynomialPublisherErrorV1> {
        self.identity_calls += 1;
        if self
            .drift_at_identity_call
            .is_some_and(|call| self.identity_calls >= call)
        {
            Ok(FIXTURE_DRIFTED_SOURCE_ID_V1)
        } else {
            Ok(FIXTURE_SOURCE_ID_V1)
        }
    }

    fn fill_next_chunk_v1(
        &mut self,
        request: RnsNativePublicPolynomialChunkRequestV1,
        destination: &mut [u64; COEFFICIENTS_PER_CHUNK_V1],
    ) -> Result<(), RnsNativePublicPolynomialPublisherErrorV1> {
        if self.fault == FixtureSourceFaultV1::PanicFill {
            panic!("fixture source fill panic");
        }
        if request.position_v1() != self.expected_position
            || usize::from(request.chunk_v1()) != self.next_chunk
            || self.next_chunk >= self.expected_chunks
        {
            return Err(RnsNativePublicPolynomialPublisherErrorV1::InvalidOrder);
        }
        let leave = if self.fault == FixtureSourceFaultV1::LeaveOneUnfilled {
            Some(517)
        } else {
            None
        };
        for (index, coefficient) in destination.iter_mut().enumerate() {
            if leave == Some(index) {
                continue;
            }
            let absolute = usize::try_from(request.first_coefficient_v1()).unwrap() + index;
            *coefficient = (absolute as u64 + 17) % request.position_v1().modulus_v1();
        }
        self.next_chunk += 1;
        Ok(())
    }

    fn finish_source_v1(
        self,
    ) -> Result<RnsNativePublicPolynomialSourceTerminalV1, RnsNativePublicPolynomialPublisherErrorV1>
    {
        // A fixture prefix must never mint the production 3,520-object source
        // terminal, even after its own compact traversal succeeds.
        Err(RnsNativePublicPolynomialPublisherErrorV1::Incomplete)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FixtureCasFaultV1 {
    None,
    BeginError,
    BeginPanic,
    WriteError,
    WritePanic,
    SealError,
    SealPanic,
    SealedReadError,
    SealedReadPanic,
    SealedReadMutation,
    PublishBeforeError,
    PublishBeforePanic,
    PublishAfterError,
    PublishAfterPanic,
    LookupError,
    LookupWrongBinding,
    ReadbackError,
    ReadbackPanic,
    ReadbackShort,
    ReadbackMutation,
    ProviderDriftAfterRead,
    SnapshotDriftAfterRead,
    ObjectLengthMutation,
}

struct FixtureStageV1 {
    staging_identity: [u8; 32],
    token_digest: [u8; 32],
    kind: ZkAmsMkheDirectObjectKindV1,
    expected_bytes: u64,
    bytes: Vec<u8>,
}

struct FixtureSealV1 {
    seal_identity: [u8; 32],
    token_digest: [u8; 32],
    kind: ZkAmsMkheDirectObjectKindV1,
    expected_bytes: u64,
    bytes: Vec<u8>,
}

struct FixtureObjectV1 {
    pointer: ZkAmsMkheDirectObjectPointerV1,
    bytes: Vec<u8>,
}

struct FixtureCasV1 {
    fault: FixtureCasFaultV1,
    publication_identity: [u8; 32],
    provider_identity: [u8; 32],
    snapshot_identity: [u8; 32],
    stage: Option<FixtureStageV1>,
    seal: Option<FixtureSealV1>,
    object: Option<FixtureObjectV1>,
    begin_calls: usize,
    write_calls: usize,
    sealed_read_calls: usize,
    publish_calls: usize,
    provider_read_calls: usize,
}

impl FixtureCasV1 {
    const fn new_v1(fault: FixtureCasFaultV1) -> Self {
        Self {
            fault,
            publication_identity: FIXTURE_PUBLICATION_ID_V1,
            provider_identity: FIXTURE_PROVIDER_ID_V1,
            snapshot_identity: FIXTURE_SNAPSHOT_ID_V1,
            stage: None,
            seal: None,
            object: None,
            begin_calls: 0,
            write_calls: 0,
            sealed_read_calls: 0,
            publish_calls: 0,
            provider_read_calls: 0,
        }
    }

    fn invalid_v1() -> ZkAmsMkheErrorV1 {
        ZkAmsMkheErrorV1::InvalidKeyMaterial
    }

    fn object_v1(
        &self,
        pointer: ZkAmsMkheDirectObjectPointerV1,
    ) -> Result<&FixtureObjectV1, ZkAmsMkheErrorV1> {
        self.object
            .as_ref()
            .filter(|object| object.pointer == pointer)
            .ok_or_else(Self::invalid_v1)
    }
}

impl ZkAmsMkheDirectObjectReadAtProviderV1 for FixtureCasV1 {
    fn provider_identity(&mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        Ok(self.provider_identity)
    }

    fn snapshot_identity(&mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        Ok(self.snapshot_identity)
    }

    fn object_len(
        &mut self,
        pointer: ZkAmsMkheDirectObjectPointerV1,
    ) -> Result<u64, ZkAmsMkheErrorV1> {
        let length =
            u64::try_from(self.object_v1(pointer)?.bytes.len()).map_err(|_| Self::invalid_v1())?;
        if self.fault == FixtureCasFaultV1::ObjectLengthMutation {
            length.checked_add(1).ok_or_else(Self::invalid_v1)
        } else {
            Ok(length)
        }
    }

    fn read_at(
        &mut self,
        pointer: ZkAmsMkheDirectObjectPointerV1,
        absolute_offset: u64,
        destination: &mut [u8],
    ) -> Result<usize, ZkAmsMkheErrorV1> {
        self.provider_read_calls += 1;
        match self.fault {
            FixtureCasFaultV1::ReadbackError => return Err(Self::invalid_v1()),
            FixtureCasFaultV1::ReadbackPanic => panic!("fixture provider read panic"),
            _ => {}
        }
        let start = usize::try_from(absolute_offset).map_err(|_| Self::invalid_v1())?;
        let copied = if self.fault == FixtureCasFaultV1::ReadbackShort {
            destination.len().saturating_sub(1)
        } else {
            destination.len()
        };
        if self.fault == FixtureCasFaultV1::ReadbackMutation {
            let object = self
                .object
                .as_mut()
                .filter(|object| object.pointer == pointer)
                .ok_or_else(Self::invalid_v1)?;
            let byte = object.bytes.get_mut(start).ok_or_else(Self::invalid_v1)?;
            *byte ^= 0x80;
        }
        let end = start.checked_add(copied).ok_or_else(Self::invalid_v1)?;
        let source = self
            .object_v1(pointer)?
            .bytes
            .get(start..end)
            .ok_or_else(Self::invalid_v1)?;
        destination[..copied].copy_from_slice(source);
        if self.fault == FixtureCasFaultV1::ProviderDriftAfterRead {
            self.provider_identity = FIXTURE_DRIFTED_PROVIDER_ID_V1;
        }
        if self.fault == FixtureCasFaultV1::SnapshotDriftAfterRead {
            self.snapshot_identity = FIXTURE_DRIFTED_SNAPSHOT_ID_V1;
        }
        Ok(copied)
    }
}

impl ZkAmsMkheDirectObjectCasPublicationV1 for FixtureCasV1 {
    fn publication_identity(&mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        Ok(self.publication_identity)
    }

    fn begin_staging(
        &mut self,
        kind: ZkAmsMkheDirectObjectKindV1,
        payload_bytes: u64,
    ) -> Result<ZkAmsMkheDirectObjectStagingTokenV1, ZkAmsMkheErrorV1> {
        self.begin_calls += 1;
        match self.fault {
            FixtureCasFaultV1::BeginError => return Err(Self::invalid_v1()),
            FixtureCasFaultV1::BeginPanic => panic!("fixture begin panic"),
            _ => {}
        }
        if self.stage.is_some() || payload_bytes == 0 {
            return Err(Self::invalid_v1());
        }
        let token = ZkAmsMkheDirectObjectStagingTokenV1::new(
            self.publication_identity,
            FIXTURE_STAGING_ID_V1,
            kind,
            payload_bytes,
        )?;
        self.stage = Some(FixtureStageV1 {
            staging_identity: token.staging_identity(),
            token_digest: token.token_digest(),
            kind,
            expected_bytes: payload_bytes,
            bytes: Vec::new(),
        });
        Ok(token)
    }

    fn staged_len(
        &mut self,
        staging: &ZkAmsMkheDirectObjectStagingTokenV1,
    ) -> Result<u64, ZkAmsMkheErrorV1> {
        let stage = self.stage.as_ref().ok_or_else(Self::invalid_v1)?;
        if stage.staging_identity != staging.staging_identity()
            || stage.token_digest != staging.token_digest()
            || stage.kind != staging.kind()
            || stage.expected_bytes != staging.payload_bytes()
        {
            return Err(Self::invalid_v1());
        }
        u64::try_from(stage.bytes.len()).map_err(|_| Self::invalid_v1())
    }

    fn write_staged_at(
        &mut self,
        staging: &ZkAmsMkheDirectObjectStagingTokenV1,
        absolute_offset: u64,
        source: &[u8],
    ) -> Result<usize, ZkAmsMkheErrorV1> {
        self.write_calls += 1;
        match self.fault {
            FixtureCasFaultV1::WriteError => return Err(Self::invalid_v1()),
            FixtureCasFaultV1::WritePanic => panic!("fixture write panic"),
            _ => {}
        }
        let stage = self.stage.as_mut().ok_or_else(Self::invalid_v1)?;
        if stage.staging_identity != staging.staging_identity()
            || stage.token_digest != staging.token_digest()
            || usize::try_from(absolute_offset).ok() != Some(stage.bytes.len())
            || source.is_empty()
            || source.len() > ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1
        {
            return Err(Self::invalid_v1());
        }
        let after = stage
            .bytes
            .len()
            .checked_add(source.len())
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or_else(Self::invalid_v1)?;
        if after > stage.expected_bytes {
            return Err(Self::invalid_v1());
        }
        stage.bytes.extend_from_slice(source);
        Ok(source.len())
    }

    fn seal_staged(
        &mut self,
        staging: ZkAmsMkheDirectObjectStagingTokenV1,
    ) -> Result<ZkAmsMkheDirectObjectSealTokenV1, ZkAmsMkheErrorV1> {
        match self.fault {
            FixtureCasFaultV1::SealError => return Err(Self::invalid_v1()),
            FixtureCasFaultV1::SealPanic => panic!("fixture seal panic"),
            _ => {}
        }
        let stage = self.stage.take().ok_or_else(Self::invalid_v1)?;
        if stage.staging_identity != staging.staging_identity()
            || stage.token_digest != staging.token_digest()
            || stage.kind != staging.kind()
            || stage.expected_bytes != staging.payload_bytes()
            || u64::try_from(stage.bytes.len()).ok() != Some(stage.expected_bytes)
        {
            return Err(Self::invalid_v1());
        }
        let seal = ZkAmsMkheDirectObjectSealTokenV1::from_staging(staging, FIXTURE_SEAL_ID_V1)?;
        self.seal = Some(FixtureSealV1 {
            seal_identity: seal.seal_identity(),
            token_digest: seal.token_digest(),
            kind: seal.kind(),
            expected_bytes: seal.payload_bytes(),
            bytes: stage.bytes,
        });
        Ok(seal)
    }

    fn sealed_len(
        &mut self,
        seal: &ZkAmsMkheDirectObjectSealTokenV1,
    ) -> Result<u64, ZkAmsMkheErrorV1> {
        let stored = self.seal.as_ref().ok_or_else(Self::invalid_v1)?;
        if stored.seal_identity != seal.seal_identity()
            || stored.token_digest != seal.token_digest()
            || stored.kind != seal.kind()
            || stored.expected_bytes != seal.payload_bytes()
        {
            return Err(Self::invalid_v1());
        }
        u64::try_from(stored.bytes.len()).map_err(|_| Self::invalid_v1())
    }

    fn read_sealed_at(
        &mut self,
        seal: &ZkAmsMkheDirectObjectSealTokenV1,
        absolute_offset: u64,
        destination: &mut [u8],
    ) -> Result<usize, ZkAmsMkheErrorV1> {
        self.sealed_read_calls += 1;
        match self.fault {
            FixtureCasFaultV1::SealedReadError => return Err(Self::invalid_v1()),
            FixtureCasFaultV1::SealedReadPanic => panic!("fixture sealed read panic"),
            _ => {}
        }
        let stored = self.seal.as_mut().ok_or_else(Self::invalid_v1)?;
        if stored.seal_identity != seal.seal_identity()
            || stored.token_digest != seal.token_digest()
        {
            return Err(Self::invalid_v1());
        }
        let start = usize::try_from(absolute_offset).map_err(|_| Self::invalid_v1())?;
        if self.fault == FixtureCasFaultV1::SealedReadMutation {
            let byte = stored.bytes.get_mut(start).ok_or_else(Self::invalid_v1)?;
            *byte ^= 0x40;
        }
        let end = start
            .checked_add(destination.len())
            .ok_or_else(Self::invalid_v1)?;
        let source = stored.bytes.get(start..end).ok_or_else(Self::invalid_v1)?;
        destination.copy_from_slice(source);
        Ok(destination.len())
    }

    fn publish_sealed_by_pointer(
        &mut self,
        seal: &ZkAmsMkheDirectObjectSealTokenV1,
        pointer: ZkAmsMkheDirectObjectPointerV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.publish_calls += 1;
        match self.fault {
            FixtureCasFaultV1::PublishBeforeError => return Err(Self::invalid_v1()),
            FixtureCasFaultV1::PublishBeforePanic => panic!("fixture pre-commit publish panic"),
            _ => {}
        }
        let stored = self.seal.as_ref().ok_or_else(Self::invalid_v1)?;
        if stored.seal_identity != seal.seal_identity()
            || stored.token_digest != seal.token_digest()
            || ZkAmsMkheDirectObjectPointerV1::from_payload(stored.kind, &stored.bytes)? != pointer
        {
            return Err(Self::invalid_v1());
        }
        self.object = Some(FixtureObjectV1 {
            pointer,
            bytes: stored.bytes.clone(),
        });
        match self.fault {
            FixtureCasFaultV1::PublishAfterError => Err(Self::invalid_v1()),
            FixtureCasFaultV1::PublishAfterPanic => panic!("fixture post-commit publish panic"),
            _ => Ok(()),
        }
    }

    fn lookup_published_pointer(
        &mut self,
        pointer: ZkAmsMkheDirectObjectPointerV1,
    ) -> Result<Option<ZkAmsMkheDirectObjectPublishedBindingV1>, ZkAmsMkheErrorV1> {
        if self.fault == FixtureCasFaultV1::LookupError {
            return Err(Self::invalid_v1());
        }
        if self.object_v1(pointer).is_err() {
            return Ok(None);
        }
        let publication_identity = if self.fault == FixtureCasFaultV1::LookupWrongBinding {
            [0x52; 32]
        } else {
            self.publication_identity
        };
        Ok(Some(ZkAmsMkheDirectObjectPublishedBindingV1::new(
            publication_identity,
            FIXTURE_PUBLISHED_ID_V1,
            pointer,
        )?))
    }
}

const FIXTURE_PUBLISHED_BINDING_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.public-polynomial-publisher.fixture-published";
const FIXTURE_HANDOFF_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.public-polynomial-publisher.fixture-handoff";
const FIXTURE_READ_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.public-polynomial-publisher.fixture-read";

struct FixturePublishedSetV1 {
    position: RnsNativePublicPolynomialPositionV1,
    object_bytes: u64,
    receipt: ZkAmsMkheDirectObjectPublicationReceiptV1,
    provider: FixtureCasV1,
    retained_receipt_digest: [u8; 32],
    binding_digest: [u8; 32],
}

impl FixturePublishedSetV1 {
    fn new_v1(
        position: RnsNativePublicPolynomialPositionV1,
        object_bytes: u64,
        receipt: ZkAmsMkheDirectObjectPublicationReceiptV1,
        provider: FixtureCasV1,
    ) -> Result<Self, RnsNativePublicPolynomialPublisherErrorV1> {
        validate_publication_receipt_for_bytes_v1(
            FIXTURE_PUBLICATION_ID_V1,
            position,
            object_bytes,
            &receipt,
        )?;
        let retained_receipt_digest = receipt.receipt_digest();
        let binding_digest = fixture_published_binding_digest_v1(
            position,
            object_bytes,
            receipt.pointer(),
            retained_receipt_digest,
        );
        let value = Self {
            position,
            object_bytes,
            receipt,
            provider,
            retained_receipt_digest,
            binding_digest,
        };
        value.validate_v1()?;
        Ok(value)
    }

    fn validate_v1(&self) -> Result<(), RnsNativePublicPolynomialPublisherErrorV1> {
        validate_publication_receipt_for_bytes_v1(
            FIXTURE_PUBLICATION_ID_V1,
            self.position,
            self.object_bytes,
            &self.receipt,
        )?;
        if self.retained_receipt_digest == [0; 32]
            || self.retained_receipt_digest != self.receipt.receipt_digest()
            || self.binding_digest == [0; 32]
            || self.binding_digest
                != fixture_published_binding_digest_v1(
                    self.position,
                    self.object_bytes,
                    self.receipt.pointer(),
                    self.retained_receipt_digest,
                )
        {
            return Err(RnsNativePublicPolynomialPublisherErrorV1::InvalidPublication);
        }
        Ok(())
    }

    fn into_handoff_v1(
        mut self,
    ) -> Result<FixtureReaderHandoffV1, RnsNativePublicPolynomialPublisherErrorV1> {
        self.validate_v1()?;
        let provider_identity = self
            .provider
            .provider_identity()
            .map_err(|_| RnsNativePublicPolynomialPublisherErrorV1::ReaderHandoff)?;
        let snapshot_identity = self
            .provider
            .snapshot_identity()
            .map_err(|_| RnsNativePublicPolynomialPublisherErrorV1::ReaderHandoff)?;
        let published_snapshot = self.receipt.post_publish_read_receipt().snapshot();
        if provider_identity != published_snapshot.provider_identity()
            || snapshot_identity != published_snapshot.snapshot_identity()
        {
            return Err(RnsNativePublicPolynomialPublisherErrorV1::ReaderHandoff);
        }
        let handoff_digest = fixture_handoff_digest_v1(
            self.binding_digest,
            self.retained_receipt_digest,
            self.receipt.post_publish_read_receipt().receipt_digest(),
            provider_identity,
            snapshot_identity,
            self.receipt.pointer(),
        );
        Ok(FixtureReaderHandoffV1 {
            position: self.position,
            object_bytes: self.object_bytes,
            pointer: self.receipt.pointer(),
            provider: self.provider,
            retained_publication_receipt_digest: self.retained_receipt_digest,
            retained_post_read_receipt_digest: self
                .receipt
                .post_publish_read_receipt()
                .receipt_digest(),
            published_binding_digest: self.binding_digest,
            provider_identity,
            snapshot_identity,
            handoff_digest,
        })
    }
}

fn fixture_published_binding_digest_v1(
    position: RnsNativePublicPolynomialPositionV1,
    object_bytes: u64,
    pointer: ZkAmsMkheDirectObjectPointerV1,
    receipt_digest: [u8; 32],
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(FIXTURE_PUBLISHED_BINDING_DOMAIN_V1);
    hash.update(&position.position_digest_v1());
    hash.update(&object_bytes.to_be_bytes());
    hash.update(&pointer.encode());
    hash.update(&receipt_digest);
    hash.finalize()
}

fn fixture_handoff_digest_v1(
    published_binding_digest: [u8; 32],
    publication_receipt_digest: [u8; 32],
    post_read_receipt_digest: [u8; 32],
    provider_identity: [u8; 32],
    snapshot_identity: [u8; 32],
    pointer: ZkAmsMkheDirectObjectPointerV1,
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(FIXTURE_HANDOFF_DOMAIN_V1);
    hash.update(&published_binding_digest);
    hash.update(&publication_receipt_digest);
    hash.update(&post_read_receipt_digest);
    hash.update(&provider_identity);
    hash.update(&snapshot_identity);
    hash.update(&pointer.encode());
    hash.finalize()
}

struct FixtureReaderHandoffV1 {
    position: RnsNativePublicPolynomialPositionV1,
    object_bytes: u64,
    pointer: ZkAmsMkheDirectObjectPointerV1,
    provider: FixtureCasV1,
    retained_publication_receipt_digest: [u8; 32],
    retained_post_read_receipt_digest: [u8; 32],
    published_binding_digest: [u8; 32],
    provider_identity: [u8; 32],
    snapshot_identity: [u8; 32],
    handoff_digest: [u8; 32],
}

impl FixtureReaderHandoffV1 {
    fn finish_v1(
        mut self,
    ) -> Result<FixtureReadReceiptV1, RnsNativePublicPolynomialPublisherErrorV1> {
        if self.retained_publication_receipt_digest == [0; 32]
            || self.retained_post_read_receipt_digest == [0; 32]
            || self.handoff_digest
                != fixture_handoff_digest_v1(
                    self.published_binding_digest,
                    self.retained_publication_receipt_digest,
                    self.retained_post_read_receipt_digest,
                    self.provider_identity,
                    self.snapshot_identity,
                    self.pointer,
                )
            || self
                .provider
                .provider_identity()
                .map_err(|_| RnsNativePublicPolynomialPublisherErrorV1::ReaderHandoff)?
                != self.provider_identity
            || self
                .provider
                .snapshot_identity()
                .map_err(|_| RnsNativePublicPolynomialPublisherErrorV1::ReaderHandoff)?
                != self.snapshot_identity
        {
            return Err(RnsNativePublicPolynomialPublisherErrorV1::ReaderHandoff);
        }
        let read = validate_zk_ams_mkhe_direct_object_v1(
            object_kind_v1(self.position.role_v1()),
            self.pointer,
            &mut self.provider,
        )
        .map_err(|_| RnsNativePublicPolynomialPublisherErrorV1::ReaderHandoff)?;
        let snapshot = read.snapshot();
        if snapshot.provider_identity() != self.provider_identity
            || snapshot.snapshot_identity() != self.snapshot_identity
            || snapshot.pointer() != self.pointer
            || read.canonical_bytes() != self.object_bytes
            || read.payload_blake3() != self.pointer.payload_blake3()
            || read.receipt_digest() != self.retained_post_read_receipt_digest
        {
            return Err(RnsNativePublicPolynomialPublisherErrorV1::ReaderHandoff);
        }
        let mut hash = Keccak256::new();
        hash.update(FIXTURE_READ_DOMAIN_V1);
        hash.update(&self.handoff_digest);
        hash.update(&self.retained_publication_receipt_digest);
        hash.update(&read.receipt_digest());
        let digest = hash.finalize();
        Ok(FixtureReadReceiptV1 {
            pointer: self.pointer,
            canonical_bytes: read.canonical_bytes(),
            digest,
        })
    }
}

struct FixtureReadReceiptV1 {
    pointer: ZkAmsMkheDirectObjectPointerV1,
    canonical_bytes: u64,
    digest: [u8; 32],
}

fn run_fixture_publication_v1(
    source_fault: FixtureSourceFaultV1,
    cas_fault: FixtureCasFaultV1,
) -> Result<FixturePublishedSetV1, RnsNativePublicPolynomialPublisherErrorV1> {
    let position = RnsNativePublicPolynomialPositionV1::from_ordinal_v1(0)?;
    let plan = RnsNativePublicPolynomialTraversalPlanV1::fixture_v1(position, 2)?;
    let mut source = FixtureCoefficientSourceV1::new_v1(position, 2, source_fault);
    let mut provider = FixtureCasV1::new_v1(cas_fault);
    let source_identity = source.source_identity_v1()?;
    let publication_identity = provider
        .publication_identity()
        .map_err(|_| RnsNativePublicPolynomialPublisherErrorV1::InvalidPublication)?;
    let mut stream = Keccak256::new();
    stream.update(b"fixture-production-traversal-seam");
    let receipt = {
        let mut traversal = RnsNativePublicPolynomialAuthenticatedTraversalV1::new_v1(
            &mut source,
            &mut provider,
            source_identity,
            publication_identity,
        )?;
        traversal.publish_next_v1(plan, &mut stream)?
    };
    FixturePublishedSetV1::new_v1(position, plan.object_bytes, receipt, provider)
}

fn eliminate_production_source_v1(value: RnsNativePhase23FortyLimbProductionSourceV1) -> ! {
    match value {}
}

#[test]
fn fixture_roundtrip_uses_production_traversal_and_typed_handoff() {
    let published =
        run_fixture_publication_v1(FixtureSourceFaultV1::None, FixtureCasFaultV1::None).unwrap();
    assert_eq!(published.object_bytes, 4 + 2 * 1_024 * 8);
    assert_eq!(published.provider.begin_calls, 1);
    assert_eq!(published.provider.write_calls, 3);
    assert_eq!(published.provider.sealed_read_calls, 3);
    assert_eq!(published.provider.publish_calls, 1);
    assert_eq!(published.provider.provider_read_calls, 3);
    let pointer = published.receipt.pointer();
    let handoff = published.into_handoff_v1().unwrap();
    let read = handoff.finish_v1().unwrap();
    assert_eq!(read.pointer, pointer);
    assert_eq!(read.canonical_bytes, 16_388);
    assert_ne!(read.digest, [0; 32]);
}

#[test]
fn fixture_lost_publish_ack_reconciles_but_precommit_failure_does_not() {
    let reconciled = run_fixture_publication_v1(
        FixtureSourceFaultV1::None,
        FixtureCasFaultV1::PublishAfterError,
    )
    .unwrap();
    assert!(reconciled.receipt.reconciled_after_publish_error());
    assert!(reconciled.into_handoff_v1().unwrap().finish_v1().is_ok());

    assert!(
        run_fixture_publication_v1(
            FixtureSourceFaultV1::None,
            FixtureCasFaultV1::PublishBeforeError,
        )
        .is_err()
    );
}

#[test]
fn fixture_all_cas_error_surfaces_issue_no_typed_handoff() {
    for fault in [
        FixtureCasFaultV1::BeginError,
        FixtureCasFaultV1::WriteError,
        FixtureCasFaultV1::SealError,
        FixtureCasFaultV1::SealedReadError,
        FixtureCasFaultV1::SealedReadMutation,
        FixtureCasFaultV1::PublishBeforeError,
        FixtureCasFaultV1::LookupError,
        FixtureCasFaultV1::LookupWrongBinding,
        FixtureCasFaultV1::ReadbackError,
        FixtureCasFaultV1::ReadbackShort,
        FixtureCasFaultV1::ReadbackMutation,
        FixtureCasFaultV1::ProviderDriftAfterRead,
        FixtureCasFaultV1::SnapshotDriftAfterRead,
        FixtureCasFaultV1::ObjectLengthMutation,
    ] {
        assert!(
            run_fixture_publication_v1(FixtureSourceFaultV1::None, fault).is_err(),
            "fault unexpectedly issued a published set: {fault:?}"
        );
    }
}

#[test]
fn source_drift_fails_before_staging_and_permanently_poisons_traversal() {
    let position = RnsNativePublicPolynomialPositionV1::from_ordinal_v1(0).unwrap();
    let plan = RnsNativePublicPolynomialTraversalPlanV1::fixture_v1(position, 2).unwrap();
    let mut source = FixtureCoefficientSourceV1::new_v1(position, 2, FixtureSourceFaultV1::None);
    let source_identity = source.source_identity_v1().unwrap();
    source.drift_at_identity_call = Some(2);
    {
        let mut provider = FixtureCasV1::new_v1(FixtureCasFaultV1::None);
        let mut hash = Keccak256::new();
        let mut traversal = RnsNativePublicPolynomialAuthenticatedTraversalV1::new_v1(
            &mut source,
            &mut provider,
            source_identity,
            FIXTURE_PUBLICATION_ID_V1,
        )
        .unwrap();
        assert!(matches!(
            traversal.publish_next_v1(plan, &mut hash),
            Err(RnsNativePublicPolynomialPublisherErrorV1::InvalidSource)
        ));
        assert!(traversal.is_poisoned_v1());
        assert_eq!(traversal.publisher.begin_calls, 0);
        assert!(traversal.publish_next_v1(plan, &mut hash).is_err());
        assert_eq!(traversal.publisher.begin_calls, 0);
    }
    assert!(source.finish_source_v1().is_err());
}

#[test]
fn unfilled_source_chunk_is_rejected_without_seal_or_retry_capability() {
    let position = RnsNativePublicPolynomialPositionV1::from_ordinal_v1(0).unwrap();
    let plan = RnsNativePublicPolynomialTraversalPlanV1::fixture_v1(position, 2).unwrap();
    let mut source =
        FixtureCoefficientSourceV1::new_v1(position, 2, FixtureSourceFaultV1::LeaveOneUnfilled);
    let source_identity = source.source_identity_v1().unwrap();
    {
        let mut provider = FixtureCasV1::new_v1(FixtureCasFaultV1::None);
        let mut hash = Keccak256::new();
        let mut traversal = RnsNativePublicPolynomialAuthenticatedTraversalV1::new_v1(
            &mut source,
            &mut provider,
            source_identity,
            FIXTURE_PUBLICATION_ID_V1,
        )
        .unwrap();
        assert!(matches!(
            traversal.publish_next_v1(plan, &mut hash),
            Err(RnsNativePublicPolynomialPublisherErrorV1::InvalidCoefficient)
        ));
        assert!(traversal.is_poisoned_v1());
        assert!(traversal.publisher.seal.is_none());
        assert!(traversal.publisher.object.is_none());
        let writes = traversal.publisher.write_calls;
        assert!(traversal.publish_next_v1(plan, &mut hash).is_err());
        assert_eq!(traversal.publisher.write_calls, writes);
    }
    assert!(source.finish_source_v1().is_err());
}

#[test]
fn caught_source_unwind_leaves_traversal_permanently_poisoned() {
    let position = RnsNativePublicPolynomialPositionV1::from_ordinal_v1(0).unwrap();
    let plan = RnsNativePublicPolynomialTraversalPlanV1::fixture_v1(position, 2).unwrap();
    let mut source =
        FixtureCoefficientSourceV1::new_v1(position, 2, FixtureSourceFaultV1::PanicFill);
    let source_identity = source.source_identity_v1().unwrap();
    {
        let mut provider = FixtureCasV1::new_v1(FixtureCasFaultV1::None);
        let mut hash = Keccak256::new();
        let mut traversal = RnsNativePublicPolynomialAuthenticatedTraversalV1::new_v1(
            &mut source,
            &mut provider,
            source_identity,
            FIXTURE_PUBLICATION_ID_V1,
        )
        .unwrap();
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = traversal.publish_next_v1(plan, &mut hash);
        }));
        assert!(unwind.is_err());
        assert!(traversal.is_poisoned_v1());
        assert!(traversal.publish_next_v1(plan, &mut hash).is_err());
        assert!(traversal.publisher.seal.is_none());
        assert!(traversal.publisher.object.is_none());
    }
    assert!(source.finish_source_v1().is_err());
}

#[test]
fn caught_cas_unwinds_leave_no_receipt_or_retry_capability() {
    for fault in [
        FixtureCasFaultV1::BeginPanic,
        FixtureCasFaultV1::WritePanic,
        FixtureCasFaultV1::SealPanic,
        FixtureCasFaultV1::SealedReadPanic,
        FixtureCasFaultV1::PublishBeforePanic,
        FixtureCasFaultV1::PublishAfterPanic,
        FixtureCasFaultV1::ReadbackPanic,
    ] {
        let position = RnsNativePublicPolynomialPositionV1::from_ordinal_v1(0).unwrap();
        let plan = RnsNativePublicPolynomialTraversalPlanV1::fixture_v1(position, 2).unwrap();
        let mut source =
            FixtureCoefficientSourceV1::new_v1(position, 2, FixtureSourceFaultV1::None);
        let source_identity = source.source_identity_v1().unwrap();
        {
            let mut provider = FixtureCasV1::new_v1(fault);
            let mut hash = Keccak256::new();
            let mut traversal = RnsNativePublicPolynomialAuthenticatedTraversalV1::new_v1(
                &mut source,
                &mut provider,
                source_identity,
                FIXTURE_PUBLICATION_ID_V1,
            )
            .unwrap();
            let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = traversal.publish_next_v1(plan, &mut hash);
            }));
            assert!(unwind.is_err(), "fault did not unwind: {fault:?}");
            assert!(traversal.is_poisoned_v1());
            let begin_calls = traversal.publisher.begin_calls;
            assert!(traversal.publish_next_v1(plan, &mut hash).is_err());
            assert_eq!(traversal.publisher.begin_calls, begin_calls);
        }
        assert!(source.finish_source_v1().is_err());
    }
}

#[test]
fn typed_handoff_rejects_receipt_provider_and_snapshot_mutation() {
    let mut receipt_mutation =
        run_fixture_publication_v1(FixtureSourceFaultV1::None, FixtureCasFaultV1::None).unwrap();
    receipt_mutation.retained_receipt_digest[0] ^= 1;
    assert!(receipt_mutation.into_handoff_v1().is_err());

    let mut binding_mutation =
        run_fixture_publication_v1(FixtureSourceFaultV1::None, FixtureCasFaultV1::None).unwrap();
    binding_mutation.binding_digest[0] ^= 1;
    assert!(binding_mutation.into_handoff_v1().is_err());

    let mut provider_mutation =
        run_fixture_publication_v1(FixtureSourceFaultV1::None, FixtureCasFaultV1::None).unwrap();
    provider_mutation.provider.provider_identity = FIXTURE_DRIFTED_PROVIDER_ID_V1;
    assert!(provider_mutation.into_handoff_v1().is_err());

    let mut snapshot_mutation =
        run_fixture_publication_v1(FixtureSourceFaultV1::None, FixtureCasFaultV1::None).unwrap();
    snapshot_mutation.provider.snapshot_identity = FIXTURE_DRIFTED_SNAPSHOT_ID_V1;
    assert!(snapshot_mutation.into_handoff_v1().is_err());

    let published =
        run_fixture_publication_v1(FixtureSourceFaultV1::None, FixtureCasFaultV1::None).unwrap();
    let mut handoff = published.into_handoff_v1().unwrap();
    handoff.provider.provider_identity = FIXTURE_DRIFTED_PROVIDER_ID_V1;
    assert!(handoff.finish_v1().is_err());

    let published =
        run_fixture_publication_v1(FixtureSourceFaultV1::None, FixtureCasFaultV1::None).unwrap();
    let mut handoff = published.into_handoff_v1().unwrap();
    handoff.provider.snapshot_identity = FIXTURE_DRIFTED_SNAPSHOT_ID_V1;
    assert!(handoff.finish_v1().is_err());

    let published =
        run_fixture_publication_v1(FixtureSourceFaultV1::None, FixtureCasFaultV1::None).unwrap();
    let mut handoff = published.into_handoff_v1().unwrap();
    handoff.retained_publication_receipt_digest[0] ^= 1;
    assert!(handoff.finish_v1().is_err());

    let published =
        run_fixture_publication_v1(FixtureSourceFaultV1::None, FixtureCasFaultV1::None).unwrap();
    let mut handoff = published.into_handoff_v1().unwrap();
    handoff.retained_post_read_receipt_digest[0] ^= 1;
    assert!(handoff.finish_v1().is_err());

    let published =
        run_fixture_publication_v1(FixtureSourceFaultV1::None, FixtureCasFaultV1::None).unwrap();
    let mut handoff = published.into_handoff_v1().unwrap();
    handoff.provider.fault = FixtureCasFaultV1::ReadbackMutation;
    assert!(handoff.finish_v1().is_err());
}

fn fake_pointer_v1(
    position: RnsNativePublicPolynomialPositionV1,
) -> ZkAmsMkheDirectObjectPointerV1 {
    let mut hash = Keccak256::new();
    hash.update(b"publisher-test-distinct-payload");
    hash.update(&position.position_digest_v1());
    let payload_blake3 = hash.finalize();
    ZkAmsMkheDirectObjectPointerV1::new(
        object_kind_v1(position.role_v1()),
        OBJECT_BYTES_V1,
        payload_blake3,
    )
    .unwrap()
}

#[test]
fn audited_upstream_is_exactly_legacy_38_and_retains_no_native_ciphertext() {
    let manifest = include_str!("manifest.rs");
    let streaming = include_str!("collective/incremental_source.rs");
    let phase23 = include_str!("collective/incremental_source_phase23.rs");

    assert!(manifest.contains("pub(super) const RELEASE_MODULI_V1: [u64; 38]"));
    assert!(streaming.contains("publishes 38 independently addressed `c0` limbs"));
    assert!(streaming.contains("38 independently addressed `c1` limbs"));
    assert!(streaming.contains("no native `2P` ciphertext owner or secret opening is retained"));
    assert!(
        streaming
            .contains("const STREAMING_COLLECTIVE_RNS_LIMBS_V1: usize = RELEASE_MODULI_V1.len();")
    );

    let owner = phase23
        .split_once("struct ZkAmsPhase23MaterializedEncryptedSourceOwnerV1<K, P> {")
        .unwrap()
        .1
        .split_once("\n}")
        .unwrap()
        .0;
    assert!(owner.contains("manifests: Vec<ZkAmsMkheStreamingCollectiveCiphertextV1>"));
    assert!(!owner.contains("RnsPolynomial"));
    assert!(!owner.contains("coefficients"));

    assert_eq!(LEGACY_OBJECTS_V1, 3_344);
    assert_eq!(OBJECTS_V1, 3_520);
    assert_eq!(MISSING_NEW_LIMB_OBJECTS_V1, 176);
    let missing = (0..OBJECTS_V1)
        .map(|ordinal| RnsNativePublicPolynomialPositionV1::from_ordinal_v1(ordinal).unwrap())
        .filter(|position| usize::from(position.limb_v1()) >= LEGACY_RELEASE_LIMBS_V1)
        .count();
    assert_eq!(missing, MISSING_NEW_LIMB_OBJECTS_V1);
}

#[test]
fn publication_geometry_and_resource_accounting_are_exact() {
    assert_eq!(ROLES_PER_RECORD_SET_V1, 88);
    assert_eq!(CIPHERTEXT_OBJECTS_PER_COMPONENT_V1, 1_720);
    assert_eq!(OBJECTS_V1, 3_520);
    assert_eq!(COEFFICIENTS_PER_CHUNK_V1, 1_024);
    assert_eq!(CHUNKS_PER_OBJECT_V1, 128);
    assert_eq!(OBJECT_BYTES_V1, 1_048_580);
    assert_eq!(SOURCE_CHUNK_CALLS_V1, 450_560);
    assert_eq!(PUBLICATION_WRITE_CALLS_V1, 454_080);
    assert_eq!(PUBLICATION_TRANSPORT_CALLS_V1, 1_362_240);
    assert_eq!(CANONICAL_COEFFICIENTS_V1, 461_373_440);
    assert_eq!(CANONICAL_BYTES_V1, 3_691_001_600);
    assert_eq!(AUTHENTICATED_TRANSFER_BYTES_V1, 11_073_004_800);
    assert_eq!(COARSE_WORK_UNITS_V1, 11_534_378_240);
    assert_eq!(POINTER_FRAME_BYTES_V1, 274_560);
    assert_eq!(PUBLICATION_STACK_WORKSPACE_BYTES_V1, 24_576);
    assert!(
        PUBLICATION_RESOURCE_ACCOUNTING_SCOPE_V1
            .windows(b"excludes-later-public-polynomial-reader".len())
            .any(|window| window == b"excludes-later-public-polynomial-reader")
    );
    assert!(
        PUBLICATION_RESOURCE_ACCOUNTING_SCOPE_V1
            .windows(b"excludes-measured-rss-and-device-evidence".len())
            .any(|window| window == b"excludes-measured-rss-and-device-evidence")
    );
    let position = RnsNativePublicPolynomialPositionV1::from_ordinal_v1(0).unwrap();
    let production = RnsNativePublicPolynomialTraversalPlanV1::production_v1(position).unwrap();
    assert_eq!(production.coefficient_count, 131_072);
    assert_eq!(production.chunks, 128);
    assert_eq!(production.object_bytes, 1_048_580);
    let source = include_str!("rns_native_public_polynomial_publisher.rs");
    assert!(source.contains("#[cfg(test)]\n    fn fixture_v1("));
}

fn assert_position_v1(
    ordinal: usize,
    role: RnsNativePublicPolynomialRoleV1,
    record: Option<u8>,
    limb: u8,
) {
    let position = RnsNativePublicPolynomialPositionV1::from_ordinal_v1(ordinal).unwrap();
    assert_eq!(usize::from(position.ordinal_v1()), ordinal);
    assert_eq!(position.role_v1(), role);
    assert_eq!(position.record_v1(), record);
    assert_eq!(position.limb_v1(), limb);
    assert_eq!(
        position.modulus_v1(),
        ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[usize::from(limb)]
    );
    assert_ne!(position.position_digest_v1(), [0; 32]);
}

#[test]
fn sole_manifest_order_has_exact_boundaries() {
    assert_position_v1(0, RnsNativePublicPolynomialRoleV1::PublicA, None, 0);
    assert_position_v1(39, RnsNativePublicPolynomialRoleV1::PublicA, None, 39);
    assert_position_v1(40, RnsNativePublicPolynomialRoleV1::PublicB, None, 0);
    assert_position_v1(79, RnsNativePublicPolynomialRoleV1::PublicB, None, 39);
    assert_position_v1(
        80,
        RnsNativePublicPolynomialRoleV1::CiphertextC0,
        Some(0),
        0,
    );
    assert_position_v1(
        119,
        RnsNativePublicPolynomialRoleV1::CiphertextC0,
        Some(0),
        39,
    );
    assert_position_v1(
        120,
        RnsNativePublicPolynomialRoleV1::CiphertextC0,
        Some(1),
        0,
    );
    assert_position_v1(
        1_799,
        RnsNativePublicPolynomialRoleV1::CiphertextC0,
        Some(42),
        39,
    );
    assert_position_v1(
        1_800,
        RnsNativePublicPolynomialRoleV1::CiphertextC1,
        Some(0),
        0,
    );
    assert_position_v1(
        3_519,
        RnsNativePublicPolynomialRoleV1::CiphertextC1,
        Some(42),
        39,
    );
    assert!(RnsNativePublicPolynomialPositionV1::from_ordinal_v1(OBJECTS_V1).is_err());
}

#[test]
fn chunk_requests_cover_each_object_exactly_once() {
    let position = RnsNativePublicPolynomialPositionV1::from_ordinal_v1(3_519).unwrap();
    let first = RnsNativePublicPolynomialChunkRequestV1::new_v1(position, 0).unwrap();
    let last = RnsNativePublicPolynomialChunkRequestV1::new_v1(position, CHUNKS_PER_OBJECT_V1 - 1)
        .unwrap();
    assert_eq!(first.first_coefficient_v1(), 0);
    assert_eq!(first.coefficient_count_v1(), 1_024);
    assert_eq!(last.chunk_v1(), 127);
    assert_eq!(last.first_coefficient_v1(), 130_048);
    assert_eq!(
        usize::try_from(last.first_coefficient_v1()).unwrap()
            + usize::from(last.coefficient_count_v1()),
        ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
    );
    assert_ne!(first.request_digest_v1(), last.request_digest_v1());
    assert!(
        RnsNativePublicPolynomialChunkRequestV1::new_v1(position, CHUNKS_PER_OBJECT_V1).is_err()
    );
}

#[test]
fn canonical_encoder_is_big_endian_strict_and_kat_bound() {
    let position = RnsNativePublicPolynomialPositionV1::from_ordinal_v1(0).unwrap();
    let request = RnsNativePublicPolynomialChunkRequestV1::new_v1(position, 0).unwrap();
    let mut coefficients = [0_u64; COEFFICIENTS_PER_CHUNK_V1];
    for (index, coefficient) in coefficients.iter_mut().enumerate() {
        *coefficient = index as u64;
    }
    let mut encoded = [0xa5_u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1];
    encode_chunk_v1(request, &coefficients, &mut encoded).unwrap();
    assert_eq!(&encoded[..8], &0_u64.to_be_bytes());
    assert_eq!(&encoded[8..16], &1_u64.to_be_bytes());
    assert_eq!(&encoded[encoded.len() - 8..], &1_023_u64.to_be_bytes());

    let mut hash = Keccak256::new();
    hash.update(&encoded);
    assert_eq!(
        hex::encode(hash.finalize()),
        "bc78455563e56879d53ef95fd945a45b7d313e811648f0a6f294512d74baae87"
    );
    assert_eq!(
        hex::encode(encoding_contract_digest_v1()),
        "8341749b474d7e14a813c68c5f2b3d3306fd1f2261fa1c05e0c8ff98a62f26e9"
    );
    assert_eq!(
        hex::encode(source_contract_digest_v1()),
        "efcc96131f02b62cd968970c95bd7b2b767fdd21e87405f0679726610233a080"
    );
    assert_eq!(
        hex::encode(position.position_digest_v1()),
        "4540deaf37df90497bd7682481a729439a21a879679132598b35c78bed306997"
    );
    assert_eq!(
        hex::encode(request.request_digest_v1()),
        "f052363efc33c1c8aad26478c7fb5cd97903246e1a806b6042ac5e34657880d4"
    );
}

#[test]
fn canonical_encoder_rejects_modulus_and_does_not_reduce() {
    let position = RnsNativePublicPolynomialPositionV1::from_ordinal_v1(0).unwrap();
    let request = RnsNativePublicPolynomialChunkRequestV1::new_v1(position, 0).unwrap();
    let mut coefficients = [0_u64; COEFFICIENTS_PER_CHUNK_V1];
    coefficients[517] = position.modulus_v1();
    let mut encoded = [0xa5_u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1];
    assert_eq!(
        encode_chunk_v1(request, &coefficients, &mut encoded),
        Err(RnsNativePublicPolynomialPublisherErrorV1::InvalidCoefficient)
    );
    assert_eq!(encoded, [0xa5; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1]);
}

#[test]
fn manifest_builder_accepts_only_the_complete_distinct_exact_order() {
    let mut builder = RnsNativePublicPolynomialManifestBuilderV1::try_new_v1().unwrap();
    for ordinal in 0..OBJECTS_V1 {
        let position = RnsNativePublicPolynomialPositionV1::from_ordinal_v1(ordinal).unwrap();
        builder
            .absorb_pointer_v1(position, fake_pointer_v1(position))
            .unwrap();
    }
    let manifest = builder.finish_v1().unwrap();
    assert_ne!(manifest.manifest_digest_v1(), [0; 32]);
    let a0 = manifest
        .statement_artifact_digest_v1(RnsNativePublicPolynomialRoleV1::PublicA, None, 0)
        .unwrap();
    let c1_last = manifest
        .statement_artifact_digest_v1(RnsNativePublicPolynomialRoleV1::CiphertextC1, Some(42), 39)
        .unwrap();
    assert_ne!(a0, [0; 32]);
    assert_ne!(c1_last, [0; 32]);
    assert_ne!(a0, c1_last);
}

#[test]
fn manifest_builder_rejects_skip_kind_length_and_duplicate() {
    let position0 = RnsNativePublicPolynomialPositionV1::from_ordinal_v1(0).unwrap();
    let position1 = RnsNativePublicPolynomialPositionV1::from_ordinal_v1(1).unwrap();

    let mut skipped = RnsNativePublicPolynomialManifestBuilderV1::try_new_v1().unwrap();
    assert!(
        skipped
            .absorb_pointer_v1(position1, fake_pointer_v1(position1))
            .is_err()
    );

    let wrong_kind = ZkAmsMkheDirectObjectPointerV1::new(
        ZkAmsMkheDirectObjectKindV1::CollectivePublicB,
        OBJECT_BYTES_V1,
        [0x31; 32],
    )
    .unwrap();
    let mut kinds = RnsNativePublicPolynomialManifestBuilderV1::try_new_v1().unwrap();
    assert!(kinds.absorb_pointer_v1(position0, wrong_kind).is_err());

    let wrong_length = ZkAmsMkheDirectObjectPointerV1::new(
        ZkAmsMkheDirectObjectKindV1::CollectivePublicA,
        OBJECT_BYTES_V1 - 1,
        [0x32; 32],
    )
    .unwrap();
    let mut lengths = RnsNativePublicPolynomialManifestBuilderV1::try_new_v1().unwrap();
    assert!(lengths.absorb_pointer_v1(position0, wrong_length).is_err());

    let duplicate = fake_pointer_v1(position0);
    let mut duplicates = RnsNativePublicPolynomialManifestBuilderV1::try_new_v1().unwrap();
    duplicates.absorb_pointer_v1(position0, duplicate).unwrap();
    assert_eq!(
        duplicates.absorb_pointer_v1(position1, duplicate),
        Err(RnsNativePublicPolynomialPublisherErrorV1::DuplicatePointer)
    );
}

#[test]
fn publication_chronology_uses_complete_authenticated_transactions() {
    let source = include_str!("rns_native_public_polynomial_publisher.rs");
    let production = source
        .split_once("pub(super) fn publish_rns_native_public_polynomials_v1")
        .unwrap()
        .1;
    let plan = production.find("::production_v1(position)").unwrap();
    let traverse = production
        .find("traversal.publish_next_v1(plan, &mut source_stream_hash)")
        .unwrap();
    let receipt = production
        .find("validate_publication_receipt_v1(publication_identity, position, &receipt)")
        .unwrap();
    let manifest = production
        .find("manifest.absorb_pointer_v1(position, receipt.pointer())")
        .unwrap();
    assert!(plan < traverse && traverse < receipt && receipt < manifest);
    assert!(!production.contains("fixture_v1("));

    let seam = source
        .split_once("fn publish_next_inner_v1(")
        .unwrap()
        .1
        .split_once("#[cfg(test)]\n    const fn is_poisoned_v1")
        .unwrap()
        .0;
    let identity = seam.find("self.source.source_identity_v1()").unwrap();
    let begin = seam
        .find("ZkAmsMkheDirectObjectPublicationTransactionV1::begin(")
        .unwrap();
    let prefix = seam.find(".write_exact(&count)").unwrap();
    let fill = seam.find(".fill_next_chunk_v1(request").unwrap();
    let encode = seam.find("encode_chunk_v1(request").unwrap();
    let write = seam.find(".write_exact(&self.encoded)").unwrap();
    let finish = seam.find(".finish()").unwrap();
    let authenticate = seam
        .find("validate_publication_receipt_for_bytes_v1(")
        .unwrap();
    assert!(identity < begin);
    assert!(begin < prefix);
    assert!(prefix < fill);
    assert!(fill < encode);
    assert!(encode < write);
    assert!(write < finish);
    assert!(finish < authenticate);
    assert!(!seam.contains("from_payload("));
}

#[test]
fn source_terminals_are_exact_and_production_adapter_remains_uninhabited() {
    let terminal = RnsNativePublicPolynomialSourceTerminalV1::new_v1(
        [0x11; 32],
        OBJECTS_V1 as u16,
        CANONICAL_COEFFICIENTS_V1,
        [0x22; 32],
    );
    let terminal = terminal.unwrap();
    assert_eq!(terminal.upstream_owner_digest_v1(), [0x22; 32]);
    assert_eq!(
        hex::encode(terminal.terminal_digest_v1()),
        "851183293836640219b827073183bc191e80bbfbeec8a5a59af6da5491e2af25"
    );
    assert!(
        RnsNativePublicPolynomialSourceTerminalV1::new_v1(
            [0x11; 32],
            (OBJECTS_V1 - 1) as u16,
            CANONICAL_COEFFICIENTS_V1,
            [0x22; 32],
        )
        .is_err()
    );
    assert!(
        RnsNativePublicPolynomialSourceTerminalV1::new_v1(
            [0x11; 32],
            OBJECTS_V1 as u16,
            CANONICAL_COEFFICIENTS_V1,
            [0x11; 32],
        )
        .is_err()
    );
    assert!(
        RnsNativePublicPolynomialSourceTerminalV1::new_v1(
            [0x11; 32],
            OBJECTS_V1 as u16,
            CANONICAL_COEFFICIENTS_V1 - 1,
            [0x22; 32],
        )
        .is_err()
    );
    assert_eq!(
        core::mem::size_of::<RnsNativePhase23FortyLimbProductionSourceV1>(),
        0
    );
    let _: fn(RnsNativePhase23FortyLimbProductionSourceV1) -> ! = eliminate_production_source_v1;
    let source = include_str!("rns_native_public_polynomial_publisher.rs");
    assert!(source.contains("pub(super) enum RnsNativePhase23FortyLimbProductionSourceV1 {}"));
    let adapter_declaration = source
        .split_once("pub(super) enum RnsNativePhase23FortyLimbProductionSourceV1")
        .unwrap()
        .1
        .split_once("impl RnsNativePublicPolynomialCoefficientSourceV1")
        .unwrap()
        .0;
    assert!(!adapter_declaration.contains("fn new"));
    assert!(!adapter_declaration.contains("unsafe"));
    let parent = include_str!("../mkhe.rs");
    assert!(parent.contains("mod rns_native_public_polynomial_publisher;"));
}
