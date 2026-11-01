//! Android JNI upcalls over a retained opaque Kotlin platform object.

use super::{
    Failure, INVALID, Result, advance,
    platform::{PlatformReply, reason},
};
use advance::{
    KagemushaWalletPlatformV1 as Platform, KagemushaWalletProbeV1 as Probe,
    KagemushaWalletSlotIdV1 as Slot, KagemushaWalletUnavailableV1 as U,
};
use iroha_data_model::kagemusha::KagemushaDevicePublicKeyV1 as PublicKey;
use jni::{
    Env, JavaVM,
    objects::{Global, JByteArray, JObject, JObjectArray, JValue},
};

/// Retains one Kotlin platform object. Each upcall gets the calling thread's JNI environment:
/// a Kotlin thread is already attached, and a Rust-owned thread (the background fold worker)
/// is attached only for the upcall and detached before it returns. No environment
/// or reply is kept between upcalls: scoped attachments cannot hold up JVM shutdown,
/// and each call runs inside its own local frame and leaves no Java
/// exception pending. The callback exposes no public arbitrary-sign method and receives only
/// G2 typed messages.
pub struct AndroidPlatform {
    vm: JavaVM,
    object: Global<JObject<'static>>,
}
impl AndroidPlatform {
    /// Retain a Kotlin KagemushaWalletAndroidPlatformV1 and check its mandatory no-anchor policy.
    /// # Errors
    /// Rejects another class, a throwing upcall or an unexpected anchor policy.
    pub fn new(env: &mut Env<'_>, object: &JObject<'_>) -> Result<Self> {
        let result = (|| -> jni::errors::Result<Self> {
            if !env.is_instance_of(
                object,
                jni::jni_str!(
                    "org/hyperledger/iroha/sdk/offline/wallet/KagemushaWalletAndroidPlatformV1"
                ),
            )? {
                return Err(jni::errors::Error::WrongJValueType(
                    "platform",
                    "wallet platform",
                ));
            }
            if env
                .call_method(
                    object,
                    jni::jni_str!("anchorPolicyTag"),
                    jni::jni_sig!("()I"),
                    &[],
                )?
                .i()?
                != 0
            {
                return Err(jni::errors::Error::WrongJValueType(
                    "anchor",
                    "not required",
                ));
            }
            Ok(Self {
                vm: env.get_java_vm()?,
                object: env.new_global_ref(object)?,
            })
        })();
        if result.is_err() {
            env.exception_clear();
        }
        result.map_err(|_| Failure::code(INVALID))
    }
    fn call(
        &self,
        operation: i32,
        slot: Option<&Slot>,
        input: &[u8],
        auxiliary: i32,
        bound: usize,
    ) -> (PlatformReply, Vec<u8>) {
        let result = self.vm.attach_current_thread_for_scope(|env| {
        let result = env.with_local_frame(16, |env| -> jni::errors::Result<(PlatformReply, Vec<u8>)> {
            let slot = env.byte_array_from_slice(slot.map_or(&[], |s| &s.0))?;
            let input = env.byte_array_from_slice(input)?;
            let reply = env.call_method(self.object.as_obj(), jni::jni_str!("nativeCall"), jni::jni_sig!("(I[B[BI)Lorg/hyperledger/iroha/sdk/offline/wallet/KagemushaWalletNativeReplyV1;"), &[JValue::Int(operation), JValue::Object(&slot), JValue::Object(&input), JValue::Int(auxiliary)])?.l()?;
            let tag = env.get_field(&reply, jni::jni_str!("tag"), jni::jni_sig!("I"))?.i()?;
            let reason = env.get_field(&reply, jni::jni_str!("reason"), jni::jni_sig!("I"))?.i()?;
            let code = env.get_field(&reply, jni::jni_str!("code"), jni::jni_sig!("I"))?.i()?;
            let bytes = env.call_method(&reply, jni::jni_str!("bytes"), jni::jni_sig!("()[B"), &[])?.l()?;
            let bytes = JByteArray::cast_local(env, bytes)?;
            let len = bytes.len(env)?;
            if len > bound { return Err(jni::errors::Error::WrongJValueType("oversized", "bounded bytes")); }
            let bytes = env.convert_byte_array(&bytes)?;
            Ok((PlatformReply { tag: tag as u32, reason: reason as u32, code, length: bytes.len() }, bytes))
        });
        let result = result.and_then(|value| no_pending_exception(env).map(|()| value));
        if result.is_err() {
            env.exception_clear();
        }
        result
        });
        match result {
            Ok(value) => value,
            Err(_) => (
                PlatformReply {
                    tag: u32::MAX,
                    ..PlatformReply::default()
                },
                vec![],
            ),
        }
    }
    /// Return the credential-encrypted no-backup root; Rust creates/opens it durably.
    /// # Errors
    /// Reject unavailable or malformed paths; nothing is inferred about key/custody absence.
    pub fn custody_root(&self) -> std::result::Result<std::path::PathBuf, U> {
        let (reply, bytes) = self.call(9, None, &[], 0, 4096);
        if reply.tag == 2 && bytes.is_empty() {
            return Err(reason(reply));
        }
        if reply.tag != 0 || bytes.contains(&0) {
            return Err(U::Platform(0));
        }
        let path =
            std::path::PathBuf::from(std::str::from_utf8(&bytes).map_err(|_| U::Platform(0))?);
        if !path.is_absolute() {
            return Err(U::Platform(0));
        }
        Ok(path)
    }
}
// A successful upcall that still has a pending Java exception fails closed (cleared by caller).
fn no_pending_exception(env: &mut Env<'_>) -> jni::errors::Result<()> {
    if env.exception_check() {
        return Err(jni::errors::Error::JavaException);
    }
    Ok(())
}
impl Platform for AndroidPlatform {
    fn key_probe(&self, slot: &Slot) -> Probe<PublicKey> {
        let (reply, bytes) = self.call(0, Some(slot), &[], 0, 65);
        match reply.tag {
            0 => PublicKey::from_sec1_bytes(&bytes)
                .map_or(Probe::Unavailable(U::KeyUnusable), Probe::Present),
            1 if bytes.is_empty() => Probe::Absent,
            2 if bytes.is_empty() => Probe::Unavailable(reason(reply)),
            _ => Probe::Unavailable(U::Platform(0)),
        }
    }
    fn key_attestation_chain(&self, slot: &Slot) -> Probe<Vec<Vec<u8>>> {
        let answer = self.vm.attach_current_thread_for_scope(|env| {
        let answer = env.with_local_frame(32, |env| -> jni::errors::Result<(PlatformReply,Vec<Vec<u8>>)> {
            let slot = env.byte_array_from_slice(&slot.0)?;
            let empty = env.byte_array_from_slice(&[])?;
            let reply = env.call_method(self.object.as_obj(), jni::jni_str!("nativeCall"),
                jni::jni_sig!("(I[B[BI)Lorg/hyperledger/iroha/sdk/offline/wallet/KagemushaWalletNativeReplyV1;"),
                &[JValue::Int(12),JValue::Object(&slot),JValue::Object(&empty),JValue::Int(0)])?.l()?;
            let tag=env.get_field(&reply,jni::jni_str!("tag"),jni::jni_sig!("I"))?.i()?;
            let reason=env.get_field(&reply,jni::jni_str!("reason"),jni::jni_sig!("I"))?.i()?;
            let code=env.get_field(&reply,jni::jni_str!("code"),jni::jni_sig!("I"))?.i()?;
            let bytes=env.call_method(&reply,jni::jni_str!("bytes"),jni::jni_sig!("()[B"),&[])?.l()?;
            let bytes=JByteArray::cast_local(env,bytes)?;
            if bytes.len(env)? != 0 { return Err(jni::errors::Error::WrongJValueType("nonempty", "chain-only bytes")); }
            let originals=env.call_method(&reply,jni::jni_str!("certificatesDer"),jni::jni_sig!("()[[B"),&[])?.l()?;
            let originals=JObjectArray::<JByteArray>::cast_local(env, originals)?;
            let count=originals.len(env)?;
            if (tag==0 && !(2..=8).contains(&count)) || (tag!=0 && count!=0) {
                return Err(jni::errors::Error::WrongJValueType("chain count", "bounded original DER chain"));
            }
            let mut chain=Vec::with_capacity(count);
            for index in 0..count {
                let original=originals.get_element(env,index)?;
                let length=original.len(env)?;
                if !(1..=16_384).contains(&length) { return Err(jni::errors::Error::WrongJValueType("DER extent", "bounded original certificate")); }
                chain.push(env.convert_byte_array(&original)?);
            }
            Ok((PlatformReply {tag:tag as u32,reason:reason as u32,code,length:0},chain))
        });
        let answer = answer.and_then(|value| no_pending_exception(env).map(|()| value));
        if answer.is_err() {
            env.exception_clear();
        }
        answer
        });
        match answer {
            Ok((reply, chain)) => match reply.tag {
                0 => Probe::Present(chain),
                1 if chain.is_empty() => Probe::Absent,
                2 if chain.is_empty() => Probe::Unavailable(reason(reply)),
                _ => Probe::Unavailable(U::Platform(0)),
            },
            Err(_) => Probe::Unavailable(U::Platform(0)),
        }
    }

