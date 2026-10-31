//! Genuine maximum payment originals shared by Core and downstream HTTP-owner tests.
//!
//! This module exists only with the enclosing certified test-chain feature. It uses
//! original signed genesis, ordinary paid execution, mandatory availability and exact
//! committee certificates; it supplies no synthetic receipt or finality authority.

use super::{CertifiedTestChain, Signers, TestChainConfig};
use crate::state::{World, WorldReadOnly};
use iroha_data_model::{
    account::{Account, AccountId},
    asset::{AssetBalancePolicy, AssetDefinition, AssetDefinitionId, AssetId},
    block::{SignedBlock, consensus::NexusFeeSettlementV1},
    domain::Domain,
    isi::{
        AtomicSettlementEscrows, AtomicSettlementMovement, AtomicSettlementMovements, Mint,
        Register, SettleAtomic, SettlementId,
    },
    nexus::FeeDebitSource,
    parameter::{Parameter, Parameters},
    transaction::{
        FeeChargeKind, FeeChargeLimit, FeePaymentIntent, SignedTransaction, TransactionBuilder,
    },
};
use iroha_model_base::{domain::DomainId, metadata::Metadata};
use iroha_primitives::json::Json;
use iroha_sumeragi::types::ControlWitness;
use std::{num::NonZeroU64, time::Duration};

/// Real execution shape selected by the signed original and its funded genesis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaymentKind {
    /// Unfunded original using the chain's ordinary rejected execution path.
    Rejected,
    /// Funded principal transfer with an explicit one-unit native fee.
    Applied,
}

/// Deterministic principal asset under the fixture domain.
pub fn asset(name: &str) -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("nativeproof", "universal").unwrap(),
        name.parse().unwrap(),
    )
}

/// Registered universal recipient of the funded originals.
pub fn recipient() -> AccountId {
    AccountId::new(
        iroha_crypto::KeyPair::from_seed(vec![0xD1; 32], iroha_crypto::Algorithm::Ed25519)
            .public_key()
            .clone(),
    )
}

/// Exact native fee asset named by the fixture policy.
pub fn fee_asset() -> AssetDefinitionId {
    AssetDefinitionId::parse_address_literal(
        &iroha_config::parameters::defaults::nexus::fees::fee_asset_id(),
    )
    .unwrap()
}

fn signed_payment(
    chain: &CertifiedTestChain,
    signer: &iroha_crypto::KeyPair,
    reference: &str,
    created_ms: u64,
    text_bytes: usize,
    kind: PaymentKind,
) -> SignedTransaction {
    let payer = chain.genesis_account().clone();
    let mut metadata = Metadata::default();
    let mut remaining = text_bytes;
    let mut index = 0;
    while remaining != 0 {
        let count = remaining.min(iroha_primitives::json::MAX_JSON_BYTES - 2);
        metadata.insert(
            format!("reconciliation_text_{index:02}").parse().unwrap(),
            Json::try_new("r".repeat(count)).unwrap(),
        );
        remaining -= count;
        index += 1;
    }
    let payment = SettleAtomic::new(
        chain.network_id(),
        SettlementId::new(payer.clone(), reference.parse().unwrap()),
        AtomicSettlementMovements::try_from(vec![AtomicSettlementMovement {
            source: AssetId::of(asset("cash"), payer.clone()),
            recipient: recipient(),
            quantity: 1_u32.into(),
        }])
        .unwrap(),
        AtomicSettlementEscrows::empty(),
        NonZeroU64::new(101).unwrap(),
        metadata,
    );
    match kind {
        PaymentKind::Rejected => chain.sign(signer, [payment.into()], created_ms),
        PaymentKind::Applied => {
            let mut builder = TransactionBuilder::new(
                chain.network_id(),
                payer,
                FeePaymentIntent::authority(
                    vec![FeeChargeLimit::new(
                        FeeChargeKind::Nexus,
                        fee_asset(),
                        1_u32.into(),
                    )],
                    None,
                ),
            );
            builder.set_creation_time(Duration::from_millis(created_ms));
            builder
                .with_instructions([payment])
                .sign(signer.private_key())
        }
    }
}

