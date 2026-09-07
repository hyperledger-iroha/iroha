using System.Collections.ObjectModel;
using System.Text.Json;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Kaigi;

/// <summary>One original account's retained private sequence; absence of a commitment records a completed leave.</summary>
public sealed record KaigiPrivateParticipationV1
{
    internal KaigiPrivateParticipationV1(string originalAccount, ulong sequence, KaigiAuthorizationScalarV1? activeCommitment)
    {
        OriginalAccount = TransactionEncodingContext.CanonicalizeAccountId(originalAccount, nameof(originalAccount));
        if (sequence == 0 || (sequence == ulong.MaxValue && activeCommitment is not null))
            throw new ArgumentException("Invalid retained Kaigi participation sequence.", nameof(sequence));
        Sequence = sequence; ActiveCommitment = activeCommitment;
    }
    public string OriginalAccount { get; }
    public ulong Sequence { get; }
    public KaigiAuthorizationScalarV1? ActiveCommitment { get; }
}

/// <summary>A bounded retained participation projection. Core owns canonical ordering, rekey and authorization checks.</summary>
public sealed class KaigiPrivateParticipationLedgerV1
{
    internal KaigiPrivateParticipationLedgerV1(KaigiPrivateParticipationV1[] entries)
    {
        if (entries.Length > 4096 || entries.Select(static entry => entry.OriginalAccount).Distinct(StringComparer.Ordinal).Count() != entries.Length)
            throw new ArgumentException("Duplicate or oversized Kaigi participation ledger.", nameof(entries));
        var active = entries.Where(static entry => entry.ActiveCommitment is not null).Select(static entry => entry.ActiveCommitment!).ToArray();
        if (active.Distinct().Count() != active.Length) throw new ArgumentException("Duplicate live Kaigi commitment.", nameof(entries));
        Entries = Array.AsReadOnly(entries);
    }
    public IReadOnlyList<KaigiPrivateParticipationV1> Entries { get; }
}

