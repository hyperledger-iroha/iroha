use super::super::ZkAmsMkheErrorV1;
use super::*;

const TEST_SCHEDULE_POINT_COUNT_V1: usize = ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 * REPETITIONS_V1;

const _: () = {
    assert!(PUBLIC_POLYNOMIAL_CANONICAL_BYTES_V1 < ZK_AMS_MKHE_RNS_NATIVE_IO_MAX_BYTES_V1);
    assert!(PUBLIC_POLYNOMIAL_COARSE_WORK_UNITS_V1 < ZK_AMS_MKHE_RNS_NATIVE_WORK_MAX_V1);
};

const _: () = {
    assert!(RNS_NATIVE_PUBLIC_POLYNOMIAL_READER_SOURCE_SETTLED_V1);
    assert!(RNS_NATIVE_PUBLIC_POLYNOMIAL_READER_DECLARED_V1);
    assert!(!RNS_NATIVE_PUBLIC_POLYNOMIAL_UPSTREAM_40_LIMB_MANIFEST_INTEGRATED_V1);
    assert!(!RNS_NATIVE_PUBLIC_POLYNOMIAL_SOURCE_PREFLIGHT_INTEGRATED_V1);
    assert!(!RNS_NATIVE_PUBLIC_POLYNOMIAL_DIRECT_NUMERIC_SOURCE_INTEGRATED_V1);
    assert!(!RNS_NATIVE_PUBLIC_POLYNOMIAL_MEASURED_RSS_QUALIFIED_V1);
    assert!(!RNS_NATIVE_PUBLIC_POLYNOMIAL_PRODUCTION_READY_V1);
};

struct ManifestPartsV1 {
    public_a: Vec<RnsNativePublicPolynomialDescriptorV1>,
    public_b: Vec<RnsNativePublicPolynomialDescriptorV1>,
    ciphertext_c0: Vec<RnsNativePublicPolynomialDescriptorV1>,
    ciphertext_c1: Vec<RnsNativePublicPolynomialDescriptorV1>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ObjectProviderFaultV1 {
    None,
    WrongLength,
    ShortReadAt(usize),
    ProviderDriftAfterRead(usize),
    SnapshotDriftAfterRead(usize),
}

struct TestObjectProviderV1 {
    expected_pointer: ZkAmsMkheDirectObjectPointerV1,
    bytes: Vec<u8>,
    fault: ObjectProviderFaultV1,
    read_calls: usize,
    object_len_calls: usize,
}

impl TestObjectProviderV1 {
    fn new(
        expected_pointer: ZkAmsMkheDirectObjectPointerV1,
        bytes: Vec<u8>,
        fault: ObjectProviderFaultV1,
    ) -> Self {
        Self {
            expected_pointer,
            bytes,
            fault,
            read_calls: 0,
            object_len_calls: 0,
        }
    }

    fn provider_identity_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        if matches!(
            self.fault,
            ObjectProviderFaultV1::ProviderDriftAfterRead(limit) if self.read_calls >= limit
        ) {
            [0xd1; DIGEST_BYTES_V1]
        } else {
            [0xa1; DIGEST_BYTES_V1]
        }
    }

    fn snapshot_identity_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        if matches!(
            self.fault,
            ObjectProviderFaultV1::SnapshotDriftAfterRead(limit) if self.read_calls >= limit
        ) {
            [0xd2; DIGEST_BYTES_V1]
        } else {
            [0xa2; DIGEST_BYTES_V1]
        }
    }
}

impl ZkAmsMkheDirectObjectReadAtProviderV1 for TestObjectProviderV1 {
    fn provider_identity(&mut self) -> Result<[u8; DIGEST_BYTES_V1], ZkAmsMkheErrorV1> {
        Ok(self.provider_identity_v1())
    }

    fn snapshot_identity(&mut self) -> Result<[u8; DIGEST_BYTES_V1], ZkAmsMkheErrorV1> {
        Ok(self.snapshot_identity_v1())
    }