    fn key_generate(
        &self,
        slot: &Slot,
        request: &advance::KagemushaWalletKeyGenerationRequestV1,
    ) -> advance::KagemushaWalletKeyGenerationV1 {
        use advance::KagemushaWalletKeyGenerationV1 as G;
        let (reply, bytes) = self.call(
            1,
            Some(slot),
            &request.challenge_digest,
            i32::from(request.profile.tag()),
            65,
        );
        match reply.tag {
            0 => PublicKey::from_sec1_bytes(&bytes)
                .map_or(G::Unavailable(U::KeyUnusable), G::Generated),
            3 if bytes.is_empty() => G::AlreadyPresent,
            2 if bytes.is_empty() => G::Unavailable(reason(reply)),
            _ => G::Unavailable(U::Platform(0)),
        }
    }
    fn key_recover_generation_reply(
        &self,
        slot: &Slot,
        request: &advance::KagemushaWalletKeyGenerationRequestV1,
    ) -> std::result::Result<Option<PublicKey>, U> {
        let (reply, bytes) = self.call(
            13,
            Some(slot),
            &request.challenge_digest,
            i32::from(request.profile.tag()),
            65,
        );
        match reply.tag {
            0 => PublicKey::from_sec1_bytes(&bytes)
                .map(Some)
                .map_err(|_| U::KeyUnusable),
            // This is no retained successful return, never a hardware key-absence verdict.
            3 if bytes.is_empty() => Ok(None),
            2 if bytes.is_empty() => Err(reason(reply)),
            _ => Err(U::Platform(0)),
        }
    }

