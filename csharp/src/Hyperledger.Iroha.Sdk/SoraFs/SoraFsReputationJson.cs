using System.Globalization;
using System.Text;
using System.Text.Json;

namespace Hyperledger.Iroha.SoraFs;

internal static class SoraFsReputationJson
{
    private static readonly Encoding StrictUtf8 = new UTF8Encoding(false, true);

    private static readonly string[] SnapshotFields =
    [
        "snapshot_id_hex",
        "generated_at_unix",
        "previous_snapshot_id_hex",
        "merkle_root_hex",
        "provider_count",
        "returned_provider_count",
        "limit",
        "truncated_providers",
        "alpha_bps",
        "current_score_weight_bps",
        "weights",
        "providers",
    ];

    private static readonly string[] ProviderResponseFields =
    [
        "snapshot_id_hex",
        "generated_at_unix",
        "merkle_root_hex",
        "provider",
        "proof",
    ];

    private static readonly string[] WeightsResponseFields =
    [
        "snapshot_id_hex",
        "generated_at_unix",
        "alpha_bps",
        "current_score_weight_bps",
        "weights",
    ];

    private static readonly string[] WeightsFields =
    [
        "version",
        "por_success_bps",
        "pdp_success_bps",
        "potr_success_bps",
        "latency_bps",
        "dispute_bps",
        "token_violation_bps",
        "repair_breach_bps",
    ];

    private static readonly string[] ProviderFields =
    [
        "provider_id",
        "score_bps",
        "degradation_flags",
        "raw_metrics",
        "raw_metrics_hash_hex",
    ];

    private static readonly string[] ProviderMetricsFields =
    [
        "version",
        "por_success_bps",
        "pdp_success_bps",
        "potr_success_bps",
        "latency_health_bps",
        "dispute_rate_bps",
        "token_violation_rate_bps",
        "repair_breach_rate_bps",
    ];

    private static readonly string[] DegradationFlagFields = ["flag", "value"];
    private static readonly string[] ProofFields =
        ["provider_id", "leaf_index", "leaf_count", "siblings_hex"];
    private static readonly string[] EventFields =
    [
        "version",
        "sequence",
        "snapshot_id_hex",
        "generated_at_unix",
        "merkle_root_hex",
        "provider_count",
        "previous_snapshot_id_hex",
    ];
    private static readonly string[] EventPageFields =
        ["since", "limit", "count", "next_since", "events"];

    private static readonly IReadOnlyDictionary<string, SoraFsReputationDegradationFlagNameV1>
        DegradationFlags =
            new Dictionary<string, SoraFsReputationDegradationFlagNameV1>(StringComparer.Ordinal)
            {
                ["reserve_warning"] = SoraFsReputationDegradationFlagNameV1.ReserveWarning,
                ["reserve_grace"] = SoraFsReputationDegradationFlagNameV1.ReserveGrace,
                ["reserve_delinquent"] = SoraFsReputationDegradationFlagNameV1.ReserveDelinquent,
                ["reserve_default"] = SoraFsReputationDegradationFlagNameV1.ReserveDefault,
                ["proof_success_below90"] = SoraFsReputationDegradationFlagNameV1.ProofSuccessBelow90,
                ["proof_success_below80"] = SoraFsReputationDegradationFlagNameV1.ProofSuccessBelow80,
                ["active_dispute"] = SoraFsReputationDegradationFlagNameV1.ActiveDispute,
                ["slashing_event"] = SoraFsReputationDegradationFlagNameV1.SlashingEvent,
                ["low_score"] = SoraFsReputationDegradationFlagNameV1.LowScore,
            };

