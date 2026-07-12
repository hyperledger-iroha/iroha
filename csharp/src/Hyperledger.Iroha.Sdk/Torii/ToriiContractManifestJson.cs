using System.Globalization;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace Hyperledger.Iroha.Torii;

internal static class ToriiContractManifestJson
{
    private const int MaxSchemaNodes = 256;
    private const int MaxSchemaDepth = 256;
    private const int MaxBoundaryWords = 13;

    private static readonly HashSet<string> Keywords = new(StringComparer.Ordinal)
    {
        "authorize", "break", "const", "continue", "else", "enum", "error", "false",
        "fn", "for", "hajimari", "if", "in", "kaizen", "kotoage", "let", "match",
        "module", "return", "seiyaku", "state", "struct", "trigger", "true", "var", "view",
    };

    private static readonly HashSet<string> ReservedDeclarationNames = new(StringComparer.Ordinal)
    {
        "int", "decimal", "quantity", "bool", "string", "bytes", "Json", "AccountId",
        "AssetDefinitionId", "AssetId", "DomainId", "Name", "NftId", "DataSpaceId",
        "Option", "Result", "List", "StateMap", "Secret", "AccountView", "AssetView",
        "AssetDefinitionView", "DomainView", "NftView", "QueryPage", "AxtDescriptor",
        "AssetHandle", "ProofBlob", "SoracloudRequest", "SoracloudResponse",
        "state_map_get", "__kotodama_list_len", "__kotodama_list_get",
        "__kotodama_list_try_set", "__kotodama_list_try_push", "__kotodama_list_pop",
        "__kotodama_list_contains", "__kotodama_list_take", "__kotodama_list_enumerate",
        "__kotodama_decimal_div_round", "__kotodama_quantity_div_round",
        "__kotodama_quantity_ratio_round", "__kotodama_decimal_to_int_trunc",
        "__kotodama_decimal_to_int_round",
    };

    private static readonly IReadOnlyDictionary<string, ToriiEntrypointValueKindV1> ValueKinds =
        new Dictionary<string, ToriiEntrypointValueKindV1>(StringComparer.Ordinal)
        {
            ["Int"] = ToriiEntrypointValueKindV1.Int,
            ["Decimal"] = ToriiEntrypointValueKindV1.Decimal,
            ["Quantity"] = ToriiEntrypointValueKindV1.Quantity,
            ["Bool"] = ToriiEntrypointValueKindV1.Bool,
            ["String"] = ToriiEntrypointValueKindV1.String,
            ["Json"] = ToriiEntrypointValueKindV1.Json,
            ["Name"] = ToriiEntrypointValueKindV1.Name,
            ["AccountId"] = ToriiEntrypointValueKindV1.AccountId,
            ["AssetDefinitionId"] = ToriiEntrypointValueKindV1.AssetDefinitionId,
            ["AssetId"] = ToriiEntrypointValueKindV1.AssetId,
            ["DomainId"] = ToriiEntrypointValueKindV1.DomainId,
            ["NftId"] = ToriiEntrypointValueKindV1.NftId,
            ["DataSpaceId"] = ToriiEntrypointValueKindV1.DataSpaceId,
            ["Blob"] = ToriiEntrypointValueKindV1.Blob,
        };

    internal static ToriiContractManifest ReadManifest(ref Utf8JsonReader reader, string context)
    {
        return ParseManifest(ToriiExplorerJson.ReadObject(ref reader, context), context);
    }

    internal static ToriiContractCodeRecord ReadRecord(ref Utf8JsonReader reader, string context)
    {
        var root = ToriiExplorerJson.ReadObject(ref reader, context);
        EnsureOnly(root, context, "manifest", "code_hash", "abi_hash");
        var manifestObject = RequiredObject(root, "manifest", $"{context}.manifest");
        var manifest = ParseManifest(manifestObject, $"{context}.manifest");
        var codeHash = OptionalConvenienceHash(root, "code_hash", $"{context}.code_hash");
        var abiHash = OptionalConvenienceHash(root, "abi_hash", $"{context}.abi_hash");
        if (!string.Equals(codeHash, manifest.CodeHash, StringComparison.Ordinal))
        {
            throw new JsonException($"{context}.code_hash must exactly match manifest.code_hash.");
        }
        if (!string.Equals(abiHash, manifest.AbiHash, StringComparison.Ordinal))
        {
            throw new JsonException($"{context}.abi_hash must exactly match manifest.abi_hash.");
        }
        return new ToriiContractCodeRecord
        {
            Manifest = manifest,
            CodeHash = codeHash,
            AbiHash = abiHash,
        };
    }

    internal static void ValidateRecord(ToriiContractCodeRecord value, string context)
    {
        ArgumentNullException.ThrowIfNull(value);
        if (value.Manifest is null)
        {
            throw new JsonException($"{context}.manifest is required.");
        }
        _ = BuildManifestNode(value.Manifest, $"{context}.manifest");
        ValidateConvenienceHash(value.CodeHash, $"{context}.code_hash");
        ValidateConvenienceHash(value.AbiHash, $"{context}.abi_hash");
        if (!string.Equals(value.CodeHash, value.Manifest.CodeHash, StringComparison.Ordinal)
            || !string.Equals(value.AbiHash, value.Manifest.AbiHash, StringComparison.Ordinal))
        {
            throw new JsonException($"{context} convenience hashes must exactly match the manifest hashes.");
        }
    }

    internal static void WriteRecord(Utf8JsonWriter writer, ToriiContractCodeRecord value, string context)
    {
        ValidateRecord(value, context);
        var root = new JsonObject
        {
            ["manifest"] = BuildManifestNode(value.Manifest, $"{context}.manifest"),
            ["code_hash"] = value.CodeHash,
            ["abi_hash"] = value.AbiHash,
        };
        root.WriteTo(writer);
    }

    internal static void WriteManifest(Utf8JsonWriter writer, ToriiContractManifest value, string context)
    {
        ArgumentNullException.ThrowIfNull(value);
        BuildManifestNode(value, context).WriteTo(writer);
    }

    private static ToriiContractManifest ParseManifest(JsonObject root, string context)
    {
        EnsureOnly(
            root,
            context,
            "seiyaku_name",
            "code_hash",
            "abi_hash",
            "compiler_fingerprint",
            "features_bitmap",
            "access_set_hints",
            "entrypoints",
            "states",
            "error_codes",
            "kotoba",
            "provenance");

        var seiyakuName = OptionalExactString(root, "seiyaku_name", $"{context}.seiyaku_name");
        if (seiyakuName is not null && !IsCanonicalDeclarationIdentifier(seiyakuName))
        {
            throw new JsonException($"{context}.seiyaku_name must be a canonical Kotodama declaration identifier.");
        }

        var entrypoints = OptionalObjectList(root, "entrypoints", $"{context}.entrypoints", ParseEntrypoint);
        var states = OptionalObjectList(root, "states", $"{context}.states", ParseState);
        var errorCodes = OptionalObjectList(root, "error_codes", $"{context}.error_codes", ParseErrorCode);
        var kotoba = OptionalObjectList(root, "kotoba", $"{context}.kotoba", ParseKotobaEntry);
        var featuresBitmap = OptionalUInt64(root, "features_bitmap", $"{context}.features_bitmap");
        if (featuresBitmap is > 3)
        {
            throw new JsonException($"{context}.features_bitmap contains unsupported Kotodama V1 bits.");
        }
        ValidateManifestCollections(entrypoints, states, errorCodes, kotoba, context);

        return new ToriiContractManifest
        {
            SeiyakuName = seiyakuName,
            CodeHash = OptionalManifestHash(root, "code_hash", $"{context}.code_hash"),
            AbiHash = OptionalManifestHash(root, "abi_hash", $"{context}.abi_hash"),
            CompilerFingerprint = OptionalExactString(
                root,
                "compiler_fingerprint",
                $"{context}.compiler_fingerprint"),
            FeaturesBitmap = featuresBitmap,
            AccessSetHints = OptionalObject(root, "access_set_hints", $"{context}.access_set_hints")
                is { } access ? ParseAccessSetHints(access, $"{context}.access_set_hints") : null,
            Entrypoints = entrypoints,
            States = states,
            ErrorCodes = errorCodes,
            Kotoba = kotoba,
            Provenance = OptionalObject(root, "provenance", $"{context}.provenance")
                is { } provenance ? ParseProvenance(provenance, $"{context}.provenance") : null,
        };
    }

