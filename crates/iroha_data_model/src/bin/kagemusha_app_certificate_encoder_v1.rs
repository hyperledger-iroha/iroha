//! Canonical app-enrollment certificate encoding using Iroha's model and signer.

use std::io::{self, Read, Write};

use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_ORDINARY_APP_CREDENTIAL_BODY_BYTES_V1, KagemushaAppOperationApprovalEvidenceV1,
    KagemushaOrdinaryAppCredentialSubjectV1, KagemushaOrdinaryAppCredentialV1,
    KagemushaPlayIntegrityRefreshLeaseSubjectV1, KagemushaPlayIntegrityRefreshLeaseV1,
};
use sha2::{Digest as _, Sha256};
/// Secret bytes retained until the native key implementation consumes them.
struct SecretSeed(Vec<u8>);
impl Drop for SecretSeed {
    fn drop(&mut self) {
        iroha_crypto::zeroize_value_for_confidential_discard(&mut self.0);
    }
}

const REQUEST_MAGIC: &[u8; 5] = b"KOAC\x01";
const REQUEST_LEN: usize = 5 + KAGEMUSHA_ORDINARY_APP_CREDENTIAL_BODY_BYTES_V1 + 32;
const MAX_CERTIFICATE_LEN: usize = 16 * 1024;
const REFRESH_MAGIC: &[u8; 5] = b"KRPI\x01";
const REFRESH_BODY_LEN: usize = 402;

fn fail(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn protect_private_process() -> io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        use rustix::process::{DumpableBehavior, dumpable_behavior, set_dumpable_behavior};
        set_dumpable_behavior(DumpableBehavior::NotDumpable)?;
        if dumpable_behavior()? != DumpableBehavior::NotDumpable {
            return Err(fail("private signer process protection unavailable"));
        }
    }
    Ok(())
}

// dup validates the offered descriptor and creates a new owned descriptor.
// Unlike opening /proc/self/fd, this preserves inherited access after uid drop.
#[allow(unsafe_code)]
fn duplicate_inherited_descriptor(descriptor: i32) -> io::Result<std::fs::File> {
    use std::os::fd::FromRawFd as _;
    unsafe extern "C" {
        fn dup(descriptor: i32) -> i32;
    }
    // SAFETY: dup accepts any integer and reports invalid descriptors as an error.
    let owned = unsafe { dup(descriptor) };
    if owned < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful dup created this open descriptor, owned only here.
    Ok(unsafe { std::fs::File::from_raw_fd(owned) })
}

fn parse_request(input: &[u8]) -> io::Result<(KagemushaOrdinaryAppCredentialSubjectV1, [u8; 32])> {
    if input.len() != REQUEST_LEN || &input[..5] != REQUEST_MAGIC {
        return Err(fail("invalid ordinary credential signing request"));
    }
    let subject =
        KagemushaOrdinaryAppCredentialSubjectV1::from_signing_body(&input[5..REQUEST_LEN - 32])
            .map_err(|_| fail("invalid ordinary credential subject"))?;
    let pin: [u8; 32] = input[REQUEST_LEN - 32..]
        .try_into()
        .expect("checked request length");
    if pin == [0; 32] {
        return Err(fail("empty app authority signer pin"));
    }
    Ok((subject, pin))
}

fn sign(input: &[u8], seed: Vec<u8>) -> io::Result<Vec<u8>> {
    if input.starts_with(REFRESH_MAGIC) {
        return sign_refresh(input, seed);
    }
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
        .map_err(|_| fail("ordinary credential signing subject rejected"))?;
    let signature = Signature::new(key.private_key(), &message);
    signature
        .verify(key.public_key(), &message)
        .map_err(|_| fail("ordinary credential signature self-check failed"))?;
    let certificate = KagemushaOrdinaryAppCredentialV1 { subject, signature };
    let canonical = norito::encode_canonical(&certificate)
        .map_err(|_| fail("canonical app-enrollment certificate encoding failed"))?;
    if canonical.len() > MAX_CERTIFICATE_LEN {
        return Err(fail("canonical app-enrollment certificate exceeds bound"));
    }
    let decoded: KagemushaOrdinaryAppCredentialV1 = norito::decode_from_bytes(&canonical)
        .map_err(|_| fail("canonical app-enrollment certificate roundtrip failed"))?;
    if decoded != certificate {
        return Err(fail(
            "canonical app-enrollment certificate changed on decode",
        ));
    }
    Ok(canonical)
}

