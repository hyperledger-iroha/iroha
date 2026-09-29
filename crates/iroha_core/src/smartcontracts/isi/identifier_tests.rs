// Component and refusal tests; synthetic metadata never admits encrypted execution.
use super::*;
use crate::{kura::Kura, prelude::World, query::store::LiveQueryStore,
    state::{State, StateReadOnly}};
use iroha_crypto::{Algorithm, KeyPair, PrivateKey, Signature, SignatureOf,
    PolicyCommitment, derive_phone_retail_nullifier_v1, ram_lfe_output_hash};
use iroha_data_model::{IntoKeyValue, NetworkId,
    account::{Account, OpaqueAccountId}, block::BlockHeader,
    identifier::{IdentifierPolicyId, IdentifierResolutionReceiptPayload,
        PhoneRetailCanonicalityAttestationV1, PhoneRetailCanonicalityPayloadV1},
    isi::identifier::{ActivateIdentifierPolicy, ClaimIdentifier, RegisterIdentifierPolicy, RevokeIdentifier},
    isi::ram_lfe::{ActivateRamLfeProgramPolicy, RegisterRamLfeProgramPolicy},
    nexus::UniversalAccountId, prelude::Domain,
    ram_lfe::{RamLfeOutputOpeningPayload, RamLfeProgramId}};
use iroha_model_base::{domain::DomainId, metadata::Metadata};
use nonzero_ext::nonzero;

