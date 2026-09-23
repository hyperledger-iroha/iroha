//! Checked flat grouping in the ordinary multiopening's first-encounter order.

use super::{Fields, StoredLookupErrorV1, StoredOpeningSourceV1, add, mul, reserve};
use crate::{
    plonk::prover::stored::proof_evaluations::StoredOpeningQueryV1,
    poly::stored_advice::assignment::StoredAssignmentFieldV1,
};

#[derive(Clone, Copy, Default)]
pub(super) struct Query {
    pub(super) source: usize,
    pub(super) point: usize,
}
#[derive(Clone, Copy)]
pub(super) struct Source {
    pub(super) identity: StoredOpeningSourceV1,
    pub(super) representative: usize,
    pub(super) start: usize,
    pub(super) len: usize,
    pub(super) set: usize,
    filled: usize,
}
impl Default for Source {
    fn default() -> Self {
        Self {
            identity: StoredOpeningSourceV1::Random,
            representative: 0,
            start: 0,
            len: 0,
            set: 0,
            filled: 0,
        }
    }
}

/// No nested allocation: each source's sorted membership and evaluation slots share a range.
pub(super) struct Planner<F: StoredAssignmentFieldV1> {
    pub(super) queries: Vec<Query>,
    pub(super) sources: Vec<Source>,
    pub(super) points: Fields<F>,
    pub(super) memberships: Vec<usize>,
    pub(super) sets: Vec<usize>,
    pub(super) evaluations: Fields<F>,
    pub(super) source_count: usize,
    pub(super) point_count: usize,
    pub(super) set_count: usize,
}
fn filled<T: Default>(count: usize) -> Result<Vec<T>, StoredLookupErrorV1> {
    let mut values = reserve(count)?;
    values.resize_with(count, T::default);
    Ok(values)
}
impl<F: StoredAssignmentFieldV1> Planner<F> {
    pub(super) fn new(query_count: usize) -> Result<Self, StoredLookupErrorV1> {
        if query_count == 0 {
            return Err(StoredLookupErrorV1::Context);
        }
        Ok(Self {
            queries: filled(query_count)?,
            sources: filled(query_count)?,
            points: Fields::new(query_count)?,
            memberships: filled(query_count)?,
            sets: filled(query_count)?,
            evaluations: Fields::new(query_count)?,
            source_count: 0,
            point_count: 0,
            set_count: 0,
        })
    }
    pub(super) fn minimum_payload(query_count: usize) -> Result<usize, StoredLookupErrorV1> {
        payload::<F>(
            query_count,
            query_count,
            query_count,
            query_count,
            query_count,
            query_count,
        )
    }
    pub(super) fn actual_payload(&self) -> Result<usize, StoredLookupErrorV1> {
        payload::<F>(
            self.queries.capacity(),
            self.sources.capacity(),
            self.points.0.capacity(),
            self.memberships.capacity(),
            self.sets.capacity(),
            self.evaluations.0.capacity(),
        )
    }
    /// Called only after x1/x2. Duplicate identity/point pairs fail before any get_eval read.
    pub(super) fn populate(
        &mut self,
        plan: &[StoredOpeningQueryV1<F>],
    ) -> Result<(), StoredLookupErrorV1> {
        if plan.len() != self.queries.len()
            || self.source_count != 0
            || self.point_count != 0
            || self.set_count != 0
        {
            return Err(StoredLookupErrorV1::Context);
        }
        for (index, query) in plan.iter().enumerate() {
            let source = if let Some(existing) = self.sources[..self.source_count]
                .iter()
                .position(|s| s.identity == query.source())
            {
                existing
            } else {
                let next = self.source_count;
                self.sources[next].identity = query.source();
                self.sources[next].representative = index;
                self.source_count = add(next, 1)?;
                next
            };
            let point = if let Some(existing) = self.points.0[..self.point_count]
                .iter()
                .position(|point| *point == query.point())
            {
                existing
            } else {
                let next = self.point_count;
                self.points.0[next] = query.point();
                self.point_count = add(next, 1)?;
                next
            };
            if self.queries[..index]
                .iter()
                .any(|q| q.source == source && q.point == point)
            {
                return Err(StoredLookupErrorV1::Context);
            }
            self.queries[index] = Query { source, point };
            self.sources[source].len = add(self.sources[source].len, 1)?;
        }
        let mut next = 0;
        for source in &mut self.sources[..self.source_count] {
            source.start = next;
            next = add(next, source.len)?;
        }
        if next != plan.len() {
            return Err(StoredLookupErrorV1::Context);
        }
        for query in &self.queries {
            let source = &mut self.sources[query.source];
            let slot = add(source.start, source.filled)?;
            *self
                .memberships
                .get_mut(slot)
                .ok_or(StoredLookupErrorV1::Context)? = query.point;
            source.filled = add(source.filled, 1)?;
        }
        for source in &self.sources[..self.source_count] {
            if source.filled != source.len || source.len == 0 {
                return Err(StoredLookupErrorV1::Context);
            }
            self.memberships[source.start..add(source.start, source.len)?].sort_unstable();
        }
        for source in 0..self.source_count {
            let set = self.sets[..self.set_count]
                .iter()
                .position(|representative| {
                    self.point_ids(source) == self.point_ids(*representative)
                });
            self.sources[source].set = if let Some(set) = set {
                set
            } else {
                let set = self.set_count;
                self.sets[set] = source;
                self.set_count = add(set, 1)?;
                set
            };
        }
        Ok(())
    }
    pub(super) fn point_ids(&self, source: usize) -> &[usize] {
        let source = &self.sources[source];
        // Both bounds were established by the checked prefix sum in populate, never by a caller.
        &self.memberships[source.start..source.start + source.len]
    }
    pub(super) fn evaluation_slot(&self, query: usize) -> Result<usize, StoredLookupErrorV1> {
        let query = self
            .queries
            .get(query)
            .ok_or(StoredLookupErrorV1::Context)?;
        let source = &self.sources[query.source];
        let local = self
            .point_ids(query.source)
            .binary_search(&query.point)
            .map_err(|_| StoredLookupErrorV1::Context)?;
        add(source.start, local)
    }
}
fn payload<F: StoredAssignmentFieldV1>(
    queries: usize,
    sources: usize,
    points: usize,
    memberships: usize,
    sets: usize,
    evaluations: usize,
) -> Result<usize, StoredLookupErrorV1> {
    let rows = add(
        mul(queries, std::mem::size_of::<Query>())?,
        mul(sources, std::mem::size_of::<Source>())?,
    )?;
    let integers = mul(add(memberships, sets)?, std::mem::size_of::<usize>())?;
    let fields = mul(add(points, evaluations)?, std::mem::size_of::<F>())?;
    add(
        add(add(rows, integers)?, fields)?,
        std::mem::size_of::<Planner<F>>(),
    )
}
