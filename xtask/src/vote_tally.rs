use std::{convert::TryInto, error::Error, fs, path::Path};

use blake2::{
    Blake2bVar,
    digest::{Update as _, VariableOutput as _},
};
use norito::json::{self, Value};

/// Canonical file names within the vote tally bundle directory.
pub fn bundle_file_names() -> &'static [&'static str] {
    &[
        "vote_tally_meta.json",
        "vote_tally_proof.zk1",
        "vote_tally_vk.zk1",
    ]
}

/// Human-readable summary derived from the vote tally bundle artifacts.
#[derive(Debug)]
pub struct BundleSummary {
    pub backend: String,
    pub circuit_id: String,
    pub commit_hex: String,
    pub root_hex: String,
    pub schema_hash_hex: String,
    pub vk_commit_hex: String,
    pub vk_len: usize,
    pub proof_len: usize,
}

impl std::fmt::Display for BundleSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "vote tally summary:")?;
        writeln!(f, "  backend: {}", self.backend)?;
        writeln!(f, "  circuit_id: {}", self.circuit_id)?;
        writeln!(f, "  commit: {}", self.commit_hex)?;
        writeln!(f, "  root: {}", self.root_hex)?;
        writeln!(f, "  schema_hash: {}", self.schema_hash_hex)?;
        writeln!(f, "  vk_commitment: {}", self.vk_commit_hex)?;
        writeln!(f, "  vk_len: {}", self.vk_len)?;
        writeln!(f, "  proof_len: {}", self.proof_len)
    }
}

/// Generate the Halo2 vote tally bundle and write it to `out_dir`.
#[cfg(feature = "vote-tally")]
pub fn write_bundle(out_dir: &Path) -> Result<BundleSummary, Box<dyn Error>> {
    vote_tally_backend::write_bundle(out_dir)
}

#[cfg(not(feature = "vote-tally"))]
pub fn write_bundle(_out_dir: &Path) -> Result<BundleSummary, Box<dyn Error>> {
    Err("xtask compiled without the `vote-tally` feature; re-run with `--features vote-tally` to generate bundles".into())
}

