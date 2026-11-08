//! Exact semantic originals from selected releases and an independent anchor credit tree.
//!
//! The coordinator authenticates the indexed range, refuses collection, and pins each result.
//! This builder checks custody/model bindings; only the shared Native verifier checks sigma,
//! payer Omega and the complete suffix. It never publishes a live map or grants proof authority.

use super::*;
use crate::kagemusha_wallet_advance_v1::kagemusha_wallet_archive_object_digest_v1 as object_digest;
use crate::kagemusha_wallet_state_v1::{
    credit_tree::CreditTree, preparation_custody::IssuedRequestCustodyV1,
};

/// Build one exact step using only an already source-selected release and local custody.
/// `credit_tree` is the independent clone selected with the anchor, never the live manifest.
/// Failed reads, decoding, validation or encoding leave that clone unchanged. Staged immutable
/// map nodes may remain unreachable after a failed attempt; no selected root is changed.
#[expect(
    clippy::too_many_arguments,
    reason = "separate selected originals and independent map"
)]
pub(super) fn step_original(
    store: &mut impl ObjectWorkspace,
    released: &ReleasedStep,
    before: &KagemushaWalletStateV1,
    issued_requests: &IndexRoot,
    ordinary_charge: Option<&ChargeOriginalsV1>,
    scheme: &KagemushaWalletSchemeV1,
    credit_tree: &mut CreditTree,
) -> Result<Vec<u8>, Error> {
    released.verify_completion()?;
    let capsule = released.capsule();
    let body = capsule.body();
    valid(before.validate_for_credential(released.credential()))?;
    valid(body.statement.validate_for_scheme(scheme))?;
    if body.scheme_id != scheme.scheme_id()
        || body.wallet_id != before.core.wallet_id
        || body.statement.predecessor != valid(before.commitment())?
        || body.statement.successor != valid(body.successor_state.commitment())?
        || before.core.sequence.checked_add(1) != Some(body.statement.sequence)
    {
        return Err(Error::WitnessLost("suffix selected step joins"));
    }

    let mut next_tree = credit_tree.clone();
    let (kind, originals) = match body.statement.effect {
        KagemushaWalletEffectV1::Receive { .. } => {
            if released.suffix().is_some() || ordinary_charge.is_some() {
                return Err(Error::WitnessLost("suffix Receive source schema"));
            }
            use KagemushaWalletRetainedInputRoleV1 as R;
            let payment_original = retained(body.retained_inputs, R::Payment)?;
            let payer_original = retained(body.retained_inputs, R::Credential)?;
            let certificates_original = retained(body.retained_inputs, R::CertificateSet)?;
            let payment = read_model_original(
                || KagemushaWalletPaymentV1::decode_canonical(payment_original, &body.scheme_id),
                Error::WitnessLost("suffix retained Payment"),
            )?;
            let digests = valid(payment.digests())?;
            let (request, request_original) = issued_request(
                store,
                issued_requests,
                &body.scheme_id,
                &body.wallet_id,
                &digests.request,
            )?;
            let payer = read_model_original(
                || KagemushaWalletCredentialV1::decode_canonical(payer_original, &body.scheme_id),
                Error::WitnessLost("suffix retained payer credential"),
            )?;
            let certificates: KagemushaWalletCertificateSetV1 =
                archive::decode(certificates_original)?;
            let prepared = valid(payment.prepare_receive(
                scheme,
                &payer,
                &certificates,
                &request,
                released.credential(),
                before,
                None,
            ))?;
            if prepared.effect != body.statement.effect
                || prepared.digests.payment != body.payment_digest
                || prepared.digests.payment != released.receipt().payment_digest
            {
                return Err(Error::WitnessLost("suffix retained Receive effect"));
            }
            let credit = record_credit(
                store,
                &mut next_tree,
                &KagemushaWalletCreditDigestLeafV1 {
                    credit_id: prepared.digests.credit_id,
                    payment_digest: prepared.digests.payment,
                    burned: false,
                },
            )?;
            use KagemushaWalletSuffixOriginalRoleV1 as O;
            let originals = [
                (O::Request, request_original),
                (O::Payment, payment_original.to_vec()),
                (O::PayerCredential, payer_original.to_vec()),
                (O::PayerCertificates, certificates_original.to_vec()),
            ]
            .into_iter()
            .map(|(role, bytes)| KagemushaWalletSuffixOriginalV1 { role, bytes })
            .collect();
            (
                KagemushaWalletSuffixStepKindV1::Receive { credit },
                originals,
            )
        }
        KagemushaWalletEffectV1::Unload { .. } => {
            let selected_charge = match released.suffix() {
                Some((frozen, _)) => {
                    if ordinary_charge.is_some() {
                        return Err(Error::WitnessLost("suffix Unload charge source schema"));
                    }
                    let original = &frozen.capsule.body().authority_header;
                    let header = read_model_original(
                        || {
                            KagemushaWalletSuffixHeaderV1::decode_canonical(
                                original,
                                &body.scheme_id,
                            )
                        },
                        Error::WitnessLost("suffix prior authority header"),
                    )?;
                    if header.statement != body.statement {
                        return Err(Error::WitnessLost("suffix prior authority statement"));
                    }
                    header
                        .charge
                        .quote()
                        .map(|quote| {
                            // Header decode requires canonical encoding, so this is exactly its
                            // signed quote's canonical original, without a new signature.
                            Ok::<_, Error>(ChargeOriginalsV1 {
                                quote: valid(quote.to_canonical_bytes())?,
                                certificates: frozen.certificates.clone(),
                            })
                        })
                        .transpose()?
                }
                None => ordinary_charge.cloned(),
            };
            let charge = checked_charge(scheme, before, &body.statement, selected_charge.as_ref())?;
            (
                KagemushaWalletSuffixStepKindV1::PriorUnload {
                    proof_digest: valid(capsule.proof_digest())?,
                    charge,
                },
                Vec::new(),
            )
        }
        // Classification happens before pinning. An unexpected kind here is an authenticated
        // source mismatch, never a request to silently select a different payment path.
        _ => return Err(Error::WitnessLost("unsupported selected suffix step")),
    };
    let step = KagemushaWalletSuffixStepV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        kind,
        before: *before,
        after: body.successor_state,
        statement: body.statement,
        sigma: body.step_proof.clone(),
        receipt: *released.receipt(),
        map_openings: body.map_openings.clone(),
        originals,
    };
    let original = valid(step.to_canonical_bytes())?;
    *credit_tree = next_tree;
    Ok(original)
}

