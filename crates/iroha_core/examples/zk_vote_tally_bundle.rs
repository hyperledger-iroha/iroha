//! Developer helper for regenerating the vote tally Halo2 bundle artifacts.
#![allow(unexpected_cfgs)]

#[cfg(not(all(feature = "dev-tests", feature = "halo2-dev-tests")))]
fn main() {
    eprintln!("Enable both `dev-tests` and `halo2-dev-tests` features to build this example.");
}

#[cfg(all(feature = "dev-tests", feature = "halo2-dev-tests"))]
fn main() -> anyhow::Result<()> {
    vote_tally_bundle::run()
}

#[cfg(all(feature = "dev-tests", feature = "halo2-dev-tests"))]
mod vote_tally_bundle {
    use std::{env, fs, path::PathBuf};

    use anyhow::{Context, Result};
    use halo2_proofs::{
        halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
        plonk::{keygen_pk, keygen_vk},
        poly::{
            commitment::CommitmentScheme,
            ipa::{commitment::ParamsIPA, multiopen::ProverIPA},
        },
        transcript::{Blake2bWrite, Challenge255},
    };
    use iroha_core::zk::{self, hash_vk, zk1_test_helpers};
    use iroha_crypto::Hash as CryptoHash;
    use iroha_data_model::proof::VerifyingKeyBox;
    use norito::json::{self, Value};
    use rand::rngs::OsRng;

    pub fn run() -> Result<()> {
        let out_dir = env::args()
            .nth(1)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("artifacts/zk_vote_tally"));
        fs::create_dir_all(&out_dir)
            .with_context(|| format!("failed to create {}", out_dir.display()))?;

        let bundle = generate_bundle()?;

        let vk_path = out_dir.join("vote_tally_vk.zk1");
        fs::write(&vk_path, &bundle.vk_bytes)
            .with_context(|| format!("failed to write {}", vk_path.display()))?;

        let proof_path = out_dir.join("vote_tally_proof.zk1");
        fs::write(&proof_path, &bundle.proof_bytes)
            .with_context(|| format!("failed to write {}", proof_path.display()))?;

        let meta = json::json!({
            "generated_unix_ms": chrono::Utc::now().timestamp_millis(),
            "bundle": vote_tally::summary_to_json(&bundle.summary),
        });
        let meta_path = out_dir.join("vote_tally_meta.json");
        fs::write(&meta_path, json::to_vec_pretty(&meta)?)
            .with_context(|| format!("failed to write {}", meta_path.display()))?;

        let summary_json = vote_tally::summary_to_json(&bundle.summary);
        println!("wrote vote tally bundle artifacts to {}", out_dir.display());
        println!(
            "bundle summary:\n{}",
            json::to_string_pretty(&summary_json)?
        );
        Ok(())
    }

    struct VoteTallyArtifacts {
        summary: vote_tally::BundleSummary,
        vk_bytes: Vec<u8>,
        proof_bytes: Vec<u8>,
    }

    fn generate_bundle() -> Result<VoteTallyArtifacts> {
        const BACKEND: &str = "halo2/pasta/ipa/vote-bool-commit-merkle8";
        const CIRCUIT_ID: &str = "halo2/pasta/vote-bool-commit-merkle8";
        const K: u32 = 6;

        let params = ParamsIPA::<Curve>::new(K);
        let circuit = zk::depth::VoteBoolCommitMerkle::<8>;
        let vk = keygen_vk(&params, &circuit)?;
        let pk = keygen_pk(&params, vk.clone(), &circuit)?;
        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        let instances = zk1_test_helpers::vote_bundle_instances();
        halo2_axiom::plonk::create_proof::<_, _, _, _>(
            &params,
            &pk,
            &[circuit],
            &[&instances
                .iter()
                .map(|column| &column[..])
                .collect::<Vec<_>>()],
            OsRng,
            &mut transcript,
        )?;
        let proof_bytes = transcript.finalize();

        let commit = instances[0][0];
        let root = instances[1][0];

        let vk_bytes = zk::write_vk_bytes(&VerifyingKeyBox::try_from(&vk)?)?;
        let vk_commitment = hash_vk(&vk_bytes);
        let summary = vote_tally::BundleSummary {
            backend: BACKEND.to_owned(),
            circuit_id: CIRCUIT_ID.to_owned(),
            commit_hex: hex::encode(commit.to_repr()),
            root_hex: hex::encode(root.to_repr()),
            schema_hash_hex: hex::encode(zk1_test_helpers::vote_bundle_schema_hash()),
            vk_commit_hex: hex::encode(vk_commitment),
            vk_len: vk_bytes.len(),
            proof_len: proof_bytes.len(),
        };

        Ok(VoteTallyArtifacts {
            summary,
            vk_bytes,
            proof_bytes,
        })
    }

    mod vote_tally {
        use super::*;

        #[derive(Debug, Clone)]
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

        pub fn summary_to_json(summary: &BundleSummary) -> Value {
            json::json!({
                "backend": summary.backend,
                "circuit_id": summary.circuit_id,
                "commit_hex": summary.commit_hex,
                "root_hex": summary.root_hex,
                "schema_hash_hex": summary.schema_hash_hex,
                "vk_commit_hex": summary.vk_commit_hex,
                "vk_len": summary.vk_len,
                "proof_len": summary.proof_len,
            })
        }

        #[cfg(test)]
        mod tests {
            use super::*;

            #[test]
            fn summary_to_json_emits_expected_structure() {
                let backend = "backend-x".to_owned();
                let circuit_id = "circuit-y".to_owned();
                let commit_hex = "deadbeef".to_owned();
                let root_hex = "aabbccdd".to_owned();
                let schema_hash_hex = "11223344".to_owned();
                let vk_commit_hex = "55667788".to_owned();
                let vk_len = 1024usize;
                let proof_len = 2048usize;

                let summary = BundleSummary {
                    backend: backend.clone(),
                    circuit_id: circuit_id.clone(),
                    commit_hex: commit_hex.clone(),
                    root_hex: root_hex.clone(),
                    schema_hash_hex: schema_hash_hex.clone(),
                    vk_commit_hex: vk_commit_hex.clone(),
                    vk_len,
                    proof_len,
                };

                let value = summary_to_json(&summary);
                let object = value
                    .as_object()
                    .expect("vote tally summary should serialize as JSON object");

                assert_eq!(
                    object.get("backend").and_then(Value::as_str),
                    Some(backend.as_str())
                );
                assert_eq!(
                    object.get("circuit_id").and_then(Value::as_str),
                    Some(circuit_id.as_str())
                );
                assert_eq!(
                    object.get("commit_hex").and_then(Value::as_str),
                    Some(commit_hex.as_str())
                );
                assert_eq!(
                    object.get("root_hex").and_then(Value::as_str),
                    Some(root_hex.as_str())
                );
                assert_eq!(
                    object.get("schema_hash_hex").and_then(Value::as_str),
                    Some(schema_hash_hex.as_str())
                );
                assert_eq!(
                    object.get("vk_commit_hex").and_then(Value::as_str),
                    Some(vk_commit_hex.as_str())
                );
                assert_eq!(
                    object.get("vk_len").and_then(Value::as_u64),
                    Some(vk_len as u64)
                );
                assert_eq!(
                    object.get("proof_len").and_then(Value::as_u64),
                    Some(proof_len as u64)
                );
                assert_eq!(object.len(), 8, "unexpected additional vote summary fields");
            }
        }
    }
}
