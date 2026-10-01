//! Private owner tuple mutation and independent stack-history controls.

use super::*;
use packet::{AFTER, BEFORE, Event, Space};

fn value(word: u16) -> [u8; 16] {
    let mut bytes = [0; 16];
    bytes[..2].copy_from_slice(&word.to_le_bytes());
    bytes
}

fn owner(
    generation: u16,
    index: u32,
    before: u16,
    after: u16,
    write: bool,
    clock: u32,
) -> [F; packet::WIDTH] {
    Event {
        space: Space::Owner,
        vm: 7,
        generation,
        index,
        write,
        before: value(before),
        after: value(after),
        before_private: 0,
        after_private: 0,
    }
    .fields(clock as usize)
}

#[derive(Clone)]
struct Fixture {
    schedule: Schedule,
    row: [F; WIDTH],
    selection: [F; 3],
    ports: [[F; packet::WIDTH]; 3],
}

impl Fixture {
    fn entry(active: u16, last: u16, root: bool) -> Self {
        let transition = if root {
            Transition::RootEntry
        } else {
            Transition::ChildEntry
        };
        Self {
            schedule: Schedule::new(7, transition, [10, 11, 12]).unwrap(),
            row: witness(active, transition),
            selection: if root {
                [F::ONE, F::ZERO, F::ZERO]
            } else {
                [F::ZERO, F::ONE, F::ZERO]
            },
            ports: [
                owner(0, GENERATION_COUNTER, last, last + 1, true, 10),
                owner(0, ACTIVE, active, last + 1, true, 11),
                owner(last + 1, PARENT, 0, active, true, 12),
            ],
        }
    }
    fn returning(active: u16, parent: u16, last: u16) -> Self {
        Self {
            schedule: Schedule::new(7, Transition::Return, [20, 21, 22]).unwrap(),
            row: witness(active, Transition::Return),
            selection: [F::ZERO, F::ZERO, F::ONE],
            ports: [
                owner(0, GENERATION_COUNTER, last, last, false, 20),
                owner(0, ACTIVE, active, parent, true, 21),
                owner(active, PARENT, parent, parent, false, 22),
            ],
        }
    }
    fn residues(&self) -> Vec<F> {
        let mut output = Vec::new();
        append_residues(
            &mut output,
            self.schedule,
            &self.row,
            &self.selection,
            Ports {
                counter: &self.ports[0],
                active: &self.ports[1],
                parent: &self.ports[2],
            },
        );
        assert_eq!(output.len(), CONSTRAINTS);
        output
    }
    fn accepts(&self) -> bool {
        self.residues().into_iter().all(|value| value == F::ZERO)
    }
}

#[test]
fn lifecycle_retains_private_generations_across_nested_returns_and_new_roots() {
    // Independent stack model chooses immediate parents and never decrements the
    // allocator. No frame generation or parent is a fixed schedule input.
    let mut stack = Vec::new();
    let mut last = 0u16;
    for depth in [1, 32, 257] {
        for _ in 0..depth {
            let active = stack.last().copied().unwrap_or(0);
            assert!(Fixture::entry(active, last, stack.is_empty()).accepts());
            last += 1;
            stack.push(last);
        }
        while let Some(active) = stack.pop() {
            let parent = stack.last().copied().unwrap_or(0);
            let f = Fixture::returning(active, parent, last);
            assert!(f.accepts());
            assert_eq!(f.ports[0][AFTER], F(u64::from(last)));
        }
    }
    assert_eq!(last, 290);
}

#[test]
fn every_port_field_mutation_and_wrong_phase_is_rejected() {
    for fixture in [
        Fixture::entry(0, 0, true),
        Fixture::entry(3, 9, false),
        Fixture::returning(10, 3, 10),
    ] {
        for port in 0..3 {
            for field in 0..packet::WIDTH {
                let mut bad = fixture.clone();
                bad.ports[port][field] = bad.ports[port][field].add(F::ONE);
                assert!(!bad.accepts(), "port={port} field={field}");
            }
        }
        let mut bad = fixture.clone();
        bad.row[0] = bad.row[0].add(F::ONE);
        assert!(!bad.accepts());
    }
    assert!(!Fixture::entry(7, 9, true).accepts());
    assert!(!Fixture::entry(0, 9, false).accepts());
    assert!(!Fixture::returning(0, 0, 9).accepts());
    assert!(Schedule::new(7, Transition::RootEntry, [0, 0, 1]).is_none());
    assert!(Schedule::new(7, Transition::Return, [2, 1, 3]).is_none());
}

#[test]
fn reset_reuse_parent_transplant_and_wrapping_counter_do_not_form_lifecycle_steps() {
    let mut child = Fixture::entry(3, 9, false);
    child.ports[2][AFTER] = F(2); // A valid ancestor is not the immediate active parent.
    assert!(!child.accepts());
    let mut returning = Fixture::returning(10, 3, 10);
    returning.ports[0][AFTER] = F::ZERO;
    assert!(!returning.accepts());
    let mut reused = Fixture::entry(0, 9, true);
    reused.ports[0][AFTER] = F(3);
    reused.ports[1][AFTER] = F(3);
    reused.ports[2][packet::GENERATION] = F(3);
    assert!(!reused.accepts());
    let mut wrapping = Fixture::entry(0, 0, true);
    wrapping.ports[0][BEFORE] = F(u64::from(u16::MAX));
    wrapping.ports[0][AFTER] = F::ZERO;
    wrapping.ports[1][AFTER] = F::ZERO;
    wrapping.ports[2] = owner(0, PARENT, 0, 0, true, 12);
    assert!(!wrapping.accepts());
}

