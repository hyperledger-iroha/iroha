//! Authenticated borrowed paid AMX admission retains the original persisted proof graph.
//!
//! This control covers borrowed source ownership only. Historical proof DTOs and their
//! scratch remain ordinary allocations; neither this control nor shared cached wire
//! establishes complete physical graph funding, owned Queue custody or H2 qualification.

use super::*;
use crate::{
    state::{StateReadOnly, WorldReadOnly},
    tx::AcceptedTransaction,
};
use iroha_data_model::{
    nexus::FeeDebitSource,
    sumeragi_amx::AmxRecordProofV1,
    transaction::{
        Executable, FeeChargeKind, FeeChargeLimit, FeePaymentIntent, SignedTransaction,
        TransactionBuilder, TransactionEntrypoint,
    },
};
use iroha_primitives::time::TimeSource;
use std::{sync::Arc, time::Duration};

const GLOBAL_FEE: u32 = 2;
const GLOBAL_INITIAL: u32 = 1_000_000;

fn paid_global_config() -> TestChainConfig {
    let mut config = global_config();
    let domain = DomainId::try_new(
        "fees",
        iroha_config::parameters::defaults::nexus::DEFAULT_DATASPACE_ALIAS,
    )
    .unwrap();
    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    nexus.fees.settlement_mode = iroha_config::parameters::actual::NexusFeeSettlementMode::Direct;
    nexus.fees.fee_asset_id = fee_asset().canonical_address();
    nexus.fees.fee_sink_account_id = account(&config.genesis_key).to_string();
    nexus.fees.base_fee = GLOBAL_FEE.into();
    nexus.fees.per_byte_fee = Quantity::zero();
    nexus.fees.per_instruction_fee = Quantity::zero();
    nexus.fees.per_gas_unit_fee = Quantity::zero();
    config.nexus = Some(nexus);
    config.pipeline.gas.accepted_assets.clear();
    config.pipeline.gas.units_per_gas.clear();
    config.genesis_instructions.extend([
        Register::domain(iroha_data_model::domain::Domain::new(domain.clone())).into(),
        Register::asset_definition(AssetDefinition::numeric(
            fee_asset(),
            "AMX global fee",
            AssetBalancePolicy::Global,
            Some(domain),
        ))
        .into(),
        Mint::asset_quantity(
            GLOBAL_INITIAL,
            AssetId::with_scope(
                fee_asset(),
                account(&config.genesis_key),
                AssetBalanceScope::Global,
            ),
        )
        .into(),
        Mint::asset_quantity(
            GLOBAL_INITIAL,
            AssetId::with_scope(
                fee_asset(),
                account(&KeyPair::from_seed(vec![0xCC; 32], Algorithm::Ed25519)),
                AssetBalanceScope::Global,
            ),
        )
        .into(),
    ]);
    config
}

fn paid_global_sign(
    chain: &CertifiedTestChain,
    instructions: impl IntoIterator<Item = InstructionBox>,
    created_ms: u64,
) -> SignedTransaction {
    let key = global_config().genesis_key;
    let intent = FeePaymentIntent::authority(
        vec![FeeChargeLimit::new(
            FeeChargeKind::Nexus,
            fee_asset(),
            GLOBAL_FEE.into(),
        )],
        None,
    );
    let mut builder = TransactionBuilder::new(chain.network_id(), account(&key), intent);
    builder.set_creation_time(Duration::from_millis(created_ms));
    // Move the original proof into the sole ordinary instruction graph: no builder clone,
    // second decoder, DTO clone, or accounting wrapper between the reader and admission.
    let signed = builder
        .with_instructions(instructions)
        .sign(key.private_key());
    let view = chain.state().view();
    let quote = crate::executor::quote_nexus_fee_admission(
        view.world(),
        view.nexus(),
        view.pipeline(),
        &signed,
        created_ms,
        chain.height() + 1,
        Some(DataSpaceId::UNIVERSAL),
    )
    .expect("the original signed fee policy admits this genuinely funded paid input");
    assert_eq!(quote.debit_source, FeeDebitSource::Account(account(&key)));
    assert_eq!(quote.charges.len(), 1);
    assert_eq!(quote.charges[0].kind, FeeChargeKind::Nexus);
    assert_eq!(quote.charges[0].asset_definition_id, fee_asset());
    assert_eq!(quote.charges[0].max_bound, GLOBAL_FEE.into());
    signed
        .payload()
        .validate_fee_payment_intent()
        .expect("original canonical finite signed fee intent");
    assert_eq!(signed.fee_payment_intent().charge_limits().len(), 1);
    assert_eq!(
        signed.fee_payment_intent().charge_limits()[0].max_amount,
        GLOBAL_FEE.into()
    );
    signed
}

