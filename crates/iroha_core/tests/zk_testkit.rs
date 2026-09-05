#![doc = "Helpers for generating minimal Halo2 proofs for governance tests."]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(dead_code, unused_imports)]
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
mod halo2_bundle {
    use base64::Engine as _;
    use halo2_proofs::{
        halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
        poly::{
            VerificationStrategy as _,
            commitment::{CommitmentScheme, ParamsProver as _},
            ipa::{
                commitment::{IPACommitmentScheme, ParamsIPA},
                multiopen::ProverIPA,
            },
        },
        transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer as _},
    };
    use iroha_core::zk;
    use iroha_crypto::Hash as CryptoHash;
    use iroha_data_model::{
        confidential::ConfidentialStatus,
        proof::{ProofBox, VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
        zk::{BackendTag, OpenVerifyEnvelope},
    };
    use rand_core_06::OsRng;
    mod zk1 {
        use halo2_proofs::{
            SerdeFormat,
            halo2curves::pasta::{EqAffine as Curve, Fp},
            plonk::VerifyingKey,
        };
        const MAGIC: &[u8; 4] = b"ZK1\0";
        fn write_tlv(buf: &mut Vec<u8>, tag: [u8; 4], payload: &[u8]) {
            buf.extend_from_slice(&tag);
            let len = u32::try_from(payload.len()).expect("ZK1 TLV payload length should fit u32");
            buf.extend_from_slice(&len.to_le_bytes());
            buf.extend_from_slice(payload);
        }
        pub fn wrap_start() -> Vec<u8> {
            MAGIC.to_vec()
        }
        pub fn wrap_append_proof(buf: &mut Vec<u8>, transcript_bytes: &[u8]) {
            write_tlv(buf, *b"PROF", transcript_bytes);
        }
        pub fn wrap_append_ipa_k(buf: &mut Vec<u8>, k: u32) {
            let mut payload = Vec::with_capacity(4);
            payload.extend_from_slice(&k.to_le_bytes());
            write_tlv(buf, *b"IPAK", &payload);
        }
        pub fn wrap_append_vk_pasta(buf: &mut Vec<u8>, vk: &VerifyingKey<Curve>) {
            let bytes = vk.to_bytes(SerdeFormat::Processed);
            write_tlv(buf, *b"H2VK", &bytes);
        }
        pub fn wrap_append_instances_pasta_fp_cols(columns: &[&[Fp]], buf: &mut Vec<u8>) {
            use halo2_proofs::halo2curves::ff::PrimeField as _;
            if columns.is_empty() {
                return;
            }
            let cols = u32::try_from(columns.len()).expect("instance column count should fit u32");
            let rows = u32::try_from(columns[0].len()).expect("instance row count should fit u32");
            if columns
                .iter()
                .any(|column| u32::try_from(column.len()).ok() != Some(rows))
            {
                return;
            }
            let row_count = usize::try_from(rows).expect("row count should fit usize");
            let col_count = usize::try_from(cols).expect("column count should fit usize");
            let mut payload = Vec::with_capacity(8 + row_count * col_count * 32);
            payload.extend_from_slice(&cols.to_le_bytes());
            payload.extend_from_slice(&rows.to_le_bytes());
            for row in 0..row_count {
                for column in columns.iter().take(col_count) {
                    payload.extend_from_slice(column[row].to_repr().as_ref());
                }
            }
            write_tlv(buf, *b"I10P", &payload);
        }
    }
    /// Deterministic Halo2/IPA vote tally circuit (depth 8) exercising the production backend.
    pub struct VoteTallyProofBundle {
        /// Backend identifier for the proof attachment (`halo2/ipa`).
        pub backend: &'static str,
        /// Circuit identifier recorded alongside the verifying key.
        pub circuit_id: &'static str,
        /// Verifying key identifier (backend/name).
        pub vk_id: VerifyingKeyId,
        /// Registry-style verifying key record (inline bytes populated).
        pub vk_record: VerifyingKeyRecord,
        /// Norito-encoded `OpenVerifyEnvelope` bytes carrying the proof payload.
        pub proof_bytes: Vec<u8>,
        /// Canonical public-input encoding (commit || root).
        pub public_inputs: Vec<u8>,
        /// Deterministic commitment witness used when generating the proof.
        pub commit: Scalar,
        /// Deterministic Merkle root witness used when generating the proof.
        pub root: Scalar,
    }
    impl VoteTallyProofBundle {
        /// Return the commitment as raw 32-byte little-endian bytes.
        pub fn commit_bytes(&self) -> [u8; 32] {
            use halo2_proofs::halo2curves::ff::PrimeField as _;
            let mut out = [0u8; 32];
            out.copy_from_slice(self.commit.to_repr().as_ref());
            out
        }
        /// Return the root as raw 32-byte little-endian bytes.
        pub fn root_bytes(&self) -> [u8; 32] {
            use halo2_proofs::halo2curves::ff::PrimeField as _;
            let mut out = [0u8; 32];
            out.copy_from_slice(self.root.to_repr().as_ref());
            out
        }
        /// Base64-encode the proof envelope bytes.
        pub fn proof_b64(&self) -> String {
            base64::engine::general_purpose::STANDARD.encode(&self.proof_bytes)
        }
    }
    /// Generate a Halo2/IPA proof bundle for the production vote tally circuit.
    pub fn vote_merkle8_bundle() -> VoteTallyProofBundle {
        use halo2_proofs::{
            halo2curves::{
                ff::PrimeField as _,
                pasta::{EqAffine as Curve, Fp as Scalar},
            },
            plonk::{create_proof, keygen_pk, keygen_vk},
            transcript::{Blake2bWrite, Challenge255},
        };
        let backend = "halo2/ipa";
        let envelope_circuit_id = "halo2/pasta/ipa/vote-bool-commit-merkle8";
        let circuit_id = "halo2/pasta/vote-bool-commit-merkle8";
        let name = "tally_current";
        let k: u32 = 6;
        let params: <IPACommitmentScheme<Curve> as CommitmentScheme>::ParamsProver =
            ParamsIPA::<Curve>::new(k);
        let circuit = iroha_core::zk::depth::VoteBoolCommitMerkle::<8>::default();
        let vk_h2 = keygen_vk(&params, &circuit).expect("vk");
        let pk = keygen_pk(&params, vk_h2.clone(), &circuit).expect("pk");
        let rc0 = Scalar::from(7u64);
        let rc1 = Scalar::from(13u64);
        let two = Scalar::from(2u64);
        let three = Scalar::from(3u64);
        let compress = |a: Scalar, b: Scalar| {
            let a_shift = a + rc0;
            let b_shift = b + rc1;
            let a2 = a_shift * a_shift;
            let a4 = a2 * a2;
            let a5 = a4 * a_shift;
            let b2 = b_shift * b_shift;
            let b4 = b2 * b2;
            let b5 = b4 * b_shift;
            two * a5 + three * b5
        };
        let commit = compress(Scalar::one(), Scalar::from(12345u64));
        let siblings: [Scalar; 8] = core::array::from_fn(|i| Scalar::from(20u64 + i as u64));
        let mut root = commit;
        for sib in siblings {
            // All direction bits are zero in synthesize(), so follow the left branch.
            let expected = compress(root, sib);
            root = expected;
        }
        let inst_commit = [commit];
        let inst_root = [root];
        let inst_refs = [&inst_commit[..], &inst_root[..]];
        let insts = [&inst_refs[..]];
        let circuit_for_proof = circuit.clone();
        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        create_proof::<
            IPACommitmentScheme<Curve>,
            ProverIPA<'_, Curve>,
            Challenge255<Curve>,
            _,
            _,
            _,
        >(
            &params,
            &pk,
            &[circuit_for_proof],
            &insts,
            OsRng,
            &mut transcript,
        )
        .expect("create proof");
        let proof_raw = transcript.finalize();
        // Sanity check: the generated proof should verify before wrapping.
        {
            use halo2_proofs::transcript::{Blake2bRead, TranscriptReadBuffer};
            use std::io::Cursor;
            let cursor = Cursor::new(proof_raw.as_slice());
            let mut transcript = Blake2bRead::<_, Curve, Challenge255<Curve>>::init(cursor);
            let strategy = halo2_proofs::poly::ipa::strategy::SingleStrategy::<Curve>::new(&params);
            let res = halo2_proofs::plonk::verify_proof(
                &params,
                &vk_h2,
                strategy,
                &insts,
                &mut transcript,
            );
            #[cfg(debug_assertions)]
            if res.is_err() {
                let inst_refs_swapped = [&inst_root[..], &inst_commit[..]];
                let alt_insts = [&inst_refs_swapped[..]];
                let mut transcript_alt = Blake2bRead::<_, Curve, Challenge255<Curve>>::init(
                    Cursor::new(proof_raw.as_slice()),
                );
                let strategy_alt =
                    halo2_proofs::poly::ipa::strategy::SingleStrategy::<Curve>::new(&params);
                let res_swapped = halo2_proofs::plonk::verify_proof(
                    &params,
                    &vk_h2,
                    strategy_alt,
                    &alt_insts,
                    &mut transcript_alt,
                );
                panic!(
                    "vote tally halo2 verify_proof failed: {res:?}, swapped order: {res_swapped:?}"
                );
            }
            assert!(res.is_ok(), "vote tally halo2 verify_proof failed: {res:?}");
        }
        let commit_col = [commit];
        let root_col = [root];
        let mut proof_payload = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_payload, &proof_raw);
        zk1::wrap_append_instances_pasta_fp_cols(
            &[&commit_col[..], &root_col[..]],
            &mut proof_payload,
        );
        let mut vk_bytes = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_bytes, k);
        zk1::wrap_append_vk_pasta(&mut vk_bytes, &vk_h2);
        let mut public_inputs = Vec::with_capacity(64);
        public_inputs.extend_from_slice(commit.to_repr().as_ref());
        public_inputs.extend_from_slice(root.to_repr().as_ref());
        let public_inputs_hash: [u8; 32] = CryptoHash::new(&public_inputs).into();
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_bytes.clone());
        let commitment = zk::hash_vk(&vk_box);
        let mut vk_record = VerifyingKeyRecord::new(
            1,
            circuit_id,
            BackendTag::Halo2IpaPasta,
            "pallas",
            public_inputs_hash,
            commitment,
        );
        vk_record.vk_len = vk_bytes.len() as u32;
        // `max_proof_bytes` is enforced against the submitted proof payload length, which for
        // `halo2/ipa` is the full OpenVerifyEnvelope bytes.
        vk_record.gas_schedule_id = Some("halo2_default".into());
        vk_record.key = Some(vk_box);
        vk_record.status = ConfidentialStatus::Active;
        let envelope = OpenVerifyEnvelope {
            backend: BackendTag::Halo2IpaPasta,
            circuit_id: envelope_circuit_id.to_string(),
            vk_hash: commitment,
            public_inputs: public_inputs.clone(),
            proof_bytes: proof_payload,
            aux: Vec::new(),
        };
        let proof_bytes =
            norito::to_bytes(&envelope).expect("OpenVerifyEnvelope Norito serialization must work");
        vk_record.max_proof_bytes = proof_bytes.len() as u32;
        let proof_box = ProofBox::new(backend.into(), proof_bytes.clone());
        let vk_box = vk_record.key.as_ref().expect("VK bytes populated").clone();
        let report = zk::verify_backend_with_timing(backend, &proof_box, Some(&vk_box));
        assert!(report.ok, "vote tally proof must verify: {report:?}");
        VoteTallyProofBundle {
            backend,
            circuit_id,
            vk_id: VerifyingKeyId::new(backend, name),
            vk_record,
            proof_bytes,
            public_inputs,
            commit,
            root,
        }
    }
}
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub use halo2_bundle::{VoteTallyProofBundle, vote_merkle8_bundle};
#[cfg(all(
    test,
    feature = "zk-tests",
    any(feature = "zk-halo2", feature = "zk-halo2-ipa")
))]
mod vote_bundle_sanity {
    use super::halo2_bundle::vote_merkle8_bundle;
    use halo2_proofs::{
        dev::MockProver,
        halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
        plonk::{Circuit, ConstraintSystem},
        poly::{commitment::ParamsProver as _, ipa::commitment::ParamsIPA},
    };
    #[test]
    fn mock_prover_satisfies_vote_bundle() {
        let bundle = vote_merkle8_bundle();
        let k = 6;
        let params = ParamsIPA::<Curve>::new(k);
        let circuit = iroha_core::zk::depth::VoteBoolCommitMerkle::<8>::default();
        // Ensure circuit config does not panic when synthesized without witnesses.
        let mut cs = ConstraintSystem::<Scalar>::default();
        let _ = iroha_core::zk::depth::VoteBoolCommitMerkle::<8>::configure(&mut cs);
        let instances = vec![vec![bundle.commit], vec![bundle.root]];
        let prover = MockProver::run(k, &circuit, instances).expect("mock prover should run");
        assert!(prover.verify().is_ok());
        drop(params);
    }
}
