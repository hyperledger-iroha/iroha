using System.Globalization;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace Hyperledger.Iroha.Offline;

public enum OfflineNoteWalletNoteState
{
    Spendable,
    ReceivePending,
    Spent,
    RedeemPending,
    Redeemed,
    Cancelled,
}

public abstract class OfflineNoteCommitmentOrigin
{
    private OfflineNoteCommitmentOrigin()
    {
    }

    public sealed class IssuerLoad : OfflineNoteCommitmentOrigin
    {
        public IssuerLoad(string operationId, string lineageId, ulong localRevision)
        {
            OperationId = OfflineNoteWalletNote.RequireExactNonBlankText(
                operationId,
                "origin.operation_id",
                nameof(operationId));
            LineageId = OfflineNoteWalletNote.RequireExactNonBlankText(
                lineageId,
                "origin.lineage_id",
                nameof(lineageId));
            LocalRevision = localRevision;
        }

        public string OperationId { get; }

        public string LineageId { get; }

        public ulong LocalRevision { get; }
    }

    public sealed class P2pOutput : OfflineNoteCommitmentOrigin
    {
        public P2pOutput(string paymentRequestId, uint outputIndex)
        {
            PaymentRequestId = OfflineNoteWalletNote.RequireExactNonBlankText(
                paymentRequestId,
                "origin.payment_request_id",
                nameof(paymentRequestId));
            OutputIndex = outputIndex;
        }

        public string PaymentRequestId { get; }

        public uint OutputIndex { get; }
    }
}

public sealed class OfflineNoteWalletNote
{
    public const int HashLength = 32;

    private static readonly Encoding StrictUtf8 = new UTF8Encoding(false, true);

    private readonly byte[] keyCertificateNorito;
    private readonly byte[] noteCommitment;
    private readonly byte[] noteSecret;
    private readonly List<byte[]> bearerAuditTrailNorito;

    public OfflineNoteWalletNote(
        string chainId,
        string accountId,
        string assetId,
        string amount,
        byte[] keyCertificateNorito,
        byte[] noteCommitment,
        byte[] noteSecret,
        OfflineNoteCommitmentOrigin origin,
        OfflineNoteWalletNoteState state,
        ulong createdAtMs,
        ulong updatedAtMs,
        string? spentPaymentRequestId = null,
        IEnumerable<byte[]>? bearerAuditTrailNorito = null)
    {
        ChainId = RequireExactNonBlankText(chainId, "chain_id", nameof(chainId));
        AccountId = OfflineNoteCanonicalPayloadCodec.CanonicalAccountId(
            accountId,
            "account_id",
            nameof(accountId));
        AssetId = OfflineNoteCanonicalPayloadCodec.CanonicalAssetId(
            assetId,
            "asset_id",
            nameof(assetId));
        var parsedAssetId = AssetId.Split('#', StringSplitOptions.None);
        if (!string.Equals(parsedAssetId[1], AccountId, StringComparison.Ordinal))
        {
            throw new ArgumentException("asset_id account must match account_id.", nameof(assetId));
        }

        Amount = RequireCanonicalAmount(amount);
        this.keyCertificateNorito = CopyRequiredNonEmptyBytes(
            keyCertificateNorito,
            nameof(keyCertificateNorito),
            "key_certificate_norito_base64");
        this.noteCommitment = CopyRequiredFixedBytes(
            noteCommitment,
            HashLength,
            nameof(noteCommitment),
            "note_commitment_hex");
        this.noteSecret = CopyRequiredFixedBytes(
            noteSecret,
            HashLength,
            nameof(noteSecret),
            "note_secret_base64");
        Origin = origin ?? throw new ArgumentNullException(nameof(origin));
        State = RequireDefinedState(state);
        if (createdAtMs == 0)
        {
            throw new ArgumentException("created_at_ms must be positive.", nameof(createdAtMs));
        }

        if (updatedAtMs < createdAtMs)
        {
            throw new ArgumentException(
                "updated_at_ms must be greater than or equal to created_at_ms.",
                nameof(updatedAtMs));
        }

        CreatedAtMs = createdAtMs;
        UpdatedAtMs = updatedAtMs;
        SpentPaymentRequestId = spentPaymentRequestId is null
            ? null
            : RequireExactNonBlankText(
                spentPaymentRequestId,
                "spent_payment_request_id",
                nameof(spentPaymentRequestId));
        this.bearerAuditTrailNorito = CopyAuditTrail(bearerAuditTrailNorito);
    }

