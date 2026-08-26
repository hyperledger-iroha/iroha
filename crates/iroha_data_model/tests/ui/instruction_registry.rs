use iroha_data_model::{isi::InstructionRegistry, prelude::*};
fn main() {
    let _registry: InstructionRegistry = InstructionRegistry::new()
        .register_with_id::<Log>("test.log.v1")
        .register_with_id::<Register<Domain>>("test.domain.register.v1")
        .register_with_id::<Unregister<Domain>>("test.domain.unregister.v1")
        .register_with_id::<Mint<Quantity, Asset>>("test.asset.mint.v1")
        .register_with_id::<Burn<Quantity, Asset>>("test.asset.burn.v1")
        .register_with_id::<SetParameter>("test.parameter.set.v1")
        .register_with_id::<SetKeyValue<Domain>>("test.domain.metadata.set.v1")
        .register_with_id::<RemoveKeyValue<Domain>>("test.domain.metadata.remove.v1")
        .register_with_id::<Transfer<Asset, Quantity, Account>>("test.asset.transfer.v1");
}
