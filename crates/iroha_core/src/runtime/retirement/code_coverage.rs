//! Exact native code identity coverage; retained source ownership remains a separate gate.

use super::*;
use iroha_data_model::runtime::{
    definition::{DefinitionDescriptorV1, ImplementationCodeV1},
    retirement::MAX_RETIREMENT_DEFINITIONS,
};
use mv::storage::StorageReadOnly as _;

/// Authenticated retirement of all compiled and retained native implementation aliases.
///
/// This seal does not authorize deleting source. In particular, descriptor identities cannot
/// prove that shared Rust source is unused by retained codecs, readers or proof verification.
pub(crate) struct NativeCodeCoverage {
    identity: Hash,
    aliases: usize,
    roots: usize,
    evidence: Hash,
}

impl NativeCodeCoverage {
    /// Canonical native module plus archived reference-closure identity.
    pub(crate) fn identity(&self) -> Hash {
        self.identity
    }
    /// Exact number of immutable implementation definitions covered.
    pub(crate) fn aliases(&self) -> usize {
        self.aliases
    }
    /// Number of authenticated roots covered for each alias.
    pub(crate) fn roots(&self) -> usize {
        self.roots
    }
    /// Digest of the code identity and every original alias/root evidence graph.
    pub(crate) fn evidence(&self) -> Hash {
        self.evidence
    }
}

fn implementation_code(descriptor: &DefinitionDescriptorV1) -> Option<&ImplementationCodeV1> {
    match descriptor {
        DefinitionDescriptorV1::Implementation(implementation) => Some(&implementation.code),
        _ => None,
    }
}

/// Require every immutable alias of an exact native implementation on every registered root.
///
/// The compiled catalog supplies aliases even if this root never selected them. Retained
/// published definitions extend that set; neither retirement of one alias nor absence from
/// this root's selection can hide another alias. A matching migration remains retained.
/// Evidence is streamed one alias/root at a time, with a fixed alias-count bound.
///
/// # Errors
/// An absent alias/root receipt, mismatched code, retained migration, invalid finality, or an
/// original resource refusal. Source ownership and physical source removal are not assessed.
pub(crate) fn verify_native_code_coverage(
    view: &impl StateReadOnly,
    definition: DefinitionId,
    read: &mut dyn FnMut(
        DefinitionId,
        RuntimeRetirementRootV1,
    ) -> Result<Option<RuntimeRetirementProofV1>, Attempt<Error>>,
) -> Result<NativeCodeCoverage, Attempt<Error>> {
    let catalog = crate::runtime::release_catalog().map_err(invalid)?;
    let code = catalog
        .definitions
        .get(&definition)
        .and_then(implementation_code)
        .filter(|code| matches!(code, ImplementationCodeV1::Native { .. }))
        .ok_or_else(|| invalid("code coverage requires a compiled native implementation"))?;
    let bytes = norito::core::classify_decode_attempt(|| norito::encode_canonical(code))
        .map_err(|original| canonical_decode_attempt_error(original, invalid))?;
    let identity = Hash::new(bytes);
    let chain = CertifiedChain::new(view).map_err(|error| error.map_rejection(invalid))?;
    let parent = RuntimeRetirementRootV1 {
        network: *view.network_id(),
        scope: SumeragiRootScope::Global,
        instance: chain.instance().0,
    };
    let mut evidence = Hash::new_from_chunks(&[
        b"iroha:runtime:native-code-coverage:v1\0",
        identity.as_ref(),
    ]);
    let mut aliases = 0usize;
    let mut roots = None;
    let retained = view.world().runtime_definitions();
    for (alias, descriptor) in catalog.definitions.iter().chain(
        retained
            .iter()
            .filter(|(id, _)| !catalog.definitions.contains_key(id)),
    ) {
        if matches!(descriptor, DefinitionDescriptorV1::Migration(migration) if &migration.code == code)
        {
            return Err(invalid("native source identity also owns a retained migration").into());
        }
        if implementation_code(descriptor) != Some(code) {
            continue;
        }
        aliases = aliases
            .checked_add(1)
            .filter(|count| *count <= MAX_RETIREMENT_DEFINITIONS)
            .ok_or_else(|| invalid("native code alias coverage exceeds capacity"))?;
        let proof = read(*alias, parent)?.ok_or_else(|| {
            invalid(format!(
                "native code alias {alias} lacks its parent receipt"
            ))
        })?;
        let coverage = verify_coverage(view, *alias, &proof, &mut |root| read(*alias, root))?;
        if roots.is_some_and(|count| count != coverage.roots()) {
            return Err(invalid("native code aliases do not cover the same root registry").into());
        }
        roots = Some(coverage.roots());
        evidence =
            Hash::new_from_chunks(&[evidence.as_ref(), &alias.0, coverage.evidence().as_ref()]);
    }
    Ok(NativeCodeCoverage {
        identity,
        aliases,
        roots: roots.ok_or_else(|| invalid("native code has no compiled alias"))?,
        evidence,
    })
}
