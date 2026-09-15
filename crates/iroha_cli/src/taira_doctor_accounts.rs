//! Mandatory signer-free wallet discovery checks for the Taira doctor.
use super::{DEFAULT_CHAIN_DISCRIMINANT, Duration, Result, Value, push_check};
use iroha::{account_bootstrap::DiscoveryHttpError, blocking::account_bootstrap::Client};

pub(super) const EXPECTED: [(&str, u64, Option<String>); 2] = [
    ("account_capabilities", 200, None),
    ("account_faucet_policy", 200, None),
];

pub(super) fn append_checks(
    public_root: &str,
    checks: &mut Vec<Value>,
    failures: &mut Vec<String>,
) -> Result<()> {
    let client = Client::new(public_root.parse()?, Duration::from_secs(30))?;
    let capabilities = match client.capabilities() {
        Ok(capabilities) if capabilities.network_prefix == DEFAULT_CHAIN_DISCRIMINANT => {
            push_check(checks, EXPECTED[0].0, 200, true, None);
            Some(capabilities)
        }
        Ok(_) => {
            record_failure(
                checks,
                failures,
                EXPECTED[0].0,
                Some(200),
                &eyre::eyre!(
                    "Taira wallet discovery requires address profile {DEFAULT_CHAIN_DISCRIMINANT}"
                ),
            );
            None
        }
        Err(error) => {
            record_failure(checks, failures, EXPECTED[0].0, None, &error);
            None
        }
    };
    let Some(capabilities) = capabilities else {
        let detail = "account_faucet_policy cannot be verified without the discovered exact Taira network identity".to_owned();
        push_check(checks, EXPECTED[1].0, 0, false, Some(detail.clone()));
        failures.push(detail);
        return Ok(());
    };
    let xor = iroha_wallet::operations::XOR_ASSET_DEFINITION
        .parse::<iroha::data_model::asset::AssetDefinitionId>()?;
    match client.faucet_policy(capabilities.network_id, capabilities.network_prefix) {
        Ok(policy) if policy.asset_definition_id == xor => {
            push_check(checks, EXPECTED[1].0, 200, true, None)
        }
        Ok(_) => record_failure(
            checks,
            failures,
            EXPECTED[1].0,
            Some(200),
            &eyre::eyre!(
                "Taira wallet funding requires the canonical XOR asset {}",
                iroha_wallet::operations::XOR_ASSET_DEFINITION
            ),
        ),
        Err(error) => record_failure(checks, failures, EXPECTED[1].0, None, &error),
    }
    Ok(())
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