fn global_paid_images(chain: &CertifiedTestChain, paid_inputs: u32) {
    let view = chain.state().view();
    let expected = GLOBAL_INITIAL - GLOBAL_FEE * paid_inputs;
    let clock = account(&KeyPair::from_seed(vec![0xCC; 32], Algorithm::Ed25519));
    let clock_asset = AssetId::with_scope(fee_asset(), clock, AssetBalanceScope::Global);
    assert_eq!(
        view.world().assets().get(&clock_asset).unwrap().as_ref(),
        &Quantity::from(GLOBAL_INITIAL)
    );
    let owner = account(&global_config().genesis_key);
    let asset = AssetId::with_scope(fee_asset(), owner, AssetBalanceScope::Global);
    assert_eq!(
        view.world().assets().get(&asset).unwrap().as_ref(),
        &Quantity::from(expected)
    );
    assert_eq!(
        view.world()
            .asset_definition(&fee_asset())
            .unwrap()
            .total_quantity(),
        &Quantity::from(GLOBAL_INITIAL + expected),
        "actual mandatory direct fees burn supply, without crediting a replacement owner",
    );
}

#[inline(never)]
fn paid_roots() -> Roots {
    let mut global = CertifiedTestChain::start(paid_global_config()).unwrap();
    {
        let view = global.state().view();
        let definition = view.world().asset_definition(&fee_asset()).unwrap();
        assert_eq!(
            definition.balance_scope_policy(),
            AssetBalancePolicy::Global
        );
        let home = definition
            .owning_domain()
            .as_ref()
            .expect("signed fee asset home");
        assert_eq!(
            crate::sns::resolve_active_dataspace_id_by_alias(
                view.world(),
                &view.nexus().dataspace_catalog,
                home.dataspace().as_ref(),
                global.committed(1).block_time_ms(),
            )
            .expect("the actual signed catalog authenticates the fee asset home"),
            DataSpaceId::UNIVERSAL,
        );
    }
    assert_eq!(
        global.validators().len(),
        4,
        "original signed global committee"
    );
    let h2 = paid_global_sign(
        &global,
        [iroha_data_model::isi::Log::new(
            iroha_logger::Level::INFO,
            "authenticate paid AMX global genesis".into(),
        )
        .into()],
        1_499,
    );
    assert_eq!(global.commit_at(1_500, vec![h2]), vec![true]);
    global_paid_images(&global, 1);
    let participants =
        [FIRST, SECOND].map(|id| CertifiedTestChain::start(private_config(&global, id)).unwrap());
    assert_ne!(participants[0].network_id(), participants[1].network_id());
    assert_ne!(
        participants[0].kura().store_root(),
        participants[1].kura().store_root()
    );
    assert!(
        !participants[0]
            .state()
            .ivm_execution_budget()
            .same_pool(&participants[1].state().ivm_execution_budget())
    );
    for participant in &participants {
        assert_ne!(participant.network_id(), global.network_id());
        assert_ne!(participant.kura().store_root(), global.kura().store_root());
        assert!(
            !participant
                .state()
                .ivm_execution_budget()
                .same_pool(&global.state().ivm_execution_budget())
        );
        assert_eq!(
            participant.validators().len(),
            4,
            "original signed private committee"
        );
    }
    let registrations = [FIRST, SECOND]
        .into_iter()
        .zip(&participants)
        .map(|(id, chain)| {
            RegisterAmxDataspaceV1 {
                dataspace: id,
                instance: chain.instance().0,
                anchor: norito::encode_canonical(&genesis_epoch(chain.genesis()).unwrap()).unwrap(),
            }
            .into()
        });
    let signed = paid_global_sign(&global, registrations, 1_999);
    assert_eq!(global.commit_at(2_000, vec![signed]), vec![true]);
    global_paid_images(&global, 2);
    Roots {
        global,
        participants,
    }
}

type ProofPointers = (*const u8, *const u8, *const u8, *const [u8; 32]);

fn proof_pointers(proof: &AmxRecordProofV1) -> ProofPointers {
    assert!(!proof.block.consensus_header.is_empty());
    assert!(!proof.block.commit_qc.is_empty());
    assert!(!proof.block.result_preimage.is_empty());
    assert!(
        !proof.write.siblings.is_empty(),
        "the actual full write tree has real siblings"
    );
    (
        proof.block.consensus_header.as_ptr(),
        proof.block.commit_qc.as_ptr(),
        proof.block.result_preimage.as_ptr(),
        proof.write.siblings.as_ptr(),
    )
}

fn retained_relay<'accepted>(
    accepted: &'accepted AcceptedTransaction<'_>,
) -> &'accepted AmxRecordProofV1 {
    let signed = accepted
        .external()
        .expect("ordinary signed external entrypoint");
    let Executable::Instructions(instructions) = signed.instructions() else {
        panic!("one actual native relay instruction");
    };
    assert_eq!(instructions.len(), 1);
    &instructions[0]
        .as_any()
        .downcast_ref::<RelayAmxPreparedV1>()
        .expect("the actual closed Prepared relay instruction")
        .proof
}

