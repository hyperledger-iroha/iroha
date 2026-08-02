//! Independent cross-conformance gate for the pinned Microsoft Vega-MC engine.
//!
//! The verifier key and proof were generated and self-verified by Microsoft's
//! stand-alone Python reference implementation at the pinned revision. No
//! Iroha code or Rust prover participated in producing either byte string.

use vega_prover::{
    provider::T256HyraxEngine,
    traits::{Engine as VegaEngine, circuit::VegaCircuit},
    vega_mc_zkp::{VegaMcVerifierKey, VegaMcZkSNARK},
};

use bellpepper_core::{ConstraintSystem, SynthesisError, num::AllocatedNum};

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

type Engine = T256HyraxEngine;

const PYTHON_VERIFIER_KEY: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../vendor/vega-prover/reference/fixtures/cubic/python_vk.bin"
));
const PYTHON_PROOF: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../vendor/vega-prover/reference/fixtures/cubic/python_standalone_proof.bin"
));
const TRANSCRIPT_VECTOR_SHA256: [u8; 32] =
    hex_literal::hex!("94967a280907fb3c5c61ff90ac593ff824d0029a1497dba819e701a4de507bc2");
const UPSTREAM_MANIFEST: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../vendor/vega-prover/UPSTREAM_MANIFEST.sha256"
));
const VENDOR_MANIFEST_SHA256: [u8; 32] =
    hex_literal::hex!("539c54251c8853fa99673e71d777966a3e3e238e64028d47b3e683329023236f");
const PATCHED_FILES: [(&str, [u8; 32], [u8; 32]); 5] = [
    (
        "src/lib.rs",
        hex_literal::hex!("a4f4c282f52d5d2f1d9799dc2bb17b43d07a7ea1ff51b4f4bf585630db65f3ff"),
        hex_literal::hex!("936bb79781b01f910826eeb708eb7fe01cc02be01c1d815e252cb50e62bdbcb8"),
    ),
    (
        "src/provider/pcs/hyrax_pc.rs",
        hex_literal::hex!("65fb6259969172c15863cd35824aa85d24e908fc38b036ab2eb4606fa71adf6a"),
        hex_literal::hex!("cee54fc22fcb7ec552b7a187a05aafaf1667b5a57e1ca1350c5e07a340d4990c"),
    ),
    (
        "src/provider/pcs/ipa.rs",
        hex_literal::hex!("a16f7d0560fe3363113a81768e57a0b06dbcf1996b5bb48cf3dc909d3aa49125"),
        hex_literal::hex!("61a92fa6f6f833ee10c6ed57da6cabdceb05abe8db9a6985f65bb7afc47ba77e"),
    ),
    (
        "src/r1cs/mod.rs",
        hex_literal::hex!("c152a02ea03e0bd506ce0f60023c291ffaeda4b251998be499d47d999a702aae"),
        hex_literal::hex!("891cc378df2a338da263a60cfed0a5ec15985dc4ffff0c78cc5cc252f92a62fc"),
    ),
    (
        "src/vega_mc_zkp.rs",
        hex_literal::hex!("05f2aba7947851447dade720e8501e1c9336ae5d0a29810ecebe6da9e245253c"),
        hex_literal::hex!("b5f69e7a4c956efc5359f54408530824be0e7232d4eb39ba06eae4f5f4788d75"),
    ),
];
const IROHA_ADDITIONS: [&str; 7] = [
    "IROHA_PATCHES.md",
    "IROHA_PROVENANCE.md",
    "UPSTREAM_MANIFEST.sha256",
    "reference/fixtures/cubic/.gitignore",
    "reference/fixtures/cubic/python_standalone_proof.bin",
    "reference/fixtures/cubic/python_vk.bin",
    "src/iroha_rng.rs",
];

