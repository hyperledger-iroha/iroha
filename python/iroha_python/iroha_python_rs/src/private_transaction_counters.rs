//! Native canonical builders and independently anchored private-counter verification.

use std::{
    num::NonZeroU64,
    time::{SystemTime, UNIX_EPOCH},
};

use iroha_crypto::{HashOf, KeyPair};
use iroha_data_model::{
    block::{consensus::SumeragiRootScope, decode_framed_signed_block},
    private_transaction_counters::{
        CounterCategoryV1, CounterCutV1, CounterExecutableBindingV1, CounterPartyV1,
        CounterRejectionV1, CounterResultV1, CounterRunBindingV1, CounterSemanticV1,
        CounterTransactionRoleV1, MAX_PRIVATE_COUNTER_FRAME_BYTES_V1,
        MAX_PRIVATE_COUNTER_MANIFEST_BYTES_V1, MAX_PRIVATE_COUNTER_POLICY_BYTES_V1,
        PRIVATE_COUNTER_REQUEST_DOMAIN_V1, PrivateCountersExpectedV1, PrivateCountersManifestV1,
        PrivateCountersPolicyV1, PrivateCountersRequestV1, collect_private_counters_v1,
        counter_plan_commitment_v1, verify_private_counters_v1,
    },
    sumeragi_finality::{
        SumeragiFinalityCheckpoint, SumeragiFinalityProof, SumeragiFinalityVerifier,
        VerifiedFinalityPage, genesis_epoch, signed_genesis_consensus_metadata,
        verify_checkpoint_page,
    },
    transaction::Executable,
};
use norito::json;
use pyo3::{
    Bound, Py, PyRef, PyResult, Python,
    exceptions::PyValueError,
    pyfunction,
    types::{PyBytes, PyBytesMethods, PyList, PyListMethods, PyModule, PyModuleMethods},
    wrap_pyfunction,
};

const MAX_CHAIN_JSON: usize = 16 * 1024 * 1024;
const MAX_CHAIN_PROOFS: usize = 4096;
const MAX_CHECKPOINT: usize = 68 * 1024 * 1024;
const MAX_AUTHORED_EXECUTABLE: usize = 32 * 1024 * 1024;
const MAX_PRIVATE_GENESIS: usize = 64 * 1024 * 1024;

fn refused() -> pyo3::PyErr {
    PyValueError::new_err("native private counters admission failed")
}

fn bounded_json<T: json::JsonDeserialize>(original: &str, maximum: usize) -> PyResult<T> {
    if original.is_empty() || original.len() > maximum {
        return Err(refused());
    }
    // Current model types deny unknown fields. JSON is only a bounded native-model
    // construction input; the transported/signature-bearing representation is Norito.
    let limits = norito::canonical_decode_limits(maximum);
    json::preflight_slice(
        original.as_bytes(),
        json::JsonPreflightLimits::from_decode_limits(maximum, limits),
    )
    .map_err(|_| refused())?;
    norito::with_decode_limits_scope(limits, || json::from_json(original)).map_err(|_| refused())
}

fn native_now_ms() -> PyResult<u64> {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| refused())?
            .as_millis(),
    )
    .ok()
    .filter(|now| *now != 0)
    .ok_or_else(refused)
}

#[pyfunction]
#[pyo3(name = "canonical_private_genesis_authority_v1")]
fn private_genesis_authority(original_signed_genesis: &[u8]) -> PyResult<String> {
    // The caller must independently authenticate this exact snapshot through its
    // installed root. This reader verifies its current syntax/signatures/scope;
    // an offered valid genesis alone never selects or installs a trusted root.
    if original_signed_genesis.is_empty() || original_signed_genesis.len() > MAX_PRIVATE_GENESIS {
        return Err(refused());
    }
    norito::with_decode_limits_scope(norito::canonical_decode_limits(MAX_PRIVATE_GENESIS), || {
        let block = decode_framed_signed_block(original_signed_genesis).map_err(|_| refused())?;
        if !block.header().is_genesis()
            || block.encode_wire().map_err(|_| refused())? != original_signed_genesis
        {
            return Err(refused());
        }
        genesis_epoch(&block).map_err(|_| refused())?;
        let metadata = signed_genesis_consensus_metadata(&block).map_err(|_| refused())?;
        if !matches!(
            metadata.sumeragi_context.root_scope,
            SumeragiRootScope::Dataspace { .. }
        ) {
            return Err(refused());
        }
        metadata
            .sumeragi_context
            .root_scope
            .validate()
            .map_err(|_| refused())?;
        let authority = block
            .external_transactions()
            .next()
            .ok_or_else(refused)?
            .authority();
        if authority.try_signatory().is_none()
            || block
                .external_transactions()
                .any(|transaction| transaction.authority() != authority)
        {
            return Err(refused());
        }
        Ok(authority.to_string())
    })
}

