//! Borrowed ancestor chain for the static authority declaration graph.
//!
//! Each entry lives in its existing traversal stack frame. No heap scratch,
//! source cloning or new graph limit is needed to reject the first cycle.

pub(super) struct DerivationPath<'a> {
    identity: &'static str,
    parent: Option<&'a DerivationPath<'a>>,
}

impl DerivationPath<'_> {
    pub(super) fn root(identity: &'static str) -> Self {
        Self {
            identity,
            parent: None,
        }
    }

    pub(super) fn child<'a>(&'a self, identity: &'static str) -> DerivationPath<'a> {
        DerivationPath {
            identity,
            parent: Some(self),
        }
    }

    pub(super) fn contains(&self, identity: &str) -> bool {
        let mut current = Some(self);
        while let Some(entry) = current {
            if entry.identity == identity {
                return true;
            }
            current = entry.parent;
        }
        false
    }
}

#[cfg(test)]
mod tests;
