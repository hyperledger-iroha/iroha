//! Reference oracle of the keyed State contract. Test only.
//!
//! The oracle is deliberately not a candidate construction: its root is one hash of
//! the canonical dump of every entry and every witness is that complete dump, so a
//! witness is as large as the State. It exists to show that the contract is
//! satisfiable and to give the conformance suite a known-good subject. It must never
//! be compiled into a node, published or measured as a candidate.
//!
//! `FAULT` selects one deliberate contract violation. The faulty variants are the
//! negative controls of `conformance::check_contract`: each must be reported under
//! the rule it breaks.

use std::collections::BTreeMap;

use super::{
    CELL_KEY, Change, Entry, KeyRange, KeyedStateCommitment, KeyedStateRoot, ProveError, Rejection,
    StateSchema, StatementError, TableShape, UpdateError, Witness,
};

const ROOT_DOMAIN: &[u8] = b"iroha:state-keyed-commitment:reference-oracle:root:v1\0";

/// Deliberate contract violations of the faulty oracle variants.
pub(crate) mod fault {
    /// The conforming oracle.
    pub(crate) const NONE: u8 = 0;
    /// K1: the root omits the schema descriptor.
    pub(crate) const SCHEMA_UNBOUND: u8 = 1;
    /// K2: the root binds the number of applied change sets.
    pub(crate) const HISTORY_DEPENDENT: u8 = 2;
    /// K3: the root binds the committed keys but not their values.
    pub(crate) const VALUE_NOT_IN_ROOT: u8 = 12;
    /// K4: inclusion verification ignores the claimed value.
    pub(crate) const VALUE_UNBOUND: u8 = 3;
    /// K5: an entry with an empty value verifies as absent.
    pub(crate) const EMPTY_VALUE_IS_ABSENT: u8 = 4;
    /// K6: range verification accepts any subset of the committed entries.
    pub(crate) const RANGE_INCOMPLETE: u8 = 5;
    /// K6: the upper bound of a range is treated as inclusive.
    pub(crate) const RANGE_UPPER_INCLUSIVE: u8 = 6;
    /// K6: claimed entries outside the interval are dropped instead of rejected.
    pub(crate) const RANGE_OUTSIDE_CLAIM_IGNORED: u8 = 14;
    /// K7: point lookups ignore the table.
    pub(crate) const TABLE_UNBOUND: u8 = 7;
    /// K8: a cell accepts nonempty keys.
    pub(crate) const CELL_KEY_UNCHECKED: u8 = 8;
    /// K9: bytes after the canonical witness are ignored.
    pub(crate) const TRAILING_BYTES: u8 = 9;
    /// K10: a refused change set leaves its earlier changes applied.
    pub(crate) const NON_ATOMIC_APPLY: u8 = 10;
    /// K11: once the State holds more than sixteen entries, the incrementally updated
    /// root binds the number of applied change sets and drifts from a cold build.
    pub(crate) const LARGE_STATE_DRIFT: u8 = 13;
    /// K12: verification does not compare the witness with the root.
    pub(crate) const ROOT_UNBOUND: u8 = 11;
}

type Entries = BTreeMap<(u32, Vec<u8>), Vec<u8>>;

/// The oracle: every committed entry, keyed by canonical table position and key bytes.
#[derive(Clone, Debug)]
pub(crate) struct Reference<const FAULT: u8 = { fault::NONE }> {
    schema: StateSchema,
    entries: Entries,
    applied: u64,
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    <[u8; 32]>::from(iroha_crypto::Hash::new(bytes))
}

fn push_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    let length = u32::try_from(bytes.len()).expect("oracle test values fit u32");
    out.extend_from_slice(&length.to_le_bytes());
    out.extend_from_slice(bytes);
}

/// `le64(applied) ‖ le32(count) ‖ (le32(table) ‖ le32(len) ‖ key ‖ le32(len) ‖ value)*`,
/// entries ascending by `(table, key)`.
fn dump(entries: &Entries, applied: u64) -> Vec<u8> {
    let mut out = applied.to_le_bytes().to_vec();
    let count = u32::try_from(entries.len()).expect("oracle test states fit u32");
    out.extend_from_slice(&count.to_le_bytes());
    for ((table, key), value) in entries {
        out.extend_from_slice(&table.to_le_bytes());
        push_bytes(&mut out, key);
        push_bytes(&mut out, value);
    }
    out
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], Rejection> {
        if self.0.len() < length {
            return Err(Rejection::Malformed);
        }
        let (head, tail) = self.0.split_at(length);
        self.0 = tail;
        Ok(head)
    }

    fn le32(&mut self) -> Result<u32, Rejection> {
        let bytes: [u8; 4] = self.take(4)?.try_into().map_err(|_| Rejection::Malformed)?;
        Ok(u32::from_le_bytes(bytes))
    }

    fn le64(&mut self) -> Result<u64, Rejection> {
        let bytes: [u8; 8] = self.take(8)?.try_into().map_err(|_| Rejection::Malformed)?;
        Ok(u64::from_le_bytes(bytes))
    }

    fn bytes(&mut self) -> Result<&'a [u8], Rejection> {
        let length = usize::try_from(self.le32()?).map_err(|_| Rejection::Malformed)?;
        self.take(length)
    }
}

