//! Deterministic KAGEMUSHA ledger orchestration (§6).
//!
//! Native proof verification is a mandatory injected dependency. Model validation alone never
//! authorizes a transfer. The transaction owner atomically persists transfers and permanent
//! replay indexes; finalized load vouchers have a separate source-bound publication boundary.
// TODO(G6): install the authenticated native package artifact loader and qualify the
// complete online/offline flow with the production A/Ω artifact set.

use iroha_data_model::{account::AccountId, asset::AssetBalanceScope, kagemusha::*};
use norito::{Decode, Encode};

mod authorizer;
pub use authorizer::{
    LOAD_AUTHORIZER_KEYRING_MAX_BYTES, LOAD_AUTHORIZER_MAX_KEYS, LoadAuthorizer,
    LoadAuthorizerKeyV1, LoadAuthorizerKeyringV1, PreparedVoucher, PublicationWorker,
};
pub(crate) mod custody;
mod finalized;
pub use finalized::FinalizedLedger;
mod ledger;
mod pending;
pub use pending::{MAX_PENDING_PAGE, PendingPublication};
pub(crate) mod routing;
mod storage;
pub(crate) mod wsv;
pub use ledger::{
    abandon, activate, close_loads, issue_load, pay_fee, pay_unload, publish_voucher,
};
pub(crate) use storage::validate_snapshot;
pub use storage::{CredentialRecord, LedgerKey, validate_row};
#[cfg(test)]
mod tests;

