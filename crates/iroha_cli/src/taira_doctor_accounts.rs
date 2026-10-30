//! Mandatory signer-free wallet discovery and Parliament seating checks for the Taira doctor.
//!
//! The Parliament seating checks (`specs/sccp.md` §4.14.5) cover what signer-free public routes
//! can prove, and their names and details say so: `canonical_profile_seating` evaluates the
//! compiled canonical Taira `[gov]`/`[torii.faucet]` profile, and `canonical_bond_faucet_reach`
//! measures the live faucet amount against the compiled citizenship bond. Neither is evidence of
//! the live `[gov]` profile. `--parliament` also observes public readiness body sizes and the
//! active beacon/roster, while warning about seating and progress requirements the projection
//! cannot prove. These peer-reported diagnostics never authorize a Parliament launch.
use super::{
    DEFAULT_CHAIN_DISCRIMINANT, Duration, Result, Value, parliament_seating::SeatingProfile,
    push_check,
};
use eyre::WrapErr as _;
use iroha::{account_bootstrap::DiscoveryHttpError, blocking::account_bootstrap::Client};
use iroha_sccp::api::SccpGovernanceReadinessV1;
use std::io::Read as _;

/// Match the public SCCP SDK's response ceiling without loading a signing identity.
const PARLIAMENT_READINESS_MAX_BYTES: u64 = 16 * 1024 * 1024;
const PARLIAMENT_READINESS: &str = "parliament_readiness";
const PARLIAMENT_BEACON: &str = "parliament_global_beacon_session";

const ACCOUNT_CAPABILITIES: &str = "account_capabilities";
const ACCOUNT_FAUCET_POLICY: &str = "account_faucet_policy";
/// Live faucet amount against the compiled canonical bond; the live bond is not read.
const CANONICAL_BOND_FAUCET_REACH: &str = "canonical_bond_faucet_reach";
/// Static §4.14.5 rules over the compiled canonical profile; the live profile is not read.
const CANONICAL_PROFILE_SEATING: &str = "canonical_profile_seating";
/// Exact detail of a passing [`CANONICAL_BOND_FAUCET_REACH`] check.
pub(super) const CANONICAL_BOND_FAUCET_REACH_DETAIL: &str = "live faucet amount against the compiled canonical citizenship_bond_amount; the live bond is not verified";
/// Exact detail of a passing [`CANONICAL_PROFILE_SEATING`] check.
pub(super) const CANONICAL_PROFILE_SEATING_DETAIL: &str = "compiled canonical configs/soranexus/taira/config.toml [gov] and [torii.faucet] profile; the live profile is not verified";