/// <summary>
/// Exact retained KaigiRecord JSON projection. Host is the original host, including after rekey.
/// The redacted Torii application call view does not supply this record. Decoding does not authenticate a node response or verify a proof.
/// </summary>
public sealed class KaigiRecordV1
{
    private readonly byte[] rosterRoot;
    private readonly IReadOnlyDictionary<string, IReadOnlyDictionary<string, JsonNode?>> participantMetadata;
    internal KaigiRecordV1(NewKaigi call, KaigiParticipantCommitment? hostCommitment, byte[] rosterRoot,
        KaigiParticipantCommitment[] rosterCommitments, KaigiPrivateParticipationLedgerV1 privateParticipation,
        KaigiParticipantNullifier[] nullifierLog, KaigiAuthorizationScalarV1[] usageCommitments,
        KaigiStatus status, ulong createdAtMs, ulong? endedAtMs, ulong totalDurationMs, ulong totalBilledGas,
        uint segmentsRecorded, string[] participants, IReadOnlyDictionary<string, IReadOnlyDictionary<string, JsonNode?>> participantMetadata)
    {
        Call = call; HostCommitment = hostCommitment; this.rosterRoot = rosterRoot;
        RosterCommitments = Array.AsReadOnly(rosterCommitments); PrivateParticipation = privateParticipation;
        NullifierLog = Array.AsReadOnly(nullifierLog); UsageCommitments = Array.AsReadOnly(usageCommitments);
        Status = status; CreatedAtMs = createdAtMs; EndedAtMs = endedAtMs; TotalDurationMs = totalDurationMs;
        TotalBilledGas = totalBilledGas; SegmentsRecorded = segmentsRecorded; Participants = Array.AsReadOnly(participants);
        this.participantMetadata = participantMetadata;
        if (rosterCommitments.Select(static item => item.Commitment).Distinct().Count() != rosterCommitments.Length
            || nullifierLog.Select(static item => item.Digest).Distinct().Count() != nullifierLog.Length
            || participants.Distinct(StringComparer.Ordinal).Count() != participants.Length || participants.Contains(Host, StringComparer.Ordinal))
            throw new ArgumentException("Duplicate Kaigi roster/nullifier or explicit host participant.");
        var active = privateParticipation.Entries.Where(static entry => entry.ActiveCommitment is not null).Select(static entry => entry.ActiveCommitment!).ToHashSet();
        if (!active.SetEquals(rosterCommitments.Select(static item => item.Commitment)))
            throw new ArgumentException("Kaigi retained participation does not match the private roster.");
        if (privateParticipation.Entries.Any(entry => entry.OriginalAccount == Host))
            throw new ArgumentException("Kaigi host cannot be a private participant.");
        if (call.PrivacyMode == KaigiPrivacyMode.Transparent)
        {
            if (hostCommitment is not null || rosterCommitments.Length != 0 || privateParticipation.Entries.Count != 0 || nullifierLog.Length != 0 || usageCommitments.Length != 0)
                throw new ArgumentException("Transparent Kaigi record contains private state.");
        }
        else if (hostCommitment is null || participants.Length != 0 || usageCommitments.Length != segmentsRecorded)
            throw new ArgumentException("Private Kaigi record has inconsistent authorization/usage state.");
        if ((status == KaigiStatus.Ended) != endedAtMs.HasValue || endedAtMs < createdAtMs)
            throw new ArgumentException("Kaigi end timestamp does not match its lifecycle.");
    }
    /// <summary>The immutable full call configuration retained in the record.</summary>
    public NewKaigi Call { get; }
    public KaigiId Id => Call.Id;
    /// <summary>Original host account; a successor signer does not replace it.</summary>
    public string Host => Call.Host;
    public KaigiParticipantCommitment? HostCommitment { get; }
    public byte[] RosterRoot => (byte[])rosterRoot.Clone();
    public IReadOnlyList<KaigiParticipantCommitment> RosterCommitments { get; }
    public KaigiPrivateParticipationLedgerV1 PrivateParticipation { get; }
    public IReadOnlyList<KaigiParticipantNullifier> NullifierLog { get; }
    public IReadOnlyList<KaigiAuthorizationScalarV1> UsageCommitments { get; }
    public KaigiStatus Status { get; }
    public ulong CreatedAtMs { get; }
    public ulong? EndedAtMs { get; }
    public ulong TotalDurationMs { get; }
    public ulong TotalBilledGas { get; }
    public uint SegmentsRecorded { get; }
    public IReadOnlyList<string> Participants { get; }
    public IReadOnlyDictionary<string, IReadOnlyDictionary<string, JsonNode?>> ParticipantMetadata =>
        new ReadOnlyDictionary<string, IReadOnlyDictionary<string, JsonNode?>>(participantMetadata.ToDictionary(
            static entry => entry.Key, static entry => KaigiValidationV1.Metadata(entry.Value), StringComparer.Ordinal));

    /// <summary>Decode the exact current retained-record JSON with bounded arrays, duplicate/unknown-field rejection and canonical scalars.</summary>
    public static KaigiRecordV1 FromJson(ReadOnlyMemory<byte> utf8Json) => KaigiRecordJsonV1.Parse(utf8Json);
}

