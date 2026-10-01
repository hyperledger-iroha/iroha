//! Purpose-specific raw-attestation admission encoding using the actual Iroha model.

use std::io::{self, Read, Write};

use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_RAW_APP_ATTESTATION_SIGNING_REQUEST_BYTES_V1,
    KagemushaRawAppAttestationAdmissionSubjectV1, KagemushaRawAppAttestationAdmissionV1,
};
/// Secret bytes retained until the native key implementation consumes them.
struct SecretSeed(Vec<u8>);
impl Drop for SecretSeed {
    fn drop(&mut self) {
        iroha_crypto::zeroize_value_for_confidential_discard(&mut self.0);
    }
}

const REQUEST_LEN: usize = KAGEMUSHA_RAW_APP_ATTESTATION_SIGNING_REQUEST_BYTES_V1;

fn fail(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[path = "private_signer_process.rs"]
mod private_signer_process;

fn protect_private_process() -> io::Result<()> {
    private_signer_process::protect()
}

// dup preserves the inherited root-owned key's existing access after uid drop.
#[allow(unsafe_code)]
fn duplicate_inherited_descriptor(descriptor: i32) -> io::Result<std::fs::File> {
    use std::os::fd::FromRawFd as _;
    unsafe extern "C" {
        fn dup(descriptor: i32) -> i32;
    }
    // SAFETY: dup validates the integer descriptor and returns a new owned FD.
    let owned = unsafe { dup(descriptor) };
    if owned < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: only this File owns the successful dup result.
    Ok(unsafe { std::fs::File::from_raw_fd(owned) })
}

fn parse_request(
    input: &[u8],
) -> io::Result<(KagemushaRawAppAttestationAdmissionSubjectV1, [u8; 32])> {
    KagemushaRawAppAttestationAdmissionSubjectV1::from_signing_request(input)
        .map_err(|_| fail("invalid raw-attestation admission signing request"))
}

fn sign(input: &[u8], seed: Vec<u8>) -> io::Result<Vec<u8>> {
    let mut seed = SecretSeed(seed);
    let (subject, pinned_key) = parse_request(input)?;
    let key = KeyPair::try_from_seed(std::mem::take(&mut seed.0), Algorithm::Ed25519)
        .map_err(|_| fail("invalid authority signing seed"))?;
    let (algorithm, public) = key.public_key().to_bytes();
    if algorithm != Algorithm::Ed25519 || public != pinned_key {
        return Err(fail("authority signer differs from pinned release policy"));
    }
    let message = subject
        .canonical_signing_bytes()
        .map_err(|_| fail("raw-attestation admission signing subject rejected"))?;
    let signature = Signature::new(key.private_key(), &message);
    signature
        .verify(key.public_key(), &message)
        .map_err(|_| fail("raw-attestation admission signature self-check failed"))?;
    let certificate = KagemushaRawAppAttestationAdmissionV1 { subject, signature };
    let canonical = certificate
        .to_transport_bytes()
        .map_err(|_| fail("raw-attestation admission encoding failed"))?;
    let decoded = KagemushaRawAppAttestationAdmissionV1::from_transport_bytes(&canonical)
        .map_err(|_| fail("raw-attestation admission roundtrip failed"))?;
    if decoded != certificate {
        return Err(fail(
            "canonical raw-attestation admission changed on decode",
        ));
    }
    Ok(canonical)
}

fn read_seed(key_file: &std::fs::File) -> io::Result<SecretSeed> {
    let mut seed = SecretSeed(vec![0; 32]);
    let mut trailing = [0; 1];
    let trailing_bytes = if key_file.metadata()?.is_file() {
        // A retained regular key descriptor may serve multiple requests. Positional
        // reads preserve its offset and do not make the second issue see EOF.
        use std::os::unix::fs::FileExt as _;
        key_file.read_exact_at(seed.0.as_mut_slice(), 0)?;
        key_file.read_at(&mut trailing, 32)?
    } else {
        (&mut &*key_file).read_exact(seed.0.as_mut_slice())?;
        (&mut &*key_file).read(&mut trailing)?
    };
    if trailing_bytes != 0 {
        return Err(fail("authority signing seed outside bound"));
    }
    Ok(seed)
}

fn main() -> io::Result<()> {
    protect_private_process()?;
    let mut arguments = std::env::args();
    if arguments.next().is_none() || arguments.next().as_deref() != Some("--key-fd") {
        return Err(fail("expected --key-fd and one inherited descriptor"));
    }
    let descriptor = arguments
        .next()
        .ok_or_else(|| fail("missing key descriptor"))?
        .parse::<i32>()
        .map_err(|_| fail("invalid key descriptor"))?;
    if descriptor < 3 || arguments.next().is_some() {
        return Err(fail("invalid key descriptor"));
    }
    let mut request = Vec::with_capacity(REQUEST_LEN);
    io::stdin()
        .take((REQUEST_LEN + 1) as u64)
        .read_to_end(&mut request)?;
    if request.len() != REQUEST_LEN {
        return Err(fail("certificate signing request outside bound"));
    }
    // The issuer passes the key through an inherited descriptor. No key appears in CLI
    // arguments, environment variables, stdout, or the repository.
    let key_file = duplicate_inherited_descriptor(descriptor)?;
    let mut seed = read_seed(&key_file)?;
    let result = sign(&request, std::mem::take(&mut seed.0));
    io::stdout().write_all(&result?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::kagemusha::{
        KagemushaAppKeySecurityLevelV1, KagemushaDevicePublicKeyV1,
        KagemushaHardwarePlatformClassV1,
    };
    use sha2::{Digest as _, Sha256};

    fn subject() -> KagemushaRawAppAttestationAdmissionSubjectV1 {
        // Public generator and deterministic selectors exercise only codec/signature behavior.
        let point = hex::decode(concat!(
            "04",
            "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296",
            "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5"
        ))
        .unwrap();
        KagemushaRawAppAttestationAdmissionSubjectV1 {
            version: 1,
            enrollment_challenge_digest: [1; 32],
            authority_policy_digest: [2; 32],
            platform_class: KagemushaHardwarePlatformClassV1::AndroidKeyMint,
            security_level: KagemushaAppKeySecurityLevelV1::TrustedExecutionEnvironment,
            app_public_key: KagemushaDevicePublicKeyV1::from_sec1_bytes(&point).unwrap(),
            attested_key_id: Sha256::digest(&point).into(),
            raw_platform_evidence_digest: [3; 32],
            app_signing_identity_digest: [4; 32],
            original_app_attest_counter: 0,
            issued_at_ms: 1000,
            expires_at_ms: 121000,
        }
    }
    #[test]
    fn raw_encoder_signs_only_exact_model_purpose_and_pinned_authority() {
        let key = KeyPair::from_seed(vec![73; 32], Algorithm::Ed25519);
        let original_subject = subject();
        let request = original_subject
            .to_signing_request(key.public_key())
            .unwrap();
        let original = sign(&request, vec![73; 32]).unwrap();
        assert_eq!(original.len(), 314);
        let admission =
            KagemushaRawAppAttestationAdmissionV1::from_transport_bytes(&original).unwrap();
        assert_eq!(admission.subject, original_subject);
        admission
            .signature
            .verify(
                key.public_key(),
                &original_subject.canonical_signing_bytes().unwrap(),
            )
            .unwrap();
        assert_eq!(sign(&request, vec![73; 32]).unwrap(), original);
        assert!(sign(&request, vec![74; 32]).is_err());
        for length in [0, 5, 287] {
            assert!(parse_request(&request[..length]).is_err());
        }
        let mut extra = request.clone();
        extra.push(0);
        assert!(parse_request(&extra).is_err());
        let mut foreign = request;
        foreign[..6].copy_from_slice(b"KOAC01");
        assert!(parse_request(&foreign).is_err());
    }
    #[test]
    fn raw_encoder_retained_key_descriptor_keeps_offset_and_exact_seed_width() {
        use std::io::{Seek as _, SeekFrom};
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(&[73; 32]).unwrap();
        file.seek(SeekFrom::Start(7)).unwrap();
        assert_eq!(read_seed(&file).unwrap().0, vec![73; 32]);
        assert_eq!(read_seed(&file).unwrap().0, vec![73; 32]);
        assert_eq!(file.stream_position().unwrap(), 7);
        file.seek(SeekFrom::End(0)).unwrap();
        file.write_all(&[1]).unwrap();
        assert!(read_seed(&file).is_err());
    }
    #[test]
    fn inherited_descriptor_is_duplicated_without_reopening_key_path() {
        use std::os::fd::AsRawFd as _;
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(&[73; 32]).unwrap();
        let duplicate = duplicate_inherited_descriptor(file.as_raw_fd()).unwrap();
        assert_ne!(file.as_raw_fd(), duplicate.as_raw_fd());
        assert_eq!(read_seed(&duplicate).unwrap().0, vec![73; 32]);
        drop(duplicate);
        assert_eq!(read_seed(&file).unwrap().0, vec![73; 32]);
        assert!(duplicate_inherited_descriptor(-1).is_err());
    }
}
