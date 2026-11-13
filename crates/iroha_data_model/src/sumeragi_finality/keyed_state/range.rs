//! Complete half-open intervals of one table in canonical bare key order.

use super::*;

const MAX_RANGE_ROWS: usize = 8_192;

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::StateRadixBoundaryV1")]
pub(super) struct Boundary {
    key: StateKeyV1,
    proof: StateLookupProofV1,
}

/// An ordered interval witness. It opens the immediate outside neighbors by key
/// and value digest. Exact-key reads use a point proof, which hides those keys.
/// Verification checks consecutive authenticated ranks including both boundaries.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::StateRangeProofV1")]
pub struct StateRangeProofV1 {
    #[norito(required)]
    before: Option<Boundary>,
    pub(super) rows: Vec<StateLookupProofV1>,
    #[norito(required)]
    pub(super) after: Option<Boundary>,
}

impl KeyedStateTreeV1 {
    fn at_rank(&self, mut rank: u64) -> Option<&Leaf> {
        if rank >= self.len() {
            return None;
        }
        let mut node = self.top.as_ref()?;
        loop {
            match &node.kind {
                Kind::Leaf(leaf) => return Some(leaf),
                Kind::Branch { children, .. } => {
                    let left = children[0].summary.count;
                    node = if rank < left {
                        &children[0]
                    } else {
                        rank -= left;
                        &children[1]
                    };
                }
                Kind::Released => unreachable!("released node retained"),
            }
        }
    }

    fn partition(&self, mut less: impl FnMut(&Leaf) -> bool) -> u64 {
        let mut lo = 0;
        let mut hi = self.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if less(self.at_rank(mid).expect("rank is within map")) {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        lo
    }

    fn boundary(&self, rank: u64) -> Result<Boundary, KeyedStateError> {
        let key = self.at_rank(rank).ok_or(KeyedStateError::Count)?.raw_key();
        Ok(Boundary {
            proof: self.prove(&key)?,
            key,
        })
    }

    /// Prove a half-open interval in exactly one table. A missing upper endpoint
    /// extends through that table's last key. Proving work is bounded by the
    /// selected row count and the paths to these rows and neighbors.
    ///
    /// # Errors
    /// Invalid interval or too many rows for the requested local limit.
    pub fn prove_range(
        &self,
        table: &str,
        lower: &[u8],
        upper: Option<&[u8]>,
        max_rows: usize,
    ) -> Result<StateRangeProofV1, KeyedStateError> {
        if upper.is_some_and(|end| lower >= end) {
            return Err(KeyedStateError::Range);
        }
        let lo = StateKeyV1 {
            table: table.into(),
            key: lower.to_vec(),
        };
        lo.validate()?;
        let start =
            self.partition(|leaf| leaf.encoded_key().iter().copied().cmp(lo.bytes()).is_lt());
        let end = if let Some(end) = upper {
            let hi = StateKeyV1 {
                table: table.into(),
                key: end.to_vec(),
            };
            hi.validate()?;
            self.partition(|leaf| leaf.encoded_key().iter().copied().cmp(hi.bytes()).is_lt())
        } else {
            self.partition(|leaf| leaf.table() <= table.as_bytes())
        };
        let count = usize::try_from(end - start).map_err(|_| KeyedStateError::Capacity)?;
        if count > max_rows.min(MAX_RANGE_ROWS) {
            return Err(KeyedStateError::Capacity);
        }
        let mut rows = Vec::new();
        rows.try_reserve_exact(count)
            .map_err(|_| KeyedStateError::Capacity)?;
        for rank in start..end {
            let key = self.at_rank(rank).expect("rank is within map").raw_key();
            rows.push(self.prove(&key)?);
        }
        Ok(StateRangeProofV1 {
            before: start
                .checked_sub(1)
                .map(|rank| self.boundary(rank))
                .transpose()?,
            rows,
            after: (end < self.len()).then(|| self.boundary(end)).transpose()?,
        })
    }
}

impl Leaf {
    fn table(&self) -> &[u8] {
        self.encoded_key()
            .split(|&byte| byte == 0)
            .next()
            .expect("table prefix")
    }

    fn raw_key(&self) -> StateKeyV1 {
        let table = self.table();
        StateKeyV1 {
            table: std::str::from_utf8(table)
                .expect("validated ASCII identity")
                .into(),
            key: self.encoded_key()[table.len() + 1..].to_vec(),
        }
    }
}

impl Boundary {
    fn rank(&self, schema: &Digest, root: &Digest) -> Option<(u64, u64)> {
        (self.proof.terminal?.key == self.key.digest()).then_some(())?;
        self.proof.rank(schema, root, &self.key)
    }
}

impl StateRangeProofV1 {
    /// Authenticate exactly the supplied ascending bare key/value pairs in
    /// `[lower, upper)` under one table; an omitted upper bound means the end
    /// of that table. Truncated, reordered, extra or foreign rows are rejected.
    #[must_use]
    pub fn verify(
        &self,
        schema: &Digest,
        root: &Digest,
        table: &str,
        lower: &[u8],
        upper: Option<&[u8]>,
        rows: &[(&[u8], &[u8])],
    ) -> bool {
        if upper.is_some_and(|end| lower >= end)
            || self.rows.len() != rows.len()
            || rows.len() > MAX_RANGE_ROWS
        {
            return false;
        }
        let mut key = StateKeyV1 {
            table: table.into(),
            key: lower.to_vec(),
        };
        if key.validate().is_err() {
            return false;
        }
        let (mut next, mut total) = if let Some(before) = &self.before {
            if before.key >= key {
                return false;
            }
            let Some((rank, count)) = before.rank(schema, root) else {
                return false;
            };
            (rank + 1, count)
        } else {
            (0, 0)
        };

        for (i, ((raw, value), proof)) in rows.iter().zip(&self.rows).enumerate() {
            if *raw < lower
                || upper.is_some_and(|end| *raw >= end)
                || (i != 0 && rows[i - 1].0 >= *raw)
            {
                return false;
            }
            key.key.clear();
            key.key.extend_from_slice(raw);
            let digest = key.digest();
            let Some((rank, count)) = proof.rank(schema, root, &key) else {
                return false;
            };
            if rank != next
                || proof.terminal
                    != Some(Opening {
                        key: digest,
                        value: value_digest(&digest, value),
                    })
            {
                return false;
            }
            next = rank + 1;
            total = count;
        }
        if let Some(after) = &self.after {
            if after.key.table.as_str() < table
                || (after.key.table == table
                    && upper.is_none_or(|end| after.key.key.as_slice() < end))
            {
                return false;
            }
            let Some((rank, _)) = after.rank(schema, root) else {
                return false;
            };
            return rank == next;
        }
        if total == 0 {
            return EMPTY.root(schema) == *root;
        }
        next == total
    }
}
