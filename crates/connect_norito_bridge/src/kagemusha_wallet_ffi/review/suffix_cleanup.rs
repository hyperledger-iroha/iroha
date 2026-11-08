//! Bounded metadata for idempotent cleanup of this exact origin's retired suffix review.
//! A tombstone contains no review, projection, proof, or authority to execute anything.
use super::*;

const MAX_RETIRED_SUFFIX_REVIEWS: usize = 64;

fn remember<T>(tokens: &mut Tokens<T>, token: u64, origin: u64) {
    // Native token allocation is monotonic and never reuses an identity. Repeating a
    // cleanup does not renew its eviction position or let an old origin starve new ones.
    if tokens.suffix_retired.iter().any(|(id, _)| *id == token) {
        return;
    }
    if tokens.suffix_retired.len() == MAX_RETIRED_SUFFIX_REVIEWS {
        tokens.suffix_retired.pop_front();
    }
    tokens.suffix_retired.push_back((token, origin));
}

fn prune<T>(tokens: &mut Tokens<OriginBound<T>>, ended_suffix: impl Fn(&T) -> bool) {
    let mut retired = Vec::new();
    tokens.values.retain(|token, bound| {
        if ended_suffix(&bound.value) {
            retired.push((*token, bound.origin));
            false
        } else {
            true
        }
    });
    for (token, origin) in retired {
        remember(tokens, token, origin);
    }
}

fn discard<T>(
    tokens: &mut Tokens<OriginBound<T>>,
    origin: u64,
    token: u64,
    is_suffix: impl Fn(&T) -> bool,
) -> Result<()> {
    if let Some(bound) = tokens.get(token) {
        // Check before removal: a foreign caller cannot consume either ordinary or
        // suffix authority, including an ended suffix cell awaiting metadata pruning.
        bound.require_origin(origin)?;
        let suffix = is_suffix(&bound.value);
        let removed = tokens.take(token)?;
        if suffix {
            remember(tokens, token, origin);
        }
        drop(removed);
        return Ok(());
    }
    if origin != 0
        && token != 0
        && tokens
            .suffix_retired
            .iter()
            .any(|(id, owner)| *id == token && *owner == origin)
    {
        Ok(())
    } else {
        Err(Failure::code(INVALID))
    }
}

impl Tokens<OriginBound<FinancialReview>> {
    pub(crate) fn prune_ended_suffix(&mut self) {
        prune(
            self,
            |review| matches!(review, FinancialReview::Suffix(cell) if cell.ended()),
        );
    }

    pub(crate) fn discard_financial(&mut self, origin: u64, token: u64) -> Result<()> {
        discard(self, origin, token, |review| {
            matches!(review, FinancialReview::Suffix(_))
        })
    }
}

#[cfg(test)]
#[path = "suffix_cleanup/tests.rs"]
mod tests;