    public string ChainId { get; }

    public string AccountId { get; }

    public string AssetId { get; }

    public string Amount { get; }

    public byte[] KeyCertificateNorito => keyCertificateNorito.ToArray();

    public byte[] NoteCommitment => noteCommitment.ToArray();

    public string NoteCommitmentHex => Convert.ToHexString(noteCommitment).ToLowerInvariant();

    public byte[] NoteSecret => noteSecret.ToArray();

    public OfflineNoteCommitmentOrigin Origin { get; }

    public IReadOnlyList<byte[]> BearerAuditTrailNorito =>
        bearerAuditTrailNorito.Select(audit => audit.ToArray()).ToArray();

    public OfflineNoteWalletNoteState State { get; }

    public ulong CreatedAtMs { get; }

    public ulong UpdatedAtMs { get; }

    public string? SpentPaymentRequestId { get; }

    public OfflineNoteWalletNote WithState(OfflineNoteWalletNoteState state, ulong updatedAtMs)
    {
        return new OfflineNoteWalletNote(
            ChainId,
            AccountId,
            AssetId,
            Amount,
            keyCertificateNorito,
            noteCommitment,
            noteSecret,
            Origin,
            state,
            CreatedAtMs,
            updatedAtMs,
            SpentPaymentRequestId,
            bearerAuditTrailNorito);
    }

    public OfflineNoteWalletNote WithSpentPaymentRequestId(
        string? spentPaymentRequestId,
        ulong updatedAtMs)
    {
        return new OfflineNoteWalletNote(
            ChainId,
            AccountId,
            AssetId,
            Amount,
            keyCertificateNorito,
            noteCommitment,
            noteSecret,
            Origin,
            State,
            CreatedAtMs,
            updatedAtMs,
            spentPaymentRequestId,
            bearerAuditTrailNorito);
    }

    internal static string RequireExactNonBlankText(
        string value,
        string field,
        string parameterName)
    {
        ArgumentNullException.ThrowIfNull(value, parameterName);
        if (value.Trim().Length == 0)
        {
            throw new ArgumentException($"{field} must not be blank.", parameterName);
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new ArgumentException($"{field} must not contain surrounding whitespace.", parameterName);
        }

        if (value.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException($"{field} must not contain whitespace.", parameterName);
        }

        if (value.Any(char.IsControl))
        {
            throw new ArgumentException($"{field} must not contain control characters.", parameterName);
        }

        try
        {
            StrictUtf8.GetByteCount(value);
        }
        catch (EncoderFallbackException exception)
        {
            throw new ArgumentException($"{field} must be valid UTF-8 text.", parameterName, exception);
        }

        return value;
    }

    private static string RequireCanonicalAmount(string amount)
    {
        var exact = RequireExactNonBlankText(amount, "amount", nameof(amount));
        var canonical = OfflineNoteCanonicalPayloadCodec.ParseCanonicalNumeric(exact);
        if (!string.Equals(canonical, exact, StringComparison.Ordinal))
        {
            throw new ArgumentException("amount must be canonical numeric text.", nameof(amount));
        }

        return canonical;
    }

    private static OfflineNoteWalletNoteState RequireDefinedState(OfflineNoteWalletNoteState state)
    {
        return state switch
        {
            OfflineNoteWalletNoteState.Spendable
            or OfflineNoteWalletNoteState.ReceivePending
            or OfflineNoteWalletNoteState.Spent
            or OfflineNoteWalletNoteState.RedeemPending
            or OfflineNoteWalletNoteState.Redeemed
            or OfflineNoteWalletNoteState.Cancelled => state,
            _ => throw new ArgumentException("state is unsupported.", nameof(state)),
        };
    }

    private static byte[] CopyRequiredNonEmptyBytes(
        byte[] value,
        string parameterName,
        string field)
    {
        ArgumentNullException.ThrowIfNull(value, parameterName);
        if (value.Length == 0)
        {
            throw new ArgumentException($"{field} must not be empty.", parameterName);
        }

        return value.ToArray();
    }

    private static byte[] CopyRequiredFixedBytes(
        byte[] value,
        int length,
        string parameterName,
        string field)
    {
        ArgumentNullException.ThrowIfNull(value, parameterName);
        if (value.Length != length)
        {
            throw new ArgumentException($"{field} must be {length} bytes.", parameterName);
        }

        return value.ToArray();
    }

