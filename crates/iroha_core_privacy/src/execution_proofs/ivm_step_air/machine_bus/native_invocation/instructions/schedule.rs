//! Exact original compact prefix before the single fixed root return window.

use super::super::{F, Fields};
use super::{packet, private_dispatch};
use crate::execution_proofs::ivm_step_air::residues::Sink;
use ivm::execution_packets::{MAX_STEPS, NativeInvocation, instruction_clocks};

pub(super) fn append_native(
    out: &mut impl Sink,
    native: &NativeInvocation,
    window: usize,
    decoded: &private_dispatch::Decoded<'_>,
) {
    assert!(window < MAX_STEPS);
    let current =
        Fields::native(&native.packets()[instruction_clocks(window).unwrap()[0] as usize]);
    let next = (window + 1 < MAX_STEPS).then(|| {
        Fields::native(&native.packets()[instruction_clocks(window + 1).unwrap()[0] as usize])
    });
    append(
        out,
        window,
        current.0[packet::ENABLED],
        next.as_ref()
            .map_or(F::ZERO, |next| next.0[packet::ENABLED]),
        decoded.returning,
    );
}
fn append(out: &mut impl Sink, window: usize, current: F, next: F, returning: F) {
    out.push(next.mul(F::ONE.sub(current)));
    // The producer always places root JALR in its dedicated complete scan window.
    out.push(returning);
    // The profile counts root JALR among its 64 total instructions.
    out.push(current.mul(F(u64::from(window + 1 == MAX_STEPS))));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compact_prefix_rejects_holes_return_roles_and_the_sixty_fourth_nonreturn() {
        for window in 0..MAX_STEPS {
            for current in [F::ZERO, F::ONE] {
                for next in [F::ZERO, F::ONE] {
                    for returning in [F::ZERO, F::ONE] {
                        let mut residues = Vec::new();
                        append(&mut residues, window, current, next, returning);
                        assert_eq!(
                            residues.iter().all(|x| *x == F::ZERO),
                            (current == F::ONE || next == F::ZERO)
                                && returning == F::ZERO
                                && (window + 1 != MAX_STEPS || current == F::ZERO)
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn separately_valid_original_dispatch_windows_cannot_authorize_a_different_schedule() {
        use super::super::{
            Instructions, original,
            tests::{artifact, check, native, root},
        };
        let (native, budget) = native(artifact(&[], 0, false));
        let instructions = Instructions::new(&native, &root(&native), &budget).unwrap();
        // These banks are actual original producers, not a raw NativeInvocation
        // constructor or a caller's asserted opcode role.
        for window in [0, MAX_STEPS - 1, MAX_STEPS] {
            assert!(check(&instructions, &native, window));
        }
        let active = Fields::native(&native.packets()[instruction_clocks(0).unwrap()[0] as usize]);
        let inactive = Fields::native(
            &native.packets()[instruction_clocks(MAX_STEPS - 1).unwrap()[0] as usize],
        );
        let packets = original(&native, MAX_STEPS);
        let mut shared = Vec::new();
        let decoded = private_dispatch::append_residues(
            &mut shared,
            &instructions.program,
            private_dispatch::Schedule::new(0, instruction_clocks(MAX_STEPS).unwrap()).unwrap(),
            &instructions.rows.as_slice()[MAX_STEPS].0,
            &packets,
        );
        assert!(shared.iter().all(|x| *x == F::ZERO));
        for (window, current, next, returning) in [
            (
                0,
                inactive.0[packet::ENABLED],
                active.0[packet::ENABLED],
                F::ZERO,
            ),
            (0, active.0[packet::ENABLED], F::ZERO, decoded.returning),
            (MAX_STEPS - 1, active.0[packet::ENABLED], F::ZERO, F::ZERO),
        ] {
            let mut residues = Vec::new();
            append(&mut residues, window, current, next, returning);
            assert!(residues.iter().any(|x| *x != F::ZERO));
        }
    }
    #[test]
    fn compact_schedule_has_degree_two_in_original_selectors() {
        use crate::execution_proofs::stark::proof_managed_note_stark::degree_audit::measured_maximum_affine_degree_v1;
        let degree = measured_maximum_affine_degree_v1(
            [0x7c; 32],
            [3, 0, 0, 0, 0],
            8,
            2,
            |row, _, _, _, _| {
                let mut out = Vec::new();
                for window in 0..MAX_STEPS {
                    append(&mut out, window, row[0], row[1], row[2]);
                }
                Ok::<_, core::convert::Infallible>(out)
            },
        );
        assert_eq!(degree, 2);
    }
}