    internal static SoraFsReputationSnapshotSummaryV1 ParseSnapshot(
        ReadOnlyMemory<byte> payload,
        int expectedLimit,
        string? expectedSnapshotIdHex,
        string context)
    {
        using var document = ParseDocument(payload, context);
        var root = RequireExactObject(document.RootElement, SnapshotFields, context);
        var snapshotId = RequireSnapshotId(root, "snapshot_id_hex", context);
        if (expectedSnapshotIdHex is not null
            && !string.Equals(snapshotId, expectedSnapshotIdHex, StringComparison.Ordinal))
        {
            throw Invalid($"{context} does not match the requested snapshot.");
        }

        var previousSnapshotId = RequireOptionalSnapshotId(
            root,
            "previous_snapshot_id_hex",
            context);
        if (string.Equals(previousSnapshotId, snapshotId, StringComparison.Ordinal))
        {
            throw Invalid($"{context}.previous_snapshot_id_hex must differ from snapshot_id_hex.");
        }

        var providerCount = RequireUnsigned(root, "provider_count", 1, 65_536, context);
        var returnedProviderCount = RequireUnsigned(
            root,
            "returned_provider_count",
            1,
            500,
            context);
        var limit = RequireUnsigned(root, "limit", 1, 500, context);
        if (limit != (ulong)expectedLimit)
        {
            throw Invalid($"{context}.limit does not match the requested limit.");
        }

        var providerValues = RequireArray(root, "providers", context);
        var providers = new List<SoraFsReputationProviderV1>(providerValues.GetArrayLength());
        var index = 0;
        foreach (var providerValue in providerValues.EnumerateArray())
        {
            providers.Add(ParseProvider(providerValue, $"{context}.providers[{index}]"));
            index++;
        }

        if ((ulong)providers.Count != returnedProviderCount
            || returnedProviderCount != Math.Min(providerCount, limit))
        {
            throw Invalid($"{context} provider counts are inconsistent.");
        }
        for (var providerIndex = 1; providerIndex < providers.Count; providerIndex++)
        {
            if (string.CompareOrdinal(
                    providers[providerIndex - 1].ProviderId,
                    providers[providerIndex].ProviderId) >= 0)
            {
                throw Invalid($"{context}.providers must be strictly ordered by provider_id.");
            }
        }

        var truncated = RequireBoolean(root, "truncated_providers", context);
        if (truncated != (providerCount > returnedProviderCount))
        {
            throw Invalid($"{context}.truncated_providers is inconsistent with provider counts.");
        }

        return new SoraFsReputationSnapshotSummaryV1(
            snapshotId,
            RequireUnsigned(root, "generated_at_unix", 1, ulong.MaxValue, context),
            previousSnapshotId,
            RequireDigest(root, "merkle_root_hex", context),
            providerCount,
            returnedProviderCount,
            limit,
            truncated,
            checked((ushort)RequireUnsigned(root, "alpha_bps", 8_500, 8_500, context)),
            checked((ushort)RequireUnsigned(
                root,
                "current_score_weight_bps",
                7_000,
                7_000,
                context)),
            ParseWeights(RequireProperty(root, "weights", context), $"{context}.weights"),
            providers);
    }

    internal static SoraFsReputationProviderResponseV1 ParseProviderResponse(
        ReadOnlyMemory<byte> payload,
        string expectedProviderId,
        string context)
    {
        using var document = ParseDocument(payload, context);
        var root = RequireExactObject(document.RootElement, ProviderResponseFields, context);
        var provider = ParseProvider(
            RequireProperty(root, "provider", context),
            $"{context}.provider");
        var proof = ParseProof(
            RequireProperty(root, "proof", context),
            $"{context}.proof");
        if (!string.Equals(provider.ProviderId, proof.ProviderId, StringComparison.Ordinal)
            || !string.Equals(provider.ProviderId, expectedProviderId, StringComparison.Ordinal))
        {
            throw Invalid($"{context} provider and proof do not bind the requested provider.");
        }

        return new SoraFsReputationProviderResponseV1(
            RequireSnapshotId(root, "snapshot_id_hex", context),
            RequireUnsigned(root, "generated_at_unix", 1, ulong.MaxValue, context),
            RequireDigest(root, "merkle_root_hex", context),
            provider,
            proof);
    }