#[test]
fn pinned_native_verifier_accepts_independent_python_mc_proof() {
    assert_eq!(
        sha256(PYTHON_VERIFIER_KEY),
        hex_literal::hex!("fdb982961889d7fe5757bf12b12a3a8b9fb18f764c024ad179d5eb145dec5b2e")
    );
    assert_eq!(
        sha256(PYTHON_PROOF),
        hex_literal::hex!("59aa887109f509268e21614589198071f4a84beabb8ebb63bcd2ba23844fec8a")
    );

    let verifier_key = VegaMcVerifierKey::<Engine>::decode_iroha_canonical(
        PYTHON_VERIFIER_KEY,
        PYTHON_VERIFIER_KEY.len(),
    )
    .expect("pinned Python Vega-MC verifier key");
    let proof = VegaMcZkSNARK::<Engine>::decode_iroha_canonical(PYTHON_PROOF, PYTHON_PROOF.len())
        .expect("pinned Python Vega-MC proof");
    let (steps, core) = proof
        .verify(&verifier_key, 2)
        .expect("native verifier must accept the independent Python proof");

    assert_eq!(steps.len(), 2);
    assert!(steps.iter().all(|public| public.len() == 1));
    assert_eq!(core.len(), 1);
}

#[test]
fn canonical_codec_is_exact_and_enforces_its_allocation_limit() {
    let verifier_key = VegaMcVerifierKey::<Engine>::decode_iroha_canonical(
        PYTHON_VERIFIER_KEY,
        PYTHON_VERIFIER_KEY.len(),
    )
    .expect("pinned verifier key is canonical");
    let proof = VegaMcZkSNARK::<Engine>::decode_iroha_canonical(PYTHON_PROOF, PYTHON_PROOF.len())
        .expect("pinned proof is canonical");
    assert_eq!(
        proof.encode_iroha_canonical().expect("encode pinned proof"),
        PYTHON_PROOF
    );
    assert_eq!(
        bincode::serialize(&proof).expect("upstream test oracle encodes proof"),
        PYTHON_PROOF,
        "the vendored adapter must remain byte-identical to Microsoft's test-only codec oracle"
    );
    assert_eq!(verifier_key.proof_dimensions().num_steps, 2);

    assert!(
        VegaMcVerifierKey::<Engine>::decode_iroha_canonical(
            PYTHON_VERIFIER_KEY,
            PYTHON_VERIFIER_KEY.len() - 1,
        )
        .is_err()
    );
    assert!(
        VegaMcZkSNARK::<Engine>::decode_iroha_canonical(PYTHON_PROOF, PYTHON_PROOF.len() - 1,)
            .is_err()
    );

    let mut trailing_key = PYTHON_VERIFIER_KEY.to_vec();
    trailing_key.push(0);
    assert!(
        VegaMcVerifierKey::<Engine>::decode_iroha_canonical(&trailing_key, trailing_key.len(),)
            .is_err()
    );
    let mut trailing_proof = PYTHON_PROOF.to_vec();
    trailing_proof.push(0);
    assert!(
        VegaMcZkSNARK::<Engine>::decode_iroha_canonical(&trailing_proof, trailing_proof.len(),)
            .is_err()
    );
}

#[test]
fn independent_python_proof_rejects_wrong_vk_and_equation_corruption() {
    let verifier_key = VegaMcVerifierKey::<Engine>::decode_iroha_canonical(
        PYTHON_VERIFIER_KEY,
        PYTHON_VERIFIER_KEY.len(),
    )
    .expect("pinned Python Vega-MC verifier key");
    let proof = VegaMcZkSNARK::<Engine>::decode_iroha_canonical(PYTHON_PROOF, PYTHON_PROOF.len())
        .expect("pinned Python Vega-MC proof");

    let different = DifferentCubicCircuit;
    let (_, wrong_vk) = VegaMcZkSNARK::<Engine>::setup(&different, &different, 2)
        .expect("setup deliberately different verifier key");
    assert!(
        proof.verify(&wrong_vk, 2).is_err(),
        "proof must not verify under a different circuit key"
    );

    let mut corrupted_bytes = PYTHON_PROOF.to_vec();
    let final_scalar_low_byte = corrupted_bytes
        .len()
        .checked_sub(32)
        .expect("proof contains its final scalar");
    corrupted_bytes[final_scalar_low_byte] ^= 1;
    let corrupted =
        VegaMcZkSNARK::<Engine>::decode_iroha_canonical(&corrupted_bytes, corrupted_bytes.len())
            .expect("low-byte scalar mutation remains structurally decodable");
    assert!(
        corrupted.verify(&verifier_key, 2).is_err(),
        "canonical but equation-invalid proof must be rejected"
    );
}

