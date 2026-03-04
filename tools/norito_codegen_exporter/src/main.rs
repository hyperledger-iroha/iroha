//! Export Norito metadata for the Android codegen documentation pipeline (AND3).
//!
//! The current implementation focuses on surfacing a deterministic manifest of
//! all built-in instructions plus placeholder builder metadata. Follow-ups will
//! extend this binary with richer doc extraction, Kotlin references, and live
//! Norito payload fixtures.

use std::{
    any::TypeId,
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use clap::Parser;
use iroha_data_model::{instruction_registry, isi::InstructionRegistry, prelude as dm};
use iroha_schema::{
    ArrayMeta, BitmapMeta, EnumMeta, IntoSchema, MapMeta, MetaMapEntry, Metadata, NamedFieldsMeta,
    ResultMeta, UnnamedFieldsMeta,
};
use norito::{
    derive::JsonDeserialize,
    json::{self, Map, Number, Value},
};
use sha2::{Digest, Sha256};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

macro_rules! for_each_instruction_type {
    ($macro:ident) => {
        $macro!(iroha_data_model::isi::RegisterPeerWithPop);
        $macro!(iroha_data_model::isi::Register<dm::Domain>);
        $macro!(iroha_data_model::isi::Register<dm::Account>);
        $macro!(iroha_data_model::isi::Register<dm::AssetDefinition>);
        $macro!(iroha_data_model::isi::Register<dm::Nft>);
        $macro!(iroha_data_model::isi::Register<dm::Role>);
        $macro!(iroha_data_model::isi::Register<dm::Trigger>);
        $macro!(iroha_data_model::isi::RegisterBox);
        $macro!(iroha_data_model::isi::Unregister<dm::Peer>);
        $macro!(iroha_data_model::isi::Unregister<dm::Domain>);
        $macro!(iroha_data_model::isi::Unregister<dm::Account>);
        $macro!(iroha_data_model::isi::Unregister<dm::AssetDefinition>);
        $macro!(iroha_data_model::isi::Unregister<dm::Nft>);
        $macro!(iroha_data_model::isi::Unregister<dm::Role>);
        $macro!(iroha_data_model::isi::Unregister<dm::Trigger>);
        $macro!(iroha_data_model::isi::UnregisterBox);
        $macro!(iroha_data_model::isi::Mint<dm::Numeric, dm::Asset>);
        $macro!(iroha_data_model::isi::Mint<u32, dm::Trigger>);
        $macro!(iroha_data_model::isi::MintBox);
        $macro!(iroha_data_model::isi::Burn<dm::Numeric, dm::Asset>);
        $macro!(iroha_data_model::isi::Burn<u32, dm::Trigger>);
        $macro!(iroha_data_model::isi::BurnBox);
        $macro!(iroha_data_model::isi::Transfer<dm::Account, dm::DomainId, dm::Account>);
        $macro!(iroha_data_model::isi::Transfer<dm::Account, dm::AssetDefinitionId, dm::Account>);
        $macro!(iroha_data_model::isi::Transfer<dm::Asset, dm::Numeric, dm::Account>);
        $macro!(iroha_data_model::isi::Transfer<dm::Account, dm::NftId, dm::Account>);
        $macro!(iroha_data_model::isi::transfer::TransferAssetBatch);
        $macro!(iroha_data_model::isi::TransferBox);
        $macro!(iroha_data_model::isi::repo::RepoInstructionBox);
        $macro!(iroha_data_model::isi::repo::RepoIsi);
        $macro!(iroha_data_model::isi::repo::ReverseRepoIsi);
        $macro!(iroha_data_model::isi::settlement::SettlementInstructionBox);
        $macro!(iroha_data_model::isi::settlement::DvpIsi);
        $macro!(iroha_data_model::isi::settlement::PvpIsi);
        $macro!(iroha_data_model::isi::SetParameter);
        $macro!(iroha_data_model::isi::SetKeyValue<dm::Domain>);
        $macro!(iroha_data_model::isi::SetKeyValue<dm::Account>);
        $macro!(iroha_data_model::isi::SetKeyValue<dm::AssetDefinition>);
        $macro!(iroha_data_model::isi::SetKeyValue<dm::Nft>);
        $macro!(iroha_data_model::isi::SetKeyValue<dm::Trigger>);
        $macro!(iroha_data_model::isi::SetKeyValueBox);
        $macro!(iroha_data_model::isi::SetAssetKeyValue);
        $macro!(iroha_data_model::isi::RemoveKeyValue<dm::Domain>);
        $macro!(iroha_data_model::isi::RemoveKeyValue<dm::Account>);
        $macro!(iroha_data_model::isi::RemoveKeyValue<dm::AssetDefinition>);
        $macro!(iroha_data_model::isi::RemoveKeyValue<dm::Nft>);
        $macro!(iroha_data_model::isi::RemoveKeyValue<dm::Trigger>);
        $macro!(iroha_data_model::isi::RemoveKeyValueBox);
        $macro!(iroha_data_model::isi::RemoveAssetKeyValue);
        $macro!(iroha_data_model::isi::Grant<dm::Permission, dm::Account>);
        $macro!(iroha_data_model::isi::Grant<dm::RoleId, dm::Account>);
        $macro!(iroha_data_model::isi::Grant<dm::Permission, dm::Role>);
        $macro!(iroha_data_model::isi::GrantBox);
        $macro!(iroha_data_model::isi::Revoke<dm::Permission, dm::Account>);
        $macro!(iroha_data_model::isi::Revoke<dm::RoleId, dm::Account>);
        $macro!(iroha_data_model::isi::Revoke<dm::Permission, dm::Role>);
        $macro!(iroha_data_model::isi::RevokeBox);
        $macro!(iroha_data_model::isi::ExecuteTrigger);
        $macro!(iroha_data_model::isi::Upgrade);
        $macro!(iroha_data_model::isi::Log);
        $macro!(iroha_data_model::isi::CustomInstruction);
        $macro!(iroha_data_model::isi::verifying_keys::RegisterVerifyingKey);
        $macro!(iroha_data_model::isi::verifying_keys::UpdateVerifyingKey);
        $macro!(iroha_data_model::isi::sorafs::RegisterPinManifest);
        $macro!(iroha_data_model::isi::sorafs::ApprovePinManifest);
        $macro!(iroha_data_model::isi::sorafs::RetirePinManifest);
        $macro!(iroha_data_model::isi::sorafs::BindManifestAlias);
        $macro!(iroha_data_model::isi::sorafs::RegisterCapacityDeclaration);
        $macro!(iroha_data_model::isi::sorafs::RecordCapacityTelemetry);
        $macro!(iroha_data_model::isi::sorafs::RegisterCapacityDispute);
        $macro!(iroha_data_model::isi::sorafs::IssueReplicationOrder);
        $macro!(iroha_data_model::isi::sorafs::CompleteReplicationOrder);
        $macro!(iroha_data_model::isi::smart_contract_code::RegisterSmartContractCode);
        $macro!(iroha_data_model::isi::smart_contract_code::DeactivateContractInstance);
        $macro!(iroha_data_model::isi::smart_contract_code::ActivateContractInstance);
        $macro!(iroha_data_model::isi::smart_contract_code::RegisterSmartContractBytes);
        $macro!(iroha_data_model::isi::smart_contract_code::RemoveSmartContractBytes);
        $macro!(iroha_data_model::isi::zk::VerifyProof);
        $macro!(iroha_data_model::isi::kaigi::CreateKaigi);
        $macro!(iroha_data_model::isi::kaigi::JoinKaigi);
        $macro!(iroha_data_model::isi::kaigi::LeaveKaigi);
        $macro!(iroha_data_model::isi::kaigi::EndKaigi);
        $macro!(iroha_data_model::isi::kaigi::RecordKaigiUsage);
        $macro!(iroha_data_model::isi::kaigi::SetKaigiRelayManifest);
        $macro!(iroha_data_model::isi::kaigi::RegisterKaigiRelay);
        $macro!(iroha_data_model::isi::zk::RegisterZkAsset);
        $macro!(iroha_data_model::isi::zk::ScheduleConfidentialPolicyTransition);
        $macro!(iroha_data_model::isi::zk::CancelConfidentialPolicyTransition);
        $macro!(iroha_data_model::isi::zk::Shield);
        $macro!(iroha_data_model::isi::zk::ZkTransfer);
        $macro!(iroha_data_model::isi::zk::Unshield);
        $macro!(iroha_data_model::isi::zk::CreateElection);
        $macro!(iroha_data_model::isi::zk::SubmitBallot);
        $macro!(iroha_data_model::isi::zk::FinalizeElection);
        $macro!(iroha_data_model::isi::governance::ProposeDeployContract);
        $macro!(iroha_data_model::isi::governance::CastZkBallot);
        $macro!(iroha_data_model::isi::governance::CastPlainBallot);
        $macro!(iroha_data_model::isi::governance::EnactReferendum);
        $macro!(iroha_data_model::isi::governance::FinalizeReferendum);
        $macro!(iroha_data_model::isi::governance::PersistCouncilForEpoch);
        $macro!(iroha_data_model::isi::runtime_upgrade::ProposeRuntimeUpgrade);
        $macro!(iroha_data_model::isi::runtime_upgrade::ActivateRuntimeUpgrade);
        $macro!(iroha_data_model::isi::runtime_upgrade::CancelRuntimeUpgrade);
    };
}

/// Command line arguments accepted by the exporter.
#[derive(Parser, Debug)]
#[command(
    name = "norito_codegen_exporter",
    about = "Emit manifests consumed by the Android Norito doc toolchain"
)]
struct Args {
    /// Output directory that will host the manifest and builder metadata.
    #[arg(long)]
    out: PathBuf,
    /// Default Kotlin/Java package recorded in the builder manifest.
    #[arg(long, default_value = "iroha.android.codegen")]
    package: String,
    /// Stability annotation (e.g., experimental/beta/ga) used by builders.
    #[arg(long, default_value = "experimental")]
    stability: String,
    /// Optional feature gates recorded for every builder entry.
    #[arg(long = "feature", action = clap::ArgAction::Append)]
    features: Vec<String>,
    /// Optional rustdoc JSON file used to fill documentation fields.
    #[arg(long)]
    doc_json: Option<PathBuf>,
    /// Pretty-print JSON output for easier diffing.
    #[arg(long)]
    pretty: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    run(args)
}

