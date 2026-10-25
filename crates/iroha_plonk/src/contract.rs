//! Mathematical ABI of the finite built-in proof registry.
//!
//! This contract contains protocol data and explicit semantic revisions, never
//! source paths, timestamps, compiler versions or a workspace file census.
//! An encoding, admission, transcript or verifier semantic change must revise
//! its component below and regenerate the registry through strict source intake.
//! Implementation changes preserving these semantics need no new contract.
//! Build provenance remains a separate packaging concern.

use std::sync::OnceLock;

use iroha_pasta::{Fp, Fq, params::PARAMS_DOMAIN, poseidon};
use norito::NoritoSchema;
use sha2::{Digest as _, Sha256};

use crate::{
    cs::{
        CircuitDescriptorV2, CurveV1, InstanceType,
        descriptor::{PINNED_PARAMS_V1, PROTOCOL_VERSION, pinned_params_digest},
    },
    keys::{VK_VERSION, pk::artifact},
    transcript::INSTANCE_FRAME_TAG,
};

/// Canonical recursive proof transcript domain, also used by the circuit verifier.
pub const PIPA_R_PROOF_DOMAIN: [u8; 8] = *b"pipa-rb1";
/// Canonical accumulation transcript domain, also used by the circuit verifier.
pub const PIPA_AS_FOLD_DOMAIN: [u8; 8] = *b"pipa-as1";
/// Descriptor V2 digest personalization, shared with its encoder.
pub(crate) const DESCRIPTOR_DIGEST_DOMAIN: &[u8; 16] = b"PIPA-v2-CircDesc";
/// Descriptor-bound V2 verifying-key transcript personalization.
pub(crate) const VK_TRANSCRIPT_DOMAIN: &[u8; 16] = b"Iroha-PlonkVK-v2";
/// Native verifying-key word hash domain.
pub(crate) const VK_WORD_DOMAIN: [u8; 8] = *b"kgwvkey1";
/// Exact public-source table fingerprint framing domain.
pub(crate) const SOURCE_FINGERPRINT_DOMAIN: &[u8] = b"iroha:pipa:source-lookup:v1\0";

// Revisions are semantic obligations, not implementation release counters.
// Descriptor: canonical Norito V2, exact frame, bounded dimensions/selector plan.
// VK: version 0x02, canonical nonidentity points, packed selectors, exact shape.
// Compact PK: descriptor-bound header, lengths, canonical scalars, no trailing data.
// Source: descriptor/copy/selector/fixed/permutation order and canonical field bytes.
// Admission: exact whole-original identities plus role, params, shape and bounds;
// proving still reconstructs and verifies the native source fingerprint.
// Verifier: exact proof/instance shape, all argument terms and opening rounds,
// complete-formula MSM, mandatory final decision, no soft verdict acceptance.
// Transcript: injective point/scalar absorption, instance framing, total maps
// x=5*nz(r)^2, multiopen=25*nz(r)^4, and zero-to-one IPA challenges.
// Other supported V2 transcript profiles retain their Challenge255 BLAKE2b
// schedule or scalar-field RP57 absorption/point encoding and challenge schedule.
const SEMANTIC_COMPONENTS: &[(&[u8], u32)] = &[
    (b"descriptor-v2-canonical-norito", 1),
    (b"verifying-key-v2-canonical-points-selectors", 1),
    (b"compact-proving-key-v2-exact-framing", 1),
    (b"source-fingerprint-v2-public-tables", 1),
    (b"builtin-admission-exact-originals-and-source", 1),
    (b"pipa-verifier-complete-decision", 1),
    (b"pipa-r-transcript-injective-total-challenges", 1),
    (b"blake2b-challenge255-transcript", 1),
    (b"scalar-poseidon-rp57-transcript", 1),
];

pub(crate) const BUILTIN_TARGET_SUPPORTED: bool = cfg!(all(
    target_endian = "little",
    any(
        all(
            target_arch = "aarch64",
            target_pointer_width = "64",
            any(
                target_os = "android",
                target_os = "ios",
                target_os = "macos",
                target_os = "linux"
            )
        ),
        all(
            target_arch = "x86_64",
            target_pointer_width = "64",
            any(
                target_os = "android",
                target_os = "ios",
                target_os = "macos",
                target_os = "linux",
                target_os = "windows"
            )
        ),
        all(
            target_arch = "arm",
            target_os = "android",
            target_pointer_width = "32"
        )
    )
));

