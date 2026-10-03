//! Immutable segmented history columns; global adjacency and fixed cells are derived.
//!
//! All segments borrow one permutation challenge family. The first/last masks
//! apply only at global endpoints; no lookup or product state resets at a seam.
//! TODO: bind every segment commitment in one masked execution transcript.
//! This borrowed relation view grants no proof or native-producer authority.

use super::*;

#[derive(Debug, PartialEq, Eq)]
pub(in super::super) enum ShapeError {
    SegmentCount,
    ColumnLength,
    Window,
}

/// Lightweight borrowed column descriptors, never copied column backing.
#[derive(Clone, Copy)]
pub(in super::super) struct Segment<'a> {
    base: [&'a [F]; ROW_WIDTH],
    aux: [&'a [F]; permutation::WIDTH],
}
impl<'a> Segment<'a> {
    pub(in super::super) fn new(
        base: [&'a [F]; ROW_WIDTH],
        aux: [&'a [F]; permutation::WIDTH],
    ) -> Self {
        Self { base, aux }
    }
}

/// Private column candidates, not State or initializer authority. The fixed
/// descriptor array retains original backing without a second history copy.
pub(in super::super) struct View<'a> {
    schedule: Schedule,
    segments: [Option<Segment<'a>>; MAX_SEGMENTS],
    challenges: &'a permutation::Challenges,
}
struct Rows {
    current: [F; ROW_WIDTH],
    next: [F; ROW_WIDTH],
    aux: [F; permutation::WIDTH],
    next_aux: [F; permutation::WIDTH],
}
impl Drop for Rows {
    fn drop(&mut self) {
        for value in self
            .current
            .iter_mut()
            .chain(&mut self.next)
            .chain(&mut self.aux)
            .chain(&mut self.next_aux)
        {
            value.zeroize_v1();
        }
    }
}
impl<'a> View<'a> {
    pub(in super::super) fn new(
        schedule: Schedule,
        segments: impl IntoIterator<Item = Segment<'a>>,
        challenges: &'a permutation::Challenges,
    ) -> Result<Self, ShapeError> {
        let mut source = segments.into_iter();
        let mut retained = [None; MAX_SEGMENTS];
        for destination in retained.iter_mut().take(usize::from(schedule.segments)) {
            let segment = source.next().ok_or(ShapeError::SegmentCount)?;
            if segment
                .base
                .iter()
                .chain(segment.aux.iter())
                .any(|column| column.len() != schedule.segment_size())
            {
                return Err(ShapeError::ColumnLength);
            }
            *destination = Some(segment);
        }
        if source.next().is_some() {
            return Err(ShapeError::SegmentCount);
        }
        Ok(Self {
            schedule,
            segments: retained,
            challenges,
        })
    }
    pub(in super::super) fn contains_window(
        &self,
        first: u32,
        slots: usize,
    ) -> Result<(), ShapeError> {
        let first = usize::try_from(first).map_err(|_| ShapeError::Window)?;
        if first
            .checked_add(slots)
            .is_none_or(|end| end > self.schedule.size() / PHASES)
        {
            return Err(ShapeError::Window);
        }
        Ok(())
    }
    /// A complete native source must cover both global endpoints. Accepting a
    /// prefix of a larger history would leave its first/last constraints open.
    pub(in super::super) fn require_complete_slots(&self, slots: usize) -> Result<(), ShapeError> {
        if self.schedule.size() / PHASES != slots {
            return Err(ShapeError::Window);
        }
        Ok(())
    }
    pub(in super::super) fn append_row(
        &self,
        out: &mut impl crate::execution_proofs::ivm_step_air::residues::Sink,
        index: usize,
        producer: &[F; packet::WIDTH],
    ) {
        let (current, offset) = self.row_at(index);
        let (next, next_offset) = self.row_at((index + 1) % self.schedule.size());
        let rows = Rows {
            current: core::array::from_fn(|column| current.base[column][offset]),
            next: core::array::from_fn(|column| next.base[column][next_offset]),
            aux: core::array::from_fn(|column| current.aux[column][offset]),
            next_aux: core::array::from_fn(|column| next.aux[column][next_offset]),
        };
        append_residues(
            out,
            &rows.current,
            &rows.next,
            &rows.aux,
            &rows.next_aux,
            &self.schedule.fixed(index).expect("validated history row"),
            producer,
            self.challenges,
        );
    }
    fn row_at(&self, index: usize) -> (&Segment<'a>, usize) {
        assert!(index < self.schedule.size(), "validated global history row");
        (
            self.segments[index / self.schedule.segment_size()]
                .as_ref()
                .expect("validated segment count"),
            index % self.schedule.segment_size(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shape_and_window_are_checked_before_any_row_access() {
        let schedule = Schedule::new(MAX_LOG, 1).unwrap();
        let column = vec![F::ZERO; schedule.size()];
        let mut base = [column.as_slice(); ROW_WIDTH];
        let aux = [column.as_slice(); permutation::WIDTH];
        let challenges = permutation::Challenges::testing(E::ONE, E::ONE);
        base[3] = &column[..column.len() - 1];
        assert!(matches!(
            View::new(schedule, [Segment::new(base, aux)], &challenges),
            Err(ShapeError::ColumnLength)
        ));
        base[3] = &column;
        let view = View::new(schedule, [Segment::new(base, aux)], &challenges).unwrap();
        assert_eq!(view.contains_window(8087, 8297), Ok(()));
        assert_eq!(view.contains_window(8088, 8297), Err(ShapeError::Window));
        assert_eq!(
            view.contains_window(u32::MAX, 8297),
            Err(ShapeError::Window)
        );
        assert_eq!(view.contains_window(0, usize::MAX), Err(ShapeError::Window));
    }
}
