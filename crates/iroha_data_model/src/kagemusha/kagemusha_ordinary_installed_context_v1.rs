//! Ordinary application installed-context DATA and purpose authentication only.
//! This family grants no account, first-device, FI, State, Guard or money authority.
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_crypto::{Algorithm, PublicKey, Signature};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

/// Sole ordinary installed-context approval domain, including NUL.
pub const KAGEMUSHA_ORDINARY_INSTALLED_CONTEXT_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:ordinary-installed-context-nonmonetary\0";
/// Complete public context input bound; proof artifacts have independent streamed bounds.
pub const KAGEMUSHA_ORDINARY_INSTALLED_CONTEXT_MAX_V1: usize = 256 * 1024;
const ROOT_MAGIC: &[u8] = b"KGMROOT1";
const MAX_TOTAL: u64 = 16 * 1024 * 1024 * 1024;
type Result<T> = core::result::Result<T, String>;

/// Common SDK compile original: no bank, FI, package, runtime, JNI hash or final SDK body hash.
/// Approval of this public build input belongs to the independently authenticated SDK producer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KagemushaOrdinaryInstalledContextCompiledBindingV1 {
    /// Existing independently selected mobile SDK signer; not a newly generated key.
    pub sdk_signer: [u8; 32],
    /// Exact admitted common SDK/native source inventory, fixed before artifact compilation.
    pub sdk_source_sha256: [u8; 32],
    /// Exact common Native bridge ABI compiled by that same source producer.
    pub native_abi: u32,
}
impl KagemushaOrdinaryInstalledContextCompiledBindingV1 {
    /// Shape encoder only; it neither approves the input nor signs a release.
    /// # Errors
    /// Rejects a zero SDK signer or source digest, or a zero Native ABI.
    pub fn encode_original(&self) -> Result<Vec<u8>> {
        nonzero(self.sdk_signer)?;
        nonzero(self.sdk_source_sha256)?;
        if self.native_abi == 0 {
            return Err("ordinary compiled ABI rejected".into());
        }
        let mut bytes = ROOT_MAGIC.to_vec();
        bytes.extend(self.sdk_signer);
        bytes.extend(self.sdk_source_sha256);
        bytes.extend(self.native_abi.to_le_bytes());
        Ok(bytes)
    }
    /// Parse the sole exact complete public compile original. Decoding grants no root.
    /// # Errors
    /// Rejects empty, oversized, truncated or incorrectly framed input and invalid compiled bindings.
    pub fn decode_original(bytes: &[u8]) -> Result<Self> {
        let mut r = Reader::new(bytes, 76)?;
        r.expect(ROOT_MAGIC)?;
        let result = Self {
            sdk_signer: r.digest()?,
            sdk_source_sha256: r.digest()?,
            native_abi: r.u32()?,
        };
        if result.encode_original()? != bytes {
            return Err("ordinary compiled original rejected".into());
        }
        Ok(result)
    }
}