fn retained(
    inputs: &[KagemushaWalletRetainedInputV1],
    role: KagemushaWalletRetainedInputRoleV1,
) -> Result<&[u8], Error> {
    let mut matches = inputs.iter().filter(|input| input.role == role);
    let value = matches
        .next()
        .ok_or(Error::WitnessLost("suffix retained input missing"))?;
    let maximum = match role {
        KagemushaWalletRetainedInputRoleV1::Credential => KAGEMUSHA_WALLET_CREDENTIAL_MAX_BYTES_V1,
        KagemushaWalletRetainedInputRoleV1::Payment
        | KagemushaWalletRetainedInputRoleV1::CertificateSet => {
            KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1
        }
        _ => return Err(Error::Invalid("suffix retained input role")),
    };
    if matches.next().is_some() || value.bytes.is_empty() || value.bytes.len() > maximum {
        return Err(Error::WitnessLost(
            "suffix retained input uniqueness or bound",
        ));
    }
    Ok(&value.bytes)
}

fn issued_request(
    store: &mut impl ObjectWorkspace,
    issued_requests: &IndexRoot,
    scheme: &[u8; 32],
    wallet: &[u8; 32],
    digest: &[u8; 32],
) -> Result<(KagemushaWalletRequestV1, Vec<u8>), Error> {
    let value = issued_requests
        .get(store, digest)?
        .ok_or(Error::WitnessLost("suffix issued Request custody"))?;
    let record: IssuedRequestCustodyV1 = archive::decode(&value)?;
    let result = record.read(store, scheme, wallet, digest)?;
    if object_digest(&result.1) != record.request {
        return Err(Error::WitnessLost(
            "suffix issued Request original identity",
        ));
    }
    Ok(result)
}