internal static class KaigiRecordJsonV1
{
    internal static KaigiRecordV1 Parse(ReadOnlyMemory<byte> json)
    {
        if (json.Length is < 1 or > 1_048_576) throw new ArgumentException("Kaigi record exceeds its V1 JSON bound.", nameof(json));
        using var document = JsonDocument.Parse(json, new JsonDocumentOptions { MaxDepth = 64 });
        var root = Object(document.RootElement, "id", "host", "billing_account", "title", "description", "max_participants",
            "gas_rate_per_minute", "metadata", "scheduled_start_ms", "privacy_mode", "room_policy", "relay_manifest",
            "host_commitment", "roster_root", "roster_commitments", "private_participation", "nullifier_log", "usage_commitments",
            "status", "created_at_ms", "ended_at_ms", "total_duration_ms", "total_billed_gas", "segments_recorded", "participants", "participant_metadata");
        var id = Object(root["id"], "domain_id", "call_name");
        var call = new NewKaigi(new KaigiId(String(id["domain_id"]), String(id["call_name"])), String(root["host"]),
            OptionalString(root["title"]), OptionalString(root["description"]), OptionalU32(root["max_participants"]),
            U64(root["gas_rate_per_minute"]), Metadata(root["metadata"]), OptionalU64(root["scheduled_start_ms"]), OptionalString(root["billing_account"]),
            Tagged<KaigiPrivacyMode>(root["privacy_mode"], "mode"), Tagged<KaigiRoomPolicy>(root["room_policy"], "policy"),
            root["relay_manifest"].ValueKind == JsonValueKind.Null ? null : Relay(root["relay_manifest"]));
        var participation = Object(root["private_participation"], "entries");
        var entries = Array(participation["entries"], 4096).Select(static value =>
        {
            var entry = Object(value, "original_account", "sequence", "active_commitment");
            return new KaigiPrivateParticipationV1(String(entry["original_account"]), U64(entry["sequence"]),
                entry["active_commitment"].ValueKind == JsonValueKind.Null ? null : Scalar(entry["active_commitment"]));
        }).ToArray();
        var participantMetadata = new Dictionary<string, IReadOnlyDictionary<string, JsonNode?>>(StringComparer.Ordinal);
        foreach (var (key, value) in Object(root["participant_metadata"]))
            participantMetadata.Add(TransactionEncodingContext.CanonicalizeAccountId(key), Metadata(value));
        // NetworkId owns the SDK's existing strict Norito Hash literal codec. Retain only its hash bytes here.
        var rosterRoot = NetworkId.Parse(String(root["roster_root"])).ToBytes();
        return new KaigiRecordV1(call,
            root["host_commitment"].ValueKind == JsonValueKind.Null ? null : Commitment(root["host_commitment"]), rosterRoot,
            Array(root["roster_commitments"], 4096).Select(Commitment).ToArray(), new KaigiPrivateParticipationLedgerV1(entries),
            Array(root["nullifier_log"], 8194).Select(static value => new KaigiParticipantNullifier(Scalar(Object(value, "digest")["digest"]))).ToArray(),
            Array(root["usage_commitments"], 4096).Select(Scalar).ToArray(), Tagged<KaigiStatus>(root["status"], "status"),
            U64(root["created_at_ms"]), OptionalU64(root["ended_at_ms"]), U64(root["total_duration_ms"]), U64(root["total_billed_gas"]),
            U32(root["segments_recorded"]), Array(root["participants"], 4096).Select(static value => TransactionEncodingContext.CanonicalizeAccountId(String(value))).ToArray(),
            new ReadOnlyDictionary<string, IReadOnlyDictionary<string, JsonNode?>>(participantMetadata));
    }