/// Read an on-disk vote tally bundle summary back from `dir`.
pub fn read_summary(dir: &Path) -> Result<BundleSummary, Box<dyn Error>> {
    let meta_path = dir.join("vote_tally_meta.json");
    let meta_text = fs::read_to_string(&meta_path)?;
    let meta: Value = json::from_str(&meta_text)?;

    let backend = meta["backend"].as_str().unwrap_or_default().to_string();
    let circuit_id = meta["circuit_id"].as_str().unwrap_or_default().to_string();
    let commit_hex = meta["commit_hex"].as_str().unwrap_or_default().to_string();
    let root_hex = meta["root_hex"].as_str().unwrap_or_default().to_string();
    let schema_hash_hex = meta["public_inputs_schema_hash_hex"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let vk_commit_hex = meta["vk_commitment_hex"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let vk_len = fs::metadata(dir.join("vote_tally_vk.zk1"))?.len() as usize;
    let proof_len = fs::metadata(dir.join("vote_tally_proof.zk1"))?.len() as usize;

    Ok(BundleSummary {
        backend,
        circuit_id,
        commit_hex,
        root_hex,
        schema_hash_hex,
        vk_commit_hex,
        vk_len,
        proof_len,
    })
}

/// Convert a summary into a JSON representation used for the `--summary-json` flag.
pub fn summary_to_json(summary: &BundleSummary) -> Value {
    let mut map = norito::json::Map::new();
    map.insert("backend".into(), Value::from(summary.backend.clone()));
    map.insert("circuit_id".into(), Value::from(summary.circuit_id.clone()));
    map.insert("commit_hex".into(), Value::from(summary.commit_hex.clone()));
    map.insert("root_hex".into(), Value::from(summary.root_hex.clone()));
    map.insert(
        "public_inputs_schema_hash_hex".into(),
        Value::from(summary.schema_hash_hex.clone()),
    );
    map.insert(
        "vk_commitment_hex".into(),
        Value::from(summary.vk_commit_hex.clone()),
    );
    map.insert("vk_len".into(), Value::from(summary.vk_len as u64));
    map.insert("proof_len".into(), Value::from(summary.proof_len as u64));
    Value::Object(map)
}

/// Build an attestation manifest describing the bundle summary and artifact hashes.
pub fn attestation_manifest(summary: &BundleSummary, dir: &Path) -> Result<Value, Box<dyn Error>> {
    let mut artifacts = Vec::new();
    for name in bundle_file_names() {
        let path = dir.join(name);
        let bytes = fs::read(&path)?;
        let digest = iroha_hash(&bytes);
        let mut entry = norito::json::Map::new();
        entry.insert("file".into(), Value::from(*name));
        entry.insert("len".into(), Value::from(bytes.len() as u64));
        entry.insert("blake2b_256".into(), Value::from(hex::encode(digest)));
        artifacts.push(Value::Object(entry));
    }
    let generated_ms = deterministic_timestamp(summary);
    let mut manifest = norito::json::Map::new();
    manifest.insert("generated_unix_ms".into(), Value::from(generated_ms));
    manifest.insert("hash_algorithm".into(), Value::from("blake2b-256"));
    manifest.insert("bundle".into(), summary_to_json(summary));
    manifest.insert("artifacts".into(), Value::Array(artifacts));
    Ok(Value::Object(manifest))
}

fn deterministic_timestamp(summary: &BundleSummary) -> u64 {
    let mut combined = summary.commit_hex.clone();
    combined.push('@');
    combined.push_str(&summary.vk_commit_hex);
    let hash = iroha_hash(combined.as_bytes());
    u64::from_be_bytes(hash[..8].try_into().expect("slice length"))
}

#[cfg(feature = "vote-tally")]
mod vote_tally_backend {
    use halo2_proofs as halo2_axiom;
    use halo2_proofs::{
        SerdeFormat,
        halo2curves::{
            ff::PrimeField as _,
            pasta::{EqAffine as Curve, Fp as Scalar},
        },
        plonk::{keygen_pk, keygen_vk},
        poly::{
            commitment::ParamsProver as _,
            ipa::{
                commitment::{IPACommitmentScheme, ParamsIPA},
                multiopen::ProverIPA,
            },
        },
        transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer},
    };
    use iroha_core::zk::depth::VoteBoolCommitMerkle;
    use rand_chacha::{ChaCha20Rng, rand_core::SeedableRng as _};
    use sha2::{Digest as ShaDigest, Sha256};

    use super::*;

    #[allow(dead_code)]
    fn _assert_scalar_hashable() {
        fn assert_hash<T: std::hash::Hash>() {}
        assert_hash::<Scalar>();
    }

    pub(super) fn write_bundle(out_dir: &Path) -> Result<BundleSummary, Box<dyn Error>> {
        fs::create_dir_all(out_dir)?;
        let bundle = generate_bundle()?;

        let vk_path = out_dir.join("vote_tally_vk.zk1");
        fs::write(&vk_path, &bundle.vk_bytes)?;

        let proof_path = out_dir.join("vote_tally_proof.zk1");
        fs::write(&proof_path, &bundle.proof_bytes)?;

        let summary = BundleSummary {
            backend: bundle.backend.into(),
            circuit_id: bundle.circuit_id.into(),
            commit_hex: hex::encode(bundle.commit.to_repr()),
            root_hex: hex::encode(bundle.root.to_repr()),
            schema_hash_hex: hex::encode(bundle.public_inputs_schema_hash),
            vk_commit_hex: hex::encode(bundle.vk_commitment),
            vk_len: bundle.vk_bytes.len(),
            proof_len: bundle.proof_bytes.len(),
        };

        let meta_path = out_dir.join("vote_tally_meta.json");
        let mut meta_text = json::to_string_pretty(&summary_to_json(&summary))?;
        meta_text.push('\n');
        fs::write(&meta_path, meta_text)?;

        Ok(summary)
    }

    struct VoteTallyBundle {
        backend: &'static str,
        circuit_id: &'static str,
        commit: Scalar,
        root: Scalar,
        vk_bytes: Vec<u8>,
        proof_bytes: Vec<u8>,
        vk_commitment: [u8; 32],
        public_inputs_schema_hash: [u8; 32],
    }

    fn generate_bundle() -> Result<VoteTallyBundle, Box<dyn Error>> {
        const BACKEND: &str = "halo2/pasta/ipa-v1/vote-bool-commit-merkle8-v1";
        const CIRCUIT_ID: &str = "halo2/pasta/vote-bool-commit-merkle8-v1";
        const K: u32 = 6;
        const RNG_SEED: [u8; 32] = *b"iroha_halo2_vote_tally_seed_____";

        let params = ParamsIPA::<Curve>::new(K);
        let circuit = VoteBoolCommitMerkle::<8>;
        let vk_h2 = keygen_vk(&params, &circuit)?;
        let pk = keygen_pk(&params, vk_h2.clone(), &circuit)?;

        let commit = compute_commit();
        let root = compute_root(commit);

        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        let instance_columns = vec![vec![commit], vec![root]];
        let instances = instance_columns
            .iter()
            .map(|column| column.as_slice())
            .collect::<Vec<_>>();
        halo2_axiom::plonk::create_proof::<
            IPACommitmentScheme<Curve>,
            ProverIPA<'_, Curve>,
            Challenge255<Curve>,
            ChaCha20Rng,
            Blake2bWrite<Vec<u8>, Curve, Challenge255<Curve>>,
            VoteBoolCommitMerkle<8>,
        >(
            &params,
            &pk,
            &[circuit],
            &[&instances],
            ChaCha20Rng::from_seed(RNG_SEED),
            &mut transcript,
        )?;
        let proof_raw = transcript.finalize();

        let mut vk_bytes = wrap_start();
        wrap_append_ipa_k(&mut vk_bytes, K);
        wrap_append_vk_pasta(&mut vk_bytes, &vk_h2);

        let mut proof_bytes = wrap_start();
        wrap_append_proof(&mut proof_bytes, &proof_raw);
        wrap_append_instances_pasta_fp_cols(&instances, &mut proof_bytes);

        let mut public_inputs = Vec::with_capacity(64);
        public_inputs.extend_from_slice(commit.to_repr().as_ref());
        public_inputs.extend_from_slice(root.to_repr().as_ref());
        let schema_hash = iroha_hash(&public_inputs);

        let vk_commitment = hash_vk_bytes(BACKEND, &vk_bytes);

        Ok(VoteTallyBundle {
            backend: BACKEND,
            circuit_id: CIRCUIT_ID,
            commit,
            root,
            vk_bytes,
            proof_bytes,
            vk_commitment,
            public_inputs_schema_hash: schema_hash,
        })
    }

    fn compute_commit() -> Scalar {
        let vote = Scalar::one();
        let rho = Scalar::from(12345u64);
        compress(vote, rho)
    }

    fn compute_root(mut acc: Scalar) -> Scalar {
        for i in 0..8u64 {
            acc = compress(acc, Scalar::from(20 + i));
        }
        acc
    }

    fn compress(left: Scalar, right: Scalar) -> Scalar {
        let rc0 = Scalar::from(7u64);
        let rc1 = Scalar::from(13u64);
        let two = Scalar::from(2u64);
        let three = Scalar::from(3u64);

        let a = left + rc0;
        let b = right + rc1;

        let a2 = a * a;
        let a4 = a2 * a2;
        let a5 = a4 * a;
        let b2 = b * b;
        let b4 = b2 * b2;
        let b5 = b4 * b;

        two * a5 + three * b5
    }

    fn wrap_start() -> Vec<u8> {
        b"ZK1\0".to_vec()
    }

    fn wrap_append_proof(buf: &mut Vec<u8>, transcript_bytes: &[u8]) {
        write_tlv(buf, *b"PROF", transcript_bytes);
    }

    fn wrap_append_ipa_k(buf: &mut Vec<u8>, k: u32) {
        let bytes = k.to_le_bytes();
        write_tlv(buf, *b"IPAK", &bytes);
    }

    fn wrap_append_vk_pasta(buf: &mut Vec<u8>, vk: &halo2_proofs::plonk::VerifyingKey<Curve>) {
        let bytes = vk.to_bytes(SerdeFormat::Processed);
        write_tlv(buf, *b"H2VK", &bytes);
    }

    fn wrap_append_instances_pasta_fp_cols(columns: &[&[Scalar]], buf: &mut Vec<u8>) {
        if columns.is_empty() {
            return;
        }
        let cols: u32 = columns.len() as u32;
        let rows: u32 = columns[0].len() as u32;
        if columns.iter().any(|c| c.len() as u32 != rows) {
            return;
        }

        let mut payload = Vec::with_capacity(8 + (rows as usize) * (cols as usize) * 32);
        payload.extend_from_slice(&cols.to_le_bytes());
        payload.extend_from_slice(&rows.to_le_bytes());
        for row in 0..rows as usize {
            for column in columns.iter() {
                payload.extend_from_slice(column[row].to_repr().as_ref());
            }
        }
        write_tlv(buf, *b"I10P", &payload);
    }

    fn write_tlv(buf: &mut Vec<u8>, tag: [u8; 4], payload: &[u8]) {
        buf.extend_from_slice(&tag);
        buf.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        buf.extend_from_slice(payload);
    }

    fn hash_vk_bytes(backend: &str, bytes: &[u8]) -> [u8; 32] {
        let mut h = Sha256::new();
        ShaDigest::update(&mut h, backend.as_bytes());
        ShaDigest::update(&mut h, bytes);
        h.finalize().into()
    }
}

pub fn iroha_hash(bytes: &[u8]) -> [u8; 32] {
    let vec_hash = Blake2bVar::new(32)
        .expect("failed to construct blake2b-256 hasher")
        .chain(bytes)
        .finalize_boxed();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&vec_hash);
    hash[31] |= 1;
    hash
}
