//! Crypto/selection fixtures only: these keys and public statements create no native Core.
use super::*;
use crate::kagemusha_device_bridge_v1::sender_payload::{
    SenderHardwareAuthorizationV1, canonical_command_body_for_tests,
};
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

struct Source {
    key: SigningKey,
    public: KagemushaDevicePublicKeyV1,
    calls: AtomicUsize,
    wrong_message: AtomicBool,
    drift_after_sign: AtomicBool,
    live: AtomicBool,
}
impl KagemushaNativeCoreAuthorizationSignerV1 for Source {
    fn recheck_originals(&self) -> Result<(), Error> {
        if self.live.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(Error::Rejected)
        }
    }
    fn original_public_key(&self) -> Result<KagemushaDevicePublicKeyV1, Error> {
        Ok(self.public)
    }
    fn sign_authorization_id(&self, mut id: [u8; 32]) -> Result<Vec<u8>, Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.wrong_message.load(Ordering::SeqCst) {
            id[0] ^= 1;
        }
        let signature: Signature = self.key.sign(&id);
        if self.drift_after_sign.load(Ordering::SeqCst) {
            self.live.store(false, Ordering::SeqCst);
        }
        Ok(signature
            .normalize_s()
            .unwrap_or(signature)
            .to_bytes()
            .to_vec())
    }
}
fn fixture() -> (SenderCommandV1, SenderHardwareAuthorizationV1, Arc<Source>) {
    let command = canonical_command_body_for_tests(7).unwrap();
    let command: SenderCommandV1 = norito::decode_from_bytes(&command).unwrap();
    let SenderCommandBodyV1::Commit {
        hardware_authorization,
        ..
    } = &command.body
    else {
        panic!("fixture command7");
    };
    let auth =
        SenderHardwareAuthorizationV1::decode_canonical_exact(hardware_authorization).unwrap();
    let source = Arc::new(Source {
        key: SigningKey::from_bytes((&[0x61; 32]).into()).unwrap(),
        public: auth.authorization_public_key,
        calls: AtomicUsize::new(0),
        wrong_message: AtomicBool::new(false),
        drift_after_sign: AtomicBool::new(false),
        live: AtomicBool::new(true),
    });
    (command, auth, source)
}
#[test]
fn actual_original_signature_is_independently_verified_against_exact_command7() {
    let (command, auth, source) = fixture();
    let retained =
        RetainedCoreAuthorizationSignerV1::new(auth.authorization_public_key, source.clone())
            .unwrap();
    let signed = retained
        .sign_command7(auth.unsigned_preimage(), &command.context)
        .unwrap();
    let output = SenderHardwareAuthorizationV1::decode_canonical_exact(&signed).unwrap();
    assert_eq!(output, auth);
    assert_eq!(source.calls.load(Ordering::SeqCst), 1);
}
#[test]
fn foreign_key_or_unsigned_scope_refuses_before_native_signing() {
    let (command, auth, source) = fixture();
    let foreign = KagemushaDevicePublicKeyV1::from_sec1_bytes(
        SigningKey::from_bytes((&[0x62; 32]).into())
            .unwrap()
            .verifying_key()
            .to_encoded_point(false)
            .as_bytes(),
    )
    .unwrap();
    assert!(RetainedCoreAuthorizationSignerV1::new(foreign, source.clone()).is_err());
    let retained =
        RetainedCoreAuthorizationSignerV1::new(auth.authorization_public_key, source.clone())
            .unwrap();
    let mut scope = command.context;
    scope.core_authorization_key_reference = [99; 32];
    assert!(
        retained
            .sign_command7(auth.unsigned_preimage(), &scope)
            .is_err()
    );
    assert_eq!(source.calls.load(Ordering::SeqCst), 0);
}
#[test]
fn wrong_signed_transcript_and_original_custody_loss_never_publish_authorization() {
    let (command, auth, source) = fixture();
    let retained =
        RetainedCoreAuthorizationSignerV1::new(auth.authorization_public_key, source.clone())
            .unwrap();
    source.wrong_message.store(true, Ordering::SeqCst);
    assert!(
        retained
            .sign_command7(auth.unsigned_preimage(), &command.context)
            .is_err()
    );
    source.wrong_message.store(false, Ordering::SeqCst);
    source.drift_after_sign.store(true, Ordering::SeqCst);
    assert!(
        retained
            .sign_command7(auth.unsigned_preimage(), &command.context)
            .is_err()
    );
    assert_eq!(source.calls.load(Ordering::SeqCst), 2);
}
