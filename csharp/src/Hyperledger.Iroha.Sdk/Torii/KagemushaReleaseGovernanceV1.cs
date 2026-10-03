using System.Text.Json;
using System.Text.RegularExpressions;
using Hyperledger.Iroha.Address;

namespace Hyperledger.Iroha.Torii;

/// <summary>One exact public Parliament proposal for a governed KAGEMUSHA verifier release.</summary>
/// <remarks>This SDK shape check does not authenticate release evidence or threshold signatures;
/// native Parliament admission owns those decisions.</remarks>
public abstract class KagemushaReleaseProposalV1
{
    internal KagemushaReleaseProposalV1(string operatorId, NetworkId networkId, JsonElement predecessor)
    {
        ProposalOperator = operatorId;
        NetworkId = networkId;
        ExpectedPredecessor = predecessor.Clone();
    }

    /// <summary>Canonical transaction authority that proposed this release transition.</summary>
    public string ProposalOperator { get; }

    /// <summary>Exact genesis-derived network identity.</summary>
    public NetworkId NetworkId { get; }

    /// <summary>Complete, schema-checked predecessor registry.</summary>
    public JsonElement ExpectedPredecessor { get; }

    /// <summary>Parse one release-install, first-activation, or unused standby-retirement proposal.</summary>
    /// <exception cref="JsonException">The proposal is malformed or outside the closed V1 shape.</exception>
    public static KagemushaReleaseProposalV1 Parse(ReadOnlySpan<byte> utf8Json) =>
        KagemushaReleaseGovernanceJsonV1.Parse(utf8Json);
}

/// <summary>One exact proposed installation of an inactive verifier release.</summary>
public sealed class KagemushaReleaseInstallProposalV1 : KagemushaReleaseProposalV1
{
    internal KagemushaReleaseInstallProposalV1(
        string operatorId,
        NetworkId networkId,
        JsonElement predecessor,
        JsonElement manifest,
        JsonElement receipt,
        JsonElement attestation)
        : base(operatorId, networkId, predecessor)
    {
        Manifest = manifest.Clone();
        Receipt = receipt.Clone();
        Attestation = attestation.Clone();
    }

    /// <summary>Complete immutable release manifest, checked against the closed V1 schema.</summary>
    public JsonElement Manifest { get; }

    /// <summary>Complete internal validation receipt, checked against the closed V1 schema.</summary>
    public JsonElement Receipt { get; }

    /// <summary>Complete release attestation, checked against the closed V1 schema.</summary>
    public JsonElement Attestation { get; }
}

/// <summary>One exact first activation of the sole installed standby release.</summary>
public sealed class KagemushaReleaseActivateProposalV1 : KagemushaReleaseProposalV1
{
    private readonly byte[] successorReleaseId;

    internal KagemushaReleaseActivateProposalV1(
        string operatorId,
        NetworkId networkId,
        JsonElement predecessor,
        byte[] successorReleaseId)
        : base(operatorId, networkId, predecessor) =>
        this.successorReleaseId = (byte[])successorReleaseId.Clone();

    /// <summary>Defensive copy of the selected standby release identifier.</summary>
    public byte[] SuccessorReleaseId => (byte[])successorReleaseId.Clone();
}

/// <summary>One exact retirement of an unused governed standby release.</summary>
public sealed class KagemushaReleaseRetireProposalV1 : KagemushaReleaseProposalV1
{
    private readonly byte[] standbyReleaseId;

    internal KagemushaReleaseRetireProposalV1(
        string operatorId, NetworkId networkId, JsonElement predecessor, byte[] standbyReleaseId)
        : base(operatorId, networkId, predecessor) =>
        this.standbyReleaseId = (byte[])standbyReleaseId.Clone();

    /// <summary>Defensive copy of the selected unused standby release identifier.</summary>
    public byte[] StandbyReleaseId => (byte[])standbyReleaseId.Clone();
}

internal static class KagemushaReleaseGovernanceJsonV1
{
    private const int MaximumJsonBytes = 16 * 1024 * 1024;
    private const ulong MaximumExactJsonInteger = 9_007_199_254_740_991;
    private const string SchemaResource = "Hyperledger.Iroha.Torii.KagemushaReleaseSchemasV1.json";
    private const string SchemaPrefix = "#/components/schemas/";
    private static readonly JsonDocument Schemas = LoadSchemas();