fn run(args: Args) -> Result<()> {
    fs::create_dir_all(&args.out)
        .with_context(|| format!("create output directory {}", args.out.display()))?;

    let registry = instruction_registry::default();
    let doc_index = if let Some(doc_path) = &args.doc_json {
        Some(
            load_doc_index(doc_path)
                .with_context(|| format!("load rustdoc JSON from {}", doc_path.display()))?,
        )
    } else {
        None
    };
    let specs = gather_instruction_specs(&registry, doc_index.as_ref());
    let timestamp = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .context("format timestamp")?;

    let manifest = build_manifest(&specs, &timestamp);
    let manifest_path = args.out.join("instruction_manifest.json");
    write_json_with_digest(&manifest_path, &manifest, args.pretty)?;

    let builder_index = build_builder_index(
        &specs,
        &timestamp,
        &args.package,
        &args.stability,
        &args.features,
    );
    let builder_index_path = args.out.join("builder_index.json");
    write_json_with_digest(&builder_index_path, &builder_index, args.pretty)?;

    let examples_dir = args.out.join("instruction_examples");
    write_examples(&examples_dir, &specs, &timestamp, args.pretty)?;

    Ok(())
}

type TypeIndex = HashMap<TypeId, MetaMapEntry>;
type DocIndex = HashMap<String, String>;