fn selected_page(
    expected: &PrivateCountersExpectedV1,
    chain_json: &str,
    expected_chain: &str,
    checkpoint_original: &[u8],
) -> PyResult<VerifiedFinalityPage> {
    if checkpoint_original.is_empty()
        || checkpoint_original.len() > MAX_CHECKPOINT
        || expected_chain.is_empty()
        || expected_chain.len() > 1024
    {
        return Err(refused());
    }
    let checkpoint =
        SumeragiFinalityCheckpoint::decode_canonical(checkpoint_original).map_err(|_| refused())?;
    if checkpoint.chain_id() != expected_chain {
        return Err(refused());
    }
    let verifier = SumeragiFinalityVerifier::from_trusted_checkpoint(
        &checkpoint,
        &expected.network_id,
        expected_chain,
    )
    .map_err(|_| refused())?;
    if verifier.root_scope().map_err(|_| refused())? != expected.scope {
        return Err(refused());
    }
    let proofs: Vec<SumeragiFinalityProof> = bounded_json(chain_json, MAX_CHAIN_JSON)?;
    if proofs.is_empty() || proofs.len() > MAX_CHAIN_PROOFS {
        return Err(refused());
    }
    verify_checkpoint_page(
        expected.network_id,
        &checkpoint,
        &proofs,
        MAX_CHAIN_PROOFS,
        MAX_CHAIN_JSON,
    )
    .map_err(|_| refused())
}

#[pyfunction]
#[pyo3(name = "encode_private_transaction_counters_policy_v1")]
fn encode_policy(py: Python<'_>, policy_json: &str) -> PyResult<(Py<PyBytes>, String)> {
    let policy: PrivateCountersPolicyV1 =
        bounded_json(policy_json, MAX_PRIVATE_COUNTER_POLICY_BYTES_V1)?;
    let original = policy.encode_canonical().map_err(|_| refused())?;
    let hash = policy.commitment().map_err(|_| refused())?;
    Ok((
        Py::from(PyBytes::new(py, &original)),
        hex::encode(hash.as_ref()),
    ))
}

#[pyfunction]
#[pyo3(name = "encode_private_transaction_counters_manifest_v1")]
fn encode_manifest(py: Python<'_>, manifest_json: &str) -> PyResult<(Py<PyBytes>, String)> {
    let manifest: PrivateCountersManifestV1 =
        bounded_json(manifest_json, MAX_PRIVATE_COUNTER_MANIFEST_BYTES_V1)?;
    let original = manifest.encode_canonical().map_err(|_| refused())?;
    let hash = manifest.commitment().map_err(|_| refused())?;
    Ok((
        Py::from(PyBytes::new(py, &original)),
        hex::encode(hash.as_ref()),
    ))
}

#[pyfunction]
#[pyo3(name = "commitments_private_transaction_counters_v1")]
fn original_commitments(
    original_policy: &[u8],
    original_manifest: &[u8],
    expected_run_binding_json: &str,
) -> PyResult<(String, String)> {
    let policy = PrivateCountersPolicyV1::decode_bounded_canonical(original_policy)
        .map_err(|_| refused())?;
    let manifest = PrivateCountersManifestV1::decode_bounded_canonical(original_manifest)
        .map_err(|_| refused())?;
    let expected_run_binding: CounterRunBindingV1 = bounded_json(
        expected_run_binding_json,
        MAX_PRIVATE_COUNTER_POLICY_BYTES_V1,
    )?;
    expected_run_binding.validate().map_err(|_| refused())?;
    if policy.purpose != expected_run_binding.purpose()
        || policy.run_binding != expected_run_binding
    {
        return Err(refused());
    }
    manifest
        .validate_against_policy(&policy)
        .map_err(|_| refused())?;
    let policy_hash = policy.commitment().map_err(|_| refused())?;
    let manifest_hash = manifest.commitment().map_err(|_| refused())?;
    Ok((
        hex::encode(policy_hash.as_ref()),
        hex::encode(manifest_hash.as_ref()),
    ))
}