/// The wallet-discovery and Parliament seating checks every doctor report carries, in order.
#[cfg(test)]
pub(super) fn expected() -> [(&'static str, u64, Option<String>); 4] {
    [
        (ACCOUNT_CAPABILITIES, 200, None),
        (ACCOUNT_FAUCET_POLICY, 200, None),
        (
            CANONICAL_BOND_FAUCET_REACH,
            200,
            Some(CANONICAL_BOND_FAUCET_REACH_DETAIL.to_owned()),
        ),
        (
            CANONICAL_PROFILE_SEATING,
            0,
            Some(CANONICAL_PROFILE_SEATING_DETAIL.to_owned()),
        ),
    ]
}

pub(super) fn append_checks(
    public_root: &str,
    checks: &mut Vec<Value>,
    failures: &mut Vec<String>,
) -> Result<()> {
    let client = Client::new(public_root.parse()?, Duration::from_secs(30))?;
    let capabilities = match client.capabilities() {
        Ok(capabilities) if capabilities.network_prefix == DEFAULT_CHAIN_DISCRIMINANT => {
            push_check(checks, ACCOUNT_CAPABILITIES, 200, true, None);
            Some(capabilities)
        }
        Ok(_) => {
            record_failure(
                checks,
                failures,
                ACCOUNT_CAPABILITIES,
                Some(200),
                &eyre::eyre!(
                    "Taira wallet discovery requires address profile {DEFAULT_CHAIN_DISCRIMINANT}"
                ),
            );
            None
        }
        Err(error) => {
            record_failure(checks, failures, ACCOUNT_CAPABILITIES, None, &error);
            None
        }
    };
    let Some(capabilities) = capabilities else {
        let detail = "account_faucet_policy cannot be verified without the discovered exact Taira network identity".to_owned();
        push_check(
            checks,
            ACCOUNT_FAUCET_POLICY,
            0,
            false,
            Some(detail.clone()),
        );
        failures.push(detail);
        append_parliament_checks(None, checks, failures);
        return Ok(());
    };
    let xor = iroha_wallet::operations::XOR_ASSET_DEFINITION
        .parse::<iroha::data_model::asset::AssetDefinitionId>()?;
    let live_amount =
        match client.faucet_policy(capabilities.network_id, capabilities.network_prefix) {
            Ok(policy) if policy.asset_definition_id == xor => {
                push_check(checks, ACCOUNT_FAUCET_POLICY, 200, true, None);
                Some(policy.amount)
            }
            Ok(_) => {
                record_failure(
                    checks,
                    failures,
                    ACCOUNT_FAUCET_POLICY,
                    Some(200),
                    &eyre::eyre!(
                        "Taira wallet funding requires the canonical XOR asset {}",
                        iroha_wallet::operations::XOR_ASSET_DEFINITION
                    ),
                );
                None
            }
            Err(error) => {
                record_failure(checks, failures, ACCOUNT_FAUCET_POLICY, None, &error);
                None
            }
        };
    append_parliament_checks(live_amount.as_ref(), checks, failures);
    Ok(())
}

/// Push the two canonical-profile Parliament seating checks.
///
/// Both judge the compiled canonical profile, so a passing check carries the exact detail that
/// says the live values are not verified. Without a verified live faucet policy the reach check
/// is reported as not verified; its root cause is the faucet-policy failure already recorded, so
/// no second failure is added.
fn append_parliament_checks(
    live_faucet_amount: Option<&iroha_primitives::numeric::Quantity>,
    checks: &mut Vec<Value>,
    failures: &mut Vec<String>,
) {
    let profile = match SeatingProfile::canonical() {
        Ok(profile) => profile,
        Err(error) => {
            let detail = format!("canonical Taira seating profile is malformed: {error:#}");
            push_check(
                checks,
                CANONICAL_BOND_FAUCET_REACH,
                0,
                false,
                Some(detail.clone()),
            );
            push_check(
                checks,
                CANONICAL_PROFILE_SEATING,
                0,
                false,
                Some(detail.clone()),
            );
            failures.push(detail);
            return;
        }
    };
    match live_faucet_amount {
        Some(amount) => {
            let (ok, detail) = profile.bond_beyond_faucet_reach(Some(amount));
            if ok {
                push_check(
                    checks,
                    CANONICAL_BOND_FAUCET_REACH,
                    200,
                    true,
                    Some(CANONICAL_BOND_FAUCET_REACH_DETAIL.to_owned()),
                );
            } else {
                let detail = format!("{CANONICAL_BOND_FAUCET_REACH}: {detail}");
                push_check(checks, CANONICAL_BOND_FAUCET_REACH, 200, false, Some(detail.clone()));
                failures.push(detail);
            }
        }
        None => push_check(
            checks,
            CANONICAL_BOND_FAUCET_REACH,
            0,
            false,
            Some(
                "canonical_bond_faucet_reach cannot be verified without the canonical faucet policy"
                    .to_owned(),
            ),
        ),
    }
    let failed = profile
        .requirements(None)
        .into_iter()
        .filter(|requirement| !requirement.ok)
        .map(|requirement| format!("{}: {}", requirement.name, requirement.detail))
        .collect::<Vec<_>>();
    if failed.is_empty() {
        push_check(
            checks,
            CANONICAL_PROFILE_SEATING,
            0,
            true,
            Some(CANONICAL_PROFILE_SEATING_DETAIL.to_owned()),
        );
    } else {
        let detail = format!("{}: {}", CANONICAL_PROFILE_SEATING, failed.join("; "));
        push_check(
            checks,
            CANONICAL_PROFILE_SEATING,
            0,
            false,
            Some(detail.clone()),
        );
        failures.push(detail);
    }
}

/// Read the public readiness projection using the doctor's bounded, signer-free HTTP client.
///
/// Body sizes are observations, not a seating verdict. The projection has no network identity,
/// so these values are labeled peer-reported and never used as launch authorization.
pub(super) fn append_live_parliament_checks(
    http: &super::HttpClient,
    public_root: &str,
    checks: &mut Vec<Value>,
    failures: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> Result<()> {
    warnings.extend(parliament_warnings()?);
    let url = super::join_url(
        public_root,
        iroha_torii_shared::route_catalog::sccp::GOVERNANCE_READINESS.path(),
    )?;
    let response = match http
        .get(url)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
    {
        Ok(response) => response,
        Err(error) => {
            record_failure(checks, failures, PARLIAMENT_READINESS, None, &error.into());
            return Ok(());
        }
    };
    let status = response.status().as_u16();
    if status != 200 {
        record_failure(
            checks,
            failures,
            PARLIAMENT_READINESS,
            Some(status),
            &eyre::eyre!("public readiness returned HTTP {status}; expected 200"),
        );
        return Ok(());
    }
    let readiness = match decode_parliament_readiness(response) {
        Ok(readiness) => readiness,
        Err(error) => {
            record_failure(checks, failures, PARLIAMENT_READINESS, Some(status), &error);
            return Ok(());
        }
    };
    push_check(
        checks,
        PARLIAMENT_READINESS,
        status,
        true,
        Some(format!(
            "peer-reported committed height {}; diagnostics do not authorize launch",
            readiness.committed_height
        )),
    );
    push_check(
        checks,
        "parliament_body_sizes",
        status,
        true,
        Some(format!(
            "peer-reported body sizes at committed height {}: {}; not a seating verdict without the qualified citizen census and live bond",
            readiness.committed_height,
            norito::json::to_json(&readiness.body_sizes)?
        )),
    );
    let beacon_ok =
        readiness.active_beacon_session.is_some() && readiness.beacon_matches_current_roster;
    let beacon_detail = format!(
        "{PARLIAMENT_BEACON}: peer-reported committed height {}; active_session={}; matches_current_roster={}",
        readiness.committed_height,
        readiness
            .active_beacon_session
            .map_or_else(|| "unavailable".to_owned(), hex::encode),
        readiness.beacon_matches_current_roster
    );
    push_check(
        checks,
        PARLIAMENT_BEACON,
        status,
        beacon_ok,
        Some(beacon_detail.clone()),
    );
    if !beacon_ok {
        failures.push(beacon_detail);
    }
    if !readiness.parliament_binding_ballot_available || !readiness.reconciliation_available {
        warnings.push(format!(
            "parliament_launch: unavailable in the peer-reported readiness observation; binding_ballot_available={}, reconciliation_available={}; diagnostics do not authorize launch",
            readiness.parliament_binding_ballot_available,
            readiness.reconciliation_available
        ));
    }
    Ok(())
}

/// Decode only a bounded canonical readiness DTO; malformed or absent fields remain failures.
fn decode_parliament_readiness(
    response: reqwest::blocking::Response,
) -> Result<SccpGovernanceReadinessV1> {
    if response
        .content_length()
        .is_some_and(|size| size > PARLIAMENT_READINESS_MAX_BYTES)
    {
        eyre::bail!("Parliament readiness response exceeds the byte limit");
    }
    let mut bytes = Vec::new();
    response
        .take(PARLIAMENT_READINESS_MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .wrap_err("failed to read Parliament readiness response")?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > PARLIAMENT_READINESS_MAX_BYTES {
        eyre::bail!("Parliament readiness response exceeds the byte limit");
    }
    norito::json::from_slice(&bytes).wrap_err("malformed Parliament readiness response")
}

/// Seating and progress requirements outside the current public readiness projection.
///
/// TODO(ws35): expose the qualified citizen census, live citizenship bond/escrow and invitation
/// window, and adaptive faucet policy/anchor age; then evaluate live seating and faucet reach.
/// The public readiness projection already supplies body sizes and the active beacon/roster.
/// TODO(ws42): report every active SCCP attempt with its next due checkpoint height, its last
/// progress height, and fail when an attempt is active and the tip is not growing.
pub(super) fn parliament_warnings() -> Result<Vec<String>> {
    let profile = SeatingProfile::canonical()?;
    let (body, size) = profile.largest_required_body();
    Ok(vec![
        format!(
            "parliament_eligible_citizens: not verified; public readiness does not supply the qualified citizen census (the canonical profile needs at least {} citizens bonded at {} XOR; largest body {body}={size})",
            profile.minimum_citizens(),
            profile.citizenship_bond()
        ),
        "parliament_live_profile: not verified; the live citizenship bond/escrow and invitation window require account-signed /v1/gov/capabilities, while the adaptive faucet policy/anchor age are outside public readiness (canonical_profile_seating and canonical_bond_faucet_reach judge the compiled canonical profile)".to_owned(),
        "parliament_attempt_progress: not verified; a single readiness observation does not verify complete next-due checkpoints or tip progress; diagnostics do not authorize launch".to_owned(),
    ])
}

fn record_failure(
    checks: &mut Vec<Value>,
    failures: &mut Vec<String>,
    name: &str,
    observed_status: Option<u16>,
    error: &eyre::Report,
) {
    let status = observed_status
        .or_else(|| {
            error
                .downcast_ref::<DiscoveryHttpError>()
                .map(|failure| failure.status)
        })
        .unwrap_or(0);
    let detail = format!("{name}: {error:#}");
    push_check(checks, name, status, false, Some(detail.clone()));
    failures.push(detail);
}
