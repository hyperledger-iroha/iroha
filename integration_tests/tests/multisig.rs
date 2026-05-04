#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration tests validating multi-signature transaction flows.

use std::{
    collections::BTreeMap,
    num::{NonZeroU16, NonZeroU64},
    path::Path,
    time::{Duration, Instant},
};

use eyre::{Result, WrapErr, eyre};
use integration_tests::{
    binary_resolver::{iroha_program, prepare_iroha_cli_test_environment},
    sandbox,
};
use iroha::{
    client::{Client, MultisigApprovalEntry, MultisigApprovalsListRequest},
    config::DEFAULT_TRANSACTION_TIME_TO_LIVE,
    crypto::{ExposedPrivateKey, KeyPair},
    data_model::{
        Level,
        account::{MultisigMember, MultisigPolicy},
        isi::AddSignatory,
        prelude::*,
    },
    executor_data_model::isi::multisig::*,
};
use iroha_test_network::*;
use iroha_test_samples::{
    ALICE_ID, BOB_ID, BOB_KEYPAIR, CARPENTER_ID, CARPENTER_KEYPAIR, gen_account_in, load_sample_ivm,
};
use iroha_torii::{
    MultisigAccountSelectorDto, MultisigCancelRequestDto, MultisigProposalsGetRequestDto,
    MultisigProposalsListRequestDto,
};
use norito::json::Value as JsonValue;
use reqwest::header::CONTENT_TYPE;
use tokio::runtime::Runtime;

const DOMAIN_REGISTRATION_RECOVERY_TIMEOUT: Duration = Duration::from_secs(60);
const DOMAIN_REGISTRATION_RECOVERY_POLL: Duration = Duration::from_millis(250);

fn start_network(
    builder: NetworkBuilder,
    context: &'static str,
) -> Option<(sandbox::SerializedNetwork, Runtime)> {
    prepare_iroha_cli_test_environment();
    sandbox::start_network_blocking_or_skip(
        builder.with_peer_startup_timeout(Duration::from_secs(300)),
        context,
    )
    .unwrap()
}

fn multisig_supported(_client: &Client) -> bool {
    // Multisig instructions are carried via `CustomInstruction` envelope and are
    // executed by the core runtime path; they are not guaranteed to appear as
    // dedicated identifiers in `FindExecutorDataModel`.
    true
}

fn upgrade_executor(client: &Client, executor: impl AsRef<str>) -> Result<()> {
    let upgrade_executor = Upgrade::new(Executor::new(load_sample_ivm(executor)));
    client
        .submit_blocking(upgrade_executor)
        .wrap_err("Have you set IvmFuelConfig::Auto?")?;
    Ok(())
}

fn is_inconclusive_domain_registration_error(err: &eyre::Report) -> bool {
    const NEEDLES: [&str; 4] = [
        "haven't got tx confirmation within",
        "transaction queued for too long",
        "fallback status check failed",
        "operation timed out",
    ];
    err.chain().any(|cause| {
        let text = cause.to_string();
        NEEDLES.iter().any(|needle| text.contains(needle))
    })
}

fn domain_visible(client: &Client, domain: &DomainId) -> Result<bool> {
    Ok(client
        .query(FindDomains::new())
        .execute_all()?
        .into_iter()
        .any(|registered| registered.id() == domain))
}

fn wait_for_domain_visibility(
    client: &Client,
    domain: &DomainId,
    timeout: Duration,
) -> Result<bool> {
    let deadline = Instant::now() + timeout;
    let mut last_err = None;

    loop {
        match domain_visible(client, domain) {
            Ok(true) => return Ok(true),
            Ok(false) => {}
            Err(err) => last_err = Some(err),
        }

        if Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(DOMAIN_REGISTRATION_RECOVERY_POLL);
    }

    if let Some(err) = last_err {
        Err(err).wrap_err_with(|| {
            format!("timed out waiting for multisig test domain `{domain}` visibility")
        })
    } else {
        Ok(false)
    }
}

fn register_runtime_domain(network: &Network, client: &Client, domain: &DomainId) -> Result<()> {
    let register_domain =
        || submit_register_domain_with_network_lease(network, client, Domain::new(domain.clone()));
    match register_domain() {
        Ok(()) => Ok(()),
        Err(err) if is_inconclusive_domain_registration_error(&err) => {
            if wait_for_domain_visibility(client, domain, DOMAIN_REGISTRATION_RECOVERY_TIMEOUT)? {
                return Ok(());
            }

            let retry = client.submit_blocking(Register::domain(Domain::new(domain.clone())));
            match retry {
                Ok(_) => Ok(()),
                Err(retry_err)
                    if wait_for_domain_visibility(
                        client,
                        domain,
                        DOMAIN_REGISTRATION_RECOVERY_TIMEOUT,
                    )? =>
                {
                    Ok(())
                }
                Err(retry_err) => Err(retry_err),
            }
        }
        Err(err) => Err(err),
    }
    .wrap_err_with(|| format!("register multisig test domain `{domain}`"))
}

fn register_runtime_domain_and_transfer_to_bob(
    network: &Network,
    client: &Client,
    domain: &DomainId,
) -> Result<()> {
    register_runtime_domain(network, client, domain)?;
    client
        .submit_blocking(Transfer::domain(
            ALICE_ID.clone(),
            domain.clone(),
            BOB_ID.clone(),
        ))
        .wrap_err_with(|| format!("transfer multisig test domain `{domain}` to bob"))?;
    Ok(())
}

fn canonical_multisig_account_id(spec: &MultisigSpec) -> AccountId {
    let members = spec
        .signatories
        .iter()
        .map(|(account, weight)| {
            let signatory = account
                .controller()
                .single_signatory()
                .expect("multisig signatories must remain single-key accounts");
            MultisigMember::new(signatory.clone(), u16::from(*weight))
                .expect("multisig member should derive from valid spec")
        })
        .collect();
    let policy =
        MultisigPolicy::new(spec.quorum.get(), members).expect("multisig policy should derive");
    AccountId::new_multisig(policy)
}

fn post_torii_app_json<T: norito::json::JsonSerialize + ?Sized>(
    rt: &Runtime,
    endpoint: &str,
    body: &T,
) -> Result<JsonValue> {
    let payload = norito::json::to_vec(body)?;
    let response_body = rt.block_on(async {
        let response = reqwest::Client::new()
            .post(endpoint)
            .header(CONTENT_TYPE, "application/json")
            .body(payload)
            .send()
            .await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(eyre!(
                "HTTP status {status} for `{endpoint}` with body: {body}"
            ));
        }
        Ok(body)
    })?;
    norito::json::from_str(&response_body).map_err(Into::into)
}