/// Exact public installed original descriptor. This is DATA, not filesystem custody.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryInstalledContextFileV1")]
pub struct KagemushaOrdinaryInstalledContextFileV1 {
    /// Canonical Native relative path with an exact declared original/artifact role.
    pub path: String,
    /// Complete public original byte identity.
    pub sha256: [u8; 32],
    /// Complete size; only content-addressed proof artifacts can exceed 16 MiB.
    pub byte_len: u64,
}
/// One genuine measured library selection, in strictly sorted unique Android ABI order.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryInstalledContextLibraryV1")]
pub struct KagemushaOrdinaryInstalledContextLibraryV1 {
    /// Android packaging ABI, later selected by Native's own compiled target.
    pub abi: String,
    /// Complete loaded JNI original SHA256.
    pub sha256: [u8; 32],
}
/// Sole nonmonetary context preimage. All three existing roles approve every field.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryInstalledContextV1")]
pub struct KagemushaOrdinaryInstalledContextV1 {
    /// Sole first-release layout version.
    pub version: u16,
    /// Exactly one: ordinary nonmonetary installed context, never first-device or money authority.
    pub purpose: u8,
    /// Existing app release role delegated by the independently compiled SDK role approval.
    pub app_signer: [u8; 32],
    /// Existing runtime role; also the ordinary signed inventory authority.
    pub runtime_signer: [u8; 32],
    /// Must equal the independently compiled common SDK role, never an offered root.
    pub sdk_signer: [u8; 32],
    /// Genuine source-owned context policy original SHA, admitted before producing signatures.
    pub source_policy_sha256: [u8; 32],
    /// Exact app source inventory admitted by the existing Git source owner.
    pub app_source_sha256: [u8; 32],
    /// Exact common Native SDK source inventory, also in the compiled original.
    pub sdk_source_sha256: [u8; 32],
    /// Exact already-finalized runtime body original SHA; packet digest is not inserted into it.
    pub runtime_manifest_sha256: [u8; 32],
    /// Exact already-finalized SDK body original SHA; context is external to its artifact.
    pub sdk_release_sha256: [u8; 32],
    /// Actual existing source-policy runtime signature domain, approved by all three roles.
    pub runtime_signature_domain: String,
    /// Actual existing source-policy SDK signature domain, approved by all three roles.
    pub sdk_signature_domain: String,
    /// Unmodified runtime-domain Ed25519 signature over the complete runtime original.
    pub runtime_signature: [u8; 64],
    /// Unmodified SDK-domain Ed25519 signature over the complete SDK original.
    pub sdk_signature: [u8; 64],
    /// Actual installed package name; not a UI or server override.
    pub package_name: String,
    /// Actual package version code, checked against Android framework metadata.
    pub version_code: u64,
    /// SHA of the sole actual package signing certificate.
    pub certificate_sha256: [u8; 32],
    /// Exact ordered DEX digest; excludes signed context assets and APK signing blocks.
    pub dex_sha256: [u8; 32],
    /// Common ABI contract; must equal the actual loaded Native implementation.
    pub native_abi: u32,
    /// Exact independently admitted library originals for this package.
    pub libraries: Vec<KagemushaOrdinaryInstalledContextLibraryV1>,
    /// Actual signed ordinary inventory original; its body remains admitted by Native's sole codec.
    pub inventory_sha256: [u8; 32],
    /// Independently source-selected positive u64 floor, never chosen by the packet.
    pub minimum_sequence: u64,
    /// SHA of the complete existing recursive profile digest preimage original.
    pub recursive_profile_sha256: [u8; 32],
    /// Exact independently approved released profile original size, bounded to 16 MiB.
    pub recursive_profile_size: u64,
    /// Existing threshold release native-layout identity, joined again by artifact loading.
    pub native_layout_digest: [u8; 32],
    /// Full exact public copy roster. Native intake independently checks its packet/body roles.
    pub originals: Vec<KagemushaOrdinaryInstalledContextFileV1>,
}
impl KagemushaOrdinaryInstalledContextV1 {
    /// Validate DATA only. No certificate, measured code, signature or descriptor authority occurs.
    /// # Errors
    /// Rejects zero binding digests, invalid version, purpose, sequence floor or Native ABI,
    /// conflicting role keys or signature domains, malformed package or library declarations,
    /// and disallowed or oversized public originals and recursive profiles.
    pub fn validate(&self) -> Result<()> {
        for value in [
            self.app_signer,
            self.runtime_signer,
            self.sdk_signer,
            self.source_policy_sha256,
            self.app_source_sha256,
            self.sdk_source_sha256,
            self.runtime_manifest_sha256,
            self.sdk_release_sha256,
            self.certificate_sha256,
            self.dex_sha256,
            self.inventory_sha256,
            self.recursive_profile_sha256,
            self.native_layout_digest,
        ] {
            nonzero(value)?;
        }
        if self.version != 1
            || self.purpose != 1
            || self.app_signer == self.runtime_signer
            || self.app_signer == self.sdk_signer
            || self.runtime_signer == self.sdk_signer
            || !valid_domain(&self.runtime_signature_domain)
            || !valid_domain(&self.sdk_signature_domain)
            || self.runtime_signature_domain == self.sdk_signature_domain
            || self.version_code == 0
            || self.minimum_sequence == 0
            || self.native_abi == 0
            || self.recursive_profile_size == 0
            || self.recursive_profile_size > 16 * 1024 * 1024
            || self.package_name.len() > 128
            || !self.package_name.contains('.')
            || !self.package_name.split('.').all(|part| {
                !part.is_empty() && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            })
            || self.libraries.is_empty()
            || self.libraries.len() > 4
            || !self.libraries.windows(2).all(|w| w[0].abi < w[1].abi)
            || self.libraries.iter().any(|l| {
                l.sha256 == [0; 32]
                    || !matches!(
                        l.abi.as_str(),
                        "arm64-v8a" | "armeabi-v7a" | "x86" | "x86_64"
                    )
            })
            || self.originals.is_empty()
            || self.originals.len() > 130
            || !self.originals.windows(2).all(|w| w[0].path < w[1].path)
        {
            return Err("ordinary installed context DATA rejected".into());
        }
        let mut total = 0_u64;
        for file in &self.originals {
            nonzero(file.sha256)?;
            let artifact = file.path.strip_prefix("artifacts/").is_some_and(|s| {
                s.len() == 64
                    && s.bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            });
            let public = matches!(
                file.path.as_str(),
                "originals/authority-policy.norito"
                    | "originals/release-manifest.norito"
                    | "originals/validation-receipt.norito"
                    | "originals/release-attestation.norito"
                    | "originals/issuer-policy.norito"
                    | "originals/ordinary-identity-authority.norito"
                    | "originals/ordinary-identity-policy.norito"
                    | "originals/ordinary-core-issuer-policy.norito"
                    | "originals/ordinary-lineage-cas-policy.norito"
                    | "originals/ordinary-trust.norito"
                    | "originals/app-authority.bin"
                    | "originals/finality-checkpoint.norito"
                    | "originals/integrity-provider-policy.norito"
            );
            if !(artifact || public)
                || file.byte_len == 0
                || file.byte_len
                    > if artifact {
                        MAX_TOTAL
                    } else {
                        16 * 1024 * 1024
                    }
            {
                return Err("ordinary context public original role rejected".into());
            }
            total = total
                .checked_add(file.byte_len)
                .ok_or("ordinary context original sum overflow")?;
            if total > MAX_TOTAL {
                return Err("ordinary context original total rejected".into());
            }
        }
        Ok(())
    }
    /// Sole `DataModel` canonical Norito encoder; no JS codec, signature or authority is generated.
    /// # Errors
    /// Rejects shape, purpose or complete canonical archive bound failures.
    pub fn encode_original(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let bytes = norito::encode_canonical(self).map_err(|e| e.to_string())?;
        if bytes.is_empty() || bytes.len() > KAGEMUSHA_ORDINARY_INSTALLED_CONTEXT_MAX_V1 {
            return Err("ordinary context canonical Norito bound rejected".into());
        }
        Ok(bytes)
    }
    /// Parse exactly the sole canonical Norito layout under payload-derived resource limits.
    /// Decoding is DATA only, without installation, current membership or financial authority.
    /// # Errors
    /// Rejects alternate framing/flags/compression, suffixes, shape or allocation bounds.
    pub fn decode_original(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() || bytes.len() > KAGEMUSHA_ORDINARY_INSTALLED_CONTEXT_MAX_V1 {
            return Err("ordinary context canonical Norito input bound rejected".into());
        }
        let value: Self = norito::decode_canonical(bytes).map_err(|e| e.to_string())?;
        value.validate()?;
        norito::verify_exact_canonical_frame(&value, bytes).map_err(|e| e.to_string())?;
        Ok(value)
    }
    /// Verify three mandatory distinct existing role approvals and both complete parent signatures.
    /// `compiled` must be the producer-admitted original embedded in the common measured SDK;
    /// offered bytes cannot select it. Package and FD custody are separately mandatory in Native.
    /// # Errors
    /// Rejects invalid DATA or compiled bindings, mismatched SDK or parent originals,
    /// and invalid role keys, context approvals or parent signatures.
    pub fn authenticate(
        &self,
        compiled: &KagemushaOrdinaryInstalledContextCompiledBindingV1,
        approvals: &[[u8; 64]; 3],
        runtime_original: &[u8],
        sdk_original: &[u8],
    ) -> Result<()> {
        self.validate()?;
        compiled.encode_original()?;
        if self.sdk_signer != compiled.sdk_signer
            || self.sdk_source_sha256 != compiled.sdk_source_sha256
            || self.native_abi != compiled.native_abi
            || runtime_original.is_empty()
            || sdk_original.is_empty()
            || runtime_original.len() > 32 * 1024 * 1024
            || sdk_original.len() > 32 * 1024 * 1024
            || <[u8; 32]>::from(Sha256::digest(runtime_original)) != self.runtime_manifest_sha256
            || <[u8; 32]>::from(Sha256::digest(sdk_original)) != self.sdk_release_sha256
        {
            return Err("ordinary context independent SDK/parent binding rejected".into());
        }
        let mut message = KAGEMUSHA_ORDINARY_INSTALLED_CONTEXT_DOMAIN_V1.to_vec();
        message.extend(self.encode_original()?);
        // SDK approval is the independent root. It explicitly delegates the exact app/runtime
        // roles from the admitted source policy. Neither external role key is independently trusted.
        for (key, signature) in [self.app_signer, self.runtime_signer, compiled.sdk_signer]
            .into_iter()
            .zip(approvals)
        {
            verify(key, signature, &message)?;
        }
        let mut runtime = self.runtime_signature_domain.as_bytes().to_vec();
        runtime.push(0);
        runtime.extend(runtime_original);
        verify(self.runtime_signer, &self.runtime_signature, &runtime)?;
        let mut sdk = self.sdk_signature_domain.as_bytes().to_vec();
        sdk.push(0);
        sdk.extend(sdk_original);
        verify(compiled.sdk_signer, &self.sdk_signature, &sdk)
    }
}
fn verify(key: [u8; 32], signature: &[u8; 64], message: &[u8]) -> Result<()> {
    let key = PublicKey::from_bytes(Algorithm::Ed25519, &key)
        .map_err(|_| "ordinary context role key rejected")?;
    Signature::from_bytes(signature)
        .verify(&key, message)
        .map_err(|_| "ordinary context role signature rejected".into())
}
fn nonzero(d: [u8; 32]) -> Result<()> {
    if d == [0; 32] {
        Err("ordinary context nonzero binding rejected".into())
    } else {
        Ok(())
    }
}
struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn new(b: &'a [u8], max: usize) -> Result<Self> {
        if b.is_empty() || b.len() > max {
            return Err("ordinary context input bound rejected".into());
        }
        Ok(Self { b, at: 0 })
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .at
            .checked_add(n)
            .ok_or("ordinary context size overflow")?;
        let b = self
            .b
            .get(self.at..end)
            .ok_or("ordinary context truncated")?;
        self.at = end;
        Ok(b)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        self.take(N)?
            .try_into()
            .map_err(|_| "ordinary context fixed width rejected".into())
    }
    fn expect(&mut self, b: &[u8]) -> Result<()> {
        if self.take(b.len())? != b {
            return Err("ordinary context magic rejected".into());
        }
        Ok(())
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn digest(&mut self) -> Result<[u8; 32]> {
        self.array()
    }
}

