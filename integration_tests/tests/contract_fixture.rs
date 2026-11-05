//! Real deployment and exact instance authority for IVM integration samples.

use eyre::{Result, ensure, eyre};
use iroha::blocking::Client;
use iroha_data_model::{
    isi::smart_contract_code::{
        CommitContractDeployment, FinalizeSmartContractCodeUpload, RegisterSmartContractCode,
        SMART_CONTRACT_CODE_CHUNK_BYTES, UploadSmartContractCodeChunk,
    },
    prelude::*,
    smart_contract::{
        ContractAddress, ContractAlias, ContractArtifactId,
        manifest::{ContractPermissionScopeV1, EntrypointAuthorizationV1},
    },
};
use iroha_executor_data_model::permission::{
    account::{AccountAliasPermissionScope, CanManageAccountAlias},
    smart_contract::{CanManageSmartContractCode, CanUseContractPermission},
};
use iroha_model_base::{metadata::Metadata, name::Name, topology::DataSpaceId};

/// Exact genesis grants needed to register code and deploy universal sample aliases.
pub fn deployment_permissions(
    authority: &AccountId,
    aliases: &[ContractAlias],
) -> Vec<InstructionBox> {
    let mut permissions = vec![
        Grant::account_permission(
            Permission::from(CanManageSmartContractCode),
            authority.clone(),
        )
        .into(),
    ];
    for alias in aliases {
        assert_eq!(
            alias.dataspace_segment(),
            "universal",
            "sample fixture data space"
        );
        let scope = iroha_data_model::alias_setup::AccountAliasName::try_new(
            alias.name_segment(),
            alias.domain_segment(),
            alias.dataspace_segment(),
        )
        .expect("canonical contract alias components");
        let permission = Grant::account_permission(
            CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Alias(
                    iroha_data_model::alias_setup::ResolvedAccountAliasV1::new(
                        scope,
                        DataSpaceId::UNIVERSAL,
                    ),
                ),
            },
            authority.clone(),
        )
        .into();
        if !permissions.contains(&permission) {
            permissions.push(permission);
        }
    }
    permissions
}

/// Bind raw sample bytecode to its live alias and declared `run` callback.
pub fn call_metadata(alias: &ContractAlias) -> Metadata {
    let mut metadata = Metadata::default();
    metadata.insert(
        "contract_alias".parse().expect("static alias key"),
        Json::new(alias.to_string()),
    );
    metadata.insert(
        "contract_entrypoint".parse().expect("static callback key"),
        Json::new("run"),
    );
    metadata
}

fn sample_permission(artifact: &[u8]) -> Result<Name> {
    let verified = ivm::verify_contract_artifact(artifact)
        .map_err(|error| eyre!("verify sample artifact: {error}"))?;
    let entrypoint = verified
        .manifest
        .entrypoints
        .as_ref()
        .and_then(|entries| entries.iter().find(|entry| entry.name == "run"))
        .ok_or_else(|| eyre!("sample must declare run"))?;
    let EntrypointAuthorizationV1::Permission(name) = &entrypoint.authorization else {
        return Err(eyre!(
            "sample run must require its declared instance permission"
        ));
    };
    ensure!(
        verified
            .manifest
            .permissions
            .iter()
            .any(|permission| permission.name == *name
                && permission.scope == ContractPermissionScopeV1::Instance),
        "sample run permission must have instance scope"
    );
    Ok(name.clone())
}