fn wait_for_multisig_proposal_status(
    rt: &Runtime,
    torii_base: &str,
    selector: &MultisigAccountSelectorDto,
    proposal_id: &str,
    expected_status: &str,
) -> Result<JsonValue> {
    let endpoint = format!("{torii_base}/v1/multisig/proposals/get");
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut last_status = None;
    let mut last_error = None;

    while Instant::now() < deadline {
        match post_torii_app_json(
            rt,
            &endpoint,
            &MultisigProposalsGetRequestDto {
                selector: selector.clone(),
                proposal_id: Some(proposal_id.to_owned()),
                instructions_hash: None,
            },
        ) {
            Ok(payload) => {
                let status = payload
                    .get("status")
                    .and_then(JsonValue::as_str)
                    .map(ToOwned::to_owned);
                if status.as_deref() == Some(expected_status) {
                    return Ok(payload);
                }
                last_status = status;
                last_error = None;
            }
            Err(err) => last_error = Some(err),
        }

        std::thread::sleep(Duration::from_millis(250));
    }

    if let Some(status) = last_status {
        Err(eyre!(
            "timed out waiting for multisig proposal `{proposal_id}` status `{expected_status}`; last status `{status}`"
        ))
    } else if let Some(err) = last_error {
        Err(err).wrap_err_with(|| {
            format!(
                "timed out waiting for multisig proposal `{proposal_id}` status `{expected_status}`"
            )
        })
    } else {
        Err(eyre!(
            "timed out waiting for multisig proposal `{proposal_id}` status `{expected_status}`"
        ))
    }
}

fn wait_for_multisig_cancel_action(
    rt: &Runtime,
    torii_base: &str,
    selector: &MultisigAccountSelectorDto,
    signer_account_id: &AccountId,
    proposal_id: &str,
    expected_action: &str,
) -> Result<JsonValue> {
    let endpoint = format!("{torii_base}/v1/multisig/cancel");
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut last_action = None;
    let mut last_error = None;

    while Instant::now() < deadline {
        match post_torii_app_json(
            rt,
            &endpoint,
            &MultisigCancelRequestDto {
                selector: selector.clone(),
                signer_account_id: signer_account_id.clone(),
                private_key: None,
                public_key_hex: None,
                signature_b64: None,
                creation_time_ms: None,
                fee_sponsor: None,
                proposal_id: Some(proposal_id.to_owned()),
                instructions_hash: None,
            },
        ) {
            Ok(payload) => {
                let action = payload
                    .get("action")
                    .and_then(JsonValue::as_str)
                    .map(ToOwned::to_owned);
                if action.as_deref() == Some(expected_action) {
                    return Ok(payload);
                }
                last_action = action;
                last_error = None;
            }
            Err(err) => last_error = Some(err),
        }

        std::thread::sleep(Duration::from_millis(250));
    }

    if let Some(action) = last_action {
        Err(eyre!(
            "timed out waiting for multisig cancel action `{expected_action}`; last action `{action}`"
        ))
    } else if let Some(err) = last_error {
        Err(err).wrap_err_with(|| {
            format!("timed out waiting for multisig cancel action `{expected_action}`")
        })
    } else {
        Err(eyre!(
            "timed out waiting for multisig cancel action `{expected_action}`"
        ))
    }
}

fn cli_envs_for_signatory(
    client: &Client,
    account_domain: &DomainId,
    key_pair: &KeyPair,
) -> Vec<(&'static str, String)> {
    let ttl = client
        .transaction_ttl
        .unwrap_or(DEFAULT_TRANSACTION_TIME_TO_LIVE);
    vec![
        ("CHAIN", client.chain.to_string()),
        ("TORII_URL", client.torii_url.to_string()),
        ("ACCOUNT_DOMAIN", account_domain.to_string()),
        ("ACCOUNT_PUBLIC_KEY", key_pair.public_key().to_string()),
        (
            "ACCOUNT_PRIVATE_KEY",
            ExposedPrivateKey(key_pair.private_key().clone()).to_string(),
        ),
        (
            "TRANSACTION_STATUS_TIMEOUT_MS",
            client.transaction_status_timeout.as_millis().to_string(),
        ),
        ("TRANSACTION_TIME_TO_LIVE_MS", ttl.as_millis().to_string()),
    ]
}

fn multisig_role_suffix(role: &RoleId) -> Option<&str> {
    role.name()
        .as_ref()
        .strip_prefix("MULTISIG_SIGNATORY/")?
        .rsplit_once('/')
        .map(|(_, suffix)| suffix)
}

const COLLECTING_SIGNATURES_STATUS: &str = "COLLECTING_SIGNATURES";

fn collect_authority_multisig_approvals(client: &Client) -> Result<Vec<MultisigApprovalEntry>> {
    let mut cursor = None;
    let mut items = Vec::new();

    loop {
        let response =
            client.post_multisig_approvals_list_for_authority(&MultisigApprovalsListRequest {
                status: vec![COLLECTING_SIGNATURES_STATUS.to_owned()],
                operation_type: Vec::new(),
                requires_my_signature: false,
                cursor: cursor.clone(),
                limit: Some(100),
            })?;
        items.extend(response.items);
        let Some(next_cursor) = response.next_cursor else {
            break;
        };
        cursor = Some(next_cursor);
    }

    Ok(items)
}

fn wait_for_authority_multisig_approvals(
    client: &Client,
    minimum_count: usize,
) -> Result<Vec<MultisigApprovalEntry>> {
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut last_count = 0_usize;

    while Instant::now() < deadline {
        let approvals = collect_authority_multisig_approvals(client)?;
        if approvals.len() >= minimum_count {
            return Ok(approvals);
        }
        last_count = approvals.len();
        std::thread::sleep(Duration::from_millis(250));
    }

    Err(eyre!(
        "timed out waiting for at least {minimum_count} authority-scoped approvals; last count {last_count}"
    ))
}

fn run_multisig_list_all_cli(
    cli_program: &Path,
    client: &Client,
    account_domain: &DomainId,
    key_pair: &KeyPair,
    extra_args: &[&str],
) -> Result<std::process::Output> {
    let cli_dir = tempfile::tempdir().wrap_err("create CLI working directory")?;
    let mut command = std::process::Command::new(cli_program);
    command
        .current_dir(cli_dir.path())
        .envs(cli_envs_for_signatory(client, account_domain, key_pair));

    let mut list_args = Vec::new();
    let mut index = 0;
    while index < extra_args.len() {
        let arg = extra_args[index];
        if arg == "--output-format" {
            let value = *extra_args
                .get(index + 1)
                .ok_or_else(|| eyre!("missing value for `--output-format`"))?;
            command.arg(arg).arg(value);
            index += 2;
            continue;
        }
        if arg.starts_with("--output-format=") {
            command.arg(arg);
            index += 1;
            continue;
        }
        list_args.push(arg);
        index += 1;
    }

    command
        .arg("ledger")
        .arg("multisig")
        .arg("list")
        .arg("all")
        .args(list_args)
        .output()
        .wrap_err("run `iroha ledger multisig list all`")
}

#[test]
fn multisig_normal() -> Result<()> {
    multisig_base(TestSuite::normal(), stringify!(multisig_normal))
}

#[test]
fn multisig_unauthorized() -> Result<()> {
    multisig_base(TestSuite::unauthorized(), stringify!(multisig_unauthorized))
}

#[test]
fn multisig_expires() -> Result<()> {
    multisig_base(TestSuite::expires(), stringify!(multisig_expires))
}

#[test]
fn multisig_recursion_normal() -> Result<()> {
    multisig_recursion_base(TestSuite::normal(), stringify!(multisig_recursion_normal))
}