fn original_executable(executable: &Executable) -> PyResult<(Vec<u8>, String)> {
    let length = norito::canonical_frame_len(executable).map_err(|_| refused())?;
    if length == 0 || length > MAX_AUTHORED_EXECUTABLE {
        return Err(refused());
    }
    let original = norito::encode_canonical(executable).map_err(|_| refused())?;
    let hash = HashOf::<Executable>::try_new(executable).map_err(|_| refused())?;
    Ok((original, hex::encode(hash.as_ref())))
}

#[pyfunction]
#[pyo3(name = "authored_private_transaction_counters_executable_v1")]
fn authored_executable(
    py: Python<'_>,
    builder: PyRef<'_, super::TransactionBuilder>,
) -> PyResult<(Py<PyBytes>, String)> {
    builder.validate_executable()?;
    let current = builder.to_model_builder();
    let (original, hash) = original_executable(&current.payload().instructions)?;
    Ok((Py::from(PyBytes::new(py, &original)), hash))
}

#[pyfunction]
#[pyo3(name = "authored_private_transaction_counters_prepared_executable_v1")]
fn authored_prepared_executable(
    py: Python<'_>,
    executable_json: &str,
) -> PyResult<(Py<PyBytes>, String)> {
    let executable: Executable = bounded_json(executable_json, MAX_AUTHORED_EXECUTABLE)?;
    let (original, hash) = original_executable(&executable)?;
    Ok((Py::from(PyBytes::new(py, &original)), hash))
}

#[pyfunction]
#[pyo3(name = "private_transaction_counters_run_commitment_v1")]
fn run_commitment(run_binding_json: &str) -> PyResult<String> {
    let binding: CounterRunBindingV1 =
        bounded_json(run_binding_json, MAX_PRIVATE_COUNTER_POLICY_BYTES_V1)?;
    Ok(hex::encode(
        binding.commitment().map_err(|_| refused())?.as_ref(),
    ))
}

#[pyfunction]
#[pyo3(name = "authored_private_transaction_counters_signed_executable_v1")]
fn authored_signed_executable(
    py: Python<'_>,
    original_signed_transaction_versioned: &[u8],
    expected_network_id: &super::PyNetworkId,
    expected_authority: &str,
) -> PyResult<(Py<PyBytes>, String)> {
    if original_signed_transaction_versioned.is_empty()
        || original_signed_transaction_versioned.len() > MAX_AUTHORED_EXECUTABLE
    {
        return Err(refused());
    }
    let signed =
        super::decode_canonical_signed_transaction_v1(original_signed_transaction_versioned)?;
    let authority = super::parse_exact_i105_account_id(expected_authority, "authored authority")?;
    if signed.network_id() != Some(expected_network_id.as_inner())
        || signed.authority() != &authority
    {
        return Err(refused());
    }
    signed.verify_signature().map_err(|_| refused())?;
    let (original, hash) = original_executable(signed.instructions())?;
    Ok((Py::from(PyBytes::new(py, &original)), hash))
}

#[pyfunction]
#[pyo3(name = "private_transaction_counters_plan_commitment_v1")]
fn plan_commitment(semantics_json: &str, executable_bindings_json: &str) -> PyResult<String> {
    let semantics: Vec<CounterSemanticV1> =
        bounded_json(semantics_json, MAX_PRIVATE_COUNTER_MANIFEST_BYTES_V1)?;
    let bindings: Vec<CounterExecutableBindingV1> = bounded_json(
        executable_bindings_json,
        MAX_PRIVATE_COUNTER_POLICY_BYTES_V1,
    )?;
    let commitment = counter_plan_commitment_v1(&semantics, &bindings).map_err(|_| refused())?;
    Ok(hex::encode(commitment.as_ref()))
}

