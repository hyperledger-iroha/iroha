//! Demonstrates a conditional asset transfer using canonical pointer-ABI
//! account, asset-definition, quantity, and dataspace TLVs.
use std::{
    any::Any,
    collections::HashMap,
    sync::{Arc, Mutex},
};

use iroha_data_model::{
    account::AccountId, asset::AssetDefinitionId, domain::DomainId, nexus::DataSpaceId,
};
use iroha_primitives::{numeric::Quantity, numeric_abi::QuantityValueV1};
use ivm::{
    IVM, PointerType, VMError, encoding, host::IVMHost, instruction, kotodama::wide as kwide,
    syscalls,
};

#[derive(Clone)]
struct AssetHost {
    state: Arc<Mutex<HashMap<(AccountId, AssetDefinitionId), u64>>>,
}

impl AssetHost {
    fn new(initial_balances: &[(AccountId, AssetDefinitionId, u64)]) -> Self {
        let mut map = HashMap::new();
        for (account, asset, balance) in initial_balances {
            map.insert((account.clone(), asset.clone()), *balance);
        }
        AssetHost {
            state: Arc::new(Mutex::new(map)),
        }
    }

    fn balance(&self, account: &AccountId, asset: &AssetDefinitionId) -> u64 {
        let state = self.state.lock().expect("state mutex poisoned");
        *state.get(&(account.clone(), asset.clone())).unwrap_or(&0)
    }

    fn decode_account(vm: &IVM, register: usize) -> Result<AccountId, VMError> {
        let tlv = vm.validate_tlv(vm.register(register))?;
        if tlv.type_id != PointerType::AccountId {
            return Err(VMError::NoritoInvalid);
        }
        norito::decode_from_bytes(tlv.payload).map_err(|_| VMError::DecodeError)
    }

    fn decode_asset(vm: &IVM) -> Result<AssetDefinitionId, VMError> {
        let tlv = vm.validate_tlv(vm.register(12))?;
        if tlv.type_id != PointerType::AssetDefinitionId {
            return Err(VMError::NoritoInvalid);
        }
        norito::decode_from_bytes(tlv.payload).map_err(|_| VMError::DecodeError)
    }

    fn decode_amount(vm: &IVM) -> Result<u64, VMError> {
        let tlv = vm.validate_tlv(vm.register(13))?;
        if tlv.type_id != PointerType::Quantity {
            return Err(VMError::NoritoInvalid);
        }
        let quantity = QuantityValueV1::decode_frame(tlv.payload)
            .map_err(|_| VMError::DecodeError)?
            .into_quantity();
        u64::try_from(quantity.into_numeric()).map_err(|_| VMError::DecodeError)
    }

    fn decode_dataspace(vm: &IVM) -> Result<DataSpaceId, VMError> {
        let tlv = vm.validate_tlv(vm.register(14))?;
        if tlv.type_id != PointerType::DataSpaceId {
            return Err(VMError::NoritoInvalid);
        }
        norito::decode_from_bytes(tlv.payload).map_err(|_| VMError::DecodeError)
    }
}

impl IVMHost for AssetHost {
    fn prepare_syscall(&self, number: u32, _vm: &IVM) -> Result<u64, VMError> {
        match number {
            syscalls::SYSCALL_TRANSFER_ASSET_SCOPED | syscalls::SYSCALL_ABORT => Ok(0),
            _ => Err(VMError::UnknownSyscall(number)),
        }
    }

    fn syscall(&mut self, number: u32, vm: &mut IVM) -> Result<u64, VMError> {
        match number {
            syscalls::SYSCALL_TRANSFER_ASSET_SCOPED => {
                let from = Self::decode_account(vm, 10)?;
                let to = Self::decode_account(vm, 11)?;
                let asset = Self::decode_asset(vm)?;
                let amount = Self::decode_amount(vm)?;
                let _dataspace = Self::decode_dataspace(vm)?;
                let from_key = (from, asset.clone());
                let to_key = (to, asset);
                let mut state = self.state.lock().expect("state mutex poisoned");
                let from_balance = state.get(&from_key).copied().unwrap_or_default();
                if from_balance < amount {
                    return Err(VMError::DecodeError);
                }
                if from_key != to_key {
                    let to_balance = state.get(&to_key).copied().unwrap_or_default();
                    let updated_to = to_balance.checked_add(amount).ok_or(VMError::DecodeError)?;
                    state.insert(from_key, from_balance - amount);
                    state.insert(to_key, updated_to);
                }
                Ok(0)
            }
            syscalls::SYSCALL_ABORT => {
                println!("Transaction aborted");
                Ok(0)
            }
            _ => Err(VMError::UnknownSyscall(number)),
        }
    }