#[test]
fn multisig_recursion_unauthorized() -> Result<()> {
    multisig_recursion_base(
        TestSuite::unauthorized(),
        stringify!(multisig_recursion_unauthorized),
    )
}

#[test]
fn multisig_recursion_expires() -> Result<()> {
    multisig_recursion_base(TestSuite::expires(), stringify!(multisig_recursion_expires))
}

#[test]
fn multisig_cancel_route_persists_canceled_terminal_state() -> Result<()> {
    let context = stringify!(multisig_cancel_route_persists_canceled_terminal_state);
    let builder = NetworkBuilder::new();
    let Some((network, rt)) = start_network(builder, context) else {
        return Ok(());
    };
    let test_client = network.client();
    if !multisig_supported(&test_client) {
        eprintln!("skipping {context}: executor does not support multisig");
        return Ok(());
    }

    let domain: DomainId = DomainId::try_new("multisig-cancel-terminal", "universal").unwrap();
    register_runtime_domain(&network, &test_client, &domain)
        .wrap_err("register multisig cancel test domain")?;

    let spec = MultisigSpec::new(
        BTreeMap::from([(ALICE_ID.clone(), 1), (BOB_ID.clone(), 1)]),
        NonZeroU16::new(2).unwrap(),
        NonZeroU64::new(60_000).unwrap(),
    );
    let multisig_seed_account_id = AccountId::new(KeyPair::random().public_key().clone());
    test_client
        .submit_blocking::<InstructionBox>(
            MultisigRegister::with_account(multisig_seed_account_id, domain, spec.clone()).into(),
        )
        .wrap_err("register multisig account for cancel test")?;
    let multisig_account_id = canonical_multisig_account_id(&spec);

    let proposal_key: Name = "cancel_marker".parse().unwrap();
    let instructions = vec![
        SetKeyValue::account(
            multisig_account_id.clone(),
            proposal_key,
            "still-pending".parse::<Json>().unwrap(),
        )
        .into(),
    ];
    let instructions_hash = HashOf::new(&instructions).to_string();
    test_client
        .submit_blocking::<InstructionBox>(
            MultisigPropose::new(multisig_account_id.clone(), instructions, None).into(),
        )
        .wrap_err("submit target multisig proposal")?;

    let selector = MultisigAccountSelectorDto {
        multisig_account_id: Some(multisig_account_id.clone()),
        multisig_account_alias: None,
    };
    let torii_base = network
        .peers()
        .first()
        .expect("network should expose at least one peer")
        .torii_url()
        .to_string();

    let propose_cancel = post_torii_app_json(
        &rt,
        &format!("{torii_base}/v1/multisig/cancel"),
        &MultisigCancelRequestDto {
            selector: selector.clone(),
            signer_account_id: BOB_ID.clone(),
            private_key: None,
            public_key_hex: None,
            signature_b64: None,
            creation_time_ms: None,
            fee_sponsor: None,
            proposal_id: Some(instructions_hash.clone()),
            instructions_hash: None,
        },
    )?;
    assert_eq!(
        propose_cancel.get("action").and_then(JsonValue::as_str),
        Some("PROPOSE")
    );
    assert_eq!(
        propose_cancel
            .get("target_proposal_id")
            .and_then(JsonValue::as_str),
        Some(instructions_hash.as_str())
    );
    let cancel_proposal_id = propose_cancel
        .get("cancel_proposal_id")
        .and_then(JsonValue::as_str)
        .expect("cancel proposal id should be returned")
        .to_owned();
    let target_instructions_hash = instructions_hash.parse().unwrap();
    let cancel_instructions =
        vec![MultisigCancel::new(multisig_account_id.clone(), target_instructions_hash).into()];
    let expected_cancel_proposal_id = HashOf::new(&cancel_instructions).to_string();
    assert_eq!(
        cancel_proposal_id, expected_cancel_proposal_id,
        "cancel route should report the deterministic cancel proposal hash"
    );
    alt_client((BOB_ID.clone(), BOB_KEYPAIR.clone()), &test_client)
        .submit_blocking::<InstructionBox>(
            MultisigPropose::new(multisig_account_id.clone(), cancel_instructions, None).into(),
        )
        .wrap_err("submit cancel wrapper proposal for cancel route test")?;
    let approve_cancel = wait_for_multisig_cancel_action(
        &rt,
        &torii_base,
        &selector,
        &ALICE_ID,
        &instructions_hash,
        "APPROVE",
    )
    .wrap_err("wait for cancel route to expose approval mode")?;
    assert_eq!(
        approve_cancel.get("action").and_then(JsonValue::as_str),
        Some("APPROVE")
    );
    test_client
        .submit_blocking::<InstructionBox>(
            MultisigApprove::new(
                multisig_account_id.clone(),
                expected_cancel_proposal_id.parse().unwrap(),
            )
            .into(),
        )
        .wrap_err("submit cancel wrapper approval for cancel route test")?;

    let canceled = wait_for_multisig_proposal_status(
        &rt,
        &torii_base,
        &selector,
        &instructions_hash,
        "CANCELED",
    )
    .wrap_err("wait for canceled terminal state")?;
    assert_eq!(
        canceled.get("status").and_then(JsonValue::as_str),
        Some("CANCELED")
    );
    assert!(
        canceled
            .get("terminal_at_ms")
            .and_then(JsonValue::as_u64)
            .is_some(),
        "terminal proposal state should expose cancellation time"
    );

    let canceled_list = post_torii_app_json(
        &rt,
        &format!("{torii_base}/v1/multisig/proposals/list"),
        &MultisigProposalsListRequestDto {
            selector,
            status: vec!["CANCELED".to_owned()],
        },
    )?;
    let proposals = canceled_list
        .get("proposals")
        .and_then(JsonValue::as_array)
        .expect("canceled proposals array");
    assert!(
        proposals.iter().any(|proposal| {
            proposal.get("proposal_id").and_then(JsonValue::as_str)
                == Some(instructions_hash.as_str())
                && proposal.get("status").and_then(JsonValue::as_str) == Some("CANCELED")
        }),
        "canceled proposal should remain visible in terminal proposal listings"
    );

    Ok(())
}

