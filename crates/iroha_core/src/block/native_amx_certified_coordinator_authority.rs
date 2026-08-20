//! Certified-coordinator authority checks for historical Native AMX bundles.

use super::*;

pub(super) fn validate_historical_native_amx_certified_coordinator_authority(
    bundle: &crate::kura::AutonomousLaneMergeBundleV1,
    authenticated_committee: &[PeerId],
    key_authority: &dyn NativeAmxAuthorityContext,
) -> Result<(), String> {
    let descriptor = &bundle.certified.proposal.descriptor;
    let mut authoritative_validators = authenticated_committee.to_vec();
    authoritative_validators.sort();
    authoritative_validators.dedup();
    authoritative_validators.retain(|peer| {
        peer.public_key().try_algorithm().ok() == Some(iroha_crypto::Algorithm::BlsNormal)
    });
    if authoritative_validators.is_empty() || descriptor.validator_set != authoritative_validators {
        return Err(
            "historical native AMX certified coordinator committee is not authoritative".to_owned(),
        );
    }
    let availability = bundle
        .certified
        .prepare_qc
        .payload_availability_qc
        .as_ref()
        .ok_or_else(|| {
            "historical native AMX certified coordinator lacks availability authority".to_owned()
        })?;
    for (validator, pop) in availability
        .validator_set
        .iter()
        .zip(&availability.validator_set_pops)
    {
        if !key_authority.consensus_pop_matches_authority(
            descriptor.lane_id,
            validator,
            descriptor.proposal_height,
            pop,
        ) {
            return Err(
                "historical native AMX certified coordinator availability PoP is not authoritative"
                    .to_owned(),
            );
        }
    }
    for (public_key, pop) in &bundle.certified.signer_pops {
        let Some(validator) = descriptor
            .validator_set
            .iter()
            .find(|validator| validator.public_key() == public_key)
        else {
            return Err(
                "historical native AMX certified coordinator signer is outside its committee"
                    .to_owned(),
            );
        };
        if !key_authority.consensus_pop_matches_authority(
            descriptor.lane_id,
            validator,
            descriptor.proposal_height,
            pop,
        ) {
            return Err(
                "historical native AMX certified coordinator signer PoP is not authoritative"
                    .to_owned(),
            );
        }
    }
    Ok(())
}
