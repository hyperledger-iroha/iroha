use ivm::{self, IVM, IVMHost, Memory};

// Exercise the NoritoBytes fallback decode in WsvHost for typed ZK ISIs
// (SubmitBallot/FinalizeElection) when an InstructionBox wrapper is not used.

fn sample_account() -> ivm::mock_wsv::AccountId {
    let _domain: ivm::mock_wsv::DomainId = "domain".parse().expect("domain id");
    ivm::mock_wsv::AccountId::new(
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
            .parse()
            .expect("public key"),
    )
}

#[test]
fn decode_typed_submitballot_fallback_yields_permission_denied_without_verify() {
    use std::collections::HashMap;

    use iroha_data_model::isi::BuiltInInstruction as _;

    // Seed WSV with a simple election
    let mut wsv = ivm::MockWorldStateView::new();
    assert!(wsv.create_election("e1".to_string(), 2, [0u8; 32], 0, u64::MAX));

    // Caller/account (matches other tests' format)
    let caller: ivm::mock_wsv::AccountId = sample_account();

    // Host + VM
    let host = ivm::mock_wsv::WsvHost::new_with_subject(wsv, caller.clone(), HashMap::new());
    let mut vm = IVM::new(0);
    vm.set_host(host);

    // Build a typed SubmitBallot and encode it directly (no InstructionBox envelope)
    let sb = iroha_data_model::isi::zk::SubmitBallot {
        election_id: "e1".to_string(),
        ciphertext: vec![1, 2, 3],
        ballot_proof: iroha_data_model::proof::ProofAttachment::new_inline(
            "halo2/ipa".into(),
            iroha_data_model::proof::ProofBox::new("halo2/ipa".into(), vec![0x01]),
            iroha_data_model::proof::VerifyingKeyBox::new("halo2/ipa".into(), vec![0x02]),
        ),
        nullifier: [7u8; 32],
    };
    let body = sb.encode_as_instruction_box();

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

    // Execute via vendor execute-instruction syscall; without a prior verify the host should
    // decode the typed payload via the fallback and reject with PermissionDenied.
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
fn decode_typed_finalize_fallback_yields_permission_denied_without_verify() {
    use std::collections::HashMap;

    use iroha_data_model::isi::BuiltInInstruction as _;

    // Seed WSV with an election
    let mut wsv = ivm::MockWorldStateView::new();
    assert!(wsv.create_election("e2".to_string(), 3, [0u8; 32], 0, u64::MAX));

    let caller: ivm::mock_wsv::AccountId = sample_account();
    let host = ivm::mock_wsv::WsvHost::new_with_subject(wsv, caller.clone(), HashMap::new());
    let mut vm = IVM::new(0);
    vm.set_host(host);

    // Build typed FinalizeElection directly
    let fin = iroha_data_model::isi::zk::FinalizeElection {
        election_id: "e2".to_string(),
        tally: vec![5, 2, 1],
        tally_proof: iroha_data_model::proof::ProofAttachment::new_inline(
            "halo2/ipa".into(),
            iroha_data_model::proof::ProofBox::new("halo2/ipa".into(), vec![0x03]),
            iroha_data_model::proof::VerifyingKeyBox::new("halo2/ipa".into(), vec![0x04]),
        ),
    };
    let body = fin.encode_as_instruction_box();

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

    // Without verify_tally latch, expect PermissionDenied
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