    private static ToriiContractAccessSetHints ParseAccessSetHints(JsonObject root, string context)
    {
        EnsureOnly(root, context, "read_keys", "write_keys", "dynamic_reads", "dynamic_writes");
        return new ToriiContractAccessSetHints
        {
            ReadKeys = RequiredStringList(root, "read_keys", $"{context}.read_keys"),
            WriteKeys = RequiredStringList(root, "write_keys", $"{context}.write_keys"),
            DynamicReads = DefaultObjectList(
                root,
                "dynamic_reads",
                $"{context}.dynamic_reads",
                ParseDynamicHint),
            DynamicWrites = DefaultObjectList(
                root,
                "dynamic_writes",
                $"{context}.dynamic_writes",
                ParseDynamicHint),
        };
    }

    private static ToriiContractDynamicAccessHint ParseDynamicHint(JsonObject root, string context)
    {
        EnsureOnly(root, context, "base_key", "key_type", "bound_kind", "max_keys");
        var baseKey = RequiredExactString(root, "base_key", $"{context}.base_key");
        if (!baseKey.StartsWith("state:", StringComparison.Ordinal) || baseKey == "state:*")
        {
            throw new JsonException($"{context}.base_key must be a concrete state: key.");
        }
        var maxKeys = RequiredUInt32(root, "max_keys", $"{context}.max_keys");
        if (maxKeys == 0)
        {
            throw new JsonException($"{context}.max_keys must be positive.");
        }
        return new ToriiContractDynamicAccessHint
        {
            BaseKey = baseKey,
            KeyType = RequiredExactString(root, "key_type", $"{context}.key_type"),
            BoundKind = RequiredExactString(root, "bound_kind", $"{context}.bound_kind"),
            MaxKeys = maxKeys,
        };
    }

    private static ToriiContractEntrypointDescriptor ParseEntrypoint(JsonObject root, string context)
    {
        EnsureOnly(
            root,
            context,
            "name",
            "kind",
            "params",
            "argument_schema",
            "return_type",
            "return_schema",
            "permission",
            "read_keys",
            "write_keys",
            "access_hints_complete",
            "access_hints_skipped",
            "triggers");
        var name = RequiredExactString(root, "name", $"{context}.name");
        if (!IsCanonicalEntrypointName(name))
        {
            throw new JsonException($"{context}.name must be a canonical Kotodama identifier or branded lifecycle selector.");
        }
        var kind = ParseEntrypointKind(RequiredObject(root, "kind", $"{context}.kind"), $"{context}.kind");
        ValidateLifecycleName(kind, name, context);
        var parameters = DefaultObjectList(root, "params", $"{context}.params", ParseParameter);
        if (parameters.Count > MaxBoundaryWords)
        {
            throw new JsonException($"{context}.params exceeds the V1 argument limit.");
        }
        RequireUnique(parameters.Select(value => value.Name), $"{context}.params");
        var argumentSchema = OptionalObject(root, "argument_schema", $"{context}.argument_schema")
            is { } argument ? ParseArgumentSchema(argument, $"{context}.argument_schema") : null;
        ValidateExactArguments(parameters, argumentSchema, context);
        var returnType = OptionalExactString(root, "return_type", $"{context}.return_type");
        var returnSchema = OptionalObject(root, "return_schema", $"{context}.return_schema")
            is { } returns ? ParseValueType(returns, $"{context}.return_schema") : null;
        if ((returnType is null) != (returnSchema is null)
            || (returnSchema is not null
                && (returnSchema.WordCount > MaxBoundaryWords
                    || !string.Equals(
                        returnSchema.CanonicalTypeName,
                        returnType,
                        StringComparison.Ordinal))))
        {
            throw new JsonException($"{context} return_type and return_schema must describe the same exact V1 type.");
        }
        var permission = OptionalExactString(root, "permission", $"{context}.permission");
        if (kind == ToriiContractEntrypointKind.Kotoage && permission is null)
        {
            throw new JsonException($"{context} kotoage entrypoints must declare permission.");
        }
        if (kind is ToriiContractEntrypointKind.Hajimari or ToriiContractEntrypointKind.Kaizen
            && permission is not null)
        {
            throw new JsonException($"{context} hajimari and kaizen use runtime-defined authorization.");
        }
        var complete = OptionalBoolean(root, "access_hints_complete", $"{context}.access_hints_complete");
        var skipped = DefaultStringList(root, "access_hints_skipped", $"{context}.access_hints_skipped");
        if (complete == true && skipped.Count != 0)
        {
            throw new JsonException($"{context} complete access hints must not contain skipped reasons.");
        }
        if (complete == false && skipped.Count == 0)
        {
            throw new JsonException($"{context} incomplete access hints must contain a skipped reason.");
        }
        return new ToriiContractEntrypointDescriptor
        {
            Name = name,
            Kind = kind,
            Parameters = parameters,
            ArgumentSchema = argumentSchema,
            ReturnType = returnType,
            ReturnSchema = returnSchema,
            Permission = permission,
            ReadKeys = DefaultStringList(root, "read_keys", $"{context}.read_keys"),
            WriteKeys = DefaultStringList(root, "write_keys", $"{context}.write_keys"),
            AccessHintsComplete = complete,
            AccessHintsSkipped = skipped,
            Triggers = DefaultObjectList(root, "triggers", $"{context}.triggers", ParseTrigger),
        };
    }

    private static ToriiContractEntrypointKind ParseEntrypointKind(JsonObject root, string context)
    {
        EnsureOnly(root, context, "kind", "value");
        if (!root.ContainsKey("value") || root["value"] is not null)
        {
            throw new JsonException($"{context}.value must be null.");
        }
        return RequiredExactString(root, "kind", $"{context}.kind") switch
        {
            "Kotoage" => ToriiContractEntrypointKind.Kotoage,
            "View" => ToriiContractEntrypointKind.View,
            "Hajimari" => ToriiContractEntrypointKind.Hajimari,
            "Kaizen" => ToriiContractEntrypointKind.Kaizen,
            var unsupported => throw new JsonException($"{context}.kind `{unsupported}` is not a branded Kotodama V1 kind."),
        };
    }

    private static ToriiContractEntrypointParameter ParseParameter(JsonObject root, string context)
    {
        EnsureOnly(root, context, "name", "type_name");
        var name = RequiredExactString(root, "name", $"{context}.name");
        if (!IsCanonicalBoundaryIdentifier(name))
        {
            throw new JsonException($"{context}.name must be a canonical Kotodama identifier.");
        }
        return new ToriiContractEntrypointParameter
        {
            Name = name,
            TypeName = RequiredExactString(root, "type_name", $"{context}.type_name"),
        };
    }

    private static ToriiEntrypointArgumentSchemaV1 ParseArgumentSchema(JsonObject root, string context)
    {
        EnsureOnly(root, context, "fields");
        var fields = RequiredObjectList(root, "fields", $"{context}.fields", ParseArgumentField);
        if (fields.Count is < 1 or > MaxBoundaryWords)
        {
            throw new JsonException($"{context}.fields must contain 1..13 items.");
        }
        RequireUnique(fields.Select(field => field.Name), $"{context}.fields");
        var words = fields.Sum(field => field.ValueType.WordCount);
        if (words > MaxBoundaryWords)
        {
            throw new JsonException($"{context} exceeds the V1 register window.");
        }
        return new ToriiEntrypointArgumentSchemaV1 { Fields = fields, WordCount = words };
    }

    private static ToriiEntrypointArgumentFieldV1 ParseArgumentField(JsonObject root, string context)
    {
        EnsureOnly(root, context, "name", "ty");
        var name = RequiredExactString(root, "name", $"{context}.name");
        if (!IsCanonicalBoundaryIdentifier(name))
        {
            throw new JsonException($"{context}.name must be a canonical Kotodama identifier.");
        }
        return new ToriiEntrypointArgumentFieldV1
        {
            Name = name,
            ValueType = ParseValueType(RequiredObject(root, "ty", $"{context}.ty"), $"{context}.ty"),
        };
    }

    private static ToriiEntrypointValueTypeV1 ParseValueType(JsonObject root, string context)
    {
        EnsureOnly(root, context, "nodes");
        var nodes = RequiredObjectList(root, "nodes", $"{context}.nodes", ParseValueTypeNode);
        if (nodes.Count is < 1 or > MaxSchemaNodes)
        {
            throw new JsonException($"{context}.nodes must contain 1..256 items.");
        }
        var analysis = AnalyzeValueType(nodes, 0, 1, context);
        if (analysis.NextIndex != nodes.Count || analysis.NodeCount > MaxSchemaNodes)
        {
            throw new JsonException($"{context} is not a canonical flat preorder V1 schema.");
        }
        return new ToriiEntrypointValueTypeV1
        {
            Nodes = nodes,
            WordCount = analysis.WordCount,
            CanonicalTypeName = analysis.TypeName,
        };
    }