    internal static SoraFsReputationWeightsResponseV1 ParseWeightsResponse(
        ReadOnlyMemory<byte> payload,
        string context)
    {
        using var document = ParseDocument(payload, context);
        var root = RequireExactObject(document.RootElement, WeightsResponseFields, context);
        return new SoraFsReputationWeightsResponseV1(
            RequireSnapshotId(root, "snapshot_id_hex", context),
            RequireUnsigned(root, "generated_at_unix", 1, ulong.MaxValue, context),
            checked((ushort)RequireUnsigned(root, "alpha_bps", 8_500, 8_500, context)),
            checked((ushort)RequireUnsigned(
                root,
                "current_score_weight_bps",
                7_000,
                7_000,
                context)),
            ParseWeights(RequireProperty(root, "weights", context), $"{context}.weights"));
    }

    internal static SoraFsReputationEventsResponseV1 ParseEventPage(
        ReadOnlyMemory<byte> payload,
        ulong? expectedSince,
        int expectedLimit,
        string context)
    {
        using var document = ParseDocument(payload, context);
        var root = RequireExactObject(document.RootElement, EventPageFields, context);
        var since = RequireOptionalUnsigned(root, "since", 0, ulong.MaxValue, context);
        if (since != expectedSince)
        {
            throw Invalid($"{context}.since does not match the requested cursor.");
        }

        var limit = RequireUnsigned(root, "limit", 1, 500, context);
        if (limit != (ulong)expectedLimit)
        {
            throw Invalid($"{context}.limit does not match the requested limit.");
        }

        var count = RequireUnsigned(root, "count", 0, 500, context);
        var eventValues = RequireArray(root, "events", context);
        var events = new List<SoraFsReputationSnapshotEventV1>(eventValues.GetArrayLength());
        var index = 0;
        foreach (var eventValue in eventValues.EnumerateArray())
        {
            events.Add(ParseEventElement(eventValue, $"{context}.events[{index}]"));
            index++;
        }
        if ((ulong)events.Count != count || count > limit)
        {
            throw Invalid($"{context}.count is inconsistent with events and limit.");
        }

        var nextSince = RequireOptionalUnsigned(root, "next_since", 1, ulong.MaxValue, context);
        ulong? expectedNextSince = events.Count == 0 ? null : events[^1].Sequence;
        if (nextSince != expectedNextSince)
        {
            throw Invalid($"{context}.next_since must equal the final event sequence.");
        }

        var previousSequence = since ?? 0;
        for (var eventIndex = 0; eventIndex < events.Count; eventIndex++)
        {
            var current = events[eventIndex];
            if (eventIndex == 0)
            {
                if (current.Sequence <= previousSequence)
                {
                    throw Invalid($"{context} first sequence must be greater than since.");
                }
            }
            else
            {
                if (previousSequence == ulong.MaxValue
                    || current.Sequence != previousSequence + 1)
                {
                    throw Invalid($"{context} event sequences must be contiguous.");
                }

                var previous = events[eventIndex - 1];
                if (!string.Equals(
                        current.PreviousSnapshotIdHex,
                        previous.SnapshotIdHex,
                        StringComparison.Ordinal))
                {
                    throw Invalid($"{context} adjacent snapshot ids do not form a chain.");
                }
                if (current.GeneratedAtUnix <= previous.GeneratedAtUnix)
                {
                    throw Invalid($"{context} event timestamps must strictly increase.");
                }
            }

            previousSequence = current.Sequence;
        }

        return new SoraFsReputationEventsResponseV1(
            since,
            limit,
            count,
            nextSince,
            events);
    }

    internal static SoraFsReputationSnapshotEventV1 ParseCompactEvent(
        ReadOnlyMemory<byte> payload,
        string context)
    {
        if (payload.IsEmpty || ContainsJsonWhitespace(payload.Span))
        {
            throw Invalid($"{context} must be exact compact JSON.");
        }

        using var document = ParseDocument(payload, context);
        return ParseEventElement(document.RootElement, context);
    }