/// Upload, sign and commit a sample, then grant its instance and exact effect permissions.
///
/// Uses ordinary paid transactions and the current account deployment nonce. The
/// caller must seed [`deployment_permissions`] and retain its original deadline.
/// Effect permissions are granted to the deployed subject; the invoking account
/// receives only the sample's declared entrypoint permission.
///
/// # Errors
/// Propagates malformed artifacts, missing authority, stale nonce, signing or any
/// authoritative transaction rejection; no failure is retried or reclassified.
pub fn deploy(
    client: &Client,
    artifact: &[u8],
    alias: ContractAlias,
    effect_permissions: &[Permission],
) -> Result<ContractAddress> {
    let encoding = integration_tests::manifest::ManifestEncodingBudget::new()?;
    let permission = sample_permission(artifact)?;
    let verified = ivm::verify_contract_artifact(artifact)
        .map_err(|error| eyre!("verify deployment artifact: {error}"))?;
    let authority = client.client().account().clone();
    let account = client
        .client()
        .query_single(FindAccountById::new(authority.clone()))?;
    let nonce_key: Name =
        iroha_data_model::smart_contract::CONTRACT_DEPLOY_NONCE_METADATA_KEY.parse()?;
    let nonce = account
        .metadata()
        .get(&nonce_key)
        .map(|value| value.try_into_any_norito::<u64>())
        .transpose()?
        .unwrap_or(0);
    let address = ContractAddress::derive(
        client.client().network_id(),
        &authority,
        nonce,
        DataSpaceId::UNIVERSAL,
    )
    .map_err(|error| eyre!("derive sample deployment address: {error}"))?;
    let mut metadata = Metadata::default();
    for key in ["gov_contract_address", "contract_address"] {
        metadata.insert(key.parse::<Name>()?, Json::new(address.to_string()));
    }
    let manifest = verified
        .manifest
        .try_signed(
            encoding.context(),
            encoding.max_frame_bytes(),
            client.client().key_pair(),
        )
        .map_err(|error| eyre!("sign sample manifest: {error}"))?;
    let artifact_id = ContractArtifactId::new(DataSpaceId::UNIVERSAL, verified.code_hash);
    let total_size = u64::try_from(artifact.len())?;
    let chunk_count = u32::try_from(artifact.len().div_ceil(SMART_CONTRACT_CODE_CHUNK_BYTES))?;
    ensure!(chunk_count > 0, "sample artifact must be nonempty");
    for (index, chunk) in artifact.chunks(SMART_CONTRACT_CODE_CHUNK_BYTES).enumerate() {
        let chunk_index = u32::try_from(index)?;
        let mut instructions = vec![InstructionBox::from(UploadSmartContractCodeChunk {
            artifact_id,
            total_size,
            chunk_index,
            chunk_count,
            chunk: chunk.to_vec(),
        })];
        if chunk_index + 1 == chunk_count {
            instructions.push(
                FinalizeSmartContractCodeUpload {
                    artifact_id,
                    total_size,
                    chunk_count,
                }
                .into(),
            );
        }
        client.submit_all_with_metadata(
            instructions,
            FeePaymentIntent::authority(Vec::new(), None),
            metadata.clone(),
        )?;
    }
    client.submit_with_metadata(
        RegisterSmartContractCode {
            artifact_id,
            manifest,
        },
        FeePaymentIntent::authority(Vec::new(), None),
        metadata.clone(),
    )?;
    let mut deployment = vec![
        InstructionBox::from(CommitContractDeployment {
            expected_deploy_nonce: nonce,
            contract_address: address.clone(),
            code_hash: verified.code_hash,
            contract_alias: alias,
            lease_expiry_ms: None,
            expected_previous_contract_address: None,
        }),
        Grant::account_permission(
            CanUseContractPermission {
                contract: address.clone(),
                permission,
            },
            authority,
        )
        .into(),
    ];
    deployment.extend(
        effect_permissions
            .iter()
            .cloned()
            .map(|permission| Grant::account_permission(permission, address.subject_id()).into()),
    );
    client.submit_all_with_metadata(
        deployment,
        FeePaymentIntent::authority(Vec::new(), None),
        metadata,
    )?;
    Ok(address)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_retain_declared_instance_authorization_and_reject_invalid_artifacts() {
        for (name, permission) in [
            ("mint_rose_trigger", "Admin"),
            ("query_assets_and_save_cursor", "Admin"),
            ("smart_contract_can_filter_queries", "Admin"),
            ("create_nft_for_every_user_trigger", "Entry"),
        ] {
            let artifact = iroha_test_samples::load_sample_ivm(name);
            assert_eq!(
                sample_permission(artifact.as_ref()).unwrap().as_ref(),
                permission
            );
            assert!(sample_permission(&artifact.as_ref()[..artifact.as_ref().len() - 1]).is_err());
        }
        assert!(sample_permission(&[]).is_err());
    }

    #[test]
    fn binding_and_deployment_grants_name_only_the_exact_alias() {
        let alias = ContractAlias::from_components("sample_fixture", None, "universal").unwrap();
        let metadata = call_metadata(&alias);
        assert_eq!(metadata.iter().len(), 2);
        let selected: String = metadata
            .get(&"contract_alias".parse::<Name>().unwrap())
            .unwrap()
            .try_into_any()
            .unwrap();
        assert_eq!(selected, alias.to_string());
        let callback: String = metadata
            .get(&"contract_entrypoint".parse::<Name>().unwrap())
            .unwrap()
            .try_into_any()
            .unwrap();
        assert_eq!(callback, "run");
        let grants = deployment_permissions(
            &iroha_test_samples::ALICE_ID,
            &[alias.clone(), alias.clone()],
        );
        assert_eq!(grants.len(), 2);
        let encoded = norito::to_bytes(&grants).unwrap();
        let decoded: Vec<InstructionBox> = norito::decode_from_bytes(&encoded).unwrap();
        assert_eq!(decoded, grants);
    }
}