    private static ToriiEntrypointValueTypeNodeV1 ParseValueTypeNode(JsonObject root, string context)
    {
        EnsureOnly(root, context, "kind", "value");
        if (!root.ContainsKey("value"))
        {
            throw new JsonException($"{context}.value is required.");
        }
        return RequiredExactString(root, "kind", $"{context}.kind") switch
        {
            "Struct" => new ToriiEntrypointValueTypeNodeV1
            {
                Kind = ToriiEntrypointValueTypeNodeKindV1.Struct,
                StructValue = ParseStructNode(RequiredObject(root, "value", $"{context}.value"), $"{context}.value"),
            },
            "Tuple" => ParseTupleNode(root, context),
            "Option" => ParseNullNode(root, context, ToriiEntrypointValueTypeNodeKindV1.Option),
            "Result" => ParseNullNode(root, context, ToriiEntrypointValueTypeNodeKindV1.Result),
            "List" => new ToriiEntrypointValueTypeNodeV1
            {
                Kind = ToriiEntrypointValueTypeNodeKindV1.List,
                ListValue = ParseListNode(RequiredObject(root, "value", $"{context}.value"), $"{context}.value"),
            },
            "Leaf" => new ToriiEntrypointValueTypeNodeV1
            {
                Kind = ToriiEntrypointValueTypeNodeKindV1.Leaf,
                LeafKind = ParseLeafKind(RequiredObject(root, "value", $"{context}.value"), $"{context}.value"),
            },
            var unsupported => throw new JsonException($"{context}.kind `{unsupported}` is unsupported."),
        };
    }

    private static ToriiEntrypointValueTypeNodeV1 ParseTupleNode(JsonObject root, string context)
    {
        var arity = RequiredUInt16(root, "value", $"{context}.value");
        if (arity < 2)
        {
            throw new JsonException($"{context}.value tuple arity must be in 2..65535.");
        }
        return new ToriiEntrypointValueTypeNodeV1
        {
            Kind = ToriiEntrypointValueTypeNodeKindV1.Tuple,
            TupleArity = arity,
        };
    }

    private static ToriiEntrypointValueTypeNodeV1 ParseNullNode(
        JsonObject root,
        string context,
        ToriiEntrypointValueTypeNodeKindV1 kind)
    {
        if (root["value"] is not null)
        {
            throw new JsonException($"{context}.value must be null for {kind}.");
        }
        return new ToriiEntrypointValueTypeNodeV1 { Kind = kind };
    }

    private static ToriiEntrypointStructTypeNodeV1 ParseStructNode(JsonObject root, string context)
    {
        EnsureOnly(root, context, "name", "fields");
        var name = RequiredExactString(root, "name", $"{context}.name");
        var fields = RequiredStringList(root, "fields", $"{context}.fields");
        if (!IsCanonicalBoundaryIdentifier(name)
            || fields.Count == 0
            || fields.Any(field => !IsCanonicalBoundaryIdentifier(field)))
        {
            throw new JsonException($"{context} must use canonical Kotodama identifiers.");
        }
        RequireUnique(fields, $"{context}.fields");
        return new ToriiEntrypointStructTypeNodeV1 { Name = name, Fields = fields };
    }

    private static ToriiEntrypointListTypeNodeV1 ParseListNode(JsonObject root, string context)
    {
        EnsureOnly(root, context, "capacity");
        var capacity = RequiredByte(root, "capacity", $"{context}.capacity");
        if (capacity is < 1 or > 64)
        {
            throw new JsonException($"{context}.capacity must be in 1..64.");
        }
        return new ToriiEntrypointListTypeNodeV1 { Capacity = capacity };
    }

    private static ToriiEntrypointValueKindV1 ParseLeafKind(JsonObject root, string context)
    {
        EnsureOnly(root, context, "kind", "value");
        if (!root.ContainsKey("value") || root["value"] is not null)
        {
            throw new JsonException($"{context}.value must be null.");
        }
        var label = RequiredExactString(root, "kind", $"{context}.kind");
        return ValueKinds.TryGetValue(label, out var kind)
            ? kind
            : throw new JsonException($"{context}.kind `{label}` is unsupported.");
    }

    private static TypeAnalysis AnalyzeValueType(
        IReadOnlyList<ToriiEntrypointValueTypeNodeV1> nodes,
        int index,
        int depth,
        string context)
    {
        if (index != 0 || depth != 1 || nodes.Count is < 1 or > MaxSchemaNodes)
        {
            throw new JsonException($"{context} exceeds flat V1 schema bounds.");
        }

        var frames = new List<AnalysisFrame>();
        var wordCount = 0;
        var maxDepth = 0;
        for (var nodeIndex = 0; nodeIndex < nodes.Count; nodeIndex++)
        {
            while (frames.Count != 0 && frames[^1].RemainingChildren == 0)
            {
                frames.RemoveAt(frames.Count - 1);
            }

            var suppressWords = false;
            if (nodeIndex != 0)
            {
                if (frames.Count == 0 || frames[^1].RemainingChildren == 0)
                {
                    throw new JsonException(
                        $"{context}.nodes is not one complete canonical prefix type tree.");
                }
                var parent = frames[^1];
                suppressWords = parent.SuppressWords;
                frames[^1] = parent with
                {
                    RemainingChildren = parent.RemainingChildren - 1,
                };
            }

            var currentDepth = frames.Count + 1;
            if (currentDepth > MaxSchemaDepth)
            {
                throw new JsonException($"{context} exceeds flat V1 schema bounds.");
            }
            maxDepth = Math.Max(maxDepth, currentDepth);

            var node = nodes[nodeIndex];
            var childCount = ValidateNodeAndGetChildCount(node, context);
            var isHandle = node.Kind is ToriiEntrypointValueTypeNodeKindV1.Option or
                ToriiEntrypointValueTypeNodeKindV1.Result or
                ToriiEntrypointValueTypeNodeKindV1.List;
            if (!suppressWords
                && (isHandle || node.Kind == ToriiEntrypointValueTypeNodeKindV1.Leaf))
            {
                wordCount = checked(wordCount + 1);
            }
            if (childCount != 0)
            {
                frames.Add(new AnalysisFrame(childCount, suppressWords || isHandle));
            }
        }
        while (frames.Count != 0 && frames[^1].RemainingChildren == 0)
        {
            frames.RemoveAt(frames.Count - 1);
        }
        if (frames.Count != 0)
        {
            throw new JsonException(
                $"{context}.nodes ends before its prefix type tree is complete.");
        }

        var rendered = new List<RenderedType>();
        for (var nodeIndex = nodes.Count - 1; nodeIndex >= 0; nodeIndex--)
        {
            var node = nodes[nodeIndex];
            var childCount = ValidateNodeAndGetChildCount(node, context);
            if (rendered.Count < childCount)
            {
                throw new JsonException(
                    $"{context}.nodes ends before its prefix type tree is complete.");
            }
            var children = childCount == 0
                ? new List<RenderedType>()
                : rendered.GetRange(rendered.Count - childCount, childCount);
            if (childCount != 0)
            {
                rendered.RemoveRange(rendered.Count - childCount, childCount);
                children.Reverse();
            }

            RenderedType result;
            switch (node.Kind)
            {
                case ToriiEntrypointValueTypeNodeKindV1.Struct:
                    var product = node.StructValue!;
                    var childTypeNames = children.Select(child => child.TypeName).ToArray();
                    if (string.Equals(product.Name, "QueryPage", StringComparison.Ordinal))
                    {
                        var viewName = children.Count == 2
                            ? children[0].ListElementCoreQueryViewName
                            : null;
                        if (!product.Fields.SequenceEqual(
                                new[] { "items", "next_offset" },
                                StringComparer.Ordinal)
                            || children.Count != 2
                            || children[0].ListCapacity != 64
                            || viewName is null
                            || !string.Equals(
                                children[1].TypeName,
                                "Option<int>",
                                StringComparison.Ordinal))
                        {
                            throw new JsonException(
                                $"{context} contains a forged QueryPage schema.");
                        }
                        result = new RenderedType($"QueryPage<{viewName}>", null, null, null);
                    }
                    else if (IsCoreQueryViewName(product.Name))
                    {
                        if (!IsExactCoreQueryView(product, childTypeNames))
                        {
                            throw new JsonException(
                                $"{context} contains a forged {product.Name} projection schema.");
                        }
                        result = new RenderedType(product.Name, product.Name, null, null);
                    }
                    else
                    {
                        result = new RenderedType($"struct {product.Name}", null, null, null);
                    }
                    break;
                case ToriiEntrypointValueTypeNodeKindV1.Tuple:
                    result = new RenderedType(
                        $"({string.Join(", ", children.Select(child => child.TypeName))})",
                        null,
                        null,
                        null);
                    break;
                case ToriiEntrypointValueTypeNodeKindV1.Option:
                    result = new RenderedType($"Option<{children[0].TypeName}>", null, null, null);
                    break;
                case ToriiEntrypointValueTypeNodeKindV1.Result:
                    result = new RenderedType(
                        $"Result<{children[0].TypeName}, {children[1].TypeName}>",
                        null,
                        null,
                        null);
                    break;
                case ToriiEntrypointValueTypeNodeKindV1.List:
                    var list = node.ListValue!;
                    result = new RenderedType(
                        $"List<{children[0].TypeName}, {list.Capacity}>",
                        null,
                        list.Capacity,
                        children[0].CoreQueryViewName);
                    break;
                case ToriiEntrypointValueTypeNodeKindV1.Leaf:
                    result = new RenderedType(
                        CanonicalLeafName(node.LeafKind!.Value),
                        null,
                        null,
                        null);
                    break;
                default:
                    throw new JsonException($"{context} contains an unsupported node.");
            }
            rendered.Add(result);
        }
        if (rendered.Count != 1)
        {
            throw new JsonException(
                $"{context}.nodes is not one complete canonical prefix type tree.");
        }
        var root = rendered[0];
        return new TypeAnalysis(
            nodes.Count,
            nodes.Count,
            wordCount,
            maxDepth,
            root.TypeName,
            root.CoreQueryViewName,
            root.ListElementCoreQueryViewName);
    }

