//! Overlay and height-expiry bounds under committed transaction policy.

//! Ensures validation rejects transactions whose overlays exceed the committed
//! `overlay_max_instructions` or `overlay_max_bytes` of the pipeline validity
//! policy, or whose height expiry has been reached, while other transactions in
//! the same certified block still apply (non-forking semantics).
use iroha_core::sumeragi::test_chain::{CertifiedTestChain, Signers, TestChainConfig};
use iroha_data_model::{pipeline_validity_policy::PipelineValidityPolicyV1, prelude::*};
use iroha_model_base::chain::ChainId;
use iroha_model_base::domain::DomainId;
use norito::codec::Encode as NoritoEncode;
use std::str::FromStr;

/// Configure a certified test chain with one ordinary transaction authority.
/// A block of ordinary transactions follows an authenticated genesis, and the block
/// is produced and certified by the chain fixture.
fn min_world_chain_config() -> (TestChainConfig, AccountId, iroha_crypto::KeyPair) {
    let chain_id: ChainId = "chain".parse().unwrap();
    let (authority_id, kp) = iroha_test_samples::gen_account_in("wonderland");
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let domain: Domain = Domain::new(domain_id).build(&authority_id);
    let account = Account::new(authority_id.clone()).build(&authority_id);
    let world = iroha_core::state::World::with([domain], [account], []);
    let mut config = TestChainConfig::new(world, 0);
    config.chain_id = chain_id;
    (config, authority_id, kp)
}
/// Commit the overlay limits in the signed genesis of the chain fixture.
///
/// The overlay limits are chain policy: their source is the committed pipeline validity
/// policy, never a node's own settings. A policy the genesis commits governs every block
/// after genesis, so the genesis overlay itself is not measured against it.
fn commit_overlay_limits(config: &mut TestChainConfig, max_instructions: usize, max_bytes: u64) {
    let mut policy = PipelineValidityPolicyV1::protocol_default();
    policy.overlay_max_instructions =
        u64::try_from(max_instructions).expect("instruction cap fits u64");
    policy.overlay_max_bytes = max_bytes;
    policy
        .validate()
        .expect("the overlay limits of the fixture are a valid policy");
    config
        .genesis_parameters
        .push(Parameter::Custom(policy.into_custom_parameter()));
}
#[test]
fn overlay_instruction_cap_rejects_and_rest_apply() {
    // The committed cap: one overlay has exactly that many instructions and passes, the
    // other has one more and is rejected. Zero bytes means no byte limit.
    const INSTRUCTION_CAP: usize = 64;
    let (mut config, authority_id, kp) = min_world_chain_config();
    commit_overlay_limits(&mut config, INSTRUCTION_CAP, 0);
    let mut chain =
        Box::new(CertifiedTestChain::start(config).expect("authenticated overlay-bounds genesis"));
    let network_id = chain.network_id();
    // tx_a: one instruction more than the cap → should be rejected by instruction cap
    let tx_a = TransactionBuilder::new(
        network_id,
        authority_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions(
        (0..=INSTRUCTION_CAP).map(|index| Log::new(Level::INFO, format!("a{index}"))),
    )
    .sign(kp.private_key());
    // tx_b: exactly the cap → should be approved
    let tx_b = TransactionBuilder::new(
        network_id,
        authority_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions((0..INSTRUCTION_CAP).map(|index| Log::new(Level::INFO, format!("b{index}"))))
    .sign(kp.private_key());
    // The same original inputs pass native payload execution and three-of-four
    // certification. The fixture does not author transaction results.
    let proposal = chain.proposal(None, vec![tx_a, tx_b]);
    let committed = chain.commit_proposal(
        proposal,
        Signers::Quorum,
        iroha_sumeragi::types::ControlWitness::default(),
    );
    // Expect first tx rejected with NotPermitted("overlay exceeds max instructions: ...") and second approved
    let block = committed.block();
    assert_eq!(block.external_transactions().count(), 2);
    let errors = [0, 1].map(|index| {
        block
            .network_output_at(index)
            .expect("validated transaction has an output")
            .1
            .result
            .as_ref()
            .err()
    });
    let rejections = errors
        .iter()
        .filter(|e| {
            matches!(
                e,
                Some(
                    iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                        iroha_data_model::ValidationFail::NotPermitted(msg)
                    )
                ) if msg.contains("overlay exceeds max instructions")
            )
        })
        .count();
    assert_eq!(
        rejections, 1,
        "the overlay of one instruction above the cap is rejected, got {errors:?}"
    );
    assert_eq!(
        errors.iter().filter(|error| error.is_none()).count(),
        1,
        "the overlay of exactly the cap is approved, got {errors:?}"
    );
}
#[test]
fn overlay_bytes_cap_rejects_and_rest_apply() {
    // The committed cap: one overlay has exactly that many bytes and passes; the other has
    // one byte more and is rejected.
    const BYTE_CAP: u64 = 256 * 1024;
    let (mut config, authority_id, kp) = min_world_chain_config();
    let log_of = |message_bytes: usize| -> InstructionBox {
        Log::new(Level::INFO, "x".repeat(message_bytes)).into()
    };
    let encoded = |instruction: &InstructionBox| NoritoEncode::encode(instruction).len() as u64;
    // The encoding of a log grows by one byte per message byte at this size: derive the
    // message length whose instruction encodes to exactly the cap.
    let probe = usize::try_from(BYTE_CAP / 2).expect("probe length fits usize");
    let framing = encoded(&log_of(probe)) - probe as u64;
    let exact_message = usize::try_from(BYTE_CAP - framing).expect("message length fits usize");
    let small_instr = log_of(exact_message);
    let small_bytes = encoded(&small_instr);
    let big_instr = log_of(exact_message + 1);
    let big_bytes = encoded(&big_instr);
    assert_eq!(
        (small_bytes, big_bytes),
        (BYTE_CAP, BYTE_CAP + 1),
        "the fixture overlays have exactly the cap and one byte more"
    );
    // Zero instructions means no instruction limit.
    commit_overlay_limits(&mut config, 0, BYTE_CAP);
    let mut chain =
        Box::new(CertifiedTestChain::start(config).expect("authenticated overlay-bounds genesis"));
    let network_id = chain.network_id();
    let tx_big = TransactionBuilder::new(
        network_id,
        authority_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([big_instr])
    .sign(kp.private_key());
    // tx_small: a Log of exactly the byte cap
    let tx_small = TransactionBuilder::new(
        network_id,
        authority_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([small_instr])
    .sign(kp.private_key());
    let proposal = chain.proposal(None, vec![tx_big, tx_small]);
    let committed = chain.commit_proposal(
        proposal,
        Signers::Quorum,
        iroha_sumeragi::types::ControlWitness::default(),
    );
    // Expect first tx rejected with NotPermitted("overlay exceeds max bytes: ...") and second approved
    let block = committed.block();
    assert_eq!(block.external_transactions().count(), 2);
    let errors = [0, 1].map(|index| {
        block
            .network_output_at(index)
            .expect("validated transaction has an output")
            .1
            .result
            .as_ref()
            .err()
    });
    // Encoding stops at the committed bound, so the canonical diagnostic names the bound
    // and not the unmeasured full length (`EffectFault::Bytes`).
    let expected_rejection = format!("overlay exceeds max bytes: more than {BYTE_CAP}");
    let overlay_errs = errors
        .iter()
        .filter(|e| {
            matches!(
                e,
                Some(
                    iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                        iroha_data_model::ValidationFail::NotPermitted(msg)
                    )
                ) if msg == &expected_rejection
            )
        })
        .count();
    assert_eq!(
        overlay_errs, 1,
        "the overlay of one byte above the cap is rejected, got {errors:?}"
    );
    assert_eq!(
        errors.iter().filter(|error| error.is_none()).count(),
        1,
        "the overlay of exactly the cap is approved, got {errors:?}"
    );
}
#[test]
fn height_expiry_rejects_at_the_boundary_and_other_transactions_apply() {
    const BLOCK_HEIGHT: u64 = 2;
    let (mut config, authority_id, kp) = min_world_chain_config();
    config.genesis_parameters.push(Parameter::Transaction(
        iroha_data_model::parameter::system::TransactionParameter::RequireHeightTtl(true),
    ));
    let mut chain =
        Box::new(CertifiedTestChain::start(config).expect("authenticated height-expiry genesis"));
    assert_eq!(chain.height(), BLOCK_HEIGHT - 1);
    let network_id = chain.network_id();
    let transaction_expiring_at = |expiry| {
        let mut metadata = iroha_model_base::metadata::Metadata::default();
        metadata.insert(
            iroha_model_base::name::Name::from_str("expires_at_height").unwrap(),
            iroha_primitives::json::Json::new(expiry),
        );
        TransactionBuilder::new(
            network_id,
            authority_id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, format!("expires at {expiry}"))])
        .with_metadata(metadata)
        .sign(kp.private_key())
    };
    // Equality is expired; an expiry one height later still permits execution. Both
    // transactions satisfy the signed policy's stateless requirement for a height TTL.
    let proposal = chain.proposal(
        None,
        vec![
            transaction_expiring_at(BLOCK_HEIGHT),
            transaction_expiring_at(BLOCK_HEIGHT + 1),
        ],
    );
    let committed = chain.commit_proposal(
        proposal,
        Signers::Quorum,
        iroha_sumeragi::types::ControlWitness::default(),
    );
    let block = committed.block();
    assert_eq!(block.header().height().get(), BLOCK_HEIGHT);
    assert_eq!(block.external_transactions().count(), 2);
    let errors = [0, 1].map(|index| {
        block
            .network_output_at(index)
            .expect("validated transaction has an output")
            .1
            .result
            .as_ref()
            .err()
    });
    let expected_rejection =
        format!("Transaction expired at height {BLOCK_HEIGHT}; current height is {BLOCK_HEIGHT}");
    assert!(
        matches!(
            errors[0],
            Some(
                iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                    iroha_data_model::ValidationFail::NotPermitted(message)
                )
            ) if message == &expected_rejection
        ),
        "the transaction expiring at the block height must be rejected, got {errors:?}"
    );
    assert!(
        errors[1].is_none(),
        "the transaction expiring one height later must be approved, got {errors:?}"
    );
}
