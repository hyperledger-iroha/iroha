//! Compiled wire-schema identity shared by release manifests and `/status.build`.
//!
//! The identity hashes a canonical rendering of the compiled [`iroha_schema`]
//! description of the block wire ([`SignedBlock`]), together with the IVM ABI
//! hash. Two binaries that report equal hashes decode the same block layouts and
//! admit the same contract ABI. Consensus messages are not covered yet; their
//! frames carry their own Norito schema hash. The value is independent of the
//! compilation target and of process-local `TypeId` values, but it depends on
//! the enabled features: for example `PublicKey`'s `Algorithm` lists its `bls`,
//! `gost` and `sm` variants only when those features are compiled in. Release
//! tooling must read it from a build with the release feature set.
//!
//! `ivm_abi` depends on this crate, so the IVM ABI hash is an explicit input
//! rather than computed here. Executables obtain it from
//! `ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1)`.

use std::collections::BTreeMap;

use iroha_schema::{IntoSchema, MetaMap, Metadata};

use crate::block::SignedBlock;

/// Domain separator of the first-release wire-schema identity.
pub const WIRE_SCHEMA_HASH_DOMAIN_V1: &[u8] = b"iroha.wire_schema.v1\0";

/// Compiled schema of every type reachable from the covered wire roots.
#[must_use]
pub fn covered_wire_schema() -> MetaMap {
    let mut schema = MetaMap::new();
    // TODO(sumeragi): cover the live consensus wire (`iroha_sumeragi::message::WireMessage`)
    // again. The retired runtime's consensus-message root was removed with that runtime; the
    // core's message derives only `norito::NoritoSchema` (its frames carry
    // `norito::schema::identity::frame_hash::<WireMessage>()`), not `iroha_schema::IntoSchema`,
    // so it must gain a compiled schema description (or its Norito schema identity must be bound
    // into this preimage) before this identity covers consensus messages.
    <SignedBlock as IntoSchema>::update_schema_map(&mut schema);
    schema
}

/// Deterministic 32-byte identity of the compiled block wire plus the IVM ABI.
///
/// Pass the IVM ABI hash of the policy this binary executes
/// (`ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1)`).
#[must_use]
pub fn wire_schema_hash(ivm_abi_hash: [u8; 32]) -> [u8; 32] {
    wire_schema_hash_of(&covered_wire_schema(), ivm_abi_hash)
}

/// Hash an explicit compiled schema together with an IVM ABI hash.
///
/// Every type reference is rendered through its schema identifier, and the
/// rendered entries are ordered by identifier and then by their complete
/// rendering. Schema identifiers are not guaranteed unique (derived ids are bare
/// type names), so the tie-break keeps the order, and the result, independent of
/// process-local `TypeId` values and map iteration order even when two entries
/// share an identifier.
#[must_use]
pub fn wire_schema_hash_of(schema: &MetaMap, ivm_abi_hash: [u8; 32]) -> [u8; 32] {
    let names: BTreeMap<core::any::TypeId, &str> = schema
        .iter()
        .map(|(type_id, entry)| (*type_id, entry.type_id.as_str()))
        .collect();
    hash_rendered_entries(
        schema
            .iter()
            .map(|(_, entry)| render_entry(&names, entry))
            .collect(),
        ivm_abi_hash,
    )
}

/// One entry's identifier and its canonical rendering (identifier, name, metadata).
fn render_entry<'a>(
    names: &BTreeMap<core::any::TypeId, &str>,
    entry: &'a iroha_schema::MetaMapEntry,
) -> (&'a str, Vec<u8>) {
    let mut rendered = CanonicalPreimage::default();
    rendered.text(&entry.type_id);
    rendered.text(&entry.type_name);
    render_metadata(&mut rendered, names, &entry.metadata);
    (entry.type_id.as_str(), rendered.0)
}

/// Order rendered entries by `(identifier, rendering)` and hash them after the domain and ABI.
fn hash_rendered_entries(mut entries: Vec<(&str, Vec<u8>)>, ivm_abi_hash: [u8; 32]) -> [u8; 32] {
    entries.sort_unstable();
    let mut preimage = CanonicalPreimage::default();
    preimage.bytes(WIRE_SCHEMA_HASH_DOMAIN_V1);
    preimage.bytes(&ivm_abi_hash);
    preimage.count(entries.len());
    for (_, rendered) in entries {
        preimage.0.extend_from_slice(&rendered);
    }
    iroha_crypto::Hash::new(preimage.0).into()
}

/// Length-prefixed canonical byte stream; every item is self-delimiting.
#[derive(Default)]
struct CanonicalPreimage(Vec<u8>);

