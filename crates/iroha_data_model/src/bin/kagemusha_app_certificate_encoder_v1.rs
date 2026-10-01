//! Canonical app-enrollment certificate encoding using Iroha's model and signer.

use std::{
    io::{self, Read, Write},
    os::fd::FromRawFd,
};

use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_ORDINARY_APP_CREDENTIAL_BODY_BYTES_V1, KagemushaOrdinaryAppCredentialSubjectV1,
    KagemushaOrdinaryAppCredentialV1,
};
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

fn fail(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
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
    let key_file = unsafe { std::fs::File::from_raw_fd(descriptor) };
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
}