#[test]
fn multisig_cli_list_all_resolves_hashed_role_suffixes() -> Result<()> {
    let context = stringify!(multisig_cli_list_all_resolves_hashed_role_suffixes);
    let cli_program =
        iroha_program().wrap_err("resolve `iroha` CLI before creating expiring proposals")?;
    let builder = NetworkBuilder::new().with_min_peers(4);
    let Some((network, _rt)) = start_network(builder, context) else {
        return Ok(());
    };
    let test_client = network.client();
    if !multisig_supported(&test_client) {
        eprintln!("skipping {context}: executor does not support multisig");
        return Ok(());
    }

    let domain: DomainId = DomainId::try_new("multisig-cli-hash-list", "universal").unwrap();
    register_runtime_domain_and_transfer_to_bob(&network, &test_client, &domain)
        .wrap_err("register multisig CLI test domain")?;

    let signatories = core::iter::repeat_with(|| gen_account_in(&domain))
        .take(8)
        .collect::<BTreeMap<AccountId, KeyPair>>();
    alt_client((BOB_ID.clone(), BOB_KEYPAIR.clone()), &test_client)
        .submit_all_blocking(
            signatories
                .keys()
                .cloned()
                .map(|account_id| Account::new(account_id.clone()))
                .map(Register::account),
        )
        .wrap_err("register multisig CLI signatories")?;

    let spec = MultisigSpec::new(
        signatories
            .keys()
            .cloned()
            .map(|account_id| (account_id, 1))
            .collect(),
        NonZeroU16::new(signatories.len().try_into().unwrap()).unwrap(),
        NonZeroU64::new(60_000).unwrap(),
    );
    let multisig_seed_account_id = AccountId::new(KeyPair::random().public_key().clone());
    alt_client((BOB_ID.clone(), BOB_KEYPAIR.clone()), &test_client)
        .submit_blocking::<InstructionBox>(
            MultisigRegister::with_account(multisig_seed_account_id, domain.clone(), spec.clone())
                .into(),
        )
        .wrap_err("register multisig account for CLI hash listing test")?;
    let multisig_account_id = canonical_multisig_account_id(&spec);
    let canonical_i105 = multisig_account_id
        .canonical_i105()
        .wrap_err("render multisig account as canonical I105")?;
    assert!(
        canonical_i105.len() > 128,
        "test precondition failed: multisig account should require a hashed role suffix"
    );

    let proposer = signatories
        .iter()
        .next()
        .map(|(account_id, key_pair)| (account_id.clone(), key_pair.clone()))
        .expect("signatory set must not be empty");
    let proposer_roles = test_client
        .query(FindRolesByAccountId::new(proposer.0.clone()))
        .execute_all()
        .wrap_err("fetch proposer roles after multisig registration")?;
    assert!(
        proposer_roles
            .iter()
            .filter_map(multisig_role_suffix)
            .any(|suffix| {
                suffix != canonical_i105 && AccountId::parse_encoded(suffix).is_err()
            }),
        "proposer should receive a hashed MULTISIG_SIGNATORY role for the long multisig account"
    );
    let proposer_client = alt_client(proposer.clone(), &test_client);
    let mut expected_proposal_ids = Vec::new();
    for suffix in ["a", "b", "c"] {
        let marker: Name = format!("cli_pending_marker_{suffix}").parse().unwrap();
        let instructions = vec![
            SetKeyValue::account(
                multisig_account_id.clone(),
                marker,
                Json::new(format!("pending-{suffix}")),
            )
            .into(),
        ];
        let proposal_id = HashOf::new(&instructions).to_string();
        proposer_client
            .submit_blocking::<InstructionBox>(
                MultisigPropose::new(multisig_account_id.clone(), instructions, None).into(),
            )
            .wrap_err_with(|| format!("submit multisig proposal `{proposal_id}` for CLI test"))?;
        expected_proposal_ids.push(proposal_id);
    }

    let expected_approvals =
        wait_for_authority_multisig_approvals(&proposer_client, expected_proposal_ids.len())
            .wrap_err("wait for authority-scoped approvals before invoking CLI")?;

    let json_output = run_multisig_list_all_cli(
        &cli_program,
        &proposer_client,
        &domain,
        &proposer.1,
        &["--output-format", "json"],
    )?;
    assert!(
        json_output.status.success(),
        "CLI exited with status {} and stderr: {}",
        json_output.status,
        String::from_utf8_lossy(&json_output.stderr)
    );

    let payload: JsonValue = norito::json::from_slice(&json_output.stdout)
        .wrap_err("decode CLI multisig list JSON output")?;
    let proposals = payload
        .as_array()
        .expect("CLI multisig list should emit a JSON array");
    assert_eq!(
        proposals.len(),
        expected_approvals.len(),
        "CLI JSON output should mirror the authority-scoped approvals page set"
    );
    let expected_multisig_id = multisig_account_id.to_string();
    let proposal = proposals
        .iter()
        .find(|proposal| {
            proposal.get("proposal_id").and_then(JsonValue::as_str)
                == expected_proposal_ids
                    .first()
                    .map(std::string::String::as_str)
        })
        .unwrap_or_else(|| {
            panic!(
                "CLI should surface the pending proposal discovered via the hashed role suffix; expected proposal ids {expected_proposal_ids:?}, payload {payload:?}"
            );
        });
    let expected_entry = expected_approvals
        .iter()
        .find(|entry| {
            Some(entry.proposal_id.as_str()) == expected_proposal_ids.first().map(String::as_str)
        })
        .expect("expected approval entry");
    assert!(
        proposals.iter().all(|entry| {
            entry.get("multisig_account_id").and_then(JsonValue::as_str)
                == Some(expected_multisig_id.as_str())
        }),
        "every CLI approval entry should resolve through the authority-scoped multisig account id"
    );
    assert_eq!(
        proposal.get("status").and_then(JsonValue::as_str),
        Some(COLLECTING_SIGNATURES_STATUS)
    );
    assert_eq!(
        proposal.get("operation_type").and_then(JsonValue::as_str),
        Some(expected_entry.operation_type.as_str())
    );
    assert_eq!(
        proposal.get("proposal_id").and_then(JsonValue::as_str),
        Some(expected_entry.proposal_id.as_str())
    );

    let text_output = run_multisig_list_all_cli(
        &cli_program,
        &proposer_client,
        &domain,
        &proposer.1,
        &["--output-format", "text"],
    )?;
    assert!(
        text_output.status.success(),
        "text CLI exited with status {} and stderr: {}",
        text_output.status,
        String::from_utf8_lossy(&text_output.stderr)
    );
    let text = String::from_utf8(text_output.stdout).expect("text output should be UTF-8");
    let blocks = text.trim().split("\n\n").collect::<Vec<_>>();
    assert_eq!(
        blocks.len(),
        expected_approvals.len(),
        "text output should contain one block per approval entry"
    );
    let first_text_entry = &expected_approvals[0];
    assert!(blocks[0].contains(&format!(
        "multisig_account_id: {}",
        first_text_entry.multisig_account_id
    )));
    assert!(blocks[0].contains(&format!("proposal_id: {}", first_text_entry.proposal_id)));
    assert!(blocks[0].contains("status: COLLECTING_SIGNATURES"));
    assert!(blocks[0].contains(&format!(
        "operation_type: {}",
        first_text_entry.operation_type
    )));
    assert!(blocks[0].contains(&format!(
        "proposed_at_ms: {}",
        first_text_entry.proposal.proposed_at_ms
    )));

    let paged_output = run_multisig_list_all_cli(
        &cli_program,
        &proposer_client,
        &domain,
        &proposer.1,
        &[
            "--output-format",
            "json",
            "--fetch-size",
            "1",
            "--offset",
            "1",
            "--limit",
            "2",
        ],
    )?;
    assert!(
        paged_output.status.success(),
        "paged CLI exited with status {} and stderr: {}",
        paged_output.status,
        String::from_utf8_lossy(&paged_output.stderr)
    );
    let paged_payload: JsonValue = norito::json::from_slice(&paged_output.stdout)
        .wrap_err("decode paged CLI multisig list JSON output")?;
    let paged_items = paged_payload
        .as_array()
        .expect("paged CLI multisig list should emit a JSON array");
    let paged_ids = paged_items
        .iter()
        .map(|entry| {
            entry
                .get("proposal_id")
                .and_then(JsonValue::as_str)
                .expect("proposal_id")
                .to_owned()
        })
        .collect::<Vec<_>>();
    let expected_paged_ids = expected_approvals
        .iter()
        .skip(1)
        .take(2)
        .map(|entry| entry.proposal_id.clone())
        .collect::<Vec<_>>();
    assert_eq!(paged_ids, expected_paged_ids);

    Ok(())
}