    fn object_len(
        &mut self,
        pointer: ZkAmsMkheDirectObjectPointerV1,
    ) -> Result<u64, ZkAmsMkheErrorV1> {
        self.object_len_calls = self.object_len_calls.saturating_add(1);
        if pointer != self.expected_pointer {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let length = u64::try_from(self.bytes.len())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if self.fault == ObjectProviderFaultV1::WrongLength {
            length
                .checked_add(1)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
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
        if pointer != self.expected_pointer {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let start = usize::try_from(absolute_offset)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let end = start
            .checked_add(destination.len())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let source = self
            .bytes
            .get(start..end)
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        self.read_calls = self.read_calls.saturating_add(1);
        if matches!(
            self.fault,
            ObjectProviderFaultV1::ShortReadAt(call) if self.read_calls == call
        ) {
            let short = destination.len().saturating_sub(1);
            destination[..short].copy_from_slice(&source[..short]);
            return Ok(short);
        }
        destination.copy_from_slice(source);
        Ok(destination.len())
    }
}

#[derive(Default)]
struct IdentityOnlyProviderV1 {
    object_calls: usize,
}

impl ZkAmsMkheDirectObjectReadAtProviderV1 for IdentityOnlyProviderV1 {
    fn provider_identity(&mut self) -> Result<[u8; DIGEST_BYTES_V1], ZkAmsMkheErrorV1> {
        Ok([0xb1; DIGEST_BYTES_V1])
    }

    fn snapshot_identity(&mut self) -> Result<[u8; DIGEST_BYTES_V1], ZkAmsMkheErrorV1> {
        Ok([0xb2; DIGEST_BYTES_V1])
    }

    fn object_len(
        &mut self,
        _pointer: ZkAmsMkheDirectObjectPointerV1,
    ) -> Result<u64, ZkAmsMkheErrorV1> {
        self.object_calls = self.object_calls.saturating_add(1);
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    }

    fn read_at(
        &mut self,
        _pointer: ZkAmsMkheDirectObjectPointerV1,
        _absolute_offset: u64,
        _destination: &mut [u8],
    ) -> Result<usize, ZkAmsMkheErrorV1> {
        self.object_calls = self.object_calls.saturating_add(1);
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    }
}

fn synthetic_pointer_v1(
    role: RnsNativePublicPolynomialRoleV1,
    ordinal: usize,
    payload_bytes: u64,
) -> ZkAmsMkheDirectObjectPointerV1 {
    let mut payload_blake3 = [0_u8; DIGEST_BYTES_V1];
    payload_blake3[..8].copy_from_slice(
        &u64::try_from(ordinal + 1)
            .expect("test ordinal fits u64")
            .to_be_bytes(),
    );
    payload_blake3[8] = role as u8 + 1;
    payload_blake3[DIGEST_BYTES_V1 - 1] = 0xa5;
    ZkAmsMkheDirectObjectPointerV1::new(role.object_kind_v1(), payload_bytes, payload_blake3)
        .expect("synthetic pointer is canonical")
}

fn descriptor_v1(
    role: RnsNativePublicPolynomialRoleV1,
    record: Option<usize>,
    limb: usize,
    ordinal: usize,
) -> RnsNativePublicPolynomialDescriptorV1 {
    RnsNativePublicPolynomialDescriptorV1::new(
        role,
        record.map(|value| u8::try_from(value).expect("test record fits u8")),
        limb,
        synthetic_pointer_v1(role, ordinal, LIMB_OBJECT_BYTES_V1),
    )
    .expect("synthetic descriptor is canonical")
}

fn exact_manifest_parts_v1() -> ManifestPartsV1 {
    let mut ordinal = 0_usize;
    let mut public_a = Vec::with_capacity(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1);
    for limb in 0..ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 {
        public_a.push(descriptor_v1(
            RnsNativePublicPolynomialRoleV1::PublicA,
            None,
            limb,
            ordinal,
        ));
        ordinal += 1;
    }
    let mut public_b = Vec::with_capacity(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1);
    for limb in 0..ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 {
        public_b.push(descriptor_v1(
            RnsNativePublicPolynomialRoleV1::PublicB,
            None,
            limb,
            ordinal,
        ));
        ordinal += 1;
    }
    let mut ciphertext_c0 = Vec::with_capacity(PUBLIC_CIPHERTEXT_LIMB_OBJECTS_V1);
    for record in 0..RECORDS_V1 {
        for limb in 0..ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 {
            ciphertext_c0.push(descriptor_v1(
                RnsNativePublicPolynomialRoleV1::CiphertextC0,
                Some(record),
                limb,
                ordinal,
            ));
            ordinal += 1;
        }
    }
    let mut ciphertext_c1 = Vec::with_capacity(PUBLIC_CIPHERTEXT_LIMB_OBJECTS_V1);
    for record in 0..RECORDS_V1 {
        for limb in 0..ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 {
            ciphertext_c1.push(descriptor_v1(
                RnsNativePublicPolynomialRoleV1::CiphertextC1,
                Some(record),
                limb,
                ordinal,
            ));
            ordinal += 1;
        }
    }
    assert_eq!(ordinal, PUBLIC_POLYNOMIAL_OBJECTS_V1);
    ManifestPartsV1 {
        public_a,
        public_b,
        ciphertext_c0,
        ciphertext_c1,
    }
}

fn manifest_from_parts_v1(
    parts: ManifestPartsV1,
) -> Result<RnsNativePublicPolynomialManifestV1, RnsNativePublicPolynomialReaderErrorV1> {
    RnsNativePublicPolynomialManifestV1::new(
        parts.public_a.into_boxed_slice(),
        parts.public_b.into_boxed_slice(),
        parts.ciphertext_c0.into_boxed_slice(),
        parts.ciphertext_c1.into_boxed_slice(),
    )
}

fn encode_coefficients_v1(coefficients: &[u64]) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(coefficients.len() * RESIDUE_BYTES_V1);
    for coefficient in coefficients {
        encoded.extend_from_slice(&coefficient.to_be_bytes());
    }
    encoded
}

fn naive_evaluation_v1(coefficients: &[u64], point: u64, modulus: u64) -> u64 {
    let mut value = 0_u64;
    let mut power = 1_u64;
    for coefficient in coefficients {
        value = mod_add_v1(value, mod_mul_v1(*coefficient, power, modulus), modulus);
        power = mod_mul_v1(power, point, modulus);
    }
    value
}

fn production_object_bytes_v1(limb: usize, terms: &[(usize, u64)]) -> Vec<u8> {
    let modulus = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[limb];
    let mut bytes =
        vec![0_u8; usize::try_from(LIMB_OBJECT_BYTES_V1).expect("limb object length fits usize")];
    bytes[..LIMB_COUNT_PREFIX_BYTES_V1].copy_from_slice(
        &u32::try_from(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1)
            .expect("ring degree fits u32")
            .to_be_bytes(),
    );
    for &(coefficient, residue) in terms {
        assert!(coefficient < ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1);
        assert!(residue <= modulus);
        let start = LIMB_COUNT_PREFIX_BYTES_V1 + coefficient * RESIDUE_BYTES_V1;
        bytes[start..start + RESIDUE_BYTES_V1].copy_from_slice(&residue.to_be_bytes());
    }
    bytes
}

fn public_a_descriptor_for_payload_v1(
    limb: usize,
    bytes: &[u8],
) -> RnsNativePublicPolynomialDescriptorV1 {
    let pointer = ZkAmsMkheDirectObjectPointerV1::from_payload(
        ZkAmsMkheDirectObjectKindV1::CollectivePublicA,
        bytes,
    )
    .expect("bounded object pointer");
    RnsNativePublicPolynomialDescriptorV1::new(
        RnsNativePublicPolynomialRoleV1::PublicA,
        None,
        limb,
        pointer,
    )
    .expect("position-bound public-A descriptor")
}

fn production_plan_v1(limb: usize) -> FivePointBlockPlanV1 {
    FivePointBlockPlanV1::new_v1(
        ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[limb],
        [2, 3, 5, 7, 11],
        ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1,
        COEFFICIENTS_PER_READ_V1,
    )
    .expect("production object plan")
}

fn sparse_evaluations_v1(
    terms: &[(usize, u64)],
    points: [u64; REPETITIONS_V1],
    modulus: u64,
) -> [u64; REPETITIONS_V1] {
    points.map(|point| {
        terms.iter().fold(0_u64, |sum, &(coefficient, residue)| {
            let exponent = u64::try_from(coefficient).expect("coefficient fits u64");
            let power = mod_pow_with_work_v1(point, exponent, modulus).0;
            mod_add_v1(sum, mod_mul_v1(residue, power, modulus), modulus)
        })
    })
}

fn object_test_reader_v1(
    mut provider: TestObjectProviderV1,
) -> RnsNativePublicPolynomialReaderV1<TestObjectProviderV1> {
    let provider_identity = provider.provider_identity().expect("provider identity");
    let snapshot_identity = provider.snapshot_identity().expect("snapshot identity");
    let mut read_set_hash = Keccak256::new();
    read_set_hash.update(b"iroha.zk-ams.v1.mkhe.public-polynomial-reader.test-object");
    RnsNativePublicPolynomialReaderV1 {
        manifest: RnsNativePublicPolynomialManifestV1 {
            public_a: Vec::new().into_boxed_slice(),
            public_b: Vec::new().into_boxed_slice(),
            ciphertext_c0: Vec::new().into_boxed_slice(),
            ciphertext_c1: Vec::new().into_boxed_slice(),
            manifest_digest: [0xc1; DIGEST_BYTES_V1],
        },
        provider,
        provider_identity,
        snapshot_identity,
        schedule_identity: None,
        next_limb: 0,
        next_repetition: 0,
        cache_limb: None,
        cache: [RnsNativePublicPolynomialEvaluationV1::UNFILLED; REPETITIONS_V1],
        scratch: [0; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1],
        objects_read: 0,
        canonical_bytes: 0,
        work: EvaluationWorkV1::default(),
        step_multiplications: 0,
        read_set_hash,
        poisoned: false,
    }
}

fn assert_object_failure_poisoned_v1(
    descriptor: RnsNativePublicPolynomialDescriptorV1,
    bytes: Vec<u8>,
    fault: ObjectProviderFaultV1,
    expected: RnsNativePublicPolynomialReaderErrorV1,
) -> RnsNativePublicPolynomialReaderV1<TestObjectProviderV1> {
    let provider = TestObjectProviderV1::new(descriptor.pointer_v1(), bytes, fault);
    let mut reader = object_test_reader_v1(provider);
    let plan = production_plan_v1(usize::from(descriptor.limb));
    assert_eq!(
        reader.take_one_object_for_test_v1(descriptor, plan),
        Err(expected)
    );
    let reads_after_failure = reader.provider.read_calls;
    let lengths_after_failure = reader.provider.object_len_calls;
    assert_eq!(
        reader.take_one_object_for_test_v1(descriptor, plan),
        Err(RnsNativePublicPolynomialReaderErrorV1::Poisoned)
    );
    assert_eq!(reader.provider.read_calls, reads_after_failure);
    assert_eq!(reader.provider.object_len_calls, lengths_after_failure);
    reader
}

fn digest_from_hex_v1(encoded: &str) -> [u8; DIGEST_BYTES_V1] {
    hex::decode(encoded)
        .expect("digest hex")
        .try_into()
        .expect("32-byte digest")
}

fn schedule_v1(points: [u64; TEST_SCHEDULE_POINT_COUNT_V1]) -> RnsNativeQpcsRelationScheduleV1 {
    RnsNativeQpcsRelationScheduleV1::test_fixture_with_binding_v1(
        [0x91; DIGEST_BYTES_V1],
        [0x92; DIGEST_BYTES_V1],
        [0x93; DIGEST_BYTES_V1],
        [0x94; DIGEST_BYTES_V1],
        points,
    )
}

#[test]
fn exact_geometry_counts_caps_and_cache_are_frozen() {
    assert_eq!(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1, 40);
    assert_eq!(LEGACY_RNS_LIMBS_V1, 38);
    assert_eq!(RECORDS_V1, 43);
    assert_eq!(REPETITIONS_V1, 5);
    assert_eq!(POLYNOMIALS_PER_LIMB_V1, 88);
    assert_eq!(PUBLIC_CIPHERTEXT_LIMB_OBJECTS_V1, 1_720);
    assert_eq!(PUBLIC_POLYNOMIAL_OBJECTS_V1, 3_520);
    assert_eq!(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1, 131_072);
    assert_eq!(LIMB_COUNT_PREFIX_BYTES_V1, 4);
    assert_eq!(RESIDUE_BYTES_V1, 8);
    assert_eq!(ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1, 8_192);
    assert_eq!(COEFFICIENTS_PER_READ_V1, 1_024);
    assert_eq!(COEFFICIENT_READS_PER_OBJECT_V1, 128);
    assert_eq!(READS_PER_LIMB_OBJECT_V1, 129);
    assert_eq!(LIMB_OBJECT_BYTES_V1, 1_048_580);
    assert_eq!(PUBLIC_POLYNOMIAL_CANONICAL_BYTES_V1, 3_691_001_600);
    assert_eq!(PUBLIC_POLYNOMIAL_POINTER_FRAME_BYTES_V1, 274_560);
    assert_eq!(PUBLIC_POLYNOMIAL_READ_CALLS_V1, 454_080);
    assert_eq!(PUBLIC_POLYNOMIAL_COEFFICIENTS_V1, 461_373_440);
    assert_eq!(PUBLIC_POLYNOMIAL_MODULAR_MULTIPLICATIONS_V1, 2_311_357_600);
    assert_eq!(PUBLIC_POLYNOMIAL_MODULAR_ADDITIONS_V1, 2_309_120_000);
    assert_eq!(PUBLIC_POLYNOMIAL_COARSE_WORK_UNITS_V1, 8_772_852_640);
    assert_eq!(
        core::mem::size_of::<RnsNativePublicPolynomialEvaluationV1>(),
        704
    );
    assert_eq!(PUBLIC_EVALUATION_CACHE_BYTES_V1, 3_520);
}

#[test]
fn descriptor_rejects_wrong_role_record_limb_and_length() {
    let wrong_role = synthetic_pointer_v1(
        RnsNativePublicPolynomialRoleV1::PublicB,
        1,
        LIMB_OBJECT_BYTES_V1,
    );
    assert_eq!(
        RnsNativePublicPolynomialDescriptorV1::new(
            RnsNativePublicPolynomialRoleV1::PublicA,
            None,
            0,
            wrong_role,
        ),
        Err(RnsNativePublicPolynomialReaderErrorV1::InvalidRole)
    );

    let wrong_length = synthetic_pointer_v1(
        RnsNativePublicPolynomialRoleV1::PublicA,
        2,
        LIMB_OBJECT_BYTES_V1 - 1,
    );
    assert_eq!(
        RnsNativePublicPolynomialDescriptorV1::new(
            RnsNativePublicPolynomialRoleV1::PublicA,
            None,
            0,
            wrong_length,
        ),
        Err(RnsNativePublicPolynomialReaderErrorV1::InvalidPointer)
    );

    let pointer = synthetic_pointer_v1(
        RnsNativePublicPolynomialRoleV1::PublicA,
        3,
        LIMB_OBJECT_BYTES_V1,
    );
    assert_eq!(
        RnsNativePublicPolynomialDescriptorV1::new(
            RnsNativePublicPolynomialRoleV1::PublicA,
            Some(0),
            0,
            pointer,
        ),
        Err(RnsNativePublicPolynomialReaderErrorV1::InvalidOrder)
    );
    assert_eq!(
        RnsNativePublicPolynomialDescriptorV1::new(
            RnsNativePublicPolynomialRoleV1::PublicA,
            None,
            ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1,
            pointer,
        ),
        Err(RnsNativePublicPolynomialReaderErrorV1::InvalidOrder)
    );

    let c0 = synthetic_pointer_v1(
        RnsNativePublicPolynomialRoleV1::CiphertextC0,
        4,
        LIMB_OBJECT_BYTES_V1,
    );
    assert_eq!(
        RnsNativePublicPolynomialDescriptorV1::new(
            RnsNativePublicPolynomialRoleV1::CiphertextC0,
            None,
            0,
            c0,
        ),
        Err(RnsNativePublicPolynomialReaderErrorV1::InvalidOrder)
    );
    assert_eq!(
        RnsNativePublicPolynomialDescriptorV1::new(
            RnsNativePublicPolynomialRoleV1::CiphertextC0,
            Some(u8::try_from(RECORDS_V1).expect("records fit u8")),
            0,
            c0,
        ),
        Err(RnsNativePublicPolynomialReaderErrorV1::InvalidOrder)
    );
}

#[test]
fn exact_manifest_is_position_bound_and_statement_addressable() {
    let parts = exact_manifest_parts_v1();
    let expected_a = parts.public_a[0].artifact_digest_v1();
    let expected_c1 =
        parts.ciphertext_c1[PUBLIC_CIPHERTEXT_LIMB_OBJECTS_V1 - 1].artifact_digest_v1();
    let manifest = manifest_from_parts_v1(parts).expect("exact manifest");
    assert_ne!(manifest.manifest_digest_v1(), [0; DIGEST_BYTES_V1]);
    assert_eq!(
        manifest.statement_artifact_digest_v1(RnsNativePublicPolynomialRoleV1::PublicA, None, 0,),
        Some(expected_a)
    );
    assert_eq!(
        manifest.statement_artifact_digest_v1(
            RnsNativePublicPolynomialRoleV1::CiphertextC1,
            Some(RECORDS_V1 - 1),
            ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 - 1,
        ),
        Some(expected_c1)
    );
    assert_eq!(
        manifest
            .statement_artifact_digest_v1(RnsNativePublicPolynomialRoleV1::PublicA, Some(0), 0,),
        None
    );
    assert_eq!(
        manifest.statement_artifact_digest_v1(
            RnsNativePublicPolynomialRoleV1::CiphertextC0,
            Some(usize::MAX),
            0,
        ),
        None
    );
    assert_eq!(
        manifest.statement_artifact_digest_v1(
            RnsNativePublicPolynomialRoleV1::PublicB,
            None,
            usize::MAX,
        ),
        None
    );
}

#[test]
fn representative_encoding_artifact_manifest_and_qpcs_digests_are_frozen() {
    assert_eq!(
        public_polynomial_encoding_digest_v1(),
        digest_from_hex_v1("b63d7fa91179d4c57550f3cb8c7727e93b4dc8e31c53da19102ceb478c4e7b36")
    );
    let parts = exact_manifest_parts_v1();
    assert_eq!(
        parts.public_a[0].artifact_digest_v1(),
        digest_from_hex_v1("04084b7081c52198553a01c000aca087c87a8b769aa4ac2219d780fccd59f4a5")
    );
    assert_eq!(
        parts.public_b[ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 - 1].artifact_digest_v1(),
        digest_from_hex_v1("3e8cd685e57bf965fe92e1aa39586f457b078f080fa2fff521349bf5eabc14af")
    );
    assert_eq!(
        parts.ciphertext_c0[0].artifact_digest_v1(),
        digest_from_hex_v1("40c130aa4e57e980f153c4a38a3a60a3bd9383a20010ee76293b505a3e9742ab")
    );
    assert_eq!(
        parts.ciphertext_c1[PUBLIC_CIPHERTEXT_LIMB_OBJECTS_V1 - 1].artifact_digest_v1(),
        digest_from_hex_v1("cb241c38bcfc786879e44338b5c2d1d1860d0c8349ca944bb4ae8ca256ce20f1")
    );
    let manifest = manifest_from_parts_v1(parts).expect("exact KAT manifest");
    assert_eq!(
        manifest.manifest_digest_v1(),
        digest_from_hex_v1("dea87b62272f8aa185ff11dd260de9789d4ca1be2d01fef4330399bbf2d197c9")
    );

    let points = core::array::from_fn(|ordinal| {
        u64::try_from(ordinal % REPETITIONS_V1 + 1).expect("test point fits u64")
    });
    let identity = QpcsScheduleIdentityV1::from_schedule_v1(&schedule_v1(points))
        .expect("canonical KAT schedule");
    assert_eq!(
        identity.binding_digest,
        digest_from_hex_v1("e07d958499fb92bbd8d7cc57728c087e6796280d00067b1095981343a0cdf9be")
    );
}

#[test]
fn legacy_38_missing_extra_limb_and_short_ciphertext_manifests_fail_closed() {
    let mut missing_new_limbs = exact_manifest_parts_v1();
    missing_new_limbs.public_a.truncate(LEGACY_RNS_LIMBS_V1);
    assert!(matches!(
        manifest_from_parts_v1(missing_new_limbs),
        Err(RnsNativePublicPolynomialReaderErrorV1::InvalidCount)
    ));

    let mut parts = exact_manifest_parts_v1();
    parts.ciphertext_c1.pop();
    assert!(matches!(
        manifest_from_parts_v1(parts),
        Err(RnsNativePublicPolynomialReaderErrorV1::InvalidCount)
    ));
}

#[test]
fn manifest_rejects_wrong_order_and_duplicate_pointers() {
    let mut wrong_order = exact_manifest_parts_v1();
    wrong_order.public_a.swap(0, 1);
    assert!(matches!(
        manifest_from_parts_v1(wrong_order),
        Err(RnsNativePublicPolynomialReaderErrorV1::InvalidOrder)
    ));

    let mut wrong_role = exact_manifest_parts_v1();
    wrong_role.public_a[0] = wrong_role.public_b[0];
    assert!(matches!(
        manifest_from_parts_v1(wrong_role),
        Err(RnsNativePublicPolynomialReaderErrorV1::InvalidRole)
    ));

    let mut duplicate = exact_manifest_parts_v1();
    let duplicate_pointer = duplicate.public_a[0].pointer_v1();
    duplicate.public_a[1] = RnsNativePublicPolynomialDescriptorV1::new(
        RnsNativePublicPolynomialRoleV1::PublicA,
        None,
        1,
        duplicate_pointer,
    )
    .expect("position-bound duplicate descriptor");
    assert!(matches!(
        manifest_from_parts_v1(duplicate),
        Err(RnsNativePublicPolynomialReaderErrorV1::DuplicatePointer)
    ));
}

#[test]
fn tiny_block_horner_matches_ascending_naive_evaluation_and_exact_work() {
    let modulus = 97_u64;
    let points = [1_u64, 2, 3, 4, 5];
    let coefficients = [3_u64, 11, 5, 17, 23, 31];
    let plan = FivePointBlockPlanV1::new_v1(modulus, points, coefficients.len(), 3)
        .expect("tiny block plan");
    assert_eq!(plan.block_count, 2);
    assert_eq!(plan.step_multiplications, 20);
    let mut evaluation = FivePointBlockEvaluationV1::new_v1(plan);
    evaluation
        .absorb_block_v1(&encode_coefficients_v1(&coefficients[..3]))
        .expect("first ascending block");
    evaluation
        .absorb_block_v1(&encode_coefficients_v1(&coefficients[3..]))
        .expect("second ascending block");
    let (values, work) = evaluation.finish_v1().expect("complete evaluation");
    let expected = points.map(|point| naive_evaluation_v1(&coefficients, point, modulus));
    assert_eq!(values, expected);
    assert_eq!(work.coefficients, 6);
    assert_eq!(work.multiplications, 45);
    assert_eq!(work.additions, 40);
}

#[test]
fn tiny_evaluator_rejects_noncanonical_residue_shape_schedule_and_incomplete_input() {
    let modulus = 97_u64;
    assert!(matches!(
        FivePointBlockPlanV1::new_v1(modulus, [1, 1, 2, 3, 4], 6, 3),
        Err(RnsNativePublicPolynomialReaderErrorV1::InvalidSchedule)
    ));
    assert!(matches!(
        FivePointBlockPlanV1::new_v1(modulus, [0, 1, 2, 3, 4], 6, 3),
        Err(RnsNativePublicPolynomialReaderErrorV1::InvalidSchedule)
    ));
    assert!(matches!(
        FivePointBlockPlanV1::new_v1(modulus, [1, 2, 3, 4, 97], 6, 3),
        Err(RnsNativePublicPolynomialReaderErrorV1::InvalidSchedule)
    ));
    assert!(matches!(
        FivePointBlockPlanV1::new_v1(modulus, [1, 2, 3, 4, 5], 7, 3),
        Err(RnsNativePublicPolynomialReaderErrorV1::InvalidSchedule)
    ));

    let plan =
        FivePointBlockPlanV1::new_v1(modulus, [1, 2, 3, 4, 5], 3, 3).expect("one-block plan");
    let mut noncanonical = FivePointBlockEvaluationV1::new_v1(plan);
    assert_eq!(
        noncanonical.absorb_block_v1(&encode_coefficients_v1(&[1, modulus, 2])),
        Err(RnsNativePublicPolynomialReaderErrorV1::NonCanonicalCoefficient)
    );
    let mut short = FivePointBlockEvaluationV1::new_v1(plan);
    assert_eq!(
        short.absorb_block_v1(&encode_coefficients_v1(&[1, 2])),
        Err(RnsNativePublicPolynomialReaderErrorV1::InvalidCount)
    );
    assert!(matches!(
        FivePointBlockEvaluationV1::new_v1(plan).finish_v1(),
        Err(RnsNativePublicPolynomialReaderErrorV1::Incomplete)
    ));
}

#[test]
fn production_block_step_has_exact_binary_exponentiation_work() {
    let modulus = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[0];
    let points = [1_u64, 2, 3, 4, 5];
    let plan = FivePointBlockPlanV1::new_v1(
        modulus,
        points,
        ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1,
        COEFFICIENTS_PER_READ_V1,
    )
    .expect("production geometry plan");
    assert_eq!(plan.block_count, 128);
    assert_eq!(
        plan.step_multiplications,
        REPETITIONS_V1 as u64 * BLOCK_STEP_MULTIPLICATIONS_PER_POINT_V1
    );
    for (point, step) in points.into_iter().zip(plan.block_steps) {
        let (expected, work) = mod_pow_with_work_v1(point, 1_024, modulus);
        assert_eq!(step, expected);
        assert_eq!(work, BLOCK_STEP_MULTIPLICATIONS_PER_POINT_V1);
    }
}

#[test]
fn canonical_production_object_authenticates_and_evaluates_sparse_block_boundaries() {
    let limb = 0_usize;
    let modulus = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[limb];
    let terms = [
        (0_usize, 3_u64),
        (1_023, 5),
        (1_024, 7),
        (ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 - 1, modulus - 1),
    ];
    let bytes = production_object_bytes_v1(limb, &terms);
    assert_eq!(&bytes[..4], &[0, 2, 0, 0]);
    for &(coefficient, residue) in &terms {
        let start = LIMB_COUNT_PREFIX_BYTES_V1 + coefficient * RESIDUE_BYTES_V1;
        assert_eq!(
            &bytes[start..start + RESIDUE_BYTES_V1],
            &residue.to_be_bytes()
        );
    }
    let descriptor = public_a_descriptor_for_payload_v1(limb, &bytes);
    let provider =
        TestObjectProviderV1::new(descriptor.pointer_v1(), bytes, ObjectProviderFaultV1::None);
    let mut reader = object_test_reader_v1(provider);
    let plan = production_plan_v1(limb);
    let values = reader
        .take_one_object_for_test_v1(descriptor, plan)
        .expect("canonical object authenticates");
    assert_eq!(
        values,
        sparse_evaluations_v1(&terms, [2, 3, 5, 7, 11], modulus)
    );
    assert!(!reader.poisoned);
    assert_eq!(reader.provider.read_calls, READS_PER_LIMB_OBJECT_V1);
    assert_eq!(reader.objects_read, 1);
    assert_eq!(reader.canonical_bytes, LIMB_OBJECT_BYTES_V1);
    assert_eq!(
        reader.work,
        EvaluationWorkV1 {
            coefficients: ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 as u64,
            multiplications: MODULAR_MULTIPLICATIONS_PER_OBJECT_V1,
            additions: MODULAR_ADDITIONS_PER_OBJECT_V1,
        }
    );
}

#[test]
fn malformed_or_drifting_production_objects_fail_once_then_remain_poisoned() {
    let limb = 0_usize;
    let modulus = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[limb];
    let canonical = production_object_bytes_v1(
        limb,
        &[
            (0, 3),
            (1_023, 5),
            (1_024, 7),
            (ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 - 1, modulus - 1),
        ],
    );
    let canonical_descriptor = public_a_descriptor_for_payload_v1(limb, &canonical);

    let wrong_length = assert_object_failure_poisoned_v1(
        canonical_descriptor,
        canonical.clone(),
        ObjectProviderFaultV1::WrongLength,
        RnsNativePublicPolynomialReaderErrorV1::Authentication,
    );
    assert_eq!(wrong_length.provider.read_calls, 0);

    let short = assert_object_failure_poisoned_v1(
        canonical_descriptor,
        canonical.clone(),
        ObjectProviderFaultV1::ShortReadAt(2),
        RnsNativePublicPolynomialReaderErrorV1::Authentication,
    );
    assert_eq!(short.provider.read_calls, 2);

    let provider_drift = assert_object_failure_poisoned_v1(
        canonical_descriptor,
        canonical.clone(),
        ObjectProviderFaultV1::ProviderDriftAfterRead(1),
        RnsNativePublicPolynomialReaderErrorV1::Authentication,
    );
    assert_eq!(provider_drift.provider.read_calls, 1);

    let snapshot_drift = assert_object_failure_poisoned_v1(
        canonical_descriptor,
        canonical.clone(),
        ObjectProviderFaultV1::SnapshotDriftAfterRead(1),
        RnsNativePublicPolynomialReaderErrorV1::Authentication,
    );
    assert_eq!(snapshot_drift.provider.read_calls, 1);

    let mut wrong_hash = canonical_descriptor.pointer_v1().payload_blake3();
    wrong_hash[0] ^= 0x80;
    let wrong_hash_pointer = ZkAmsMkheDirectObjectPointerV1::new(
        ZkAmsMkheDirectObjectKindV1::CollectivePublicA,
        LIMB_OBJECT_BYTES_V1,
        wrong_hash,
    )
    .expect("canonical wrong-hash pointer frame");
    let wrong_hash_descriptor = RnsNativePublicPolynomialDescriptorV1::new(
        RnsNativePublicPolynomialRoleV1::PublicA,
        None,
        limb,
        wrong_hash_pointer,
    )
    .expect("position-bound wrong-hash descriptor");
    let late_auth = assert_object_failure_poisoned_v1(
        wrong_hash_descriptor,
        canonical.clone(),
        ObjectProviderFaultV1::None,
        RnsNativePublicPolynomialReaderErrorV1::Authentication,
    );
    assert_eq!(late_auth.provider.read_calls, READS_PER_LIMB_OBJECT_V1);

    let mut wrong_count = canonical.clone();
    wrong_count[..4].copy_from_slice(
        &u32::try_from(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1)
            .expect("degree fits u32")
            .to_le_bytes(),
    );
    let wrong_count_descriptor = public_a_descriptor_for_payload_v1(limb, &wrong_count);
    let wrong_count_reader = assert_object_failure_poisoned_v1(
        wrong_count_descriptor,
        wrong_count,
        ObjectProviderFaultV1::None,
        RnsNativePublicPolynomialReaderErrorV1::InvalidCount,
    );
    assert_eq!(wrong_count_reader.provider.read_calls, 1);

    let noncanonical =
        production_object_bytes_v1(limb, &[(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 - 1, modulus)]);
    let noncanonical_descriptor = public_a_descriptor_for_payload_v1(limb, &noncanonical);
    let noncanonical_reader = assert_object_failure_poisoned_v1(
        noncanonical_descriptor,
        noncanonical,
        ObjectProviderFaultV1::None,
        RnsNativePublicPolynomialReaderErrorV1::NonCanonicalCoefficient,
    );
    assert_eq!(
        noncanonical_reader.provider.read_calls,
        READS_PER_LIMB_OBJECT_V1
    );
}

#[test]
fn captured_qpcs_identity_binds_all_200_points_and_rejects_invalid_limb_schedules() {
    let points = core::array::from_fn(|ordinal| {
        u64::try_from(ordinal % REPETITIONS_V1 + 1).expect("test point fits u64")
    });
    let identity = QpcsScheduleIdentityV1::from_schedule_v1(&schedule_v1(points))
        .expect("canonical test schedule");
    assert_ne!(identity.binding_digest, [0; DIGEST_BYTES_V1]);

    let mut substituted = points;
    substituted[TEST_SCHEDULE_POINT_COUNT_V1 - 1] = 6;
    let substituted_identity = QpcsScheduleIdentityV1::from_schedule_v1(&schedule_v1(substituted))
        .expect("canonical substituted schedule");
    assert_ne!(identity, substituted_identity);

    let mut duplicate = points;
    duplicate[1] = duplicate[0];
    assert_eq!(
        QpcsScheduleIdentityV1::from_schedule_v1(&schedule_v1(duplicate)),
        Err(RnsNativePublicPolynomialReaderErrorV1::InvalidSchedule)
    );

    let mut noncanonical = points;
    noncanonical[0] = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[0];
    assert_eq!(
        QpcsScheduleIdentityV1::from_schedule_v1(&schedule_v1(noncanonical)),
        Err(RnsNativePublicPolynomialReaderErrorV1::InvalidSchedule)
    );
}

#[test]
fn bounded_limb_spy_pins_runtime_order_and_withholds_evaluation_until_object_88() {
    let points = core::array::from_fn(|ordinal| {
        u64::try_from(ordinal % REPETITIONS_V1 + 1).expect("test point fits u64")
    });
    let schedule = schedule_v1(points);
    let manifest = manifest_from_parts_v1(exact_manifest_parts_v1()).expect("exact manifest");
    let mut reader =
        RnsNativePublicPolynomialReaderV1::new(manifest, IdentityOnlyProviderV1::default())
            .expect("identity-only reader");
    let mut observed = Vec::with_capacity(POLYNOMIALS_PER_LIMB_V1);
    let result = reader.take_next_evaluation_with_evaluator_v1(
        &schedule,
        0,
        0,
        |reader, descriptor, _plan| {
            assert_eq!(reader.cache_limb, None);
            observed.push((descriptor.role, descriptor.record, descriptor.limb));
            if observed.len() == POLYNOMIALS_PER_LIMB_V1 {
                return Err(RnsNativePublicPolynomialReaderErrorV1::Authentication);
            }
            Ok([u64::try_from(observed.len()).expect("slot fits u64"); REPETITIONS_V1])
        },
    );
    assert_eq!(
        result,
        Err(RnsNativePublicPolynomialReaderErrorV1::Authentication)
    );
    assert!(reader.poisoned);
    assert_eq!(reader.cache_limb, None);
    assert_eq!(reader.provider.object_calls, 0);

    let mut expected = Vec::with_capacity(POLYNOMIALS_PER_LIMB_V1);
    expected.push((RnsNativePublicPolynomialRoleV1::PublicA, None, 0));
    expected.push((RnsNativePublicPolynomialRoleV1::PublicB, None, 0));
    for record in 0..RECORDS_V1 {
        let record = Some(u8::try_from(record).expect("record fits u8"));
        expected.push((RnsNativePublicPolynomialRoleV1::CiphertextC0, record, 0));
        expected.push((RnsNativePublicPolynomialRoleV1::CiphertextC1, record, 0));
    }
    assert_eq!(observed, expected);

    let manifest = manifest_from_parts_v1(exact_manifest_parts_v1()).expect("exact manifest");
    let mut reader =
        RnsNativePublicPolynomialReaderV1::new(manifest, IdentityOnlyProviderV1::default())
            .expect("identity-only reader");
    let mut authenticated = 0_usize;
    let evaluation = reader
        .take_next_evaluation_with_evaluator_v1(&schedule, 0, 0, |reader, _descriptor, _plan| {
            assert_eq!(reader.cache_limb, None);
            authenticated += 1;
            let slot = u64::try_from(authenticated).expect("slot fits u64") * 100;
            Ok(core::array::from_fn(|repetition| {
                slot + u64::try_from(repetition).expect("repetition fits u64")
            }))
        })
        .expect("all 88 authenticated objects release one evaluation");
    assert_eq!(authenticated, POLYNOMIALS_PER_LIMB_V1);
    assert_eq!(evaluation.public_a, 100);
    assert_eq!(evaluation.public_b, 200);
    assert_eq!(evaluation.ciphertext_c0[0], 300);
    assert_eq!(evaluation.ciphertext_c1[0], 400);
    assert_eq!(evaluation.ciphertext_c0[RECORDS_V1 - 1], 8_700);
    assert_eq!(evaluation.ciphertext_c1[RECORDS_V1 - 1], 8_800);
    assert!(!reader.poisoned);
    assert_eq!(reader.cache_limb, Some(0));
    assert_eq!(reader.next_repetition, 1);
    assert_eq!(reader.provider.object_calls, 0);
}

#[test]
fn source_contract_is_move_only_poisoned_bounded_and_authenticates_before_escape() {
    let source = include_str!("rns_native_public_polynomial_reader.rs");
    for declaration in [
        "pub(super) struct RnsNativePublicPolynomialManifestV1",
        "pub(super) struct RnsNativePublicPolynomialReaderV1<P>",
        "pub(super) struct RnsNativePublicPolynomialReadReceiptV1",
    ] {
        let offset = source.find(declaration).expect("owner declaration");
        let attributes = &source[offset.saturating_sub(240)..offset];
        assert!(!attributes.contains("derive(Clone"));
        assert!(!attributes.contains("derive(Copy"));
    }
    assert!(source.contains("P: ZkAmsMkheDirectObjectReadAtProviderV1"));
    assert!(source.contains("qpcs_schedule_digest: [u8; DIGEST_BYTES_V1]"));
    assert!(source.contains("self.read_set_hash.update(&schedule_identity.binding_digest)"));
    assert!(source.contains("ZkAmsMkheDirectObjectReadTransactionV1::begin"));
    assert!(source.contains("transaction.remaining_bytes() != 0"));
    assert!(source.contains("usize::try_from(u32::from_be_bytes(count)).ok()"));
    assert!(source.contains("scratch: [u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1]"));
    assert_eq!(
        source
            .matches("cache: [RnsNativePublicPolynomialEvaluationV1; REPETITIONS_V1]",)
            .count(),
        1
    );
    assert!(!source.contains("let mut cache ="));

    let poison = source
        .find("self.poisoned = true;")
        .expect("pre-call poison");
    let call = source[poison..]
        .find("let result = self.take_next_evaluation_inner_v1")
        .map(|offset| poison + offset)
        .expect("fallible inner call");
    let clear = source[call..]
        .find("if result.is_ok()")
        .map(|offset| call + offset)
        .expect("success-only poison clear");
    assert!(poison < call && call < clear);

    let read_start = source
        .find("fn read_and_evaluate_object_v1(")
        .expect("object read function");
    let read_end = source[read_start..]
        .find("fn validate_and_absorb_receipt_v1(")
        .map(|offset| read_start + offset)
        .expect("receipt function");
    let read = &source[read_start..read_end];
    let finish = read
        .find(".finish(&mut self.provider)")
        .expect("complete content transaction");
    let receipt = read
        .find("self.validate_and_absorb_receipt_v1")
        .expect("authenticated receipt validation");
    let values = read.rfind("Ok(values)").expect("value release");
    assert!(finish < receipt && receipt < values);

    let prepare_start = source
        .find("fn prepare_limb_with_evaluator_v1<F>(")
        .expect("limb prepare");
    let prepare_end = source[prepare_start..]
        .find("fn required_descriptor_v1(")
        .map(|offset| prepare_start + offset)
        .expect("limb prepare end");
    let prepare = &source[prepare_start..prepare_end];
    assert!(
        prepare
            .find("for record in 0..RECORDS_V1")
            .expect("all records")
            < prepare
                .find("self.cache_limb = Some(limb)")
                .expect("cache commit")
    );
    assert!(prepare.contains("self.cache\n            .fill"));
}

#[test]
fn source_is_settled_but_integration_and_release_remain_explicitly_false() {
    let delta =
        core::str::from_utf8(RNS_NATIVE_PUBLIC_POLYNOMIAL_READER_REMAINING_INTEGRATION_DELTA_V1)
            .expect("integration delta is UTF-8");
    for required in [
        "40-limb-phase23-publication-owner",
        "replace-the-detached-RnsNativePublicArtifactViewV1-preflight-input",
        "derive-every-source-statement-limb-identity-from-descriptor-artifact_digest_v1",
        "thread-provider-P-through-RnsNativeRlweSourceStatementStageV1",
        "bind-qpcs_schedule_digest-and-read_set_digest-into-the-source-terminal-token",
        "purpose-specific-sealed-direct-source-transition",
        "remove-production-caller-supplied-a-A-B-C0-C1-qpcs-numeric-values",
        "measured-rss",
        "keep-composite-readiness-and-release-false",
    ] {
        assert!(
            delta.contains(required),
            "missing integration delta: {required}"
        );
    }

    let source = include_str!("rns_native_public_polynomial_reader.rs");
    for required in [
        "u32(131072)-big-endian || 131072*u64-big-endian",
        "coefficient-domain;ascending-c0-through-c131071",
        "strict-residue-less-than-position-modulus;no-reduction;no-ntt-order",
        "A[40]-then-B[40]-then-C0[record-major-43][limb-major-40]-then-C1",
        "all-3520-pointer-digests-distinct;legacy-38-and-missing-new-limbs-rejected",
        "bind-all-200-points-in-read-receipt",
        "8192-byte-maximum-read;3520-byte-evaluation-cache",
    ] {
        assert!(
            source.contains(required),
            "missing frozen semantic: {required}"
        );
    }

    let parent = include_str!("../mkhe.rs");
    assert!(parent.contains("mod rns_native_public_polynomial_reader;"));
}