fn authenticate_original_prepared(
    roots: &Roots,
    index: usize,
    proof: &AmxRecordProofV1,
    tx: [u8; 32],
) {
    let chain = &roots.participants[index];
    let id = [FIRST, SECOND][index];
    assert!(matches!(&proof.record, AmxRecordV1::Prepared(record)
        if record.tx == tx && record.participant == id && matches!(&record.vote, AmxVoteV1::Yes(_))));
    let committed = chain.committed(chain.height());
    let certificate = committed.block().commit_certificate().unwrap();
    assert_eq!(
        proof.block.consensus_header.as_slice(),
        certificate.consensus_header()
    );
    assert_eq!(proof.block.commit_qc.as_slice(), certificate.commit_qc());
    assert_eq!(
        proof.block.result_preimage.as_slice(),
        certificate.result_preimage()
    );
    let view = roots.global.state().view();
    let registration = view.world().sumeragi_amx().dataspace(id).unwrap();
    assert_eq!(registration.tracker.instance, chain.instance().0);
    let verified = registration
        .tracker
        .verify_record(proof)
        .expect("genuine exact registered source certificate and writes");
    assert_eq!(verified.height, chain.height());
    assert!(
        verified.height >= 2,
        "original certified non-genesis persisted Prepare"
    );
    proof_pointers(proof);
}

#[inline(never)]
fn paid_original_relay() -> (Roots, AcceptedTransaction<'static>, ProofPointers, [u8; 32]) {
    let mut roots = paid_roots();
    let transaction = roots.transaction(100, 0x63);
    let tx = transaction.id().unwrap();
    let signed = paid_global_sign(
        &roots.global,
        [BeginAmxV1 {
            transaction: transaction.clone(),
        }
        .into()],
        2_999,
    );
    assert_eq!(roots.global.commit_at(3_000, vec![signed]), vec![true]);
    global_paid_images(&roots.global, 3);
    let begin = super::super::super::amx_record_proof(
        &roots.global.state().view(),
        roots.global.height(),
        AmxRecordKind::Begin,
        tx,
    )
    .unwrap()
    .unwrap();
    // Existing preparation is customer-authorized, paid, applied and persisted by the real
    // Worker/Kura/archive path. The reader authenticates the full original write root.
    let first = roots.prepare(0, &transaction, &begin);
    let second = roots.prepare(1, &transaction, &begin);
    authenticate_original_prepared(&roots, 0, &first, tx);
    authenticate_original_prepared(&roots, 1, &second, tx);
    for (id, chain) in [FIRST, SECOND].into_iter().zip(&roots.participants) {
        assert_eq!(
            balance(chain, id, principal(), account(&payer())),
            900_u32.into()
        );
        assert_eq!(
            balance(chain, id, principal(), native(chain).custody),
            100_u32.into()
        );
        assert_eq!(
            balance(chain, id, principal(), account(&receiver())),
            Quantity::zero()
        );
        assert!(balance(chain, id, fee_asset(), account(&payer())) < 1_000_000_u32.into());
    }
    // Prove genuine paid target execution before the new clone assertion can fail. This
    // sibling remains only a Yes vote; it cannot manufacture a Decision for the first proof.
    let sibling = paid_global_sign(
        &roots.global,
        [RelayAmxPreparedV1 { proof: second }.into()],
        4_499,
    );
    assert_eq!(roots.global.commit_at(4_500, vec![sibling]), vec![true]);
    global_paid_images(&roots.global, 4);
    {
        let view = roots.global.state().view();
        let entry = view.world().sumeragi_amx().transaction(&tx).unwrap();
        assert_eq!(entry.yes.len(), 1);
        assert_eq!(entry.yes[0].participant, SECOND);
        assert!(entry.decided.is_none());
    }
    let original = proof_pointers(&first);
    let signed = paid_global_sign(
        &roots.global,
        [RelayAmxPreparedV1 { proof: first }.into()],
        4_999,
    );
    let (_, clock) = TimeSource::new_mock(Duration::from_millis(5_000));
    let accepted = {
        let view = roots.global.state().view();
        AcceptedTransaction::accept_with_time_source(
            signed,
            &roots.global.network_id(),
            Duration::from_secs(1),
            view.world().parameters().transaction(),
            &view.crypto(),
            &clock,
        )
        .expect("ordinary original network/signature/TTL/NTS transaction admission")
    };
    assert_eq!(
        proof_pointers(retained_relay(&accepted)),
        original,
        "move-only reader-to-signer-to-admission setup"
    );
    (roots, accepted, original, tx)
}