    internal static string RequireProviderId(string? value, string paramName)
    {
        if (string.IsNullOrEmpty(value)
            || value.Length > 256
            || !value.All(IsProviderIdCharacter)
            || value is "." or "..")
        {
            throw new ArgumentException(
                $"{paramName} must contain 1..256 ASCII characters from [A-Za-z0-9_.:-] and must not be a dot segment.",
                paramName);
        }

        return value!;
    }

    internal static string RequireSnapshotId(string? value, string paramName)
    {
        if (!IsLowerHex(value, 32) || value!.All(static character => character == '0'))
        {
            throw new ArgumentException(
                $"{paramName} must be a nonzero 32-character lowercase hexadecimal identifier.",
                paramName);
        }

        return value!;
    }

    private static SoraFsReputationWeightsV1 ParseWeights(JsonElement value, string context)
    {
        var root = RequireExactObject(value, WeightsFields, context);
        var por = RequireBasisPoints(root, "por_success_bps", context);
        var pdp = RequireBasisPoints(root, "pdp_success_bps", context);
        var potr = RequireBasisPoints(root, "potr_success_bps", context);
        var latency = RequireBasisPoints(root, "latency_bps", context);
        var dispute = RequireBasisPoints(root, "dispute_bps", context);
        var tokenViolation = RequireBasisPoints(root, "token_violation_bps", context);
        var repairBreach = RequireBasisPoints(root, "repair_breach_bps", context);
        var total = (uint)por + pdp + potr + latency + dispute + tokenViolation + repairBreach;
        if (total != 10_000)
        {
            throw Invalid($"{context} basis-point fields must sum to exactly 10000.");
        }

        return new SoraFsReputationWeightsV1(
            checked((byte)RequireUnsigned(root, "version", 1, 1, context)),
            por,
            pdp,
            potr,
            latency,
            dispute,
            tokenViolation,
            repairBreach);
    }

    private static SoraFsReputationProviderV1 ParseProvider(JsonElement value, string context)
    {
        var root = RequireExactObject(value, ProviderFields, context);
        var flagValues = RequireArray(root, "degradation_flags", context);
        if (flagValues.GetArrayLength() > 5)
        {
            throw Invalid($"{context}.degradation_flags must contain at most five entries.");
        }

        var flags = new List<SoraFsReputationDegradationFlagV1>(flagValues.GetArrayLength());
        var previousOrder = -1;
        var index = 0;
        foreach (var flagValue in flagValues.EnumerateArray())
        {
            var flagContext = $"{context}.degradation_flags[{index}]";
            var flagObject = RequireExactObject(flagValue, DegradationFlagFields, flagContext);
            if (RequireProperty(flagObject, "value", flagContext).ValueKind != JsonValueKind.Null)
            {
                throw Invalid($"{flagContext}.value must be null.");
            }

            var label = RequireString(flagObject, "flag", flagContext);
            if (!DegradationFlags.TryGetValue(label, out var flag))
            {
                throw Invalid($"{flagContext}.flag is unsupported.");
            }
            var order = (int)flag;
            if (order <= previousOrder)
            {
                throw Invalid(
                    $"{context}.degradation_flags must use canonical order without duplicates.");
            }
            previousOrder = order;
            flags.Add(new SoraFsReputationDegradationFlagV1(flag));
            index++;
        }

        return new SoraFsReputationProviderV1(
            RequireProviderId(root, "provider_id", context),
            checked((ushort)RequireUnsigned(root, "score_bps", 500, 9_900, context)),
            flags,
            ParseProviderMetrics(
                RequireProperty(root, "raw_metrics", context),
                $"{context}.raw_metrics"),
            RequireDigest(root, "raw_metrics_hash_hex", context));
    }

