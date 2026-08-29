use iroha_data_model::isi::InstructionBox;
use ivm::{self, IVM, IVMHost, Memory};
use ivm_abi::codec::encode_canonical_norito;
// Exercise canonical NoritoBytes(InstructionBox) decoding in WsvHost for typed ZK ISIs.
fn sample_account() -> ivm::mock_wsv::AccountId {
    let _domain: ivm::mock_wsv::DomainId =
        iroha_data_model::DomainId::try_new("domain", "universal").expect("domain id");
    ivm::mock_wsv::AccountId::new(
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
            .parse()
            .expect("public key"),
    )
}
#[test]
fn boxed_submit_ballot_yields_permission_denied_without_verify() {
    // Seed WSV with a simple election
    let mut wsv = ivm::MockWorldStateView::new();
    assert!(wsv.create_election("e1".to_string(), 2, [0u8; 32], 0, u64::MAX));
    // Caller/account (matches other tests' format)
    let caller: ivm::mock_wsv::AccountId = sample_account();
    // Host + VM
    let host = ivm::mock_wsv::WsvHost::new_with_subject(wsv, caller.clone());
    let mut vm = IVM::new(0);
    vm.set_host(host);
    // Build and canonically encode a boxed SubmitBallot.
    let sb = iroha_data_model::isi::zk::SubmitBallot {
        election_id: "e1".to_string(),
        ciphertext: vec![1, 2, 3],
        ballot_proof: iroha_data_model::proof::ProofAttachment::new_ref(
            "halo2/ipa".into(),
            iroha_data_model::proof::ProofBox::new("halo2/ipa".into(), vec![0x01]),
            iroha_data_model::proof::VerifyingKeyId::new("halo2/ipa", "ballot_vk"),
        ),
        nullifier: [7u8; 32],
    };
    let body = encode_canonical_norito(&InstructionBox::from(sb))
        .expect("encode canonical InstructionBox");
    // Wrap into a NoritoBytes TLV (type=0x0009, ver=1)
    let mut tlv = Vec::with_capacity(7 + body.len() + 32);
    tlv.extend_from_slice(&(ivm::PointerType::NoritoBytes as u16).to_be_bytes());
    tlv.push(1);
    tlv.extend_from_slice(&(body.len() as u32).to_be_bytes());
    tlv.extend_from_slice(&body);
    let h: [u8; 32] = iroha_crypto::Hash::new(&body).into();
    tlv.extend_from_slice(&h);
    vm.memory.preload_input(0, &tlv).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.set_register(
        11,
        ivm::syscalls::SMARTCONTRACT_INSTRUCTION_TAG_SUBMIT_BALLOT,
    );
    // Without a prior verify the host should decode the boxed payload and reject the mutation.
    let res = unsafe {
        let host_ptr = vm
            .host_mut_any()
            .unwrap()
            .downcast_mut::<ivm::mock_wsv::WsvHost>()
            .unwrap() as *mut ivm::mock_wsv::WsvHost;
        (*host_ptr).syscall(
            ivm::syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION,
            &mut vm,
        )
    };
    assert!(matches!(res, Err(ivm::VMError::PermissionDenied)));
}
#[test]
fn boxed_finalize_is_rejected_for_submit_ballot_tag() {
    // Seed WSV with an election
    let mut wsv = ivm::MockWorldStateView::new();
    assert!(wsv.create_election("e2".to_string(), 3, [0u8; 32], 0, u64::MAX));
    let caller: ivm::mock_wsv::AccountId = sample_account();
    let host = ivm::mock_wsv::WsvHost::new_with_subject(wsv, caller.clone());
    let mut vm = IVM::new(0);
    vm.set_host(host);
    // Build and canonically encode a boxed FinalizeElection.
    let fin = iroha_data_model::isi::zk::FinalizeElection {
        election_id: "e2".to_string(),
        tally: vec![5, 2, 1],
        tally_proof: iroha_data_model::proof::ProofAttachment::new_ref(
            "halo2/ipa".into(),
            iroha_data_model::proof::ProofBox::new("halo2/ipa".into(), vec![0x03]),
            iroha_data_model::proof::VerifyingKeyId::new("halo2/ipa", "tally_vk"),
        ),
    };
    let body = encode_canonical_norito(&InstructionBox::from(fin))
        .expect("encode canonical InstructionBox");
    // NoritoBytes TLV
    let mut tlv = Vec::with_capacity(7 + body.len() + 32);
    tlv.extend_from_slice(&(ivm::PointerType::NoritoBytes as u16).to_be_bytes());
    tlv.push(1);
    tlv.extend_from_slice(&(body.len() as u32).to_be_bytes());
    tlv.extend_from_slice(&body);
    let h: [u8; 32] = iroha_crypto::Hash::new(&body).into();
    tlv.extend_from_slice(&h);
    vm.memory.preload_input(0, &tlv).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.set_register(
        11,
        ivm::syscalls::SMARTCONTRACT_INSTRUCTION_TAG_SUBMIT_BALLOT,
    );
    // FinalizeElection is outside the V1 0xA0 allowlist and cannot be authorized by another tag.
    let res = unsafe {
        let host_ptr = vm
            .host_mut_any()
            .unwrap()
            .downcast_mut::<ivm::mock_wsv::WsvHost>()
            .unwrap() as *mut ivm::mock_wsv::WsvHost;
        (*host_ptr).syscall(
            ivm::syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION,
            &mut vm,
        )
    };
    assert!(matches!(res, Err(ivm::VMError::PermissionDenied)));
}
