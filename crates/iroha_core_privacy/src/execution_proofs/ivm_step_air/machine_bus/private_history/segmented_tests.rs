//! Original segment seams, absolute clocks and one shared challenge family.
//!
//! These use the existing algebraic witness oracle; neither this fixture nor
//! the borrowed segment view is an interpreter producer or admitted proof.

use super::*;
use crate::execution_proofs::ivm_step_air::residues::{Scratch, Stream};

struct Fixture {
    bus: PublicPacketBus,
    base: Vec<Vec<F>>,
    aux: Vec<Vec<F>>,
    challenges: permutation::Challenges,
    schedule: Schedule,
}
impl Fixture {
    fn new() -> Self {
        let slots = (1 << MIN_LOG) / PHASES;
        let events = (0..=slots)
            .map(|index| {
                Some(packet::Event {
                    space: packet::Space::Owner,
                    vm: 7,
                    generation: 3,
                    index: 9,
                    write: true,
                    before: (index as u128).to_le_bytes(),
                    after: (index as u128 + 1).to_le_bytes(),
                    before_private: 0,
                    after_private: 0,
                })
            })
            .collect();
        // A legal single log14 public oracle supplies two log13 candidate banks.
        // Its adapter/proof parameters are unchanged by the private view.
        let bus = PublicPacketBus::new(events).unwrap();
        let base = bus.columns();
        let challenges = permutation::Challenges::testing(
            E::canonical([2, 1, 0, 0]).unwrap(),
            E::canonical([7, 0, 1, 0]).unwrap(),
        );
        let aux = permutation::columns(
            &base[NOTE_COPY_WIDTH_V1 + ORDERED..NOTE_COPY_WIDTH_V1 + SORTED],
            &base[NOTE_COPY_WIDTH_V1 + SORTED..NOTE_COPY_WIDTH_V1 + PREVIOUS],
            &challenges,
            bus.size(),
        )
        .unwrap();
        Self {
            bus,
            base,
            aux,
            challenges,
            schedule: Schedule::new(MIN_LOG, 2).unwrap(),
        }
    }
    fn view_with<'a>(&'a self, challenges: &'a permutation::Challenges) -> View<'a> {
        let size = self.schedule.segment_size();
        View::new(
            self.schedule,
            (0..2).map(|segment| {
                let range = segment * size..(segment + 1) * size;
                Segment::new(
                    core::array::from_fn(|column| {
                        &self.base[NOTE_COPY_WIDTH_V1 + column][range.clone()]
                    }),
                    core::array::from_fn(|column| &self.aux[column][range.clone()]),
                )
            }),
            challenges,
        )
        .unwrap()
    }
    fn accepts_with(&self, index: usize, challenges: &permutation::Challenges) -> bool {
        let mut scratch = Scratch::new();
        let mut check = |values: &[F]| {
            if values.iter().all(|value| *value == F::ZERO) {
                Ok(())
            } else {
                Err(())
            }
        };
        let mut out = Stream::new(&mut scratch, &mut check);
        self.view_with(challenges)
            .append_row(&mut out, index, &self.bus.ordered(index / PHASES));
        out.finish().is_ok()
    }
    fn accepts(&self, index: usize) -> bool {
        self.accepts_with(index, &self.challenges)
    }
}

#[test]
fn only_global_endpoints_reset_and_close_the_original_history() {
    let fixture = Fixture::new();
    let seam = fixture.schedule.segment_size();
    for index in [0, seam - 1, seam, fixture.schedule.size() - 1] {
        assert!(fixture.accepts(index), "row {index}");
        let fixed = fixture.schedule.fixed(index).unwrap();
        assert_eq!(fixed[FIRST], F(u64::from(index == 0)));
        assert_eq!(
            fixed[LAST],
            F(u64::from(index + 1 == fixture.schedule.size()))
        );
        assert_eq!(
            fixed[TRANSITION],
            F(u64::from(index + 1 < fixture.schedule.size()))
        );
        assert_eq!(fixed[SLOT], F((index / PHASES) as u64));
    }
    assert_ne!(fixture.aux[0][seam], F::ONE);
    assert!(fixture.base[NOTE_COPY_WIDTH_V1 + ORDERED_COUNT][seam].0 > 0);
    assert!(fixture.base[NOTE_COPY_WIDTH_V1 + PREVIOUS + PREV_VALID][seam].0 > 0);
}

