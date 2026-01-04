#![doc = "Gated test: `CastZkBallot` verifies via real VK path (halo2/pasta tiny-add) with dev toggle OFF.\nRequires feature `zk-halo2`. Skipped by default; run with `IROHA_RUN_IGNORED=1`."]
#![cfg(feature = "zk-tests")]
#![cfg(feature = "halo2-dev-tests")]

#[cfg(feature = "zk-halo2")]
#[test]
fn zk_ballot_verifies_with_registered_vk_tiny_add() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!("Skipping: gated (IROHA_RUN_IGNORED!=1)");
        return;
    }

    use core::num::NonZeroU64;

    use halo2_proofs::{
        halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
        plonk::{Circuit, ConstraintSystem, Error as PlonkError, Selector, keygen_pk, keygen_vk},
        poly::{Rotation, commitment::Params},
        transcript::Blake2bWrite,
    };
    use iroha_core::{
        block::ValidBlock, executor::Executor, kura::Kura, query::store::LiveQueryStore,
        smartcontracts::Execute, state::State, zk::zk1_test_helpers as zk1,
    };
    use iroha_data_model::{
        confidential::ConfidentialStatus,
        isi::{governance::CastZkBallot, verifying_keys, zk::CreateElection},
        prelude::{InstructionBox, PeerId},
        proof::{VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
        zk::BackendTag,
    };
    use iroha_test_samples::ALICE_ID;
    use rand::rngs::OsRng;

    // Tiny-add circuit without public instances
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
            use halo2_proofs::circuit::Value;
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

    // Build State (dev toggle OFF by default)
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(iroha_core::state::World::default(), kura, query);

    // Ensure dev toggle is OFF explicitly
    let mut gov_cfg = state.gov.clone();
    gov_cfg.vk_ballot = Some(iroha_config::parameters::actual::VerifyingKeyRef {
        backend: "halo2/pasta/tiny-add-v1".to_string(),
        name: "ballot_v1".to_string(),
    });
    state.set_gov(gov_cfg);

    // Begin a transaction context
    let (pk, sk) = iroha_crypto::KeyPair::random().into_parts();
    let topo = iroha_core::sumeragi::network_topology::Topology::new(vec![PeerId::new(pk)]);
    let block = ValidBlock::new_dummy_and_modify_header(&sk, |h| {
        h.set_height(NonZeroU64::new(1).unwrap());
    })
    .commit(&topo)
    .unpack(|_| {})
    .unwrap();
    let mut sblock = state.block(block.as_ref().header());
    let mut stx = sblock.transaction();

    // Register inline VK record (ZK1 IPAK + H2VK)
    let backend = "halo2/pasta/tiny-add-v1";
    let k: u32 = 5;
    let params: Params<Curve> = Params::<Curve>::new(k);
    let vk_h2 = keygen_vk(&params, &TinyAdd::default()).expect("vk");
    let pk = keygen_pk(&params, vk_h2.clone(), &TinyAdd::default()).expect("pk");
    let mut vk_bytes = zk1::wrap_start();
    zk1::wrap_append_ipa_k(&mut vk_bytes, k);
    zk1::wrap_append_vk_pasta(&mut vk_bytes, &vk_h2);
    let vk_box = VerifyingKeyBox::new(backend.into(), vk_bytes);
    let vk_id = VerifyingKeyId::new(backend, "ballot_v1");
    let commitment = iroha_core::zk::hash_vk(&vk_box);
    let vk_len = vk_box.bytes.len() as u32;
    let mut rec = VerifyingKeyRecord::new(
        1,
        "ballot_v1",
        BackendTag::Halo2IpaPasta,
        "pallas",
        [0x56; 32],
        commitment,
    );
    rec.vk_len = vk_len;
    rec.status = ConfidentialStatus::Active;
    rec.key = Some(vk_box);
    rec.gas_schedule_id = Some("halo2_default".into());
    let exec = Executor::default();
    let reg_instr: InstructionBox = verifying_keys::RegisterVerifyingKey {
        id: vk_id.clone(),
        record: rec,
    }
    .into();
    exec.execute_instruction(&mut stx, &ALICE_ID.clone(), reg_instr)
        .expect("register vk");

    // Create election with 1 option, use the same backend tag
    let create = CreateElection::new(
        "ref-vk".to_string(),
        1,
        [0u8; 32],
        0,
        0,
        vk_id.clone(),
        vk_id.clone(),
        "gov:ballot:v1".to_string(),
    );
    create.execute(&ALICE_ID, &mut stx).expect("create ok");
    stx.world.governance_referenda_mut().insert(
        "ref-vk".to_string(),
        iroha_core::state::GovernanceReferendumRecord {
            h_start: 0,
            h_end: 100,
            status: iroha_core::state::GovernanceReferendumStatus::Proposed,
            mode: iroha_core::state::GovernanceReferendumMode::Zk,
        },
    );

    // Build valid proof for TinyAdd @ k and wrap as ZK1
    let mut transcript = Blake2bWrite::<_, Curve, _>::init(vec![]);
    halo2_proofs::plonk::create_proof::<_, _, _, _>(
        &params,
        &pk,
        &[TinyAdd::default()],
        &[&[]],
        OsRng,
        &mut transcript,
    )
    .expect("create proof");
    let proof_raw = transcript.finalize();
    let mut proof_container = zk1::wrap_start();
    zk1::wrap_append_proof(&mut proof_container, &proof_raw);

    // Cast ballot with base64-encoded ZK1
    let proof_b64 = base64::engine::general_purpose::STANDARD.encode(&proof_container);
    let public_inputs = norito::json::object([
        (
            "nullifier_hex",
            norito::json::to_value(&"55".repeat(32)).expect("serialize nullifier"),
        ),
        (
            "owner",
            norito::json::to_value(&ALICE_ID.to_string()).expect("serialize owner"),
        ),
        (
            "salt",
            norito::json::to_value(&"66".repeat(32)).expect("serialize salt"),
        ),
    ])
    .expect("serialize public inputs");
    let public_inputs =
        norito::json::to_json(&public_inputs).expect("encode public inputs to JSON");
    let cast = CastZkBallot {
        election_id: "ref-vk".to_string(),
        proof_b64,
        public_inputs_json: public_inputs,
    };
    cast.execute(&ALICE_ID, &mut stx).expect("cast ok");
}
