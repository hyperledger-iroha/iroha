//! Routing reads the same permanent scope as execution. Routing is not proof authorization.
use super::*;
use crate::state::WorldReadOnly;
use iroha_data_model::isi::kagemusha_wallet::{
    KagemushaWalletLedgerActionV1 as Action, KagemushaWalletLedgerV1,
};
use iroha_model_base::topology::DataSpaceId;
use mv::storage::StorageReadOnly as _;

/// Read the same registered reserve scope in queue admission and execution.
pub(crate) enum Operation<'a> {
    Ledger(&'a KagemushaWalletLedgerV1),
    Registered {
        scheme: &'a Digest,
        asset: &'a Digest,
    },
}
impl<'a> Operation<'a> {
    pub(crate) fn from_instruction(
        instruction: &'a dyn iroha_data_model::isi::Instruction,
    ) -> Option<Self> {
        use iroha_data_model::isi::kagemusha_wallet::spartan::{
            ActivateKagemushaSpartanV1, InstallKagemushaSpartanProgramsV1, UnloadKagemushaSpartanV1,
        };
        let value = instruction.as_any();
        if let Some(op) = value.downcast_ref::<KagemushaWalletLedgerV1>() {
            return Some(Self::Ledger(op));
        }
        if let Some(op) = value.downcast_ref::<InstallKagemushaSpartanProgramsV1>() {
            return Some(Self::Registered {
                scheme: &op.scheme,
                asset: &op.asset,
            });
        }
        if let Some(op) = value.downcast_ref::<ActivateKagemushaSpartanV1>() {
            return Some(Self::Registered {
                scheme: &op.scheme,
                asset: &op.asset,
            });
        }
        value
            .downcast_ref::<UnloadKagemushaSpartanV1>()
            .map(|op| Self::Registered {
                scheme: &op.scheme,
                asset: &op.asset,
            })
    }
    pub(crate) fn dataspace(self, world: &impl WorldReadOnly) -> Result<DataSpaceId> {
        match self {
            Self::Ledger(op) => dataspace(world, op),
            Self::Registered { scheme, asset } => registered_dataspace(world, *scheme, *asset),
        }
    }
}

pub fn dataspace(
    world: &impl WorldReadOnly,
    instruction: &KagemushaWalletLedgerV1,
) -> Result<DataSpaceId> {
    let scheme = instruction.scheme;
    let asset = match &instruction.action {
        Action::Register { balance_scope, .. } => return exact_scope(*balance_scope),
        Action::Activate(bytes) => {
            KagemushaWalletActivationV1::decode_canonical(bytes, &scheme)?
                .credential
                .body
                .asset_digest
        }
        Action::Abandon(bytes) => {
            KagemushaWalletAbandonmentV1::decode_canonical(bytes, &scheme)?
                .control
                .body
                .asset_digest
        }
        Action::CloseLoads(bytes) => {
            KagemushaWalletCloseLoadsV1::decode_canonical(bytes, &scheme)?
                .credential
                .body
                .asset_digest
        }
        // Suffix stages route by the immutable wallet row of the outer wallet, never by a
        // stage frame: every stage of one Unload names the same scope.
        Action::IssueLoad { wallet, .. }
        | Action::SuffixUnloadBegin { wallet, .. }
        | Action::SuffixUnloadAppend { wallet, .. }
        | Action::SuffixUnloadFinalize { wallet, .. } => {
            let key = storage::key(storage::WALLET, scheme, *wallet);
            let bytes = world
                .kagemusha_wallet_ledger()
                .get(&key)
                .ok_or(Error::Unavailable)?;
            validate_row(&key, bytes)?;
            storage::decode::<WalletRecord>(bytes, bytes.len())?.asset
        }
        Action::Unload(bytes) => {
            KagemushaWalletUnloadClaimV1::decode_canonical(bytes, &scheme)?
                .credential
                .body
                .asset_digest
        }
        Action::ClaimFee(bytes) => {
            KagemushaWalletFeeClaimV1::decode_canonical(bytes, &scheme)?
                .payment
                .send
                .statement
                .asset_digest
        }
        Action::InstallVerifierPack { asset, .. } | Action::RetainCertificate { asset, .. } => {
            *asset
        }
        Action::RetainCredential { credential, .. } => {
            KagemushaWalletCredentialV1::decode_canonical(credential, &scheme)?
                .body
                .asset_digest
        }
        Action::RetainRequest(bytes) => {
            let value: KagemushaWalletRequestV1 =
                storage::decode(bytes, KAGEMUSHA_WALLET_SESSION_MAX_BYTES_V1)?;
            if value.body.scheme_id != scheme {
                return Err(Error::Binding);
            }
            value.body.asset_digest
        }
    };
    registered_dataspace(world, scheme, asset)
}
fn registered_dataspace(
    world: &impl WorldReadOnly,
    scheme: Digest,
    asset: Digest,
) -> Result<DataSpaceId> {
    let key = storage::key(storage::REGISTRATION, scheme, asset);
    let bytes = world
        .kagemusha_wallet_ledger()
        .get(&key)
        .ok_or(Error::Unavailable)?;
    validate_row(&key, bytes)?;
    exact_scope(storage::decode::<Registration>(bytes, bytes.len())?.balance_scope)
}
fn exact_scope(scope: AssetBalanceScope) -> Result<DataSpaceId> {
    match scope {
        AssetBalanceScope::Global => Ok(DataSpaceId::UNIVERSAL),
        AssetBalanceScope::Dataspace(id) if id != DataSpaceId::UNIVERSAL => Ok(id),
        _ => Err(Error::Binding),
    }
}
