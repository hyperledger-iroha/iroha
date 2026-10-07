//! Bind the exact independently selected native A originals to the executed receipt.
use super::*;

pub(super) struct Target {
    pub(super) frame: Vec<u8>,
    pub(super) originals: BTreeMap<String, Vec<u8>>,
}

pub(super) fn validate(
    selection: &Selection<'_>,
    setup: &source_only::Setup,
    native: &SumeragiFinalityVerifier,
    receipt: &KagemushaWalletLoadReceiptV1,
) -> Result<Target, Error> {
    let frame = pinned(selection.target, 16 << 10, selection.target_sha256)?;
    let json: Value = norito::json::from_slice(&frame).map_err(|_| Error::Input)?;
    need(
        value(&json, "schema")?.as_str() == Some("iroha.kagemusha.native-load-target.v1")
            && fixed::<32>(&json, "executed_ledger_setup_sha256")? == selection.setup_sha256
            && value(&json, "native_chain_id")?.as_str() == Some(native.chain_id())
            && fixed::<32>(&json, "native_instance")? == native.instance().0
            && fixed::<32>(&json, "native_initial_epoch_sha256")?
                == <[u8; 32]>::from(Sha256::digest(
                    norito::encode_canonical(native.initial_epoch()).map_err(|_| Error::Input)?,
                )),
    )?;
    let names = [
        ("account.norito", 4096),
        ("asset.norito", 4096),
        ("credential.norito", 1024),
        ("certificates.norito", 32_768),
        ("bootstrap.norito", 10_000),
        ("activation.norito", 16_384),
        ("issue-load.norito", 4096),
    ];
    let originals = originals(
        selection.target.parent().ok_or(Error::Input)?,
        &json,
        "files",
        &names,
    )?;
    need(
        originals["account.norito"] == setup.originals["account.norito"]
            && originals["asset.norito"] == setup.originals["asset.norito"],
    )?;
    let account: AccountId = canonical(&originals["account.norito"])?;
    let asset: KagemushaWalletAssetScopeV1 = canonical(&originals["asset.norito"])?;
    let scheme = fixed::<32>(&json, "scheme_id")?;
    need(scheme != [0; 32] && fixed::<32>(&json, "manifest_digest")? != [0; 32])?;
    // The activation shape and exact nested originals are checked here; monetary
    // admission was the ledger's job and is authenticated by the native result/event.
    let activation =
        KagemushaWalletActivationV1::decode_canonical(&originals["activation.norito"], &scheme)
            .map_err(|_| Error::Input)?;
    need(
        activation.asset == asset
            && activation.credential.body.account_digest
                == kagemusha_wallet_account_digest_v1(&account).map_err(|_| Error::Input)?
            && activation
                .credential
                .to_canonical_bytes()
                .map_err(|_| Error::Input)?
                == originals["credential.norito"]
            && norito::encode_canonical(&activation.certificates).map_err(|_| Error::Input)?
                == originals["certificates.norito"]
            && norito::encode_canonical(&activation.bootstrap).map_err(|_| Error::Input)?
                == originals["bootstrap.norito"],
    )?;
    let action: KagemushaWalletLedgerV1 = canonical(&originals["issue-load.norito"])?;
    need(
        action
            == KagemushaWalletLedgerV1::new(
                scheme,
                Action::IssueLoad {
                    wallet: activation.credential.body.wallet_id,
                    asset: asset.asset_digest(),
                    ordinal: 0,
                    request_id: [201; 32],
                    amount: 100,
                    charge: None,
                },
            ),
    )?;
    need(
        receipt.scheme_id == scheme
            && receipt.wallet_id == activation.credential.body.wallet_id
            && receipt.asset_digest == asset.asset_digest()
            && receipt.payer_account_digest == activation.credential.body.account_digest
            && receipt.request_id == [201; 32]
            && receipt.ordinal == 0
            && receipt.amount == 100
            && receipt.online_charge == 0
            && receipt.charge_quote == [0; 32],
    )?;
    need(
        fixed::<32>(&json, "wallet_id")? == receipt.wallet_id
            && fixed::<32>(&json, "asset_digest")? == receipt.asset_digest
            && fixed::<32>(&json, "payer_account_digest")? == receipt.payer_account_digest
            && fixed::<32>(&json, "request_id")? == receipt.request_id
            && fixed::<32>(&json, "charge_quote")? == receipt.charge_quote
            && value(&json, "ordinal")?.as_str() == Some("0")
            && value(&json, "amount")?.as_str() == Some("100")
            && value(&json, "online_charge")?.as_str() == Some("0"),
    )?;
    need(pinned(selection.target, 16 << 10, selection.target_sha256)? == frame)?;
    Ok(Target { frame, originals })
}
