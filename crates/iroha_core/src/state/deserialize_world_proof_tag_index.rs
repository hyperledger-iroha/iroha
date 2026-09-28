//! Exact proof-tag reverse-index validation for current and predecessor WSV cuts.

use super::*;
use iroha_data_model::proof::ProofId;

fn validate_cut(
    tags_by_proof: &impl StorageReadOnly<ProofId, Vec<[u8; 4]>>,
    proofs_by_tag: &impl StorageReadOnly<[u8; 4], Vec<ProofId>>,
    cut: &str,
) -> Result<(), json::Error> {
    let invalid = |message: &str| json::Error::InvalidField {
        field: "world.proofs_by_tag".to_owned(),
        message: format!("{cut} proof-tag index: {message}"),
    };
    for (proof, tags) in tags_by_proof.iter() {
        if tags.is_empty() || tags.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(invalid(
                "proof tag lists must be nonempty, sorted and unique",
            ));
        }
        for tag in tags {
            let Some(proofs) = proofs_by_tag.get(tag) else {
                return Err(invalid("proof tag is absent from its reverse bucket"));
            };
            if proofs.binary_search(proof).is_err() {
                return Err(invalid("proof tag is missing its reverse proof id"));
            }
        }
    }
    for (tag, proofs) in proofs_by_tag.iter() {
        if proofs.is_empty() || proofs.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(invalid(
                "reverse proof buckets must be nonempty, sorted and unique",
            ));
        }
        for proof in proofs {
            let Some(tags) = tags_by_proof.get(proof) else {
                return Err(invalid("reverse proof bucket contains an unknown proof id"));
            };
            if tags.binary_search(tag).is_err() {
                return Err(invalid("reverse proof bucket contains an undeclared tag"));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_proof_tag_index(world: &World) -> Result<(), json::Error> {
    validate_cut(
        &world.proof_tags.view(),
        &world.proofs_by_tag.view(),
        "current",
    )?;
    // Dropping the uncommitted rollback views preserves the retained undo logs.
    validate_cut(
        &world.proof_tags.block_and_revert(),
        &world.proofs_by_tag.block_and_revert(),
        "predecessor",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proof(sequence: u8) -> ProofId {
        ProofId {
            backend: "halo2/test".into(),
            proof_hash: [sequence; 32],
        }
    }

    fn valid_world() -> World {
        let world = World::default();
        let mut block = world.block();
        block.proof_tags.insert(proof(1), vec![*b"TAG1", *b"TAG2"]);
        block.proofs_by_tag.insert(*b"TAG1", vec![proof(1)]);
        block.proofs_by_tag.insert(*b"TAG2", vec![proof(1)]);
        block.commit();
        world
    }

    #[test]
    fn current_cut_rejects_omissions_orphans_and_noncanonical_order() {
        validate_proof_tag_index(&valid_world()).unwrap();
        for mutation in 0..5 {
            let world = valid_world();
            let mut block = world.block();
            match mutation {
                0 => {
                    block.proofs_by_tag.remove(*b"TAG1");
                }
                1 => {
                    block.proofs_by_tag.insert(*b"TAG1", vec![proof(2)]);
                }
                2 => {
                    block
                        .proofs_by_tag
                        .insert(*b"TAG1", vec![proof(1), proof(1)]);
                }
                3 => {
                    block.proof_tags.insert(proof(1), vec![*b"TAG2", *b"TAG1"]);
                }
                4 => {
                    block.proofs_by_tag.insert(*b"TAG3", vec![proof(1)]);
                }
                _ => unreachable!(),
            }
            block.commit();
            let error = validate_proof_tag_index(&world).unwrap_err();
            assert!(error.to_string().contains("world.proofs_by_tag"));
        }
    }

    #[test]
    fn predecessor_cut_cannot_hide_an_unrepaired_reverse_index() {
        let world = World::default();
        {
            let mut block = world.block();
            block.proof_tags.insert(proof(1), vec![*b"TAG1"]);
            block.commit();
        }
        {
            let mut block = world.block();
            block.proofs_by_tag.insert(*b"TAG1", vec![proof(1)]);
            block.commit();
        }
        let error = validate_proof_tag_index(&world).unwrap_err();
        assert!(error.to_string().contains("predecessor"), "{error}");
    }
}