#[test]
fn every_lookup_and_product_continuation_comes_from_the_original_next_segment() {
    let mut fixture = Fixture::new();
    let seam = fixture.schedule.segment_size();
    for column in (PREVIOUS..SOURCES).chain([ORDERED_COUNT, SORTED_COUNT, SORTED_ENDED]) {
        let column = NOTE_COPY_WIDTH_V1 + column;
        let original = fixture.base[column][seam];
        fixture.base[column][seam] = original.add(F::ONE);
        assert!(!fixture.accepts(seam - 1), "base column {column}");
        fixture.base[column][seam] = original;
    }
    for column in 0..permutation::WIDTH {
        let original = fixture.aux[column][seam];
        fixture.aux[column][seam] = original.add(F::ONE);
        assert!(
            !fixture.accepts(seam - 1) || !fixture.accepts(seam),
            "aux column {column}"
        );
        fixture.aux[column][seam] = original;
    }
    assert!(fixture.accepts(seam - 1));
    assert!(fixture.accepts(seam));
}

#[test]
fn segment_clock_rebasing_and_a_second_challenge_family_are_rejected() {
    let mut fixture = Fixture::new();
    let seam = fixture.schedule.segment_size();
    let column = NOTE_COPY_WIDTH_V1 + ORDERED + packet::CLOCK;
    assert_eq!(fixture.base[column][seam], F((seam / PHASES) as u64));
    fixture.base[column][seam] = F::ZERO;
    assert!(!fixture.accepts(seam));
    fixture.base[column][seam] = F((seam / PHASES) as u64);
    let wrong = permutation::Challenges::testing(E::ONE, E::ONE);
    // The view owns one borrowed family; neither a per-row nor per-window
    // callable parameter can substitute a different segment challenge.
    assert!(!fixture.accepts_with(PHASES - 1, &wrong));
    assert!(!fixture.accepts_with(seam + PHASES - 1, &wrong));
}

#[test]
fn bounded_segment_inventory_and_absolute_maximum_clock_are_checked() {
    assert!(Schedule::new(MIN_LOG, 0).is_none());
    assert!(Schedule::new(MIN_LOG, 3).is_none());
    assert!(Schedule::new(MAX_LOG + 1, 2).is_none());
    let schedule = Schedule::new(MAX_LOG, 2).unwrap();
    assert_eq!(schedule.segment_size(), 131_072);
    assert_eq!(schedule.size(), 262_144);
    assert_eq!(
        schedule.fixed(schedule.size() - 1).unwrap()[SLOT],
        F(32_767)
    );
    let column = vec![F::ZERO; schedule.segment_size()];
    let segment = Segment::new(
        [column.as_slice(); ROW_WIDTH],
        [column.as_slice(); permutation::WIDTH],
    );
    let challenges = permutation::Challenges::testing(E::ONE, E::ONE);
    for supplied in [0, 1, 3] {
        assert!(matches!(
            View::new(
                schedule,
                core::iter::repeat_n(segment, supplied),
                &challenges
            ),
            Err(ShapeError::SegmentCount)
        ));
    }
    let malformed = Segment::new(
        [&column[..column.len() - 1]; ROW_WIDTH],
        [column.as_slice(); permutation::WIDTH],
    );
    assert!(matches!(
        View::new(schedule, [segment, malformed], &challenges),
        Err(ShapeError::ColumnLength)
    ));
    let view = View::new(schedule, [segment, segment], &challenges).unwrap();
    assert_eq!(view.contains_window(24_471, 8_297), Ok(()));
    assert_eq!(view.contains_window(24_472, 8_297), Err(ShapeError::Window));
    // The existing public diagnostic proof has not acquired a larger cap.
    assert!(PublicPacketBus::new(vec![None; MAX_PACKETS + 1]).is_err());
    let public = PublicPacketBus::new(Vec::new()).unwrap().protocol_v1();
    assert_eq!(public.parameters.maximum_segment_instances, 1);
    assert_eq!(public.parameters.maximum_trace_log2, MAX_LOG);
    assert_eq!(public.parameters.maximum_proof_bytes, 4 * 1024 * 1024);
    assert_eq!(public.parameters.query_count, 136);
    assert_eq!(public.maximum_constraint_degree, 4);
}
