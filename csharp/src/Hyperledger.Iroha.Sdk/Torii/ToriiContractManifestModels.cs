using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization;

namespace Hyperledger.Iroha.Torii;

/// <summary>Branded Kotodama V1 entrypoint categories.</summary>
public enum ToriiContractEntrypointKind
{
    Kotoage,
    View,
    Hajimari,
    Kaizen,
}

/// <summary>Scalar and pointer leaves supported by an exact V1 boundary schema.</summary>
public enum ToriiEntrypointValueKindV1
{
    Int,
    Decimal,
    Quantity,
    Bool,
    String,
    Json,
    Name,
    AccountId,
    AssetDefinitionId,
    AssetId,
    DomainId,
    NftId,
    DataSpaceId,
    Blob,
}

/// <summary>Flat preorder node categories supported by an exact V1 boundary schema.</summary>
public enum ToriiEntrypointValueTypeNodeKindV1
{
    Struct,
    Tuple,
    Option,
    Result,
    List,
    Leaf,
}

/// <summary>Named product metadata for a flat boundary-schema node.</summary>
public sealed record class ToriiEntrypointStructTypeNodeV1
{
    private string[] fields = Array.Empty<string>();

    public string Name { get; init; } = string.Empty;

    public IReadOnlyList<string> Fields
    {
        get => ToriiListSnapshots.CopyRequired(fields);
        init => fields = ToriiListSnapshots.CopyRequired(value);
    }
}

/// <summary>Bounded-list metadata; its one element subtree follows on the flat node tape.</summary>
public sealed record class ToriiEntrypointListTypeNodeV1
{
    public byte Capacity { get; init; }
}

/// <summary>One validated preorder node in an exact Kotodama V1 boundary schema.</summary>
public sealed record class ToriiEntrypointValueTypeNodeV1
{
    public ToriiEntrypointValueTypeNodeKindV1 Kind { get; init; }

    public ToriiEntrypointStructTypeNodeV1? StructValue { get; init; }

    public ushort? TupleArity { get; init; }

    public ToriiEntrypointListTypeNodeV1? ListValue { get; init; }

    public ToriiEntrypointValueKindV1? LeafKind { get; init; }
}

/// <summary>Exact flat preorder value schema used at a Kotodama V1 public boundary.</summary>
public sealed record class ToriiEntrypointValueTypeV1
{
    private ToriiEntrypointValueTypeNodeV1[] nodes = Array.Empty<ToriiEntrypointValueTypeNodeV1>();

    public IReadOnlyList<ToriiEntrypointValueTypeNodeV1> Nodes
    {
        get => ToriiListSnapshots.CopyRequired(nodes);
        init => nodes = ToriiListSnapshots.CopyNonNullItems(value, nameof(Nodes))
            ?? Array.Empty<ToriiEntrypointValueTypeNodeV1>();
    }

    public int WordCount { get; init; }

    public string CanonicalTypeName { get; init; } = string.Empty;
}

/// <summary>One named field in a canonical V1 entrypoint argument record.</summary>
public sealed record class ToriiEntrypointArgumentFieldV1
{
    public string Name { get; init; } = string.Empty;

    public ToriiEntrypointValueTypeV1 ValueType { get; init; } = new();
}

/// <summary>Exact canonical V1 schema for one public entrypoint argument record.</summary>
public sealed record class ToriiEntrypointArgumentSchemaV1
{
    private ToriiEntrypointArgumentFieldV1[] fields = Array.Empty<ToriiEntrypointArgumentFieldV1>();

    public IReadOnlyList<ToriiEntrypointArgumentFieldV1> Fields
    {
        get => ToriiListSnapshots.CopyRequired(fields);
        init => fields = ToriiListSnapshots.CopyNonNullItems(value, nameof(Fields))
            ?? Array.Empty<ToriiEntrypointArgumentFieldV1>();
    }

    public int WordCount { get; init; }
}

/// <summary>One public parameter advertised by a Kotodama manifest.</summary>
public sealed record class ToriiContractEntrypointParameter
{
    public string Name { get; init; } = string.Empty;

    public string TypeName { get; init; } = string.Empty;
}

/// <summary>One compiler-advertised bounded dynamic state access.</summary>
public sealed record class ToriiContractDynamicAccessHint
{
    public string BaseKey { get; init; } = string.Empty;

    public string KeyType { get; init; } = string.Empty;

    public string BoundKind { get; init; } = string.Empty;

    public uint MaxKeys { get; init; }
}

/// <summary>Static and bounded-dynamic scheduler hints in a manifest.</summary>
public sealed record class ToriiContractAccessSetHints
{
    private string[] readKeys = Array.Empty<string>();
    private string[] writeKeys = Array.Empty<string>();
    private ToriiContractDynamicAccessHint[] dynamicReads = Array.Empty<ToriiContractDynamicAccessHint>();
    private ToriiContractDynamicAccessHint[] dynamicWrites = Array.Empty<ToriiContractDynamicAccessHint>();