/// A native mathematical transcript cannot be represented from its fixed inputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContractErrorV1 {
    /// A frame exceeds its canonical 32-bit length.
    FrameLength,
    /// A required native parameter identity is absent.
    MissingParams,
}

impl core::fmt::Display for ContractErrorV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::FrameLength => "native protocol contract frame exceeds u32",
            Self::MissingParams => "native protocol contract parameter identity absent",
        })
    }
}
impl std::error::Error for ContractErrorV1 {}

fn put_u32(bytes: &mut Vec<u8>, value: usize) -> Result<(), ContractErrorV1> {
    bytes.extend_from_slice(
        &u32::try_from(value)
            .map_err(|_| ContractErrorV1::FrameLength)?
            .to_le_bytes(),
    );
    Ok(())
}

fn put_frame(bytes: &mut Vec<u8>, frame: &[u8]) -> Result<(), ContractErrorV1> {
    put_u32(bytes, frame.len())?;
    bytes.extend_from_slice(frame);
    Ok(())
}

/// Native PIPA-R mathematical transcript beginning with the protocol version.
///
/// Owners prepend their own version and supply their actual recursion domains.
/// The returned data grants no admission authority. Both curve preimages retain
/// the existing native wallet protocol bytes exactly.
///
/// # Errors
/// A fixed frame length is unrepresentable or a required parameter pin is absent.
pub fn pipa_r_protocol_transcript_v1(
    curve: CurveV1,
    proof_domain: [u8; 8],
    fold_domain: [u8; 8],
) -> Result<Vec<u8>, ContractErrorV1> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&PROTOCOL_VERSION.to_le_bytes());
    bytes.push(curve_code(curve));
    bytes.extend_from_slice(&curve.base_modulus());
    bytes.extend_from_slice(&curve.scalar_modulus());
    put_frame(&mut bytes, CircuitDescriptorV2::frame_name().as_bytes())?;
    bytes.extend_from_slice(DESCRIPTOR_DIGEST_DOMAIN);
    bytes.push(VK_VERSION);
    bytes.extend_from_slice(VK_TRANSCRIPT_DOMAIN);
    bytes.extend_from_slice(&VK_WORD_DOMAIN);
    bytes.extend_from_slice(&proof_domain);
    bytes.extend_from_slice(&fold_domain);
    bytes.extend_from_slice(&INSTANCE_FRAME_TAG);
    for value in [
        poseidon::WIDTH,
        poseidon::RATE,
        poseidon::FULL_ROUNDS,
        poseidon::PARTIAL_ROUNDS,
        poseidon::SECURE_MDS,
    ] {
        put_u32(&mut bytes, value)?;
    }
    let table = match curve {
        CurveV1::Pallas => <Fp as poseidon::PoseidonField>::rp57().to_table(),
        CurveV1::Vesta => <Fq as poseidon::PoseidonField>::rp57().to_table(),
    };
    put_u32(&mut bytes, table.len())?;
    bytes.extend_from_slice(&Sha256::digest(&table));
    put_frame(&mut bytes, PARAMS_DOMAIN.as_bytes())?;
    put_u32(&mut bytes, 5)?;
    for k in 12_u8..=16 {
        bytes.push(k);
        bytes.extend_from_slice(
            &pinned_params_digest(curve, u32::from(k)).ok_or(ContractErrorV1::MissingParams)?,
        );
    }
    bytes.extend_from_slice(match curve {
        CurveV1::Pallas => &[2, 0],
        CurveV1::Vesta => &[1, 1],
    });
    Ok(bytes)
}

const fn curve_code(curve: CurveV1) -> u8 {
    match curve {
        CurveV1::Pallas => 0,
        CurveV1::Vesta => 1,
    }
}

