//! Public Refresh action across loss of a prepared review and of a successful execute reply.
//! This runs only on a private copy of an already funded host fixture.
use super::super::*;
use iroha_data_model::kagemusha::{
    KagemushaDevicePublicKeyV1, KagemushaWalletSignerCertificateBodyV1,
    KagemushaWalletSignerCertificateV1, KagemushaWalletSignerRoleV1,
    KagemushaWalletTimeAnchorBodyV1, KagemushaWalletTimeAnchorV1,
};
use p256::ecdsa::SigningKey;

fn signed_originals(wallet: [u8; 32]) -> Result<(KagemushaWalletTimeAnchorV1, Vec<u8>)> {
    let scheme = native::device_fixture::scheme();
    let root = SigningKey::from_slice(&[0x11; 32]).map_err(fail)?;
    let key = SigningKey::from_slice(&[0x56; 32]).map_err(fail)?;
    let body = KagemushaWalletSignerCertificateBodyV1 {
        version: 1,
        scheme_id: scheme.scheme_id(),
        role: KagemushaWalletSignerRoleV1::TimeAnchor,
        key: KagemushaDevicePublicKeyV1::from_sec1_bytes(
            key.verifying_key().to_sec1_point(false).as_bytes(),
        )
        .map_err(fail)?,
        serial: 2,
    };
    let signature = super::local_blacklist::signed(&root, &body.signing_message());
    let issuer =
        KagemushaWalletSignerCertificateV1::sign(body, &scheme, signature).map_err(fail)?;
    let body = KagemushaWalletTimeAnchorBodyV1 {
        version: 1,
        scheme_id: scheme.scheme_id(),
        wallet_id: wallet,
        nonce: [0x69; 32],
        issuer_time_ms: 1_800_000_000_000,
        signer_certificate: issuer.certificate_digest(),
    };
    let signature = super::local_blacklist::signed(&key, &body.signing_message());
    let original = KagemushaWalletTimeAnchorV1::sign(body, &issuer, signature).map_err(fail)?;
    Ok((original, norito::encode_canonical(&issuer).map_err(fail)?))
}

pub(in super::super) fn check(
    root: &Path,
    runtime: &mut SessionAdapter,
    slot: &[u8; 32],
    wallet: [u8; 32],
    stage: &str,
) -> Result<String> {
    let (anchor, issuer) = signed_originals(wallet)?;
    let original = anchor.to_canonical_bytes().map_err(fail)?;
    let command = wallet_action(7, &[&original, &issuer])?;
    let review_file = root.join("public-refresh-review.bin");
    let completion_file = root.join("public-refresh-completion.norito");
    let before = runtime.output(slot, &[0; 32], 3)?;
    if before.status != 101 || before.bytes.len() != 196 || before.bytes[2] != 3 {
        return Err("public Refresh needs an existing Released fixture wallet".into());
    }
    if matches!(stage, "refresh-prepare" | "refresh-recover") {
        let mut changed = anchor;
        changed.body.issuer_time_ms += 1;
        let mut other_wallet = wallet;
        other_wallet[0] ^= 1;
        let (foreign, _) = signed_originals(other_wallet)?;
        for (label, rejected) in [
            ("changed issuer time", changed),
            ("foreign wallet", foreign),
        ] {
            let bytes = rejected.to_canonical_bytes().map_err(fail)?;
            if runtime
                .prepare(slot, &wallet_action(7, &[&bytes, &issuer])?, &[])
                .is_ok()
            {
                return Err(format!("public Refresh accepted {label}"));
            }
        }
        if runtime.output(slot, &[0; 32], 3)?.bytes != before.bytes {
            return Err("rejected Refresh inputs moved the released head".into());
        }
    }
    let previous_sequence = u128::from_le_bytes(before.bytes[132..148].try_into().unwrap());
    let prepared = runtime.prepare(slot, &command, &[])?;
    let review = &prepared.review;
    if review.len() != 243
        || review[..3] != [2, 0, 7]
        || review[3..35] != anchor.body.scheme_id
        || review[35..67] != wallet
        || review[131..163] != anchor.time_anchor_digest()
        || review[163..171] != anchor.body.issuer_time_ms.to_le_bytes()
        || review[171..179] != anchor.body.issuer_time_ms.to_le_bytes()
        || review[179..211] == review[211..243]
    {
        return Err("public Refresh review differs from the signed update or wallet".into());
    }
    let op: [u8; 32] = review[67..99].try_into().expect("fixed Refresh review");
    if stage != "refresh-recover" && before.bytes[100..132] != review[179..211] {
        return Err("public Refresh does not follow the current released head".into());
    }
    match stage {
        "refresh-prepare" => {
            if review_file.exists() || completion_file.exists() {
                return Err("public Refresh needs an unused host fixture copy".into());
            }
            if runtime.output(slot, &[0; 32], 3)?.bytes != before.bytes {
                return Err("preparing a public Refresh moved the released wallet head".into());
            }
            save_fixture_input(&review_file, review)?;
        }
        "refresh-execute" | "refresh-recover" => {
            if *review != std::fs::read(&review_file).map_err(fail)? {
                return Err("cold public Refresh changed the exact review".into());
            }
            if stage == "refresh-recover" {
                let saved = std::fs::read(&completion_file).map_err(fail)?;
                if before.bytes[68..100] != op || runtime.output(slot, &op, 2)?.bytes != saved {
                    return Err("cold public Refresh lost the original completion".into());
                }
            }
            let result = runtime.execute(prepared.token)?;
            if result.status != 1 || result.bytes.is_empty() {
                return Err(format!(
                    "public Refresh execution returned status {}",
                    result.status
                ));
            }
            let selected = runtime.output(slot, &[0; 32], 3)?;
            let retained = runtime.output(slot, &op, 2)?;
            if selected.status != 101
                || selected.bytes.len() != 196
                || selected.bytes[68..100] != op
                || retained.status != 1
                || retained.bytes != result.bytes
            {
                return Err("published public Refresh differs from the selected completion".into());
            }
            native::CompletionRecord::decode(&result.bytes, &wallet).map_err(fail)?;
            if stage == "refresh-execute" {
                let sequence = u128::from_le_bytes(selected.bytes[132..148].try_into().unwrap());
                if completion_file.exists()
                    || Some(sequence) != previous_sequence.checked_add(1)
                    || selected.bytes[100..132] != review[211..243]
                    || before.bytes[148..196] != selected.bytes[148..196]
                {
                    return Err(
                        "public Refresh did not advance once and preserve spendable value".into(),
                    );
                }
                save_fixture_input(&completion_file, &result.bytes)?;
            } else if before.bytes != selected.bytes
                || result.bytes != std::fs::read(&completion_file).map_err(fail)?
            {
                return Err("retry moved the head or changed the public Refresh completion".into());
            }
        }
        _ => return Err("unknown public Refresh host fixture phase".into()),
    }
    if runtime.next_load_ordinal(slot)? != 1 {
        return Err("public Refresh changed the funded fixture's Load ordinal".into());
    }
    runtime.close()?;
    Ok(format!(
        "Public Refresh {stage}: verified {}-byte signed anchor and {}-byte review",
        original.len(),
        review.len()
    ))
}