#[inline(never)]
/// Start a genuine signed genesis, optionally funding principal and native fees.
pub fn payment_chain(
    kind: PaymentKind,
    principal: u32,
) -> (CertifiedTestChain, iroha_crypto::KeyPair) {
    let mut config = TestChainConfig::new(World::new(), 10_000);
    if kind == PaymentKind::Applied {
        let payer = AccountId::new(config.genesis_key.public_key().clone());
        let mut fees = iroha_config::parameters::defaults::nexus::fees::genesis_policy();
        fees.fee_asset_id = fee_asset();
        fees.fee_sink_account_id = recipient();
        fees.base_fee = 1_u32.into();
        fees.per_byte_fee = 0_u32.into();
        fees.per_instruction_fee = 0_u32.into();
        fees.per_gas_unit_fee = 0_u32.into();
        config
            .genesis_parameters
            .push(Parameter::Custom(fees.into_custom_parameter().unwrap()));
        config.genesis_instructions.extend([
            Register::account(Account::new(recipient())).into(),
            Register::domain(Domain::new(
                DomainId::try_new("nativeproof", "universal").unwrap(),
            ))
            .into(),
            Register::asset_definition(AssetDefinition::numeric(
                asset("cash"),
                "Maximum payment principal",
                AssetBalancePolicy::Global,
                None,
            ))
            .into(),
            Mint::asset_quantity(principal, AssetId::of(asset("cash"), payer.clone())).into(),
            Register::asset_definition(AssetDefinition::numeric(
                fee_asset(),
                "Maximum payment fee",
                AssetBalancePolicy::Global,
                None,
            ))
            .into(),
            Mint::asset_quantity(10_u32, AssetId::of(fee_asset(), payer)).into(),
        ]);
    }
    let signer = config.genesis_key.clone();
    (CertifiedTestChain::start(config).unwrap(), signer)
}

#[inline(never)]
/// Commit two exact maximum signed payment originals at H2 and H3.
pub fn maximum_payment_chain(kind: PaymentKind) -> CertifiedTestChain {
    let (mut chain, signer) = payment_chain(kind, 2);
    let maximum =
        usize::try_from(Parameters::default().transaction().max_tx_bytes().get()).unwrap();
    assert_eq!(maximum, 10 * 1024 * 1024);
    for (reference, created_ms) in [("maximum_first", 10_100), ("maximum_second", 10_200)] {
        let initial_text = maximum - 16 * 1024;
        let probe = signed_payment(&chain, &signer, reference, created_ms, initial_text, kind);
        let measured = norito::canonical_frame_len(&probe).unwrap();
        let text_bytes = initial_text + maximum.checked_sub(measured).unwrap();
        drop(probe);
        let payment = signed_payment(&chain, &signer, reference, created_ms, text_bytes, kind);
        assert_eq!(norito::canonical_frame_len(&payment).unwrap(), maximum);
        payment.verify_signature().unwrap();
        // Both real execution outcomes traverse SignedBlockWire, genuine quorum
        // certification, Kura and native finality. The applied shape additionally
        // retains the complete reconciliation text in its immutable receipt.
        assert_eq!(chain.commit(vec![payment]), [kind == PaymentKind::Applied]);
        let view = chain.state().view();
        let receipt = view.world().settlement_receipts.get(&SettlementId::new(
            chain.genesis_account().clone(),
            reference.parse().unwrap(),
        ));
        assert_eq!(receipt.is_some(), kind == PaymentKind::Applied);
        if let Some(receipt) = receipt {
            assert_eq!(
                receipt.metadata.iter().len(),
                text_bytes.div_ceil(iroha_primitives::json::MAX_JSON_BYTES - 2)
            );
            assert!(norito::canonical_frame_len(receipt).unwrap() >= maximum - 16 * 1024);
        }
    }
    if kind == PaymentKind::Applied {
        let view = chain.state().view();
        assert_eq!(
            view.world()
                .assets
                .get(&AssetId::of(asset("cash"), recipient()))
                .unwrap()
                .0,
            2_u32.into(),
            "both original principal effects execute"
        );
        assert_eq!(
            view.world()
                .assets
                .get(&AssetId::of(fee_asset(), chain.genesis_account().clone()))
                .unwrap()
                .0,
            8_u32.into(),
            "the original payer pays both one-unit fees"
        );
        assert_eq!(
            view.world()
                .asset_definitions
                .get(&fee_asset())
                .unwrap()
                .total_quantity(),
            &8_u32.into(),
            "direct Nexus fee settlement burns the two paid units"
        );
        assert!(
            view.world()
                .assets
                .get(&AssetId::of(fee_asset(), recipient()))
                .is_none(),
            "the committed fee sink receives no fee credit"
        );
        eprintln!(
            "maximum successful payment original source policy: {:?}",
            view.world().parameters().block().fastpq_source()
        );
        for height in [2, 3] {
            let committed = chain.committed(height);
            let block = committed.block();
            let input = block.network_entrypoint_at(0).unwrap();
            let result = &block.network_output_at(0).unwrap().1.result;
            assert!(result.is_ok());
            let fee = result
                .nexus_fee_receipt()
                .expect("original paid native output");
            fee.validate_for_network_input(input, height).unwrap();
            assert_eq!(fee.fee_asset_id, fee_asset());
            assert_eq!(fee.fee_amount, 1_u32.into());
            assert_eq!(
                fee.debit_source,
                FeeDebitSource::Account(chain.genesis_account().clone())
            );
            assert_eq!(fee.settlement, NexusFeeSettlementV1::Burn);
            let output_bytes: Vec<_> = committed
                .block()
                .execution_outputs()
                .iter()
                .map(|output| norito::canonical_frame_len(output).unwrap())
                .collect();
            eprintln!(
                "maximum successful payment H{height} actual output frame bytes: {output_bytes:?}"
            );
        }
    }
    chain
}

