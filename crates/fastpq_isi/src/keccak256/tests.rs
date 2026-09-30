//! Independent SHA3/SHAKE vectors, exact prefix replay, and owned-cell erasure.

use super::*;

fn decode_hex(source: &str) -> Vec<u8> {
    assert!(source.len().is_multiple_of(2));
    source
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(core::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn input(length: usize) -> Vec<u8> {
    (0..length)
        .map(|i| u8::try_from((i * 37 + 11) & 255).unwrap())
        .collect()
}

#[test]
fn fips202_outputs_match_independent_rate_and_squeeze_boundary_vectors() {
    for line in include_str!("../assets/keccak256_reference_v1.tsv").lines() {
        let columns: Vec<_> = line.split('\t').collect();
        assert_eq!(columns.len(), 3);
        let length = columns[0].parse::<usize>().unwrap();
        let bytes = input(length);
        let expected_hash = decode_hex(columns[1]);
        let expected_xof = decode_hex(columns[2]);
        for chunk in [1, 7, 8, 135, 136, 137, 4096] {
            let mut hash = Sha3_256V1::new();
            let mut xof = Shake256V1::new();
            for part in bytes.chunks(chunk) {
                hash.update(part);
                xof.update(part);
            }
            assert_eq!(
                hash.finalize().as_bytes(),
                expected_hash.as_slice(),
                "input {length}, chunk {chunk}"
            );
            let mut reader = xof.finalize();
            let mut output = vec![0; expected_xof.len()];
            for part in output.chunks_mut(chunk) {
                reader.read(part);
            }
            assert_eq!(output, expected_xof, "input {length}, chunk {chunk}");
        }
    }
}

#[test]
fn cloned_prefix_preserves_full_input_and_partial_rate_block() {
    let bytes = input(8192);
    let mut whole_hash = Sha3_256V1::new();
    whole_hash.update(&bytes);
    let expected_hash = whole_hash.finalize();
    let mut whole_xof = Shake256V1::new();
    whole_xof.update(&bytes);
    let mut expected_xof = [0; 744];
    whole_xof.finalize().read(&mut expected_xof);
    for split in [0, 1, 7, 8, 135, 136, 137, 271, 272, 4096, 8192] {
        let mut hash_prefix = Sha3_256V1::new();
        hash_prefix.update(&bytes[..split]);
        let mut xof_prefix = Shake256V1::new();
        xof_prefix.update(&bytes[..split]);
        for _ in 0..3 {
            let mut hash = hash_prefix.clone();
            hash.update(&bytes[split..]);
            assert_eq!(hash.finalize(), expected_hash);
            let mut xof = xof_prefix.clone();
            xof.update(&bytes[split..]);
            let mut actual = [0; 744];
            xof.finalize().read(&mut actual);
            assert_eq!(actual, expected_xof);
        }
    }
}

struct Observation;
impl Observation {
    fn start() -> Self {
        ERASURE.with(|counts| assert_eq!(counts.replace(Some((0, 0))), None));
        Self
    }
    fn counts() -> (usize, usize) {
        ERASURE.with(|counts| counts.get().unwrap())
    }
}
impl Drop for Observation {
    fn drop(&mut self) {
        ERASURE.with(|counts| counts.set(None));
    }
}

#[test]
fn actual_states_and_permutation_scratch_clear_on_success_error_and_unwind() {
    let observation = Observation::start();
    let mut hash = Sha3_256V1::new();
    hash.update(&[0xa5; 137]);
    let _public_digest = hash.finalize();
    assert_eq!(Observation::counts(), (2 * 35 + 25, 0));
    let before = Observation::counts().0;
    let error = {
        let mut xof = Shake256V1::new();
        xof.update(&[0x79; 31]);
        let mut reader = xof.finalize();
        let mut output = zeroize::Zeroizing::new([0; 288]);
        reader.read(&mut output[..]);
        Err::<(), ()>(())
    };
    assert!(error.is_err());
    assert_eq!(Observation::counts(), (before + 3 * 35 + 25, 0));
    let before = Observation::counts().0;
    assert!(
        std::panic::catch_unwind(|| {
            let mut hash = Sha3_256V1::new();
            hash.update(&[0x17; 17]);
            let _copy = hash.clone();
            panic!("owned hash unwind fixture");
        })
        .is_err()
    );
    assert_eq!(Observation::counts(), (before + 50, 0));
    drop(observation);
}

#[test]
fn distinct_suffixes_opaque_bits_and_redacted_bounded_states() {
    let mut hash = Sha3_256V1::new();
    hash.update(b"same exact bytes");
    let mut xof = Shake256V1::new();
    xof.update(b"same exact bytes");
    assert_eq!(format!("{hash:?}"), "Sha3_256V1 { .. }");
    assert_eq!(format!("{xof:?}"), "Shake256V1 { .. }");
    let mut reader = xof.finalize();
    assert_eq!(format!("{reader:?}"), "Shake256ReaderV1 { .. }");
    let mut output = [0; 32];
    reader.read(&mut output);
    assert_ne!(hash.finalize().as_bytes(), &output);
    for bytes in [[0; 32], [0xff; 32], [0x80; 32]] {
        assert_eq!(Sha3Digest256V1::from_bytes(bytes).into_bytes(), bytes);
    }
    assert_eq!(Sha3Digest256V1::BYTES, 32);
    assert_eq!(core::mem::size_of::<Sha3Digest256V1>(), 32);
    assert_eq!(
        Sha3_256V1::RETAINED_BYTES,
        core::mem::size_of::<([u64; 25], usize)>()
    );
    assert_eq!(Shake256V1::RETAINED_BYTES, Sha3_256V1::RETAINED_BYTES);
    assert_eq!(Shake256ReaderV1::RETAINED_BYTES, Sha3_256V1::RETAINED_BYTES);
}
