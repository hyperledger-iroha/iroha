//! Bounded delivery state; neither cached replies nor fingerprints grant authority.
use super::*;
use iroha_crypto::{Blake2b256, blake2::digest::Digest as _};

/// Created only after Native has authenticated all installation originals.
pub(super) struct InstalledOriginals {
    parameters: [u8; 32],
    fingerprint: [u8; 32],
}
impl InstalledOriginals {
    pub(super) fn authenticated(
        parameters: [u8; 32],
        bundle: &[u8],
        manifest: &[u8],
        signature: &[u8; 64],
    ) -> Self {
        Self {
            parameters,
            fingerprint: Self::fingerprint(&parameters, bundle, manifest, signature),
        }
    }
    fn fingerprint(
        parameters: &[u8; 32],
        bundle: &[u8],
        manifest: &[u8],
        signature: &[u8; 64],
    ) -> [u8; 32] {
        let mut hash = Blake2b256::new();
        hash.update(b"iroha.kagemusha.spartan.installed-originals.v1\0");
        for original in [
            parameters.as_slice(),
            bundle,
            manifest,
            signature.as_slice(),
        ] {
            hash.update((original.len() as u64).to_le_bytes());
            hash.update(original);
        }
        hash.finalize().into()
    }
    pub(super) fn retry(
        &self,
        bundle: &[u8],
        manifest: &[u8],
        signature: &[u8; 64],
    ) -> Result<Response> {
        if Self::fingerprint(&self.parameters, bundle, manifest, signature) != self.fingerprint {
            return Err(Failure::code(CONFLICT));
        }
        Ok(response(81, self.parameters.to_vec()))
    }
}

struct Entry<T> {
    review: T,
    terminal: Option<Result<Response>>,
}
pub(super) struct ReviewCache<T> {
    next: u64,
    entries: BTreeMap<u64, Entry<T>>,
}
impl<T> Default for ReviewCache<T> {
    fn default() -> Self {
        Self {
            next: 0,
            entries: BTreeMap::new(),
        }
    }
}
fn copy_reply(reply: &Response) -> Response {
    Response {
        kind: reply.kind,
        sequence: reply.sequence,
        detail: reply.detail,
        bytes: reply.bytes.clone(),
        backlog: reply.backlog,
    }
}
impl<T> ReviewCache<T> {
    pub(super) fn find(&self, predicate: impl Fn(&T) -> bool) -> Option<(u64, &T)> {
        self.entries.iter().find_map(|(&token, entry)| {
            // A new Prepare after a delivered NotPerformed is a fresh review.
            // Keep the prior terminal token only for an exact Execute reply retry.
            (entry.terminal.is_none() && predicate(&entry.review)).then_some((token, &entry.review))
        })
    }
    pub(super) fn insert(&mut self, review: T) -> Result<u64> {
        let token = self
            .next
            .checked_add(1)
            .filter(|v| *v <= i64::MAX as u64)
            .ok_or(Failure::code(RESOURCE))?;
        if self.entries.len() >= 32 {
            let completed = self
                .entries
                .iter()
                .find_map(|(&id, entry)| entry.terminal.is_some().then_some(id))
                .ok_or(Failure::code(RESOURCE))?;
            self.entries.remove(&completed);
        }
        self.entries.insert(
            token,
            Entry {
                review,
                terminal: None,
            },
        );
        self.next = token;
        Ok(token)
    }
    pub(super) fn active(&mut self, token: u64) -> Result<&mut T> {
        self.entries
            .get_mut(&token)
            .filter(|entry| entry.terminal.is_none())
            .map(|entry| &mut entry.review)
            .ok_or(Failure::code(INVALID))
    }
    pub(super) fn execute(
        &mut self,
        token: u64,
        action: impl FnOnce(&T) -> Result<Response>,
    ) -> Result<Response> {
        let entry = self.entries.get_mut(&token).ok_or(Failure::code(INVALID))?;
        if let Some(result) = &entry.terminal {
            return result.as_ref().map(copy_reply).map_err(|error| *error);
        }
        // A definite refusal belongs to this token even when the caller loses the reply
        // or a later regulatory update would permit a newly reviewed transfer.
        let terminal = match action(&entry.review) {
            Ok(reply) if matches!(reply.kind, 1 | 3 | 4 | 5) => Ok(reply),
            Ok(reply) if reply.kind == 2 => return Ok(reply),
            Ok(_) => return Err(Failure::code(INTERNAL)),
            Err(error) if error.status == BLOCKED => Err(error),
            Err(error) => return Err(error),
        };
        let reply = terminal.as_ref().map(copy_reply).map_err(|error| *error);
        entry.terminal = Some(terminal);
        reply
    }
    pub(super) fn discard(&mut self, token: u64) -> Option<T> {
        let entry = self.entries.remove(&token)?;
        entry.terminal.is_none().then_some(entry.review)
    }
}

#[cfg(test)]
mod tests;
