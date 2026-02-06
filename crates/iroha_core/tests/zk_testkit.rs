#![doc = "Helpers for generating minimal Halo2 proofs for governance tests."]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(all(feature = "zk-tests", feature = "halo2-dev-tests"))]

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
mod halo2_bundle {
    use base64::Engine as _;
    use halo2_proofs::{
        SerdeFormat,
        circuit::Value,
        halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
        plonk::{
            Circuit, ConstraintSystem, Error as PlonkError, Selector, create_proof, keygen_pk,
            keygen_vk,
        },
        poly::{
            Rotation, VerificationStrategy as _,
            commitment::{CommitmentScheme, ParamsProver as _},
            ipa::{
                commitment::{IPACommitmentScheme, ParamsIPA},
                multiopen::ProverIPA,
            },
        },
        transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer as _},
    };
    use iroha_core::zk::{self, zk1_test_helpers as zk1};
    use iroha_crypto::Hash as CryptoHash;
    use iroha_data_model::{
        confidential::ConfidentialStatus,
        proof::{ProofBox, VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
        zk::BackendTag,
    };
    use rand_core_06::OsRng;

    /// Deterministic tiny add circuit used across governance ZK tests.
    #[derive(Clone, Default)]
    struct TinyAdd;

    impl Circuit<Scalar> for TinyAdd {
        type Config = (
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            Selector,
        );
        type FloorPlanner = halo2_proofs::circuit::SimpleFloorPlanner;
        type Params = ();

        fn without_witnesses(&self) -> Self {
            Self
        }

        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let a = meta.advice_column();
            let b = meta.advice_column();
            let c = meta.advice_column();
            let s = meta.selector();
            meta.create_gate("add", |meta| {
                let s = meta.query_selector(s.clone());
                let a = meta.query_advice(a, Rotation::cur());
                let b = meta.query_advice(b, Rotation::cur());
                let c = meta.query_advice(c, Rotation::cur());
                vec![s * (a + b - c)]
            });
            (a, b, c, s)
        }

        fn synthesize(
            &self,
            (a, b, c, s): Self::Config,
            mut layouter: impl halo2_proofs::circuit::Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            layouter.assign_region(
                || "add",
                |mut region| {
                    s.enable(&mut region, 0)?;
                    region.assign_advice(a, 0, Value::known(Scalar::from(2)));
                    region.assign_advice(b, 0, Value::known(Scalar::from(2)));
                    region.assign_advice(c, 0, Value::known(Scalar::from(4)));
                    Ok(())
                },
            )
        }
    }

    /// Two-instance public circuit used to emit (commit, root) public inputs.
    #[derive(Clone)]
    struct AddTwoInstPublic {
        commit: Scalar,
        root: Scalar,
    }

    impl Default for AddTwoInstPublic {
        fn default() -> Self {
            Self {
                commit: Scalar::from(0),
                root: Scalar::from(0),
            }
        }
    }

    impl Circuit<Scalar> for AddTwoInstPublic {
        type Config = (
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>,
            Selector,
        );
        type FloorPlanner = halo2_proofs::circuit::SimpleFloorPlanner;
        type Params = ();

        fn without_witnesses(&self) -> Self {
            Self::default()
        }

        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let a = meta.advice_column();
            let b = meta.advice_column();
            let c = meta.advice_column();
            let inst0 = meta.instance_column();
            let inst1 = meta.instance_column();
            let s = meta.selector();
            meta.create_gate("add2inst", |meta| {
                let s = meta.query_selector(s.clone());
                let a = meta.query_advice(a, Rotation::cur());
                let b = meta.query_advice(b, Rotation::cur());
                let c = meta.query_advice(c, Rotation::cur());
                let i0 = meta.query_instance(inst0, Rotation::cur());
                let i1 = meta.query_instance(inst1, Rotation::cur());
                vec![
                    s.clone() * (a.clone() + b.clone() - c),
                    s.clone() * (a - i0),
                    s * (b - i1),
                ]
            });
            (a, b, c, inst0, inst1, s)
        }

        fn synthesize(
            &self,
            (a, b, c, _i0, _i1, s): Self::Config,
            mut layouter: impl halo2_proofs::circuit::Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            let sum = self.commit + self.root;
            layouter.assign_region(
                || "add2inst",
                |mut region| {
                    s.enable(&mut region, 0)?;
                    region.assign_advice(a, 0, Value::known(self.commit));
                    region.assign_advice(b, 0, Value::known(self.root));
                    region.assign_advice(c, 0, Value::known(sum));
                    Ok(())
                },
            )
        }
    }

    /// Pre-generated Halo2 proof and verifying key artifacts for TinyAdd.
    pub struct TinyAddProofBundle {
        /// Backend tag to use for ballots and tallies.
        pub backend: &'static str,
        /// Verifying key identifier (backend + name).
        pub vk_id: VerifyingKeyId,
        /// Registry record carrying the verifying key bytes.
        pub vk_record: VerifyingKeyRecord,
        /// Base64-encoded proof (ZK1 envelope) suitable for `CastZkBallot`.
        #[allow(dead_code)]
        pub proof_b64: String,
        /// Raw proof envelope bytes (ZK1) for tally attachments.
        #[allow(dead_code)]
        pub proof_bytes: Vec<u8>,
    }

    /// Proof bundle for the two-instance public circuit emitting (commit, root).
    pub struct AddTwoInstPublicProofBundle {
        /// Backend tag to use for ballots.
        pub backend: &'static str,
        /// Verifying key identifier (backend + name).
        pub vk_id: VerifyingKeyId,
        /// Registry record carrying the verifying key bytes.
        pub vk_record: VerifyingKeyRecord,
        /// Base64-encoded proof (ZK1 envelope) suitable for `CastZkBallot`.
        pub proof_b64: String,
        /// Raw proof envelope bytes (ZK1).
        pub proof_bytes: Vec<u8>,
        /// Commitment public input (commit).
        pub commit: Scalar,
        /// Merkle root public input (root).
        pub root: Scalar,
    }

    impl AddTwoInstPublicProofBundle {
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
    }

    /// Generate a deterministic TinyAdd proof bundle for tests.
    pub fn tiny_add_bundle() -> TinyAddProofBundle {
        let backend = "halo2/pasta/tiny-add-v1";
        let name = "ballot_v1";
        let k: u32 = 5;

        // Build Halo2 parameters and verifying/proving keys.
        let params: <IPACommitmentScheme<Curve> as CommitmentScheme>::ParamsProver =
            ParamsIPA::<Curve>::new(k);
        let vk_h2 = keygen_vk(&params, &TinyAdd::default()).expect("vk");
        let pk = keygen_pk(&params, vk_h2.clone(), &TinyAdd::default()).expect("pk");

        // Create a single proof transcript for TinyAdd with witness a=b=2, c=4.
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
            &[TinyAdd::default()],
            &[&[][..]],
            OsRng,
            &mut transcript,
        )
        .expect("create proof");
        let proof_raw = transcript.finalize();

        // Wrap proof in the ZK1 envelope expected by the verifier.
        let mut proof_bytes = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_bytes, &proof_raw);
        let proof_b64 = base64::engine::general_purpose::STANDARD.encode(&proof_bytes);

        // Construct verifying key bytes (ZK1 envelope with IPAK + H2VK) and registry record.
        let vk_bytes_raw = vk_h2.to_bytes(SerdeFormat::Processed);
        let mut vk_bytes = Vec::new();
        vk_bytes.extend_from_slice(b"ZK1\0");
        vk_bytes.extend_from_slice(b"IPAK");
        vk_bytes.extend_from_slice(&(4u32).to_le_bytes());
        vk_bytes.extend_from_slice(&k.to_le_bytes());
        vk_bytes.extend_from_slice(b"H2VK");
        vk_bytes.extend_from_slice(&(vk_bytes_raw.len() as u32).to_le_bytes());
        vk_bytes.extend_from_slice(&vk_bytes_raw);
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_bytes);
        let vk_id = VerifyingKeyId::new(backend, name);
        let vk_len = vk_box.bytes.len() as u32;
        let commitment = zk::hash_vk(&vk_box);
        let mut vk_record = VerifyingKeyRecord::new(
            1,
            name,
            BackendTag::Halo2IpaPasta,
            "pallas",
            [0x33; 32],
            commitment,
        );
        vk_record.vk_len = vk_len;
        vk_record.max_proof_bytes = proof_bytes.len() as u32;
        vk_record.status = ConfidentialStatus::Active;
        vk_record.key = Some(vk_box);
        vk_record.gas_schedule_id = Some("halo2_default".into());

        TinyAddProofBundle {
            backend,
            vk_id,
            vk_record,
            proof_b64,
            proof_bytes,
        }
    }

    /// Generate a Halo2/IPA proof bundle for the two-instance public circuit.
    pub fn add2inst_public_bundle(commit: u64, root: u64) -> AddTwoInstPublicProofBundle {
        use halo2_proofs::halo2curves::ff::PrimeField as _;

        let backend = "halo2/pasta/ipa-v1/tiny-add2inst-public-v1";
        let name = "ballot_v1";
        let k: u32 = 6;

        let params: <IPACommitmentScheme<Curve> as CommitmentScheme>::ParamsProver =
            ParamsIPA::<Curve>::new(k);
        let vk_h2 = keygen_vk(&params, &AddTwoInstPublic::default()).expect("vk");
        let pk = keygen_pk(&params, vk_h2.clone(), &AddTwoInstPublic::default()).expect("pk");

        let commit = Scalar::from(commit);
        let root = Scalar::from(root);
        let inst0 = vec![commit];
        let inst1 = vec![root];
        let inst_cols: Vec<&[Scalar]> = vec![inst0.as_slice(), inst1.as_slice()];
        let inst_refs: Vec<&[&[Scalar]]> = vec![inst_cols.as_slice()];
        let circuit = AddTwoInstPublic { commit, root };

        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        create_proof::<
            IPACommitmentScheme<Curve>,
            ProverIPA<'_, Curve>,
            Challenge255<Curve>,
            _,
            _,
            _,
        >(&params, &pk, &[circuit], &inst_refs, OsRng, &mut transcript)
        .expect("create proof");
        let proof_raw = transcript.finalize();

        let mut proof_bytes = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_bytes, &proof_raw);
        zk1::wrap_append_instances_pasta_fp_cols(&inst_cols, &mut proof_bytes);
        let proof_b64 = base64::engine::general_purpose::STANDARD.encode(&proof_bytes);

        let mut vk_bytes = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_bytes, k);
        zk1::wrap_append_vk_pasta(&mut vk_bytes, &vk_h2);
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_bytes.clone());
        let commitment = zk::hash_vk(&vk_box);

        let mut public_inputs = Vec::with_capacity(64);
        public_inputs.extend_from_slice(commit.to_repr().as_ref());
        public_inputs.extend_from_slice(root.to_repr().as_ref());
        let public_inputs_hash: [u8; 32] = CryptoHash::new(&public_inputs).into();
        let mut vk_record = VerifyingKeyRecord::new(
            1,
            "halo2/pasta/tiny-add2inst-public-v1",
            BackendTag::Halo2IpaPasta,
            "pallas",
            public_inputs_hash,
            commitment,
        );
        vk_record.vk_len = vk_bytes.len() as u32;
        vk_record.max_proof_bytes = proof_bytes.len() as u32;
        vk_record.gas_schedule_id = Some("halo2_default".into());
        vk_record.key = Some(vk_box);
        vk_record.status = ConfidentialStatus::Active;

        AddTwoInstPublicProofBundle {
            backend,
            vk_id: VerifyingKeyId::new(backend, name),
            vk_record,
            proof_b64,
            proof_bytes,
            commit,
            root,
        }
    }

    /// Deterministic Halo2/IPA vote tally circuit (depth 8) exercising the production backend.
    pub struct VoteTallyProofBundle {
        /// Backend tag for the tally circuit.
        pub backend: &'static str,
        /// Circuit identifier recorded alongside the verifying key.
        pub circuit_id: &'static str,
        /// Verifying key identifier (backend/name).
        pub vk_id: VerifyingKeyId,
        /// Registry-style verifying key record (inline bytes populated).
        pub vk_record: VerifyingKeyRecord,
        /// ZK1 envelope bytes carrying the proof and public instances.
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

        let backend = "halo2/pasta/ipa-v1/vote-bool-commit-merkle8-v1";
        let circuit_id = "halo2/pasta/vote-bool-commit-merkle8-v1";
        let name = "tally_v1";
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
        #[cfg(debug_assertions)]
        println!("vote_merkle commit = {:?}", commit);
        let siblings: [Scalar; 8] = core::array::from_fn(|i| Scalar::from(20u64 + i as u64));
        let mut root = commit;
        for sib in siblings {
            // All direction bits are zero in synthesize(), so follow the left branch.
            let expected = compress(root, sib);
            #[cfg(debug_assertions)]
            {
                let t0a = root + Scalar::from(7u64);
                let t1a = sib + Scalar::from(13u64);
                let t0a2 = t0a * t0a;
                let t0a4 = t0a2 * t0a2;
                let t0a5 = t0a4 * t0a;
                let t1a2 = t1a * t1a;
                let t1a4 = t1a2 * t1a2;
                let t1a5 = t1a4 * t1a;
                let h01 = Scalar::from(2u64) * t0a5 + Scalar::from(3u64) * t1a5;
                debug_assert_eq!(expected, h01, "computed compress mismatch at level");
            }
            root = expected;
        }
        #[cfg(debug_assertions)]
        println!("vote_merkle root = {:?}", root);

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
            use std::io::Cursor;

            use halo2_proofs::transcript::{Blake2bRead, TranscriptReadBuffer};

            #[cfg(debug_assertions)]
            {
                use halo2_proofs::dev::MockProver;

                let instances = vec![vec![commit], vec![root]];
                match MockProver::run(k, &circuit, instances) {
                    Ok(prover) => match prover.verify() {
                        Ok(()) => println!("mock prover verify: ok"),
                        Err(failures) => println!("mock prover failures: {:?}", failures),
                    },
                    Err(err) => {
                        println!("mock prover run error: {err:?}");
                    }
                }
            }

            let cursor = Cursor::new(proof_raw.as_slice());
            let mut transcript = Blake2bRead::<_, Curve, Challenge255<Curve>>::init(cursor);
            let strategy = halo2_proofs::poly::ipa::strategy::SingleStrategy::new(&params);
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
                let strategy_alt = halo2_proofs::poly::ipa::strategy::SingleStrategy::new(&params);
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
        let mut proof_bytes = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_bytes, &proof_raw);
        zk1::wrap_append_instances_pasta_fp_cols(
            &[&commit_col[..], &root_col[..]],
            &mut proof_bytes,
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
        vk_record.max_proof_bytes = proof_bytes.len() as u32;
        vk_record.gas_schedule_id = Some("halo2_default".into());
        vk_record.key = Some(vk_box);
        vk_record.status = ConfidentialStatus::Active;

        let proof_box = ProofBox::new(backend.into(), proof_bytes.clone());
        let vk_inline = vk_record.key.as_ref().expect("inline vk populated").clone();
        let report = zk::verify_backend_with_timing(backend, &proof_box, Some(&vk_inline));
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
pub use halo2_bundle::{
    AddTwoInstPublicProofBundle, TinyAddProofBundle, VoteTallyProofBundle, add2inst_public_bundle,
    tiny_add_bundle, vote_merkle8_bundle,
};

#[cfg(all(
    test,
    feature = "zk-tests",
    any(feature = "zk-halo2", feature = "zk-halo2-ipa")
))]
mod vote_bundle_sanity {
    use halo2_proofs::{
        dev::MockProver,
        halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
        plonk::{Circuit, ConstraintSystem},
        poly::{commitment::ParamsProver as _, ipa::commitment::ParamsIPA},
    };

    use super::halo2_bundle::vote_merkle8_bundle;

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