#[derive(JsonDeserialize)]
struct RustdocCrate {
    #[norito(default)]
    paths: HashMap<String, RustdocPath>,
    #[norito(default)]
    index: HashMap<String, RustdocItem>,
}

#[derive(JsonDeserialize)]
struct RustdocPath {
    #[allow(dead_code)]
    #[norito(default)]
    crate_id: u32,
    path: Vec<String>,
    #[allow(dead_code)]
    #[norito(default)]
    kind: String,
}

#[derive(JsonDeserialize)]
struct RustdocItem {
    #[norito(default)]
    docs: Option<String>,
}

#[derive(Debug, Clone)]
struct InstructionSpec {
    discriminant: String,
    type_name: &'static str,
    schema_hash_hex: String,
    layout: Value,
    documentation: String,
}

impl InstructionSpec {
    fn new<T>(registry: &InstructionRegistry, docs: Option<&DocIndex>) -> Self
    where
        T: IntoSchema + 'static,
    {
        let type_name = std::any::type_name::<T>();
        let wire_id = registry.wire_id(type_name).unwrap_or(type_name).to_owned();
        let layout = layout_for::<T>();
        let schema_hash_hex = hex_lower(&norito::core::type_name_schema_hash::<T>());
        let documentation = docs
            .and_then(|map| map.get(base_type_name(type_name)))
            .map(|doc| doc.trim())
            .filter(|doc| !doc.is_empty())
            .map(str::to_owned)
            .or_else(|| layout_documentation(&layout))
            .unwrap_or_else(|| {
                format!("Schema summary unavailable for `{type_name}`; consult the layout table.")
            });
        Self {
            discriminant: wire_id,
            type_name,
            schema_hash_hex,
            layout,
            documentation,
        }
    }