    private static int ValidateNodeAndGetChildCount(
        ToriiEntrypointValueTypeNodeV1 node,
        string context)
    {
        var exactPayload = node.Kind switch
        {
            ToriiEntrypointValueTypeNodeKindV1.Struct =>
                node.StructValue is not null
                && node.TupleArity is null
                && node.ListValue is null
                && node.LeafKind is null,
            ToriiEntrypointValueTypeNodeKindV1.Tuple =>
                node.StructValue is null
                && node.TupleArity is not null
                && node.ListValue is null
                && node.LeafKind is null,
            ToriiEntrypointValueTypeNodeKindV1.Option or
                ToriiEntrypointValueTypeNodeKindV1.Result =>
                node.StructValue is null
                && node.TupleArity is null
                && node.ListValue is null
                && node.LeafKind is null,
            ToriiEntrypointValueTypeNodeKindV1.List =>
                node.StructValue is null
                && node.TupleArity is null
                && node.ListValue is not null
                && node.LeafKind is null,
            ToriiEntrypointValueTypeNodeKindV1.Leaf =>
                node.StructValue is null
                && node.TupleArity is null
                && node.ListValue is null
                && node.LeafKind is not null,
            _ => false,
        };
        if (!exactPayload)
        {
            throw new JsonException($"{context} contains inconsistent node metadata.");
        }

        switch (node.Kind)
        {
            case ToriiEntrypointValueTypeNodeKindV1.Struct:
                var product = node.StructValue!;
                if (product.Fields.Count == 0
                    || !IsCanonicalBoundaryIdentifier(product.Name)
                    || product.Fields.Any(field => !IsCanonicalBoundaryIdentifier(field)))
                {
                    throw new JsonException($"{context} contains a noncanonical struct node.");
                }
                RequireUnique(product.Fields, $"{context}.fields");
                return product.Fields.Count;
            case ToriiEntrypointValueTypeNodeKindV1.Tuple:
                if (node.TupleArity!.Value < 2)
                {
                    throw new JsonException($"{context} tuple arity must be at least 2.");
                }
                return node.TupleArity.Value;
            case ToriiEntrypointValueTypeNodeKindV1.Option:
            case ToriiEntrypointValueTypeNodeKindV1.List:
                if (node.Kind == ToriiEntrypointValueTypeNodeKindV1.List
                    && node.ListValue!.Capacity is < 1 or > 64)
                {
                    throw new JsonException($"{context} list capacity must be in 1..64.");
                }
                return 1;
            case ToriiEntrypointValueTypeNodeKindV1.Result:
                return 2;
            case ToriiEntrypointValueTypeNodeKindV1.Leaf:
                return 0;
            default:
                throw new JsonException($"{context} contains an unsupported node.");
        }
    }

    private static bool IsCoreQueryViewName(string name)
    {
        return name is "AccountView" or "AssetView" or "AssetDefinitionView" or
            "DomainView" or "NftView";
    }

    private static bool IsExactCoreQueryView(
        ToriiEntrypointStructTypeNodeV1 product,
        IReadOnlyList<string> childTypeNames)
    {
        string[] expectedFields;
        string[] expectedTypes;
        switch (product.Name)
        {
            case "AccountView":
                expectedFields = new[] { "id", "metadata" };
                expectedTypes = new[] { "AccountId", "Json" };
                break;
            case "AssetView":
                expectedFields = new[] { "id", "amount" };
                expectedTypes = new[] { "AssetId", "quantity" };
                break;
            case "AssetDefinitionView":
                expectedFields = new[]
                {
                    "id", "name", "description", "owned_by", "total_quantity", "metadata",
                };
                expectedTypes = new[]
                {
                    "AssetDefinitionId", "string", "Option<string>", "AccountId", "quantity", "Json",
                };
                break;
            case "DomainView":
                expectedFields = new[] { "id", "owned_by", "metadata" };
                expectedTypes = new[] { "DomainId", "AccountId", "Json" };
                break;
            case "NftView":
                expectedFields = new[] { "id", "owned_by", "content" };
                expectedTypes = new[] { "NftId", "AccountId", "Json" };
                break;
            default:
                return false;
        }
        return product.Fields.SequenceEqual(expectedFields, StringComparer.Ordinal)
            && childTypeNames.SequenceEqual(expectedTypes, StringComparer.Ordinal);
    }

    private static string CanonicalLeafName(ToriiEntrypointValueKindV1 kind)
    {
        return kind switch
        {
            ToriiEntrypointValueKindV1.Int => "int",
            ToriiEntrypointValueKindV1.Decimal => "decimal",
            ToriiEntrypointValueKindV1.Quantity => "quantity",
            ToriiEntrypointValueKindV1.Bool => "bool",
            ToriiEntrypointValueKindV1.String => "string",
            ToriiEntrypointValueKindV1.Json => "Json",
            ToriiEntrypointValueKindV1.Name => "Name",
            ToriiEntrypointValueKindV1.AccountId => "AccountId",
            ToriiEntrypointValueKindV1.AssetDefinitionId => "AssetDefinitionId",
            ToriiEntrypointValueKindV1.AssetId => "AssetId",
            ToriiEntrypointValueKindV1.DomainId => "DomainId",
            ToriiEntrypointValueKindV1.NftId => "NftId",
            ToriiEntrypointValueKindV1.DataSpaceId => "DataSpaceId",
            ToriiEntrypointValueKindV1.Blob => "bytes",
            _ => throw new JsonException("Unsupported Kotodama V1 leaf kind."),
        };
    }

    private static ToriiContractTriggerDescriptor ParseTrigger(JsonObject root, string context)
    {
        EnsureOnly(root, context, "id", "repeats", "filter", "authority", "metadata", "callback");
        var filter = RequiredExactString(root, "filter", $"{context}.filter");
        ValidateCanonicalBase64(filter, $"{context}.filter");
        var authority = OptionalExactString(root, "authority", $"{context}.authority");
        if (authority is not null)
        {
            try
            {
                authority = ToriiExplorerDirectMetadata.RequireCanonicalAccountId(authority, $"{context}.authority");
            }
            catch (ArgumentException exception)
            {
                throw new JsonException($"{context}.authority must be a canonical I105 account id.", exception);
            }
        }
        return new ToriiContractTriggerDescriptor
        {
            Id = RequiredExactString(root, "id", $"{context}.id"),
            Repeats = ParseRepeats(RequiredObject(root, "repeats", $"{context}.repeats"), $"{context}.repeats"),
            FilterBase64 = filter,
            Authority = authority,
            Metadata = RequiredObject(root, "metadata", $"{context}.metadata"),
            Callback = ParseCallback(RequiredObject(root, "callback", $"{context}.callback"), $"{context}.callback"),
        };
    }

    private static ToriiContractTriggerRepeats ParseRepeats(JsonObject root, string context)
    {
        if (root.Count != 1)
        {
            throw new JsonException($"{context} must contain exactly one enum variant.");
        }
        if (root.TryGetPropertyValue("Indefinitely", out var indefinite))
        {
            if (indefinite is not null)
            {
                throw new JsonException($"{context}.Indefinitely must be null.");
            }
            return new ToriiContractTriggerRepeats
            {
                Kind = ToriiContractTriggerRepeatsKind.Indefinitely,
            };
        }
        if (root.ContainsKey("Exactly"))
        {
            return new ToriiContractTriggerRepeats
            {
                Kind = ToriiContractTriggerRepeatsKind.Exactly,
                Exactly = RequiredUInt32(root, "Exactly", $"{context}.Exactly"),
            };
        }
        throw new JsonException($"{context} contains an unsupported repetition policy.");
    }