#[test]
fn multisig_register_materializes_missing_signatory_account() -> Result<()> {
    let context = stringify!(multisig_register_materializes_missing_signatory_account);
    let builder = NetworkBuilder::new();
    let Some((network, _rt)) = start_network(builder, context) else {
        return Ok(());
    };
    let test_client = network.client();
    if !multisig_supported(&test_client) {
        eprintln!("skipping {context}: executor does not support multisig register");
        return Ok(());
    }

    let domain: DomainId = DomainId::try_new("multisig-register-materialize", "universal").unwrap();
    register_runtime_domain_and_transfer_to_bob(&network, &test_client, &domain)?;

    let existing_signer = gen_account_in(&domain);
    alt_client((BOB_ID.clone(), BOB_KEYPAIR.clone()), &test_client)
        .submit_blocking(Register::account(Account::new(existing_signer.0.clone())))?;

    let missing_signer = gen_account_in(&domain);
    let spec = MultisigSpec::new(
        BTreeMap::from([
            (existing_signer.0.clone(), 1),
            (missing_signer.0.clone(), 1),
        ]),
        NonZeroU16::new(2).unwrap(),
        NonZeroU64::MAX,
    );
    let seed_account = AccountId::new(KeyPair::random().public_key().clone());
    alt_client((BOB_ID.clone(), BOB_KEYPAIR.clone()), &test_client)
        .submit_blocking::<InstructionBox>(
            MultisigRegister::with_account(seed_account, domain.clone(), spec).into(),
        )?;

    let fetch_account = |id: &AccountId| {
        test_client
            .query(FindAccounts::new())
            .execute_all()
            .ok()
            .and_then(|accounts| accounts.into_iter().find(|account| account.id() == id))
    };
    let created_via_key: Name = "iroha:created_via".parse().unwrap();
    let created = fetch_account(&missing_signer.0)
        .expect("missing signatory account should be created during multisig register");
    assert_eq!(
        created.metadata().get(&created_via_key),
        Some(&Json::new("multisig")),
        "materialized signatory should be marked as multisig-created"
    );

    Ok(())
}

#[test]
fn multisig_register_by_non_signatory_materializes_missing_signatory_account() -> Result<()> {
    let context =
        stringify!(multisig_register_by_non_signatory_materializes_missing_signatory_account);
    let builder = NetworkBuilder::new();
    let Some((network, _rt)) = start_network(builder, context) else {
        return Ok(());
    };
    let test_client = network.client();
    if !multisig_supported(&test_client) {
        eprintln!("skipping {context}: executor does not support multisig register");
        return Ok(());
    }

    let domain: DomainId =
        DomainId::try_new("multisig-register-rejected-materialize", "universal").unwrap();
    register_runtime_domain_and_transfer_to_bob(&network, &test_client, &domain)?;

    let existing_signer = gen_account_in(&domain);
    let non_signatory = gen_account_in(&domain);
    let register_accounts: [InstructionBox; 2] = [
        Register::account(Account::new(existing_signer.0.clone())).into(),
        Register::account(Account::new(non_signatory.0.clone())).into(),
    ];
    alt_client((BOB_ID.clone(), BOB_KEYPAIR.clone()), &test_client)
        .submit_all_blocking(register_accounts)?;

    let missing_signer = gen_account_in(&domain);
    let spec = MultisigSpec::new(
        BTreeMap::from([
            (existing_signer.0.clone(), 1),
            (missing_signer.0.clone(), 1),
        ]),
        NonZeroU16::new(2).unwrap(),
        NonZeroU64::MAX,
    );
    let seed_account = AccountId::new(KeyPair::random().public_key().clone());
    let register = MultisigRegister::with_account(seed_account, domain.clone(), spec);
    alt_client(non_signatory, &test_client)
        .submit_blocking::<InstructionBox>(register.into())
        .expect("non-signatory should register multisig without a separate grant");

    let created_via_key: Name = "iroha:created_via".parse().unwrap();
    let created = test_client
        .query(FindAccounts::new())
        .execute_all()?
        .into_iter()
        .find(|account| account.id() == &missing_signer.0)
        .expect("missing signatory account should be created during multisig register");
    assert_eq!(
        created.metadata().get(&created_via_key),
        Some(&Json::new("multisig")),
        "materialized signatory should be marked as multisig-created"
    );

    Ok(())
}

#[test]
fn multisig_register_materializes_missing_signatory_account_after_executor_upgrade() -> Result<()> {
    let context =
        stringify!(multisig_register_materializes_missing_signatory_account_after_executor_upgrade);
    let builder = NetworkBuilder::new().with_ivm_fuel(IvmFuelConfig::Auto);
    let Some((network, _rt)) = start_network(builder, context) else {
        return Ok(());
    };
    let test_client = network.client();
    if !multisig_supported(&test_client) {
        eprintln!("skipping {context}: executor does not support multisig register");
        return Ok(());
    }

    let domain: DomainId =
        DomainId::try_new("multisig-register-materialize-upgraded", "universal").unwrap();
    register_runtime_domain_and_transfer_to_bob(&network, &test_client, &domain)?;
    // This regression targets multisig account materialization after the executor
    // upgrade. Keep the domain bootstrap on the pre-upgrade executor so the test
    // stays scoped to the multisig path rather than unrelated domain admission.
    upgrade_executor(&test_client, "executor_with_admin")?;

    let existing_signer = gen_account_in(&domain);
    alt_client((BOB_ID.clone(), BOB_KEYPAIR.clone()), &test_client)
        .submit_blocking(Register::account(Account::new(existing_signer.0.clone())))?;

    let missing_signer = gen_account_in(&domain);
    let spec = MultisigSpec::new(
        BTreeMap::from([
            (existing_signer.0.clone(), 1),
            (missing_signer.0.clone(), 1),
        ]),
        NonZeroU16::new(2).unwrap(),
        NonZeroU64::MAX,
    );
    let seed_account = AccountId::new(KeyPair::random().public_key().clone());
    alt_client((BOB_ID.clone(), BOB_KEYPAIR.clone()), &test_client)
        .submit_blocking::<InstructionBox>(
            MultisigRegister::with_account(seed_account, domain.clone(), spec).into(),
        )?;

    let created_via_key: Name = "iroha:created_via".parse().unwrap();
    let created = test_client
        .query(FindAccounts::new())
        .execute_all()?
        .into_iter()
        .find(|account| account.id() == &missing_signer.0)
        .expect("missing signatory account should be created during multisig register");
    assert_eq!(
        created.metadata().get(&created_via_key),
        Some(&Json::new("multisig")),
        "materialized signatory should be marked as multisig-created"
    );

    Ok(())
}