impl CanonicalPreimage {
    fn count(&mut self, value: usize) {
        self.0.extend_from_slice(&(value as u64).to_le_bytes());
    }
    fn bytes(&mut self, value: &[u8]) {
        self.count(value.len());
        self.0.extend_from_slice(value);
    }
    fn text(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }
    fn tag(&mut self, value: &str) {
        self.text(value);
    }
    fn reference(&mut self, names: &BTreeMap<core::any::TypeId, &str>, id: core::any::TypeId) {
        // `IntoSchema` registers every referenced type; an unregistered edge
        // still hashes deterministically but can never equal a closed schema.
        self.text(names.get(&id).copied().unwrap_or("\0unregistered"));
    }
}

fn render_metadata(
    out: &mut CanonicalPreimage,
    names: &BTreeMap<core::any::TypeId, &str>,
    metadata: &Metadata,
) {
    match metadata {
        Metadata::Struct(fields) => {
            out.tag("struct");
            out.count(fields.declarations.len());
            for field in &fields.declarations {
                out.text(&field.name);
                out.reference(names, field.ty);
            }
        }
        Metadata::Tuple(fields) => {
            out.tag("tuple");
            out.count(fields.types.len());
            for ty in &fields.types {
                out.reference(names, *ty);
            }
        }
        Metadata::Enum(variants) => {
            out.tag("enum");
            out.count(variants.variants.len());
            for variant in &variants.variants {
                out.text(&variant.tag);
                out.bytes(&variant.discriminant.to_le_bytes());
                match variant.ty {
                    Some(ty) => {
                        out.tag("some");
                        out.reference(names, ty);
                    }
                    None => out.tag("none"),
                }
            }
        }
        Metadata::Int(mode) => out.tag(match mode {
            iroha_schema::IntMode::FixedWidth => "int:fixed",
            iroha_schema::IntMode::Compact => "int:compact",
        }),
        Metadata::Float(mode) => out.tag(match mode {
            iroha_schema::FloatMode::Binary32 => "float:32",
            iroha_schema::FloatMode::Binary64 => "float:64",
        }),
        Metadata::String => out.tag("string"),
        Metadata::Bool => out.tag("bool"),
        Metadata::FixedPoint(fixed) => {
            out.tag("fixed");
            out.reference(names, fixed.base);
            out.bytes(&fixed.decimal_places.to_le_bytes());
        }
        Metadata::Array(array) => {
            out.tag("array");
            out.reference(names, array.ty);
            out.bytes(&array.len.to_le_bytes());
        }
        Metadata::Vec(vector) => {
            out.tag("vec");
            out.reference(names, vector.ty);
        }
        Metadata::Map(map) => {
            out.tag("map");
            out.reference(names, map.key);
            out.reference(names, map.value);
        }
        Metadata::Option(inner) => {
            out.tag("option");
            out.reference(names, *inner);
        }
        Metadata::Result(result) => {
            out.tag("result");
            out.reference(names, result.ok);
            out.reference(names, result.err);
        }
        Metadata::Bitmap(bitmap) => {
            out.tag("bitmap");
            out.reference(names, bitmap.repr);
            out.count(bitmap.masks.len());
            for mask in &bitmap.masks {
                out.text(&mask.name);
                out.bytes(&mask.mask.to_le_bytes());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use iroha_schema::{Declaration, NamedFieldsMeta};

    use super::*;
    use crate::{block::BlockHeader, sumeragi::SumeragiStatus};

    const ABI: [u8; 32] = [0xA1; 32];

    fn references(metadata: &Metadata) -> Vec<core::any::TypeId> {
        match metadata {
            Metadata::Struct(fields) => fields.declarations.iter().map(|field| field.ty).collect(),
            Metadata::Tuple(fields) => fields.types.clone(),
            Metadata::Enum(variants) => variants
                .variants
                .iter()
                .filter_map(|variant| variant.ty)
                .collect(),
            Metadata::FixedPoint(fixed) => vec![fixed.base],
            Metadata::Array(array) => vec![array.ty],
            Metadata::Vec(vector) => vec![vector.ty],
            Metadata::Map(map) => vec![map.key, map.value],
            Metadata::Option(inner) => vec![*inner],
            Metadata::Result(result) => vec![result.ok, result.err],
            Metadata::Bitmap(bitmap) => vec![bitmap.repr],
            Metadata::Int(_) | Metadata::Float(_) | Metadata::String | Metadata::Bool => Vec::new(),
        }
    }

    #[test]
    fn wire_schema_hash_is_stable_across_calls_and_schema_rebuilds() {
        let first = wire_schema_hash(ABI);
        assert_eq!(first, wire_schema_hash(ABI));
        assert_eq!(first, wire_schema_hash_of(&covered_wire_schema(), ABI));
        assert_ne!(first, [0; 32]);
    }

    #[test]
    fn covered_schema_is_closed_over_consensus_and_block_wire() {
        let schema = covered_wire_schema();
        assert!(schema.contains_key::<SignedBlock>());
        assert!(schema.contains_key::<BlockHeader>());
        let registered: std::collections::BTreeSet<_> = schema.iter().map(|(id, _)| *id).collect();
        for (_, entry) in schema.iter() {
            for reference in references(&entry.metadata) {
                assert!(
                    registered.contains(&reference),
                    "{} references an unregistered type",
                    entry.type_id
                );
            }
        }
    }

    /// Entries that share a schema identifier are ordered by their rendering, so the identity
    /// never depends on which `TypeId` each entry happens to have in this build.
    #[test]
    fn shared_identifiers_are_ordered_by_rendering_not_type_id() {
        let entry = |type_name: &str, metadata: Metadata| iroha_schema::MetaMapEntry {
            type_id: "Shared".to_owned(),
            type_name: type_name.to_owned(),
            metadata,
        };
        let names = BTreeMap::new();
        let first = entry("First", Metadata::Bool);
        let second = entry("Second", Metadata::String);
        let forward = hash_rendered_entries(
            vec![render_entry(&names, &first), render_entry(&names, &second)],
            ABI,
        );
        let reverse = hash_rendered_entries(
            vec![render_entry(&names, &second), render_entry(&names, &first)],
            ABI,
        );
        assert_eq!(forward, reverse);
        let other = hash_rendered_entries(
            vec![render_entry(&names, &first), render_entry(&names, &first)],
            ABI,
        );
        assert_ne!(forward, other, "the content of each entry is bound");
    }

    /// The covered schema has no two entries with the same identifier and different content, so
    /// nothing in the identity relies on the tie-break today.
    #[test]
    fn covered_schema_identifiers_have_one_rendering() {
        let schema = covered_wire_schema();
        let names: BTreeMap<core::any::TypeId, &str> = schema
            .iter()
            .map(|(type_id, entry)| (*type_id, entry.type_id.as_str()))
            .collect();
        let mut renderings: BTreeMap<&str, Vec<u8>> = BTreeMap::new();
        for (_, entry) in schema.iter() {
            let (id, rendered) = render_entry(&names, entry);
            if let Some(existing) = renderings.insert(id, rendered.clone()) {
                assert_eq!(
                    existing, rendered,
                    "two covered types share the schema identifier `{id}` with different content"
                );
            }
        }
    }

    #[test]
    fn wire_schema_hash_binds_the_ivm_abi_hash() {
        let mut other = ABI;
        other[31] ^= 1;
        assert_ne!(wire_schema_hash(ABI), wire_schema_hash(other));
    }

    #[test]
    fn wire_schema_hash_changes_when_a_covered_type_changes() {
        let baseline = covered_wire_schema();
        let expected = wire_schema_hash_of(&baseline, ABI);

        let Some(Metadata::Struct(fields)) = baseline.get::<BlockHeader>().cloned() else {
            panic!("BlockHeader is a named-field structure");
        };
        let mut widened = baseline.clone();
        let mut declarations = fields.declarations.clone();
        declarations.push(Declaration {
            name: "hypothetical_field".to_owned(),
            ty: core::any::TypeId::of::<u64>(),
        });
        widened.insert::<BlockHeader>(Metadata::Struct(NamedFieldsMeta { declarations }));
        assert_ne!(
            wire_schema_hash_of(&widened, ABI),
            expected,
            "adding a field changes the identity"
        );

        let mut renamed = baseline.clone();
        let mut declarations = fields.declarations.clone();
        declarations[0].name.push('_');
        renamed.insert::<BlockHeader>(Metadata::Struct(NamedFieldsMeta { declarations }));
        assert_ne!(
            wire_schema_hash_of(&renamed, ABI),
            expected,
            "renaming a field changes the identity"
        );

        let mut reordered = baseline.clone();
        let mut declarations = fields.declarations;
        declarations.swap(0, 1);
        reordered.insert::<BlockHeader>(Metadata::Struct(NamedFieldsMeta { declarations }));
        assert_ne!(
            wire_schema_hash_of(&reordered, ABI),
            expected,
            "reordering fields changes the identity"
        );

        let mut extended = baseline;
        <SumeragiStatus as IntoSchema>::update_schema_map(&mut extended);
        assert_ne!(
            wire_schema_hash_of(&extended, ABI),
            expected,
            "the identity covers exactly the supplied compiled schema"
        );
        assert_eq!(
            wire_schema_hash(ABI),
            expected,
            "the production roots do not include diagnostics-only types"
        );
    }
}