#[test]
fn inactive_fixed_slots_are_zero_and_private_dispatch_is_boolean_and_exclusive() {
    for transition in [
        Transition::RootEntry,
        Transition::ChildEntry,
        Transition::Return,
    ] {
        let inactive = Fixture {
            schedule: Schedule::new(7, transition, [10, 11, 12]).unwrap(),
            row: [F::ZERO; WIDTH],
            selection: [F::ZERO; 3],
            ports: [[F::ZERO; packet::WIDTH]; 3],
        };
        assert!(inactive.accepts());
        for port in 0..3 {
            for field in 0..packet::WIDTH {
                let mut bad = inactive.clone();
                bad.ports[port][field] = F::ONE;
                assert!(!bad.accepts(), "inactive port={port} field={field}");
            }
        }
        let mut bad = inactive.clone();
        bad.row[0] = F::ONE;
        assert!(!bad.accepts());
        for selected in 0..3 {
            let mut wrong = inactive.clone();
            wrong.selection[selected] = F(2);
            assert!(!wrong.accepts());
        }
        let mut ambiguous = inactive;
        ambiguous.selection = [F::ONE, F::ONE, F::ZERO];
        assert!(!ambiguous.accepts());
    }
    for fixture in [
        Fixture::entry(0, 0, true),
        Fixture::entry(3, 9, false),
        Fixture::returning(10, 3, 10),
    ] {
        let mut disabled = fixture.clone();
        disabled.selection = [F::ZERO; 3];
        assert!(!disabled.accepts());
        let mut other_role = fixture;
        other_role.selection.rotate_left(1);
        assert!(!other_role.accepts());
    }
}

#[test]
fn lifecycle_residues_remain_polynomial_in_every_private_owner_field() {
    use crate::execution_proofs::stark::proof_managed_note_stark::degree_audit::measured_maximum_affine_degree_v1;
    let total = WIDTH + 3 * packet::WIDTH + 3;
    for transition in [
        Transition::RootEntry,
        Transition::ChildEntry,
        Transition::Return,
    ] {
        let schedule = Schedule::new(7, transition, [10, 11, 12]).unwrap();
        let degree = measured_maximum_affine_degree_v1(
            [93; 32],
            [total, 0, 0, 0, 0],
            8,
            2,
            |row, _, _, _, _| {
                let first = WIDTH;
                let second = first + packet::WIDTH;
                let third = second + packet::WIDTH;
                let mut output = Vec::new();
                append_residues(
                    &mut output,
                    schedule,
                    row[..first].try_into().unwrap(),
                    row[third + packet::WIDTH..].try_into().unwrap(),
                    Ports {
                        counter: row[first..second].try_into().unwrap(),
                        active: row[second..third].try_into().unwrap(),
                        parent: row[third..third + packet::WIDTH].try_into().unwrap(),
                    },
                );
                Ok::<_, core::convert::Infallible>(output)
            },
        );
        assert_eq!(degree, 2);
    }
}

#[test]
fn coherent_parent_substitution_fails_the_same_typed_history() {
    use super::super::{NOTE_COPY_WIDTH_V1, PublicPacketBus, sorted};
    fn events(fixtures: &mut [Fixture]) -> Vec<Option<Event>> {
        fixtures
            .iter_mut()
            .enumerate()
            .flat_map(|(step, fixture)| {
                fixture.schedule.clocks = core::array::from_fn(|slot| (3 * step + slot) as u32);
                fixture
                    .ports
                    .iter_mut()
                    .enumerate()
                    .map(|(slot, port)| {
                        port[packet::CLOCK] = F((3 * step + slot) as u64);
                        Event {
                            space: Space::Owner,
                            vm: 7,
                            generation: u16::try_from(port[packet::GENERATION].0).unwrap(),
                            index: u32::try_from(port[packet::INDEX].0).unwrap(),
                            write: port[packet::WRITE] == F::ONE,
                            before: value(u16::try_from(port[BEFORE].0).unwrap()),
                            after: value(u16::try_from(port[AFTER].0).unwrap()),
                            before_private: 0,
                            after_private: 0,
                        }
                    })
                    .map(Some)
                    .collect::<Vec<_>>()
            })
            .collect()
    }
    fn history_accepts(mut fixtures: [Fixture; 3]) -> bool {
        let inputs = events(&mut fixtures);
        assert!(fixtures.iter().all(Fixture::accepts));
        let bus = PublicPacketBus::new(inputs).unwrap();
        let columns = bus.columns();
        (0..bus.size()).all(|index| {
            let row = columns[NOTE_COPY_WIDTH_V1..]
                .iter()
                .map(|column| column[index])
                .collect::<Vec<_>>();
            let next = columns[NOTE_COPY_WIDTH_V1..]
                .iter()
                .map(|column| column[(index + 1) % bus.size()])
                .collect::<Vec<_>>();
            let mut residues = Vec::new();
            sorted::append_residues(&mut residues, &row, &next, &bus.fixed(index));
            residues.into_iter().all(|residue| residue == F::ZERO)
        })
    }
    let root = Fixture::entry(0, 0, true);
    let child = Fixture::entry(1, 1, false);
    let valid_return = Fixture::returning(2, 1, 2);
    assert!(history_accepts([root.clone(), child.clone(), valid_return]));
    let forged_return = Fixture::returning(2, 0, 2);
    assert!(
        forged_return.accepts(),
        "the local bank deliberately relies on the same original history"
    );
    assert!(!history_accepts([root, child, forged_return]));
}
