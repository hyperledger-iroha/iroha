//! Exercise both regulatory checks on the host's real prepared Send and Receive.
use super::super::*;
use iroha_data_model::kagemusha::*;
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};

pub(super) fn signed(key: &SigningKey, message: &[u8; 32]) -> KagemushaWalletSignerOutputV1<'static> {
    let signature: Signature = key.sign(message);
    KagemushaWalletSignerOutputV1::Raw(signature.to_bytes().into())
}

fn install(
    runtime: &mut SessionAdapter,
    slot: &[u8; 32],
    account: Option<[u8; 32]>,
    version: u64,
) -> Result<()> {
    let scheme = native::device_fixture::scheme();
    // These deterministic keys belong only to the isolated public device fixture.
    let root = SigningKey::from_slice(&[0x11; 32]).map_err(fail)?;
    let regulator = SigningKey::from_slice(&[0x3a; 32]).map_err(fail)?;
    let cert = KagemushaWalletSignerCertificateBodyV1 {
        version: 1,
        scheme_id: scheme.scheme_id(),
        role: KagemushaWalletSignerRoleV1::RegulatoryPolicy,
        key: KagemushaDevicePublicKeyV1::from_sec1_bytes(
            regulator.verifying_key().to_sec1_point(false).as_bytes(),
        )
        .map_err(fail)?,
        serial: 1,
    };
    let signature = signed(&root, &cert.signing_message());
    let cert = KagemushaWalletSignerCertificateV1::sign(cert, &scheme, signature).map_err(fail)?;
    let entries: Vec<_> = account
        .into_iter()
        .map(|account_digest| KagemushaWalletBlacklistEntryV1 { account_digest })
        .collect();
    let body = KagemushaWalletBlacklistBodyV1 {
        version: 1,
        scheme_id: scheme.scheme_id(),
        list_version: version,
        issued_at_ms: 1,
        entry_count: entries.len() as u32,
        entries_root: kagemusha_wallet_blacklist_root_v1(&entries).map_err(fail)?,
        signer_certificate: cert.certificate_digest(),
    };
    let signature = signed(&regulator, &body.signing_message());
    let list = KagemushaWalletBlacklistV1::sign(body, entries, &cert, signature).map_err(fail)?;
    let original = list.to_canonical_bytes().map_err(fail)?;
    let certificates =
        norito::encode_canonical(&KagemushaWalletCertificateSetV1::new(vec![cert]).map_err(fail)?)
            .map_err(fail)?;
    if runtime.install_blacklist(slot, &original, &certificates)? != list.blacklist_digest() {
        return Err("wrong C installed regulatory digest".into());
    }
    Ok(())
}

fn require_blocked<T>(reply: Result<T>) -> Result<()> {
    match reply {
        Err(error) if error.contains("status=-15,") => Ok(()),
        Err(error) => Err(error),
        Ok(_) => Err("regulatory list did not refuse its named counterparty".into()),
    }
}

pub(in super::super) fn check(
    runtime: &mut SessionAdapter,
    slot: &[u8; 32],
    token: u64,
    witness: &[u8],
    head: &[u8],
    account: [u8; 32],
    version: u64,
) -> Result<()> {
    install(runtime, slot, Some(account), version)?;
    require_blocked(runtime.prepare(slot, witness, head))?;
    require_blocked(runtime.execute(token))?;
    install(runtime, slot, None, version + 1)?;
    require_blocked(runtime.execute(token))?;
    if install(runtime, slot, Some(account), version).is_ok() {
        return Err("regulatory list accepted a signed rollback".into());
    }
    Ok(())
}