    fn manifest_value(&self) -> Value {
        object([
            (
                "discriminant".into(),
                Value::String(self.discriminant.clone()),
            ),
            ("type_name".into(), Value::String(self.type_name.to_owned())),
            (
                "schema_hash".into(),
                Value::String(self.schema_hash_hex.clone()),
            ),
            ("layout".into(), self.layout.clone()),
            (
                "documentation".into(),
                Value::String(self.documentation.clone()),
            ),
        ])
    }

    fn builder_value(&self, package: &str, stability: &str, features: &[String]) -> Value {
        let builder_name = self.builder_name();
        object([
            (
                "discriminant".into(),
                Value::String(self.discriminant.clone()),
            ),
            ("builder_name".into(), Value::String(builder_name.clone())),
            ("type_name".into(), Value::String(self.type_name.to_owned())),
            ("package".into(), Value::String(package.to_owned())),
            ("stability".into(), Value::String(stability.to_owned())),
            (
                "feature_gates".into(),
                Value::Array(features.iter().cloned().map(Value::String).collect()),
            ),
            (
                "notes".into(),
                Value::String(builder_notes(package, &builder_name, &self.layout)),
            ),
        ])
    }

    fn example_value(&self, timestamp: &str) -> Value {
        object([
            (
                "discriminant".into(),
                Value::String(self.discriminant.clone()),
            ),
            ("type_name".into(), Value::String(self.type_name.to_owned())),
            ("generated_at".into(), Value::String(timestamp.to_owned())),
            ("layout".into(), self.layout.clone()),
        ])
    }

    fn builder_name(&self) -> String {
        format!("{}Builder", sanitize_identifier(self.type_name))
    }
}

fn build_manifest(specs: &[InstructionSpec], timestamp: &str) -> Value {
    object([
        ("version".into(), Value::Number(Number::U64(1))),
        ("generated_at".into(), Value::String(timestamp.to_owned())),
        (
            "instructions".into(),
            Value::Array(specs.iter().map(InstructionSpec::manifest_value).collect()),
        ),
    ])
}

fn build_builder_index(
    specs: &[InstructionSpec],
    timestamp: &str,
    package: &str,
    stability: &str,
    features: &[String],
) -> Value {
    object([
        ("generated_at".into(), Value::String(timestamp.to_owned())),
        ("package".into(), Value::String(package.to_owned())),
        ("stability".into(), Value::String(stability.to_owned())),
        (
            "builders".into(),
            Value::Array(
                specs
                    .iter()
                    .map(|spec| spec.builder_value(package, stability, features))
                    .collect(),
            ),
        ),
    ])
}

fn builder_notes(package: &str, builder_name: &str, layout: &Value) -> String {
    let fqcn = if package.is_empty() {
        builder_name.to_owned()
    } else {
        format!("{package}.{builder_name}")
    };
    match describe_layout(layout) {
        Some(summary) => format!("Builder `{fqcn}` — {summary}"),
        None => format!("Builder `{fqcn}`"),
    }
}

fn layout_documentation(layout: &Value) -> Option<String> {
    describe_layout(layout).map(|summary| format!("Schema summary: {summary}."))
}

