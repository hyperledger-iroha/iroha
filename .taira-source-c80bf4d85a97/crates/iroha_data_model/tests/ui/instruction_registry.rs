use iroha_data_model::{isi::InstructionRegistry, prelude::*};

fn main() {
    let _registry: InstructionRegistry = instruction_registry!(
        Log,
        Register<Domain>,
        Unregister<Domain>,
        Mint<Quantity, Asset>,
        Burn<Quantity, Asset>,
        SetParameter,
        SetKeyValue<Domain>,
        RemoveKeyValue<Domain>,
        Transfer<Asset, Quantity, Account>,
    );
}