#[test]
fn pinned_vendor_inventory_and_oracle_digests_have_not_drifted() {
    let vendor_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/vega-prover");
    let mut files = Vec::new();
    collect_files(&vendor_root, &vendor_root, &mut files);
    files.sort_by(|left, right| left.0.cmp(&right.0));

    let mut manifest = Vec::new();
    for (relative, absolute) in &files {
        let digest = sha256(&fs::read(absolute).expect("read pinned Vega vendor file"));
        manifest.extend_from_slice(hex::encode(digest).as_bytes());
        manifest.extend_from_slice(b"  ");
        manifest.extend_from_slice(relative.as_bytes());
        manifest.push(b'\n');
    }
    assert_eq!(sha256(&manifest), VENDOR_MANIFEST_SHA256);

    let transcript = fs::read(vendor_root.join("reference/fixtures/cubic/transcript_vector.json"))
        .expect("pinned transcript vector");
    assert_eq!(sha256(&transcript), TRANSCRIPT_VECTOR_SHA256);
    assert_eq!(
        sha256(&fs::read(vendor_root.join("reference/fixtures/cubic/python_vk.bin")).unwrap()),
        sha256(PYTHON_VERIFIER_KEY),
    );
    assert_eq!(
        sha256(
            &fs::read(vendor_root.join("reference/fixtures/cubic/python_standalone_proof.bin"),)
                .unwrap(),
        ),
        sha256(PYTHON_PROOF),
    );
}

#[test]
fn every_upstream_file_and_integration_patch_matches_its_pinned_digest() {
    let vendor_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/vega-prover");
    let upstream = parse_upstream_manifest();
    assert!(
        upstream.len() > 100,
        "manifest must cover the vendored source tree"
    );

    for (relative, pristine_digest, patched_digest) in PATCHED_FILES {
        assert_eq!(
            upstream.get(relative),
            Some(&pristine_digest),
            "patched file must retain its pristine upstream digest"
        );
        assert_eq!(
            sha256(&fs::read(vendor_root.join(relative)).expect("read patched Vega source")),
            patched_digest,
            "undeclared change to patched Vega source {relative}"
        );
    }

    for (relative, pristine_digest) in &upstream {
        if PATCHED_FILES
            .iter()
            .any(|(patched, _, _)| patched == relative)
        {
            continue;
        }
        assert_eq!(
            sha256(&fs::read(vendor_root.join(relative)).expect("read pristine Vega source")),
            *pristine_digest,
            "vendored upstream file drifted: {relative}"
        );
    }

    let mut files = Vec::new();
    collect_files_including_provenance(&vendor_root, &vendor_root, &mut files);
    for (relative, _) in files {
        assert!(
            upstream.contains_key(&relative) || IROHA_ADDITIONS.contains(&relative.as_str()),
            "undeclared file in the Vega vendor tree: {relative}"
        );
    }
}

fn parse_upstream_manifest() -> BTreeMap<String, [u8; 32]> {
    UPSTREAM_MANIFEST
        .lines()
        .map(|line| {
            let (digest, relative) = line
                .split_once("  ")
                .expect("well-formed Vega upstream manifest line");
            let mut decoded = [0_u8; 32];
            hex::decode_to_slice(digest, &mut decoded)
                .expect("hex digest in Vega upstream manifest");
            (relative.to_owned(), decoded)
        })
        .collect()
}

