//! Run the construction-independent State witness contract against the selected radix tree.
use super::*;
use iroha_data_model::sumeragi_finality::keyed_state::{
    KeyedStateTreeV1, StateKeyV1, StateLookupProofV1, StateRangeProofV1,
};
use std::collections::BTreeSet;

struct Selected {
    schema: StateSchema,
    tree: KeyedStateTreeV1,
}

fn key(table: &str, key: &[u8]) -> StateKeyV1 {
    StateKeyV1 {
        table: table.into(),
        key: key.into(),
    }
}
fn schema_digest(schema: &StateSchema) -> [u8; 32] {
    let id = crate::runtime::keyed_state::descriptor()
        .id()
        .expect("selected definition");
    let mut h = blake3::Hasher::new_derive_key("iroha keyed State profile and schema v1");
    h.update(&id.0).update(&schema.canonical_bytes());
    *h.finalize().as_bytes()
}
fn wire<T: norito::NoritoSerialize>(value: &T) -> Result<Witness, ProveError> {
    norito::encode_canonical(value)
        .map(Witness::new)
        .map_err(|_| ProveError::Local(LocalFailure::Resources))
}
impl Selected {
    fn point(&self, table: &str, bytes: &[u8], present: bool) -> Result<Witness, ProveError> {
        self.schema.check_key(table, bytes)?;
        let k = key(table, bytes);
        match (present, self.tree.get(&k)) {
            (true, None) => Err(ProveError::KeyAbsent),
            (false, Some(_)) => Err(ProveError::KeyPresent),
            _ => wire(
                &self
                    .tree
                    .prove(&k)
                    .map_err(|_| ProveError::Local(LocalFailure::Resources))?,
            ),
        }
    }
    fn verify_point(
        schema: &StateSchema,
        root: &KeyedStateRoot,
        table: &str,
        bytes: &[u8],
        value: Option<&[u8]>,
        witness: &Witness,
    ) -> Result<(), Rejection> {
        schema.check_key(table, bytes)?;
        let proof: StateLookupProofV1 =
            norito::decode_canonical(witness.as_bytes()).map_err(|_| Rejection::Malformed)?;
        proof
            .verify(
                &schema_digest(schema),
                root.payload(),
                &key(table, bytes),
                value,
            )
            .then_some(())
            .ok_or(Rejection::NotProven)
    }
}

impl KeyedStateCommitment for Selected {
    const CONSTRUCTION: &'static str = "iroha.keyed-state.radix-blake3.v1";
    fn empty(schema: &StateSchema) -> Self {
        Self {
            schema: schema.clone(),
            tree: KeyedStateTreeV1::new(
                schema_digest(schema),
                &iroha_allocation::AllocationBudget::new(8 << 20),
            ),
        }
    }
    fn apply(&mut self, changes: &[Change<'_>]) -> Result<(), UpdateError> {
        let mut next = self.tree.clone();
        let mut seen = BTreeSet::new();
        for c in changes {
            self.schema.check_key(c.table, c.key)?;
            if !seen.insert((c.table, c.key)) {
                return Err(UpdateError::DuplicateChange);
            }
            next.replace(&key(c.table, c.key), c.value)
                .map_err(|_| UpdateError::Local(LocalFailure::Resources))?;
        }
        self.tree = next;
        Ok(())
    }
    fn root(&self) -> KeyedStateRoot {
        self.tree.root().into()
    }
    fn entries(&self) -> u64 {
        self.tree.len()
    }
    fn prove_inclusion(&self, t: &str, k: &[u8]) -> Result<Witness, ProveError> {
        self.point(t, k, true)
    }
    fn prove_absence(&self, t: &str, k: &[u8]) -> Result<Witness, ProveError> {
        self.point(t, k, false)
    }
    fn prove_range(&self, table: &str, r: KeyRange<'_>) -> Result<Witness, ProveError> {
        self.schema.check_range(table, r, &[])?;
        wire(
            &self
                .tree
                .prove_range(table, r.lower, r.upper, 8192)
                .map_err(|_| ProveError::Local(LocalFailure::Resources))?,
        )
    }
    fn verify_inclusion(
        schema: &StateSchema,
        root: &KeyedStateRoot,
        table: &str,
        key: &[u8],
        value: &[u8],
        witness: &Witness,
    ) -> Result<(), Rejection> {
        Self::verify_point(schema, root, table, key, Some(value), witness)
    }
    fn verify_absence(
        schema: &StateSchema,
        root: &KeyedStateRoot,
        table: &str,
        key: &[u8],
        witness: &Witness,
    ) -> Result<(), Rejection> {
        Self::verify_point(schema, root, table, key, None, witness)
    }
    fn verify_range(
        s: &StateSchema,
        root: &KeyedStateRoot,
        table: &str,
        r: KeyRange<'_>,
        entries: &[Entry<'_>],
        w: &Witness,
    ) -> Result<(), Rejection> {
        s.check_range(table, r, entries)?;
        let proof: StateRangeProofV1 =
            norito::decode_canonical(w.as_bytes()).map_err(|_| Rejection::Malformed)?;
        let rows: Vec<_> = entries.iter().map(|e| (e.key, e.value)).collect();
        proof
            .verify(
                &schema_digest(s),
                root.payload(),
                table,
                r.lower,
                r.upper,
                &rows,
            )
            .then_some(())
            .ok_or(Rejection::NotProven)
    }
}

#[test]
fn selected_radix_passes_all_state_contract_rules() {
    conformance::check_contract::<Selected>()
        .expect("selected production commitment must satisfy K1–K12");
}