fn commit_original_relay(roots: &mut Roots, accepted: AcceptedTransaction<'static>, tx: [u8; 32]) {
    let TransactionEntrypoint::External(signed) = accepted.into_entrypoint() else {
        panic!("ordinary external relay remained unchanged");
    };
    assert_eq!(roots.global.commit_at(5_000, vec![signed]), vec![true]);
    global_paid_images(&roots.global, 5);
    let decision = super::super::super::amx_record_proof(
        &roots.global.state().view(),
        roots.global.height(),
        AmxRecordKind::Decision,
        tx,
    )
    .unwrap()
    .unwrap();
    assert!(matches!(decision.record, AmxRecordV1::Decision(value)
        if value.tx == tx && value.outcome == AmxOutcomeV1::Commit));
}

#[test]
fn native_amx_persisted_paid_borrowed_prepared_proof_clone_retains_original_graph_and_lifetime() {
    // This fixture executes three actual native roots; use the node's existing
    // configured execution thread, with no stack override or changed resource limit.
    fn original_paid_control() {
        let (mut roots, accepted, original, tx) = paid_original_relay();
        let original_wire = accepted.entrypoint_bytes();
        let original_hash = accepted.hash_as_entrypoint();
        let entrypoint = accepted.into_entrypoint();
        let source_pointer = std::ptr::from_ref(&entrypoint);
        let borrowed =
            {
                let view = roots.global.state().view();
                AcceptedTransaction::accept_borrowed_entrypoint_at_time(
            &entrypoint,
            &roots.global.network_id(),
            Duration::from_secs(1),
            view.world().parameters().transaction(),
            &view.crypto(),
            Duration::from_millis(5_000),
        )
        .expect("the original paid source passes actual borrowed envelope/signature/TTL admission")
            };
        assert_eq!(std::ptr::from_ref(borrowed.entrypoint()), source_pointer);
        assert_eq!(proof_pointers(retained_relay(&borrowed)), original);
        assert_eq!(borrowed.hash_as_entrypoint(), original_hash);
        assert_eq!(
            borrowed.validation_time(),
            Some(Duration::from_millis(5_000))
        );
        let source_wire = borrowed.entrypoint_bytes();
        assert_eq!(source_wire.as_slice(), original_wire.as_slice());
        let clone = borrowed.clone();
        assert_eq!(clone.hash_as_entrypoint(), original_hash);
        assert_eq!(clone.entrypoint(), borrowed.entrypoint());
        assert_eq!(
            norito::encode_canonical(clone.entrypoint()).unwrap(),
            source_wire.as_slice()
        );
        assert!(
            Arc::ptr_eq(&clone.entrypoint_bytes(), &source_wire),
            "the borrowed source's wire cache remains its genuine original"
        );
        assert_eq!(clone.validation_time(), borrowed.validation_time());
        assert_eq!(
            std::ptr::from_ref(clone.entrypoint()),
            source_pointer,
            "cloning a borrowed accepted source must preserve its original owner and lifetime",
        );
        assert_eq!(
            proof_pointers(retained_relay(&clone)),
            original,
            "borrowed AcceptedTransaction clone detached the genuine persisted Prepared proof allocations",
        );
        drop(borrowed);
        assert_eq!(std::ptr::from_ref(clone.entrypoint()), source_pointer);
        assert_eq!(
            proof_pointers(retained_relay(&clone)),
            original,
            "the remaining borrow keeps the original graph while its source owner remains live"
        );
        assert_eq!(clone.hash_as_entrypoint(), original_hash);
        assert_eq!(
            clone.entrypoint_bytes().as_slice(),
            original_wire.as_slice()
        );
        global_paid_images(&roots.global, 4);
        // End every borrowed wrapper before moving the exact source into owned admission.
        // No into_owned conversion of a borrow, replacement decoder or proof clone is used.
        drop(clone);
        let owned = {
            let view = roots.global.state().view();
            AcceptedTransaction::accept_entrypoint_at_time(
            entrypoint,
            &roots.global.network_id(),
            Duration::from_secs(1),
            view.world().parameters().transaction(),
            &view.crypto(),
            Duration::from_millis(5_000),
        )
        .expect(
            "the exact original paid owner passes actual owned envelope/signature/TTL admission",
        )
        };
        assert_eq!(owned.hash_as_entrypoint(), original_hash);
        assert_eq!(
            owned.entrypoint_bytes().as_slice(),
            original_wire.as_slice()
        );
        assert_eq!(proof_pointers(retained_relay(&owned)), original);
        commit_original_relay(&mut roots, owned, tx);
    }
    let thread =
        crate::sumeragi::threads::sumeragi_thread_builder("sumeragi-paid-amx-custody-test")
            .spawn(original_paid_control)
            .expect("spawn paid AMX on the configured native execution thread");
    if let Err(original) = thread.join() {
        std::panic::resume_unwind(original);
    }
}
