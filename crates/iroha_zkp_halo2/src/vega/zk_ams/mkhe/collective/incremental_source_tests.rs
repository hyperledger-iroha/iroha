use super::super::tests::{
    KatRandom, encrypt_test_with_opening, test_canonical_plaintext, test_input_topology, test_key,
    test_profile,
};
use super::*;
use crate::vega::MaskedRelaxedRandomErrorV1;
struct FailingRandom;
impl MaskedRelaxedRandomSourceV1 for FailingRandom {
    fn fill_bytes(&mut self, _destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
        Err(MaskedRelaxedRandomErrorV1::Unavailable)
    }
}
struct TestStageV1 {
    staging_identity: [u8; 32],
    seal_identity: Option<[u8; 32]>,
    kind: ZkAmsMkheDirectObjectKindV1,
    payload_bytes: u64,
    bytes: Vec<u8>,
}
struct TestPublishedV1 {
    pointer: ZkAmsMkheDirectObjectPointerV1,
    published_object_identity: [u8; 32],
    bytes: Vec<u8>,
}
struct TestStreamingCasV1 {
    provider_identity: [u8; 32],
    snapshot_identity: [u8; 32],
    publication_identity: [u8; 32],
    next_identity: u64,
    stages: Vec<TestStageV1>,
    published: Vec<TestPublishedV1>,
    read_calls: usize,
    fail_read_at: Option<usize>,
}
impl TestStreamingCasV1 {
    fn new(domain: u8) -> Self {
        Self {
            provider_identity: [domain; 32],
            snapshot_identity: [domain.wrapping_add(1); 32],
            publication_identity: [domain.wrapping_add(2); 32],
            next_identity: 1,
            stages: Vec::new(),
            published: Vec::new(),
            read_calls: 0,
            fail_read_at: None,
        }
    }
    fn next_distinct_identity(&mut self, domain: u8) -> [u8; 32] {
        let mut identity = [domain; 32];
        identity[24..].copy_from_slice(&self.next_identity.to_be_bytes());
        self.next_identity += 1;
        identity
    }
    fn published_bytes(&self, pointer: ZkAmsMkheDirectObjectPointerV1) -> &[u8] {
        &self
            .published
            .iter()
            .find(|published| published.pointer == pointer)
            .expect("test pointer must be published")
            .bytes
    }
    fn decode_limb(&self, pointer: ZkAmsMkheDirectObjectPointerV1) -> Vec<u64> {
        let bytes = self.published_bytes(pointer);
        let count = u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize;
        let values = bytes[4..]
            .chunks_exact(8)
            .map(|encoded| u64::from_be_bytes(encoded.try_into().unwrap()))
            .collect::<Vec<_>>();
        assert_eq!(values.len(), count);
        values
    }
}
impl ZkAmsMkheDirectObjectReadAtProviderV1 for TestStreamingCasV1 {
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
        self.published
            .iter()
            .find(|published| published.pointer == pointer)
            .and_then(|published| u64::try_from(published.bytes.len()).ok())
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    }
    fn read_at(
        &mut self,
        pointer: ZkAmsMkheDirectObjectPointerV1,
        absolute_offset: u64,
        destination: &mut [u8],
    ) -> Result<usize, ZkAmsMkheErrorV1> {
        self.read_calls += 1;
        if self.fail_read_at == Some(self.read_calls) {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let published = self
            .published
            .iter()
            .find(|published| published.pointer == pointer)
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        let start = usize::try_from(absolute_offset)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let end = start
            .checked_add(destination.len())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let source = published
            .bytes
            .get(start..end)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        destination.copy_from_slice(source);
        Ok(destination.len())
    }
}
impl ZkAmsMkheDirectObjectCasPublicationV1 for TestStreamingCasV1 {
    fn publication_identity(&mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        Ok(self.publication_identity)
    }
    fn begin_staging(
        &mut self,
        kind: ZkAmsMkheDirectObjectKindV1,
        payload_bytes: u64,
    ) -> Result<ZkAmsMkheDirectObjectStagingTokenV1, ZkAmsMkheErrorV1> {
        let staging_identity = self.next_distinct_identity(0xa1);
        self.stages.push(TestStageV1 {
            staging_identity,
            seal_identity: None,
            kind,
            payload_bytes,
            bytes: Vec::new(),
        });
        ZkAmsMkheDirectObjectStagingTokenV1::new(
            self.publication_identity,
            staging_identity,
            kind,
            payload_bytes,
        )
    }
    fn staged_len(
        &mut self,
        staging: &ZkAmsMkheDirectObjectStagingTokenV1,
    ) -> Result<u64, ZkAmsMkheErrorV1> {
        self.stages
            .iter()
            .find(|candidate| candidate.staging_identity == staging.staging_identity())
            .and_then(|candidate| u64::try_from(candidate.bytes.len()).ok())
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    }
    fn write_staged_at(
        &mut self,
        staging: &ZkAmsMkheDirectObjectStagingTokenV1,
        absolute_offset: u64,
        source: &[u8],
    ) -> Result<usize, ZkAmsMkheErrorV1> {
        let candidate = self
            .stages
            .iter_mut()
            .find(|candidate| candidate.staging_identity == staging.staging_identity())
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        if usize::try_from(absolute_offset).ok() != Some(candidate.bytes.len())
            || candidate.seal_identity.is_some()
            || candidate.kind != staging.kind()
            || candidate.payload_bytes != staging.payload_bytes()
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        candidate.bytes.extend_from_slice(source);
        Ok(source.len())
    }
    fn seal_staged(
        &mut self,
        staging: ZkAmsMkheDirectObjectStagingTokenV1,
    ) -> Result<ZkAmsMkheDirectObjectSealTokenV1, ZkAmsMkheErrorV1> {
        let seal_identity = self.next_distinct_identity(0xb1);
        let candidate = self
            .stages
            .iter_mut()
            .find(|candidate| candidate.staging_identity == staging.staging_identity())
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        if candidate.seal_identity.is_some()
            || u64::try_from(candidate.bytes.len()).ok() != Some(candidate.payload_bytes)
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        candidate.seal_identity = Some(seal_identity);
        ZkAmsMkheDirectObjectSealTokenV1::from_staging(staging, seal_identity)
    }
    fn sealed_len(
        &mut self,
        seal: &ZkAmsMkheDirectObjectSealTokenV1,
    ) -> Result<u64, ZkAmsMkheErrorV1> {
        self.stages
            .iter()
            .find(|candidate| candidate.seal_identity == Some(seal.seal_identity()))
            .and_then(|candidate| u64::try_from(candidate.bytes.len()).ok())
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    }
    fn read_sealed_at(
        &mut self,
        seal: &ZkAmsMkheDirectObjectSealTokenV1,
        absolute_offset: u64,
        destination: &mut [u8],
    ) -> Result<usize, ZkAmsMkheErrorV1> {
        let candidate = self
            .stages
            .iter()
            .find(|candidate| candidate.seal_identity == Some(seal.seal_identity()))
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        let start = usize::try_from(absolute_offset)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let end = start
            .checked_add(destination.len())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        destination.copy_from_slice(
            candidate
                .bytes
                .get(start..end)
                .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?,
        );
        Ok(destination.len())
    }
    fn publish_sealed_by_pointer(
        &mut self,
        seal: &ZkAmsMkheDirectObjectSealTokenV1,
        pointer: ZkAmsMkheDirectObjectPointerV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if self
            .published
            .iter()
            .any(|published| published.pointer == pointer)
        {
            return Ok(());
        }
        let bytes = self
            .stages
            .iter()
            .find(|candidate| candidate.seal_identity == Some(seal.seal_identity()))
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?
            .bytes
            .clone();
        let published_object_identity = self.next_distinct_identity(0xc1);
        self.published.push(TestPublishedV1 {
            pointer,
            published_object_identity,
            bytes,
        });
        Ok(())
    }
    fn lookup_published_pointer(
        &mut self,
        pointer: ZkAmsMkheDirectObjectPointerV1,
    ) -> Result<Option<ZkAmsMkheDirectObjectPublishedBindingV1>, ZkAmsMkheErrorV1> {
        self.published
            .iter()
            .find(|published| published.pointer == pointer)
            .map(|published| {
                ZkAmsMkheDirectObjectPublishedBindingV1::new(
                    self.publication_identity,
                    published.published_object_identity,
                    pointer,
                )
            })
            .transpose()
    }
}
struct CountingKatRandomV1 {
    inner: KatRandom,
    calls: usize,
}
impl CountingKatRandomV1 {
    fn new(label: &'static [u8]) -> Self {
        Self {
            inner: KatRandom::new(label),
            calls: 0,
        }
    }
}
impl MaskedRelaxedRandomSourceV1 for CountingKatRandomV1 {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
        self.calls += 1;
        self.inner.fill_bytes(destination)
    }
}
fn test_streaming_key_authority_v1(
    profile: &BgvProfile,
    key: &ZkAmsMkheCollectivePublicKeyV1,
    store: &mut TestStreamingCasV1,
    sample_index: u64,
) -> ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1 {
    let limbs = profile.moduli.len();
    let mut public_a_limb_pointers = try_streaming_vec_with_capacity_v1(limbs).unwrap();
    let mut public_b_limb_pointers = try_streaming_vec_with_capacity_v1(limbs).unwrap();
    let mut public_a_publication_receipts = try_streaming_vec_with_capacity_v1(limbs).unwrap();
    let mut public_b_publication_receipts = try_streaming_vec_with_capacity_v1(limbs).unwrap();
    let mut scratch = [0_u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1];
    for limb in 0..limbs {
        let receipt = publish_streaming_collective_limb_v1(
            ZkAmsMkheDirectObjectKindV1::CollectivePublicA,
            key.public_a.limb(profile, limb),
            store,
            &mut scratch,
        )
        .unwrap();
        public_a_limb_pointers.push(receipt.pointer());
        public_a_publication_receipts.push(receipt);
    }
    for limb in 0..limbs {
        let receipt = publish_streaming_collective_limb_v1(
            ZkAmsMkheDirectObjectKindV1::CollectivePublicB,
            key.collective_public_b.limb(profile, limb),
            store,
            &mut scratch,
        )
        .unwrap();
        public_b_limb_pointers.push(receipt.pointer());
        public_b_publication_receipts.push(receipt);
    }
    let binding = ZkAmsMkheStreamingCollectiveKeyBindingV1::from_validated_native_key_v1(
        key,
        profile,
        public_a_limb_pointers,
        public_b_limb_pointers,
    )
    .unwrap();
    let authority_digest = streaming_collective_key_authority_digest_v1(
        &binding,
        &public_a_publication_receipts,
        &public_b_publication_receipts,
        profile,
    )
    .unwrap();
    let authority = ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1 {
        _seal: StreamingCollectiveEncryptionKeyAuthoritySealV1,
        binding,
        public_a_publication_receipts,
        public_b_publication_receipts,
        authority_digest,
        next_sample_index: sample_index,
        failed: false,
    };
    authority.validate_for_profile_v1(profile).unwrap();
    authority
}
#[test]
fn component_major_incremental_key_digest_matches_native_and_rejects_order_drift() {
    let profile = test_profile();
    let (key, _) = test_key(0xa4);
    let mut early = ComponentMajorCollectivePublicKeyDigestV1::new(&key, &profile).unwrap();
    assert_eq!(
        early
            .absorb_next_collective_public_b_limb_v1(0, key.collective_public_b.limb(&profile, 0),),
        Err(ZkAmsMkheErrorV1::InvalidPolynomial)
    );
    assert_eq!(
        early.finish(&key.share_digests),
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    );
    let mut incremental = ComponentMajorCollectivePublicKeyDigestV1::new(&key, &profile).unwrap();
    assert_eq!(
        incremental.absorb_next_public_a_limb_v1(1, key.public_a.limb(&profile, 1)),
        Err(ZkAmsMkheErrorV1::InvalidPolynomial)
    );
    assert_eq!(
        incremental.absorb_next_public_a_limb_v1(
            0,
            &key.public_a.limb(&profile, 0)[..profile.ring_degree - 1],
        ),
        Err(ZkAmsMkheErrorV1::InvalidPolynomial)
    );
    let mut noncanonical = key.public_a.limb(&profile, 0).to_vec();
    noncanonical[0] = profile.moduli[0];
    assert_eq!(
        incremental.absorb_next_public_a_limb_v1(0, &noncanonical),
        Err(ZkAmsMkheErrorV1::InvalidPolynomial)
    );
    for limb in 0..profile.moduli.len() {
        incremental
            .absorb_next_public_a_limb_v1(limb, key.public_a.limb(&profile, limb))
            .unwrap();
    }
    assert_eq!(
        incremental.absorb_next_public_a_limb_v1(0, key.public_a.limb(&profile, 0)),
        Err(ZkAmsMkheErrorV1::InvalidPolynomial)
    );
    for limb in 0..profile.moduli.len() {
        incremental
            .absorb_next_collective_public_b_limb_v1(
                limb,
                key.collective_public_b.limb(&profile, limb),
            )
            .unwrap();
    }
    let incremental_digest = incremental.finish(&key.share_digests).unwrap();
    assert_eq!(incremental_digest, key.digest);
    assert_eq!(
        incremental_digest,
        collective_public_key_digest(&key, &profile).unwrap()
    );
    let release = release_profile_v1();
    let release_flat_coefficient_count = release.ring_degree * release.moduli.len();
    assert_eq!(release_flat_coefficient_count, 4_980_736);
    assert_eq!(
        u32::try_from(release_flat_coefficient_count)
            .unwrap()
            .to_be_bytes(),
        [0x00, 0x4c, 0x00, 0x00]
    );
}
#[test]
fn two_limb_incremental_encryption_matches_native_bytes_digest_and_nonce_lineage() {
    let profile = test_profile();
    let (key, _) = test_key(0xa5);
    let values = [0, 1, 2, 3, 5, 8, 13, 16];
    let sample_index = 37;
    let label = b"two-limb-incremental-parity";
    let (ciphertext, opening, _message, canonical, topology, transcript_digest) =
        encrypt_test_with_opening(&profile, &key, &values, sample_index, label);
    let mut kernel = IncrementalCollectiveEncryptionKernelV1::new_validated_inner_v1(
        &profile,
        &key,
        &canonical,
        topology,
        sample_index,
        &mut KatRandom::new(label),
    )
    .unwrap();
    assert_eq!(
        kernel.input_identity.encryption_nonce.as_bytes(),
        opening.input_identity.encryption_nonce.as_bytes()
    );
    assert_eq!(kernel.ephemeral.as_slice(), opening.ephemeral.coefficients);
    assert_eq!(
        kernel.error_zero.as_slice(),
        opening.error_zero.coefficients
    );
    assert_eq!(kernel.error_one.as_slice(), opening.error_one.coefficients);
    assert!(matches!(
        kernel.absorb_next_linear_limb_v1(0),
        Err(ZkAmsMkheErrorV1::InvalidCiphertext)
    ));
    for limb in 0..profile.moduli.len() {
        let filled = kernel.absorb_next_constant_limb_v1(limb).unwrap();
        assert_eq!(filled.component(), CollectiveRnsComponentV1::First);
        assert_eq!(filled.limb(), limb);
        assert_eq!(filled.modulus(), profile.moduli[limb]);
        assert_eq!(
            filled.coefficients(),
            ciphertext.constant().limb(&profile, limb)
        );
    }
    assert!(matches!(
        kernel.absorb_next_constant_limb_v1(0),
        Err(ZkAmsMkheErrorV1::InvalidCiphertext)
    ));
    for limb in 0..profile.moduli.len() {
        let filled = kernel.absorb_next_linear_limb_v1(limb).unwrap();
        assert_eq!(filled.component(), CollectiveRnsComponentV1::Second);
        assert_eq!(filled.limb(), limb);
        assert_eq!(filled.modulus(), profile.moduli[limb]);
        assert_eq!(
            filled.coefficients(),
            ciphertext.linear().limb(&profile, limb)
        );
    }
    let completed = kernel.finish().unwrap();
    assert_eq!(completed.transcript_digest, transcript_digest);
    assert_eq!(completed.ciphertext_digest, ciphertext.digest());
    drop(opening);
}
#[test]
fn streaming_automorphism_output_digest_matches_native_component_major_bytes() {
    let profile = test_profile();
    let (key, _) = test_key(0xa6);
    let values = [0, 1, 2, 3, 5, 8, 13, 16];
    let (ciphertext, _opening, ..) = encrypt_test_with_opening(
        &profile,
        &key,
        &values,
        39,
        b"streaming-automorphism-digest-parity",
    );
    let exponent = 3;
    let constant = ciphertext
        .constant()
        .automorphism(exponent, &profile)
        .unwrap();
    let linear = ciphertext
        .linear()
        .automorphism(exponent, &profile)
        .unwrap();
    let output_transcript_digest = [0x5a; 32];
    let native = ZkAmsMkheCollectiveCiphertextV1::new_with_key(
        &profile,
        key.parties(),
        ciphertext.epoch(),
        output_transcript_digest,
        ciphertext.sample_index(),
        0,
        constant.clone(),
        linear.clone(),
        Some(key.digest()),
    )
    .unwrap();
    let coefficient_count = profile.ring_degree * profile.moduli.len();
    let mut hash = Keccak256::new();
    hash.update(COLLECTIVE_CIPHERTEXT_DOMAIN_V1);
    hash.update(&profile.digest().unwrap());
    hash.update(&native.roster_digest());
    hash.update(&native.epoch().to_be_bytes());
    hash.update(&output_transcript_digest);
    hash.update(&native.sample_index().to_be_bytes());
    hash.update(&[0]);
    hash.update(&u32::try_from(coefficient_count).unwrap().to_be_bytes());
    let mut streaming = StreamingCollectiveAutomorphismDigestV1 {
        hash,
        ring_degree: profile.ring_degree,
        moduli: profile.moduli,
        next_component: 0,
        next_limb: 0,
    };
    for limb in 0..profile.moduli.len() {
        streaming
            .absorb_limb_v1(0, limb, constant.limb(&profile, limb))
            .unwrap();
    }
    for limb in 0..profile.moduli.len() {
        streaming
            .absorb_limb_v1(1, limb, linear.limb(&profile, limb))
            .unwrap();
    }
    assert_eq!(streaming.finish().unwrap(), native.digest());
}
#[test]
fn source_authenticated_streaming_encryption_matches_native_tiny_limbs_and_digests() {
    let profile = test_profile();
    let (key, _) = test_key(0xb5);
    let values = [0, 1, 2, 3, 5, 8, 13, 16];
    let sample_index = 47;
    let label = b"source-streaming-parity";
    let (ciphertext, opening, _message, canonical, topology, transcript_digest) =
        encrypt_test_with_opening(&profile, &key, &values, sample_index, label);
    let mut key_store = TestStreamingCasV1::new(0x51);
    let authority = test_streaming_key_authority_v1(&profile, &key, &mut key_store, sample_index);
    let prepared =
        PreparedStreamingCollectiveEncryptionV1::new_v1(&authority.binding, &profile).unwrap();
    let authenticated = prepared.authenticate_key_source_v1(&mut key_store).unwrap();
    let mut active = authenticated
        .activate_v1(
            &authority.binding,
            &canonical,
            topology,
            sample_index,
            &mut KatRandom::new(label),
        )
        .unwrap();
    assert_eq!(
        active.kernel.input_identity.encryption_nonce.as_bytes(),
        opening.input_identity.encryption_nonce.as_bytes()
    );
    assert_eq!(
        active.kernel.ephemeral.as_slice(),
        opening.ephemeral.coefficients
    );
    assert_eq!(
        active.kernel.error_zero.as_slice(),
        opening.error_zero.coefficients
    );
    assert_eq!(
        active.kernel.error_one.as_slice(),
        opening.error_one.coefficients
    );
    let mut ciphertext_store = TestStreamingCasV1::new(0x61);
    active
        .publish_all_v1(&mut key_store, &mut ciphertext_store)
        .unwrap();
    let completed = active.finish().unwrap();
    let manifest = ZkAmsMkheStreamingCollectiveCiphertextV1::from_completed_v1(
        completed,
        &authority.binding,
        authority.authority_digest,
        &profile,
    )
    .unwrap();
    manifest.validate_for_profile_v1(&profile).unwrap();
    assert_eq!(manifest.sample_index(), sample_index);
    assert_eq!(manifest.level(), 0);
    assert_eq!(manifest.transcript_digest(), transcript_digest);
    assert_eq!(manifest.ciphertext_digest(), ciphertext.digest());
    assert_eq!(
        manifest.constant_limb_pointers().len(),
        profile.moduli.len()
    );
    assert_eq!(manifest.linear_limb_pointers().len(), profile.moduli.len());
    for limb in 0..profile.moduli.len() {
        assert_eq!(
            ciphertext_store.decode_limb(manifest.constant_limb_pointers()[limb]),
            ciphertext.constant().limb(&profile, limb)
        );
        assert_eq!(
            ciphertext_store.decode_limb(manifest.linear_limb_pointers()[limb]),
            ciphertext.linear().limb(&profile, limb)
        );
    }
    let binding = manifest
        .sealed_binding_with_profile_v1(&profile)
        .expect("tiny-profile sealed binding");
    let mut destination = vec![0_u64; profile.ring_degree];
    let mut scratch = [0_u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1];
    binding
        .read_constant_limb_into_v1(
            0,
            &profile,
            &mut ciphertext_store,
            &mut destination,
            &mut scratch,
        )
        .expect("stable C0 reread");
    assert_eq!(destination, ciphertext.constant().limb(&profile, 0));
    binding
        .read_linear_limb_into_v1(
            0,
            &profile,
            &mut ciphertext_store,
            &mut destination,
            &mut scratch,
        )
        .expect("same reusable limb accepts C1");
    assert_eq!(destination, ciphertext.linear().limb(&profile, 0));
    ciphertext_store.provider_identity[0] ^= 1;
    assert_eq!(
        binding.read_constant_limb_into_v1(
            0,
            &profile,
            &mut ciphertext_store,
            &mut destination,
            &mut scratch,
        ),
        Err(ZkAmsMkheErrorV1::InvalidCiphertext)
    );
    ciphertext_store.provider_identity[0] ^= 1;
    ciphertext_store.snapshot_identity[0] ^= 1;
    assert_eq!(
        binding.read_constant_limb_into_v1(
            0,
            &profile,
            &mut ciphertext_store,
            &mut destination,
            &mut scratch,
        ),
        Err(ZkAmsMkheErrorV1::InvalidCiphertext)
    );
    ciphertext_store.snapshot_identity[0] ^= 1;
    let constant_pointer = manifest.constant_limb_pointers()[0];
    ciphertext_store
        .published
        .iter_mut()
        .find(|published| published.pointer == constant_pointer)
        .expect("published C0")
        .bytes[4] ^= 1;
    assert_eq!(
        binding.read_constant_limb_into_v1(
            0,
            &profile,
            &mut ciphertext_store,
            &mut destination,
            &mut scratch,
        ),
        Err(ZkAmsMkheErrorV1::InvalidCiphertext)
    );
    drop(opening);
}
#[test]
fn streaming_prepass_precedes_entropy_and_late_source_failure_cannot_authorize_output() {
    let profile = test_profile();
    let (key, _) = test_key(0xb6);
    let canonical = test_canonical_plaintext(&[0, 1, 2, 3, 5, 8, 13, 16]);
    let topology = test_input_topology(&profile, b"streaming-failure-order");
    let mut key_store = TestStreamingCasV1::new(0x71);
    let authority = test_streaming_key_authority_v1(&profile, &key, &mut key_store, 48);
    let prepared =
        PreparedStreamingCollectiveEncryptionV1::new_v1(&authority.binding, &profile).unwrap();
    key_store.fail_read_at = Some(key_store.read_calls + 1);
    assert!(prepared.authenticate_key_source_v1(&mut key_store).is_err());
    let untouched_random = CountingKatRandomV1::new(b"prepass-must-precede-entropy");
    let untouched_output = TestStreamingCasV1::new(0x81);
    assert_eq!(untouched_random.calls, 0);
    assert!(untouched_output.stages.is_empty());
    assert!(untouched_output.published.is_empty());
    key_store.fail_read_at = None;
    let prepared =
        PreparedStreamingCollectiveEncryptionV1::new_v1(&authority.binding, &profile).unwrap();
    let authenticated = prepared.authenticate_key_source_v1(&mut key_store).unwrap();
    let mut random = CountingKatRandomV1::new(b"late-source-failure");
    let mut active = authenticated
        .activate_v1(&authority.binding, &canonical, topology, 48, &mut random)
        .unwrap();
    assert!(random.calls > 0);
    key_store.fail_read_at = Some(key_store.read_calls + 1);
    let mut output = TestStreamingCasV1::new(0x91);
    assert!(active.publish_all_v1(&mut key_store, &mut output).is_err());
    assert!(active.kernel.poisoned);
    assert_eq!(output.stages.len(), 1);
    assert!(output.published.is_empty());
    assert!(active.records.constant_publication_receipts.is_empty());
    assert!(active.records.linear_publication_receipts.is_empty());
}
#[test]
fn two_limb_incremental_kernel_rejects_foreign_key_and_zeroizes_preallocated_drop_paths() {
    let reset_drop_audits = || {
        COLLECTIVE_ENCRYPTION_LIMB_ZEROIZED_DROPS_V1.with(|drops| drops.set(0));
        ENCRYPTION_NONCE_ZEROIZED_DROPS_V1.with(|drops| drops.set(0));
        COLLECTIVE_ENCRYPTION_WITNESS_ZEROIZED_DROPS_V1.with(|drops| drops.set(0));
    };
    let drop_audits = || {
        (
            COLLECTIVE_ENCRYPTION_LIMB_ZEROIZED_DROPS_V1.with(std::cell::Cell::get),
            ENCRYPTION_NONCE_ZEROIZED_DROPS_V1.with(std::cell::Cell::get),
            COLLECTIVE_ENCRYPTION_WITNESS_ZEROIZED_DROPS_V1.with(std::cell::Cell::get),
        )
    };
    let profile = test_profile();
    let (mut key, _) = test_key(0xa6);
    let canonical = test_canonical_plaintext(&[0, 1, 2, 3, 5, 8, 13, 16]);
    let topology = test_input_topology(&profile, b"incremental-poison");
    reset_drop_audits();
    let mut failing_random = FailingRandom;
    assert!(matches!(
        IncrementalCollectiveEncryptionKernelV1::new_validated_inner_v1(
            &profile,
            &key,
            &canonical,
            topology,
            38,
            &mut failing_random,
        ),
        Err(ZkAmsMkheErrorV1::RandomUnavailable)
    ));
    assert_eq!(drop_audits(), (2, 1, 3));
    key.digest[0] ^= 1;
    let mut healthy_random = KatRandom::new(b"incremental-foreign-key");
    assert!(
        IncrementalCollectiveEncryptionKernelV1::new_validated_inner_v1(
            &profile,
            &key,
            &canonical,
            topology,
            39,
            &mut healthy_random,
        )
        .is_err()
    );
    assert_eq!(drop_audits(), (2, 1, 3));
    let mut witness = ZeroizingCollectiveEncryptionWitnessV1::new_zeroed_v1(8).unwrap();
    assert!(witness.is_zero());
    witness.as_mut_slice()[0] = 1;
    assert!(!witness.is_zero());
    drop(witness);
    assert_eq!(
        COLLECTIVE_ENCRYPTION_WITNESS_ZEROIZED_DROPS_V1.with(std::cell::Cell::get),
        4
    );
}
#[test]
fn incremental_source_has_sealed_limb_streaming_surface_and_private_native_reference() {
    let source = include_str!("incremental_source.rs");
    assert!(source.contains("Source-authenticated, limb-streamed collective encryption"));
    assert!(source.contains("38 independently addressed `c0` limbs"));
    assert!(source.contains("38 independently addressed `c1` limbs"));
    assert!(source.contains("neither a key authority nor a ciphertext manifest is issued"));
    let prerequisite = source
        .split("// BEGIN PRIVATE INCREMENTAL COLLECTIVE ENCRYPTION PREREQUISITE V1")
        .nth(1)
        .expect("incremental prerequisite start")
        .split("// END PRIVATE INCREMENTAL COLLECTIVE ENCRYPTION PREREQUISITE V1")
        .next()
        .expect("incremental prerequisite end");
    for forbidden in [
        "plaintext_lift",
        "RnsPolynomial",
        "Vec<RnsPolynomial",
        "pub struct",
        "pub fn",
        "source_owned",
        "SourceRecord",
        "mint_",
        "impl FnOnce",
        "fill_public_component",
    ] {
        assert!(
            !prerequisite.contains(forbidden),
            "incremental prerequisite contains forbidden surface: {forbidden}"
        );
    }
    assert!(prerequisite.contains("input_identity: CollectiveEncryptionInputIdentityV1"));
    assert!(prerequisite.contains("key: &'key ZkAmsMkheCollectivePublicKeyV1"));
    assert!(prerequisite.contains("ephemeral: ZeroizingCollectiveEncryptionWitnessV1"));
    assert!(prerequisite.contains("error_zero: ZeroizingCollectiveEncryptionWitnessV1"));
    assert!(prerequisite.contains("error_one: ZeroizingCollectiveEncryptionWitnessV1"));
    assert!(prerequisite.contains("left: ZeroizingCollectiveEncryptionLimbV1"));
    assert!(prerequisite.contains("right: ZeroizingCollectiveEncryptionLimbV1"));
    assert!(prerequisite.contains("CollectiveRnsComponentV1::First"));
    assert!(prerequisite.contains("CollectiveRnsComponentV1::Second"));
    assert!(prerequisite.contains("ValidatedIncrementalCollectiveKeyV1"));
    assert!(prerequisite.contains("self.key.collective_public_b.limb"));
    assert!(prerequisite.contains("self.key.public_a.limb"));
    assert_eq!(prerequisite.matches("key.validate(profile)?").count(), 1);
    assert!(!prerequisite.contains("key.validate(&profile)?"));
    let borrowed_core = source
        .split("fn encrypt_zk_ams_mkhe_collective_packed_streaming_borrowed_with_prepublication_v1")
        .nth(1)
        .expect("private borrowed streaming encryption core")
        .split("pub fn encrypt_zk_ams_mkhe_collective_packed_streaming_v1")
        .next()
        .expect("private borrowed streaming encryption body");
    let public_path = source
        .split("pub fn encrypt_zk_ams_mkhe_collective_packed_streaming_v1")
        .nth(1)
        .expect("public streaming encryption entrypoint")
        .split("#[cfg(test)]")
        .next()
        .expect("public streaming encryption body");
    for required in [
        "PreparedStreamingCollectiveEncryptionV1::new_v1",
        "authenticate_key_source_v1",
        "activate_v1",
        "publish_all_v1",
        "authority.failed = true",
        "authority.next_sample_index = next_sample_index",
    ] {
        assert!(
            borrowed_core.contains(required),
            "missing bounded step: {required}"
        );
    }
    assert!(public_path.contains("plaintext: ZkAmsT256PackedPlaintextV1"));
    assert!(public_path.contains(
        "encrypt_zk_ams_mkhe_collective_packed_streaming_borrowed_with_prepublication_v1"
    ));
    assert!(!public_path.contains("plaintext_lift"));
    assert!(!public_path.contains("ZkAmsMkheCollectiveCiphertextV1"));
    assert!(!public_path.contains("impl FnOnce"));
    let prepared = borrowed_core
        .find("PreparedStreamingCollectiveEncryptionV1::new_v1")
        .unwrap();
    let prepass = borrowed_core.find("authenticate_key_source_v1").unwrap();
    let entropy = borrowed_core.find("activate_v1").unwrap();
    let output = borrowed_core.find("publish_all_v1").unwrap();
    assert!(prepared < prepass);
    assert!(prepass < entropy);
    assert!(entropy < output);
    let second_pass = source
        .split("fn publish_next_limb_v1")
        .nth(1)
        .expect("second-pass limb publisher")
        .split("fn write_streaming_collective_limb_coefficients_v1")
        .next()
        .expect("second-pass limb publisher end");
    assert!(second_pass.contains("read_limb_into_v1"));
    assert!(!second_pass.contains("Vec<u64>"));
    let stage = second_pass.find("PublicationTransactionV1::begin").unwrap();
    let reread = second_pass
        .find("StreamingCollectiveLimbReaderV1::begin")
        .unwrap();
    let source_finish = second_pass.find("source_reader.finish").unwrap();
    let source_compare = second_pass
        .find("validate_streaming_second_source_receipt_v1")
        .unwrap();
    let output_finish = second_pass.find("output_transaction.finish").unwrap();
    assert!(stage < reread);
    assert!(reread < source_finish);
    assert!(source_finish < source_compare);
    assert!(source_compare < output_finish);
    for kind in [
        "CollectivePublicA",
        "CollectivePublicB",
        "CollectiveCiphertextC0",
        "CollectiveCiphertextC1",
    ] {
        assert!(source.contains(kind), "missing typed limb kind: {kind}");
    }
    assert!(source.contains("StreamingCollectiveKeyAdmissionSealV1"));
    assert!(source.contains("StreamingCollectiveEvalAdmissionSealV1"));
    assert!(source.contains("StreamingCollectiveCiphertextBindingSealV1"));
    assert!(!source.contains("from_raw_digest"));
    let ciphertext_binding = source
        .split("impl ZkAmsMkheStreamingCollectiveCiphertextBindingV1<'_>")
        .nth(1)
        .expect("sealed ciphertext binding implementation")
        .split("impl core::fmt::Debug for ZkAmsMkheStreamingCollectiveCiphertextV1")
        .next()
        .expect("sealed ciphertext binding boundary");
    assert!(ciphertext_binding.contains("fn read_component_limb_into_v1<P>"));
    assert!(
        ciphertext_binding
            .contains("pub(in crate::vega::zk_ams::mkhe) fn read_constant_limb_into_v1<P>")
    );
    assert!(
        ciphertext_binding
            .contains("pub(in crate::vega::zk_ams::mkhe) fn read_linear_limb_into_v1<P>")
    );
    assert!(ciphertext_binding.contains("if receipt != *publication.post_publish_read_receipt()"));
    assert!(ciphertext_binding.contains("destination: &mut [u64]"));
    assert!(
        ciphertext_binding.contains("scratch: &mut [u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1]")
    );
    assert!(!ciphertext_binding.contains("Vec<u64>"));
    assert!(source.contains("let mut common_output_snapshot = None;"));
    assert!(
        source.contains("streaming_source_snapshot_axes_v1(receipt.post_publish_read_receipt())")
    );
    let work_gate = prerequisite
        .find("checked_ring_multiplication_work(profile, 2)?")
        .expect("two-multiplication work gate");
    let workspace = prerequisite
        .find("ZeroizingCollectiveEncryptionWorkspaceV1::new_zeroed_v1")
        .expect("two-limb zeroed workspace allocation");
    let ephemeral_owner = prerequisite
        .find("let mut ephemeral =")
        .expect("ephemeral zeroed witness allocation");
    let entropy_owners = prerequisite
        .find("} = PreallocatedCollectiveEncryptionEntropyOwnersV1::new_zeroed_v1();")
        .expect("nonce and hash owner allocation");
    let nonce = prerequisite
        .find("derive_collective_encryption_nonce_into_v1(")
        .expect("nonce derivation");
    let ephemeral = prerequisite
        .find("sample_nonzero_ternary_into_v1(profile, random, &mut ephemeral)?")
        .expect("in-place ephemeral sampling");
    let error_zero = prerequisite
        .find("sample_bounded_error_into_v1(profile, random, &mut error_zero)?")
        .expect("in-place first error sampling");
    let error_one = prerequisite
        .find("sample_bounded_error_into_v1(profile, random, &mut error_one)?")
        .expect("in-place second error sampling");
    assert!(work_gate < workspace);
    assert!(workspace < ephemeral_owner);
    assert!(ephemeral_owner < entropy_owners);
    assert!(entropy_owners < nonce);
    assert!(workspace < nonce);
    assert!(nonce < ephemeral);
    assert!(ephemeral < error_zero);
    assert!(error_zero < error_one);
    let witness_is_zero = source
        .split("fn is_zero(&self) -> bool {")
        .nth(1)
        .expect("witness is-zero helper")
        .split("    }\n}")
        .next()
        .expect("witness is-zero helper end");
    assert!(witness_is_zero.contains("iter().all"));
    assert!(!witness_is_zero.contains("Vec"));
    assert!(!witness_is_zero.contains("Box"));
    let release = release_profile_v1();
    let limb_bytes = release.ring_degree * core::mem::size_of::<u64>();
    let canonical_plaintext_bytes = release.ring_degree * 32;
    let signed_witness_bytes = release.ring_degree * core::mem::size_of::<i64>();
    assert_eq!(limb_bytes, 1_048_576);
    assert_eq!(
        limb_bytes * super::super::super::manifest::RELEASE_MODULI_V1.len() * 2,
        79_691_776
    );
    assert_eq!(limb_bytes * 2 + 8_208, 2_105_360);
    assert_eq!(
        canonical_plaintext_bytes + signed_witness_bytes * 3 + limb_bytes * 2 + 8_208,
        9_445_392
    );
}