    private static List<byte[]> CopyAuditTrail(IEnumerable<byte[]>? audits)
    {
        var copied = new List<byte[]>();
        if (audits is null)
        {
            return copied;
        }

        foreach (var audit in audits)
        {
            copied.Add(CopyRequiredNonEmptyBytes(
                audit,
                nameof(audits),
                "bearer_audit_trail_norito_base64"));
        }

        return copied;
    }
}

public static class OfflineNoteWalletNoteJsonCodec
{
    public const ulong Version = 1;

    private static readonly Encoding StrictUtf8 = new UTF8Encoding(false, true);
    private static readonly HashSet<string> RootFields = new(StringComparer.Ordinal)
    {
        "version",
        "chain_id",
        "account_id",
        "asset_id",
        "amount",
        "key_certificate_norito_base64",
        "note_commitment_hex",
        "note_secret_base64",
        "origin",
        "bearer_audit_trail_norito_base64",
        "state",
        "created_at_ms",
        "updated_at_ms",
        "spent_payment_request_id",
    };
    private static readonly HashSet<string> IssuerLoadOriginFields = new(StringComparer.Ordinal)
    {
        "type",
        "operation_id",
        "lineage_id",
        "local_revision",
    };
    private static readonly HashSet<string> P2pOutputOriginFields = new(StringComparer.Ordinal)
    {
        "type",
        "payment_request_id",
        "output_index",
    };

    public static byte[] Encode(OfflineNoteWalletNote note)
    {
        ArgumentNullException.ThrowIfNull(note);

        var root = new JsonObject
        {
            ["version"] = Version,
            ["chain_id"] = note.ChainId,
            ["account_id"] = note.AccountId,
            ["asset_id"] = note.AssetId,
            ["amount"] = note.Amount,
            ["key_certificate_norito_base64"] = Convert.ToBase64String(note.KeyCertificateNorito),
            ["note_commitment_hex"] = note.NoteCommitmentHex,
            ["note_secret_base64"] = Convert.ToBase64String(note.NoteSecret),
            ["origin"] = EncodeOrigin(note.Origin),
            ["bearer_audit_trail_norito_base64"] = EncodeAuditTrail(note.BearerAuditTrailNorito),
            ["state"] = EncodeState(note.State),
            ["created_at_ms"] = note.CreatedAtMs,
            ["updated_at_ms"] = note.UpdatedAtMs,
        };
        if (note.SpentPaymentRequestId is not null)
        {
            root["spent_payment_request_id"] = note.SpentPaymentRequestId;
        }

        return JsonSerializer.SerializeToUtf8Bytes(root);
    }

    public static OfflineNoteWalletNote Decode(byte[] payload)
    {
        ArgumentNullException.ThrowIfNull(payload);

        string jsonText;
        JsonNode? parsed;
        try
        {
            jsonText = StrictUtf8.GetString(payload);
            using var document = JsonDocument.Parse(jsonText);
            RejectDuplicateProperties(document.RootElement, "json");
            parsed = JsonNode.Parse(jsonText);
        }
        catch (Exception exception) when (exception is JsonException or DecoderFallbackException)
        {
            throw new ArgumentException("Offline Note wallet note JSON is invalid.", nameof(payload), exception);
        }

        if (parsed is not JsonObject root)
        {
            throw InvalidField("json");
        }

        RejectUnknownProperties(root, "json", RootFields);
        if (ReadUInt64(root, "version") != Version)
        {
            throw InvalidField("version");
        }

        return new OfflineNoteWalletNote(
            ReadString(root, "chain_id"),
            ReadString(root, "account_id"),
            ReadString(root, "asset_id"),
            ReadString(root, "amount"),
            ReadStrictBase64(root, "key_certificate_norito_base64"),
            ReadStrictHex(root, "note_commitment_hex"),
            ReadStrictBase64(root, "note_secret_base64"),
            DecodeOrigin(ReadObject(root, "origin")),
            DecodeState(ReadString(root, "state")),
            ReadUInt64(root, "created_at_ms"),
            ReadUInt64(root, "updated_at_ms"),
            ReadOptionalString(root, "spent_payment_request_id"),
            DecodeAuditTrail(root["bearer_audit_trail_norito_base64"]));
    }