impl<const FAULT: u8> Reference<FAULT> {
    /// The update count that a faulty variant binds into its root; zero when conforming.
    fn bound_history(entries: &Entries, applied: u64) -> u64 {
        let bound = FAULT == fault::HISTORY_DEPENDENT
            || (FAULT == fault::LARGE_STATE_DRIFT && entries.len() > 16);
        if bound { applied } else { 0 }
    }

    fn root_of(schema: &StateSchema, entries: &Entries, applied: u64) -> KeyedStateRoot {
        let mut preimage = ROOT_DOMAIN.to_vec();
        if FAULT != fault::SCHEMA_UNBOUND {
            push_bytes(&mut preimage, &schema.canonical_bytes());
        }
        let applied = Self::bound_history(entries, applied);
        if FAULT == fault::VALUE_NOT_IN_ROOT {
            let keys = entries
                .keys()
                .map(|position| (position.clone(), Vec::new()))
                .collect();
            preimage.extend_from_slice(&dump(&keys, applied));
        } else {
            preimage.extend_from_slice(&dump(entries, applied));
        }
        KeyedStateRoot(digest(&preimage))
    }

    fn check_key(schema: &StateSchema, table: &str, key: &[u8]) -> Result<u32, StatementError> {
        let index = if FAULT == fault::CELL_KEY_UNCHECKED {
            schema.table(table).ok_or(StatementError::UnknownTable)?.0
        } else {
            schema.check_key(table, key)?
        };
        Ok(u32::try_from(index).expect("schema table count fits u32"))
    }

    /// Decode a canonical dump and require it to be the preimage of `root`.
    fn open(
        schema: &StateSchema,
        root: &KeyedStateRoot,
        witness: &Witness,
    ) -> Result<Entries, Rejection> {
        let mut reader = Reader(witness.as_bytes());
        let applied = reader.le64()?;
        let count = reader.le32()?;
        let mut entries = Entries::new();
        let mut previous: Option<(u32, Vec<u8>)> = None;
        for _ in 0..count {
            let table = reader.le32()?;
            let key = reader.bytes()?.to_vec();
            let value = reader.bytes()?.to_vec();
            let descriptor = schema
                .tables()
                .get(usize::try_from(table).map_err(|_| Rejection::Malformed)?)
                .ok_or(Rejection::Malformed)?;
            if descriptor.shape() == TableShape::Cell
                && key != CELL_KEY
                && FAULT != fault::CELL_KEY_UNCHECKED
            {
                return Err(Rejection::Malformed);
            }
            let position = (table, key);
            if previous
                .as_ref()
                .is_some_and(|previous| *previous >= position)
            {
                return Err(Rejection::Malformed);
            }
            previous = Some(position.clone());
            entries.insert(position, value);
        }
        if !reader.0.is_empty() && FAULT != fault::TRAILING_BYTES {
            return Err(Rejection::Malformed);
        }
        // The update count is zero unless a faulty variant binds it into this root.
        if applied != Self::bound_history(&entries, applied) {
            return Err(Rejection::Malformed);
        }
        if FAULT != fault::ROOT_UNBOUND && Self::root_of(schema, &entries, applied) != *root {
            return Err(Rejection::NotProven);
        }
        Ok(entries)
    }

    fn lookup<'a>(entries: &'a Entries, table: u32, key: &[u8]) -> Option<&'a Vec<u8>> {
        if FAULT == fault::TABLE_UNBOUND {
            return entries
                .iter()
                .find(|((_, candidate), _)| candidate.as_slice() == key)
                .map(|(_, value)| value);
        }
        entries.get(&(table, key.to_vec()))
    }

    fn in_range(range: KeyRange<'_>, key: &[u8]) -> bool {
        range.contains(key) || (FAULT == fault::RANGE_UPPER_INCLUSIVE && range.upper == Some(key))
    }

    fn witness(&self) -> Witness {
        let applied = Self::bound_history(&self.entries, self.applied);
        Witness::new(dump(&self.entries, applied))
    }
}

impl<const FAULT: u8> KeyedStateCommitment for Reference<FAULT> {
    const CONSTRUCTION: &'static str = "reference-oracle/v1 (test only)";