    private static ToriiContractTriggerCallback ParseCallback(JsonObject root, string context)
    {
        EnsureOnly(root, context, "namespace", "entrypoint");
        var entrypoint = RequiredExactString(root, "entrypoint", $"{context}.entrypoint");
        if (!IsCanonicalEntrypointName(entrypoint))
        {
            throw new JsonException($"{context}.entrypoint must be a canonical Kotodama selector.");
        }
        return new ToriiContractTriggerCallback
        {
            Namespace = OptionalExactString(root, "namespace", $"{context}.namespace"),
            Entrypoint = entrypoint,
        };
    }

    private static ToriiContractStateDescriptor ParseState(JsonObject root, string context)
    {
        EnsureOnly(root, context, "name", "type_name");
        var name = RequiredExactString(root, "name", $"{context}.name");
        if (!IsCanonicalDeclarationIdentifier(name))
        {
            throw new JsonException($"{context}.name must be a canonical Kotodama declaration identifier.");
        }
        return new ToriiContractStateDescriptor
        {
            Name = name,
            TypeName = RequiredExactString(root, "type_name", $"{context}.type_name"),
        };
    }

    private static ToriiContractErrorCodeDescriptor ParseErrorCode(JsonObject root, string context)
    {
        EnsureOnly(root, context, "namespace", "name", "code");
        var namespaceName = RequiredExactString(root, "namespace", $"{context}.namespace");
        var name = RequiredExactString(root, "name", $"{context}.name");
        if (!IsCanonicalDeclarationIdentifier(namespaceName)
            || !IsCanonicalBoundaryIdentifier(name))
        {
            throw new JsonException(
                $"{context} namespace and name must be canonical Kotodama identifiers.");
        }
        var code = RequiredUInt32(root, "code", $"{context}.code");
        if (code == 0)
        {
            throw new JsonException($"{context}.code must be a non-zero u32.");
        }
        return new ToriiContractErrorCodeDescriptor
        {
            Namespace = namespaceName,
            Name = name,
            Code = code,
        };
    }

    private static ToriiContractKotobaTranslationEntry ParseKotobaEntry(JsonObject root, string context)
    {
        EnsureOnly(root, context, "msg_id", "translations");
        var translations = RequiredObjectList(root, "translations", $"{context}.translations", ParseKotobaTranslation);
        RequireUnique(translations.Select(value => value.Language), $"{context}.translations");
        return new ToriiContractKotobaTranslationEntry
        {
            MessageId = RequiredExactString(root, "msg_id", $"{context}.msg_id"),
            Translations = translations,
        };
    }

    private static ToriiContractKotobaTranslation ParseKotobaTranslation(JsonObject root, string context)
    {
        EnsureOnly(root, context, "lang", "text");
        return new ToriiContractKotobaTranslation
        {
            Language = RequiredExactString(root, "lang", $"{context}.lang"),
            Text = RequiredStringAllowEmpty(root, "text", $"{context}.text"),
        };
    }

    private static ToriiContractManifestProvenance ParseProvenance(JsonObject root, string context)
    {
        EnsureOnly(root, context, "signer", "signature");
        return new ToriiContractManifestProvenance
        {
            Signer = RequiredExactString(root, "signer", $"{context}.signer"),
            Signature = RequiredExactString(root, "signature", $"{context}.signature"),
        };
    }

    private static void ValidateManifestCollections(
        IReadOnlyList<ToriiContractEntrypointDescriptor>? entrypoints,
        IReadOnlyList<ToriiContractStateDescriptor>? states,
        IReadOnlyList<ToriiContractErrorCodeDescriptor>? errorCodes,
        IReadOnlyList<ToriiContractKotobaTranslationEntry>? kotoba,
        string context)
    {
        if (entrypoints is not null)
        {
            RequireUnique(entrypoints.Select(value => value.Name), $"{context}.entrypoints");
            if (entrypoints.Count(value => value.Kind == ToriiContractEntrypointKind.Hajimari) > 1
                || entrypoints.Count(value => value.Kind == ToriiContractEntrypointKind.Kaizen) > 1)
            {
                throw new JsonException($"{context}.entrypoints contains duplicate lifecycle declarations.");
            }
            var kinds = entrypoints.ToDictionary(value => value.Name, value => value.Kind, StringComparer.Ordinal);
            var triggerIds = new HashSet<string>(StringComparer.Ordinal);
            foreach (var descriptor in entrypoints)
            {
                foreach (var trigger in descriptor.Triggers)
                {
                    if (!triggerIds.Add(trigger.Id))
                    {
                        throw new JsonException(
                            $"{context}.entrypoints contains duplicate trigger id `{trigger.Id}`.");
                    }
                    if (trigger.Callback.Namespace is null
                        && (!kinds.TryGetValue(trigger.Callback.Entrypoint, out var callbackKind)
                            || callbackKind != ToriiContractEntrypointKind.Kotoage))
                    {
                        throw new JsonException(
                            $"{context}.entrypoints local trigger callback `{trigger.Callback.Entrypoint}` must name a declared kotoage entrypoint.");
                    }
                }
            }
        }
        if (states is not null)
        {
            RequireUnique(states.Select(value => value.Name), $"{context}.states");
        }
        if (errorCodes is not null)
        {
            RequireUnique(
                errorCodes.Select(value => $"{value.Namespace}::{value.Name}"),
                $"{context}.error_codes");
            RequireUnique(errorCodes.Select(value => value.Code.ToString(CultureInfo.InvariantCulture)), $"{context}.error_codes.code");
        }
        if (kotoba is not null)
        {
            RequireUnique(kotoba.Select(value => value.MessageId), $"{context}.kotoba");
        }
    }

    private static void ValidateExactArguments(
        IReadOnlyList<ToriiContractEntrypointParameter> parameters,
        ToriiEntrypointArgumentSchemaV1? schema,
        string context)
    {
        if (parameters.Count == 0 && schema is null)
        {
            return;
        }
        if (schema is null || schema.Fields.Count != parameters.Count)
        {
            throw new JsonException($"{context}.argument_schema must exactly describe params.");
        }
        for (var index = 0; index < parameters.Count; index++)
        {
            if (!string.Equals(parameters[index].Name, schema.Fields[index].Name, StringComparison.Ordinal)
                || !string.Equals(
                    parameters[index].TypeName,
                    schema.Fields[index].ValueType.CanonicalTypeName,
                    StringComparison.Ordinal))
            {
                throw new JsonException($"{context}.argument_schema must exactly describe params.");
            }
        }
    }

    private static void ValidateLifecycleName(
        ToriiContractEntrypointKind kind,
        string name,
        string context)
    {
        var matches = kind switch
        {
            ToriiContractEntrypointKind.Hajimari => name is "hajimari" or "始まり",
            ToriiContractEntrypointKind.Kaizen => name is "kaizen" or "改善",
            _ => name is not ("hajimari" or "始まり" or "kaizen" or "改善"),
        };
        if (!matches)
        {
            throw new JsonException($"{context}.kind does not match its branded lifecycle selector.");
        }
    }

    private static JsonObject BuildManifestNode(ToriiContractManifest value, string context)
    {
        var root = new JsonObject
        {
            ["seiyaku_name"] = value.SeiyakuName,
            ["code_hash"] = value.CodeHash is null ? null : FormatManifestHash(value.CodeHash, $"{context}.code_hash"),
            ["abi_hash"] = value.AbiHash is null ? null : FormatManifestHash(value.AbiHash, $"{context}.abi_hash"),
            ["compiler_fingerprint"] = value.CompilerFingerprint,
            ["features_bitmap"] = value.FeaturesBitmap.HasValue
                ? JsonValue.Create(value.FeaturesBitmap.Value)
                : null,
            ["access_set_hints"] = value.AccessSetHints is null ? null : BuildAccessSetHints(value.AccessSetHints, $"{context}.access_set_hints"),
            ["entrypoints"] = BuildOptionalArray(value.Entrypoints, (item, itemContext) => BuildEntrypoint(item, itemContext), $"{context}.entrypoints"),
            ["states"] = BuildOptionalArray(value.States, BuildState, $"{context}.states"),
            ["error_codes"] = BuildOptionalArray(value.ErrorCodes, BuildErrorCode, $"{context}.error_codes"),
            ["kotoba"] = BuildOptionalArray(value.Kotoba, BuildKotobaEntry, $"{context}.kotoba"),
            ["provenance"] = value.Provenance is null ? null : BuildProvenance(value.Provenance, $"{context}.provenance"),
        };
        ValidateExactStringOptional(value.SeiyakuName, $"{context}.seiyaku_name");
        if (value.SeiyakuName is not null && !IsCanonicalDeclarationIdentifier(value.SeiyakuName))
        {
            throw new JsonException($"{context}.seiyaku_name must be a canonical Kotodama declaration identifier.");
        }
        ValidateExactStringOptional(value.CompilerFingerprint, $"{context}.compiler_fingerprint");
        if (value.FeaturesBitmap is > 3)
        {
            throw new JsonException($"{context}.features_bitmap contains unsupported Kotodama V1 bits.");
        }
        ValidateManifestCollections(value.Entrypoints, value.States, value.ErrorCodes, value.Kotoba, context);
        return root;
    }