    internal static KagemushaReleaseProposalV1 Parse(ReadOnlySpan<byte> utf8Json)
    {
        if (utf8Json.Length == 0 || utf8Json.Length > MaximumJsonBytes)
        {
            throw new JsonException("KAGEMUSHA release proposal exceeds the V1 JSON bound.");
        }
        using var document = JsonDocument.Parse(utf8Json.ToArray(), new JsonDocumentOptions
        {
            MaxDepth = 64,
            CommentHandling = JsonCommentHandling.Disallow,
            AllowTrailingCommas = false,
        });
        var root = ExactObject(document.RootElement, ["kind", "payload"], "release proposal");
        var kind = Text(root.GetProperty("kind"), "release proposal.kind");
        var payload = root.GetProperty("payload");
        var fields = kind switch
        {
            "KagemushaVerifierReleaseInstall" => new[]
            {
                "proposal_operator", "network_id", "expected_predecessor", "manifest", "receipt", "attestation",
            },
            "KagemushaVerifierReleaseActivate" => new[]
            {
                "proposal_operator", "network_id", "expected_predecessor", "successor_release_id",
            },
            "KagemushaVerifierReleaseRetire" => new[]
            {
                "proposal_operator", "network_id", "expected_predecessor", "standby_release_id",
            },
            _ => throw new JsonException("Unsupported KAGEMUSHA release proposal kind."),
        };
        payload = ExactObject(payload, fields, $"{kind}.payload");
        var operatorId = AccountId(payload.GetProperty("proposal_operator"), $"{kind}.proposal_operator");
        var networkId = ParseNetworkId(payload.GetProperty("network_id"), $"{kind}.network_id");
        var predecessor = payload.GetProperty("expected_predecessor");
        ValidateSchema("GovernanceKagemushaGovernedVerifierRegistryV1", predecessor);
        if (predecessor.GetProperty("authority_policy").ValueKind == JsonValueKind.Null)
        {
            throw new JsonException("KAGEMUSHA release predecessor requires a governed signer policy.");
        }

        if (kind == "KagemushaVerifierReleaseInstall")
        {
            var manifest = payload.GetProperty("manifest");
            var receipt = payload.GetProperty("receipt");
            var attestation = payload.GetProperty("attestation");
            ValidateSchema("GovernanceKagemushaReleaseManifestV1", manifest);
            ValidateSchema("GovernanceKagemushaInternalValidationReceiptV1", receipt);
            ValidateSchema("GovernanceKagemushaReleaseAttestationV1", attestation);
            if (!Bytes32(manifest.GetProperty("release_id"), "manifest.release_id").AsSpan().SequenceEqual(
                Bytes32(attestation.GetProperty("subject").GetProperty("release_id"), "attestation.subject.release_id")))
            {
                throw new JsonException("Attestation release id must match the manifest release id.");
            }
            return new KagemushaReleaseInstallProposalV1(
                operatorId, networkId, predecessor, manifest, receipt, attestation);
        }

        if (kind == "KagemushaVerifierReleaseRetire")
        {
            ValidateRetirementSignerPolicy(predecessor.GetProperty("authority_policy"));
            var retired = Bytes32(payload.GetProperty("standby_release_id"), "standby_release_id");
            if (retired.All(static value => value == 0))
            {
                throw new JsonException("Retirement requires a nonzero standby release id.");
            }
            byte[]? previous = null;
            var active = new List<string>();
            var selectedStandby = false;
            var rows = predecessor.GetProperty("releases");
            var pointer = predecessor.GetProperty("active_release_id");
            foreach (var row in rows.EnumerateArray())
            {
                var identity = Bytes32(row.GetProperty("release_id"), "release_id");
                if (previous is not null && previous.AsSpan().SequenceCompareTo(identity) >= 0)
                {
                    throw new JsonException("Retirement releases must be strictly ordered and unique.");
                }
                previous = identity;
                foreach (var field in row.EnumerateObject())
                {
                    if (field.Name != "status" && Bytes32(field.Value, field.Name).All(static value => value == 0))
                    {
                        throw new JsonException("Retirement releases require nonzero identities.");
                    }
                }
                var status = Unsigned(row.GetProperty("status"), "release.status");
                if (status == 1) active.Add(Convert.ToHexString(identity));
                if (pointer.ValueKind == JsonValueKind.Null && status != 2)
                {
                    throw new JsonException("Inactive retirement predecessor contains a non-standby release.");
                }
                selectedStandby |= status == 2 && identity.AsSpan().SequenceEqual(retired);
            }
            if (active.Count != (pointer.ValueKind == JsonValueKind.Null ? 0 : 1)
                || (pointer.ValueKind != JsonValueKind.Null && active[0] != pointer.GetString()))
            {
                throw new JsonException("Retirement active pointer must select the unique active release.");
            }
            if (!selectedStandby)
            {
                throw new JsonException("Retirement id must select an unused standby release.");
            }
            return new KagemushaReleaseRetireProposalV1(operatorId, networkId, predecessor, retired);
        }

        var selected = payload.GetProperty("successor_release_id");
        ValidateSchema("GovernanceKagemushaBytes32V1", selected);
        var selectedId = Bytes32(selected, "successor_release_id");
        if (predecessor.GetProperty("active_release_id").ValueKind != JsonValueKind.Null)
        {
            throw new JsonException("First activation requires an inactive predecessor.");
        }
        var releases = predecessor.GetProperty("releases");
        if (releases.GetArrayLength() != 1)
        {
            throw new JsonException("First activation requires exactly one standby release.");
        }
        var standby = releases[0];
        if (Unsigned(standby.GetProperty("status"), "standby.status") != 2
            || !selectedId.AsSpan().SequenceEqual(Bytes32(standby.GetProperty("release_id"), "standby.release_id")))
        {
            throw new JsonException("Successor id must select the sole standby release.");
        }
        return new KagemushaReleaseActivateProposalV1(operatorId, networkId, predecessor, selectedId);
    }

