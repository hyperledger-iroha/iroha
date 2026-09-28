//! Immutable inventory custody for a terminal predecessor, never executable input.
//!
//! A successor authenticates the original inventory bytes and their signed topology.
//! It must not interpret the predecessor's completed beacon protocol with its own
//! executor. The opaque payload remains covered by the exact inventory digest; no
//! field is translated, regenerated, or admitted to the current execution path.

#[cfg(any(target_os = "linux", test))]
use super::*;

#[cfg(any(target_os = "linux", test))]
pub(super) type TerminalInventory = InventoryRecordV1<Value>;

#[cfg(any(target_os = "linux", test))]
pub(super) fn decode(
    bytes: &[u8],
    label: &str,
) -> Result<(TerminalInventory, ChainDiscriminantGuard)> {
    if bytes.is_empty() || bytes.len() as u64 > MAX_JSON_BYTES {
        return Err(eyre!(
            "{label} is empty or exceeds the inventory custody bound"
        ));
    }
    let guard = ChainDiscriminantGuard::enter(CHAIN_DISCRIMINANT);
    let inventory: TerminalInventory = json::from_slice(bytes)
        .map_err(|_| eyre!("{label} is not exact inventory custody JSON"))?;
    validate_inventory_custody_with_revision(&inventory, |revision| {
        validate_revision_source_fields(revision)?;
        validate_revision_build_fields(revision)
    })?;
    Ok((inventory, guard))
}

/// Authenticate the original historical bytes, not a re-encoded projection.
/// Its finite signed lease was admitted by its executor. A successor does not
/// recalculate that lease using a different execution plan or authorize a replay.
#[cfg(any(target_os = "linux", test))]
pub(super) fn verify_authorization(
    inventory: &TerminalInventory,
    inventory_sha256: &str,
    envelope: &AuthorizationEnvelopeV1,
    trusted: &TrustedKeyV1,
) -> Result<()> {
    verify_authorization_claims(inventory, inventory_sha256, envelope, trusted)?;
    verify_authorization_signature(envelope, trusted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{KeyPair, Signature};

    fn opaque_inventory_bytes() -> Vec<u8> {
        let inventory = sample_inventory_fixture();
        let mut value: Value =
            json::from_slice(&canonical_inventory_bytes(&inventory).unwrap()).unwrap();
        value.as_object_mut().unwrap().insert(
            "beacon_bootstrap".into(),
            norito::json!({"uninterpreted_execution": {"round": 17}}),
        );
        json::to_vec(&value).unwrap()
    }

    fn sign(
        inventory: &TerminalInventory,
        bytes: &[u8],
    ) -> (AuthorizationEnvelopeV1, TrustedKeyV1) {
        let key = KeyPair::try_random_with_algorithm(Algorithm::Ed25519).unwrap();
        let issued = 990_000;
        let claims = AuthorizationClaimsV1 {
            action: "reset_and_deploy".into(),
            qualification_scope: inventory.qualification_scope,
            deployment_id: inventory.deployment_id.clone(),
            inventory_sha256: sha256_hex(bytes),
            artifact_closure_sha256: inventory.artifact_closure_sha256.clone(),
            runtime_client_config_sha256: inventory.runtime_client_config_sha256.clone(),
            onboarding_token_sha256: inventory.onboarding_token_sha256.clone(),
            validator_client_configs_sha256: inventory.validator_client_configs_sha256.clone(),
            inrou_stage_tree_sha256: inventory.inrou_stage_tree_sha256.clone(),
            faucet_policy: inventory.faucet_policy.clone(),
            fee_intent: inventory.fee_intent.clone(),
            authorization_nonce: inventory.authorization_nonce.clone(),
            issued_at_unix_ms: issued,
            not_before_unix_ms: issued,
            expires_at_unix_ms: issued + MAX_AUTHORIZATION_LIFETIME_MS,
            // Historical custody does not imply the current executor's action count.
            execution_expires_at_unix_ms: issued + 2 * MAX_AUTHORIZATION_LIFETIME_MS,
        };
        let signature =
            Signature::try_new(key.private_key(), &authorization_message(&claims).unwrap())
                .unwrap();
        (
            AuthorizationEnvelopeV1 {
                schema: AUTHORIZATION_SCHEMA_V1.into(),
                claims,
                signature_hex: hex::encode(signature.payload()),
            },
            TrustedKeyV1 {
                schema: TRUSTED_KEY_SCHEMA_V1.into(),
                algorithm: "ed25519".into(),
                public_key: key.public_key().to_string(),
            },
        )
    }

    #[test]
    fn terminal_custody_does_not_admit_an_opaque_execution_payload() {
        let bytes = opaque_inventory_bytes();
        let (inventory, _guard) = decode(&bytes, "terminal").unwrap();
        let intent = inputs::ResetTopologyIntentV1::from(&inventory);
        inputs::validate_topology_intent(&intent).unwrap();
        assert!(decode_inventory(&bytes, "candidate").is_err());
        let (authorization, trusted) = sign(&inventory, &bytes);
        verify_authorization(&inventory, &sha256_hex(&bytes), &authorization, &trusted).unwrap();
    }

    #[test]
    fn terminal_custody_still_binds_exact_raw_bytes_claims_and_signer() {
        let bytes = opaque_inventory_bytes();
        let (inventory, _guard) = decode(&bytes, "terminal").unwrap();
        let (authorization, trusted) = sign(&inventory, &bytes);
        let mut reformatted = bytes.clone();
        reformatted.push(b'\n');
        assert!(
            verify_authorization(
                &inventory,
                &sha256_hex(&reformatted),
                &authorization,
                &trusted
            )
            .is_err()
        );
        let mut changed: Value = json::from_slice(&bytes).unwrap();
        changed
            .as_object_mut()
            .unwrap()
            .insert("beacon_bootstrap".into(), Value::Null);
        let changed = json::to_vec(&changed).unwrap();
        let (changed_inventory, _changed_guard) = decode(&changed, "terminal").unwrap();
        assert!(
            verify_authorization(
                &changed_inventory,
                &sha256_hex(&changed),
                &authorization,
                &trusted
            )
            .is_err()
        );
        let mut altered = inventory.clone();
        altered.authorization_nonce = "9".repeat(32);
        assert!(
            verify_authorization(&altered, &sha256_hex(&bytes), &authorization, &trusted).is_err()
        );
        let mut wrong_signer = trusted.clone();
        wrong_signer.public_key = KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
            .unwrap()
            .public_key()
            .to_string();
        assert!(
            verify_authorization(
                &inventory,
                &sha256_hex(&bytes),
                &authorization,
                &wrong_signer
            )
            .is_err()
        );
    }

    #[test]
    fn terminal_custody_rejects_wrong_chain_roles_artifacts_and_unknown_metadata() {
        let bytes = opaque_inventory_bytes();
        for (field, replacement) in [
            ("chain_id", Value::from("wrong-chain")),
            ("chain_discriminant", Value::from(753_u16)),
            ("authorization_nonce", Value::from("bad")),
            ("artifact_closure_sha256", Value::from("0".repeat(64))),
            ("validators", Value::Array(Vec::new())),
            ("unrecognized_metadata", Value::Null),
        ] {
            let mut value: Value = json::from_slice(&bytes).unwrap();
            value
                .as_object_mut()
                .unwrap()
                .insert(field.into(), replacement);
            assert!(
                decode(&json::to_vec(&value).unwrap(), "terminal").is_err(),
                "{field}"
            );
        }
    }
}