    private static JsonObject BuildAccessSetHints(ToriiContractAccessSetHints value, string context)
    {
        return new JsonObject
        {
            ["read_keys"] = BuildStringArray(value.ReadKeys, $"{context}.read_keys"),
            ["write_keys"] = BuildStringArray(value.WriteKeys, $"{context}.write_keys"),
            ["dynamic_reads"] = BuildArray(value.DynamicReads, BuildDynamicHint, $"{context}.dynamic_reads"),
            ["dynamic_writes"] = BuildArray(value.DynamicWrites, BuildDynamicHint, $"{context}.dynamic_writes"),
        };
    }

    private static JsonObject BuildDynamicHint(ToriiContractDynamicAccessHint value, string context)
    {
        if (!value.BaseKey.StartsWith("state:", StringComparison.Ordinal)
            || value.BaseKey == "state:*"
            || value.MaxKeys == 0)
        {
            throw new JsonException($"{context} must declare a positive bounded concrete state access.");
        }
        return new JsonObject
        {
            ["base_key"] = RequireExact(value.BaseKey, $"{context}.base_key"),
            ["key_type"] = RequireExact(value.KeyType, $"{context}.key_type"),
            ["bound_kind"] = RequireExact(value.BoundKind, $"{context}.bound_kind"),
            ["max_keys"] = value.MaxKeys,
        };
    }

    private static JsonObject BuildEntrypoint(ToriiContractEntrypointDescriptor value, string context)
    {
        if (!IsCanonicalEntrypointName(value.Name))
        {
            throw new JsonException($"{context}.name must be a canonical Kotodama selector.");
        }
        ValidateLifecycleName(value.Kind, value.Name, context);
        ValidateExactArguments(value.Parameters, value.ArgumentSchema, context);
        if ((value.ReturnType is null) != (value.ReturnSchema is null)
            || (value.ReturnSchema is not null
                && (value.ReturnSchema.WordCount > MaxBoundaryWords
                    || value.ReturnSchema.CanonicalTypeName != value.ReturnType)))
        {
            throw new JsonException($"{context} return_type and return_schema must match.");
        }
        if (value.Kind == ToriiContractEntrypointKind.Kotoage && value.Permission is null)
        {
            throw new JsonException($"{context} kotoage entrypoints must declare permission.");
        }
        if (value.Kind is ToriiContractEntrypointKind.Hajimari or ToriiContractEntrypointKind.Kaizen
            && value.Permission is not null)
        {
            throw new JsonException($"{context} lifecycle authorization is runtime-defined.");
        }
        if (value.AccessHintsComplete == true && value.AccessHintsSkipped.Count != 0
            || value.AccessHintsComplete == false && value.AccessHintsSkipped.Count == 0)
        {
            throw new JsonException($"{context} access-hint completeness is inconsistent.");
        }
        return new JsonObject
        {
            ["name"] = value.Name,
            ["kind"] = new JsonObject { ["kind"] = value.Kind.ToString(), ["value"] = null },
            ["params"] = BuildArray(value.Parameters, BuildParameter, $"{context}.params"),
            ["argument_schema"] = value.ArgumentSchema is null ? null : BuildArgumentSchema(value.ArgumentSchema, $"{context}.argument_schema"),
            ["return_type"] = value.ReturnType,
            ["return_schema"] = value.ReturnSchema is null ? null : BuildValueType(value.ReturnSchema, $"{context}.return_schema"),
            ["permission"] = value.Permission,
            ["read_keys"] = BuildStringArray(value.ReadKeys, $"{context}.read_keys"),
            ["write_keys"] = BuildStringArray(value.WriteKeys, $"{context}.write_keys"),
            ["access_hints_complete"] = value.AccessHintsComplete.HasValue
                ? JsonValue.Create(value.AccessHintsComplete.Value)
                : null,
            ["access_hints_skipped"] = BuildStringArray(value.AccessHintsSkipped, $"{context}.access_hints_skipped"),
            ["triggers"] = BuildArray(value.Triggers, BuildTrigger, $"{context}.triggers"),
        };
    }

    private static JsonObject BuildParameter(ToriiContractEntrypointParameter value, string context)
    {
        if (!IsCanonicalBoundaryIdentifier(value.Name))
        {
            throw new JsonException($"{context}.name must be a canonical Kotodama identifier.");
        }
        return new JsonObject
        {
            ["name"] = value.Name,
            ["type_name"] = RequireExact(value.TypeName, $"{context}.type_name"),
        };
    }

    private static JsonObject BuildArgumentSchema(ToriiEntrypointArgumentSchemaV1 value, string context)
    {
        var fields = BuildArray(value.Fields, BuildArgumentField, $"{context}.fields");
        if (value.Fields.Count is < 1 or > MaxBoundaryWords
            || value.Fields.Sum(field => field.ValueType.WordCount) != value.WordCount
            || value.WordCount > MaxBoundaryWords)
        {
            throw new JsonException($"{context} violates V1 argument bounds.");
        }
        RequireUnique(value.Fields.Select(field => field.Name), $"{context}.fields");
        return new JsonObject { ["fields"] = fields };
    }

    private static JsonObject BuildArgumentField(ToriiEntrypointArgumentFieldV1 value, string context)
    {
        if (!IsCanonicalBoundaryIdentifier(value.Name))
        {
            throw new JsonException($"{context}.name must be a canonical Kotodama identifier.");
        }
        return new JsonObject
        {
            ["name"] = value.Name,
            ["ty"] = BuildValueType(value.ValueType, $"{context}.ty"),
        };
    }

    private static JsonObject BuildValueType(ToriiEntrypointValueTypeV1 value, string context)
    {
        if (value.Nodes.Count is < 1 or > MaxSchemaNodes)
        {
            throw new JsonException($"{context}.nodes must contain 1..256 items.");
        }
        var analysis = AnalyzeValueType(value.Nodes, 0, 1, context);
        if (analysis.NextIndex != value.Nodes.Count
            || analysis.WordCount != value.WordCount
            || analysis.TypeName != value.CanonicalTypeName)
        {
            throw new JsonException($"{context} carries inconsistent derived schema metadata.");
        }
        return new JsonObject
        {
            ["nodes"] = BuildArray(value.Nodes, BuildValueTypeNode, $"{context}.nodes"),
        };
    }

    private static JsonObject BuildValueTypeNode(ToriiEntrypointValueTypeNodeV1 value, string context)
    {
        JsonNode? payload = value.Kind switch
        {
            ToriiEntrypointValueTypeNodeKindV1.Struct => BuildStructNode(
                value.StructValue ?? throw new JsonException($"{context} struct metadata is required."),
                $"{context}.value"),
            ToriiEntrypointValueTypeNodeKindV1.Tuple => value.TupleArity is >= 2
                ? JsonValue.Create(value.TupleArity.Value)
                : throw new JsonException($"{context} tuple arity must be at least 2."),
            ToriiEntrypointValueTypeNodeKindV1.Option or ToriiEntrypointValueTypeNodeKindV1.Result => null,
            ToriiEntrypointValueTypeNodeKindV1.List => BuildListNode(
                value.ListValue ?? throw new JsonException($"{context} list metadata is required."),
                $"{context}.value"),
            ToriiEntrypointValueTypeNodeKindV1.Leaf => new JsonObject
            {
                ["kind"] = (value.LeafKind ?? throw new JsonException($"{context} leaf kind is required.")).ToString(),
                ["value"] = null,
            },
            _ => throw new JsonException($"{context} contains an unsupported node."),
        };
        return new JsonObject { ["kind"] = value.Kind.ToString(), ["value"] = payload };
    }