fn describe_layout(layout: &Value) -> Option<String> {
    let kind = layout.get("kind")?.as_str()?;
    match kind {
        "struct" => summarize_fields(layout, "struct"),
        "tuple" => summarize_fields(layout, "tuple"),
        "enum" => summarize_enum(layout),
        "array" => {
            let ty = layout.get("type").and_then(Value::as_str)?;
            let len = layout.get("len").and_then(Value::as_u64);
            let descr = len
                .map(|len| format!("array[{len}] of {ty}"))
                .unwrap_or_else(|| format!("array of {ty}"));
            Some(descr)
        }
        "vec" => layout
            .get("type")
            .and_then(Value::as_str)
            .map(|ty| format!("vec<{ty}> payload")),
        "map" => {
            let key = layout.get("key").and_then(Value::as_str)?;
            let value = layout.get("value").and_then(Value::as_str)?;
            Some(format!("map<{key}, {value}> payload"))
        }
        "option" => layout
            .get("type")
            .and_then(Value::as_str)
            .map(|ty| format!("option<{ty}> payload")),
        "result" => {
            let ok = layout.get("ok").and_then(Value::as_str)?;
            let err = layout.get("err").and_then(Value::as_str)?;
            Some(format!("result<ok={ok}, err={err}> payload"))
        }
        "bitmap" => summarize_bitmap(layout),
        "fixed_point" => {
            let base = layout.get("base").and_then(Value::as_str)?;
            let decimals = layout.get("decimal_places").and_then(Value::as_u64)?;
            Some(format!("fixed_point<{base}, {decimals}dp> payload"))
        }
        "int" => layout
            .get("mode")
            .and_then(Value::as_str)
            .map(|mode| format!("int ({mode}) payload")),
        "string" => Some("string payload".to_owned()),
        "bool" => Some("bool payload".to_owned()),
        other => Some(format!("layout kind `{other}`")),
    }
}

fn summarize_fields(layout: &Value, label: &str) -> Option<String> {
    let fields = layout.get("fields")?.as_array()?;
    if fields.is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    for field in fields {
        let name = field
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let ty = field.get("type").and_then(Value::as_str).unwrap_or("?");
        if name.is_empty() {
            parts.push(ty.to_owned());
        } else {
            parts.push(format!("{name}: {ty}"));
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(format!("{label} fields: {}", parts.join(", ")))
    }
}

fn summarize_enum(layout: &Value) -> Option<String> {
    let variants = layout.get("variants")?.as_array()?;
    if variants.is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    for variant in variants {
        let tag = variant
            .get("tag")
            .and_then(Value::as_str)
            .unwrap_or("variant");
        let payload = variant.get("payload");
        match payload {
            Some(Value::String(payload)) if !payload.is_empty() => {
                parts.push(format!("{tag} ({payload})"));
            }
            _ => parts.push(tag.to_owned()),
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(format!("enum variants: {}", parts.join(", ")))
    }
}

fn summarize_bitmap(layout: &Value) -> Option<String> {
    let repr = layout.get("repr").and_then(Value::as_str)?;
    let masks = layout.get("masks")?.as_array()?;
    let mut mask_labels = Vec::new();
    for mask in masks {
        let name = mask.get("name").and_then(Value::as_str).unwrap_or("mask");
        let value = mask.get("mask").and_then(Value::as_u64);
        if let Some(value) = value {
            mask_labels.push(format!("{name}=0x{value:x}"));
        } else {
            mask_labels.push(name.to_owned());
        }
    }
    Some(if mask_labels.is_empty() {
        format!("bitmap<{repr}>")
    } else {
        format!("bitmap<{repr}> masks: {}", mask_labels.join(", "))
    })
}

fn load_doc_index(path: &Path) -> Result<DocIndex> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let parsed: RustdocCrate =
        json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?;
    let RustdocCrate { paths, index } = parsed;
    let mut docs = HashMap::new();
    for (item_id, rustdoc_path) in paths {
        let segments = rustdoc_path.path;
        if segments.is_empty() {
            continue;
        }
        let fq = segments.join("::");
        if !fq.starts_with("iroha_data_model::") {
            continue;
        }
        let doc_value = index
            .get(&item_id)
            .and_then(|item| item.docs.as_deref())
            .map(str::trim)
            .filter(|doc| !doc.is_empty());
        if let Some(doc) = doc_value {
            docs.entry(fq).or_insert_with(|| doc.to_owned());
        }
    }
    Ok(docs)
}

fn gather_instruction_specs(
    registry: &InstructionRegistry,
    docs: Option<&DocIndex>,
) -> Vec<InstructionSpec> {
    let mut specs = Vec::new();
    macro_rules! push_spec {
        ($ty:ty) => {
            specs.push(InstructionSpec::new::<$ty>(registry, docs));
        };
    }
    for_each_instruction_type!(push_spec);
    specs.sort_by(|a, b| a.discriminant.cmp(&b.discriminant));
    specs
}

fn layout_for<T: IntoSchema + 'static>() -> Value {
    let schema = T::schema();
    let type_index: TypeIndex = schema.into_iter().collect();
    let type_id = TypeId::of::<T>();
    let metadata = type_index
        .get(&type_id)
        .map(|entry| entry.metadata.clone())
        .unwrap_or_else(|| {
            panic!(
                "missing schema metadata for `{}`",
                std::any::type_name::<T>()
            )
        });
    metadata_to_value(&metadata, &type_index)
}