    private static SoraFsReputationProviderMetricsV1 ParseProviderMetrics(
        JsonElement value,
        string context)
    {
        var root = RequireExactObject(value, ProviderMetricsFields, context);
        return new SoraFsReputationProviderMetricsV1(
            checked((byte)RequireUnsigned(root, "version", 1, 1, context)),
            RequireBasisPoints(root, "por_success_bps", context),
            RequireBasisPoints(root, "pdp_success_bps", context),
            RequireBasisPoints(root, "potr_success_bps", context),
            RequireBasisPoints(root, "latency_health_bps", context),
            RequireBasisPoints(root, "dispute_rate_bps", context),
            RequireBasisPoints(root, "token_violation_rate_bps", context),
            RequireBasisPoints(root, "repair_breach_rate_bps", context));
    }

    private static SoraFsReputationMerkleProofV1 ParseProof(JsonElement value, string context)
    {
        var root = RequireExactObject(value, ProofFields, context);
        var leafIndex = RequireUnsigned(root, "leaf_index", 0, 65_535, context);
        var leafCount = RequireUnsigned(root, "leaf_count", 1, 65_536, context);
        if (leafIndex >= leafCount)
        {
            throw Invalid($"{context}.leaf_index must be less than leaf_count.");
        }

        var siblingValues = RequireArray(root, "siblings_hex", context);
        var expectedDepth = MerkleDepth(checked((uint)leafCount));
        if (siblingValues.GetArrayLength() != expectedDepth)
        {
            throw Invalid($"{context}.siblings_hex must have the exact Merkle depth.");
        }

        var siblings = new List<string>(expectedDepth);
        var index = 0;
        foreach (var sibling in siblingValues.EnumerateArray())
        {
            siblings.Add(RequireDigest(sibling, $"{context}.siblings_hex[{index}]"));
            index++;
        }

        return new SoraFsReputationMerkleProofV1(
            RequireProviderId(root, "provider_id", context),
            checked((uint)leafIndex),
            checked((uint)leafCount),
            siblings);
    }

    private static SoraFsReputationSnapshotEventV1 ParseEventElement(
        JsonElement value,
        string context)
    {
        var root = RequireExactObject(value, EventFields, context);
        var snapshotId = RequireSnapshotId(root, "snapshot_id_hex", context);
        var previousSnapshotId = RequireOptionalSnapshotId(
            root,
            "previous_snapshot_id_hex",
            context);
        if (string.Equals(snapshotId, previousSnapshotId, StringComparison.Ordinal))
        {
            throw Invalid($"{context}.previous_snapshot_id_hex must differ from snapshot_id_hex.");
        }

        return new SoraFsReputationSnapshotEventV1(
            checked((byte)RequireUnsigned(root, "version", 1, 1, context)),
            RequireUnsigned(root, "sequence", 1, ulong.MaxValue, context),
            snapshotId,
            RequireUnsigned(root, "generated_at_unix", 1, ulong.MaxValue, context),
            RequireDigest(root, "merkle_root_hex", context),
            checked((uint)RequireUnsigned(root, "provider_count", 1, 65_536, context)),
            previousSnapshotId);
    }

    private static JsonDocument ParseDocument(ReadOnlyMemory<byte> payload, string context)
    {
        if (payload.IsEmpty)
        {
            throw Invalid($"{context} returned an empty JSON body.");
        }
        if (payload.Length >= 3
            && payload.Span[0] == 0xef
            && payload.Span[1] == 0xbb
            && payload.Span[2] == 0xbf)
        {
            throw Invalid($"{context} must not contain a UTF-8 BOM.");
        }

        try
        {
            _ = StrictUtf8.GetString(payload.Span);
        }
        catch (DecoderFallbackException exception)
        {
            throw new JsonException($"{context} must be strict UTF-8.", exception);
        }

        JsonDocument document;
        try
        {
            document = JsonDocument.Parse(
                payload,
                new JsonDocumentOptions
                {
                    AllowTrailingCommas = false,
                    CommentHandling = JsonCommentHandling.Disallow,
                    MaxDepth = 64,
                });
        }
        catch (JsonException exception)
        {
            throw new JsonException($"{context} is not valid JSON.", exception);
        }

        try
        {
            RejectDuplicateProperties(document.RootElement, context);
            return document;
        }
        catch
        {
            document.Dispose();
            throw;
        }
    }

