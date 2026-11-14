//! Immutable public fixed tails. Compiler witnesses stay dense; imported and
//! retained verifiers store only maximal nonzero runs and their literal fields.
use super::Scalar;

#[derive(Clone, Copy, Debug)]
pub(super) struct LiteralSpan {
    pub start: usize,
    pub offset: usize,
    pub len: usize,
}

pub(super) enum FixedValues<F> {
    Dense(Vec<F>),
    Runs {
        len: usize,
        spans: Box<[LiteralSpan]>,
        literals: Box<[F]>,
    },
}

impl<F: Scalar> FixedValues<F> {
    pub fn len(&self) -> usize {
        match self {
            Self::Dense(values) => values.len(),
            Self::Runs { len, .. } => *len,
        }
    }

    pub fn heap_bytes(&self) -> u128 {
        match self {
            Self::Dense(values) => (values.capacity() as u128) * std::mem::size_of::<F>() as u128,
            Self::Runs {
                spans, literals, ..
            } => {
                std::mem::size_of_val(spans.as_ref()) as u128
                    + std::mem::size_of_val(literals.as_ref()) as u128
            }
        }
    }

    pub fn compact(self) -> Self {
        let Self::Dense(values) = self else {
            return self;
        };
        let mut spans = Vec::new();
        let mut literals = Vec::new();
        let mut at = 0;
        while at < values.len() {
            if bool::from(values[at].is_zero()) {
                at += 1;
                continue;
            }
            let start = at;
            while at < values.len() && !bool::from(values[at].is_zero()) {
                at += 1;
            }
            spans.push(LiteralSpan {
                start,
                offset: literals.len(),
                len: at - start,
            });
            literals.extend_from_slice(&values[start..at]);
        }
        Self::Runs {
            len: values.len(),
            spans: spans.into_boxed_slice(),
            literals: literals.into_boxed_slice(),
        }
    }

    /// Preserve the existing Circuit API's exact expanded, row-major values.
    pub fn expanded(&self) -> Vec<F> {
        match self {
            Self::Dense(values) => values.clone(),
            Self::Runs {
                len,
                spans,
                literals,
            } => {
                let mut values = vec![F::ZERO; *len];
                for span in spans.iter() {
                    values[span.start..span.start + span.len]
                        .copy_from_slice(&literals[span.offset..span.offset + span.len]);
                }
                values
            }
        }
    }

    pub fn iter(&self) -> FixedIter<'_, F> {
        FixedIter {
            values: self,
            at: 0,
            span: 0,
        }
    }

    pub fn value(&self, at: usize) -> Option<F> {
        if at >= self.len() {
            return None;
        }
        match self {
            Self::Dense(values) => Some(values[at]),
            Self::Runs {
                spans, literals, ..
            } => {
                let before = spans.partition_point(|span| span.start <= at);
                if before > 0 {
                    let span = &spans[before - 1];
                    if at - span.start < span.len {
                        return Some(literals[span.offset + at - span.start]);
                    }
                }
                Some(F::ZERO)
            }
        }
    }

    /// Visit the same maximal alternating runs used by the existing wire codec.
    pub fn visit_runs<E>(
        &self,
        mut visit: impl FnMut(usize, Option<&[F]>) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Dense(values) => {
                let mut at = 0;
                while at < values.len() {
                    let zero = bool::from(values[at].is_zero());
                    let start = at;
                    at += 1;
                    while at < values.len() && bool::from(values[at].is_zero()) == zero {
                        at += 1;
                    }
                    visit(at - start, (!zero).then_some(&values[start..at]))?;
                }
            }
            Self::Runs {
                len,
                spans,
                literals,
            } => {
                let mut at = 0;
                for span in spans.iter() {
                    if span.start > at {
                        visit(span.start - at, None)?;
                    }
                    visit(
                        span.len,
                        Some(&literals[span.offset..span.offset + span.len]),
                    )?;
                    at = span.start + span.len;
                }
                if at < *len {
                    visit(*len - at, None)?;
                }
            }
        }
        Ok(())
    }

    /// Hash logical canonical fields, never the storage tags. Batch zeros to
    /// avoid allocating their expanded tail or issuing millions of tiny updates.
    pub fn hash_into(&self, h: &mut blake3::Hasher) {
        let mut zeros = [0u8; 8192];
        for field in zeros.chunks_exact_mut(32) {
            field.copy_from_slice(&F::ZERO.to_repr());
        }
        self.visit_runs::<std::convert::Infallible>(|mut count, values| {
            if let Some(values) = values {
                for value in values {
                    h.update(&value.to_repr());
                }
            } else {
                while count > 0 {
                    let fields = count.min(zeros.len() / 32);
                    h.update(&zeros[..fields * 32]);
                    count -= fields;
                }
            }
            Ok(())
        })
        .unwrap();
    }
}