/// Canonical 32-byte identity used by this protocol.
pub type Digest = [u8; 32];
/// Deterministic ledger failure; no failure authorizes a partial payout.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Preserve the canonical asset owner's exact execution or local-deferral failure.
    #[error(transparent)]
    Execution(#[from] iroha_data_model::isi::error::InstructionExecutionError),
    /// Invalid canonical model object or signature.
    #[error(transparent)]
    Model(#[from] KagemushaWalletValidationErrorV1),
    /// The mandatory proof verifier rejected the complete package.
    #[error("native package proof rejected")]
    Proof,
    /// The authenticated artifact set is unavailable.
    #[error("native proof artifacts unavailable")]
    ArtifactsUnavailable,
    /// A scheme, asset, account, or retained source binding differs.
    #[error("ledger source binding differs")]
    Binding,
    /// The permanent key already records a different operation.
    #[error("permanent replay key conflicts")]
    Conflict,
    /// The wallet has not been activated or is permanently closed/abandoned.
    #[error("wallet does not permit this operation")]
    Lifecycle,
    /// Issued vouchers have not all been absorbed before closing loads.
    #[error("unabsorbed load voucher")]
    OutstandingLoad,
    /// Arithmetic exceeded the fixed monetary or ordinal range.
    #[error("ledger amount or ordinal overflow")]
    Overflow,
    /// Missing historical ledger record or unavailable storage.
    #[error("ledger record unavailable")]
    Unavailable,
    /// The source block has not finalized.
    #[error("load issuance is not finalized")]
    NotFinalized,
    /// Ledger accounts cannot fund the whole atomic transfer batch.
    #[error("insufficient ledger balance")]
    InsufficientFunds,
}
/// Result of a deterministic ledger operation.
pub type Result<T> = std::result::Result<T, Error>;

/// A complete native verifier, bound to the scheme's authenticated relation descriptors.
/// Implementations must verify σ, its receipt binding and the required predecessor Ω with the
/// production proof engine. There is deliberately no default or structural-only implementation.
pub trait NativePackageVerifier {
    /// Verify the complete package for this exact credential and scheme.
    ///
    /// # Errors
    /// Return `ArtifactsUnavailable` when no authenticated artifact set can be loaded, and
    /// `Proof` on any invalid or unverified proof. Never translate either into success.
    fn verify(
        &self,
        scheme: &KagemushaWalletSchemeV1,
        credential: &KagemushaWalletCredentialV1,
        package: &KagemushaWalletPackageV1,
    ) -> Result<()>;
}

/// Ledger-owned scheme/asset registration; caller supplied registrations are not authority.
#[derive(Clone, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::kagemusha_wallet_v1::Registration")]
pub struct Registration {
    /// Scheme root and relation identity recorded by ledger governance.
    pub scheme: KagemushaWalletSchemeV1,
    /// Current asset incarnation and scale from the asset registry.
    pub asset: KagemushaWalletAssetScopeV1,
    /// Segregated reserve account for this scheme and asset.
    pub reserve: AccountId,
    /// Exact balance partition; never inferred from mutable account routing.
    pub balance_scope: AssetBalanceScope,
    /// Ledger-recorded LoadAuthorization signer certificate.
    pub load_authorizer: KagemushaWalletSignerCertificateV1,
}
impl Registration {
    fn require(&self, scheme: &Digest, asset: &Digest) -> Result<()> {
        self.scheme.validate()?;
        self.asset.validate()?;
        self.load_authorizer
            .verify_role(&self.scheme, KagemushaWalletSignerRoleV1::LoadAuthorization)?;
        if self.scheme.scheme_id() != *scheme || self.asset.asset_digest() != *asset {
            return Err(Error::Binding);
        }
        Ok(())
    }
}

/// Permanent ledger lifecycle; closure or abandonment can never reactivate an incarnation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::kagemusha_wallet_v1::Phase")]
pub enum Phase {
    /// Bootstrap accepted, load issuance permitted.
    Active,
    /// Retiring package closed further issuance.
    Closed,
    /// Unused enrollment was irreversibly abandoned.
    Abandoned,
}
/// Constant-size wallet index. It does not retain or scan the voucher history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::kagemusha_wallet_v1::WalletRecord")]
pub struct WalletRecord {
    /// Immutable asset binding.
    pub asset: Digest,
    /// Lifecycle.
    pub phase: Phase,
    /// Accepted Bootstrap package digest, or zero after abandonment.
    pub activation: Digest,
    /// First ordinal not issued by the ledger.
    pub next_load: u128,
}
/// Load command identity and pricing; `request_id` remains stable across transaction retries.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::kagemusha_wallet_v1::LoadCommand")]
pub struct LoadCommand {
    /// Scheme registration identity.
    pub scheme: Digest,
    /// Wallet incarnation.
    pub wallet: Digest,
    /// Nonzero client retry identity, scoped by wallet.
    pub request_id: Digest,
    /// Net value deposited in the reserve.
    pub amount: u128,
    /// Explicit signed online charge, separate from the reserve deposit.
    pub charge: Option<LoadCharge>,
}
/// Ledger-recorded signed load quote and fixed beneficiary.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::kagemusha_wallet_v1::LoadCharge")]
pub struct LoadCharge {
    /// Displayed quote accepted by the payer.
    pub quote: KagemushaWalletChargeQuoteV1,
    /// Account matching the quote's beneficiary digest.
    pub beneficiary: AccountId,
}
/// Durable load issuance: debit, ordinal and unsigned body are one atomic ledger transition.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::kagemusha_wallet_v1::Issuance")]
pub struct Issuance {
    /// Original command used for exact semantic retry comparison.
    pub command: LoadCommand,
    /// Authenticated transaction authority whose account funded the deposit.
    pub payer: AccountId,
    /// Body that only a finalized source may release for signing.
    pub body: KagemushaWalletLoadVoucherBodyV1,
    /// First published canonical voucher bytes; immutable once present.
    pub voucher: Option<Vec<u8>>,
}
/// Separate replay namespaces prevent fee identities from colliding with unload nullifiers.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_core::kagemusha_wallet_v1::ClaimKey")]
pub enum ClaimKey {
    /// Domain-separated unload nullifier.
    Unload(Digest),
    /// Committed Send credit identity.
    Fee(Digest),
}
/// Durable original payout result returned by every valid exact retry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::kagemusha_wallet_v1::Payout")]
pub struct Payout {
    /// Permanent exact-once key.
    pub key: ClaimKey,
    /// Package digest (Unload) or Payment digest (fee).
    pub source: Digest,
    /// Total reserve liability released; includes a quoted unload charge.
    pub amount: u128,
    /// Ledger transaction which first paid this claim.
    pub transaction: Digest,
}
/// One transfer of atomic asset units; reserve movements use the registered asset scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transfer {
    /// Debited account.
    pub from: AccountId,
    /// Credited account.
    pub to: AccountId,
    /// Exact positive amount.
    pub amount: u128,
}
/// Atomic mutation produced only after verification and all deterministic checks.
#[derive(Debug)]
pub struct Batch {
    /// Scheme whose segregated reserve and permanent indexes are affected.
    scheme: Digest,
    /// Immutable asset scope of every transfer and source binding.
    asset: Digest,
    /// Wallet index replacement, if any.
    wallet: Option<(Digest, WalletRecord)>,
    /// Unique issuance insertion or first voucher publication, if any.
    issuance: Option<Issuance>,
    /// Exact-once payout insertion, if any.
    payout: Option<Payout>,
    /// Transfers that commit in the same transaction as all indexes.
    transfers: Vec<Transfer>,
}
impl Batch {
    fn new(scheme: Digest, asset: Digest) -> Self {
        Self {
            scheme,
            asset,
            wallet: None,
            issuance: None,
            payout: None,
            transfers: Vec::new(),
        }
    }
}
/// Historical inputs retrieved by digest from immutable ledger records for a fee claim.
pub struct FeeInputs {
    /// Original full Request, including the historical fee schedule and certificates.
    pub request: KagemushaWalletRequestV1,
    /// Credential named by the Payment's payer credential digest.
    pub payer: KagemushaWalletCredentialV1,
    /// Historical certificates validating that credential.
    pub certificates: KagemushaWalletCertificateSetV1,
}
/// Single WSV instruction transaction. Reads and `apply` must share an isolated snapshot.
/// `apply` must commit every transfer/index together or none, enforce account authorization,
/// compare insert-only replay keys, and never turn unavailable data into absence.
/// Its authority/hash/height are derived from authenticated execution, not payload fields.
pub trait Transaction {
    /// Authenticated transaction authority.
    fn authority(&self) -> &AccountId;
    /// Hash of the transaction currently executing.
    fn transaction_hash(&self) -> Digest;
    /// Height of the executing block, later independently checked for finality.
    fn block_height(&self) -> u64;
    /// Read registered scheme and current asset scope.
    fn registration(&self, scheme: &Digest, asset: &Digest) -> Result<Registration>;
    /// Read the permanent wallet lifecycle and next load ordinal.
    fn wallet(&self, scheme: &Digest, wallet: &Digest) -> Result<Option<WalletRecord>>;
    /// Read a retained load by its client retry identity.
    fn issuance(
        &self,
        scheme: &Digest,
        wallet: &Digest,
        request: &Digest,
    ) -> Result<Option<Issuance>>;
    /// Read the original exact-once payout.
    fn payout(&self, scheme: &Digest, key: ClaimKey) -> Result<Option<Payout>>;
    /// Read an immutable signer certificate by its object digest.
    fn certificate(
        &self,
        scheme: &Digest,
        digest: &Digest,
    ) -> Result<KagemushaWalletSignerCertificateV1>;
    /// Resolve all historical inputs named by this Payment from ledger records.
    fn fee_inputs(&self, claim: &KagemushaWalletFeeClaimV1) -> Result<FeeInputs>;
    /// Commit the entire verified batch, or leave the transaction unchanged.
    fn apply(&mut self, batch: Batch) -> Result<()>;
}
/// Finalized ledger read capability. Implementations authenticate the exact retained issuance
/// against a finalized block, including transaction inclusion and height; a live WSV read or
/// caller-provided voucher body is insufficient. No timeout or failed delivery refunds a load.
pub trait FinalizedSource {
    /// Return the source-bound issuance only after its block finalized.
    fn issuance(&self, scheme: &Digest, wallet: &Digest, request: &Digest) -> Result<Issuance>;
}
