//! KAGEMUSHA's closed instruction boundary. Foreign callers never supply a proof verdict.
//! All ledger effects use the original WSV transaction and canonical asset movement owner.
use super::*;
use crate::kagemusha_wallet_v1::{self as wallet, NativePackageVerifier, wsv::WsvLedger};
use iroha_data_model::{
    isi::kagemusha_wallet::{KagemushaWalletLedgerActionV1 as Action, KagemushaWalletLedgerV1},
    kagemusha::*,
};

fn error(error: wallet::Error) -> Error {
    match error {
        wallet::Error::Execution(error) => error,
        error => Error::InvariantViolation(format!("KAGEMUSHA ledger: {error}").into()),
    }
}
fn decode<T>(bytes: &[u8], max: usize) -> wallet::Result<T>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    if bytes.len() > max {
        return Err(wallet::Error::Binding);
    }
    norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(max))
        .map_err(|_| wallet::Error::Binding)
}

/// Temporary closed artifact boundary. It cannot authorize any native package.
// TODO(G6): replace this owner with the authenticated scheme/descriptor artifact loader once
// the production A/Ω circuit set is complete. No config flag or foreign verdict may bypass it.
struct ArtifactsUnavailable;
impl NativePackageVerifier for ArtifactsUnavailable {
    fn verify(
        &self,
        _: &KagemushaWalletSchemeV1,
        _: &KagemushaWalletCredentialV1,
        _: &KagemushaWalletPackageV1,
    ) -> wallet::Result<()> {
        Err(wallet::Error::ArtifactsUnavailable)
    }
}
impl Execute for KagemushaWalletLedgerV1 {
    fn execute(
        self,
        authority: &AccountId,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        match execute_with_verifier(self, authority, state, &ArtifactsUnavailable) {
            Err(wallet::Error::ArtifactsUnavailable) => {
                Err(state.world.attempt_error_to_instruction_error(
                    crate::execution_attempt::ExecutionAttemptError::Deferred(
                        ivm::error::ExecutionDeferral::VerifierArtifactsUnavailable.into(),
                    ),
                ))
            }
            result => result.map_err(error),
        }
    }
}

/// Internal dependency boundary for the authenticated native artifact owner. Tests inject a
/// rejecting verifier or explicitly labelled fixture verifier; neither is a production loader.
pub(crate) fn execute_with_verifier(
    instruction: KagemushaWalletLedgerV1,
    authority: &AccountId,
    state: &mut StateTransaction<'_, '_>,
    verifier: &impl NativePackageVerifier,
) -> wallet::Result<()> {
    let scheme_id = instruction.scheme;
    if scheme_id == [0; 32] {
        return Err(wallet::Error::Binding);
    }
    let mut ledger = WsvLedger::new(state, authority)?;
    match instruction.action {
        Action::Register {
            scheme,
            asset,
            reserve,
            balance_scope,
            load_authorizer,
        } => {
            let scheme = KagemushaWalletSchemeV1::decode_canonical(&scheme, &scheme_id)?;
            let load_authorizer =
                KagemushaWalletSignerCertificateV1::decode_canonical(&load_authorizer, &scheme)?;
            ledger.register(wallet::Registration {
                scheme,
                // AssetScope consists only of a version, UUID, digest and fixed-width scale.
                asset: decode(&asset, KAGEMUSHA_WALLET_SCHEME_MAX_BYTES_V1)?,
                reserve,
                balance_scope,
                load_authorizer,
            })
        }
        Action::Activate(bytes) => wallet::activate(
            &mut ledger,
            verifier,
            &KagemushaWalletActivationV1::decode_canonical(&bytes, &scheme_id)?,
        ),
        Action::Abandon(bytes) => wallet::abandon(
            &mut ledger,
            &KagemushaWalletAbandonmentV1::decode_canonical(&bytes, &scheme_id)?,
        ),
        Action::CloseLoads(bytes) => wallet::close_loads(
            &mut ledger,
            verifier,
            &KagemushaWalletCloseLoadsV1::decode_canonical(&bytes, &scheme_id)?,
        ),
        Action::IssueLoad {
            wallet: wallet_id,
            request_id,
            amount,
            charge,
        } => {
            let charge = charge
                .map(|charge| {
                    Ok::<_, wallet::Error>(wallet::LoadCharge {
                        quote: decode(&charge.quote, KAGEMUSHA_WALLET_CHARGE_QUOTE_MAX_BYTES_V1)?,
                        beneficiary: charge.beneficiary,
                    })
                })
                .transpose()?;
            wallet::issue_load(
                &mut ledger,
                &wallet::LoadCommand {
                    scheme: scheme_id,
                    wallet: wallet_id,
                    request_id,
                    amount,
                    charge,
                },
            )
            .map(|_| ())
        }
        Action::Unload(bytes) => wallet::pay_unload(
            &mut ledger,
            verifier,
            &KagemushaWalletUnloadClaimV1::decode_canonical(&bytes, &scheme_id)?,
        )
        .map(|_| ()),
        Action::ClaimFee(bytes) => wallet::pay_fee(
            &mut ledger,
            verifier,
            &KagemushaWalletFeeClaimV1::decode_canonical(&bytes, &scheme_id)?,
        )
        .map(|_| ()),
        Action::RetainCertificate { asset, certificate } => {
            let certificate: KagemushaWalletSignerCertificateV1 =
                decode(&certificate, KAGEMUSHA_WALLET_CERTIFICATE_MAX_BYTES_V1)?;
            if certificate.body.scheme_id != scheme_id {
                return Err(wallet::Error::Binding);
            }
            ledger.retain_certificate(asset, certificate)
        }
        Action::RetainCredential {
            credential,
            certificates,
        } => {
            let credential =
                KagemushaWalletCredentialV1::decode_canonical(&credential, &scheme_id)?;
            let certificates = decode(
                &certificates,
                KAGEMUSHA_WALLET_CERTIFICATE_SET_MAX_V1 * KAGEMUSHA_WALLET_CERTIFICATE_MAX_BYTES_V1
                    + 128,
            )?;
            ledger.retain_credential(wallet::CredentialRecord {
                credential,
                certificates,
            })
        }
        Action::RetainRequest(bytes) => {
            let request: KagemushaWalletRequestV1 =
                decode(&bytes, KAGEMUSHA_WALLET_SESSION_MAX_BYTES_V1)?;
            if request.body.scheme_id != scheme_id {
                return Err(wallet::Error::Binding);
            }
            ledger.retain_request(request)
        }
        Action::PublishVoucher {
            request_id,
            voucher,
        } => {
            let voucher = KagemushaWalletLoadVoucherV1::decode_canonical(&voucher, &scheme_id)?;
            ledger.publish_voucher(request_id, &voucher)
        }
        Action::RotateLoadAuthorizer { asset, certificate } => {
            let certificate: KagemushaWalletSignerCertificateV1 =
                decode(&certificate, KAGEMUSHA_WALLET_CERTIFICATE_MAX_BYTES_V1)?;
            if certificate.body.scheme_id != scheme_id {
                return Err(wallet::Error::Binding);
            }
            ledger.rotate_load_authorizer(asset, certificate)
        }
    }
}
