//! Demonstrates mint and burn operations using canonical pointer-ABI account,
//! asset-definition, and `QuantityValueV1` TLVs.
use iroha_data_model::{account::AccountId, asset::AssetDefinitionId, domain::DomainId};
use iroha_primitives::{numeric::Quantity, numeric_abi::QuantityValueV1};
use ivm::{
    IVM, PointerType, VMError, encoding, host::IVMHost, instruction, kotodama::wide as kwide,
    syscalls,
};
use std::{
    any::Any,
    collections::HashMap,
    sync::{Arc, Mutex},
};
#[derive(Clone)]
struct AssetHost {
    balances: Arc<Mutex<HashMap<(AccountId, AssetDefinitionId), u64>>>,
    supply: Arc<Mutex<u64>>,
}
impl AssetHost {
    fn new() -> Self {
        AssetHost {
            balances: Arc::new(Mutex::new(HashMap::new())),
            supply: Arc::new(Mutex::new(0)),
        }
    }
    fn balance(&self, account: &AccountId, asset: &AssetDefinitionId) -> u64 {
        let balances = self.balances.lock().expect("balance mutex poisoned");
        *balances
            .get(&(account.clone(), asset.clone()))
            .unwrap_or(&0)
    }
    fn supply(&self) -> u64 {
        *self.supply.lock().expect("supply mutex poisoned")
    }
    fn decode_account(vm: &IVM) -> Result<AccountId, VMError> {
        let tlv = vm.validate_tlv(vm.register(10))?;
        if tlv.type_id != PointerType::AccountId {
            return Err(VMError::NoritoInvalid);
        }
        norito::decode_from_bytes(tlv.payload).map_err(|_| VMError::DecodeError)
    }
    fn decode_asset(vm: &IVM) -> Result<AssetDefinitionId, VMError> {
        let tlv = vm.validate_tlv(vm.register(11))?;
        if tlv.type_id != PointerType::AssetDefinitionId {
            return Err(VMError::NoritoInvalid);
        }
        norito::decode_from_bytes(tlv.payload).map_err(|_| VMError::DecodeError)
    }
    fn decode_amount(vm: &IVM) -> Result<u64, VMError> {
        let tlv = vm.validate_tlv(vm.register(12))?;
        if tlv.type_id != PointerType::Quantity {
            return Err(VMError::NoritoInvalid);
        }
        let quantity = QuantityValueV1::decode_frame(tlv.payload)
            .map_err(|_| VMError::DecodeError)?
            .into_quantity();
        u64::try_from(quantity.into_numeric()).map_err(|_| VMError::DecodeError)
    }
}
impl IVMHost for AssetHost {
    fn prepare_syscall(&self, number: u32, _vm: &IVM) -> Result<u64, VMError> {
        match number {
            syscalls::SYSCALL_MINT_ASSET | syscalls::SYSCALL_BURN_ASSET => Ok(0),
            _ => Err(VMError::UnknownSyscall(number)),
        }
    }
    fn syscall(&mut self, number: u32, vm: &mut IVM) -> Result<u64, VMError> {
        if !matches!(
            number,
            syscalls::SYSCALL_MINT_ASSET | syscalls::SYSCALL_BURN_ASSET
        ) {
            return Err(VMError::UnknownSyscall(number));
        }
        let account = Self::decode_account(vm)?;
        let asset = Self::decode_asset(vm)?;
        let amount = Self::decode_amount(vm)?;
        let mut balances = self.balances.lock().expect("balance mutex poisoned");
        let mut supply = self.supply.lock().expect("supply mutex poisoned");
        match number {
            syscalls::SYSCALL_MINT_ASSET => {
                let balance = balances.entry((account, asset)).or_default();
                let updated_balance = balance.checked_add(amount).ok_or(VMError::DecodeError)?;
                let updated_supply = supply.checked_add(amount).ok_or(VMError::DecodeError)?;
                *balance = updated_balance;
                *supply = updated_supply;
                Ok(0)
            }
            syscalls::SYSCALL_BURN_ASSET => {
                let balance = balances.entry((account, asset)).or_default();
                if *balance < amount {
                    return Err(VMError::DecodeError);
                }
                *balance -= amount;
                *supply = supply.checked_sub(amount).ok_or(VMError::DecodeError)?;
                Ok(0)
            }
            _ => unreachable!("mutation syscall filtered above"),
        }
    }
    /// Downcast support for hosts with extra methods/state.
    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}
fn build_program() -> Vec<u8> {
    let mut prog = ivm::ProgramMetadata::default().encode();
    // if current_supply >= cap -> burn instead of mint (skip next two instructions)
    let branch = kwide::encode_branch_checked(instruction::wide::control::BGEU, 13, 14, 2)
        .expect("branch offset");
    prog.extend_from_slice(&branch.to_le_bytes());
    prog.extend_from_slice(
        &encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            syscalls::SYSCALL_MINT_ASSET as u8,
        )
        .to_le_bytes(),
    );
    prog.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    prog.extend_from_slice(
        &encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            syscalls::SYSCALL_BURN_ASSET as u8,
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
    let host = AssetHost::new();
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host.clone());
    let prog = build_program();
    vm.load_program(&prog).unwrap();
    let account = AccountId::new(
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
            .parse()
            .expect("example public key"),
    );
    let asset = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("example domain"),
        "rose".parse().expect("example asset name"),
    );
    let account_pointer = vm
        .alloc_input_tlv(&make_tlv(
            PointerType::AccountId,
            &norito::to_bytes(&account).expect("encode account"),
        ))
        .expect("allocate account TLV");
    let asset_pointer = vm
        .alloc_input_tlv(&make_tlv(
            PointerType::AssetDefinitionId,
            &norito::to_bytes(&asset).expect("encode asset definition"),
        ))
        .expect("allocate asset TLV");
    let amount_pointer = vm
        .alloc_input_tlv(
            &ivm::numeric_tlv::encode_quantity(&Quantity::from(50_u64))
                .expect("encode quantity TLV"),
        )
        .expect("allocate quantity TLV");
    vm.set_register(10, account_pointer);
    vm.set_register(11, asset_pointer);
    vm.set_register(12, amount_pointer);
    vm.set_register(13, host.supply());
    vm.set_register(14, 100);
    vm.run().expect("VM execution failed");
    println!(
        "Balance after op: {}  Total supply: {}",
        host.balance(&account, &asset),
        host.supply()
    );
}