fn base_type_name(full: &str) -> &str {
    full.split_once('<').map(|(head, _)| head).unwrap_or(full)
}

fn metadata_to_value(meta: &Metadata, index: &TypeIndex) -> Value {
    match meta {
        Metadata::Struct(named) => struct_metadata(named, index),
        Metadata::Tuple(tuple) => tuple_metadata(tuple, index),
        Metadata::Enum(enum_meta) => enum_metadata(enum_meta, index),
        Metadata::Int(mode) => object([
            ("kind".into(), Value::String("int".to_owned())),
            ("mode".into(), Value::String(format!("{mode:?}"))),
        ]),
        Metadata::String => object([("kind".into(), Value::String("string".to_owned()))]),
        Metadata::Bool => object([("kind".into(), Value::String("bool".to_owned()))]),
        Metadata::FixedPoint(meta) => object([
            ("kind".into(), Value::String("fixed_point".to_owned())),
            (
                "base".into(),
                Value::String(lookup_type_name(index, meta.base)),
            ),
            (
                "decimal_places".into(),
                Value::Number(Number::U64(meta.decimal_places as u64)),
            ),
        ]),
        Metadata::Array(meta) => array_metadata(meta, index),
        Metadata::Vec(meta) => object([
            ("kind".into(), Value::String("vec".to_owned())),
            (
                "type".into(),
                Value::String(lookup_type_name(index, meta.ty)),
            ),
        ]),
        Metadata::Map(meta) => map_metadata(meta, index),
        Metadata::Option(inner) => object([
            ("kind".into(), Value::String("option".to_owned())),
            (
                "type".into(),
                Value::String(lookup_type_name(index, *inner)),
            ),
        ]),
        Metadata::Result(meta) => result_metadata(meta, index),
        Metadata::Bitmap(meta) => bitmap_metadata(meta, index),
    }
}

fn struct_metadata(named: &NamedFieldsMeta, index: &TypeIndex) -> Value {
    object([
        ("kind".into(), Value::String("struct".to_owned())),
        (
            "fields".into(),
            Value::Array(
                named
                    .declarations
                    .iter()
                    .map(|decl| field_value(Some(decl.name.clone()), decl.ty, index))
                    .collect(),
            ),
        ),
    ])
}

fn tuple_metadata(tuple: &UnnamedFieldsMeta, index: &TypeIndex) -> Value {
    object([
        ("kind".into(), Value::String("tuple".to_owned())),
        (
            "fields".into(),
            Value::Array(
                tuple
                    .types
                    .iter()
                    .enumerate()
                    .map(|(idx, ty)| field_value(Some(format!("_{idx}")), *ty, index))
                    .collect(),
            ),
        ),
    ])
}

fn enum_metadata(enum_meta: &EnumMeta, index: &TypeIndex) -> Value {
    object([
        ("kind".into(), Value::String("enum".to_owned())),
        (
            "variants".into(),
            Value::Array(
                enum_meta
                    .variants
                    .iter()
                    .map(|variant| {
                        let payload = variant
                            .ty
                            .map(|ty| Value::String(lookup_type_name(index, ty)))
                            .unwrap_or(Value::Null);
                        object([
                            ("tag".into(), Value::String(variant.tag.clone())),
                            (
                                "discriminant".into(),
                                Value::Number(Number::U64(variant.discriminant as u64)),
                            ),
                            ("payload".into(), payload),
                        ])
                    })
                    .collect(),
            ),
        ),
    ])
}

