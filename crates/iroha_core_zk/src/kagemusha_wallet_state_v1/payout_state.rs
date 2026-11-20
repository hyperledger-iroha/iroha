//! Certified commitment-profile dispatch and the exact rows needed for fee payout.
use super::*;
use iroha_data_model::{
    runtime::definition::{DefinitionDescriptorV1, DefinitionId, RootSelectionV1},
    sumeragi_finality::{
        KeyedStateRowsV1, SelectedStateSchemaV1, VerifiedSumeragiBlock,
        VerifiedWorldStateSnapshotV1, WorldStateSchemaPolicyV1, keyed_schema::TableShape,
        keyed_state::StateKeyV1, native_world_schema_family_v1,
    },
};

pub(super) const LEDGER_TABLE: &str = "world.kagemusha_wallet_ledger";
pub(super) const SELECTION_CELL: &str = "world.runtime_selection";
/// Maximum canonical `(RootSelectionV1, Vec<DefinitionDescriptorV1>)` payout schema bundle.
pub const PAYOUT_SELECTION_MAX_BYTES_V1: usize = 16 * 1024 * 1024;

/// State evidence interpreted only under its certified commitment profile.
/// The supplied selection remains untrusted until its cell and the payout are authenticated
/// at the same certified cut. There is no decoder fallback between these variants.
pub enum FinalizedPayoutStateV1<'a> {
    /// Historical pre-cutover World evidence under a supported LtHash schema.
    World {
        /// Snapshot already authenticated against the payout's certified block.
        world: &'a VerifiedWorldStateSnapshotV1,
    },
    /// Keyed evidence under the schema resolved from the selected storage definitions.
    Keyed {
        /// Exactly the payout row and runtime-selection cell, in canonical order.
        rows: &'a KeyedStateRowsV1,
        /// Content-addressed schema of the supplied selection.
        schema: &'a SelectedStateSchemaV1,
        /// Exact runtime selection committed in the second row.
        selection: &'a RootSelectionV1,
    },
}

type SelectionOriginal = (
    RootSelectionV1,
    BTreeMap<DefinitionId, DefinitionDescriptorV1>,
);

pub(super) fn selection_original(bytes: &[u8]) -> Result<SelectionOriginal, Error> {
    if bytes.is_empty() || bytes.len() > PAYOUT_SELECTION_MAX_BYTES_V1 {
        return Err(Error::Invalid("payout selection original bound"));
    }
    let (selection, descriptors): (RootSelectionV1, Vec<DefinitionDescriptorV1>) =
        archive::decode(bytes).map_err(|error| match error {
            Error::WitnessLost(_) => Error::Invalid("payout selection original"),
            original => original,
        })?;
    let mut definitions = BTreeMap::new();
    for descriptor in descriptors {
        let id = descriptor
            .id()
            .map_err(|_| Error::Invalid("payout storage definition"))?;
        if definitions.insert(id, descriptor).is_some() {
            return Err(Error::Invalid("duplicate payout storage definition"));
        }
    }
    if definitions.keys().copied().collect::<Vec<_>>() != selection.installed_storage {
        return Err(Error::Invalid("payout selected storage definitions"));
    }
    Ok((selection, definitions))
}

/// Exact ascending row selection required by a fee payout, including profile authentication.
pub fn fee_payout_state_keys_v1(scheme: [u8; 32], credit: [u8; 32]) -> [StateKeyV1; 2] {
    [
        StateKeyV1 {
            table: LEDGER_TABLE.into(),
            key: norito::codec::encode_adaptive(
                &KagemushaWalletPayoutKeyV1::Fee(credit).ledger_key(scheme),
            ),
        },
        StateKeyV1 {
            table: SELECTION_CELL.into(),
            key: Vec::new(),
        },
    ]
}

fn codec<T: norito::NoritoSchema>() -> String {
    format!(
        "norito:{}@{}.{}.{}",
        T::nominal_name(),
        norito::core::VERSION_MAJOR,
        norito::core::VERSION_MINOR,
        norito::core::default_encode_flags()
    )
}

impl FinalizedPayoutStateV1<'_> {
    pub(super) fn verify(
        &self,
        block: &VerifiedSumeragiBlock,
        scheme: [u8; 32],
        credit: [u8; 32],
        payout: &KagemushaWalletPayoutRecordV1,
    ) -> Result<(), Error> {
        let bytes = norito::to_bytes(payout).map_err(|_| Error::Invalid("payout encoding"))?;
        match self {
            Self::World { world } => {
                if world.height() != block.height()
                    || world.context_id() != block.context_id()
                    || block.execution().world_state_root != world.world_root()
                {
                    return Err(Error::Invalid("finalized fee payout binding"));
                }
                native_world_schema_family_v1()
                    .as_slice()
                    .select_schema(world.schema_hash())
                    .map_err(|_| Error::Invalid("finalized fee payout World schema"))?;
                world
                    .verify_table_value(LEDGER_TABLE, &payout.key.ledger_key(scheme), &bytes)
                    .map_err(|_| Error::Invalid("finalized fee payout row"))
            }
            Self::Keyed {
                rows,
                schema,
                selection,
            } => {
                if selection.commitment_profile
                    != iroha_data_model::runtime::keyed_state::profile_id()
                {
                    return Err(Error::Invalid("payout keyed commitment profile"));
                }
                // Authenticate codec identities before interpreting native projection bytes.
                let (_, ledger) = schema
                    .table(LEDGER_TABLE)
                    .ok_or(Error::Invalid("payout State schema"))?;
                let (_, runtime) = schema
                    .table(SELECTION_CELL)
                    .ok_or(Error::Invalid("payout State schema"))?;
                if ledger.shape() != TableShape::Table
                    || ledger.key_schema() != codec::<KagemushaWalletLedgerKeyV1>()
                    || ledger.value_schema() != codec::<Vec<u8>>()
                    || runtime.shape() != TableShape::Cell
                    || runtime.value_schema() != codec::<RootSelectionV1>()
                {
                    return Err(Error::Invalid("payout State schema"));
                }
                rows.verify(block, schema, &fee_payout_state_keys_v1(scheme, credit))
                    .map_err(|_| Error::Proof("payout keyed State rows"))?;
                if rows.rows[1].value.as_deref()
                    != Some(norito::codec::encode_adaptive(*selection).as_slice())
                {
                    return Err(Error::Invalid("payout certified selection"));
                }
                // The native table stores a canonical framed payout as Vec<u8>; the keyed
                // projection is the bare encoding of that Vec, not a bare payout record.
                if rows.rows[0].value.as_deref()
                    != Some(norito::codec::encode_adaptive(&bytes).as_slice())
                {
                    return Err(Error::Invalid("finalized fee payout row"));
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
pub(super) mod tests;