fn contract_preimage(components: &[(&[u8], u32)]) -> Result<Vec<u8>, ContractErrorV1> {
    let mut bytes = b"iroha:plonk:builtin-mathematical-abi:v1\0".to_vec();
    put_u32(&mut bytes, components.len())?;
    for (name, revision) in components {
        put_frame(&mut bytes, name)?;
        bytes.extend_from_slice(&revision.to_le_bytes());
    }
    for curve in [CurveV1::Pallas, CurveV1::Vesta] {
        put_frame(
            &mut bytes,
            &pipa_r_protocol_transcript_v1(curve, PIPA_R_PROOF_DOMAIN, PIPA_AS_FOLD_DOMAIN)?,
        )?;
    }
    bytes.extend_from_slice(&artifact::MAGIC);
    bytes.extend_from_slice(&[norito::core::VERSION_MAJOR, norito::core::VERSION_MINOR]);
    bytes.extend_from_slice(&norito::schema::identity::frame_hash::<CircuitDescriptorV2>());
    put_frame(&mut bytes, SOURCE_FINGERPRINT_DOMAIN)?;
    bytes.extend_from_slice(&iroha_pasta::params::MAX_K.to_le_bytes());
    put_u32(&mut bytes, PINNED_PARAMS_V1.len())?;
    for (curve, k, digest) in PINNED_PARAMS_V1 {
        bytes.extend_from_slice(&[curve_code(*curve), *k]);
        bytes.extend_from_slice(digest);
    }
    for kind in [InstanceType::Field, InstanceType::Bounded]
        .into_iter()
        .chain((0..=253).map(InstanceType::Bits))
    {
        bytes.extend_from_slice(&kind.code().to_le_bytes());
    }
    Ok(bytes)
}

/// Stable mathematical ABI digest for strict-generated built-in profiles.
/// Actual Poseidon tables, moduli, pinned parameters and canonical encodings are
/// committed together with explicit semantic revisions. Computed once per process.
#[must_use]
pub fn builtin_contract_v1() -> [u8; 32] {
    static CONTRACT: OnceLock<[u8; 32]> = OnceLock::new();
    *CONTRACT.get_or_init(|| {
        let bytes = contract_preimage(SEMANTIC_COMPONENTS)
            .expect("fixed native mathematical contract inputs are complete and bounded");
        Sha256::digest(bytes).into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_contract_commits_each_semantic_component() {
        let expected = builtin_contract_v1();
        assert_eq!(expected, builtin_contract_v1());
        println!("builtin_contract_v1 = {expected:?}");
        for index in 0..SEMANTIC_COMPONENTS.len() {
            let mut changed = SEMANTIC_COMPONENTS.to_vec();
            changed[index].1 += 1;
            let actual: [u8; 32] = Sha256::digest(contract_preimage(&changed).unwrap()).into();
            assert_ne!(expected, actual, "semantic component {index}");
        }
    }

    #[test]
    fn native_contract_contains_actual_tables_and_parameter_pins_on_both_curves() {
        let preimage = contract_preimage(SEMANTIC_COMPONENTS).unwrap();
        for curve in [CurveV1::Pallas, CurveV1::Vesta] {
            let transcript =
                pipa_r_protocol_transcript_v1(curve, PIPA_R_PROOF_DOMAIN, PIPA_AS_FOLD_DOMAIN)
                    .unwrap();
            assert_eq!(transcript.len(), 421);
            assert!(
                preimage
                    .windows(transcript.len())
                    .any(|part| part == transcript)
            );
            let table = match curve {
                CurveV1::Pallas => <Fp as poseidon::PoseidonField>::rp57().to_table(),
                CurveV1::Vesta => <Fq as poseidon::PoseidonField>::rp57().to_table(),
            };
            let digest: [u8; 32] = Sha256::digest(table).into();
            assert!(transcript.windows(32).any(|part| part == digest));
            let mut changed_domain = PIPA_R_PROOF_DOMAIN;
            changed_domain[0] ^= 1;
            assert_ne!(
                transcript,
                pipa_r_protocol_transcript_v1(curve, changed_domain, PIPA_AS_FOLD_DOMAIN).unwrap()
            );
        }
        for (_, _, digest) in PINNED_PARAMS_V1 {
            assert!(preimage.windows(32).any(|part| part == digest));
        }
    }
}
