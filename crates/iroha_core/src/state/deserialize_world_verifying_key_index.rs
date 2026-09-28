//! Exact verifying-key circuit/version inverse for current and predecessor WSV cuts.

use super::*;
use iroha_data_model::proof::{VerifyingKeyId, VerifyingKeyRecord};

fn validate_cut(
    keys: &impl StorageReadOnly<VerifyingKeyId, VerifyingKeyRecord>,
    by_circuit: &impl StorageReadOnly<(String, u32), VerifyingKeyId>,
    cut: &str,
) -> Result<(), json::Error> {
    let invalid = |message: &str| json::Error::InvalidField {
        field: "world.verifying_keys_by_circuit".to_owned(),
        message: format!("{cut} verifier-key index: {message}"),
    };
    if keys.len() != by_circuit.len() {
        return Err(invalid("registry and circuit index cardinalities differ"));
    }
    for (id, record) in keys.iter() {
        let key = (record.circuit_id.clone(), record.version);
        if by_circuit.get(&key) != Some(id) {
            return Err(invalid(
                "registry row lacks its exact circuit/version inverse",
            ));
        }
    }
    for (circuit, id) in by_circuit.iter() {
        let Some(record) = keys.get(id) else {
            return Err(invalid("circuit index names an unknown verifying key"));
        };
        if record.circuit_id != circuit.0 || record.version != circuit.1 {
            return Err(invalid(
                "circuit index names a different circuit or version",
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_verifying_key_index(world: &World) -> Result<(), json::Error> {
    validate_cut(
        &world.verifying_keys.view(),
        &world.verifying_keys_by_circuit.view(),
        "current",
    )?;
    validate_cut(
        &world.verifying_keys.block_and_revert(),
        &world.verifying_keys_by_circuit.block_and_revert(),
        "predecessor",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::zk::BackendTag;

    fn id(sequence: u8) -> VerifyingKeyId {
        VerifyingKeyId::new("halo2/ipa", format!("key-{sequence}"))
    }

    fn record(version: u32, circuit: &str) -> VerifyingKeyRecord {
        VerifyingKeyRecord::new(
            version,
            circuit,
            BackendTag::Halo2IpaPasta,
            "pasta",
            [1; 32],
            [2; 32],
        )
    }

    fn valid_world() -> World {
        let world = World::default();
        let mut block = world.block();
        block.verifying_keys.insert(id(1), record(2, "circuit"));
        block
            .verifying_keys_by_circuit
            .insert(("circuit".to_owned(), 2), id(1));
        block.commit();
        world
    }

    #[test]
    fn current_cut_rejects_missing_foreign_and_ambiguous_inverse_rows() {
        validate_verifying_key_index(&valid_world()).unwrap();
        for mutation in 0..5 {
            let world = valid_world();
            let mut block = world.block();
            match mutation {
                0 => {
                    block
                        .verifying_keys_by_circuit
                        .remove(("circuit".to_owned(), 2));
                }
                1 => {
                    block
                        .verifying_keys_by_circuit
                        .insert(("circuit".to_owned(), 2), id(2));
                }
                2 => {
                    block
                        .verifying_keys_by_circuit
                        .insert(("other".to_owned(), 2), id(1));
                }
                3 => {
                    block.verifying_keys.insert(id(1), record(3, "circuit"));
                }
                4 => {
                    block.verifying_keys.insert(id(2), record(2, "circuit"));
                }
                _ => unreachable!(),
            }
            block.commit();
            let error = validate_verifying_key_index(&world).unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("world.verifying_keys_by_circuit"),
                "{error}"
            );
        }
    }

    #[test]
    fn predecessor_cut_cannot_hide_an_unindexed_verifying_key() {
        let world = World::default();
        {
            let mut block = world.block();
            block.verifying_keys.insert(id(1), record(2, "circuit"));
            block.commit();
        }
        {
            let mut block = world.block();
            block
                .verifying_keys_by_circuit
                .insert(("circuit".to_owned(), 2), id(1));
            block.commit();
        }
        let error = validate_verifying_key_index(&world).unwrap_err();
        assert!(error.to_string().contains("predecessor"), "{error}");
    }
}