fn test_state() -> State {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    State::new_for_testing(World::default(), kura, query)
}
fn checked_keypair() -> KeyPair {
    KeyPair::try_random().expect("identifier fixture key generation should succeed")
}
fn checked_account_id() -> AccountId {
    AccountId::new(checked_keypair().public_key().clone())
}
#[test]
fn checked_keypair_helper_preserves_default_algorithm() {
    assert_eq!(checked_keypair().algorithm(), Algorithm::default());
}
fn checked_signature_of<T: norito::codec::Encode>(
    private_key: &PrivateKey,
    payload: &T,
) -> SignatureOf<T> {
    SignatureOf::try_new(private_key, payload).expect("test fixture signing should succeed")
}
fn seed_domain(state: &mut State, domain_id: &DomainId, owner: &AccountId) {
    let domain = Domain {
        id: domain_id.clone(),
        logo: None,
        metadata: Metadata::default(),
        owned_by: owner.clone(),
    };
    state.world.domains.insert(domain_id.clone(), domain);
}
fn seed_account_with_uaid(
    state: &mut State,
    account_id: &AccountId,
    domain_id: &DomainId,
    uaid: UniversalAccountId,
) {
    let account = Account {
        id: account_id.clone(),
        metadata: Metadata::default(),
        label: None,
        uaid: Some(uaid),
        opaque_ids: Vec::new(),
    };
    let (account_id, account_value) = account.into_key_value();
    let _ = domain_id;
    state
        .world
        .accounts
        .insert(account_id.clone(), account_value);
    state.world.uaid_accounts.insert(uaid, account_id.clone());
}
fn claim_receipt(
    policy_id: &IdentifierPolicyId,
    program_policy: &RamLfeProgramPolicy,
    resolver: &KeyPair,
    uaid: UniversalAccountId,
    account_id: &AccountId,
    resolved_at_ms: u64,
    expires_at_ms: Option<u64>,
    output_seed: &[u8],
) -> IdentifierResolutionReceipt {
    let output_hash = Hash::new([b"ciphertext:".as_slice(), output_seed].concat());
    let opened_output_hash = ram_lfe_output_hash(output_seed);
    let program_id_bytes = norito::encode_canonical(&program_policy.program_id)
        .expect("encode canonical program id");
    let (opaque_id, receipt_hash) =
        identifier_hashes_from_output_hash(&program_id_bytes, &opened_output_hash);
    let execution = RamLfeExecutionReceiptPayload {
        program_id: program_policy.program_id.clone(),
        program_digest: Hash::new(b"typed-program-digest"),
        backend: program_policy.backend,
        verification_mode: program_policy.verification_mode,
        input_ciphertext_hash: Hash::new(b"input-ciphertext"),
        output_ciphertext_hash: output_hash,
        parameter_digest: Hash::new(b"typed-parameter-digest"),
        evaluation_key_digest: Hash::new(b"typed-evaluation-key-digest"),
        output_hash,
        associated_data_hash: Hash::new([]),
        executed_at_ms: resolved_at_ms,
        expires_at_ms,
    };
    let opening_payload = RamLfeOutputOpeningPayload {
        program_id: program_policy.program_id.clone(),
        input_ciphertext_hash: execution.input_ciphertext_hash,
        output_ciphertext_hash: execution.output_ciphertext_hash,
        parameter_digest: execution.parameter_digest,
        evaluation_key_digest: execution.evaluation_key_digest,
        opened_output_hash,
        opened_at_ms: resolved_at_ms.saturating_add(1),
        expires_at_ms,
    };
    let opening = RamLfeOutputOpening {
        signature: checked_signature_of(resolver.private_key(), &opening_payload).into(),
        payload: opening_payload,
    };
    let payload = IdentifierResolutionReceiptPayload {
        policy_id: policy_id.clone(),
        execution,
        opening,
        opaque_id: OpaqueAccountId::from(opaque_id),
        receipt_hash,
        uaid,
        account_id: account_id.clone(),
    };
    let signature: Signature = checked_signature_of(resolver.private_key(), &payload).into();
    IdentifierResolutionReceipt {
        payload,
        attestation: RamLfeReceiptAttestation::Signed(signature),
        phone_retail_canonicality: None,
    }
}
fn attach_phone_retail_canonicality(
    receipt: &mut IdentifierResolutionReceipt,
    attestor: &KeyPair,
    network_id: &NetworkId,
    canonical_phone: &str,
) -> Hash {
    let nullifier = derive_phone_retail_nullifier_v1(
        &[0x5a; Hash::LENGTH],
        network_id.as_bytes(),
        canonical_phone,
    )
    .expect("derive canonical phone nullifier");
    let opening = &receipt.payload.opening.payload;
    let payload = PhoneRetailCanonicalityPayloadV1 {
        network_id: *network_id,
        policy_id: receipt.payload.policy_id.clone(),
        program_id: receipt.payload.execution.program_id.clone(),
        input_ciphertext_hash: opening.input_ciphertext_hash,
        output_ciphertext_hash: opening.output_ciphertext_hash,
        opened_output_hash: opening.opened_output_hash,
        canonical_phone_nullifier: nullifier,
        uaid: receipt.payload.uaid,
        account_id: receipt.payload.account_id.clone(),
        issued_at_ms: opening.opened_at_ms,
        expires_at_ms: opening
            .expires_at_ms
            .expect("phone fixture needs an attestation expiry"),
    };
    receipt.phone_retail_canonicality = Some(PhoneRetailCanonicalityAttestationV1 {
        signature: checked_signature_of(attestor.private_key(), &payload).into(),
        payload,
    });
    nullifier
}
fn phone_retail_claim_receipt(
    policy_id: &IdentifierPolicyId,
    program_policy: &RamLfeProgramPolicy,
    resolver: &KeyPair,
    attestor: &KeyPair,
    network_id: &NetworkId,
    uaid: UniversalAccountId,
    account_id: &AccountId,
    resolved_at_ms: u64,
    expires_at_ms: u64,
    canonical_phone: &str,
) -> IdentifierResolutionReceipt {
    let mut receipt = claim_receipt(
        policy_id,
        program_policy,
        resolver,
        uaid,
        account_id,
        resolved_at_ms,
        Some(expires_at_ms),
        canonical_phone.as_bytes(),
    );
    let nullifier =
        attach_phone_retail_canonicality(&mut receipt, attestor, network_id, canonical_phone);
    let program_id_bytes = norito::encode_canonical(&program_policy.program_id)
        .expect("encode canonical program id");
    let (opaque_id, receipt_hash) =
        identifier_hashes_from_output_hash(&program_id_bytes, &nullifier);
    receipt.payload.opaque_id = OpaqueAccountId::from(opaque_id);
    receipt.payload.receipt_hash = receipt_hash;
    receipt.attestation = RamLfeReceiptAttestation::Signed(
        checked_signature_of(resolver.private_key(), &receipt.payload).into(),
    );
    receipt
}
fn phone_claim_receipt(
    policy_id: &IdentifierPolicyId,
    program_policy: &RamLfeProgramPolicy,
    resolver: &KeyPair,
    network_id: NetworkId,
    uaid: UniversalAccountId,
    account_id: &AccountId,
    canonical_phone: &str,
    ciphertext_seed: &[u8],
    output_seed: &[u8],
    resolved_at_ms: u64,
) -> IdentifierResolutionReceipt {
    let mut receipt = claim_receipt(
        policy_id,
        program_policy,
        resolver,
        uaid,
        account_id,
        resolved_at_ms,
        Some(resolved_at_ms + 60_000),
        output_seed,
    );
    let input_hash = Hash::new(ciphertext_seed);
    receipt.payload.execution.input_ciphertext_hash = input_hash;
    receipt.payload.opening.payload.input_ciphertext_hash = input_hash;
    receipt.payload.opening.signature =
        checked_signature_of(resolver.private_key(), &receipt.payload.opening.payload).into();
    let nullifier =
        attach_phone_retail_canonicality(&mut receipt, resolver, &network_id, canonical_phone);
    let program_bytes =
        norito::encode_canonical(&program_policy.program_id).expect("canonical program id");
    let (opaque, receipt_hash) = identifier_hashes_from_output_hash(&program_bytes, &nullifier);
    receipt.payload.opaque_id = OpaqueAccountId::from(opaque);
    receipt.payload.receipt_hash = receipt_hash;
    receipt.attestation = RamLfeReceiptAttestation::Signed(
        checked_signature_of(resolver.private_key(), &receipt.payload).into(),
    );
    receipt
}

fn sample_program_policy(owner: &AccountId, resolver: &KeyPair, program_id: &RamLfeProgramId) -> RamLfeProgramPolicy {
    RamLfeProgramPolicy::new(program_id.clone(), owner.clone(),
        RamLfeBackend::BfvProgrammedV1, RamLfeVerificationMode::Signed,
        PolicyCommitment { backend: RamLfeBackend::BfvProgrammedV1,
            policy_hash: Hash::new(b"typed-policy"), public_parameters: Vec::new() },
        resolver.public_key().clone())
}

fn email_policy(owner: &AccountId) -> IdentifierPolicy {
    IdentifierPolicy::new("email#retail".parse().unwrap(), owner.clone(),
        IdentifierNormalization::EmailAddress, "email_retail".parse().unwrap())
}

fn seeded_state(owner: &AccountId, uaid: UniversalAccountId) -> State {
    let mut state = test_state();
    let domain = DomainId::try_new("directory", "universal").unwrap();
    seed_account_with_uaid(&mut state, owner, &domain, uaid);
    seed_domain(&mut state, &domain, owner);
    state
}