#[test]
fn multisig_register_by_non_signatory_materializes_missing_signatory_account_after_executor_upgrade()
-> Result<()> {
    let context = stringify!(
        multisig_register_by_non_signatory_materializes_missing_signatory_account_after_executor_upgrade
    );
    let builder = NetworkBuilder::new().with_ivm_fuel(IvmFuelConfig::Auto);
    let Some((network, _rt)) = start_network(builder, context) else {
        return Ok(());
    };
    let test_client = network.client();
    if !multisig_supported(&test_client) {
        eprintln!("skipping {context}: executor does not support multisig register");
        return Ok(());
    }

    let domain: DomainId =
        DomainId::try_new("multisig-register-rejected-upgraded", "universal").unwrap();
    register_runtime_domain_and_transfer_to_bob(&network, &test_client, &domain)?;
    // Keep domain bootstrap outside the upgraded executor so this test continues
    // to isolate the post-upgrade multisig register behavior it actually covers.
    upgrade_executor(&test_client, "executor_with_admin")?;

    let existing_signer = gen_account_in(&domain);
    let non_signatory = gen_account_in(&domain);
    let register_accounts: [InstructionBox; 2] = [
        Register::account(Account::new(existing_signer.0.clone())).into(),
        Register::account(Account::new(non_signatory.0.clone())).into(),
    ];
    alt_client((BOB_ID.clone(), BOB_KEYPAIR.clone()), &test_client)
        .submit_all_blocking(register_accounts)?;

    let missing_signer = gen_account_in(&domain);
    let spec = MultisigSpec::new(
        BTreeMap::from([
            (existing_signer.0.clone(), 1),
            (missing_signer.0.clone(), 1),
        ]),
        NonZeroU16::new(2).unwrap(),
        NonZeroU64::MAX,
    );
    let seed_account = AccountId::new(KeyPair::random().public_key().clone());
    let register = MultisigRegister::with_account(seed_account, domain.clone(), spec);
    alt_client(non_signatory, &test_client)
        .submit_blocking::<InstructionBox>(register.into())
        .expect("non-signatory should register multisig without a separate grant");

    let created_via_key: Name = "iroha:created_via".parse().unwrap();
    let created = test_client
        .query(FindAccounts::new())
        .execute_all()?
        .into_iter()
        .find(|account| account.id() == &missing_signer.0)
        .expect("missing signatory account should be created during multisig register");
    assert_eq!(
        created.metadata().get(&created_via_key),
        Some(&Json::new("multisig")),
        "materialized signatory should be marked as multisig-created"
    );

    Ok(())
}

#[test]
fn multisig_add_signatory_materializes_missing_account() -> Result<()> {
    let context = stringify!(multisig_add_signatory_materializes_missing_account);
    let builder = NetworkBuilder::new();
    let Some((network, _rt)) = start_network(builder, context) else {
        return Ok(());
    };
    let test_client = network.client();
    if !multisig_supported(&test_client) {
        eprintln!("skipping {context}: executor does not advertise multisig instructions");
        return Ok(());
    }

    let domain: DomainId = DomainId::try_new("multisig-auto-materialize", "universal").unwrap();
    register_runtime_domain_and_transfer_to_bob(&network, &test_client, &domain)?;

    let existing_signer = gen_account_in(&domain);
    alt_client((BOB_ID.clone(), BOB_KEYPAIR.clone()), &test_client)
        .submit_blocking(Register::account(Account::new(existing_signer.0.clone())))?;

    let spec = MultisigSpec::new(
        BTreeMap::from([(existing_signer.0.clone(), 1)]),
        NonZeroU16::new(1).unwrap(),
        NonZeroU64::MAX,
    );
    let seed_account = AccountId::new(KeyPair::random().public_key().clone());
    alt_client((BOB_ID.clone(), BOB_KEYPAIR.clone()), &test_client)
        .submit_blocking::<InstructionBox>(
            MultisigRegister::with_account(seed_account.clone(), domain.clone(), spec.clone())
                .into(),
        )?;

    let multisig_account_id = canonical_multisig_account_id(&spec);

    let missing_signer = gen_account_in(&domain);
    let fetch_account = |id: &AccountId| {
        test_client
            .query(FindAccounts::new())
            .execute_all()
            .ok()
            .and_then(|accounts| accounts.into_iter().find(|account| account.id() == id))
    };
    assert!(
        fetch_account(&missing_signer.0).is_none(),
        "precondition: missing signatory must not exist"
    );

    alt_client((BOB_ID.clone(), BOB_KEYPAIR.clone()), &test_client)
        .submit_blocking::<InstructionBox>(
            AddSignatory::new(multisig_account_id, missing_signer.1.public_key().clone()).into(),
        )?;

    let created_via_key: Name = "iroha:created_via".parse().unwrap();
    let created = fetch_account(&missing_signer.0)
        .expect("missing signatory account should be created by add-signatory");
    assert_eq!(
        created.metadata().get(&created_via_key),
        Some(&Json::new("multisig")),
        "materialized account should be marked as multisig-created"
    );

    Ok(())
}

#[test]
fn multisig_add_signatory_rejected_does_not_materialize_missing_account() -> Result<()> {
    let context = stringify!(multisig_add_signatory_rejected_does_not_materialize_missing_account);
    let builder = NetworkBuilder::new();
    let Some((network, _rt)) = start_network(builder, context) else {
        return Ok(());
    };
    let test_client = network.client();
    if !multisig_supported(&test_client) {
        eprintln!("skipping {context}: executor does not support multisig register");
        return Ok(());
    }

    let domain: DomainId =
        DomainId::try_new("multisig-add-rejected-materialize", "universal").unwrap();
    register_runtime_domain_and_transfer_to_bob(&network, &test_client, &domain)?;

    let existing_signer = gen_account_in(&domain);
    alt_client((BOB_ID.clone(), BOB_KEYPAIR.clone()), &test_client)
        .submit_blocking(Register::account(Account::new(existing_signer.0.clone())))?;

    let seed_account = AccountId::new(KeyPair::random().public_key().clone());
    let spec = MultisigSpec::new(
        BTreeMap::from([(existing_signer.0.clone(), 1)]),
        NonZeroU16::new(1).unwrap(),
        NonZeroU64::MAX,
    );
    alt_client((BOB_ID.clone(), BOB_KEYPAIR.clone()), &test_client)
        .submit_blocking::<InstructionBox>(
            MultisigRegister::with_account(seed_account.clone(), domain.clone(), spec.clone())
                .into(),
        )?;

    let multisig_account_id = canonical_multisig_account_id(&spec);

    let missing_signer = gen_account_in(&domain);
    let ghost_authority = gen_account_in(&domain);
    assert!(
        test_client
            .query(FindAccounts::new())
            .execute_all()?
            .into_iter()
            .all(|account| account.id() != &ghost_authority.0),
        "precondition: authority account must not exist on ledger"
    );
    let _err = alt_client(ghost_authority, &test_client)
        .submit_blocking::<InstructionBox>(
            AddSignatory::new(multisig_account_id, missing_signer.1.public_key().clone()).into(),
        )
        .expect_err("missing authority must not add signatory");

    let missing_found = test_client
        .query(FindAccounts::new())
        .execute_all()?
        .into_iter()
        .any(|account| account.id() == &missing_signer.0);
    assert!(
        !missing_found,
        "rejected add-signatory must not materialize missing accounts"
    );

    Ok(())
}