    private static void RejectDuplicateProperties(JsonElement value, string context)
    {
        switch (value.ValueKind)
        {
            case JsonValueKind.Object:
            {
                var fields = new HashSet<string>(StringComparer.Ordinal);
                foreach (var property in value.EnumerateObject())
                {
                    if (!fields.Add(property.Name))
                    {
                        throw Invalid($"{context}.{property.Name} appears more than once.");
                    }
                    RejectDuplicateProperties(property.Value, $"{context}.{property.Name}");
                }
                break;
            }
            case JsonValueKind.Array:
            {
                var index = 0;
                foreach (var item in value.EnumerateArray())
                {
                    RejectDuplicateProperties(item, $"{context}[{index}]");
                    index++;
                }
                break;
            }
        }
    }

    private static JsonElement RequireExactObject(
        JsonElement value,
        IEnumerable<string> expectedFields,
        string context)
    {
        if (value.ValueKind != JsonValueKind.Object)
        {
            throw Invalid($"{context} must be a JSON object.");
        }

        var expected = new HashSet<string>(expectedFields, StringComparer.Ordinal);
        var actual = value.EnumerateObject()
            .Select(static property => property.Name)
            .ToHashSet(StringComparer.Ordinal);
        if (!expected.SetEquals(actual))
        {
            var missing = expected
                .Where(field => !actual.Contains(field))
                .OrderBy(static field => field, StringComparer.Ordinal)
                .ToArray();
            var unknown = actual
                .Where(field => !expected.Contains(field))
                .OrderBy(static field => field, StringComparer.Ordinal)
                .ToArray();
            throw Invalid(
                $"{context} fields are not schema-closed; missing=[{string.Join(", ", missing)}] unknown=[{string.Join(", ", unknown)}].");
        }

        return value;
    }

    private static JsonElement RequireProperty(JsonElement root, string name, string context)
    {
        if (!root.TryGetProperty(name, out var value))
        {
            throw Invalid($"{context} omitted {name}.");
        }
        return value;
    }

    private static JsonElement RequireArray(JsonElement root, string name, string context)
    {
        var value = RequireProperty(root, name, context);
        if (value.ValueKind != JsonValueKind.Array)
        {
            throw Invalid($"{context}.{name} must be an array.");
        }
        return value;
    }

    private static string RequireString(JsonElement root, string name, string context)
    {
        var value = RequireProperty(root, name, context);
        if (value.ValueKind != JsonValueKind.String)
        {
            throw Invalid($"{context}.{name} must be a string.");
        }
        return value.GetString()
            ?? throw Invalid($"{context}.{name} must not be null.");
    }

    private static string RequireProviderId(JsonElement root, string name, string context)
    {
        var providerId = RequireString(root, name, context);
        if (providerId.Length is < 1 or > 256
            || !providerId.All(IsProviderIdCharacter)
            || providerId is "." or "..")
        {
            throw Invalid(
                $"{context}.{name} must contain 1..256 ASCII characters from [A-Za-z0-9_.:-] and must not be a dot segment.");
        }
        return providerId;
    }

    private static string RequireSnapshotId(JsonElement root, string name, string context)
    {
        var snapshotId = RequireString(root, name, context);
        if (!IsLowerHex(snapshotId, 32)
            || snapshotId.All(static character => character == '0'))
        {
            throw Invalid(
                $"{context}.{name} must be a nonzero 32-character lowercase hexadecimal identifier.");
        }
        return snapshotId;
    }