    public IReadOnlyList<string> ReadKeys
    {
        get => ToriiListSnapshots.CopyRequired(readKeys);
        init => readKeys = ToriiListSnapshots.CopyRequired(value);
    }

    public IReadOnlyList<string> WriteKeys
    {
        get => ToriiListSnapshots.CopyRequired(writeKeys);
        init => writeKeys = ToriiListSnapshots.CopyRequired(value);
    }

    public IReadOnlyList<ToriiContractDynamicAccessHint> DynamicReads
    {
        get => ToriiListSnapshots.CopyRequired(dynamicReads);
        init => dynamicReads = ToriiListSnapshots.CopyNonNullItems(value, nameof(DynamicReads))
            ?? Array.Empty<ToriiContractDynamicAccessHint>();
    }

    public IReadOnlyList<ToriiContractDynamicAccessHint> DynamicWrites
    {
        get => ToriiListSnapshots.CopyRequired(dynamicWrites);
        init => dynamicWrites = ToriiListSnapshots.CopyNonNullItems(value, nameof(DynamicWrites))
            ?? Array.Empty<ToriiContractDynamicAccessHint>();
    }
}

/// <summary>Trigger repetition policy.</summary>
public enum ToriiContractTriggerRepeatsKind
{
    Indefinitely,
    Exactly,
}

/// <summary>Exact trigger repetition metadata.</summary>
public sealed record class ToriiContractTriggerRepeats
{
    public ToriiContractTriggerRepeatsKind Kind { get; init; }

    public uint? Exactly { get; init; }
}

/// <summary>Callback target for one manifest trigger.</summary>
public sealed record class ToriiContractTriggerCallback
{
    public string? Namespace { get; init; }

    public string Entrypoint { get; init; } = string.Empty;
}

/// <summary>Complete trigger metadata attached to one entrypoint.</summary>
public sealed record class ToriiContractTriggerDescriptor
{
    private JsonObject metadata = new();

    public string Id { get; init; } = string.Empty;

    public ToriiContractTriggerRepeats Repeats { get; init; } = new();

    public string FilterBase64 { get; init; } = string.Empty;

    public string? Authority { get; init; }

    public JsonObject Metadata
    {
        get => (JsonObject)(metadata.DeepClone());
        init => metadata = value is null ? throw new ArgumentNullException(nameof(Metadata)) : (JsonObject)value.DeepClone();
    }

    public ToriiContractTriggerCallback Callback { get; init; } = new();
}

/// <summary>Exact public interface metadata for one Kotodama entrypoint.</summary>
public sealed record class ToriiContractEntrypointDescriptor
{
    private ToriiContractEntrypointParameter[] parameters = Array.Empty<ToriiContractEntrypointParameter>();
    private string[] readKeys = Array.Empty<string>();
    private string[] writeKeys = Array.Empty<string>();
    private string[] accessHintsSkipped = Array.Empty<string>();
    private ToriiContractTriggerDescriptor[] triggers = Array.Empty<ToriiContractTriggerDescriptor>();

    public string Name { get; init; } = string.Empty;

    public ToriiContractEntrypointKind Kind { get; init; }

    public IReadOnlyList<ToriiContractEntrypointParameter> Parameters
    {
        get => ToriiListSnapshots.CopyRequired(parameters);
        init => parameters = ToriiListSnapshots.CopyNonNullItems(value, nameof(Parameters))
            ?? Array.Empty<ToriiContractEntrypointParameter>();
    }

    public ToriiEntrypointArgumentSchemaV1? ArgumentSchema { get; init; }

    public string? ReturnType { get; init; }

    public ToriiEntrypointValueTypeV1? ReturnSchema { get; init; }

    public string? Permission { get; init; }

    public IReadOnlyList<string> ReadKeys
    {
        get => ToriiListSnapshots.CopyRequired(readKeys);
        init => readKeys = ToriiListSnapshots.CopyRequired(value);
    }

    public IReadOnlyList<string> WriteKeys
    {
        get => ToriiListSnapshots.CopyRequired(writeKeys);
        init => writeKeys = ToriiListSnapshots.CopyRequired(value);
    }

    public bool? AccessHintsComplete { get; init; }

    public IReadOnlyList<string> AccessHintsSkipped
    {
        get => ToriiListSnapshots.CopyRequired(accessHintsSkipped);
        init => accessHintsSkipped = ToriiListSnapshots.CopyRequired(value);
    }

