//! Constructor constness, field preservation, wire bytes and instruction conversions.

use super::*;
use iroha_crypto::{Algorithm, Hash};

fn account(seed: u8) -> AccountId {
    AccountId::of(
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("fixture key")
            .public_key()
            .clone(),
    )
}

fn instructions() -> Vec<InstructionBox> {
    vec![CustomInstruction::new(json::Value::from("constructor fixture")).into()]
}

fn proposal() -> MultisigProposalValue {
    let network_id =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([0x49; 32])));
    let proposal_nonce = MultisigProposalNonce::new([0x71; 32]).expect("nonzero nonce");
    MultisigProposalValue {
        network_id,
        proposal_nonce,
        proposal_id: MultisigProposalId::derive(&network_id, &account(1), &proposal_nonce),
        transaction_ttl_ms: None,
        instructions: instructions(),
        proposed_at_ms: 11,
        expires_at_ms: 29,
        approvals: BTreeSet::from([account(2)]),
    }
}

fn assert_same<T>(actual: T, expected: T)
where
    T: std::fmt::Debug
        + PartialEq
        + norito::NoritoSerialize
        + for<'de> norito::NoritoDeserialize<'de>,
{
    assert_eq!(actual, expected);
    let bytes = norito::to_bytes(&actual).expect("encode constructor result");
    assert_eq!(
        bytes,
        norito::to_bytes(&expected).expect("encode named fields")
    );
    let decoded: T = norito::decode_from_bytes(&bytes).expect("decode constructor result");
    assert_eq!(decoded, expected);
}

macro_rules! assert_constructor {
    ($ty:ident { $($field:ident: $field_ty:ty = $value:expr),+ $(,)? }) => {{
        // Forwarding each owned field must remain valid in a const function.
        const fn construct($($field: $field_ty),+) -> $ty {
            $ty::new($($field),+)
        }
        let expected = $ty { $($field: $value),+ };
        let actual = construct($(expected.$field.clone()),+);
        assert_same(actual, expected);
    }};
}

#[test]
fn instruction_constructors_preserve_fields_and_const_api() {
    let proposal = proposal();
    let instructions_hash = HashOf::new(&proposal.instructions);
    for ttl in [None, NonZeroU64::new(u64::MAX)] {
        assert_constructor!(MultisigPropose {
            account: AccountId = account(1),
            proposal_nonce: MultisigProposalNonce = proposal.proposal_nonce,
            instructions: Vec<InstructionBox> = proposal.instructions.clone(),
            transaction_ttl_ms: Option<NonZeroU64> = ttl,
        });
    }
    assert_constructor!(MultisigApprove {
        account: AccountId = account(2),
        proposal_id: MultisigProposalId = proposal.proposal_id,
        instructions_hash: HashOf<Vec<InstructionBox>> = instructions_hash,
    });
    assert_constructor!(MultisigCancel {
        account: AccountId = account(3),
        proposal_id: MultisigProposalId = proposal.proposal_id,
        instructions_hash: HashOf<Vec<InstructionBox>> = instructions_hash,
    });
    assert_constructor!(MultisigInvalidateOutstanding {
        account: AccountId = account(4),
    });
}

#[test]
fn state_constructors_preserve_fields_and_const_api() {
    let proposal = proposal();
    let instructions_hash = HashOf::new(&proposal.instructions);
    let terminal = MultisigProposalTerminalState {
        multisig_account_id: account(1),
        proposal_id: proposal.proposal_id,
        instructions_hash,
        proposal: proposal.clone(),
        status: MultisigProposalTerminalStatus::Canceled,
        terminal_at_ms: u64::MAX,
    };
    assert_constructor!(MultisigProposalTerminalState {
        multisig_account_id: AccountId = terminal.multisig_account_id.clone(),
        proposal_id: MultisigProposalId = terminal.proposal_id,
        instructions_hash: HashOf<Vec<InstructionBox>> = terminal.instructions_hash,
        proposal: MultisigProposalValue = terminal.proposal.clone(),
        status: MultisigProposalTerminalStatus = terminal.status,
        terminal_at_ms: u64 = terminal.terminal_at_ms,
    });
    assert_constructor!(MultisigProposalTerminalExecutionStateV1 {
        terminal: MultisigProposalTerminalState = terminal,
        entrypoint_account_id: AccountId = account(2),
        terminal_block_height: u64 = 17,
        terminal_entrypoint_hash: [u8; 32] = [0x32; 32],
    });
    assert_constructor!(MultisigApprovalOutcomeV1 {
        entrypoint_account_id: AccountId = account(2),
        resolved_multisig_account_id: AccountId = account(3),
        proposal_id: MultisigProposalId = proposal.proposal_id,
        instructions_hash: HashOf<Vec<InstructionBox>> = instructions_hash,
        status: MultisigApprovalOutcomeStatusV1 = MultisigApprovalOutcomeStatusV1::NotExecuted,
        block_height: u64 = 23,
        entrypoint_hash: [u8; 32] = [0x64; 32],
    });
}

#[test]
fn spec_constructor_preserves_weights_and_does_not_validate_quorum() {
    assert_constructor!(MultisigSpec {
        signatories: BTreeMap<AccountId, u8> = BTreeMap::from([(account(1), 0), (account(2), u8::MAX)]),
        quorum: NonZeroU16 = NonZeroU16::new(u16::MAX).expect("nonzero quorum"),
        transaction_ttl_ms: NonZeroU64 = NonZeroU64::new(u64::MAX).expect("nonzero TTL"),
    });
}

fn assert_conversion<T>(value: T, expected: MultisigInstructionBox)
where
    T: Clone + Into<MultisigInstructionBox> + Into<InstructionBox>,
{
    let actual: MultisigInstructionBox = value.clone().into();
    assert_same(actual.clone(), expected.clone());
    assert_eq!(
        json::to_json(&actual).expect("serialize converted instruction"),
        json::to_json(&expected).expect("serialize named variant"),
    );
    let instruction: InstructionBox = value.into();
    assert_eq!(
        MultisigInstructionBox::try_from(&instruction).expect("decode custom instruction"),
        expected,
    );
}

#[test]
fn instruction_conversions_preserve_all_five_variants_and_custom_payloads() {
    let proposal = proposal();
    let instructions_hash = HashOf::new(&proposal.instructions);
    let register = MultisigRegister::new(
        account(1),
        None,
        MultisigSpec::new(
            BTreeMap::from([(account(2), 1)]),
            NonZeroU16::new(1).expect("nonzero quorum"),
            NonZeroU64::new(100).expect("nonzero TTL"),
        ),
    );
    assert_conversion(register.clone(), MultisigInstructionBox::Register(register));
    let propose = MultisigPropose::new(
        account(1),
        proposal.proposal_nonce,
        proposal.instructions,
        None,
    );
    assert_conversion(propose.clone(), MultisigInstructionBox::Propose(propose));
    let approve = MultisigApprove::new(account(2), proposal.proposal_id, instructions_hash);
    assert_conversion(approve.clone(), MultisigInstructionBox::Approve(approve));
    let cancel = MultisigCancel::new(account(3), proposal.proposal_id, instructions_hash);
    assert_conversion(cancel.clone(), MultisigInstructionBox::Cancel(cancel));
    let invalidate = MultisigInvalidateOutstanding::new(account(4));
    assert_conversion(
        invalidate.clone(),
        MultisigInstructionBox::InvalidateOutstanding(invalidate),
    );
}
