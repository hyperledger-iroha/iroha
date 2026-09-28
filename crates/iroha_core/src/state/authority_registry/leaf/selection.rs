//! Inline table selection over the compile-time authoritative field inventory.
//!
//! The bitset retains no heap backing or caller strings. Its enclosing owner
//! includes these bytes in its ordinary layout; borrowed verification needs no
//! resident allocation pool. Nominal schema Strings remain a separate obligation.

use super::*;

const fn table_count(fields: &[Field]) -> usize {
    let mut count = 0;
    let mut index = 0;
    while index < fields.len() {
        count += match fields[index].role {
            Role::Canonical(Canonical::Table { .. }) => 1,
            Role::Canonical(Canonical::Owner(children)) => table_count(children),
            _ => 0,
        };
        index += 1;
    }
    count
}

const TABLE_COUNT: usize = table_count(STATE_FIELDS);
const SELECTION_WORDS: usize = TABLE_COUNT.div_ceil(u64::BITS as usize);

const fn collect_tables(
    fields: &'static [Field],
    output: &mut [&'static Field; TABLE_COUNT],
    count: &mut usize,
) {
    let mut index = 0;
    while index < fields.len() {
        match fields[index].role {
            Role::Canonical(Canonical::Table { .. }) => {
                output[*count] = &fields[index];
                *count += 1;
            }
            Role::Canonical(Canonical::Owner(children)) => {
                collect_tables(children, output, count);
            }
            _ => {}
        }
        index += 1;
    }
}

const fn id_less(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    let mut index = 0;
    while index < left.len() && index < right.len() {
        if left[index] != right[index] {
            return left[index] < right[index];
        }
        index += 1;
    }
    left.len() < right.len()
}

const fn table_catalog() -> [&'static Field; TABLE_COUNT] {
    // Every placeholder is replaced during the exhaustive recursive traversal.
    let mut tables = [&STATE_FIELDS[0]; TABLE_COUNT];
    let mut count = 0;
    collect_tables(STATE_FIELDS, &mut tables, &mut count);
    assert!(count == TABLE_COUNT);
    let mut index = 1;
    while index < tables.len() {
        let mut position = index;
        while position > 0 && id_less(tables[position].id, tables[position - 1].id) {
            tables.swap(position - 1, position);
            position -= 1;
        }
        index += 1;
    }
    // Duplicate declarations must fail at compile time, never share one bit.
    index = 1;
    while index < tables.len() {
        assert!(id_less(tables[index - 1].id, tables[index].id));
        index += 1;
    }
    tables
}

static TABLES: [&Field; TABLE_COUNT] = table_catalog();

pub(super) struct TableSelection {
    selected: [u64; SELECTION_WORDS],
    pub(super) schema: Hash,
    pub(super) limits: LeafLimits,
}

impl TableSelection {
    /// Select canonical tables with native keys and declared V1 value schemas.
    pub(super) fn new(ids: &[&str], limits: LeafLimits) -> Result<Self, LeafError> {
        if ids.len() > limits.max_tables {
            return Err(LeafError::TableLimit);
        }
        let mut selection = Self {
            selected: [0; SELECTION_WORDS],
            schema: Hash::new(SCHEMA_START),
            limits,
        };
        for id in ids {
            let field = declared_table(id)?;
            let index = TABLES
                .binary_search_by(|entry| entry.id.cmp(field.id))
                .expect("declared canonical table belongs to the static catalog");
            let bit = 1 << (index % u64::BITS as usize);
            let word = &mut selection.selected[index / u64::BITS as usize];
            if *word & bit != 0 {
                return Err(LeafError::DuplicateTable(field.id));
            }
            *word |= bit;
        }
        selection.schema = selection.fields().fold(selection.schema, |root, field| {
            let Role::Canonical(Canonical::Table { key, value }) = field.role else {
                unreachable!("static catalog contains canonical tables only")
            };
            fold_schema(root, field, key, value)
        });
        Ok(selection)
    }

    fn contains_index(&self, index: usize) -> bool {
        self.selected[index / u64::BITS as usize] & (1 << (index % u64::BITS as usize)) != 0
    }

    fn fields(&self) -> impl Iterator<Item = &'static Field> + '_ {
        self.selected
            .iter()
            .enumerate()
            .flat_map(|(word_index, &word)| {
                let mut remaining = word;
                std::iter::from_fn(move || {
                    if remaining == 0 {
                        return None;
                    }
                    let bit = remaining.trailing_zeros() as usize;
                    remaining &= remaining - 1;
                    Some(TABLES[word_index * u64::BITS as usize + bit])
                })
            })
    }

    /// Number of selected table identities, including empty tables.
    pub(super) fn len(&self) -> usize {
        self.selected
            .iter()
            .map(|word| word.count_ones() as usize)
            .sum()
    }

    /// Lexically first selected identity, used by single-table paired owners.
    pub(super) fn first_table_id(&self) -> Option<&'static str> {
        self.fields().next().map(|field| field.id)
    }

    /// Borrow the canonical identity and copy only the static schema descriptors.
    pub(super) fn table(&self, id: &str) -> Option<(&'static str, (Schema, Schema))> {
        let index = TABLES.binary_search_by(|field| field.id.cmp(id)).ok()?;
        if !self.contains_index(index) {
            return None;
        }
        let field = TABLES[index];
        let Role::Canonical(Canonical::Table { key, value }) = field.role else {
            unreachable!("static catalog contains canonical tables only")
        };
        Some((field.id, (key, value)))
    }

    pub(super) fn key_path<K: Encode + NoritoSchema>(
        &self,
        table: &str,
        key: &K,
    ) -> Result<Hash, LeafError> {
        let Some((table, (key_schema, _))) = self.table(table) else {
            return Err(LeafError::TableNotSelected);
        };
        let key_hash = typed_payload_hash(
            table,
            key_schema,
            key,
            KEY_PAYLOAD,
            self.limits.max_payload_bytes,
        )?;
        let table_len = u64::try_from(table.len()).expect("static table identity fits u64");
        Ok(Hash::new_from_chunks(&[
            PATH,
            &table_len.to_le_bytes(),
            table.as_bytes(),
            key_hash.as_ref(),
        ]))
    }
}

#[cfg(test)]
mod tests;
