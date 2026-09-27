//! Public-only fixtures and binding checks for the closed relation bridge.

use super::*;
use crate::{
    PublicInputs, StateTransition,
    backend::deep_binding::{Context, Oracle},
    gadgets::public_transfer_statement::{
        PreparedPublicTransfers, PublicTransferTranscript, encode_quantity_units_v1,
    },
    proof::PublicIO,
};
use fastpq_isi::GoldilocksDigest384V1 as Digest;
use iroha_data_model::fastpq::FastpqQuantityUnits;
use iroha_primitives::numeric::Quantity;

/// Convert only public narrow fixture rows into the canonical quantity frame.
pub(in crate::backend) fn quantity_copy(
    prepared: &PreparedPublicTransfers<'_>,
) -> (
    Vec<StateTransition>,
    Vec<PublicTransferTranscript>,
    PublicInputs,
) {
    let mut rows = prepared.transitions().to_vec();
    for row in &mut rows {
        for bytes in [&mut row.pre_value, &mut row.post_value] {
            let value = u64::from_le_bytes(bytes.as_slice().try_into().unwrap());
            let value = FastpqQuantityUnits::from_quantity(&Quantity::from(value), 0).unwrap();
            *bytes = encode_quantity_units_v1(&value).unwrap();
        }
    }
    (rows, prepared.claims().to_vec(), *prepared.public_inputs())
}

/// Independently construct the complete expected seven-field PublicIO.
pub(in crate::backend) fn expected<V>(prepared: &PreparedPublicTransfers<'_, V>) -> PublicIO {
    let inputs = prepared.public_inputs();
    PublicIO {
        dsid: inputs.dsid,
        slot: inputs.slot,
        old_root: inputs.old_root,
        new_root: inputs.new_root,
        perm_root: inputs.perm_root,
        tx_set_hash: inputs.tx_set_hash,
        ordering_hash: prepared.ordering_hash().into(),
    }
}

/// Assert exact immutable bridge preservation and hash under its outer identity.
pub(in crate::backend) fn bound_root(relation: &impl DeepRelation) -> Digest {
    assert_eq!(
        relation.statement_bytes(),
        relation.deep_relation().statement_bytes()
    );
    let schema = relation.schema();
    let inner = relation.deep_relation().schema();
    assert_eq!(
        (schema.trace_rows, schema.width, schema.constraints),
        (inner.trace_rows, inner.width, inner.constraints)
    );
    Context::for_relation(relation)
        .unwrap()
        .hash_parent(Oracle::Row, 1, 0, Digest::default(), Digest::default())
        .unwrap()
}

/// Change exactly one independently expected public input for rejection checks.
pub(in crate::backend) fn changed_input(mut value: PublicIO, field: usize) -> PublicIO {
    match field {
        0 => value.dsid[15] ^= 1,
        1 => value.slot ^= 1,
        2 => value.old_root[0] ^= 1,
        3 => value.new_root[0] ^= 1,
        4 => value.perm_root[31] ^= 0x80,
        5 => value.tx_set_hash[31] ^= 0x80,
        6 => value.ordering_hash[31] ^= 0x80,
        _ => panic!("seven-field fixture"),
    }
    value
}