    private static KaigiParticipantCommitment Commitment(JsonElement value) => new(Scalar(Object(value, "commitment")["commitment"]));
    private static KaigiAuthorizationScalarV1 Scalar(JsonElement value)
    {
        if (value.ValueKind != JsonValueKind.Array || value.GetArrayLength() != 32) throw new ArgumentException("Kaigi scalar JSON requires exactly 32 byte values.");
        var bytes = new byte[32]; var index = 0;
        foreach (var item in value.EnumerateArray())
        {
            if (item.ValueKind != JsonValueKind.Number || !item.TryGetByte(out var next)) throw new ArgumentException("Kaigi scalar JSON requires byte integers.");
            bytes[index++] = next;
        }
        return new KaigiAuthorizationScalarV1(bytes);
    }
    private static KaigiRelayManifest Relay(JsonElement value)
    {
        var manifest = Object(value, "hops", "expiry_ms");
        return new KaigiRelayManifest(Array(manifest["hops"], 8).Select(static value =>
        {
            var hop = Object(value, "relay_id", "hpke_public_key", "weight");
            var encoded = String(hop["hpke_public_key"]);
            if (encoded.Length > 5464) throw new ArgumentException("Kaigi HPKE descriptor is too large.");
            var bytes = Convert.FromBase64String(encoded);
            if (Convert.ToBase64String(bytes) != encoded) throw new ArgumentException("Kaigi HPKE descriptor requires canonical base64.");
            return new KaigiRelayHop(String(hop["relay_id"]), bytes, checked((byte)U32(hop["weight"])));
        }), U64(manifest["expiry_ms"]));
    }
    private static T Tagged<T>(JsonElement value, string tag) where T : struct, Enum
    {
        var fields = Object(value, tag, "state");
        var spelling = String(fields[tag]);
        if (fields["state"].ValueKind != JsonValueKind.Null || !Enum.TryParse<T>(spelling, out var parsed)
            || !Enum.IsDefined(parsed) || parsed.ToString() != spelling) throw new ArgumentException("Unknown or noncanonical Kaigi enum.");
        return parsed;
    }
    private static IReadOnlyDictionary<string, JsonNode?> Metadata(JsonElement value)
    {
        RequireUniqueRecursive(value);
        return KaigiValidationV1.Metadata(Object(value).ToDictionary(static entry => entry.Key,
            static entry => JsonNode.Parse(entry.Value.GetRawText()), StringComparer.Ordinal));
    }
    private static void RequireUniqueRecursive(JsonElement value)
    {
        if (value.ValueKind == JsonValueKind.Object)
        {
            foreach (var child in Object(value).Values) RequireUniqueRecursive(child);
        }
        else if (value.ValueKind == JsonValueKind.Array)
            foreach (var child in value.EnumerateArray()) RequireUniqueRecursive(child);
    }
    private static Dictionary<string, JsonElement> Object(JsonElement value, params string[] fields)
    {
        if (value.ValueKind != JsonValueKind.Object) throw new ArgumentException("Kaigi JSON requires an object.");
        var result = new Dictionary<string, JsonElement>(StringComparer.Ordinal);
        foreach (var property in value.EnumerateObject())
            if (!result.TryAdd(property.Name, property.Value)) throw new ArgumentException($"Duplicate Kaigi JSON field {property.Name}.");
        if (fields.Length != 0 && (result.Count != fields.Length || fields.Any(field => !result.ContainsKey(field))))
            throw new ArgumentException("Kaigi JSON has missing or unknown fields.");
        return result;
    }
    private static JsonElement[] Array(JsonElement value, int maximum)
    {
        if (value.ValueKind != JsonValueKind.Array || value.GetArrayLength() > maximum) throw new ArgumentException("Invalid or oversized Kaigi JSON array.");
        return value.EnumerateArray().ToArray();
    }
    private static string String(JsonElement value) => value.ValueKind == JsonValueKind.String ? value.GetString()! : throw new ArgumentException("Kaigi JSON requires a string.");
    private static string? OptionalString(JsonElement value) => value.ValueKind == JsonValueKind.Null ? null : String(value);
    private static ulong U64(JsonElement value) => value.ValueKind == JsonValueKind.Number && value.TryGetUInt64(out var number) ? number : throw new ArgumentException("Kaigi JSON requires a u64 integer.");
    private static uint U32(JsonElement value) => value.ValueKind == JsonValueKind.Number && value.TryGetUInt32(out var number) ? number : throw new ArgumentException("Kaigi JSON requires a u32 integer.");
    private static ulong? OptionalU64(JsonElement value) => value.ValueKind == JsonValueKind.Null ? null : U64(value);
    private static uint? OptionalU32(JsonElement value) => value.ValueKind == JsonValueKind.Null ? null : U32(value);
}