    private static void ValidateRetirementSignerPolicy(JsonElement policy)
    {
        var signers = policy.GetProperty("authorized_signers");
        var threshold = Unsigned(policy.GetProperty("threshold"), "authority_policy.threshold");
        if (threshold > (ulong)signers.GetArrayLength())
        {
            throw new JsonException("Retirement signer threshold exceeds its signer count.");
        }
        (int Ordinal, byte[] Payload)? previous = null;
        foreach (var signer in signers.EnumerateArray())
        {
            var current = RetirementSignerPublicKey(Text(signer, "authority_policy.authorized_signers"));
            if (previous is { } prior && (prior.Ordinal > current.Ordinal
                || prior.Ordinal == current.Ordinal && prior.Payload.AsSpan().SequenceCompareTo(current.Payload) >= 0))
            {
                throw new JsonException("Retirement signer keys must be strictly ordered and unique.");
            }
            previous = current;
        }
    }

    private static (int Ordinal, byte[] Payload) RetirementSignerPublicKey(string literal)
    {
        if (literal.Length == 0 || literal.Length > 2 * (ushort.MaxValue + 6) || literal.Length % 2 != 0)
        {
            throw new JsonException("Retirement signer must be a bounded canonical public-key multihash.");
        }
        try
        {
            var bytes = Convert.FromHexString(literal);
            var position = 0;
            var code = ReadPublicKeyVarint(bytes, ref position);
            var length = ReadPublicKeyVarint(bytes, ref position);
            if (length == 0 || length != bytes.Length - position)
            {
                throw new JsonException("Retirement signer multihash length differs.");
            }
            var (ordinal, curve) = code switch
            {
                0xed => (0, CurveId.Ed25519),
                0xe7 => (1, CurveId.Secp256k1),
                0xea => (2, CurveId.BlsNormal),
                0xeb => (3, CurveId.BlsSmall),
                0xee => (4, CurveId.MlDsa),
                0x1200 => (5, CurveId.Gost256A),
                0x1201 => (6, CurveId.Gost256B),
                0x1202 => (7, CurveId.Gost256C),
                0x1203 => (8, CurveId.Gost512A),
                0x1204 => (9, CurveId.Gost512B),
                0x1306 => (10, CurveId.Sm2),
                _ => throw new JsonException("Retirement signer has an unsupported public-key algorithm."),
            };
            var payload = bytes.AsSpan(position);
            if (literal != Convert.ToHexString(bytes.AsSpan(0, position)).ToLowerInvariant() + Convert.ToHexString(payload))
            {
                throw new JsonException("Retirement signer multihash spelling is noncanonical.");
            }
            // Reuse the mandatory original native public-key/controller validator;
            // this does not authenticate a governance signature or registry digest.
            AccountAddress.FromPublicKey(payload, curve);
            return (ordinal, payload.ToArray());
        }
        catch (FormatException error)
        {
            throw new JsonException("Retirement signer public key is malformed.", error);
        }
    }