fn categorical_key(
    group: &iroha_data_model::private_transaction_counters::CounterGroupKeyV1,
) -> json::Value {
    let party = match group.party {
        CounterPartyV1::None => "None",
        CounterPartyV1::BankA => "BankA",
        CounterPartyV1::BankB => "BankB",
        CounterPartyV1::Court => "Court",
        CounterPartyV1::Psp1 => "Psp1",
        CounterPartyV1::Psp2 => "Psp2",
    };
    let role = match group.role {
        CounterTransactionRoleV1::Business => "Business",
        CounterTransactionRoleV1::Control => "Control",
    };
    let category = match group.category {
        CounterCategoryV1::Availability => "Availability",
        CounterCategoryV1::BankLink => "BankLink",
        CounterCategoryV1::BatchLeg => "BatchLeg",
        CounterCategoryV1::Blocked => "Blocked",
        CounterCategoryV1::Control => "Control",
        CounterCategoryV1::CourtOrder => "CourtOrder",
        CounterCategoryV1::EscrowPending => "EscrowPending",
        CounterCategoryV1::FacilityDraw => "FacilityDraw",
        CounterCategoryV1::Issuance => "Issuance",
        CounterCategoryV1::Mint => "Mint",
        CounterCategoryV1::Mixed => "Mixed",
        CounterCategoryV1::Policy => "Policy",
        CounterCategoryV1::Release => "Release",
        CounterCategoryV1::Reserve => "Reserve",
        CounterCategoryV1::Seize => "Seize",
        CounterCategoryV1::Seizure => "Seizure",
        CounterCategoryV1::Transfer => "Transfer",
        CounterCategoryV1::WalletRegistration => "WalletRegistration",
    };
    let result = match group.result {
        CounterResultV1::Applied => "Applied",
        CounterResultV1::Rejected => "Rejected",
    };
    let rejection = match group.rejection {
        CounterRejectionV1::None => "None",
        CounterRejectionV1::BelowMinimum => "BelowMinimum",
        CounterRejectionV1::CapabilityNotRegistered => "CapabilityNotRegistered",
        CounterRejectionV1::HoldingLimitExceeded => "HoldingLimitExceeded",
        CounterRejectionV1::IncomingDisabled => "IncomingDisabled",
        CounterRejectionV1::InsufficientBalance => "InsufficientBalance",
        CounterRejectionV1::NotPermitted => "NotPermitted",
        CounterRejectionV1::WalletInactive => "WalletInactive",
        CounterRejectionV1::WalletLimitExceeded => "WalletLimitExceeded",
    };
    norito::json!({"party":party,"role":role,"category":category,"result":result,"rejection":rejection})
}

#[pyfunction]
#[pyo3(name = "build_private_transaction_counters_request_v1")]
fn build_request(
    py: Python<'_>,
    private_key: &[u8],
    time_to_live_ms: u64,
    original_policy: &[u8],
    native_finality_proof_chain_json: &str,
    expected_json: &str,
    expected_chain: &str,
    trusted_checkpoint: &[u8],
) -> PyResult<Py<PyBytes>> {
    if private_key.len() != 32 {
        return Err(refused());
    }
    let expected: PrivateCountersExpectedV1 =
        bounded_json(expected_json, MAX_PRIVATE_COUNTER_FRAME_BYTES_V1)?;
    let page = selected_page(
        &expected,
        native_finality_proof_chain_json,
        expected_chain,
        trusted_checkpoint,
    )?;
    let policy = PrivateCountersPolicyV1::decode_bounded_canonical(original_policy)
        .map_err(|_| refused())?;
    if policy.commitment().map_err(|_| refused())? != expected.policy_hash {
        return Err(refused());
    }
    // No host reproduces a cut, context hash, signing domain or clock. The original
    // independently selected prefix authenticates the cut before native construction.
    let request = PrivateCountersRequestV1 {
        domain: PRIVATE_COUNTER_REQUEST_DOMAIN_V1,
        version: 1,
        network_id: expected.network_id,
        scope: expected.scope,
        authority: expected.authority,
        purpose: expected.purpose,
        policy_hash: expected.policy_hash,
        manifest_hash: expected.manifest_hash,
        cut: CounterCutV1::from_verified(page.tip()).map_err(|_| refused())?,
        creation_time_ms: native_now_ms()?,
        time_to_live_ms: NonZeroU64::new(time_to_live_ms).ok_or_else(refused)?,
        nonce: expected.nonce,
    };
    request
        .validate_at(&policy, request.creation_time_ms)
        .map_err(|_| refused())?;
    let key =
        KeyPair::from_private_key(super::parse_private_key(private_key)?).map_err(|_| refused())?;
    let original = request
        .try_sign(&key)
        .map_err(|_| refused())?
        .encode_canonical()
        .map_err(|_| refused())?;
    Ok(Py::from(PyBytes::new(py, &original)))
}