fn collect_files(root: &Path, directory: &Path, output: &mut Vec<(String, PathBuf)>) {
    for entry in fs::read_dir(directory).expect("read pinned Vega vendor directory") {
        let entry = entry.expect("read pinned Vega vendor entry");
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, output);
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .expect("Vega vendor file is below root")
            .to_str()
            .expect("Vega vendor paths are UTF-8")
            .replace('\\', "/");
        if relative != "IROHA_PROVENANCE.md" {
            output.push((relative, path));
        }
    }
}

fn collect_files_including_provenance(
    root: &Path,
    directory: &Path,
    output: &mut Vec<(String, PathBuf)>,
) {
    for entry in fs::read_dir(directory).expect("read pinned Vega vendor directory") {
        let entry = entry.expect("read pinned Vega vendor entry");
        let path = entry.path();
        if path.is_dir() {
            collect_files_including_provenance(root, &path, output);
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .expect("Vega vendor file is below root")
            .to_str()
            .expect("Vega vendor paths are UTF-8")
            .replace('\\', "/");
        output.push((relative, path));
    }
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    use sha2::{Digest as _, Sha256};

    Sha256::digest(bytes).into()
}

/// Same public statement as the upstream cubic oracle, but with one extra
/// regular constraint so setup produces a genuinely different verifier key.
#[derive(Clone, Copy, Debug)]
struct DifferentCubicCircuit;

impl VegaCircuit<Engine> for DifferentCubicCircuit {
    fn public_values(&self) -> Result<Vec<<Engine as VegaEngine>::Scalar>, SynthesisError> {
        Ok(vec![<Engine as VegaEngine>::Scalar::from(15_u64)])
    }

    fn shared<CS: ConstraintSystem<<Engine as VegaEngine>::Scalar>>(
        &self,
        _: &mut CS,
    ) -> Result<Vec<AllocatedNum<<Engine as VegaEngine>::Scalar>>, SynthesisError> {
        Ok(Vec::new())
    }

    fn precommitted<CS: ConstraintSystem<<Engine as VegaEngine>::Scalar>>(
        &self,
        _: &mut CS,
        _: &[AllocatedNum<<Engine as VegaEngine>::Scalar>],
    ) -> Result<Vec<AllocatedNum<<Engine as VegaEngine>::Scalar>>, SynthesisError> {
        Ok(Vec::new())
    }

    fn num_challenges(&self) -> usize {
        0
    }

    fn synthesize<CS: ConstraintSystem<<Engine as VegaEngine>::Scalar>>(
        &self,
        cs: &mut CS,
        _: &[AllocatedNum<<Engine as VegaEngine>::Scalar>],
        _: &[AllocatedNum<<Engine as VegaEngine>::Scalar>],
        _: Option<&[<Engine as VegaEngine>::Scalar]>,
    ) -> Result<(), SynthesisError> {
        let scalar = |value| <Engine as VegaEngine>::Scalar::from(value);
        let x = AllocatedNum::alloc(cs.namespace(|| "x"), || Ok(scalar(2)))?;
        let x_squared = x.square(cs.namespace(|| "x squared"))?;
        let x_cubed = x_squared.mul(cs.namespace(|| "x cubed"), &x)?;
        let y = AllocatedNum::alloc(cs.namespace(|| "y"), || Ok(scalar(15)))?;
        cs.enforce(
            || "y equals x cubed plus x plus five",
            |lc| lc + x_cubed.get_variable() + x.get_variable() + (scalar(5), CS::one()),
            |lc| lc + CS::one(),
            |lc| lc + y.get_variable(),
        );
        // This redundant row deliberately changes the governed R1CS shape.
        cs.enforce(
            || "different verifier-key row",
            |lc| lc + x.get_variable(),
            |lc| lc + CS::one(),
            |lc| lc + x.get_variable(),
        );
        y.inputize(cs.namespace(|| "output"))?;
        Ok(())
    }
}
