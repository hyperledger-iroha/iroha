//! Independent fixed-width reduction, framing and bounds controls.

use super::*;

fn independent_bit_remainder(bytes: &[u8]) -> u16 {
    let mut remainder = 0_u16;
    for &byte in bytes {
        for bit in (0..8).rev() {
            remainder = (remainder * 2 + u16::from((byte >> bit) & 1)) % 257;
        }
    }
    remainder
}

#[test]
fn every_byte_transition_matches_euclidean_reduction() {
    for residue in 0..=256_u16 {
        for byte in 0..=u8::MAX {
            let expected = (u32::from(residue) * 256 + u32::from(byte)) % 257;
            assert_eq!(u32::from(reduce_byte(residue, byte)), expected);
        }
    }
}

#[test]
fn complete_big_endian_lanes_match_independent_bit_division() {
    for bit in 0..256 {
        let mut bytes = [0; 32];
        bytes[31 - bit / 8] = 1 << (bit % 8);
        assert_eq!(reduce_lane(&bytes), independent_bit_remainder(&bytes));
    }
    for seed in 0..=u8::MAX {
        let mut bytes = [0; 32];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = seed
                .wrapping_mul(u8::try_from(index + 1).unwrap())
                .wrapping_add(seed);
        }
        assert_eq!(reduce_lane(&bytes), independent_bit_remainder(&bytes));
    }
    assert_eq!(reduce_lane(&[0; 32]), 0);
    assert_eq!(reduce_lane(&[255; 32]), 0);
    let mut bytes = [0; 32];
    bytes[31] = 1;
    assert_eq!(reduce_lane(&bytes), 1);
    bytes.swap(30, 31);
    assert_eq!(reduce_lane(&bytes), 256);
}

#[test]
fn framed_xof_binds_secret_policy_and_associated_data_without_split_aliases() {
    let policy = Hash::new(b"initializer-test-policy");
    let first = derive_residues(b"a", policy, b"bc").unwrap();
    assert_eq!(first, derive_residues(b"a", policy, b"bc").unwrap());
    assert_ne!(first, derive_residues(b"ab", policy, b"c").unwrap());
    assert_ne!(
        first,
        derive_residues(b"a", Hash::new(b"other-policy"), b"bc").unwrap()
    );
    assert_ne!(first, derive_residues(b"a", policy, b"bd").unwrap());
    assert!(first.iter().all(|&value| value < 257));

    let input = ProgramInitializationInputV1 {
        initializer_descriptor_hash: bfv_program_initializer_descriptor_hash(),
        policy_hash: policy,
        secret: b"a",
        associated_data: b"bc",
    };
    let mut canonical = Zeroizing::new(Vec::with_capacity(
        norito::canonical_frame_len(&input).unwrap(),
    ));
    norito::core::write_canonical_to_writer(&input, &mut *canonical).unwrap();
    let mut independent = Zeroizing::new(blake3::Hasher::new_derive_key(CONTEXT));
    independent.update(&canonical);
    let mut reader = Zeroizing::new(independent.finalize_xof());
    let mut bytes = Zeroizing::new([0; 1024]);
    reader.fill(bytes.as_mut());
    for (lane, &expected) in bytes.chunks_exact(32).zip(first.iter()) {
        assert_eq!(u64::from(independent_bit_remainder(lane)), expected);
    }
    // Captured from a normal native run, then checked with independent integer
    // division. These are fixed public test inputs, never a runtime secret.
    assert_eq!(
        hex::encode(&*canonical),
        include_str!("../../tests/fixtures/ram_lfe_initializer_v1_frame.hex").trim(),
    );
    assert_eq!(
        hex::encode(bytes.as_slice()),
        include_str!("../../tests/fixtures/ram_lfe_initializer_v1_xof.hex").trim(),
    );
    assert_eq!(
        *first,
        [
            180, 45, 216, 139, 123, 250, 57, 134, 174, 12, 99, 154, 95, 195, 158, 139, 203, 6, 139,
            57, 135, 189, 237, 111, 237, 4, 197, 196, 201, 221, 232, 49,
        ],
    );
    let guard = norito::core::DecodeFlagsGuard::enter(0);
    assert_eq!(first, derive_residues(b"a", policy, b"bc").unwrap());
    drop(guard);
}

#[test]
fn initializer_bounds_are_closed_and_profile_digest_is_compiled() {
    let policy = Hash::new(b"bounds");
    assert!(derive_residues(&[], policy, &[]).is_err());
    assert!(derive_residues(&[1; RAM_LFE_SECRET_MAX_BYTES + 1], policy, &[]).is_err());
    assert!(
        derive_residues(
            b"a",
            policy,
            &[0; RAM_LFE_PROGRAM_ASSOCIATED_DATA_MAX_BYTES + 1]
        )
        .is_err()
    );
    assert!(
        derive_residues(
            &[1; RAM_LFE_SECRET_MAX_BYTES],
            policy,
            &[0; RAM_LFE_PROGRAM_ASSOCIATED_DATA_MAX_BYTES]
        )
        .is_ok()
    );
    let mut profile = super::super::bfv_program_profile();
    // Independently computed from the exact public descriptor using Blake2b-256
    // and the Iroha marker, before any native KAT regeneration.
    assert_eq!(
        bfv_program_initializer_descriptor_hash().to_string(),
        "e90e6314da02be784ba7d2d2a3e18ff171711135e80a8eddb7ad31d7b28d3d01"
    );
    assert_eq!(
        profile.initializer_descriptor_hash,
        bfv_program_initializer_descriptor_hash()
    );
    profile.initializer_descriptor_hash = Hash::new(b"retired mapping");
    assert!(super::super::validate_programmed_profile(&profile).is_err());
}

#[cfg(feature = "json")]
#[test]
fn first_release_profile_requires_the_initializer_field() {
    use super::super::BfvRamProgramProfile;
    let current = super::super::bfv_program_profile();
    let text = norito::json::to_json(&current).unwrap();
    let decoded: BfvRamProgramProfile = norito::json::from_str(&text).unwrap();
    assert_eq!(decoded, current);
    let mut unknown = norito::json::to_value(&current).unwrap();
    unknown.as_object_mut().unwrap().insert(
        "legacy_initializer".to_owned(),
        norito::json::Value::from(0_u64),
    );
    assert!(norito::json::from_value::<BfvRamProgramProfile>(unknown).is_err());
    let retired = r#"{"profile_version":1,"register_count":4,"memory_lane_count":32,"ciphertext_mul_per_step":16,"encrypted_input_mode":"encrypted_envelope_v1","min_ciphertext_modulus":4503599627370496}"#;
    assert!(norito::json::from_str::<BfvRamProgramProfile>(retired).is_err());
}