fn array_metadata(meta: &ArrayMeta, index: &TypeIndex) -> Value {
    object([
        ("kind".into(), Value::String("array".to_owned())),
        (
            "type".into(),
            Value::String(lookup_type_name(index, meta.ty)),
        ),
        ("len".into(), Value::Number(Number::U64(meta.len as u64))),
    ])
}

fn map_metadata(meta: &MapMeta, index: &TypeIndex) -> Value {
    object([
        ("kind".into(), Value::String("map".to_owned())),
        (
            "key".into(),
            Value::String(lookup_type_name(index, meta.key)),
        ),
        (
            "value".into(),
            Value::String(lookup_type_name(index, meta.value)),
        ),
    ])
}

fn result_metadata(meta: &ResultMeta, index: &TypeIndex) -> Value {
    object([
        ("kind".into(), Value::String("result".to_owned())),
        ("ok".into(), Value::String(lookup_type_name(index, meta.ok))),
        (
            "err".into(),
            Value::String(lookup_type_name(index, meta.err)),
        ),
    ])
}

fn bitmap_metadata(meta: &BitmapMeta, index: &TypeIndex) -> Value {
    object([
        ("kind".into(), Value::String("bitmap".to_owned())),
        (
            "repr".into(),
            Value::String(lookup_type_name(index, meta.repr)),
        ),
        (
            "masks".into(),
            Value::Array(
                meta.masks
                    .iter()
                    .map(|mask| {
                        object([
                            ("name".into(), Value::String(mask.name.clone())),
                            ("mask".into(), Value::Number(Number::U64(mask.mask))),
                        ])
                    })
                    .collect(),
            ),
        ),
    ])
}

fn field_value(name: Option<String>, ty: TypeId, index: &TypeIndex) -> Value {
    object([
        (
            "name".into(),
            name.map(Value::String).unwrap_or(Value::Null),
        ),
        ("type".into(), Value::String(lookup_type_name(index, ty))),
    ])
}

fn lookup_type_name(index: &TypeIndex, ty: TypeId) -> String {
    index
        .get(&ty)
        .map(|entry| entry.type_name.clone())
        .unwrap_or_else(|| format!("{ty:?}"))
}

fn object(pairs: impl IntoIterator<Item = (String, Value)>) -> Value {
    let mut map = Map::new();
    for (key, value) in pairs {
        map.insert(key, value);
    }
    Value::Object(map)
}

fn sanitize_identifier(input: &str) -> String {
    let mut out = String::new();
    for c in input.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    while out.starts_with('_') {
        out.remove(0);
    }
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    if out.is_empty() {
        return "Instruction".to_owned();
    }
    if out
        .chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false)
    {
        out.insert(0, '_');
    }
    out
}

fn write_json_with_digest(path: &Path, value: &Value, pretty: bool) -> Result<()> {
    let json = if pretty {
        json::to_json_pretty(value)?
    } else {
        json::to_json(value)?
    };
    fs::write(path, json.as_bytes()).with_context(|| format!("write {}", path.display()))?;
    write_digest_file(path, json.as_bytes())
}

fn write_digest_file(path: &Path, data: &[u8]) -> Result<()> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let digest = hasher.finalize();
    let hex = hex_lower(&digest);
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "artifact".to_owned());
    let digest_entry = format!("{hex}  {file_name}\n");
    let digest_path = path.with_file_name(format!("{file_name}.sha256"));
    fs::write(&digest_path, digest_entry)
        .with_context(|| format!("write {}", digest_path.display()))
}

fn write_examples(
    dir: &Path,
    specs: &[InstructionSpec],
    timestamp: &str,
    pretty: bool,
) -> Result<()> {
    fs::create_dir_all(dir).with_context(|| format!("create examples dir {}", dir.display()))?;
    for spec in specs {
        let path = dir.join(format!("{}.json", spec.discriminant));
        let value = spec.example_value(timestamp);
        let json = if pretty {
            json::to_json_pretty(&value)?
        } else {
            json::to_json(&value)?
        };
        fs::write(&path, json.as_bytes()).with_context(|| format!("write {}", path.display()))?;
    }
    Ok(())
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut out, "{byte:02x}").expect("write! to String is infallible");
    }
    out
}