    fn key_generation_policy(
        &self,
    ) -> std::result::Result<advance::KagemushaWalletKeyGenerationPolicyV1, U> {
        use advance::KagemushaWalletKeyGenerationPolicyV1 as P;
        let (reply, bytes) = self.call(10, None, &[], 0, 0);
        match (reply.tag, reply.reason, reply.code, bytes.is_empty()) {
            (0, 0, 0, true) => Ok(P::DefinitiveAbsence),
            (0, 0, 1, true) => Ok(P::FreshEnrollmentOnly),
            (2, _, _, true) => Err(reason(reply)),
            _ => Err(U::Platform(0)),
        }
    }

    fn key_generate_fresh(
        &self,
        grant: advance::KagemushaWalletFreshGenerationV1<'_>,
    ) -> advance::KagemushaWalletKeyGenerationV1 {
        use advance::KagemushaWalletKeyGenerationV1 as G;
        // Consuming this non-Clone Native grant binds the exact live owner, slot, issuer
        // challenge and hardware profile before the sole private JNI upcall. No caller
        // boolean, decoded intent or Kotlin DTO can manufacture this grant.
        let (slot, request) = match grant.consume(self) {
            Ok(bound) => bound,
            Err(reason) => return G::Unavailable(reason),
        };
        let (reply, bytes) = self.call(
            11,
            Some(&slot),
            &request.challenge_digest,
            i32::from(request.profile.tag()),
            65,
        );
        match reply.tag {
            0 => PublicKey::from_sec1_bytes(&bytes)
                .map_or(G::Unavailable(U::KeyUnusable), G::Generated),
            3 if bytes.is_empty() => G::AlreadyPresent,
            2 if bytes.is_empty() => G::Unavailable(reason(reply)),
            _ => G::Unavailable(U::Platform(0)),
        }
    }

    fn key_sign(
        &self,
        slot: &Slot,
        message: advance::KagemushaWalletSignMessageV1<'_>,
    ) -> std::result::Result<advance::KagemushaWalletPlatformSignatureV1, U> {
        let (reply, bytes) = self.call(2, Some(slot), message.as_bytes(), 0, 72);
        match reply.tag {
            0 if (8..=72).contains(&bytes.len()) => {
                Ok(advance::KagemushaWalletPlatformSignatureV1::Der(bytes))
            }
            2 if bytes.is_empty() => Err(reason(reply)),
            _ => Err(U::Platform(0)),
        }
    }
    fn key_delete(&self, slot: &Slot) -> advance::KagemushaWalletRemoveOutcomeV1 {
        use advance::KagemushaWalletRemoveOutcomeV1 as R;
        let (reply, _) = self.call(3, Some(slot), &[], 0, 0);
        match reply.tag {
            0 => R::Removed,
            2 => R::NotRemoved(reason(reply)),
            4 => R::Uncertain(reason(reply)),
            _ => R::Uncertain(U::Platform(0)),
        }
    }
    fn anchor_policy(&self) -> advance::KagemushaWalletAnchorPolicyV1 {
        advance::KagemushaWalletAnchorPolicyV1::NotRequired
    }
    fn storage_state(&self) -> std::result::Result<(), U> {
        let (reply, _) = self.call(7, None, &[], 0, 0);
        match reply.tag {
            0 => Ok(()),
            2 => Err(reason(reply)),
            _ => Err(U::Platform(0)),
        }
    }
}
