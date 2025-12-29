use iroha_data_model::{isi::InstructionRegistry, prelude::*};

fn main() {
    let _registry: InstructionRegistry = instruction_registry!(
        Log,
        Register<Domain>,
        Unregister<Domain>,
        Mint<Numeric, Asset>,
        Burn<Numeric, Asset>,
        SetParameter,
        SetKeyValue<Domain>,
        RemoveKeyValue<Domain>,
        Transfer<Asset, Numeric, Account>,
    );
}