    private static JsonObject EncodeOrigin(OfflineNoteCommitmentOrigin origin)
    {
        return origin switch
        {
            OfflineNoteCommitmentOrigin.IssuerLoad issuerLoad => new JsonObject
            {
                ["type"] = "issuer_load",
                ["operation_id"] = issuerLoad.OperationId,
                ["lineage_id"] = issuerLoad.LineageId,
                ["local_revision"] = issuerLoad.LocalRevision,
            },
            OfflineNoteCommitmentOrigin.P2pOutput p2pOutput => new JsonObject
            {
                ["type"] = "p2p_output",
                ["payment_request_id"] = p2pOutput.PaymentRequestId,
                ["output_index"] = p2pOutput.OutputIndex,
            },
            _ => throw new ArgumentException("Offline Note commitment origin is unsupported.", nameof(origin)),
        };
    }

    private static OfflineNoteCommitmentOrigin DecodeOrigin(JsonObject origin)
    {
        var type = ReadString(origin, "type");
        switch (type)
        {
            case "issuer_load":
                RejectUnknownProperties(origin, "origin", IssuerLoadOriginFields);
                return new OfflineNoteCommitmentOrigin.IssuerLoad(
                    ReadString(origin, "operation_id"),
                    ReadString(origin, "lineage_id"),
                    ReadUInt64(origin, "local_revision"));
            case "p2p_output":
                RejectUnknownProperties(origin, "origin", P2pOutputOriginFields);
                return new OfflineNoteCommitmentOrigin.P2pOutput(
                    ReadString(origin, "payment_request_id"),
                    ReadUInt32(origin, "output_index"));
            default:
                throw InvalidField("origin.type");
        }
    }

    private static JsonArray EncodeAuditTrail(IReadOnlyList<byte[]> audits)
    {
        var encoded = new JsonArray();
        foreach (var audit in audits)
        {
            encoded.Add(Convert.ToBase64String(audit));
        }

        return encoded;
    }

    private static IReadOnlyList<byte[]> DecodeAuditTrail(JsonNode? value)
    {
        if (value is null)
        {
            return Array.Empty<byte[]>();
        }

        if (value is not JsonArray values)
        {
            throw InvalidField("bearer_audit_trail_norito_base64");
        }

        var decoded = new List<byte[]>();
        for (var index = 0; index < values.Count; index++)
        {
            if (values[index] is not JsonValue item)
            {
                throw InvalidField($"bearer_audit_trail_norito_base64[{index}]");
            }

            decoded.Add(DecodeStrictBase64(
                ReadStringValue(item, $"bearer_audit_trail_norito_base64[{index}]"),
                $"bearer_audit_trail_norito_base64[{index}]"));
        }

        return decoded;
    }

    private static void RejectDuplicateProperties(JsonElement element, string field)
    {
        switch (element.ValueKind)
        {
            case JsonValueKind.Object:
                var seen = new HashSet<string>(StringComparer.Ordinal);
                foreach (var property in element.EnumerateObject())
                {
                    var childField = $"{field}.{property.Name}";
                    if (!seen.Add(property.Name))
                    {
                        throw InvalidField(childField);
                    }

                    RejectDuplicateProperties(property.Value, childField);
                }

                break;
            case JsonValueKind.Array:
                var index = 0;
                foreach (var item in element.EnumerateArray())
                {
                    RejectDuplicateProperties(item, $"{field}[{index}]");
                    index++;
                }

                break;
        }
    }

    private static void RejectUnknownProperties(
        JsonObject root,
        string field,
        IReadOnlySet<string> allowed)
    {
        foreach (var property in root)
        {
            if (!allowed.Contains(property.Key))
            {
                throw InvalidField($"{field}.{property.Key}");
            }
        }
    }

    private static string EncodeState(OfflineNoteWalletNoteState state)
    {
        return state switch
        {
            OfflineNoteWalletNoteState.Spendable => "spendable",
            OfflineNoteWalletNoteState.ReceivePending => "receivePending",
            OfflineNoteWalletNoteState.Spent => "spent",
            OfflineNoteWalletNoteState.RedeemPending => "redeemPending",
            OfflineNoteWalletNoteState.Redeemed => "redeemed",
            OfflineNoteWalletNoteState.Cancelled => "cancelled",
            _ => throw InvalidField("state"),
        };
    }