pub(super) struct FixedIter<'a, F> {
    values: &'a FixedValues<F>,
    at: usize,
    span: usize,
}
impl<F: Scalar> Iterator for FixedIter<'_, F> {
    type Item = F;
    fn next(&mut self) -> Option<F> {
        if self.at == self.values.len() {
            return None;
        }
        let value = match self.values {
            FixedValues::Dense(values) => values[self.at],
            FixedValues::Runs {
                spans, literals, ..
            } => {
                if self.span < spans.len() && self.at >= spans[self.span].start {
                    let span = &spans[self.span];
                    let value = literals[span.offset + self.at - span.start];
                    if self.at + 1 == span.start + span.len {
                        self.span += 1;
                    }
                    value
                } else {
                    F::ZERO
                }
            }
        };
        self.at += 1;
        Some(value)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let left = self.values.len() - self.at;
        (left, Some(left))
    }
}
impl<F: Scalar> ExactSizeIterator for FixedIter<'_, F> {}

#[cfg(test)]
mod tests {
    use super::*;

    fn check<F: Scalar>() {
        for values in [
            vec![],
            vec![F::ZERO; 4097],
            vec![F::ONE; 4097],
            (0..8193)
                .map(|i| {
                    if i % 3 == 0 {
                        F::from((i + 1) as u64)
                    } else {
                        F::ZERO
                    }
                })
                .collect(),
            vec![F::ZERO, F::from(9), F::ONE, F::ZERO, F::ZERO, F::from(3)],
        ] {
            let mut expected = blake3::Hasher::new();
            for value in &values {
                expected.update(&value.to_repr());
            }
            let dense = FixedValues::Dense(values.clone());
            let mut dense_hash = blake3::Hasher::new();
            dense.hash_into(&mut dense_hash);
            assert_eq!(expected.finalize(), dense_hash.finalize());
            let compact = dense.compact();
            assert_eq!(compact.expanded(), values);
            assert_eq!(compact.iter().collect::<Vec<_>>(), values);
            for (i, value) in values.iter().enumerate() {
                assert_eq!(compact.value(i), Some(*value));
            }
            assert!(compact.value(values.len()).is_none());
            assert_eq!(compact.iter().len(), values.len());
            let mut compact_hash = blake3::Hasher::new();
            compact.hash_into(&mut compact_hash);
            assert_eq!(
                expected.finalize(),
                compact_hash.finalize(),
                "same logical field preimage"
            );
            let FixedValues::Runs {
                spans, literals, ..
            } = &compact
            else {
                unreachable!()
            };
            assert_eq!(
                compact.heap_bytes(),
                (spans.len() * std::mem::size_of::<LiteralSpan>()
                    + literals.len() * std::mem::size_of::<F>()) as u128
            );
        }
    }

    #[test]
    fn compact_fixed_tails_preserve_values_hashes_and_exact_heap_on_both_fields() {
        check::<iroha_pasta::Fp>();
        check::<iroha_pasta::Fq>();
    }
}