struct TestSuite {
    domain: DomainId,
    unauthorized_target_opt: Option<AccountId>,
    transaction_ttl_ms_opt: Option<u64>,
}

impl TestSuite {
    fn new(
        domain: DomainId,
        unauthorized_target_opt: Option<AccountId>,
        transaction_ttl_ms_opt: Option<u64>,
    ) -> Self {
        Self {
            domain,
            unauthorized_target_opt,
            transaction_ttl_ms_opt,
        }
    }
    fn normal() -> Self {
        // New domain for this test
        let domain = DomainId::try_new("kingdom", "universal").unwrap();
        // Make some changes to the multisig account itself
        let unauthorized_target_opt = None;
        // Semi-permanently valid
        let transaction_ttl_ms_opt = None;

        Self::new(domain, unauthorized_target_opt, transaction_ttl_ms_opt)
    }

    fn unauthorized() -> Self {
        let domain = DomainId::try_new("kingdom", "universal").unwrap();
        // A target account that is not present on-ledger, ensuring the proposal execution fails
        // on final validation instead of mutating unrelated account metadata.
        let unauthorized_target_opt = Some(AccountId::new(KeyPair::random().public_key().clone()));

        Self::new(domain, unauthorized_target_opt, None)
    }

    fn expires() -> Self {
        let domain = DomainId::try_new("kingdom", "universal").unwrap();
        // Expires after 1 sec
        let transaction_ttl_ms_opt = Some(1_000);

        Self::new(domain, None, transaction_ttl_ms_opt)
    }
}

/// # Scenario
///
/// 1. Signatories are populated and ready to join a multisig account
/// 2. An arbitrary account registers a multisig account for the domain
/// 3. One of the signatories of the multisig account proposes a multisig transaction
/// 4. Other signatories approve the multisig transaction
/// 5. The multisig transaction executes when all of the following are met:
///     - Quorum reached: authenticated
///     - Transaction has not expired
///     - Every instruction validated against the multisig account: authorized
/// 6. Either execution or expiration on approval deletes the transaction entry
#[expect(clippy::cast_possible_truncation, clippy::too_many_lines)]
fn multisig_base(suite: TestSuite, context: &'static str) -> Result<()> {
    const N_SIGNATORIES: usize = 5;

    let TestSuite {
        domain,
        unauthorized_target_opt,
        transaction_ttl_ms_opt,
    } = suite;

    let builder = NetworkBuilder::new();
    let Some((network, _rt)) = start_network(builder, context) else {
        return Ok(());
    };
    let test_client = network.client();
    if !multisig_supported(&test_client) {
        eprintln!("skipping {context}: executor does not advertise multisig instructions");
        return Ok(());
    }

    // Assume some domain registered after genesis
    register_runtime_domain(&network, &test_client, &domain)
        .wrap_err("register multisig test domain")?;

    // Populate residents in the domain
    let mut residents = core::iter::repeat_with(|| gen_account_in(&domain))
        .take(1 + N_SIGNATORIES)
        .collect::<BTreeMap<AccountId, KeyPair>>();
    test_client
        .submit_all_blocking(
            residents
                .keys()
                .cloned()
                .map(|id| Account::new(id.clone()))
                .map(Register::account),
        )
        .wrap_err("register multisig test residents")?;

    let non_signatory = residents.pop_first().unwrap();
    let mut signatories = residents;

    let spec = MultisigSpec::new(
        signatories
            .keys()
            .enumerate()
            .map(|(weight, id)| (id.clone(), 1 + weight as u8))
            .collect(),
        // Quorum can be reached without the first signatory
        (1..=N_SIGNATORIES)
            .skip(1)
            .sum::<usize>()
            .try_into()
            .ok()
            .and_then(NonZeroU16::new)
            .unwrap(),
        transaction_ttl_ms_opt
            .and_then(NonZeroU64::new)
            .unwrap_or(NonZeroU64::MAX),
    );
    let multisig_account_key = KeyPair::random();
    let multisig_seed_account_id = AccountId::new(multisig_account_key.public_key().clone());
    let register_multisig_account = MultisigRegister::with_account(
        multisig_seed_account_id.clone(),
        domain.clone(),
        spec.clone(),
    );

    alt_client(
        (CARPENTER_ID.clone(), CARPENTER_KEYPAIR.clone()),
        &test_client,
    )
    .submit_blocking::<InstructionBox>(register_multisig_account.into())
    .expect("multisig account should be registered by an arbitrary account");
    let resident_ids: Vec<AccountId> = core::iter::once(non_signatory.0.clone())
        .chain(signatories.keys().cloned())
        .collect();
    let accounts_after_register = test_client
        .query(FindAccounts::new())
        .execute_all()
        .wrap_err("fetch accounts after multisig registration")?;
    for resident_id in resident_ids {
        let account = accounts_after_register
            .iter()
            .find(|account| account.id() == &resident_id)
            .expect("resident account should remain materialized after multisig registration");
        assert!(
            account.controller().single_signatory().is_some(),
            "resident account unexpectedly became multisig: {}",
            account.id()
        );
    }

    // All but the first signatory approve the proposal.
    let _non_approving_signatory = signatories.pop_first().unwrap();
    let multisig_account_id = canonical_multisig_account_id(&spec);

    let key: Name = "success_marker".parse().unwrap();
    let transaction_target = unauthorized_target_opt
        .as_ref()
        .unwrap_or(&multisig_account_id)
        .clone();
    let instructions = vec![
        SetKeyValue::account(
            transaction_target.clone(),
            key.clone(),
            "congratulations".parse::<Json>().unwrap(),
        )
        .into(),
    ];
    let instructions_hash = HashOf::new(&instructions);

    let proposer = signatories.pop_last().unwrap();
    let mut approvers = signatories.into_iter();

    let propose = MultisigPropose::new(multisig_account_id.clone(), instructions, None);
    let proposer_client = alt_client(proposer.clone(), &test_client);
    let proposer_account = test_client
        .query(FindAccounts::new())
        .execute_all()?
        .into_iter()
        .find(|account| account.id() == &proposer.0)
        .expect("proposer account must exist before multisig proposal");
    assert!(
        proposer_account.controller().single_signatory().is_some(),
        "proposer account unexpectedly became multisig before proposal: {}",
        proposer_account.id()
    );
    let proposal_tx = proposer_client.build_transaction_from_items(
        core::iter::once::<InstructionBox>(propose.into()),
        Metadata::default(),
    );
    assert_eq!(
        proposal_tx.authority().subject_id(),
        proposer.0.subject_id(),
        "proposal transaction authority subject must match proposer"
    );
    assert!(
        proposal_tx
            .authority()
            .controller()
            .single_signatory()
            .is_some(),
        "proposal transaction authority unexpectedly became multisig: {}",
        proposal_tx.authority()
    );
    proposer_client
        .submit_transaction_blocking(&proposal_tx)
        .wrap_err("submit multisig proposal")?;

    // Allow time to elapse to test the expiration
    if let Some(ms) = transaction_ttl_ms_opt {
        std::thread::sleep(Duration::from_millis(ms))
    }
    test_client
        .submit_blocking(Log::new(Level::DEBUG, "Just ticking time".to_string()))
        .wrap_err("tick time after multisig proposal")?;

    let approve: InstructionBox =
        MultisigApprove::new(multisig_account_id.clone(), instructions_hash).into();

    // Approve once to see if the proposal expires
    let approver = approvers.next().unwrap();
    alt_client(approver, &test_client)
        .submit_blocking::<InstructionBox>(approve.clone())
        .wrap_err("submit first multisig approval")?;

    // Subsequent approvals should succeed unless the proposal is expired
    for _ in 0..(N_SIGNATORIES - 4) {
        let approver = approvers.next().unwrap();
        let res =
            alt_client(approver, &test_client).submit_blocking::<InstructionBox>(approve.clone());
        match &transaction_ttl_ms_opt {
            None => {
                res.unwrap();
            }
            _ => {
                let _err = res.unwrap_err();
            }
        }
    }

    let fetch_account = |id: &AccountId| {
        test_client
            .query(FindAccounts::new())
            .execute_all()
            .ok()
            .and_then(|accounts| accounts.into_iter().find(|account| account.id() == id))
    };
    // Check that the multisig transaction has not yet executed
    assert!(
        fetch_account(&transaction_target)
            .and_then(|account| account.metadata().get(&key).cloned())
            .is_none(),
        "instructions shouldn't execute without enough approvals"
    );

    // The last approve to proceed to validate and execute the instructions
    let approver = approvers.next().unwrap();
    let res = alt_client(approver, &test_client).submit_blocking::<InstructionBox>(approve.clone());
    match (&transaction_ttl_ms_opt, &unauthorized_target_opt) {
        (None, None) => {
            res.unwrap();
        }
        _ => {
            let _err = res.unwrap_err();
        }
    }

    // Check if the multisig transaction has executed
    let res = fetch_account(&transaction_target)
        .and_then(|account| account.metadata().get(&key).cloned());
    match (&transaction_ttl_ms_opt, &unauthorized_target_opt) {
        (None, None) => {
            res.unwrap();
        }
        _ => {
            assert!(res.is_none());
        }
    }

    // Check if the transaction entry is deleted
    let res = fetch_account(&multisig_account_id).and_then(|account| {
        account
            .metadata()
            .get(format!("multisig/proposals/{instructions_hash}").as_str())
            .cloned()
    });
    // Proposals are removed once quorum processing runs, including unauthorized execution failures.
    assert!(res.is_none());

    Ok(())
}