    private static OfflineNoteWalletNoteState DecodeState(string state)
    {
        return state switch
        {
            "spendable" => OfflineNoteWalletNoteState.Spendable,
            "receivePending" => OfflineNoteWalletNoteState.ReceivePending,
            "spent" => OfflineNoteWalletNoteState.Spent,
            "redeemPending" => OfflineNoteWalletNoteState.RedeemPending,
            "redeemed" => OfflineNoteWalletNoteState.Redeemed,
            "cancelled" => OfflineNoteWalletNoteState.Cancelled,
            _ => throw InvalidField("state"),
        };
    }

    private static string? ReadOptionalString(JsonObject root, string field)
    {
        return root.TryGetPropertyValue(field, out var node) && node is not null
            ? ReadStringNode(node, field)
            : null;
    }

    private static string ReadString(JsonObject root, string field)
    {
        if (!root.TryGetPropertyValue(field, out var node) || node is null)
        {
            throw InvalidField(field);
        }

        return ReadStringNode(node, field);
    }

    private static string ReadStringNode(JsonNode node, string field)
    {
        return node is JsonValue value ? ReadStringValue(value, field) : throw InvalidField(field);
    }

    private static string ReadStringValue(JsonValue value, string field)
    {
        string? text;
        try
        {
            text = value.GetValue<string>();
        }
        catch (InvalidOperationException exception)
        {
            throw new ArgumentException($"Offline Note wallet note field {field} is invalid.", exception);
        }

        if (string.IsNullOrWhiteSpace(text))
        {
            throw InvalidField(field);
        }

        return text;
    }

    private static JsonObject ReadObject(JsonObject root, string field)
    {
        if (!root.TryGetPropertyValue(field, out var node) || node is not JsonObject value)
        {
            throw InvalidField(field);
        }

        return value;
    }

    private static byte[] ReadStrictBase64(JsonObject root, string field)
    {
        return DecodeStrictBase64(ReadString(root, field), field);
    }

    private static byte[] DecodeStrictBase64(string value, string field)
    {
        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal)
            || value.Any(char.IsWhiteSpace))
        {
            throw InvalidField(field);
        }

        byte[] decoded;
        try
        {
            decoded = Convert.FromBase64String(value);
        }
        catch (FormatException exception)
        {
            throw new ArgumentException($"Offline Note wallet note field {field} is invalid.", exception);
        }

        if (!string.Equals(Convert.ToBase64String(decoded), value, StringComparison.Ordinal))
        {
            throw InvalidField(field);
        }

        return decoded;
    }

    private static byte[] ReadStrictHex(JsonObject root, string field)
    {
        var value = ReadString(root, field);
        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal)
            || value.StartsWith("0x", StringComparison.OrdinalIgnoreCase)
            || (value.Length & 1) != 0
            || !IsLowerHex(value))
        {
            throw InvalidField(field);
        }

        try
        {
            return Convert.FromHexString(value);
        }
        catch (FormatException exception)
        {
            throw new ArgumentException($"Offline Note wallet note field {field} is invalid.", exception);
        }
    }

    private static ulong ReadUInt64(JsonObject root, string field)
    {
        if (!root.TryGetPropertyValue(field, out var node) || node is not JsonValue value)
        {
            throw InvalidField(field);
        }

        if (value.TryGetValue<ulong>(out var unsigned))
        {
            return unsigned;
        }

        if (value.TryGetValue<long>(out var signed) && signed >= 0)
        {
            return (ulong)signed;
        }

        if (value.TryGetValue<string>(out var text)
            && !string.IsNullOrWhiteSpace(text)
            && string.Equals(text.Trim(), text, StringComparison.Ordinal)
            && IsCanonicalUInt64Text(text)
            && ulong.TryParse(text, NumberStyles.None, CultureInfo.InvariantCulture, out var parsed))
        {
            return parsed;
        }

        throw InvalidField(field);
    }

    private static bool IsLowerHex(string value)
    {
        return value.All(static character =>
            (character is >= '0' and <= '9') || (character is >= 'a' and <= 'f'));
    }

    private static bool IsCanonicalUInt64Text(string value)
    {
        if (value.Length == 0 || (value.Length > 1 && value[0] == '0'))
        {
            return false;
        }

        return value.All(static character => character is >= '0' and <= '9');
    }

    private static uint ReadUInt32(JsonObject root, string field)
    {
        var value = ReadUInt64(root, field);
        if (value > uint.MaxValue)
        {
            throw InvalidField(field);
        }

        return (uint)value;
    }

    private static ArgumentException InvalidField(string field)
    {
        return new ArgumentException($"Offline Note wallet note field {field} is invalid.");
    }
}
