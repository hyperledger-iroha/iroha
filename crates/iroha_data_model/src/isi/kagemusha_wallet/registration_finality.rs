//! Exact immutable universal-asset registration authenticated by successful global execution.
//! No original decoder or transaction hash can construct the returned capability.
use super::{KagemushaWalletLedgerActionV1, KagemushaWalletLedgerV1};
use crate::{
    NetworkId,
    account::AccountId,
    asset::AssetBalanceScope,
    kagemusha::{KagemushaWalletAssetScopeV1, KagemushaWalletSchemeV1},
    query::CommittedTransaction,
    sumeragi_finality::{FinalityError, VerifiedSumeragiBlock},
    transaction::{Executable, TransactionEntrypoint},
};

/// Canonical asset DATA is fixed-size apart from its standard frame; no path or metadata is allowed.
pub const KAGEMUSHA_WALLET_REGISTRATION_ASSET_MAX_BYTES_V1: usize = 4096;

/// Failed scope, execution or immutable registration binding.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KagemushaWalletRegistrationFinalityErrorV1 {
    /// Native global scope, signatures, successful output or membership failed.
    #[error(transparent)]
    Finality(#[from] FinalityError),
    /// The exact selected direct instruction is not Register.
    #[error("selected successful instruction is not a direct KAGEMUSHA Register")]
    Instruction,
    /// A canonical scheme or asset original is malformed, foreign or changed.
    #[error("finalized registration original differs from the selected scheme or asset")]
    Original,
    /// Only an unrestricted Global balance bucket is eligible for universal selection.
    #[error("universal wallet registration requires the Global balance scope")]
    Scope,
    /// The external transaction authority did not give the exact reserve consent.
    #[error("registration reserve differs from the signed transaction authority")]
    Reserve,
}

/// Immutable registration derived only from verified successful native global execution.
/// It contains no decoder, public fields or unchecked constructor. Application policy,
/// enrollment, source qualification and account admission remain separate authorities.
#[derive(Debug, Clone)]
pub struct FinalizedKagemushaWalletRegistrationV1 {
    scheme: KagemushaWalletSchemeV1,
    asset: KagemushaWalletAssetScopeV1,
    scheme_original: Vec<u8>,
    asset_original: Vec<u8>,
    reserve: AccountId,
    block_hash: [u8; 32],
    transaction_hash: [u8; 32],
    height: u64,
    instruction_index: usize,
}
impl FinalizedKagemushaWalletRegistrationV1 {
    /// Complete scheme selected independently and matched to the successful original.
    #[must_use]
    pub const fn scheme(&self) -> &KagemushaWalletSchemeV1 {
        &self.scheme
    }
    /// Registered definition, immutable incarnation and authoritative scale.
    #[must_use]
    pub fn asset(&self) -> &KagemushaWalletAssetScopeV1 {
        &self.asset
    }
    /// Exact canonical scheme frame executed by the ledger.
    #[must_use]
    pub fn scheme_original(&self) -> &[u8] {
        &self.scheme_original
    }
    /// Exact canonical asset frame executed by the ledger.
    #[must_use]
    pub fn asset_original(&self) -> &[u8] {
        &self.asset_original
    }
    /// Consenting reserve, authenticated by the successful external transaction.
    #[must_use]
    pub fn reserve(&self) -> &AccountId {
        &self.reserve
    }
    /// Certified block identity containing the registration.
    #[must_use]
    pub const fn block_hash(&self) -> [u8; 32] {
        self.block_hash
    }
    /// Original account-signed registration transaction identity.
    #[must_use]
    pub const fn transaction_hash(&self) -> [u8; 32] {
        self.transaction_hash
    }
    /// Certified non-genesis height of successful registration.
    #[must_use]
    pub const fn height(&self) -> u64 {
        self.height
    }
    /// Exact direct instruction selected in the authenticated transaction.
    #[must_use]
    pub const fn instruction_index(&self) -> usize {
        self.instruction_index
    }
}

/// Extract immutable token registration from an existing native finalized-block capability.
///
/// The expected network, chain and complete scheme come from the installed trust owner.
/// `asset_digest` and `instruction_index` select requested DATA; they confer no authority.
/// Successful execution binds the registered incarnation/scale, the exact balance bucket,
/// governance permission and reserve consent under the canonical ledger implementation.
/// No nested batch, VM call, failed output, private root or mere transaction inclusion qualifies.
///
/// # Errors
/// Rejects wrong native scope, signature, result or membership; non-Register input; altered
/// scheme/asset original; another asset digest; restricted balance scope; or reserve mismatch.
pub fn verify_finalized_kagemusha_wallet_registration_v1(
    verified: &VerifiedSumeragiBlock,
    committed: &CommittedTransaction,
    expected_network: NetworkId,
    expected_chain: &str,
    expected_scheme: &KagemushaWalletSchemeV1,
    asset_digest: [u8; 32],
    instruction_index: usize,
) -> Result<FinalizedKagemushaWalletRegistrationV1, KagemushaWalletRegistrationFinalityErrorV1> {
    use KagemushaWalletRegistrationFinalityErrorV1 as Error;
    verified.verify_global_scope(expected_network, expected_chain)?;
    verified.verify_committed_transaction(&expected_network, committed)?;
    expected_scheme.validate().map_err(|_| Error::Original)?;
    if expected_scheme.network_id != *expected_network.as_bytes() || asset_digest == [0; 32] {
        return Err(Error::Original);
    }
    let TransactionEntrypoint::External(transaction) = committed.entrypoint() else {
        return Err(Error::Instruction);
    };
    let Executable::Instructions(instructions) = transaction.instructions() else {
        return Err(Error::Instruction);
    };
    let instruction = instructions
        .get(instruction_index)
        .and_then(|value| value.as_any().downcast_ref::<KagemushaWalletLedgerV1>())
        .ok_or(Error::Instruction)?;
    let KagemushaWalletLedgerActionV1::Register {
        scheme,
        asset,
        reserve,
        balance_scope,
    } = &instruction.action
    else {
        return Err(Error::Instruction);
    };
    if *balance_scope != AssetBalanceScope::Global {
        return Err(Error::Scope);
    }
    if reserve != transaction.authority() {
        return Err(Error::Reserve);
    }
    let selected_scheme = expected_scheme.scheme_id();
    let parsed_scheme = KagemushaWalletSchemeV1::decode_canonical(scheme, &selected_scheme)
        .map_err(|_| Error::Original)?;
    if instruction.scheme != selected_scheme
        || parsed_scheme != *expected_scheme
        || asset.is_empty()
        || asset.len() > KAGEMUSHA_WALLET_REGISTRATION_ASSET_MAX_BYTES_V1
    {
        return Err(Error::Original);
    }
    let parsed_asset: KagemushaWalletAssetScopeV1 =
        norito::decode_canonical_with_limits(asset, norito::canonical_decode_limits(asset.len()))
            .map_err(|_| Error::Original)?;
    parsed_asset.validate().map_err(|_| Error::Original)?;
    if parsed_asset.asset_digest() != asset_digest {
        return Err(Error::Original);
    }
    Ok(FinalizedKagemushaWalletRegistrationV1 {
        scheme: parsed_scheme,
        asset: parsed_asset,
        scheme_original: scheme.clone(),
        asset_original: asset.clone(),
        reserve: reserve.clone(),
        block_hash: *committed.block_hash().as_ref(),
        transaction_hash: *transaction.hash().as_ref(),
        height: verified.height(),
        instruction_index,
    })
}

#[cfg(test)]
mod tests;