    private static int ReadPublicKeyVarint(ReadOnlySpan<byte> bytes, ref int position)
    {
        var start = position;
        var value = 0;
        for (var shift = 0; shift <= 14 && position < bytes.Length; shift += 7)
        {
            var part = bytes[position++];
            value |= (part & 0x7f) << shift;
            if ((part & 0x80) == 0)
            {
                if (position - start > 1 && part == 0)
                {
                    throw new JsonException("Retirement signer multihash varint is nonminimal.");
                }
                return value;
            }
        }
        throw new JsonException("Retirement signer multihash varint is truncated or oversized.");
    }

    internal static void ValidateSchema(string name, JsonElement value)
    {
        if (!Schemas.RootElement.TryGetProperty(name, out var schema))
        {
            throw new JsonException($"Unknown KAGEMUSHA V1 release schema {name}.");
        }
        var visited = 0;
        Validate(value, schema, name, ref visited, 0);
    }

    private static void Validate(JsonElement value, JsonElement schema, string context, ref int visited, int depth)
    {
        if (++visited > 250_000 || depth > 64)
        {
            throw new JsonException($"{context} exceeds the release schema traversal bound.");
        }
        if (schema.TryGetProperty("$ref", out var reference))
        {
            var path = Text(reference, $"{context} schema reference");
            if (!path.StartsWith(SchemaPrefix, StringComparison.Ordinal)
                || !Schemas.RootElement.TryGetProperty(path[SchemaPrefix.Length..], out var target))
            {
                throw new JsonException($"{context} has an unknown release schema reference.");
            }
            Validate(value, target, context, ref visited, depth + 1);
        }
        if (schema.TryGetProperty("oneOf", out var alternatives))
        {
            var matching = 0;
            foreach (var choice in alternatives.EnumerateArray())
            {
                try
                {
                    Validate(value, choice, context, ref visited, depth + 1);
                    matching++;
                }
                catch (JsonException)
                {
                    // Exactly one complete alternative must accept this value.
                }
            }
            if (matching != 1)
            {
                throw new JsonException($"{context} must match exactly one V1 release shape.");
            }
        }
        if (schema.TryGetProperty("type", out var type))
        {
            switch (Text(type, $"{context} schema type"))
            {
                case "object":
                    ValidateObject(value, schema, context, ref visited, depth);
                    break;
                case "array":
                    ValidateArray(value, schema, context, ref visited, depth);
                    break;
                case "integer":
                    ValidateInteger(value, schema, context);
                    break;
                case "string":
                    ValidateString(value, schema, context);
                    break;
                case "null":
                    if (value.ValueKind != JsonValueKind.Null)
                    {
                        throw new JsonException($"{context} must be null.");
                    }
                    break;
                default:
                    throw new JsonException($"{context} has an unsupported V1 release schema type.");
            }
        }
        if (schema.TryGetProperty("const", out var constant) && !Equivalent(value, constant))
        {
            throw new JsonException($"{context} has the wrong V1 constant.");
        }
        if (schema.TryGetProperty("enum", out var enumeration)
            && !enumeration.EnumerateArray().Any(item => Equivalent(value, item)))
        {
            throw new JsonException($"{context} is outside the closed V1 enum.");
        }
        if (schema.TryGetProperty("not", out var prohibited))
        {
            var prohibitedMatch = false;
            try
            {
                Validate(value, prohibited, context, ref visited, depth + 1);
                prohibitedMatch = true;
            }
            catch (JsonException)
            {
                // The excluded schema did not match.
            }
            if (prohibitedMatch)
            {
                throw new JsonException($"{context} has a prohibited V1 value.");
            }
        }
    }