/// # Scenario
///
/// ```
///         012345 <--- root multisig account
///        /      \
///       /        12345
///      /        /     \
///     /       12       345
///    /       /  \     / | \
///   0       1    2   3  4  5 <--- personal signatories
/// ```
fn multisig_recursion_base(suite: TestSuite, context: &'static str) -> Result<()> {
    let _ = suite;

    let builder = NetworkBuilder::new();
    let Some((network, _rt)) = start_network(builder, context) else {
        return Ok(());
    };
    let test_client = network.client();
    if !multisig_supported(&test_client) {
        eprintln!("skipping {context}: executor does not advertise multisig instructions");
        return Ok(());
    }

    let wonderland = "wonderland";
    let signatories = core::iter::repeat_with(|| gen_account_in(wonderland))
        .take(6)
        .collect::<BTreeMap<AccountId, KeyPair>>();
    let wonderland_domain = DomainId::try_new(wonderland, "universal").unwrap();
    test_client.submit_all_blocking(
        signatories
            .keys()
            .cloned()
            .map(|id| Account::new(id.clone()))
            .map(Register::account),
    )?;

    let mut sigs = signatories.clone();
    let sigs_345 = sigs.split_off(signatories.keys().nth(3).unwrap());
    let sigs_12 = sigs.split_off(signatories.keys().nth(1).unwrap());

    let register_ms_account = |sigs: Vec<&AccountId>| -> Result<AccountId> {
        let spec = MultisigSpec::new(
            sigs.iter().copied().map(|id| (id.clone(), 1)).collect(),
            sigs.len()
                .try_into()
                .ok()
                .and_then(NonZeroU16::new)
                .unwrap(),
            NonZeroU64::MAX,
        );
        let multisig_account_key = KeyPair::random();
        let seed_account_id = AccountId::new(multisig_account_key.public_key().clone());
        let register = MultisigRegister::with_account(
            seed_account_id.clone(),
            wonderland_domain.clone(),
            spec.clone(),
        );
        test_client
            .submit_blocking::<InstructionBox>(register.into())
            .wrap_err("register multisig account in recursion setup")?;
        Ok(canonical_multisig_account_id(&spec))
    };

    let msa_12 = register_ms_account(sigs_12.keys().collect())?;
    let msa_345 = register_ms_account(sigs_345.keys().collect())?;

    let spec_with_nested_signatory = MultisigSpec::new(
        BTreeMap::from([(msa_12, 1), (msa_345, 1)]),
        NonZeroU16::new(2).unwrap(),
        NonZeroU64::MAX,
    );
    let seed_account_id = AccountId::new(KeyPair::random().public_key().clone());
    let register_nested = MultisigRegister::with_account(
        seed_account_id,
        wonderland_domain,
        spec_with_nested_signatory,
    );
    let err = test_client
        .submit_blocking::<InstructionBox>(register_nested.into())
        .expect_err("nested multisig signatories must be rejected");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("single-key account"),
        "expected nested-signatory rejection to mention single-key requirement, got: {msg}"
    );

    Ok(())
}

#[test]
fn reserved_roles() {
    let builder = NetworkBuilder::new();
    let Some((network, _rt)) = start_network(builder, stringify!(reserved_roles)) else {
        return;
    };
    let test_client = network.client();

    let account_in_another_domain = gen_account_in("garden_of_live_flowers").0;
    let register = {
        let other_domain = "garden_of_live_flowers";
        let role = format!(
            "MULTISIG_SIGNATORY/{}/{}",
            other_domain,
            account_in_another_domain.signatory()
        )
        .parse()
        .unwrap();
        Register::role(Role::new(role, ALICE_ID.clone()))
    };

    let _err = test_client.submit_blocking(register).expect_err(
        "role with this name shouldn't be registered by anyone other than the domain owner",
    );
}

fn alt_client(signatory: (AccountId, KeyPair), base_client: &Client) -> Client {
    Client {
        account: signatory.0,
        key_pair: signatory.1,
        ..base_client.clone()
    }
}

#[expect(dead_code)]
fn debug_account(account_id: &AccountId, client: &Client) {
    let account = client
        .query(FindAccounts)
        .execute_all()
        .unwrap()
        .into_iter()
        .find(|account| account.id() == account_id)
        .unwrap();

    eprintln!("{account:#?}");
}

#[test]
fn inconclusive_domain_registration_error_matches_queue_timeout() {
    let err = eyre!("transaction queued for too long");
    assert!(is_inconclusive_domain_registration_error(&err));
}

#[test]
fn inconclusive_domain_registration_error_ignores_rejections() {
    let err = eyre!("domain registration rejected");
    assert!(!is_inconclusive_domain_registration_error(&err));
}