    private static JsonObject BuildStructNode(ToriiEntrypointStructTypeNodeV1 value, string context)
    {
        if (!IsCanonicalBoundaryIdentifier(value.Name)
            || value.Fields.Count == 0
            || value.Fields.Any(field => !IsCanonicalBoundaryIdentifier(field)))
        {
            throw new JsonException($"{context} must use canonical Kotodama identifiers.");
        }
        RequireUnique(value.Fields, $"{context}.fields");
        return new JsonObject
        {
            ["name"] = value.Name,
            ["fields"] = BuildStringArray(value.Fields, $"{context}.fields"),
        };
    }

    private static JsonObject BuildListNode(ToriiEntrypointListTypeNodeV1 value, string context)
    {
        if (value.Capacity is < 1 or > 64)
        {
            throw new JsonException($"{context}.capacity must be in 1..64.");
        }
        return new JsonObject
        {
            ["capacity"] = value.Capacity,
        };
    }

    private static JsonObject BuildTrigger(ToriiContractTriggerDescriptor value, string context)
    {
        ValidateCanonicalBase64(value.FilterBase64, $"{context}.filter");
        if (value.Authority is not null)
        {
            try
            {
                _ = ToriiExplorerDirectMetadata.RequireCanonicalAccountId(value.Authority, $"{context}.authority");
            }
            catch (ArgumentException exception)
            {
                throw new JsonException($"{context}.authority must be a canonical I105 account id.", exception);
            }
        }
        return new JsonObject
        {
            ["id"] = RequireExact(value.Id, $"{context}.id"),
            ["repeats"] = value.Repeats.Kind switch
            {
                ToriiContractTriggerRepeatsKind.Indefinitely when value.Repeats.Exactly is null =>
                    new JsonObject { ["Indefinitely"] = null },
                ToriiContractTriggerRepeatsKind.Exactly when value.Repeats.Exactly is not null =>
                    new JsonObject { ["Exactly"] = JsonValue.Create(value.Repeats.Exactly.Value) },
                _ => throw new JsonException($"{context}.repeats is inconsistent."),
            },
            ["filter"] = value.FilterBase64,
            ["authority"] = value.Authority,
            ["metadata"] = value.Metadata,
            ["callback"] = BuildCallback(value.Callback, $"{context}.callback"),
        };
    }

    private static JsonObject BuildCallback(ToriiContractTriggerCallback value, string context)
    {
        if (!IsCanonicalEntrypointName(value.Entrypoint))
        {
            throw new JsonException($"{context}.entrypoint must be a canonical Kotodama selector.");
        }
        ValidateExactStringOptional(value.Namespace, $"{context}.namespace");
        return new JsonObject { ["namespace"] = value.Namespace, ["entrypoint"] = value.Entrypoint };
    }

    private static JsonObject BuildState(ToriiContractStateDescriptor value, string context)
    {
        if (!IsCanonicalDeclarationIdentifier(value.Name))
        {
            throw new JsonException($"{context}.name must be a canonical Kotodama declaration identifier.");
        }
        return new JsonObject
        {
            ["name"] = value.Name,
            ["type_name"] = RequireExact(value.TypeName, $"{context}.type_name"),
        };
    }

    private static JsonObject BuildErrorCode(ToriiContractErrorCodeDescriptor value, string context)
    {
        if (!IsCanonicalDeclarationIdentifier(value.Namespace)
            || !IsCanonicalBoundaryIdentifier(value.Name)
            || value.Code == 0)
        {
            throw new JsonException(
                $"{context} must contain canonical Kotodama names and a non-zero code.");
        }
        return new JsonObject
        {
            ["namespace"] = value.Namespace,
            ["name"] = value.Name,
            ["code"] = value.Code,
        };
    }

    private static JsonObject BuildKotobaEntry(ToriiContractKotobaTranslationEntry value, string context)
    {
        RequireUnique(value.Translations.Select(translation => translation.Language), $"{context}.translations");
        return new JsonObject
        {
            ["msg_id"] = RequireExact(value.MessageId, $"{context}.msg_id"),
            ["translations"] = BuildArray(value.Translations, BuildKotobaTranslation, $"{context}.translations"),
        };
    }

    private static JsonObject BuildKotobaTranslation(ToriiContractKotobaTranslation value, string context)
    {
        return new JsonObject
        {
            ["lang"] = RequireExact(value.Language, $"{context}.lang"),
            ["text"] = value.Text ?? throw new JsonException($"{context}.text is required."),
        };
    }

    private static JsonObject BuildProvenance(ToriiContractManifestProvenance value, string context)
    {
        return new JsonObject
        {
            ["signer"] = RequireExact(value.Signer, $"{context}.signer"),
            ["signature"] = RequireExact(value.Signature, $"{context}.signature"),
        };
    }

    private static JsonArray? BuildOptionalArray<T>(
        IReadOnlyList<T>? values,
        Func<T, string, JsonObject> builder,
        string context)
    {
        return values is null ? null : BuildArray(values, builder, context);
    }

    private static JsonArray BuildArray<T>(
        IReadOnlyList<T> values,
        Func<T, string, JsonObject> builder,
        string context)
    {
        var result = new JsonArray();
        for (var index = 0; index < values.Count; index++)
        {
            result.Add(builder(values[index], $"{context}[{index}]"));
        }
        return result;
    }

    private static JsonArray BuildStringArray(IReadOnlyList<string> values, string context)
    {
        var result = new JsonArray();
        for (var index = 0; index < values.Count; index++)
        {
            result.Add(RequireExact(values[index], $"{context}[{index}]"));
        }
        return result;
    }

    private static IReadOnlyList<T>? OptionalObjectList<T>(
        JsonObject root,
        string name,
        string context,
        Func<JsonObject, string, T> parser)
    {
        return !root.ContainsKey(name) || root[name] is null
            ? null
            : RequiredObjectList(root, name, context, parser);
    }

    private static IReadOnlyList<T> DefaultObjectList<T>(
        JsonObject root,
        string name,
        string context,
        Func<JsonObject, string, T> parser)
    {
        if (!root.ContainsKey(name))
        {
            return Array.Empty<T>();
        }
        return RequiredObjectList(root, name, context, parser);
    }

    private static IReadOnlyList<T> RequiredObjectList<T>(
        JsonObject root,
        string name,
        string context,
        Func<JsonObject, string, T> parser)
    {
        if (root[name] is not JsonArray array)
        {
            throw new JsonException($"{context} must be an array.");
        }
        var result = new List<T>(array.Count);
        for (var index = 0; index < array.Count; index++)
        {
            if (array[index] is not JsonObject item)
            {
                throw new JsonException($"{context}[{index}] must be an object.");
            }
            result.Add(parser(item, $"{context}[{index}]"));
        }
        return result;
    }

    private static IReadOnlyList<string> RequiredStringList(JsonObject root, string name, string context)
    {
        if (!root.ContainsKey(name))
        {
            throw new JsonException($"{context} is required.");
        }
        return ReadStringList(root[name], context);
    }

    private static IReadOnlyList<string> DefaultStringList(JsonObject root, string name, string context)
    {
        return !root.ContainsKey(name) ? Array.Empty<string>() : ReadStringList(root[name], context);
    }

    private static IReadOnlyList<string> ReadStringList(JsonNode? node, string context)
    {
        if (node is not JsonArray array)
        {
            throw new JsonException($"{context} must be an array.");
        }
        var result = new List<string>(array.Count);
        for (var index = 0; index < array.Count; index++)
        {
            result.Add(ReadExactString(array[index], $"{context}[{index}]"));
        }
        return result;
    }

    private static JsonObject RequiredObject(JsonObject root, string name, string context)
    {
        return root.TryGetPropertyValue(name, out var value) && value is JsonObject objectValue
            ? objectValue
            : throw new JsonException($"{context} must be an object.");
    }

    private static JsonObject? OptionalObject(JsonObject root, string name, string context)
    {
        if (!root.TryGetPropertyValue(name, out var value) || value is null)
        {
            return null;
        }
        return value as JsonObject ?? throw new JsonException($"{context} must be an object.");
    }

    private static string RequiredExactString(JsonObject root, string name, string context)
    {
        return root.ContainsKey(name)
            ? ReadExactString(root[name], context)
            : throw new JsonException($"{context} is required.");
    }

    private static string RequiredStringAllowEmpty(JsonObject root, string name, string context)
    {
        if (!root.TryGetPropertyValue(name, out var value)
            || value is not JsonValue jsonValue
            || !jsonValue.TryGetValue<string>(out var text))
        {
            throw new JsonException($"{context} must be a string.");
        }
        return text;
    }

    private static string? OptionalExactString(JsonObject root, string name, string context)
    {
        return !root.TryGetPropertyValue(name, out var value) || value is null
            ? null
            : ReadExactString(value, context);
    }

