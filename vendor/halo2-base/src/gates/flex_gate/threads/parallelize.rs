use rayon::prelude::*;

use crate::{utils::ScalarField, Context};

use super::SinglePhaseCoreManager;

/// Utility function to parallelize an operation involving [`Context`]s.
pub fn parallelize_core<F, T, R, FR>(
    builder: &mut SinglePhaseCoreManager<F>, // leaving `builder` for historical reasons, `pool` is a better name
    input: Vec<T>,
    f: FR,
) -> Vec<R>
where
    F: ScalarField,
    T: Send,
    R: Send,
    FR: Fn(&mut Context<F>, T) -> R + Send + Sync,
{
    // to prevent concurrency issues with context id, we generate all the ids first
    let thread_count = builder.thread_count();
    let mut ctxs =
        (0..input.len()).map(|i| builder.new_context(thread_count + i)).collect::<Vec<_>>();
    let outputs: Vec<_> =
        input.into_par_iter().zip(ctxs.par_iter_mut()).map(|(input, ctx)| f(ctx, input)).collect();
    // we collect the new threads to ensure they are a FIXED order, otherwise the circuit will not be deterministic
    builder.threads.append(&mut ctxs);

    outputs
}
