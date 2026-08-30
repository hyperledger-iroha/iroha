use iroha_torii_shared::offline_api::{
    OfflineOperationKind, OfflineOperationReference, OfflineOperationState,
};

pub(super) struct KagemushaOperationReferenceProjectionV1 {
    pub(super) operation_id: [u8; 32],
    pub(super) kind: &'static str,
    pub(super) transaction_hash: [u8; 32],
    pub(super) status_uri: String,
    pub(super) submitted_at_ms: u64,
}

fn exact_lower_hex_32(value: &str, field: &'static str) -> Result<[u8; 32], String> {
    if value.len() != 64
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{field} must be exact lowercase 32-byte hex"));
    }
    let mut decoded = [0_u8; 32];
    hex::decode_to_slice(value, &mut decoded)
        .map_err(|_| format!("{field} must be exact lowercase 32-byte hex"))?;
    if decoded == [0; 32] {
        return Err(format!("{field} must be non-zero"));
    }
    Ok(decoded)
}

pub(super) fn project_kagemusha_operation_reference_v1(
    reference: OfflineOperationReference,
    expected_operation_id: &str,
    expected_kind: &str,
    expected_submitted_at_ms: u64,
) -> Result<KagemushaOperationReferenceProjectionV1, String> {
    let expected_operation_id_bytes =
        exact_lower_hex_32(expected_operation_id, "expectedOperationId")?;
    let operation_id = exact_lower_hex_32(&reference.operation_id, "operationId")?;
    if operation_id != expected_operation_id_bytes
        || reference.operation_id != expected_operation_id
    {
        return Err("operation reference returned another operation id".to_owned());
    }
    let kind = match reference.kind {
        OfflineOperationKind::TopUp => "top_up",
        OfflineOperationKind::Redeem => "redeem",
    };
    if kind != expected_kind {
        return Err("operation reference returned another operation kind".to_owned());
    }
    if reference.state != OfflineOperationState::Pending {
        return Err("operation reference is not in the exact Pending state".to_owned());
    }
    let transaction_hash = exact_lower_hex_32(&reference.transaction_hash, "transactionHash")?;
    let expected_status_uri = format!("/v1/offline/operations/{expected_operation_id}");
    if reference.status_uri != expected_status_uri {
        return Err("operation reference status URI is not canonical".to_owned());
    }
    if expected_submitted_at_ms == 0 || reference.submitted_at_ms != expected_submitted_at_ms {
        return Err(
            "operation reference submitted_at_ms differs from the retained request".to_owned(),
        );
    }
    Ok(KagemushaOperationReferenceProjectionV1 {
        operation_id,
        kind,
        transaction_hash,
        status_uri: reference.status_uri,
        submitted_at_ms: reference.submitted_at_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> OfflineOperationReference {
        OfflineOperationReference {
            operation_id: "11".repeat(32),
            kind: OfflineOperationKind::TopUp,
            state: OfflineOperationState::Pending,
            transaction_hash: "22".repeat(32),
            status_uri: format!("/v1/offline/operations/{}", "11".repeat(32)),
            submitted_at_ms: 1_900_000_000_000,
        }
    }

    #[test]
    fn exact_operation_reference_projection_rejects_every_route_binding_mutation() {
        let projected = project_kagemusha_operation_reference_v1(
            fixture(),
            &"11".repeat(32),
            "top_up",
            1_900_000_000_000,
        )
        .expect("project exact operation reference");
        assert_eq!(projected.operation_id, [0x11; 32]);
        assert_eq!(projected.transaction_hash, [0x22; 32]);

        let mut mutations = Vec::new();
        let mut foreign_id = fixture();
        foreign_id.operation_id = "33".repeat(32);
        mutations.push(foreign_id);
        let mut foreign_hash = fixture();
        foreign_hash.transaction_hash = "00".repeat(32);
        mutations.push(foreign_hash);
        let mut foreign_uri = fixture();
        foreign_uri.status_uri = "/v1/offline/operations/foreign".to_owned();
        mutations.push(foreign_uri);
        for mutated in mutations {
            assert!(
                project_kagemusha_operation_reference_v1(
                    mutated,
                    &"11".repeat(32),
                    "top_up",
                    1_900_000_000_000,
                )
                .is_err()
            );
        }
        assert!(
            project_kagemusha_operation_reference_v1(
                fixture(),
                &"11".repeat(32),
                "redeem",
                1_900_000_000_000,
            )
            .is_err()
        );
        assert!(
            project_kagemusha_operation_reference_v1(
                fixture(),
                &"11".repeat(32),
                "top_up",
                1_900_000_000_001,
            )
            .is_err()
        );
    }
}
