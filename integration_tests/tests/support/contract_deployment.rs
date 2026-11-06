//! Exact locally signed contract fixture deployment shared by native integration scenarios.

use eyre::{Result, eyre};
use integration_tests::manifest::ManifestEncodingBudget;
use iroha::data_model::prelude::*;
use iroha_executor_data_model::permission::account::{
    AccountAliasPermissionScope, CanManageAccountAlias,
};
use iroha_model_base::{metadata::Metadata, name::Name};
use std::str::FromStr as _;

/// Grant management of the exact universal alias authored by a contract fixture.
pub(crate) fn contract_alias_management_permission(name: &str) -> CanManageAccountAlias {
    CanManageAccountAlias {
        scope: AccountAliasPermissionScope::Alias(
            iroha_data_model::alias_setup::ResolvedAccountAliasV1::new(
                format!("{name}@universal")
                    .parse()
                    .expect("canonical contract fixture alias permission"),
                iroha_model_base::topology::DataSpaceId::UNIVERSAL,
            ),
        ),
    }
}

/// Commit a deployment and optionally delegate only its initialization entrypoint.
pub(crate) fn contract_deployment_instructions(
    commit: iroha_data_model::isi::smart_contract_code::CommitContractDeployment,
    hajimari_grantee: Option<&AccountId>,
) -> Vec<InstructionBox> {
    let contract_address = commit.contract_address.clone();
    let mut instructions = vec![InstructionBox::from(commit)];
    if let Some(grantee) = hajimari_grantee {
        // Deployment stages the hook; its caller still needs an exact invocation grant.
        // Keep that grant after Commit in the same ordinary ordered transaction.
        let permission: Permission =
            iroha_executor_data_model::permission::smart_contract::CanInvokeContractEntrypoint {
                contract: contract_address,
                entrypoint: "hajimari".to_owned(),
            }
            .into();
        instructions.push(Grant::account_permission(permission, grantee.clone()).into());
    }
    instructions
}

/// Verify, upload and deploy an artifact using exact locally signed transactions.
pub(crate) fn deploy_contract_locally_signed(
    client: &iroha::blocking::Client,
    artifact: &[u8],
    contract_alias: iroha_data_model::smart_contract::ContractAlias,
) -> Result<(
    iroha_data_model::smart_contract::ContractAddress,
    String,
    String,
    HashOf<iroha_data_model::transaction::SignedTransaction>,
)> {
    deploy_contract_locally_signed_with_registration(client, artifact, contract_alias, false, None)
}

/// Deploy with explicit existing-artifact ownership and an optional initialization grant.
pub(crate) fn deploy_contract_locally_signed_with_registration(
    client: &iroha::blocking::Client,
    artifact: &[u8],
    contract_alias: iroha_data_model::smart_contract::ContractAlias,
    registered_in_genesis: bool,
    hajimari_grantee: Option<&AccountId>,
) -> Result<(
    iroha_data_model::smart_contract::ContractAddress,
    String,
    String,
    HashOf<iroha_data_model::transaction::SignedTransaction>,
)> {
    use iroha_data_model::isi::smart_contract_code::{
        CommitContractDeployment, FinalizeSmartContractCodeUpload, RegisterSmartContractCode,
        SMART_CONTRACT_CODE_CHUNK_BYTES, UploadSmartContractCodeChunk,
    };
    let verified = ivm::verify_contract_artifact(artifact)
        .map_err(|error| eyre!("verify contract artifact: {error}"))?;
    let authority: Account = client
        .client()
        .query_single(FindAccountById::new(client.client().account().clone()))?;
    let nonce_key =
        Name::from_str(iroha_data_model::smart_contract::CONTRACT_DEPLOY_NONCE_METADATA_KEY)?;
    let deploy_nonce = authority
        .metadata()
        .get(&nonce_key)
        .map(|value| {
            value
                .try_into_any_norito::<u64>()
                .map_err(|_| eyre!("contract deployment nonce metadata is not a canonical u64"))
        })
        .transpose()?
        .unwrap_or(0);
    let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
        client.client().network_id(),
        client.client().account(),
        deploy_nonce,
        iroha_model_base::topology::DataSpaceId::UNIVERSAL,
    )
    .map_err(|error| eyre!("derive contract address: {error}"))?;
    let mut metadata = Metadata::default();
    for key in ["gov_contract_address", "contract_address"] {
        metadata.insert(
            Name::from_str(key)?,
            iroha_primitives::json::Json::new(contract_address.to_string()),
        );
    }
    if !registered_in_genesis {
        let encoding = ManifestEncodingBudget::new()?;
        let manifest = verified
            .manifest
            .try_signed(
                encoding.context(),
                encoding.max_frame_bytes(),
                client.client().key_pair(),
            )
            .map_err(|error| eyre!("sign contract manifest locally: {error}"))?;
        let total_size = u64::try_from(artifact.len())?;
        let chunk_count = u32::try_from(artifact.len().div_ceil(SMART_CONTRACT_CODE_CHUNK_BYTES))?;
        if chunk_count == 0 {
            return Err(eyre!("contract artifact must not be empty"));
        }
        for (index, chunk) in artifact.chunks(SMART_CONTRACT_CODE_CHUNK_BYTES).enumerate() {
            let chunk_index = u32::try_from(index)?;
            let mut instructions = vec![InstructionBox::from(UploadSmartContractCodeChunk {
                artifact_id: iroha_data_model::smart_contract::ContractArtifactId::new(
                    iroha_model_base::topology::DataSpaceId::UNIVERSAL,
                    verified.code_hash,
                ),
                total_size,
                chunk_index,
                chunk_count,
                chunk: chunk.to_vec(),
            })];
            if chunk_index + 1 == chunk_count {
                instructions.push(InstructionBox::from(FinalizeSmartContractCodeUpload {
                    artifact_id: iroha_data_model::smart_contract::ContractArtifactId::new(
                        iroha_model_base::topology::DataSpaceId::UNIVERSAL,
                        verified.code_hash,
                    ),
                    total_size,
                    chunk_count,
                }));
            }
            client.submit_all_with_metadata(
                instructions,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                metadata.clone(),
            )?;
        }
        client.submit_with_metadata(
            {
                let scoped_manifest = manifest;
                RegisterSmartContractCode {
                    artifact_id: iroha_data_model::smart_contract::ContractArtifactId::new(
                        iroha_model_base::topology::DataSpaceId::UNIVERSAL,
                        scoped_manifest.code_hash.unwrap_or_else(|| {
                            iroha_crypto::Hash::new(b"missing test manifest hash")
                        }),
                    ),
                    manifest: scoped_manifest,
                }
            },
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            metadata.clone(),
        )?;
    }
    let deployment_instructions = contract_deployment_instructions(
        CommitContractDeployment {
            expected_deploy_nonce: deploy_nonce,
            contract_address: contract_address.clone(),
            code_hash: verified.code_hash,
            contract_alias,
            lease_expiry_ms: None,
            expected_previous_contract_address: None,
        },
        hajimari_grantee,
    );
    let deployment_tx_hash = client.submit_all_with_metadata(
        deployment_instructions,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        metadata,
    )?;
    Ok((
        contract_address,
        hex::encode(verified.code_hash.as_ref()),
        hex::encode(verified.abi_hash.as_ref()),
        deployment_tx_hash,
    ))
}