    private static void ValidateObject(JsonElement value, JsonElement schema, string context, ref int visited, int depth)
    {
        if (value.ValueKind != JsonValueKind.Object
            || !schema.TryGetProperty("additionalProperties", out var additional)
            || additional.ValueKind != JsonValueKind.False
            || !schema.TryGetProperty("properties", out var properties))
        {
            throw new JsonException($"{context} must match a closed V1 release object.");
        }
        var names = new HashSet<string>(StringComparer.Ordinal);
        foreach (var field in value.EnumerateObject())
        {
            if (!names.Add(field.Name) || !properties.TryGetProperty(field.Name, out var fieldSchema))
            {
                throw new JsonException($"{context} has a duplicate or unknown field {field.Name}.");
            }
            Validate(field.Value, fieldSchema, $"{context}.{field.Name}", ref visited, depth + 1);
        }
        if (schema.TryGetProperty("required", out var required))
        {
            foreach (var name in required.EnumerateArray())
            {
                var requiredName = Text(name, $"{context} required field");
                if (!names.Contains(requiredName))
                {
                    throw new JsonException($"{context} is missing required field {requiredName}.");
                }
            }
        }
    }

    private static void ValidateArray(JsonElement value, JsonElement schema, string context, ref int visited, int depth)
    {
        if (value.ValueKind != JsonValueKind.Array || !schema.TryGetProperty("items", out var items))
        {
            throw new JsonException($"{context} must be a closed V1 release array.");
        }
        var length = value.GetArrayLength();
        if ((schema.TryGetProperty("minItems", out var minimum) && length < minimum.GetInt32())
            || (schema.TryGetProperty("maxItems", out var maximum) && length > maximum.GetInt32()))
        {
            throw new JsonException($"{context} has an invalid V1 array length.");
        }
        var elements = value.EnumerateArray().ToArray();
        if (schema.TryGetProperty("uniqueItems", out var unique) && unique.ValueKind == JsonValueKind.True)
        {
            if (length > 256)
            {
                throw new JsonException($"{context} exceeds the bounded unique-array check.");
            }
            for (var index = 0; index < length; index++)
            {
                for (var prior = 0; prior < index; prior++)
                {
                    if (Equivalent(elements[index], elements[prior]))
                    {
                        throw new JsonException($"{context} contains duplicate items.");
                    }
                }
            }
        }
        for (var index = 0; index < length; index++)
        {
            Validate(elements[index], items, $"{context}[{index}]", ref visited, depth + 1);
        }
    }

    private static void ValidateInteger(JsonElement value, JsonElement schema, string context)
    {
        var number = Unsigned(value, context);
        if ((schema.TryGetProperty("minimum", out var minimum) && number < minimum.GetUInt64())
            || (schema.TryGetProperty("maximum", out var maximum) && number > maximum.GetUInt64()))
        {
            throw new JsonException($"{context} is outside its V1 integer range.");
        }
        if (schema.TryGetProperty("format", out var format))
        {
            var width = Text(format, $"{context} format") switch
            {
                "uint8" => 8,
                "uint16" => 16,
                "uint32" => 32,
                "uint64" => 64,
                _ => throw new JsonException($"{context} has an unsupported V1 integer format."),
            };
            if (width < 64 && number >= 1UL << width)
            {
                throw new JsonException($"{context} exceeds its unsigned integer width.");
            }
        }
    }

    private static void ValidateString(JsonElement value, JsonElement schema, string context)
    {
        var text = Text(value, context);
        if ((schema.TryGetProperty("minLength", out var minimum) && text.Length < minimum.GetInt32())
            || (schema.TryGetProperty("maxLength", out var maximum) && text.Length > maximum.GetInt32()))
        {
            throw new JsonException($"{context} has an invalid V1 string length.");
        }
        if (schema.TryGetProperty("pattern", out var pattern)
            && !Regex.IsMatch(text, $"\\A(?:{Text(pattern, context)})\\z", RegexOptions.CultureInvariant,
                TimeSpan.FromMilliseconds(100)))
        {
            throw new JsonException($"{context} is not a canonical V1 string.");
        }
    }