    public IReadOnlyList<ToriiContractTriggerDescriptor> Triggers
    {
        get => ToriiListSnapshots.CopyRequired(triggers);
        init => triggers = ToriiListSnapshots.CopyNonNullItems(value, nameof(Triggers))
            ?? Array.Empty<ToriiContractTriggerDescriptor>();
    }
}

/// <summary>One durable state slot advertised by a Kotodama seiyaku.</summary>
public sealed record class ToriiContractStateDescriptor
{
    public string Name { get; init; } = string.Empty;

    public string TypeName { get; init; } = string.Empty;
}

/// <summary>One stable application error code.</summary>
public sealed record class ToriiContractErrorCodeDescriptor
{
    public string Namespace { get; init; } = string.Empty;

    public string Name { get; init; } = string.Empty;

    public uint Code { get; init; }
}

/// <summary>One localized text in a <c>kotoba</c> table.</summary>
public sealed record class ToriiContractKotobaTranslation
{
    public string Language { get; init; } = string.Empty;

    public string Text { get; init; } = string.Empty;
}

/// <summary>One stable message id and all of its localized texts.</summary>
public sealed record class ToriiContractKotobaTranslationEntry
{
    private ToriiContractKotobaTranslation[] translations = Array.Empty<ToriiContractKotobaTranslation>();

    public string MessageId { get; init; } = string.Empty;

    public IReadOnlyList<ToriiContractKotobaTranslation> Translations
    {
        get => ToriiListSnapshots.CopyRequired(translations);
        init => translations = ToriiListSnapshots.CopyNonNullItems(value, nameof(Translations))
            ?? Array.Empty<ToriiContractKotobaTranslation>();
    }
}

/// <summary>Signature metadata binding a manifest to its approved signer.</summary>
public sealed record class ToriiContractManifestProvenance
{
    public string Signer { get; init; } = string.Empty;

    public string Signature { get; init; } = string.Empty;
}

/// <summary>Full Rust <c>ContractManifest</c> returned by Torii.</summary>
[JsonConverter(typeof(ToriiContractManifestJsonConverter))]
public sealed record class ToriiContractManifest
{
    private string? codeHash;
    private string? abiHash;
    private ToriiContractEntrypointDescriptor[]? entrypoints;
    private ToriiContractStateDescriptor[]? states;
    private ToriiContractErrorCodeDescriptor[]? errorCodes;
    private ToriiContractKotobaTranslationEntry[]? kotoba;

    public string? SeiyakuName { get; init; }

    public string? CodeHash
    {
        get => codeHash;
        init => codeHash = ToriiContractMetadataDirectMetadata.RequireOptionalExactSizedHex(value, nameof(CodeHash));
    }

    public string? AbiHash
    {
        get => abiHash;
        init => abiHash = ToriiContractMetadataDirectMetadata.RequireOptionalExactSizedHex(value, nameof(AbiHash));
    }

    public string? CompilerFingerprint { get; init; }

    public ulong? FeaturesBitmap { get; init; }

    public ToriiContractAccessSetHints? AccessSetHints { get; init; }

    public IReadOnlyList<ToriiContractEntrypointDescriptor>? Entrypoints
    {
        get => entrypoints is null ? null : ToriiListSnapshots.CopyRequired(entrypoints);
        init => entrypoints = value is null
            ? null
            : ToriiListSnapshots.CopyNonNullItems(value, nameof(Entrypoints));
    }

    public IReadOnlyList<ToriiContractStateDescriptor>? States
    {
        get => states is null ? null : ToriiListSnapshots.CopyRequired(states);
        init => states = value is null ? null : ToriiListSnapshots.CopyNonNullItems(value, nameof(States));
    }

    public IReadOnlyList<ToriiContractErrorCodeDescriptor>? ErrorCodes
    {
        get => errorCodes is null ? null : ToriiListSnapshots.CopyRequired(errorCodes);
        init => errorCodes = value is null
            ? null
            : ToriiListSnapshots.CopyNonNullItems(value, nameof(ErrorCodes));
    }

    public IReadOnlyList<ToriiContractKotobaTranslationEntry>? Kotoba
    {
        get => kotoba is null ? null : ToriiListSnapshots.CopyRequired(kotoba);
        init => kotoba = value is null ? null : ToriiListSnapshots.CopyNonNullItems(value, nameof(Kotoba));
    }

    public ToriiContractManifestProvenance? Provenance { get; init; }
}

internal sealed class ToriiContractManifestJsonConverter : JsonConverter<ToriiContractManifest>
{
    public override bool HandleNull => true;

    public override ToriiContractManifest Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        return ToriiContractManifestJson.ReadManifest(ref reader, "contract manifest");
    }

    public override void Write(
        Utf8JsonWriter writer,
        ToriiContractManifest value,
        JsonSerializerOptions options)
    {
        ToriiContractManifestJson.WriteManifest(writer, value, "contract manifest");
    }
}