/// Exact pair of independently signed originals and their canonical proposal.
pub struct PaymentBlock {
    /// Unexecuted proposal produced by the original chain's payload builder.
    pub proposal: SignedBlock,
    /// Reviewed signed payment carriers in their original order.
    pub originals: [SignedTransaction; 2],
    /// Reconciliation-text extent of the second original, used for first-over checks.
    pub second_text_bytes: usize,
}

fn sized_first(chain: &CertifiedTestChain, signer: &iroha_crypto::KeyPair) -> SignedTransaction {
    let maximum = usize::try_from(
        chain
            .state()
            .view()
            .world()
            .parameters()
            .transaction()
            .max_tx_bytes()
            .get(),
    )
    .unwrap();
    assert_eq!(maximum, 10 * 1024 * 1024);
    let initial = maximum - 16 * 1024;
    let reference = format!("block_{}_first", chain.height() + 1);
    let created_ms = 10_000 + (chain.height() + 1) * 100;
    let probe = signed_payment(
        chain,
        signer,
        &reference,
        created_ms,
        initial,
        PaymentKind::Applied,
    );
    let text = initial
        + maximum
            .checked_sub(norito::canonical_frame_len(&probe).unwrap())
            .unwrap();
    drop(probe);
    let original = signed_payment(
        chain,
        signer,
        &reference,
        created_ms,
        text,
        PaymentKind::Applied,
    );
    assert_eq!(norito::canonical_frame_len(&original).unwrap(), maximum);
    original
}

/// Assemble two native payment originals without executing or signing a block.
pub fn assemble(
    chain: &CertifiedTestChain,
    signer: &iroha_crypto::KeyPair,
    first: &SignedTransaction,
    second_text_bytes: usize,
) -> PaymentBlock {
    let second = signed_payment(
        chain,
        signer,
        &format!("block_{}_second", chain.height() + 1),
        10_001 + (chain.height() + 1) * 100,
        second_text_bytes,
        PaymentKind::Applied,
    );
    let maximum = usize::try_from(
        chain
            .state()
            .view()
            .world()
            .parameters()
            .transaction()
            .max_tx_bytes()
            .get(),
    )
    .unwrap();
    for original in [first, &second] {
        assert!(norito::canonical_frame_len(original).unwrap() <= maximum);
        original.verify_signature().unwrap();
    }
    let proposal = chain.proposal(None, vec![first.clone(), second.clone()]);
    assert_eq!(proposal.network_entrypoint_count(), 2);
    PaymentBlock {
        proposal,
        originals: [first.clone(), second],
        second_text_bytes,
    }
}

/// Build an exact committed 16 MiB proposal from two independently signed payments.
pub fn exact_block(chain: &CertifiedTestChain, signer: &iroha_crypto::KeyPair) -> PaymentBlock {
    let maximum = chain
        .state()
        .view()
        .world()
        .parameters()
        .sumeragi()
        .max_block_bytes
        .get() as usize;
    assert_eq!(maximum, 16 * 1024 * 1024);
    let first = sized_first(chain, signer);
    let mut text = maximum - 10 * 1024 * 1024 - 32 * 1024;
    // Correct the actual canonical proposal extent, including every nested
    // length prefix and original execution context. No proof/body padding exists.
    for _ in 0..8 {
        let candidate = assemble(chain, signer, &first, text);
        let actual = candidate.proposal.resultless_proposal_wire_len().unwrap();
        if actual == maximum {
            return candidate;
        }
        text = if actual < maximum {
            text.checked_add(maximum - actual).unwrap()
        } else {
            text.checked_sub(actual - maximum).unwrap()
        };
    }
    panic!("canonical two-payment proposal must attain the exact committed ceiling");
}

/// Execute two complete maximum proposals with four original principal and fee effects.
/// The returned originals are caller evidence to compare against independently verified proofs.
pub fn full_payment_chain() -> (CertifiedTestChain, [[SignedTransaction; 2]; 2]) {
    let (mut chain, signer) = payment_chain(PaymentKind::Applied, 4);
    let originals = std::array::from_fn(|_| {
        let block = exact_block(&chain, &signer);
        chain.commit_proposal(block.proposal, Signers::Quorum, ControlWitness::default());
        block.originals
    });
    assert_eq!(
        chain.height(),
        3,
        "no unrelated successor after the last payment"
    );
    (chain, originals)
}