    private static JsonElement ExactObject(JsonElement value, string[] fields, string context)
    {
        if (value.ValueKind != JsonValueKind.Object)
        {
            throw new JsonException($"{context} must be an object.");
        }
        var expected = new HashSet<string>(fields, StringComparer.Ordinal);
        var seen = new HashSet<string>(StringComparer.Ordinal);
        foreach (var field in value.EnumerateObject())
        {
            if (!expected.Contains(field.Name) || !seen.Add(field.Name))
            {
                throw new JsonException($"{context} has an unknown or duplicate field {field.Name}.");
            }
        }
        if (seen.Count != fields.Length)
        {
            throw new JsonException($"{context} is missing required V1 fields.");
        }
        return value;
    }

    private static string Text(JsonElement value, string context) =>
        value.ValueKind == JsonValueKind.String
            ? value.GetString()!
            : throw new JsonException($"{context} must be a string.");

    private static string AccountId(JsonElement value, string context)
    {
        var literal = Text(value, context);
        try
        {
            return ToriiExplorerDirectMetadata.RequireCanonicalAccountId(literal, context);
        }
        catch (ArgumentException error)
        {
            throw new JsonException($"{context} must be a canonical account id.", error);
        }
    }

    private static NetworkId ParseNetworkId(JsonElement value, string context)
    {
        try
        {
            return NetworkId.Parse(Text(value, context));
        }
        catch (FormatException error)
        {
            throw new JsonException($"{context} must be a canonical network id.", error);
        }
    }

    private static ulong Unsigned(JsonElement value, string context)
    {
        if (value.ValueKind != JsonValueKind.Number)
        {
            throw new JsonException($"{context} must be an exact JSON unsigned integer.");
        }
        var raw = value.GetRawText();
        if (raw.Length == 0 || (raw.Length > 1 && raw[0] == '0')
            || raw.Any(static character => character is < '0' or > '9')
            || !value.TryGetUInt64(out var number)
            || number > MaximumExactJsonInteger)
        {
            throw new JsonException($"{context} must be an exact JSON unsigned integer.");
        }
        return number;
    }

    private static byte[] Bytes32(JsonElement value, string context)
    {
        if (value.ValueKind != JsonValueKind.Array || value.GetArrayLength() != 32)
        {
            throw new JsonException($"{context} must contain exactly 32 bytes.");
        }
        var result = new byte[32];
        for (var index = 0; index < 32; index++)
        {
            var number = Unsigned(value[index], $"{context}[{index}]");
            if (number > byte.MaxValue)
            {
                throw new JsonException($"{context}[{index}] exceeds one byte.");
            }
            result[index] = (byte)number;
        }
        return result;
    }

    private static bool Equivalent(JsonElement left, JsonElement right)
    {
        if (left.ValueKind != right.ValueKind)
        {
            return false;
        }
        return left.ValueKind switch
        {
            JsonValueKind.Object => ObjectEquivalent(left, right),
            JsonValueKind.Array => left.GetArrayLength() == right.GetArrayLength()
                && left.EnumerateArray().Zip(right.EnumerateArray()).All(pair => Equivalent(pair.First, pair.Second)),
            JsonValueKind.String => string.Equals(left.GetString(), right.GetString(), StringComparison.Ordinal),
            JsonValueKind.Number => string.Equals(left.GetRawText(), right.GetRawText(), StringComparison.Ordinal),
            JsonValueKind.True or JsonValueKind.False or JsonValueKind.Null => true,
            _ => false,
        };
    }

    private static bool ObjectEquivalent(JsonElement left, JsonElement right)
    {
        var fields = left.EnumerateObject().ToArray();
        if (fields.Length != right.EnumerateObject().Count())
        {
            return false;
        }
        return fields.All(field => right.TryGetProperty(field.Name, out var other)
            && Equivalent(field.Value, other));
    }

    private static JsonDocument LoadSchemas()
    {
        using var resource = typeof(KagemushaReleaseGovernanceJsonV1).Assembly
            .GetManifestResourceStream(SchemaResource)
            ?? throw new InvalidOperationException("Embedded KAGEMUSHA release schema is missing.");
        return JsonDocument.Parse(resource);
    }
}