    /// Downcast support for hosts with extra methods/state.
    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

fn build_program() -> Vec<u8> {
    let mut prog = ivm::ProgramMetadata::default().encode();
    // If the scalar balance in x15 is below the scalar amount in x16, abort.
    let branch = kwide::encode_branch_checked(instruction::wide::control::BLTU, 15, 16, 2)
        .expect("branch offset");
    prog.extend_from_slice(&branch.to_le_bytes());
    // perform transfer
    prog.extend_from_slice(
        &encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            syscalls::SYSCALL_TRANSFER_ASSET_SCOPED as u8,
        )
        .to_le_bytes(),
    );
    // halt after success
    prog.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    // abort branch target
    prog.extend_from_slice(
        &encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            syscalls::SYSCALL_ABORT as u8,
        )
        .to_le_bytes(),
    );
    prog.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    prog
}

fn make_tlv(pointer_type: PointerType, payload: &[u8]) -> Vec<u8> {
    let mut envelope = Vec::with_capacity(7 + payload.len() + iroha_crypto::Hash::LENGTH);
    envelope.extend_from_slice(&(pointer_type as u16).to_be_bytes());
    envelope.push(1);
    envelope.extend_from_slice(
        &u32::try_from(payload.len())
            .expect("example payload length fits u32")
            .to_be_bytes(),
    );
    envelope.extend_from_slice(payload);
    envelope.extend_from_slice(iroha_crypto::Hash::new(payload).as_ref());
    envelope
}

fn main() {
    let alice = AccountId::new(
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
            .parse()
            .expect("Alice public key"),
    );
    let bob = AccountId::new(
        "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
            .parse()
            .expect("Bob public key"),
    );
    let asset = AssetDefinitionId::new(
        DomainId::try_new("wonderland", "universal").expect("example domain"),
        "rose".parse().expect("example asset name"),
    );
    let host = AssetHost::new(&[
        (alice.clone(), asset.clone(), 100),
        (bob.clone(), asset.clone(), 10),
    ]);
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host.clone());
    let prog = build_program();
    vm.load_program(&prog).unwrap();
    let alice_pointer = vm
        .alloc_input_tlv(&make_tlv(
            PointerType::AccountId,
            &norito::to_bytes(&alice).expect("encode Alice"),
        ))
        .expect("allocate Alice TLV");
    let bob_pointer = vm
        .alloc_input_tlv(&make_tlv(
            PointerType::AccountId,
            &norito::to_bytes(&bob).expect("encode Bob"),
        ))
        .expect("allocate Bob TLV");
    let asset_pointer = vm
        .alloc_input_tlv(&make_tlv(
            PointerType::AssetDefinitionId,
            &norito::to_bytes(&asset).expect("encode asset definition"),
        ))
        .expect("allocate asset TLV");
    let amount_pointer = vm
        .alloc_input_tlv(
            &ivm::numeric_tlv::encode_quantity(&Quantity::from(30_u64))
                .expect("encode quantity TLV"),
        )
        .expect("allocate quantity TLV");
    let dataspace_pointer = vm
        .alloc_input_tlv(&make_tlv(
            PointerType::DataSpaceId,
            &norito::to_bytes(&DataSpaceId::UNIVERSAL).expect("encode dataspace"),
        ))
        .expect("allocate dataspace TLV");
    vm.set_register(10, alice_pointer);
    vm.set_register(11, bob_pointer);
    vm.set_register(12, asset_pointer);
    vm.set_register(13, amount_pointer);
    vm.set_register(14, dataspace_pointer);
    vm.set_register(15, host.balance(&alice, &asset));
    vm.set_register(16, 30);
    vm.run().expect("VM execution failed");
    println!(
        "Balances after transfer: acc1={}, acc2={}",
        host.balance(&alice, &asset),
        host.balance(&bob, &asset)
    );
}