    private static string? RequireOptionalSnapshotId(
        JsonElement root,
        string name,
        string context)
    {
        var value = RequireProperty(root, name, context);
        return value.ValueKind == JsonValueKind.Null
            ? null
            : RequireSnapshotId(root, name, context);
    }

    private static string RequireDigest(JsonElement root, string name, string context)
    {
        var value = RequireProperty(root, name, context);
        return RequireDigest(value, $"{context}.{name}");
    }

    private static string RequireDigest(JsonElement value, string context)
    {
        if (value.ValueKind != JsonValueKind.String)
        {
            throw Invalid($"{context} must be a string.");
        }
        var digest = value.GetString();
        if (!IsLowerHex(digest, 64))
        {
            throw Invalid(
                $"{context} must be exactly 64 lowercase hexadecimal characters.");
        }
        return digest!;
    }

    private static bool RequireBoolean(JsonElement root, string name, string context)
    {
        var value = RequireProperty(root, name, context);
        return value.ValueKind switch
        {
            JsonValueKind.True => true,
            JsonValueKind.False => false,
            _ => throw Invalid($"{context}.{name} must be a boolean."),
        };
    }

    private static ushort RequireBasisPoints(JsonElement root, string name, string context) =>
        checked((ushort)RequireUnsigned(root, name, 0, 10_000, context));

    private static ulong? RequireOptionalUnsigned(
        JsonElement root,
        string name,
        ulong minimum,
        ulong maximum,
        string context)
    {
        var value = RequireProperty(root, name, context);
        return value.ValueKind == JsonValueKind.Null
            ? null
            : RequireUnsigned(value, minimum, maximum, $"{context}.{name}");
    }

    private static ulong RequireUnsigned(
        JsonElement root,
        string name,
        ulong minimum,
        ulong maximum,
        string context) =>
        RequireUnsigned(
            RequireProperty(root, name, context),
            minimum,
            maximum,
            $"{context}.{name}");

    private static ulong RequireUnsigned(
        JsonElement value,
        ulong minimum,
        ulong maximum,
        string context)
    {
        if (value.ValueKind != JsonValueKind.Number)
        {
            throw Invalid($"{context} must be a canonical unsigned integer.");
        }

        var literal = value.GetRawText();
        if (!IsCanonicalUnsignedDecimal(literal)
            || !ulong.TryParse(
                literal,
                NumberStyles.None,
                CultureInfo.InvariantCulture,
                out var parsed)
            || parsed < minimum
            || parsed > maximum)
        {
            throw Invalid(
                $"{context} must be a canonical unsigned integer in {minimum}..{maximum}.");
        }

        return parsed;
    }

    private static int MerkleDepth(uint leafCount)
    {
        var width = leafCount;
        var depth = 0;
        while (width > 1)
        {
            width = (width + 1) / 2;
            depth++;
        }
        return depth;
    }

    private static bool IsProviderIdCharacter(char value) =>
        value is >= 'A' and <= 'Z'
            or >= 'a' and <= 'z'
            or >= '0' and <= '9'
            or '_' or '.' or ':' or '-';

    private static bool IsLowerHex(string? value, int expectedLength) =>
        value is not null
        && value.Length == expectedLength
        && value.All(static character =>
            character is >= '0' and <= '9' or >= 'a' and <= 'f');

    private static bool IsCanonicalUnsignedDecimal(string value) =>
        value == "0"
        || (value.Length > 0
            && value[0] is >= '1' and <= '9'
            && value.All(static character => character is >= '0' and <= '9'));

    private static bool ContainsJsonWhitespace(ReadOnlySpan<byte> value)
    {
        foreach (var item in value)
        {
            if (item is (byte)' ' or (byte)'\t' or (byte)'\r' or (byte)'\n')
            {
                return true;
            }
        }
        return false;
    }

    private static JsonException Invalid(string message) => new(message);
}