#[cfg(test)]
mod tests {
    use std::fs;

    use iroha_schema::IntoSchema;
    use tempfile::NamedTempFile;

    use super::*;

    #[derive(IntoSchema)]
    struct Sample {
        value: u32,
    }

    #[allow(dead_code)]
    #[derive(IntoSchema)]
    struct LayoutStruct {
        foo: u32,
        bar: String,
    }

    #[allow(dead_code)]
    #[derive(IntoSchema)]
    enum LayoutEnum {
        Unit,
        Payload(u32),
    }

    #[test]
    fn sanitize_identifier_collapses_noise() {
        assert_eq!(
            sanitize_identifier("iroha::register::Register<iroha::Domain>"),
            "iroha_register_Register_iroha_Domain"
        );
        assert_eq!(sanitize_identifier("123start"), "_123start");
        assert_eq!(sanitize_identifier(""), "Instruction");
    }

    #[test]
    fn layout_exposes_struct_fields() {
        let sample = Sample { value: 42 };
        assert_eq!(sample.value, 42);
        let layout = layout_for::<Sample>();
        let Value::Object(map) = layout else {
            panic!("expected struct metadata object");
        };
        assert_eq!(map.get("kind").and_then(Value::as_str), Some("struct"));
        let Some(Value::Array(fields)) = map.get("fields") else {
            panic!("missing fields array");
        };
        assert!(
            fields.iter().any(|field| match field {
                Value::Object(field_map) =>
                    field_map.get("name").and_then(Value::as_str) == Some("value")
                        && field_map.get("type").and_then(Value::as_str) == Some("u32"),
                _ => false,
            }),
            "expected `value: u32` field in layout"
        );
    }

    #[test]
    fn describe_layout_produces_struct_summary() {
        let layout = layout_for::<LayoutStruct>();
        let summary = describe_layout(&layout).expect("summary");
        assert!(
            summary.contains("foo") && summary.contains("bar"),
            "summary should list struct fields: {summary}"
        );
    }

    #[test]
    fn describe_layout_produces_enum_summary() {
        let layout = layout_for::<LayoutEnum>();
        let summary = describe_layout(&layout).expect("summary");
        assert!(
            summary.contains("Unit") && summary.contains("Payload"),
            "summary should list enum variants: {summary}"
        );
    }

    #[test]
    fn builder_notes_include_fqcn() {
        let layout = layout_for::<LayoutStruct>();
        let notes = builder_notes("iroha.android.codegen", "FooBuilder", &layout);
        assert!(
            notes.contains("iroha.android.codegen.FooBuilder"),
            "expected fully qualified builder name in notes: {notes}"
        );
        assert!(
            notes.contains("foo") && notes.contains("bar"),
            "expected struct fields reference in notes: {notes}"
        );
    }

    #[test]
    fn layout_documentation_falls_back_to_summary() {
        let layout = layout_for::<LayoutStruct>();
        let doc = layout_documentation(&layout).expect("fallback documentation");
        assert!(
            doc.contains("Schema summary") && doc.contains("foo") && doc.contains("bar"),
            "fallback doc should reference struct fields: {doc}"
        );
    }

    #[test]
    fn load_doc_index_prefers_data_model_entries() {
        let tmp = NamedTempFile::new().expect("temp file");
        let rustdoc = r#"
        {
            "paths": {
                "1": { "crate_id": 0, "path": ["iroha_data_model", "example", "Thing"], "kind": "struct" },
                "2": { "crate_id": 0, "path": ["other", "IgnoreMe"], "kind": "struct" }
            },
            "index": {
                "1": { "docs": " Example docs " },
                "2": { "docs": "Other docs" }
            }
        }"#;
        fs::write(tmp.path(), rustdoc).expect("write rustdoc json");

        let index = load_doc_index(tmp.path()).expect("load docs");
        assert_eq!(
            index.get("iroha_data_model::example::Thing"),
            Some(&"Example docs".to_string())
        );
        assert!(!index.contains_key("other::IgnoreMe"));
    }
}
