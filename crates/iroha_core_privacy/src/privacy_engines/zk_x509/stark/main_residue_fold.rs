//! Bounded, clearing accumulator for canonically ordered MAIN residues.

use super::super::super::sha_residue_output::ShaResidueSinkV1;
use super::main_composition_mix::MainResidueMixV1;
use super::{E, F, ZkX509StarkErrorV1};

/// One partial quotient contribution; never publish an incomplete emission.
pub(super) struct MainResidueFoldV1<'a> {
    alphas: &'a [E],
    mix: MainResidueMixV1,
    emitted: usize,
    refused: bool,
}

impl<'a> MainResidueFoldV1<'a> {
    pub(super) fn new_v1(alphas: &'a [E]) -> Self {
        Self {
            alphas,
            mix: MainResidueMixV1::new_v1(),
            emitted: 0,
            refused: false,
        }
    }

    pub(super) fn finish_v1(self, inverse: F) -> Result<E, ZkX509StarkErrorV1> {
        if self.refused || self.emitted != self.alphas.len() {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Ok(self.mix.finish_v1().mul_base(inverse))
    }
}

impl ShaResidueSinkV1<F> for MainResidueFoldV1<'_> {
    fn len(&self) -> usize {
        self.emitted
    }

    fn reserve(&mut self, _additional: usize) {}

    fn push(&mut self, value: F) {
        let Some(next) = self.emitted.checked_add(1) else {
            self.refused = true;
            return;
        };
        if let Some(alpha) = self.alphas.get(self.emitted) {
            self.mix.push_v1(value, alpha);
        } else {
            self.refused = true;
        }
        self.emitted = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::privacy_engines::zk_x509::private_table::inspection;

    fn alpha(index: u64) -> E {
        E::from_coefficients([F(index + 1), F(index + 3), F(index + 7), F(index + 11)]).unwrap()
    }

    #[test]
    fn streamed_fold_matches_the_exact_ordered_vector_fold() {
        for count in [1, 155, 335, 593, 801, 1024, 1025, 2145] {
            let alphas = (0..count).map(|i| alpha(i as u64)).collect::<Vec<_>>();
            let values = (0..count)
                .map(|i| F((i * i + 3) as u64))
                .collect::<Vec<_>>();
            let mut owner = MainResidueFoldV1::new_v1(&alphas);
            owner.reserve(count);
            owner.extend_residues(values.iter().copied());
            let expected = values
                .iter()
                .zip(&alphas)
                .fold(E::ZERO, |sum, (value, a)| sum.add(a.mul_base(*value)))
                .mul_base(F(17));
            assert!(owner.finish_v1(F(17)).unwrap() == expected);
        }
    }

    #[test]
    fn incomplete_excess_and_overflow_emission_never_publish_a_sum() {
        let alphas = [alpha(1), alpha(2)];
        for emitted in [0, 1, 3, 4] {
            let mut owner = MainResidueFoldV1::new_v1(&alphas);
            for _ in 0..emitted {
                owner.push(F(23));
            }
            assert!(matches!(
                owner.finish_v1(F::ONE),
                Err(ZkX509StarkErrorV1::ProfileMismatch)
            ));
        }
        let mut owner = MainResidueFoldV1::new_v1(&alphas);
        owner.emitted = usize::MAX;
        owner.push(F(23));
        assert!(owner.refused);
        assert!(owner.finish_v1(F::ONE).is_err());
    }

    #[test]
    fn partial_fold_clears_on_success_error_and_unwind() {
        let alphas = [alpha(7), alpha(11)];
        for outcome in 0..3 {
            let (result, erased) = inspection::observe_v1(|| {
                std::panic::catch_unwind(|| {
                    let mut owner = MainResidueFoldV1::new_v1(&alphas);
                    owner.push(F(19));
                    match outcome {
                        0 => {
                            owner.push(F(29));
                            assert!(owner.finish_v1(F::ONE).is_ok());
                        }
                        1 => assert!(owner.finish_v1(F::ONE).is_err()),
                        _ => panic!("synthetic ordered residue emission unwind"),
                    }
                })
            });
            assert_eq!(result.is_err(), outcome == 2);
            assert!(erased.iter().any(|r| r.cells == 4 && r.nonzero_before > 0));
            assert!(erased.iter().all(|r| r.nonzero_after == 0));
        }
    }
}
