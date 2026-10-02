//! Dedicated private Ed25519 ordinary C encoder using the sole canonical model codec.
//!
//! The actual Native parent independently admits FI callers and governs this executable and
//! descriptor as the Core preparation signing purpose. Process/FD checks and public key equality
//! confer no installation authority. This encoder accepts no arbitrary or P256 monetary message.

use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_ORDINARY_PREPARATION_SIGNING_REQUEST_BYTES_V1,
    KagemushaOrdinaryAppEnrollmentChallengeV1, KagemushaSignedOrdinaryAppEnrollmentChallengeV1,
};
use std::io::{self, Read, Write};
use std::os::unix::fs::{FileExt as _, MetadataExt as _};

#[path = "private_signer_process.rs"]
mod private_signer_process;
const REQUEST_LEN: usize = KAGEMUSHA_ORDINARY_PREPARATION_SIGNING_REQUEST_BYTES_V1;

/// Seed retained only until consumed by the native key implementation.
struct SecretSeed(Vec<u8>);
impl Drop for SecretSeed {
    fn drop(&mut self) {
        iroha_crypto::zeroize_value_for_confidential_discard(&mut self.0);
    }
}
fn fail(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

// Dup retains the actual installed descriptor after uid drop without opening any key path.
#[allow(unsafe_code)]
fn duplicate_inherited_descriptor(descriptor: i32) -> io::Result<std::fs::File> {
    use std::os::fd::FromRawFd as _;
    unsafe extern "C" {
        fn dup(descriptor: i32) -> i32;
    }
    // SAFETY: dup validates the bounded descriptor and returns a new owned FD.
    let owned = unsafe { dup(descriptor) };
    if owned < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: this File uniquely owns the successful duplicate.
    Ok(unsafe { std::fs::File::from_raw_fd(owned) })
}
fn parse_request(
    input: &[u8],
) -> io::Result<(KagemushaOrdinaryAppEnrollmentChallengeV1, [u8; 32])> {
    KagemushaOrdinaryAppEnrollmentChallengeV1::from_signing_request(input)
        .map_err(|_| fail("invalid ordinary C signing request"))
}
fn sign(input: &[u8], seed: Vec<u8>) -> io::Result<Vec<u8>> {
    let mut seed = SecretSeed(seed);
    let (challenge, pinned_key) = parse_request(input)?;
    let key = KeyPair::try_from_seed(std::mem::take(&mut seed.0), Algorithm::Ed25519)
        .map_err(|_| fail("invalid preparation signing seed"))?;
    let (algorithm, public) = key.public_key().to_bytes();
    if algorithm != Algorithm::Ed25519 || public != pinned_key {
        return Err(fail(
            "preparation signer differs from selected issuer policy",
        ));
    }
    let message = challenge
        .canonical_signing_bytes()
        .map_err(|_| fail("ordinary C subject rejected"))?;
    let signature = Signature::new(key.private_key(), &message);
    signature
        .verify(key.public_key(), &message)
        .map_err(|_| fail("ordinary C signature self-check failed"))?;
    let original = KagemushaSignedOrdinaryAppEnrollmentChallengeV1 {
        challenge,
        signature,
    };
    let bytes = original
        .to_transport_bytes()
        .map_err(|_| fail("ordinary C encoding failed"))?;
    let decoded = KagemushaSignedOrdinaryAppEnrollmentChallengeV1::from_transport_bytes(&bytes)
        .map_err(|_| fail("ordinary C roundtrip failed"))?;
    if decoded != original {
        return Err(fail("canonical ordinary C changed on decode"));
    }
    decoded
        .authenticate(
            key.public_key(),
            &original.challenge,
            original.challenge.issued_at_ms,
        )
        .map_err(|_| fail("ordinary C authentication self-check failed"))?;
    Ok(bytes)
}
#[derive(PartialEq, Eq)]
struct KeyFileObservation {
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
    links: u64,
    length: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}
fn observe_key_descriptor(file: &std::fs::File) -> io::Result<KeyFileObservation> {
    use rustix::fs::{OFlags, fcntl_getfl};
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.uid() != 0
        || metadata.mode() & 0o7777 != 0o400
        || metadata.nlink() != 1
        || metadata.len() != 32
        || fcntl_getfl(file)? & OFlags::ACCMODE != OFlags::RDONLY
    {
        return Err(fail("preparation signing descriptor custody rejected"));
    }
    Ok(KeyFileObservation {
        device: metadata.dev(),
        inode: metadata.ino(),
        mode: metadata.mode(),
        uid: metadata.uid(),
        links: metadata.nlink(),
        length: metadata.len(),
        modified_seconds: metadata.mtime(),
        modified_nanoseconds: metadata.mtime_nsec(),
        changed_seconds: metadata.ctime(),
        changed_nanoseconds: metadata.ctime_nsec(),
    })
}
fn read_seed_payload(file: &std::fs::File) -> io::Result<SecretSeed> {
    let mut seed = SecretSeed(vec![0; 32]);
    file.read_exact_at(seed.0.as_mut_slice(), 0)?;
    let mut trailing = [0; 1];
    if file.read_at(&mut trailing, 32)? != 0 {
        return Err(fail("preparation signing seed outside bound"));
    }
    Ok(seed)
}
fn read_seed(file: &std::fs::File) -> io::Result<SecretSeed> {
    let original = observe_key_descriptor(file)?;
    let seed = read_seed_payload(file)?;
    if observe_key_descriptor(file)? != original {
        return Err(fail("preparation descriptor changed during read"));
    }
    Ok(seed)
}
fn main() -> io::Result<()> {
    private_signer_process::protect()?;
    let mut arguments = std::env::args();
    if arguments.next().is_none() || arguments.next().as_deref() != Some("--key-fd") {
        return Err(fail("expected --key-fd and one inherited descriptor"));
    }
    let descriptor = arguments
        .next()
        .ok_or_else(|| fail("missing preparation key descriptor"))?
        .parse::<i32>()
        .map_err(|_| fail("invalid preparation key descriptor"))?;
    if descriptor < 3 || arguments.next().is_some() {
        return Err(fail("invalid preparation key descriptor"));
    }
    let mut request = Vec::with_capacity(REQUEST_LEN);
    io::stdin()
        .take((REQUEST_LEN + 1) as u64)
        .read_to_end(&mut request)?;
    if request.len() != REQUEST_LEN {
        return Err(fail("ordinary C signing request outside bound"));
    }
    parse_request(&request)?;
    let file = duplicate_inherited_descriptor(descriptor)?;
    let mut seed = read_seed(&file)?;
    let bytes = sign(&request, std::mem::take(&mut seed.0))?;
    io::stdout().write_all(&bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::kagemusha::KagemushaHardwarePlatformClassV1;
    use std::io::{Seek as _, SeekFrom};
    use std::os::fd::AsRawFd as _;
    fn challenge(apple: bool) -> KagemushaOrdinaryAppEnrollmentChallengeV1 {
        // Public selectors and deterministic signing seeds test codec/signatures only.
        KagemushaOrdinaryAppEnrollmentChallengeV1 {
            version: 1,
            platform_class: if apple {
                KagemushaHardwarePlatformClassV1::AppleAppAttest
            } else {
                KagemushaHardwarePlatformClassV1::AndroidKeyMint
            },
            enrollment_id: [1; 32],
            client_nonce: [2; 32],
            server_nonce: [3; 32],
            account_binding: [4; 32],
            network_id: [5; 32],
            lane_id: [6; 32],
            release_id: [7; 32],
            hardware_profile_id: [8; 32],
            suite_id: [9; 32],
            trust_policy_digest: [10; 32],
            app_authority_policy_digest: [11; 32],
            financial_authority_commitment: [12; 32],
            issuer_policy_digest: [13; 32],
            policy_epoch: 1,
            hardware_epoch: 1,
            issued_at_ms: 1000,
            expires_at_ms: 3000,
        }
    }
    #[test]
    fn preparation_encoder_signs_exact_model_c_with_the_pinned_ed_issuer() {
        let key = KeyPair::from_seed(vec![63; 32], Algorithm::Ed25519);
        for apple in [false, true] {
            let expected = challenge(apple);
            let request = expected.to_signing_request(key.public_key()).unwrap();
            let bytes = sign(&request, vec![63; 32]).unwrap();
            assert_eq!(bytes.len(), 515);
            let original =
                KagemushaSignedOrdinaryAppEnrollmentChallengeV1::from_transport_bytes(&bytes)
                    .unwrap();
            original
                .authenticate(key.public_key(), &expected, 1000)
                .unwrap();
            assert_eq!(sign(&request, vec![63; 32]).unwrap(), bytes);
            assert!(sign(&request, vec![61; 32]).is_err());
            let mut substituted = expected;
            substituted.hardware_epoch += 1;
            assert!(
                original
                    .authenticate(key.public_key(), &substituted, 1000)
                    .is_err()
            );
            assert!(
                original
                    .authenticate(key.public_key(), &original.challenge, 3000)
                    .is_err()
            );
            for width in [0, 5, REQUEST_LEN - 1] {
                assert!(parse_request(&request[..width]).is_err());
            }
            let mut extra = request.clone();
            extra.push(0);
            assert!(parse_request(&extra).is_err());
            let mut foreign = request;
            foreign[..5].copy_from_slice(b"KOAA\x01");
            assert!(parse_request(&foreign).is_err());
        }
    }
    #[test]
    fn preparation_encoder_positional_payload_preserves_offset_and_refuses_wrong_seed_width() {
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(&[63; 32]).unwrap();
        file.seek(SeekFrom::Start(7)).unwrap();
        assert_eq!(read_seed_payload(&file).unwrap().0, vec![63; 32]);
        assert_eq!(read_seed_payload(&file).unwrap().0, vec![63; 32]);
        assert_eq!(file.stream_position().unwrap(), 7);
        file.set_len(31).unwrap();
        assert!(read_seed_payload(&file).is_err());
        file.set_len(32).unwrap();
        file.seek(SeekFrom::End(0)).unwrap();
        file.write_all(&[1]).unwrap();
        assert!(read_seed_payload(&file).is_err());
    }
    #[test]
    fn preparation_encoder_uses_actual_inherited_descriptor_and_refuses_writable_custody() {
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(&[63; 32]).unwrap();
        let duplicate = duplicate_inherited_descriptor(file.as_raw_fd()).unwrap();
        assert_ne!(duplicate.as_raw_fd(), file.as_raw_fd());
        assert_eq!(read_seed_payload(&duplicate).unwrap().0, vec![63; 32]);
        assert!(observe_key_descriptor(&duplicate).is_err());
        assert!(read_seed(&duplicate).is_err());
        drop(duplicate);
        assert_eq!(read_seed_payload(&file).unwrap().0, vec![63; 32]);
        assert!(duplicate_inherited_descriptor(-1).is_err());
    }
}