fn record_credit(
    store: &mut impl ObjectWorkspace,
    tree: &mut CreditTree,
    credit: &KagemushaWalletCreditDigestLeafV1,
) -> Result<KagemushaWalletSuffixCreditWitnessV1, Error> {
    // A prior first identity remains first, including a prior burn. Newly recorded Receive
    // credits always use burned=false; the shared verifier enforces the same branch.
    if credit.burned {
        return Err(Error::Invalid("suffix new credit burn"));
    }
    Ok(match tree.record(store, credit)? {
        KagemushaWalletCreditDigestRecordV1::Present { leaf, opening } => {
            KagemushaWalletSuffixCreditWitnessV1::Present {
                membership: opening.leaf_transcript(&leaf),
            }
        }
        KagemushaWalletCreditDigestRecordV1::Inserted { witness } => {
            KagemushaWalletSuffixCreditWitnessV1::Inserted {
                low: witness.low_opening.leaf_transcript(&witness.low),
                empty: witness.slot_opening.empty_transcript(),
            }
        }
    })
}

fn checked_charge(
    scheme: &KagemushaWalletSchemeV1,
    before: &KagemushaWalletStateV1,
    statement: &KagemushaWalletStatementV1,
    charge: Option<&ChargeOriginalsV1>,
) -> Result<Option<KagemushaWalletSuffixChargeOriginalsV1>, Error> {
    let KagemushaWalletEffectV1::Unload {
        redeem_ordinal,
        amount,
        online_charge,
        charge_quote,
        ..
    } = statement.effect
    else {
        return Err(Error::WitnessLost("suffix prior Unload effect"));
    };
    let Some(charge) = charge else {
        if charge_quote != [0; 32] || online_charge != 0 {
            return Err(Error::WitnessLost("suffix prior Unload charge missing"));
        }
        return Ok(None);
    };
    if charge.quote.is_empty()
        || charge.quote.len() > KAGEMUSHA_WALLET_CHARGE_QUOTE_MAX_BYTES_V1
        || charge.certificates.is_empty()
        || charge.certificates.len() > KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1
    {
        return Err(Error::WitnessLost("suffix prior Unload charge bound"));
    }
    let quote = read_model_original(
        || KagemushaWalletChargeQuoteV1::decode_canonical(&charge.quote, &scheme.scheme_id()),
        Error::WitnessLost("suffix prior Unload quote"),
    )?;
    let certificates: KagemushaWalletCertificateSetV1 = archive::decode(&charge.certificates)?;
    valid(certificates.verify(scheme))?;
    let signer = valid(certificates.certificate(
        &quote.body.signer_certificate,
        KagemushaWalletSignerRoleV1::RegulatoryPolicy,
    ))?;
    valid(quote.verify(scheme, signer))?;
    valid(quote.require_terms(
        KagemushaWalletChargeKindV1::Unload,
        &before.core.wallet_id,
        redeem_ordinal,
        amount,
        online_charge,
    ))?;
    if quote.charge_quote_digest() != charge_quote
        || quote.body.asset_digest != before.core.asset_digest
    {
        return Err(Error::WitnessLost("suffix prior Unload quote binding"));
    }
    Ok(Some(KagemushaWalletSuffixChargeOriginalsV1 {
        quote: charge.quote.clone(),
        certificate_set: charge.certificates.clone(),
    }))
}

#[cfg(test)]
mod tests;