#[pyfunction]
#[pyo3(name = "collect_private_transaction_counters_v1")]
fn collect(py: Python<'_>, original_responses: &Bound<'_, PyList>) -> PyResult<Py<PyBytes>> {
    if original_responses.is_empty() || original_responses.len() > 31 {
        return Err(refused());
    }
    let mut originals = Vec::new();
    originals
        .try_reserve_exact(original_responses.len())
        .map_err(|_| refused())?;
    for item in original_responses.iter() {
        let original = item.cast::<PyBytes>().map_err(|_| refused())?;
        if original.as_bytes().is_empty()
            || original.as_bytes().len() > MAX_PRIVATE_COUNTER_FRAME_BYTES_V1
        {
            return Err(refused());
        }
        let mut retained = Vec::new();
        retained
            .try_reserve_exact(original.as_bytes().len())
            .map_err(|_| refused())?;
        retained.extend_from_slice(original.as_bytes());
        originals.push(retained);
    }
    let certificate = collect_private_counters_v1(&originals).map_err(|_| refused())?;
    Ok(Py::from(PyBytes::new(py, &certificate)))
}

#[pyfunction]
#[pyo3(name = "verify_private_transaction_counters_v1")]
fn verify(
    py: Python<'_>,
    original_request: &[u8],
    original_certificate: &[u8],
    original_policy: &[u8],
    native_finality_proof_chain_json: &str,
    expected_json: &str,
    expected_chain: &str,
    trusted_checkpoint: &[u8],
) -> PyResult<(String, Py<PyBytes>)> {
    let expected: PrivateCountersExpectedV1 =
        bounded_json(expected_json, MAX_PRIVATE_COUNTER_FRAME_BYTES_V1)?;
    let page = selected_page(
        &expected,
        native_finality_proof_chain_json,
        expected_chain,
        trusted_checkpoint,
    )?;
    let verified = verify_private_counters_v1(
        original_request,
        original_certificate,
        original_policy,
        &page,
        &expected,
        native_now_ms()?,
    )
    .map_err(|_| refused())?;
    // Neither projection nor checkpoint is exposed before native context, exact
    // original signatures, clock, installed distinct-member quorum and cut admission.
    let groups = verified
        .claim()
        .groups
        .iter()
        .map(|group| norito::json!({"key":(categorical_key(&group.key)),"count":(group.count)}))
        .collect::<Vec<_>>();
    let claim = json::to_value(verified.claim()).map_err(|_| refused())?;
    let projection = json::to_json_bounded(
        &norito::json!({"claim":claim,"groups":groups}),
        MAX_PRIVATE_COUNTER_FRAME_BYTES_V1 * 4,
    )
    .map_err(|_| refused())?;
    let promoted = page
        .checkpoint()
        .encode_canonical()
        .map_err(|_| refused())?;
    Ok((projection, Py::from(PyBytes::new(py, &promoted))))
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(private_genesis_authority, module)?)?;
    module.add_function(wrap_pyfunction!(encode_policy, module)?)?;
    module.add_function(wrap_pyfunction!(encode_manifest, module)?)?;
    module.add_function(wrap_pyfunction!(original_commitments, module)?)?;
    module.add_function(wrap_pyfunction!(authored_executable, module)?)?;
    module.add_function(wrap_pyfunction!(authored_prepared_executable, module)?)?;
    module.add_function(wrap_pyfunction!(run_commitment, module)?)?;
    module.add_function(wrap_pyfunction!(authored_signed_executable, module)?)?;
    module.add_function(wrap_pyfunction!(plan_commitment, module)?)?;
    module.add_function(wrap_pyfunction!(build_request, module)?)?;
    module.add_function(wrap_pyfunction!(collect, module)?)?;
    module.add_function(wrap_pyfunction!(verify, module)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn private_genesis_fixture() -> iroha_core::sumeragi::test_chain::CertifiedTestChain {
        use std::num::NonZeroU32;

        use iroha_core::{
            state::World,
            sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
        };
        use iroha_crypto::Algorithm;
        use iroha_data_model::{
            Registrable,
            account::{Account, AccountId},
            asset::{
                Asset, AssetBalancePolicy, AssetBalanceScope, AssetDefinition, AssetDefinitionId,
                AssetId,
            },
            block::consensus::PrivateRootFeePolicy,
            domain::Domain,
            nexus::{DataSpaceCatalog, DataSpaceMetadata, LaneCatalog, LaneConfig, LaneVisibility},
            parameter::Parameter,
        };
        use iroha_model_base::{domain::DomainId, topology::DataSpaceId};

        let dataspace_id = DataSpaceId::new(7);
        let genesis_key = KeyPair::from_seed(vec![0xCE; 32], Algorithm::Ed25519);
        let clock_key = KeyPair::from_seed(vec![0xCC; 32], Algorithm::Ed25519);
        let ids: Vec<_> = [&genesis_key, &clock_key]
            .into_iter()
            .map(|key| AccountId::new(key.public_key().clone()))
            .collect();
        let owner = ids[0].clone();
        let domain = DomainId::parse_fully_qualified("app.sdk-private-genesis-test").unwrap();
        let fee_asset =
            AssetDefinitionId::derive_from_components(domain.clone(), "gas".parse().unwrap());
        let mut definition = AssetDefinition::numeric(
            fee_asset.clone(),
            "SDK private genesis gas",
            AssetBalancePolicy::DataspaceRestricted,
            Some(domain.clone()),
        )
        .build(&owner);
        definition.total_quantity = 5_000_000_u32.into();
        let world = World::with_assets(
            [Domain::new(domain).build(&owner)],
            ids.iter().map(|id| Account::new(id.clone()).build(&owner)),
            [definition],
            ids.iter().map(|id| {
                Asset::new(
                    AssetId::with_scope(
                        fee_asset.clone(),
                        id.clone(),
                        AssetBalanceScope::Dataspace(dataspace_id),
                    ),
                    2_500_000_u32,
                )
            }),
            [],
        );
        let mut config = TestChainConfig::new(world, 1_000);
        assert_eq!(config.genesis_key.public_key(), genesis_key.public_key());
        config.genesis_parameters.push(Parameter::Custom(
            PrivateRootFeePolicy {
                asset_definition_id: fee_asset.clone(),
                base_fee: 1_u32.into(),
                per_byte_fee: 0_u32.into(),
                per_instruction_fee: 1_u32.into(),
                per_gas_unit_fee: 1_u32.into(),
            }
            .into_custom_parameter()
            .unwrap(),
        ));
        config.root_scope = SumeragiRootScope::Dataspace {
            parent_network_id: iroha_data_model::NetworkId::from_genesis_hash(
                HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(b"synthetic SDK parent")),
            ),
            dataspace_id,
        };
        // TestChain's pre-genesis owner derives runtime lane geometry from this
        // authoritative catalog before signing or executing the original genesis.
        config.nexus = Some(Default::default());
        let nexus = config.nexus.as_mut().unwrap();
        nexus.fees.fee_asset_id = fee_asset.to_string();
        nexus.lane_catalog = LaneCatalog::new(
            NonZeroU32::new(1).unwrap(),
            vec![LaneConfig {
                dataspace_id,
                visibility: LaneVisibility::Restricted,
                ..LaneConfig::default()
            }],
        )
        .unwrap();
        nexus.configured_lane_catalog = nexus.lane_catalog.clone();
        nexus.dataspace_catalog = DataSpaceCatalog::new(vec![DataSpaceMetadata {
            id: dataspace_id,
            alias: "sdk-private-genesis-test".into(),
            description: None,
            fault_tolerance: 1,
        }])
        .unwrap();
        nexus.configured_dataspace_catalog = nexus.dataspace_catalog.clone();
        nexus.routing_policy.default_dataspace = dataspace_id;
        CertifiedTestChain::start(config).unwrap()
    }

    #[test]
    fn private_genesis_authority_uses_the_original_native_single_signer() {
        let chain = private_genesis_fixture();
        let original = chain.genesis().encode_wire().unwrap();
        let authority = private_genesis_authority(&original).unwrap();
        assert_eq!(authority, chain.genesis_account().to_string());
        assert_eq!(
            super::super::parse_exact_i105_account_id(&authority, "fixture authority").unwrap(),
            *chain.genesis_account(),
        );
        let mut trailing = original;
        trailing.push(0);
        assert!(private_genesis_authority(&trailing).is_err());
    }

    #[test]
    fn private_genesis_authority_refuses_global_non_genesis_and_foreign_signature() {
        use iroha_core::{
            state::World,
            sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
        };
        let global =
            CertifiedTestChain::start(TestChainConfig::new(World::default(), 1_000)).unwrap();
        assert!(private_genesis_authority(&global.genesis().encode_wire().unwrap()).is_err());
        let mut chain = private_genesis_fixture();
        // This independently decoded offchain original is untrusted syntax. The
        // negative control alters its witness without replacing a funded owner.
        let mut tampered =
            decode_framed_signed_block(&chain.genesis().encode_wire().unwrap()).unwrap();
        let foreign = KeyPair::from_seed(vec![0xC9; 32], iroha_crypto::Algorithm::Ed25519);
        let signature = iroha_data_model::block::BlockSignature::new(
            0,
            iroha_crypto::SignatureOf::new(foreign.private_key(), &tampered.header()),
        );
        tampered
            .replace_signatures(
                iroha_data_model::block::BlockSignatures::try_from_iter([signature]).unwrap(),
            )
            .unwrap();
        assert!(private_genesis_authority(&tampered.encode_wire().unwrap()).is_err());
        chain.commit(Vec::new());
        assert!(
            private_genesis_authority(&chain.committed(2).block().encode_wire().unwrap()).is_err()
        );
        assert!(private_genesis_authority(&[]).is_err());
    }

    #[test]
    fn construction_json_is_bounded_before_native_decoding() {
        assert!(bounded_json::<PrivateCountersExpectedV1>("", 2).is_err());
        assert!(bounded_json::<PrivateCountersExpectedV1>("xxx", 2).is_err());
        assert!(bounded_json::<PrivateCountersExpectedV1>("{}", 2).is_err());
    }

    #[test]
    fn native_clock_is_observed_independently() {
        assert!(native_now_ms().unwrap() > 1_700_000_000_000);
    }

    #[test]
    fn executable_original_and_hash_use_the_actual_native_model() {
        let executable = Executable::from(Vec::<iroha_data_model::isi::InstructionBox>::new());
        let (original, hash) = original_executable(&executable).unwrap();
        let decoded: Executable = norito::decode_canonical_with_limits(
            &original,
            norito::canonical_decode_limits(MAX_AUTHORED_EXECUTABLE),
        )
        .unwrap();
        assert_eq!(decoded, executable);
        assert_eq!(
            hash,
            hex::encode(HashOf::<Executable>::try_new(&executable).unwrap().as_ref())
        );
    }

    #[test]
    fn display_key_is_closed_categorical_data_without_native_evidence() {
        let key = iroha_data_model::private_transaction_counters::CounterGroupKeyV1 {
            party: CounterPartyV1::Psp1,
            role: CounterTransactionRoleV1::Business,
            category: CounterCategoryV1::Transfer,
            result: CounterResultV1::Applied,
            rejection: CounterRejectionV1::None,
        };
        let display = categorical_key(&key);
        assert_eq!(
            display,
            norito::json!({"party":"Psp1","role":"Business","category":"Transfer",
            "result":"Applied","rejection":"None"})
        );
    }
}
