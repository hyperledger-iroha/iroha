//! Tests for instruction implementations

use iroha_data_model::{
    isi::{BuiltInInstruction, register::RegisterPeerWithPop, rwa::*},
    prelude::*,
};
use iroha_primitives::numeric::Numeric;
use norito::codec::Encode;

fn assert_instruction<T: Instruction + BuiltInInstruction + Clone + Encode + 'static>() {}

#[test]
fn built_in_instructions_implement_traits() {
    macro_rules! check {
        ($($ty:ty),* $(,)?) => {
            $(assert_instruction::<$ty>();)*
        };
    }

    check!(
        SetKeyValue<Domain>,
        SetKeyValue<AssetDefinition>,
        SetKeyValue<Account>,
        SetKeyValue<Nft>,
        SetKeyValue<Rwa>,
        SetKeyValue<Trigger>,
        RemoveKeyValue<Domain>,
        RemoveKeyValue<AssetDefinition>,
        RemoveKeyValue<Account>,
        RemoveKeyValue<Nft>,
        RemoveKeyValue<Rwa>,
        RemoveKeyValue<Trigger>,
        RegisterPeerWithPop,
        Register<Domain>,
        Register<Account>,
        Register<AssetDefinition>,
        Register<Nft>,
        Register<Role>,
        Register<Trigger>,
        Unregister<Peer>,
        Unregister<Domain>,
        Unregister<Account>,
        Unregister<AssetDefinition>,
        Unregister<Nft>,
        Unregister<Role>,
        Unregister<Trigger>,
        Mint<Numeric, Asset>,
        Mint<u32, Trigger>,
        Burn<Numeric, Asset>,
        Burn<u32, Trigger>,
        Transfer<Account, DomainId, Account>,
        Transfer<Account, AssetDefinitionId, Account>,
        Transfer<Asset, Numeric, Account>,
        Transfer<Account, NftId, Account>,
        RegisterRwa,
        TransferRwa,
        MergeRwas,
        RedeemRwa,
        FreezeRwa,
        UnfreezeRwa,
        HoldRwa,
        ReleaseRwa,
        ForceTransferRwa,
        SetRwaControls,
        RwaInstructionBox,
        Grant<Permission, Account>,
        Grant<RoleId, Account>,
        Grant<Permission, Role>,
        Revoke<Permission, Account>,
        Revoke<RoleId, Account>,
        Revoke<Permission, Role>,
        SetParameter,
        Upgrade,
        ExecuteTrigger,
        Log,
    );
}