fn valid_domain(s: &str) -> bool {
    s.len() <= 128
        && !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b':' | b'-' | b'_'))
        && !s.starts_with("iroha:kagemusha:v1:ordinary-installed-context")
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::KeyPair;
    // Synthetic keys belong only to schema/crypto unit regressions. No installed Android,
    // Root, released proof artifact, current account, FI or monetary receipt is produced.
    fn fixture() -> (
        KagemushaOrdinaryInstalledContextV1,
        KagemushaOrdinaryInstalledContextCompiledBindingV1,
        [KeyPair; 3],
        Vec<u8>,
        Vec<u8>,
    ) {
        let keys = std::array::from_fn(|i| {
            KeyPair::from_seed(
                vec![21 + u8::try_from(i).expect("three synthetic authority keys"); 32],
                Algorithm::Ed25519,
            )
        });
        let public = |i: usize| keys[i].public_key().to_bytes().1.try_into().unwrap();
        let runtime = b"unit-only-runtime-original".to_vec();
        let sdk = b"unit-only-sdk-original".to_vec();
        let sign = |key: usize, domain: &str, body: &[u8]| {
            let mut m = domain.as_bytes().to_vec();
            m.push(0);
            m.extend(body);
            Signature::try_new(keys[key].private_key(), &m)
                .unwrap()
                .payload()
                .try_into()
                .unwrap()
        };
        let data = KagemushaOrdinaryInstalledContextV1 {
            version: 1,
            purpose: 1,
            app_signer: public(0),
            runtime_signer: public(1),
            sdk_signer: public(2),
            source_policy_sha256: [1; 32],
            app_source_sha256: [2; 32],
            sdk_source_sha256: [3; 32],
            runtime_manifest_sha256: Sha256::digest(&runtime).into(),
            sdk_release_sha256: Sha256::digest(&sdk).into(),
            runtime_signature_domain: "unit:runtime:v1".into(),
            sdk_signature_domain: "unit:sdk:v1".into(),
            runtime_signature: sign(1, "unit:runtime:v1", &runtime),
            sdk_signature: sign(2, "unit:sdk:v1", &sdk),
            package_name: "unit.fixture".into(),
            version_code: 1,
            certificate_sha256: [4; 32],
            dex_sha256: [5; 32],
            native_abi: 25,
            libraries: vec![KagemushaOrdinaryInstalledContextLibraryV1 {
                abi: "arm64-v8a".into(),
                sha256: [6; 32],
            }],
            inventory_sha256: [7; 32],
            minimum_sequence: u64::MAX,
            recursive_profile_sha256: [8; 32],
            recursive_profile_size: 1,
            native_layout_digest: [8; 32],
            originals: vec![KagemushaOrdinaryInstalledContextFileV1 {
                path: "originals/authority-policy.norito".into(),
                sha256: [9; 32],
                byte_len: 1,
            }],
        };
        let compiled = KagemushaOrdinaryInstalledContextCompiledBindingV1 {
            sdk_signer: public(2),
            sdk_source_sha256: [3; 32],
            native_abi: 25,
        };
        (data, compiled, keys, runtime, sdk)
    }
    fn approvals(data: &KagemushaOrdinaryInstalledContextV1, keys: &[KeyPair; 3]) -> [[u8; 64]; 3] {
        let mut m = KAGEMUSHA_ORDINARY_INSTALLED_CONTEXT_DOMAIN_V1.to_vec();
        m.extend(data.encode_original().unwrap());
        std::array::from_fn(|i| {
            Signature::try_new(keys[i].private_key(), &m)
                .unwrap()
                .payload()
                .try_into()
                .unwrap()
        })
    }
    #[test]
    fn ordinary_context_data_roundtrip_preserves_full_u64_floor() {
        let (data, compiled, _, _, _) = fixture();
        let b = data.encode_original().unwrap();
        assert_eq!(
            KagemushaOrdinaryInstalledContextV1::decode_original(&b).unwrap(),
            data
        );
        assert_eq!(
            KagemushaOrdinaryInstalledContextCompiledBindingV1::decode_original(
                &compiled.encode_original().unwrap()
            )
            .unwrap(),
            compiled
        );
        for n in [0, 1, b.len() - 1] {
            assert!(KagemushaOrdinaryInstalledContextV1::decode_original(&b[..n]).is_err());
        }
        let mut trailing = b;
        trailing.push(0);
        assert!(KagemushaOrdinaryInstalledContextV1::decode_original(&trailing).is_err());
    }
    #[test]
    fn ordinary_context_three_real_signatures_bind_sources_package_floor_and_profile() {
        let (data, compiled, keys, runtime, sdk) = fixture();
        let approvals = approvals(&data, &keys);
        assert!(
            data.authenticate(&compiled, &approvals, &runtime, &sdk)
                .is_ok()
        );
        for index in 0..8 {
            let mut changed = data.clone();
            match index {
                0 => changed.app_source_sha256[0] ^= 1,
                1 => changed.dex_sha256[0] ^= 1,
                2 => changed.minimum_sequence -= 1,
                3 => changed.native_layout_digest[0] ^= 1,
                4 => changed.version_code += 1,
                5 => changed.runtime_signature_domain = "unit:foreign:v1".into(),
                6 => changed.version = 2,
                _ => changed.purpose = 2,
            }
            assert!(
                changed
                    .authenticate(&compiled, &approvals, &runtime, &sdk)
                    .is_err()
            );
        }
        for index in 0..3 {
            let mut changed = approvals;
            changed[index][0] ^= 1;
            assert!(
                data.authenticate(&compiled, &changed, &runtime, &sdk)
                    .is_err()
            );
        }
    }
    #[test]
    fn ordinary_context_offered_sdk_root_and_parent_substitution_refuse() {
        let (data, compiled, keys, runtime, sdk) = fixture();
        let approvals = approvals(&data, &keys);
        let mut foreign = compiled;
        foreign.sdk_signer = keys[0].public_key().to_bytes().1.try_into().unwrap();
        assert!(
            data.authenticate(&foreign, &approvals, &runtime, &sdk)
                .is_err()
        );
        let mut changed = runtime;
        changed.push(0);
        assert!(
            data.authenticate(&compiled, &approvals, &changed, &sdk)
                .is_err()
        );
        let mut changed = data.clone();
        changed.runtime_signature[0] ^= 1;
        let new_approvals = super::tests::approvals(&changed, &keys);
        // Even three new context approvals cannot replace the actual signed parent original.
        assert!(
            changed
                .authenticate(
                    &compiled,
                    &new_approvals,
                    b"unit-only-runtime-original",
                    &sdk
                )
                .is_err()
        );
    }
    #[test]
    fn ordinary_context_material_path_and_aggregate_bound_refuse_before_files() {
        let (mut data, _, _, _, _) = fixture();
        data.originals[0].path = "originals/private-seed.bin".into();
        assert!(data.encode_original().is_err());
        data.originals[0].path = format!("artifacts/{}", "a".repeat(64));
        data.originals[0].byte_len = MAX_TOTAL;
        assert!(data.encode_original().is_ok());
        data.originals
            .push(KagemushaOrdinaryInstalledContextFileV1 {
                path: format!("artifacts/{}", "b".repeat(64)),
                sha256: [10; 32],
                byte_len: 1,
            });
        assert!(data.encode_original().is_err());
    }
}