    private static string ReadExactString(JsonNode? node, string context)
    {
        if (node is not JsonValue value || !value.TryGetValue<string>(out var text))
        {
            throw new JsonException($"{context} must be a string.");
        }
        return RequireExact(text, context);
    }

    private static string RequireExact(string? text, string context)
    {
        if (string.IsNullOrWhiteSpace(text))
        {
            throw new JsonException($"{context} must be a non-empty string.");
        }
        if (!string.Equals(text.Trim(), text, StringComparison.Ordinal))
        {
            throw new JsonException($"{context} must not contain surrounding whitespace.");
        }
        if (text.Any(char.IsControl))
        {
            throw new JsonException($"{context} must not contain control characters.");
        }
        return text;
    }

    private static void ValidateExactStringOptional(string? text, string context)
    {
        if (text is not null)
        {
            _ = RequireExact(text, context);
        }
    }

    private static bool? OptionalBoolean(JsonObject root, string name, string context)
    {
        if (!root.TryGetPropertyValue(name, out var value) || value is null)
        {
            return null;
        }
        return value is JsonValue scalar && scalar.TryGetValue<bool>(out var boolean)
            ? boolean
            : throw new JsonException($"{context} must be a boolean.");
    }

    private static ulong? OptionalUInt64(JsonObject root, string name, string context)
    {
        if (!root.TryGetPropertyValue(name, out var value) || value is null)
        {
            return null;
        }
        return ReadUInt64(value, context);
    }

    private static uint RequiredUInt32(JsonObject root, string name, string context)
    {
        var value = ReadUInt64(root.ContainsKey(name) ? root[name] : null, context);
        return value <= uint.MaxValue
            ? (uint)value
            : throw new JsonException($"{context} must fit in an unsigned 32-bit integer.");
    }

    private static ushort RequiredUInt16(JsonObject root, string name, string context)
    {
        var value = ReadUInt64(root.ContainsKey(name) ? root[name] : null, context);
        return value <= ushort.MaxValue
            ? (ushort)value
            : throw new JsonException($"{context} must fit in an unsigned 16-bit integer.");
    }

    private static byte RequiredByte(JsonObject root, string name, string context)
    {
        var value = ReadUInt64(root.ContainsKey(name) ? root[name] : null, context);
        return value <= byte.MaxValue
            ? (byte)value
            : throw new JsonException($"{context} must fit in an unsigned byte.");
    }

    private static ulong ReadUInt64(JsonNode? node, string context)
    {
        if (node is not JsonValue value || !value.TryGetValue<ulong>(out var number))
        {
            throw new JsonException($"{context} must be an unsigned integer.");
        }
        return number;
    }

    private static string? OptionalManifestHash(JsonObject root, string name, string context)
    {
        if (!root.TryGetPropertyValue(name, out var value) || value is null)
        {
            return null;
        }
        return ParseManifestHash(ReadStringAllowingFormat(value, context), context);
    }

    private static string? OptionalConvenienceHash(JsonObject root, string name, string context)
    {
        if (!root.TryGetPropertyValue(name, out var value) || value is null)
        {
            return null;
        }
        var hash = ReadStringAllowingFormat(value, context);
        ValidateConvenienceHash(hash, context);
        return hash;
    }

    private static string ReadStringAllowingFormat(JsonNode node, string context)
    {
        return node is JsonValue value && value.TryGetValue<string>(out var text)
            ? text
            : throw new JsonException($"{context} must be a string.");
    }

    private static string ParseManifestHash(string value, string context)
    {
        if (value.Length != 74
            || !value.StartsWith("hash:", StringComparison.Ordinal)
            || value[69] != '#')
        {
            throw new JsonException($"{context} must be a canonical checksummed Norito Hash literal.");
        }
        var body = value.Substring(5, 64);
        var checksum = value.Substring(70, 4);
        if (body.Any(character => !IsUpperHex(character))
            || checksum.Any(character => !IsUpperHex(character))
            || !ushort.TryParse(checksum, NumberStyles.HexNumber, CultureInfo.InvariantCulture, out var supplied)
            || supplied != Crc16(Encoding.ASCII.GetBytes($"hash:{body}")))
        {
            throw new JsonException($"{context} has a malformed or invalid Norito Hash checksum.");
        }
        var normalized = body.ToLowerInvariant();
        ValidateMarkerBit(normalized, context);
        return normalized;
    }

    private static string FormatManifestHash(string value, string context)
    {
        ValidateConvenienceHash(value, context);
        var body = value.ToUpperInvariant();
        var checksum = Crc16(Encoding.ASCII.GetBytes($"hash:{body}"));
        return $"hash:{body}#{checksum:X4}";
    }

    private static void ValidateConvenienceHash(string? value, string context)
    {
        if (value is null)
        {
            return;
        }
        if (value.Length != 64 || value.Any(character => !IsLowerHex(character)))
        {
            throw new JsonException($"{context} must be canonical lowercase 64-hex.");
        }
        ValidateMarkerBit(value, context);
    }

    private static void ValidateMarkerBit(string value, string context)
    {
        if (!byte.TryParse(value.AsSpan(value.Length - 2), NumberStyles.HexNumber, CultureInfo.InvariantCulture, out var last)
            || (last & 1) != 1)
        {
            throw new JsonException($"{context} must set the Iroha Hash marker bit.");
        }
    }

    private static ushort Crc16(ReadOnlySpan<byte> bytes)
    {
        var crc = 0xffff;
        foreach (var value in bytes)
        {
            crc ^= value << 8;
            for (var bit = 0; bit < 8; bit++)
            {
                crc = (crc & 0x8000) != 0
                    ? ((crc << 1) ^ 0x1021) & 0xffff
                    : (crc << 1) & 0xffff;
            }
        }
        return (ushort)crc;
    }

    private static void ValidateCanonicalBase64(string value, string context)
    {
        try
        {
            var bytes = Convert.FromBase64String(value);
            if (bytes.Length == 0 || Convert.ToBase64String(bytes) != value)
            {
                throw new JsonException($"{context} must be non-empty canonical base64.");
            }
        }
        catch (FormatException exception)
        {
            throw new JsonException($"{context} must be canonical base64.", exception);
        }
    }

    private static void EnsureOnly(JsonObject root, string context, params string[] allowed)
    {
        var names = allowed.ToHashSet(StringComparer.Ordinal);
        foreach (var property in root)
        {
            if (!names.Contains(property.Key))
            {
                throw new JsonException($"{context} contains unknown field `{property.Key}`.");
            }
        }
    }

    private static void RequireUnique(IEnumerable<string> values, string context)
    {
        var seen = new HashSet<string>(StringComparer.Ordinal);
        foreach (var value in values)
        {
            if (!seen.Add(value))
            {
                throw new JsonException($"{context} must not contain duplicates.");
            }
        }
    }

    private static bool IsCanonicalBoundaryIdentifier(string value)
    {
        return HasIdentifierSyntax(value) && !Keywords.Contains(value);
    }

    private static bool IsCanonicalDeclarationIdentifier(string value)
    {
        return IsCanonicalBoundaryIdentifier(value)
            && !ReservedDeclarationNames.Contains(value)
            && !value.StartsWith("__kotodama_link_", StringComparison.Ordinal);
    }

    private static bool IsCanonicalEntrypointName(string value)
    {
        return value is "hajimari" or "始まり" or "kaizen" or "改善"
            || IsCanonicalBoundaryIdentifier(value);
    }

    private static bool HasIdentifierSyntax(string value)
    {
        return value.Length > 0
            && (value[0] == '_' || IsAsciiLetter(value[0]))
            && value.Skip(1).All(character => character == '_' || IsAsciiLetter(character) || char.IsAsciiDigit(character));
    }

    private static bool IsAsciiLetter(char value)
    {
        return value is >= 'A' and <= 'Z' or >= 'a' and <= 'z';
    }

    private static bool IsUpperHex(char value)
    {
        return value is >= '0' and <= '9' or >= 'A' and <= 'F';
    }

    private static bool IsLowerHex(char value)
    {
        return value is >= '0' and <= '9' or >= 'a' and <= 'f';
    }

    private readonly record struct TypeAnalysis(
        int NextIndex,
        int NodeCount,
        int WordCount,
        int MaxDepth,
        string TypeName,
        string? CoreQueryViewName,
        string? ListElementCoreQueryViewName);

    private readonly record struct AnalysisFrame(int RemainingChildren, bool SuppressWords);

    private readonly record struct RenderedType(
        string TypeName,
        string? CoreQueryViewName,
        byte? ListCapacity,
        string? ListElementCoreQueryViewName);
}