fn parse_refresh_request(
    input: &[u8],
) -> io::Result<(
    KagemushaPlayIntegrityRefreshLeaseSubjectV1,
    Vec<u8>,
    [u8; 32],
)> {
    const PREFIX: usize = 5 + REFRESH_BODY_LEN + 2;
    if !input.starts_with(REFRESH_MAGIC) || input.len() < PREFIX + 8 + 32 {
        return Err(fail("invalid Integrity lease signing request"));
    }
    let der_len = u16::from_le_bytes(input[407..409].try_into().expect("checked prefix")) as usize;
    if !(8..=72).contains(&der_len) || input.len() != PREFIX + der_len + 32 {
        return Err(fail("Integrity possession width differs"));
    }
    let subject = KagemushaPlayIntegrityRefreshLeaseSubjectV1::from_signing_body(&input[5..407])
        .map_err(|_| fail("invalid Integrity lease subject"))?;
    let der = input[PREFIX..PREFIX + der_len].to_vec();
    let parsed = p256::ecdsa::Signature::from_der(&der)
        .map_err(|_| fail("invalid original Integrity possession DER"))?;
    if parsed.to_der().as_bytes() != der.as_slice()
        || subject.possession_original_digest != <[u8; 32]>::from(Sha256::digest(&der))
    {
        return Err(fail(
            "Integrity possession original differs from signed subject",
        ));
    }
    let pin: [u8; 32] = input[PREFIX + der_len..]
        .try_into()
        .expect("checked request length");
    if pin == [0; 32] {
        return Err(fail("empty app authority signer pin"));
    }
    Ok((subject, der, pin))
}

