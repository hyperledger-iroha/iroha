#![doc = "Gated test: `FinalizeElection` verifies tally proof via real VK (halo2/pasta tiny-add).\nRequires feature `zk-halo2`. Skipped by default; run with `IROHA_RUN_IGNORED=1`."]
#![cfg(all(feature = "zk-tests", feature = "halo2-dev-tests"))]

#[cfg(feature = "zk-halo2")]
#[test]
fn zk_finalize_verifies_with_inline_vk_tiny_add() {
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
        block::ValidBlock, kura::Kura, query::store::LiveQueryStore, smartcontracts::Execute,
        state::State, zk::zk1_test_helpers as zk1,
    };
    use iroha_data_model::{
        isi::zk::{CreateElection, FinalizeElection},
        prelude::PeerId,
        proof::{ProofAttachment, ProofBox, VerifyingKeyBox, VerifyingKeyId},
    };
    use iroha_test_samples::ALICE_ID;
    use rand::rngs::OsRng;

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

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(iroha_core::state::World::default(), kura, query);

    let (pk, sk) = iroha_crypto::KeyPair::random().into_parts();
    let topo = iroha_core::sumeragi::network_topology::Topology::new(vec![PeerId::new(pk)]);
    let block =
        ValidBlock::new_dummy_and_modify_header(&sk, |h| h.set_height(NonZeroU64::new(1).unwrap()))
            .commit(&topo)
            .unpack(|_| {})
            .unwrap();
    let mut sblock = state.block(block.as_ref().header());
    let mut stx = sblock.transaction();

    // Create election with 1 option (vk ids irrelevant since we provide inline VK for tally)
    let create = CreateElection::new(
        "ref-final".to_string(),
        1,
        [0u8; 32],
        0,
        0,
        VerifyingKeyId::new("halo2/pasta/tiny-add-v1", "unused"),
        VerifyingKeyId::new("halo2/pasta/tiny-add-v1", "unused"),
        "gov:ballot:v1".to_string(),
    );
    create.execute(&ALICE_ID, &mut stx).expect("create ok");

    // Build a valid ZK1 proof and inline VK
    let k: u32 = 5;
    let params: Params<Curve> = Params::<Curve>::new(k);
    let vk_h2 = keygen_vk(&params, &TinyAdd::default()).expect("vk");
    let pk = keygen_pk(&params, vk_h2.clone(), &TinyAdd::default()).expect("pk");
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
    let backend = "halo2/pasta/tiny-add-v1";
    let prf_box = ProofBox::new(backend.into(), proof_container);

    let mut vk_bytes = zk1::wrap_start();
    zk1::wrap_append_ipa_k(&mut vk_bytes, k);
    zk1::wrap_append_vk_pasta(&mut vk_bytes, &vk_h2);
    let vk_box = VerifyingKeyBox::new(backend.into(), vk_bytes);

    // Finalize with tally [0] and inline VK
    let att = ProofAttachment::new_inline(backend.into(), prf_box, vk_box);
    let fin = FinalizeElection::new("ref-final".to_string(), vec![0], att);
    fin.execute(&ALICE_ID, &mut stx).expect("finalize ok");

    // Assert finalized
    let st = stx
        .world
        .elections
        .get(&"ref-final".to_string())
        .cloned()
        .unwrap();
    assert!(st.finalized);
    assert_eq!(st.tally, vec![0]);
}