    fn empty(schema: &StateSchema) -> Self {
        Self {
            schema: schema.clone(),
            entries: Entries::new(),
            applied: 0,
        }
    }

    fn apply(&mut self, changes: &[Change<'_>]) -> Result<(), UpdateError> {
        let mut next = self.entries.clone();
        let mut seen = std::collections::BTreeSet::new();
        for change in changes {
            let checked = Self::check_key(&self.schema, change.table, change.key)
                .map_err(UpdateError::from)
                .and_then(|table| {
                    if seen.insert((table, change.key.to_vec())) {
                        Ok(table)
                    } else {
                        Err(UpdateError::DuplicateChange)
                    }
                });
            let table = match checked {
                Ok(table) => table,
                Err(error) => {
                    if FAULT == fault::NON_ATOMIC_APPLY {
                        self.entries = next;
                    }
                    return Err(error);
                }
            };
            match change.value {
                Some(value) => next.insert((table, change.key.to_vec()), value.to_vec()),
                None => next.remove(&(table, change.key.to_vec())),
            };
        }
        self.entries = next;
        self.applied += 1;
        Ok(())
    }

    fn root(&self) -> KeyedStateRoot {
        Self::root_of(&self.schema, &self.entries, self.applied)
    }

    fn entries(&self) -> u64 {
        u64::try_from(self.entries.len()).expect("oracle test states fit u64")
    }

    fn prove_inclusion(&self, table: &str, key: &[u8]) -> Result<Witness, ProveError> {
        let table = Self::check_key(&self.schema, table, key)?;
        if !self.entries.contains_key(&(table, key.to_vec())) {
            return Err(ProveError::KeyAbsent);
        }
        Ok(self.witness())
    }

    fn prove_absence(&self, table: &str, key: &[u8]) -> Result<Witness, ProveError> {
        let table = Self::check_key(&self.schema, table, key)?;
        if self.entries.contains_key(&(table, key.to_vec())) {
            return Err(ProveError::KeyPresent);
        }
        Ok(self.witness())
    }

    fn prove_range(&self, table: &str, range: KeyRange<'_>) -> Result<Witness, ProveError> {
        self.schema.check_range(table, range, &[])?;
        Ok(self.witness())
    }

    fn verify_inclusion(
        schema: &StateSchema,
        root: &KeyedStateRoot,
        table: &str,
        key: &[u8],
        value: &[u8],
        witness: &Witness,
    ) -> Result<(), Rejection> {
        let table = Self::check_key(schema, table, key)?;
        let entries = Self::open(schema, root, witness)?;
        match Self::lookup(&entries, table, key) {
            Some(committed) if FAULT == fault::VALUE_UNBOUND || committed.as_slice() == value => {
                Ok(())
            }
            _ => Err(Rejection::NotProven),
        }
    }

    fn verify_absence(
        schema: &StateSchema,
        root: &KeyedStateRoot,
        table: &str,
        key: &[u8],
        witness: &Witness,
    ) -> Result<(), Rejection> {
        let table = Self::check_key(schema, table, key)?;
        let entries = Self::open(schema, root, witness)?;
        match Self::lookup(&entries, table, key) {
            None => Ok(()),
            Some(value) if FAULT == fault::EMPTY_VALUE_IS_ABSENT && value.is_empty() => Ok(()),
            Some(_) => Err(Rejection::NotProven),
        }
    }

    fn verify_range(
        schema: &StateSchema,
        root: &KeyedStateRoot,
        table: &str,
        range: KeyRange<'_>,
        claimed: &[Entry<'_>],
        witness: &Witness,
    ) -> Result<(), Rejection> {
        let inside: Vec<Entry<'_>>;
        let claimed = if FAULT == fault::RANGE_OUTSIDE_CLAIM_IGNORED {
            inside = claimed
                .iter()
                .copied()
                .filter(|entry| range.contains(entry.key))
                .collect();
            inside.as_slice()
        } else {
            claimed
        };
        let table = u32::try_from(schema.check_range(table, range, claimed)?)
            .expect("schema table count fits u32");
        let entries = Self::open(schema, root, witness)?;
        let committed: Vec<(&[u8], &[u8])> = entries
            .iter()
            .filter(|((candidate, key), _)| *candidate == table && Self::in_range(range, key))
            .map(|((_, key), value)| (key.as_slice(), value.as_slice()))
            .collect();
        let proven = if FAULT == fault::RANGE_INCOMPLETE {
            claimed
                .iter()
                .all(|entry| committed.contains(&(entry.key, entry.value)))
        } else {
            committed.len() == claimed.len()
                && committed
                    .iter()
                    .zip(claimed)
                    .all(|(committed, entry)| *committed == (entry.key, entry.value))
        };
        if proven {
            Ok(())
        } else {
            Err(Rejection::NotProven)
        }
    }
}
