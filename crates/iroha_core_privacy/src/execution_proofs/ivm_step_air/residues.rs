//! Bounded streaming storage for private polynomial residues.

use super::F;

/// Append-only equation output; evaluation never reads back private residues.
pub(super) trait Sink {
    fn len(&self) -> usize;
    fn push(&mut self, value: F);
    fn extend(&mut self, values: impl IntoIterator<Item = F>) {
        for value in values {
            self.push(value);
        }
    }
}

impl Sink for Vec<F> {
    fn len(&self) -> usize {
        Vec::len(self)
    }
    fn push(&mut self, value: F) {
        Vec::push(self, value);
    }
}

/// Multiply each equation as it is emitted, preserving append-only streaming.
pub(super) struct Scaled<'a, S: Sink> {
    sink: &'a mut S,
    factor: F,
}
impl<'a, S: Sink> Scaled<'a, S> {
    pub(super) fn new(sink: &'a mut S, factor: F) -> Self {
        Self { sink, factor }
    }
}
impl<S: Sink> Sink for Scaled<'_, S> {
    fn len(&self) -> usize {
        self.sink.len()
    }
    fn push(&mut self, value: F) {
        self.sink.push(self.factor.mul(value));
    }
}

/// One history row is the largest repeatedly evaluated bank.
pub(super) const CAPACITY: usize = 721;

/// Fixed stack scratch, wiped on every flush and on unwind.
pub(super) struct Scratch([F; CAPACITY]);
impl Scratch {
    pub(super) fn new() -> Self {
        Self([F::ZERO; CAPACITY])
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        for value in &mut self.0 {
            value.zeroize_v1();
        }
    }
}

/// A fallible consumer sees bounded batches in exact equation order. A failed
/// consumer cannot produce success, even when the current bank finishes first.
pub(super) struct Stream<'a, E, C: FnMut(&[F]) -> Result<(), E>> {
    scratch: &'a mut Scratch,
    consumer: &'a mut C,
    count: usize,
    filled: usize,
    error: Option<E>,
}
impl<'a, E, C: FnMut(&[F]) -> Result<(), E>> Stream<'a, E, C> {
    pub(super) fn new(scratch: &'a mut Scratch, consumer: &'a mut C) -> Self {
        Self {
            scratch,
            consumer,
            count: 0,
            filled: 0,
            error: None,
        }
    }
    fn flush(&mut self) {
        if self.filled != 0 {
            if self.error.is_none() {
                self.error = (self.consumer)(&self.scratch.0[..self.filled]).err();
            }
            for value in &mut self.scratch.0[..self.filled] {
                value.zeroize_v1();
            }
            self.filled = 0;
        }
    }
    pub(super) fn finish(mut self) -> Result<(), E> {
        self.flush();
        self.error.take().map_or(Ok(()), Err)
    }
}
impl<E, C: FnMut(&[F]) -> Result<(), E>> Sink for Stream<'_, E, C> {
    fn len(&self) -> usize {
        self.count
    }
    fn push(&mut self, value: F) {
        self.count += 1;
        if self.error.is_some() {
            return;
        }
        self.scratch.0[self.filled] = value;
        self.filled += 1;
        if self.filled == CAPACITY {
            self.flush();
        }
    }
}
impl<E, C: FnMut(&[F]) -> Result<(), E>> Drop for Stream<'_, E, C> {
    fn drop(&mut self) {
        for value in &mut self.scratch.0[..self.filled] {
            value.zeroize_v1();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scaling_preserves_equation_count_order_and_streaming_cleanup() {
        let mut scratch = Scratch::new();
        let mut values = Vec::new();
        let mut consume = |batch: &[F]| {
            values.extend_from_slice(batch);
            Ok::<_, ()>(())
        };
        let mut stream = Stream::new(&mut scratch, &mut consume);
        stream.push(F(7));
        let mut scaled = Scaled::new(&mut stream, F(3));
        scaled.extend([F(2), F(4), F(6)]);
        assert_eq!(scaled.len(), 4);
        stream.finish().unwrap();
        assert_eq!(values, [F(7), F(6), F(12), F(18)]);
        assert!(scratch.0.iter().all(|value| *value == F::ZERO));
    }
    #[test]
    fn streams_exact_order_without_growing_and_wipes_scratch() {
        let mut scratch = Scratch::new();
        let mut seen = 0;
        let mut consume = |batch: &[F]| {
            assert!(batch.len() <= CAPACITY);
            for value in batch {
                assert_eq!(*value, F(seen));
                seen += 1;
            }
            Ok::<_, ()>(())
        };
        let mut stream = Stream::new(&mut scratch, &mut consume);
        stream.extend((0..(CAPACITY * 3 + 7) as u64).map(F));
        assert_eq!(stream.len(), CAPACITY * 3 + 7);
        stream.finish().unwrap();
        assert_eq!(seen, (CAPACITY * 3 + 7) as u64);
        assert!(scratch.0.iter().all(|value| *value == F::ZERO));
    }
    #[test]
    fn rejection_and_unwind_wipe_live_scratch() {
        let mut scratch = Scratch::new();
        let mut reject = |_: &[F]| Err(7);
        let mut stream = Stream::new(&mut scratch, &mut reject);
        stream.extend((0..CAPACITY * 2).map(|_| F::ONE));
        assert_eq!(stream.finish(), Err(7));
        assert!(scratch.0.iter().all(|value| *value == F::ZERO));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut accept = |_: &[F]| Ok::<_, ()>(());
            let mut stream = Stream::new(&mut scratch, &mut accept);
            stream.push(F::ONE);
            panic!("private evaluator unwind");
        }));
        assert!(result.is_err());
        assert!(scratch.0.iter().all(|value| *value == F::ZERO));
    }
    #[test]
    fn consumer_panic_during_flush_wipes_the_entire_live_batch() {
        let mut scratch = Scratch::new();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut panic_consumer =
                |_: &[F]| -> Result<(), ()> { panic!("consumer failed during flush") };
            let mut stream = Stream::new(&mut scratch, &mut panic_consumer);
            stream.extend((0..CAPACITY).map(|_| F::ONE));
        }));
        assert!(result.is_err());
        assert!(scratch.0.iter().all(|value| *value == F::ZERO));
    }
}