fn sign_refresh(input: &[u8], seed: Vec<u8>) -> io::Result<Vec<u8>> {
    let mut seed = SecretSeed(seed);
    let (subject, signature_der, pinned_key) = parse_refresh_request(input)?;
    let key = KeyPair::try_from_seed(std::mem::take(&mut seed.0), Algorithm::Ed25519)
        .map_err(|_| fail("invalid authority signing seed"))?;
    let (algorithm, public) = key.public_key().to_bytes();
    if algorithm != Algorithm::Ed25519 || public != pinned_key {
        return Err(fail("authority signer differs from pinned release policy"));
    }
    let message = subject
        .canonical_signing_bytes()
        .map_err(|_| fail("Integrity lease signing subject rejected"))?;
    let signature = Signature::new(key.private_key(), &message);
    signature
        .verify(key.public_key(), &message)
        .map_err(|_| fail("Integrity lease signature self-check failed"))?;
    let lease = KagemushaPlayIntegrityRefreshLeaseV1 {
        subject,
        signature,
        app_possession: KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der },
    };
    let canonical = lease
        .canonical_bytes()
        .map_err(|_| fail("canonical Integrity lease encoding failed"))?;
    let decoded: KagemushaPlayIntegrityRefreshLeaseV1 = norito::decode_from_bytes(&canonical)
        .map_err(|_| fail("canonical Integrity lease roundtrip failed"))?;
    if decoded != lease {
        return Err(fail("canonical Integrity lease changed on decode"));
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
    // exec resets Linux dumpability; protect this image before reading secrets.
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
    if request.len() > REQUEST_LEN
        || (request.len() != REQUEST_LEN && !request.starts_with(REFRESH_MAGIC))
    {
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
        KagemushaHardwarePlatformClassV1, kagemusha_device_key_reference_v1,
    };
    use sha2::{Digest as _, Sha256};

    fn subject() -> KagemushaOrdinaryAppCredentialSubjectV1 {
        // Public P-256 generator fixture; it is never a platform or owner key.
        let point = hex::decode(concat!(
            "04",
            "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296",
            "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5"
        ))
        .unwrap();
        let app_public_key = KagemushaDevicePublicKeyV1::from_sec1_bytes(&point).unwrap();
        KagemushaOrdinaryAppCredentialSubjectV1 {
            version: 1,
            platform_class: KagemushaHardwarePlatformClassV1::AndroidKeyMint,
            security_level: KagemushaAppKeySecurityLevelV1::StrongBox,
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
            app_signing_identity_digest: [12; 32],
            app_release_digest: [13; 32],
            attested_key_id: Sha256::digest(&point).into(),
            app_key_reference: kagemusha_device_key_reference_v1(&app_public_key),
            financial_authority_commitment: [16; 32],
            platform_evidence_digest: [17; 32],
            enrollment_challenge_digest: [18; 32],
            app_public_key,
            policy_epoch: 1,
            hardware_epoch: 2,
            issued_at_ms: 1000,
            expires_at_ms: 2000,
            app_attest_counter_floor: 0,
            play_integrity: None,
        }
    }
    fn frame() -> Vec<u8> {
        let message = subject().canonical_signing_bytes().unwrap();
        let mut frame = REQUEST_MAGIC.to_vec();
        frame.extend_from_slice(
            &message[message.len() - KAGEMUSHA_ORDINARY_APP_CREDENTIAL_BODY_BYTES_V1..],
        );
        frame.extend_from_slice(&[73; 32]);
        frame
    }
    #[test]
    fn regular_key_descriptor_reuse_preserves_shared_offset() {
        use std::io::{Seek as _, SeekFrom, Write as _};
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
    #[test]
    fn refuses_ambiguous_frame_and_key_role_substitution() {
        let original = frame();
        let (parsed, pin) = parse_request(&original).unwrap();
        assert_eq!(parsed, subject());
        assert_eq!(pin, [73; 32]);
        for malformed in [&original[..original.len() - 1], &original[..4]] {
            assert!(parse_request(malformed).is_err());
        }
        let mut extra = original.clone();
        extra.push(0);
        assert!(parse_request(&extra).is_err());
        let mut wrong = original.clone();
        wrong[0] = b'X';
        assert!(parse_request(&wrong).is_err());
        wrong = original.clone();
        wrong[5 + 4 + 13 * 32..5 + 4 + 14 * 32].fill(0);
        assert!(parse_request(&wrong).is_err());
        wrong = original;
        wrong[REQUEST_LEN - 32..].fill(0);
        assert!(parse_request(&wrong).is_err());
    }
    #[test]
    fn signs_actual_model_message_and_roundtrips_canonical_ordinary_credential() {
        let seed = vec![73; 32];
        let key = KeyPair::try_from_seed(seed.clone(), Algorithm::Ed25519).unwrap();
        let mut request = frame();
        request[REQUEST_LEN - 32..].copy_from_slice(key.public_key().to_bytes().1);
        let first = sign(&request, seed.clone()).unwrap();
        assert_eq!(first, sign(&request, seed).unwrap());
        let certificate: KagemushaOrdinaryAppCredentialV1 =
            norito::decode_from_bytes(&first).unwrap();
        assert_eq!(certificate.subject, subject());
        certificate
            .signature
            .verify(
                key.public_key(),
                &certificate.subject.canonical_signing_bytes().unwrap(),
            )
            .unwrap();
        request[REQUEST_LEN - 32..].fill(42);
        assert!(sign(&request, vec![73; 32]).is_err());
    }

    fn refresh_frame() -> Vec<u8> {
        use iroha_data_model::kagemusha::KagemushaPlayIntegrityBindingV1;
        use p256::ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _};
        // Isolated encoder fixture. Platform admission is tested by the model;
        // encoding does not authenticate this fixture as a Native credential.
        let key = SigningKey::from_bytes((&[31; 32]).into()).unwrap();
        let possession: P256Signature = key.sign(b"isolated original refresh possession");
        let der = possession.to_der();
        let subject = KagemushaPlayIntegrityRefreshLeaseSubjectV1 {
            version: 1,
            credential_digest: [1; 32],
            challenge_digest: [2; 32],
            attested_key_id: [3; 32],
            release_id: [4; 32],
            hardware_profile_id: [5; 32],
            trust_policy_digest: [6; 32],
            app_authority_policy_digest: [7; 32],
            binding: KagemushaPlayIntegrityBindingV1 {
                request_hash: [8; 32],
                evidence_digest: [9; 32],
                policy_digest: [10; 32],
                verified_at_ms: 1000,
                refresh_before_ms: 2000,
            },
            possession_original_digest: Sha256::digest(der.as_bytes()).into(),
            policy_epoch: 11,
            hardware_epoch: 12,
            issued_at_ms: 1001,
            expires_at_ms: 1999,
        };
        let message = subject.canonical_signing_bytes().unwrap();
        let mut frame = REFRESH_MAGIC.to_vec();
        frame.extend_from_slice(&message[message.len() - REFRESH_BODY_LEN..]);
        frame.extend_from_slice(&(der.as_bytes().len() as u16).to_le_bytes());
        frame.extend_from_slice(der.as_bytes());
        let authority = KeyPair::try_from_seed(vec![73; 32], Algorithm::Ed25519).unwrap();
        frame.extend_from_slice(authority.public_key().to_bytes().1);
        frame
    }

    #[test]
    fn refresh_preserves_full_possession_and_real_authority_signature_in_model_archive() {
        let frame = refresh_frame();
        let (subject, original_der, _) = parse_refresh_request(&frame).unwrap();
        let archive = sign(&frame, vec![73; 32]).unwrap();
        assert_eq!(archive, sign(&frame, vec![73; 32]).unwrap());
        let decoded: KagemushaPlayIntegrityRefreshLeaseV1 =
            norito::decode_from_bytes(&archive).unwrap();
        assert_eq!(decoded.subject, subject);
        assert_eq!(
            decoded.app_possession,
            KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
                signature_der: original_der
            }
        );
        let authority = KeyPair::try_from_seed(vec![73; 32], Algorithm::Ed25519).unwrap();
        decoded
            .signature
            .verify(
                authority.public_key(),
                &subject.canonical_signing_bytes().unwrap(),
            )
            .unwrap();
        let mut wrong_pin = frame;
        let end = wrong_pin.len();
        wrong_pin[end - 32..].fill(42);
        assert!(sign(&wrong_pin, vec![73; 32]).is_err());
    }

    #[test]
    fn refresh_refuses_changed_der_digest_ambiguous_length_and_retired_frame() {
        let frame = refresh_frame();
        let mut trailing = frame.clone();
        trailing.push(0);
        let mut wrong_digest = frame.clone();
        wrong_digest[5 + 2 + 10 * 32] ^= 1;
        let mut wrong_der = frame.clone();
        wrong_der[409] = 0;
        let mut wrong_width = frame.clone();
        wrong_width[407..409].copy_from_slice(&7u16.to_le_bytes());
        let mut empty_key = frame.clone();
        let end = empty_key.len();
        empty_key[end - 32..].fill(0);
        for original in [
            &frame[..frame.len() - 1],
            &trailing,
            &wrong_digest,
            &wrong_der,
            &wrong_width,
            &empty_key,
        ] {
            assert!(parse_refresh_request(original).is_err());
        }
        let mut old_magic = frame;
        old_magic[..5].copy_from_slice(b"KAEA\x01");
        assert!(sign(&old_magic, vec![73; 32]).is_err());
    }
}
