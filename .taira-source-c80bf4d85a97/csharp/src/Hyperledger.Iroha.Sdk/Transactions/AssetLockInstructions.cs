using System.Buffers;
using System.Buffers.Binary;
using System.Globalization;
using System.Numerics;
using System.Text;
using System.Text.Json;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Numeric;

namespace Hyperledger.Iroha.Transactions;

/// <summary>
/// Cancels one native asset lock only when its finalized remaining amount still
/// equals the amount authorized by the signer.
/// </summary>
public sealed record class CancelAssetLockInstruction : TransactionInstruction
{
    private const byte CompactLengthFlag = 0x02;

    /// <summary>The native V1 instruction type and wire identifier.</summary>
    public const string NativeTypeName =
        "iroha_data_model::isi::escrow::CancelAssetLock";

    /// <summary>Maximum accepted size of a framed native V1 instruction.</summary>
    public const int MaximumNoritoBytesV1 = 512;

    /// <summary>Maximum accepted size of a native V1 JSON payload.</summary>
    public const int MaximumJsonBytesV1 = 4_096;

    /// <summary>
    /// Maximum UTF-8 size of an application lock-id preimage accepted by V1
    /// convenience builders.
    /// </summary>
    public const int MaximumLockIdUtf8BytesV1 = 4_096;

    private readonly byte[] escrowId;
    private readonly NumericV1.QuantityValue expectedRemainingAmount;

    /// <summary>
    /// Creates a cancellation by deriving the native escrow identifier from an
    /// exact application lock identifier with Blake2b-256.
    /// </summary>
    public CancelAssetLockInstruction(
        string lockId,
        string expectedRemainingAmount)
        : this(
            AssetLockInstructionValidation.DeriveEscrowId(lockId, nameof(lockId)),
            AssetLockInstructionValidation.RequirePositiveQuantity(
                expectedRemainingAmount,
                nameof(expectedRemainingAmount)))
    {
    }

    /// <summary>
    /// Creates a cancellation by deriving the native escrow identifier from an
    /// exact application lock identifier with Blake2b-256.
    /// </summary>
    public CancelAssetLockInstruction(
        string lockId,
        NumericV1.QuantityValue expectedRemainingAmount)
        : this(
            AssetLockInstructionValidation.DeriveEscrowId(lockId, nameof(lockId)),
            AssetLockInstructionValidation.RequirePositiveQuantity(
                expectedRemainingAmount,
                nameof(expectedRemainingAmount)))
    {
    }

    private CancelAssetLockInstruction(
        ReadOnlySpan<byte> escrowId,
        NumericV1.QuantityValue expectedRemainingAmount)
    {
        this.escrowId = AssetLockInstructionValidation.RequireEscrowId(
            escrowId,
            nameof(escrowId));
        this.expectedRemainingAmount =
            AssetLockInstructionValidation.RequirePositiveQuantity(
                expectedRemainingAmount,
                nameof(expectedRemainingAmount));
    }

    /// <summary>Gets the canonical checksummed native escrow identifier.</summary>
    public string EscrowId =>
        AssetLockInstructionValidation.FormatEscrowId(escrowId);

    /// <summary>Gets the positive canonical remaining-amount precondition.</summary>
    public string ExpectedRemainingAmount => expectedRemainingAmount.ToString();

    /// <summary>Gets the exact remaining-amount precondition.</summary>
    public NumericV1.QuantityValue ExpectedRemainingAmountValue =>
        expectedRemainingAmount;

    /// <summary>
    /// Creates a cancellation from an already-derived canonical native escrow
    /// identifier.
    /// </summary>
    public static CancelAssetLockInstruction FromEscrowId(
        string escrowId,
        string expectedRemainingAmount)
    {
        return new CancelAssetLockInstruction(
            AssetLockInstructionValidation.ParseEscrowId(
                escrowId,
                nameof(escrowId)),
            AssetLockInstructionValidation.RequirePositiveQuantity(
                expectedRemainingAmount,
                nameof(expectedRemainingAmount)));
    }

    /// <summary>
    /// Creates a cancellation from an already-derived canonical native escrow
    /// identifier.
    /// </summary>
    public static CancelAssetLockInstruction FromEscrowId(
        string escrowId,
        NumericV1.QuantityValue expectedRemainingAmount)
    {
        return new CancelAssetLockInstruction(
            AssetLockInstructionValidation.ParseEscrowId(
                escrowId,
                nameof(escrowId)),
            AssetLockInstructionValidation.RequirePositiveQuantity(
                expectedRemainingAmount,
                nameof(expectedRemainingAmount)));
    }

    /// <summary>Encodes the exact two-field native V1 Norito archive.</summary>
    public byte[] EncodeNorito()
    {
        return NoritoCodec.Encode(
            NativeTypeName,
            EncodeNativePayload(compactLengths: true),
            CompactLengthFlag);
    }

    /// <summary>
    /// Strictly decodes the exact two-field native V1 Norito archive.
    /// </summary>
    public static CancelAssetLockInstruction DecodeNorito(
        ReadOnlySpan<byte> archive)
    {
        if (archive.IsEmpty || archive.Length > MaximumNoritoBytesV1)
        {
            throw new ArgumentException(
                $"CancelAssetLock Norito must contain 1..{MaximumNoritoBytesV1} bytes.",
                nameof(archive));
        }

        byte[] payload;
        byte flags;
        try
        {
            (payload, flags) = NoritoCodec.Decode(NativeTypeName, archive);
        }
        catch (ArgumentException error)
        {
            throw new ArgumentException(
                "CancelAssetLock must use the canonical native V1 Norito frame.",
                nameof(archive),
                error);
        }
        if (flags != CompactLengthFlag)
        {
            throw new ArgumentException(
                "CancelAssetLock V1 requires compact-length Norito framing.",
                nameof(archive));
        }

        var decoded = DecodeNativePayload(
            payload,
            compactLengths: true,
            nameof(archive));
        if (!decoded.EncodeNorito().AsSpan().SequenceEqual(archive))
        {
            throw new ArgumentException(
                "CancelAssetLock must use the canonical unpadded V1 Norito encoding.",
                nameof(archive));
        }
        return decoded;
    }

    /// <summary>Encodes the canonical two-field native JSON payload.</summary>
    public byte[] EncodePayloadJson()
    {
        return EncodeJson(envelope: false);
    }

    /// <summary>
    /// Encodes the canonical instruction JSON shape
    /// <c>{"CancelAssetLock":{"escrow_id":...,"expected_remaining_amount":...}}</c>.
    /// </summary>
    public byte[] EncodeInstructionJson()
    {
        return EncodeJson(envelope: true);
    }

    /// <summary>
    /// Strictly decodes the canonical single-variant instruction JSON envelope.
    /// </summary>
    public static CancelAssetLockInstruction DecodeInstructionJson(
        ReadOnlySpan<byte> json)
    {
        if (json.IsEmpty || json.Length > MaximumJsonBytesV1)
        {
            throw new ArgumentException(
                $"CancelAssetLock JSON must contain 1..{MaximumJsonBytesV1} bytes.",
                nameof(json));
        }

        try
        {
            using var document = JsonDocument.Parse(
                json.ToArray(),
                StrictJsonDocumentOptions);
            if (document.RootElement.ValueKind != JsonValueKind.Object)
            {
                throw new ArgumentException(
                    "CancelAssetLock instruction JSON must be an object.",
                    nameof(json));
            }

            JsonElement? body = null;
            var propertyCount = 0;
            foreach (var property in document.RootElement.EnumerateObject())
            {
                propertyCount++;
                if (!string.Equals(
                        property.Name,
                        "CancelAssetLock",
                        StringComparison.Ordinal)
                    || body is not null)
                {
                    throw new ArgumentException(
                        "CancelAssetLock instruction JSON must contain exactly one `CancelAssetLock` variant.",
                        nameof(json));
                }
                body = property.Value;
            }
            if (propertyCount != 1
                || body is null
                || body.Value.ValueKind != JsonValueKind.Object)
            {
                throw new ArgumentException(
                    "CancelAssetLock instruction JSON must contain exactly one object variant.",
                    nameof(json));
            }
            return DecodePayloadJson(
                Encoding.UTF8.GetBytes(body.Value.GetRawText()));
        }
        catch (JsonException error)
        {
            throw new ArgumentException(
                "CancelAssetLock instruction JSON must be a valid bounded V1 object.",
                nameof(json),
                error);
        }
    }

    /// <summary>
    /// Strictly decodes the canonical two-field native JSON payload used by the
    /// shared V1 fixture inventory.
    /// </summary>
    public static CancelAssetLockInstruction DecodePayloadJson(
        ReadOnlySpan<byte> json)
    {
        if (json.IsEmpty || json.Length > MaximumJsonBytesV1)
        {
            throw new ArgumentException(
                $"CancelAssetLock JSON must contain 1..{MaximumJsonBytesV1} bytes.",
                nameof(json));
        }

        try
        {
            using var document = JsonDocument.Parse(
                json.ToArray(),
                StrictJsonDocumentOptions);
            if (document.RootElement.ValueKind != JsonValueKind.Object)
            {
                throw new ArgumentException(
                    "CancelAssetLock JSON must be an object.",
                    nameof(json));
            }

            string? escrowId = null;
            string? expectedRemainingAmount = null;
            var seen = new HashSet<string>(StringComparer.Ordinal);
            foreach (var property in document.RootElement.EnumerateObject())
            {
                if (!seen.Add(property.Name))
                {
                    throw new ArgumentException(
                        $"CancelAssetLock JSON duplicates `{property.Name}`.",
                        nameof(json));
                }
                if (property.Value.ValueKind != JsonValueKind.String)
                {
                    throw new ArgumentException(
                        $"CancelAssetLock JSON field `{property.Name}` must be a string.",
                        nameof(json));
                }

                switch (property.Name)
                {
                    case "escrow_id":
                        escrowId = property.Value.GetString();
                        break;
                    case "expected_remaining_amount":
                        expectedRemainingAmount = property.Value.GetString();
                        break;
                    default:
                        throw new ArgumentException(
                            $"CancelAssetLock JSON contains unknown field `{property.Name}`.",
                            nameof(json));
                }
            }

            if (escrowId is null || expectedRemainingAmount is null)
            {
                throw new ArgumentException(
                    "CancelAssetLock JSON requires exactly `escrow_id` and `expected_remaining_amount`.",
                    nameof(json));
            }
            return FromEscrowId(escrowId, expectedRemainingAmount);
        }
        catch (JsonException error)
        {
            throw new ArgumentException(
                "CancelAssetLock JSON must be a valid bounded V1 object.",
                nameof(json),
                error);
        }
    }

    internal override string WireId => NativeTypeName;

    internal override string TypeName => NativeTypeName;

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        ArgumentNullException.ThrowIfNull(context);
        return EncodeNativePayload(compactLengths: false);
    }

    private byte[] EncodeNativePayload(bool compactLengths)
    {
        var writer = new AssetLockInstructionValidation.FieldWriter(
            compactLengths);
        writer.WriteField(escrowId);
        writer.WriteField(
            AssetLockInstructionValidation.EncodeQuantity(
                expectedRemainingAmount,
                compactLengths));
        return writer.ToArray();
    }

    private byte[] EncodeJson(bool envelope)
    {
        var buffer = new ArrayBufferWriter<byte>();
        using (var writer = new Utf8JsonWriter(
            buffer,
            new JsonWriterOptions { Indented = true }))
        {
            writer.WriteStartObject();
            if (envelope)
            {
                writer.WritePropertyName("CancelAssetLock");
                writer.WriteStartObject();
            }
            writer.WriteString("escrow_id", EscrowId);
            writer.WriteString(
                "expected_remaining_amount",
                ExpectedRemainingAmount);
            if (envelope)
            {
                writer.WriteEndObject();
            }
            writer.WriteEndObject();
        }

        var output = new byte[buffer.WrittenCount + 1];
        buffer.WrittenSpan.CopyTo(output);
        output[^1] = (byte)'\n';
        return output;
    }

    private static JsonDocumentOptions StrictJsonDocumentOptions =>
        new()
        {
            AllowTrailingCommas = false,
            CommentHandling = JsonCommentHandling.Disallow,
            MaxDepth = 4,
        };

    private static CancelAssetLockInstruction DecodeNativePayload(
        ReadOnlySpan<byte> payload,
        bool compactLengths,
        string parameterName)
    {
        var reader = new AssetLockInstructionValidation.FixedFieldReader(
            payload,
            "CancelAssetLock",
            parameterName,
            compactLengths);
        var escrowId = reader.ReadField("escrow_id");
        var expectedRemainingAmount = reader.ReadField(
            "expected_remaining_amount");
        reader.RequireEnd();
        return new CancelAssetLockInstruction(
            AssetLockInstructionValidation.RequireEscrowId(
                escrowId,
                parameterName),
            AssetLockInstructionValidation.DecodePositiveQuantity(
                expectedRemainingAmount,
                parameterName,
                compactLengths));
    }
}

internal static class AssetLockInstructionValidation
{
    private const int MaximumNativePayloadBytesV1 = 256;

    internal static byte[] DeriveEscrowId(
        string? lockId,
        string parameterName)
    {
        if (string.IsNullOrEmpty(lockId)
            || IsBoundaryWhitespace(lockId[0])
            || IsBoundaryWhitespace(lockId[^1]))
        {
            throw new ArgumentException(
                "Lock id must be an exact non-empty string without surrounding whitespace.",
                parameterName);
        }
        var byteLength = Encoding.UTF8.GetByteCount(lockId);
        if (byteLength > CancelAssetLockInstruction.MaximumLockIdUtf8BytesV1)
        {
            throw new ArgumentException(
                $"Lock id must contain at most {CancelAssetLockInstruction.MaximumLockIdUtf8BytesV1} UTF-8 bytes.",
                parameterName);
        }
        return IrohaHash.Hash(Encoding.UTF8.GetBytes(lockId));
    }

    private static bool IsBoundaryWhitespace(char value)
    {
        return char.IsWhiteSpace(value) || value == '\uFEFF';
    }

    internal static byte[] RequireEscrowId(
        ReadOnlySpan<byte> escrowId,
        string parameterName)
    {
        if (escrowId.Length != IrohaHash.Length
            || (escrowId[^1] & 1) == 0)
        {
            throw new ArgumentException(
                "Escrow id must be a 32-byte native Iroha hash with its marker bit set.",
                parameterName);
        }
        return escrowId.ToArray();
    }

    internal static string FormatEscrowId(ReadOnlySpan<byte> escrowId)
    {
        RequireEscrowId(escrowId, nameof(escrowId));
        var body = Convert.ToHexString(escrowId);
        var checksum = Crc16(Encoding.ASCII.GetBytes($"hash:{body}"));
        return $"hash:{body}#{checksum:X4}";
    }

    internal static byte[] ParseEscrowId(
        string? literal,
        string parameterName)
    {
        if (literal is null
            || literal.Length != 74
            || !literal.StartsWith("hash:", StringComparison.Ordinal)
            || literal[69] != '#')
        {
            throw new ArgumentException(
                "Escrow id must be a canonical checksummed Norito Hash literal.",
                parameterName);
        }

        var body = literal.AsSpan(5, 64);
        var checksum = literal.AsSpan(70, 4);
        if (body.IndexOfAnyExcept("0123456789ABCDEF".AsSpan()) >= 0
            || checksum.IndexOfAnyExcept("0123456789ABCDEF".AsSpan()) >= 0
            || !ushort.TryParse(
                checksum,
                NumberStyles.HexNumber,
                CultureInfo.InvariantCulture,
                out var supplied)
            || supplied != Crc16(
                Encoding.ASCII.GetBytes($"hash:{body.ToString()}")))
        {
            throw new ArgumentException(
                "Escrow id has a malformed or invalid Norito Hash checksum.",
                parameterName);
        }

        return RequireEscrowId(
            Convert.FromHexString(body),
            parameterName);
    }

    internal static NumericV1.QuantityValue RequirePositiveQuantity(
        string? value,
        string parameterName)
    {
        if (value is null)
        {
            throw new ArgumentException(
                "Expected remaining amount must not be null.",
                parameterName);
        }

        NumericV1.QuantityValue quantity;
        try
        {
            quantity = NumericV1.QuantityValue.ParseCanonical(value);
        }
        catch (NumericV1.NumericException error)
        {
            throw new ArgumentException(
                "Expected remaining amount must be a positive canonical V1 quantity.",
                parameterName,
                error);
        }
        return RequirePositiveQuantity(quantity, parameterName);
    }

    internal static NumericV1.QuantityValue RequirePositiveQuantity(
        NumericV1.QuantityValue? value,
        string parameterName)
    {
        if (value is null)
        {
            throw new ArgumentNullException(parameterName);
        }
        if (value.Mantissa <= BigInteger.Zero)
        {
            throw new ArgumentOutOfRangeException(
                parameterName,
                "Expected remaining amount must be greater than zero.");
        }
        return value;
    }

    internal static byte[] EncodeQuantity(
        NumericV1.QuantityValue value,
        bool compactLengths)
    {
        RequirePositiveQuantity(value, nameof(value));
        var mantissaBytes = value.Mantissa.ToByteArray(
            isUnsigned: false,
            isBigEndian: false);

        var mantissa = new OfflineNoritoWriter();
        mantissa.WriteUInt32LittleEndian((uint)mantissaBytes.Length);
        mantissa.WriteBytes(mantissaBytes);

        var writer = new FieldWriter(compactLengths);
        writer.WriteField(mantissa.ToArray());
        var scale = new byte[sizeof(uint)];
        BinaryPrimitives.WriteUInt32LittleEndian(
            scale,
            checked((uint)value.Scale));
        writer.WriteField(scale);
        return writer.ToArray();
    }

    internal static NumericV1.QuantityValue DecodePositiveQuantity(
        ReadOnlySpan<byte> payload,
        string parameterName,
        bool compactLengths)
    {
        var reader = new FixedFieldReader(
            payload,
            "CancelAssetLock.expected_remaining_amount",
            parameterName,
            compactLengths);
        var mantissaPayload = reader.ReadField("mantissa");
        var scalePayload = reader.ReadField("scale");
        reader.RequireEnd();
        if (mantissaPayload.Length < sizeof(uint)
            || scalePayload.Length != sizeof(uint))
        {
            throw new ArgumentException(
                "CancelAssetLock quantity uses an invalid native V1 layout.",
                parameterName);
        }

        var mantissaLength = BinaryPrimitives.ReadUInt32LittleEndian(
            mantissaPayload);
        if (mantissaLength > 64
            || mantissaLength != (uint)(mantissaPayload.Length - sizeof(uint)))
        {
            throw new ArgumentException(
                "CancelAssetLock quantity mantissa length is invalid.",
                parameterName);
        }
        var mantissaBytes = mantissaPayload[sizeof(uint)..];
        if ((mantissaBytes.Length == 1 && mantissaBytes[0] == 0)
            || (mantissaBytes.Length > 1
                && ((mantissaBytes[^1] == 0
                        && (mantissaBytes[^2] & 0x80) == 0)
                    || (mantissaBytes[^1] == 0xff
                        && (mantissaBytes[^2] & 0x80) != 0))))
        {
            throw new ArgumentException(
                "CancelAssetLock quantity mantissa is not minimally encoded.",
                parameterName);
        }

        var mantissa = mantissaBytes.IsEmpty
            ? BigInteger.Zero
            : new BigInteger(
                mantissaBytes,
                isUnsigned: false,
                isBigEndian: false);
        var scale = BinaryPrimitives.ReadUInt32LittleEndian(scalePayload);
        if (scale > NumericV1.MaxScale)
        {
            throw new ArgumentException(
                $"CancelAssetLock quantity scale exceeds {NumericV1.MaxScale}.",
                parameterName);
        }

        NumericV1.QuantityValue quantity;
        try
        {
            quantity = NumericV1.QuantityValue.FromMantissa(
                mantissa,
                checked((int)scale));
        }
        catch (NumericV1.NumericException error)
        {
            throw new ArgumentException(
                "CancelAssetLock quantity is outside the canonical V1 domain.",
                parameterName,
                error);
        }
        RequirePositiveQuantity(quantity, parameterName);
        if (!EncodeQuantity(
                quantity,
                compactLengths).AsSpan().SequenceEqual(payload))
        {
            throw new ArgumentException(
                "CancelAssetLock quantity is not canonically encoded.",
                parameterName);
        }
        return quantity;
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

    internal sealed class FieldWriter
    {
        private readonly List<byte> buffer = [];
        private readonly bool compactLengths;

        internal FieldWriter(bool compactLengths)
        {
            this.compactLengths = compactLengths;
        }

        internal void WriteField(ReadOnlySpan<byte> payload)
        {
            if (compactLengths)
            {
                WriteVarUInt64(checked((ulong)payload.Length));
            }
            else
            {
                Span<byte> length = stackalloc byte[sizeof(ulong)];
                BinaryPrimitives.WriteUInt64LittleEndian(
                    length,
                    checked((ulong)payload.Length));
                WriteBytes(length);
            }
            WriteBytes(payload);
        }

        internal byte[] ToArray() => [.. buffer];

        private void WriteVarUInt64(ulong value)
        {
            do
            {
                var next = (byte)(value & 0x7f);
                value >>= 7;
                if (value != 0)
                {
                    next |= 0x80;
                }
                buffer.Add(next);
            }
            while (value != 0);
        }

        private void WriteBytes(ReadOnlySpan<byte> bytes)
        {
            foreach (var value in bytes)
            {
                buffer.Add(value);
            }
        }
    }

    internal ref struct FixedFieldReader
    {
        private readonly ReadOnlySpan<byte> payload;
        private readonly string context;
        private readonly string parameterName;
        private int offset;

        internal FixedFieldReader(
            ReadOnlySpan<byte> payload,
            string context,
            string parameterName,
            bool compactLengths = false)
        {
            if (payload.Length > MaximumNativePayloadBytesV1)
            {
                throw new ArgumentException(
                    $"{context} exceeds the bounded native V1 payload size.",
                    parameterName);
            }
            this.payload = payload;
            this.context = context;
            this.parameterName = parameterName;
            this.compactLengths = compactLengths;
            offset = 0;
        }

        private readonly bool compactLengths;

        internal ReadOnlySpan<byte> ReadField(string fieldName)
        {
            ulong length;
            if (compactLengths)
            {
                length = ReadCanonicalVarUInt64(fieldName);
            }
            else
            {
                if (offset > payload.Length - sizeof(ulong))
                {
                    throw new ArgumentException(
                        $"{context}.{fieldName} length is truncated.",
                        parameterName);
                }
                length = BinaryPrimitives.ReadUInt64LittleEndian(
                    payload.Slice(offset, sizeof(ulong)));
                offset += sizeof(ulong);
            }
            if (length > int.MaxValue
                || (int)length > payload.Length - offset)
            {
                throw new ArgumentException(
                    $"{context}.{fieldName} length is invalid.",
                    parameterName);
            }
            var field = payload.Slice(offset, (int)length);
            offset += (int)length;
            return field;
        }

        private ulong ReadCanonicalVarUInt64(string fieldName)
        {
            ulong value = 0;
            var shift = 0;
            for (var index = 0; index < 10; index++)
            {
                if (offset >= payload.Length)
                {
                    throw new ArgumentException(
                        $"{context}.{fieldName} compact length is truncated.",
                        parameterName);
                }
                var current = payload[offset++];
                if (index == 9 && (current & 0xfe) != 0)
                {
                    throw new ArgumentException(
                        $"{context}.{fieldName} compact length overflows u64.",
                        parameterName);
                }
                value |= (ulong)(current & 0x7f) << shift;
                if ((current & 0x80) == 0)
                {
                    if (index > 0 && current == 0)
                    {
                        throw new ArgumentException(
                            $"{context}.{fieldName} compact length is not canonical.",
                            parameterName);
                    }
                    return value;
                }
                shift += 7;
            }
            throw new ArgumentException(
                $"{context}.{fieldName} compact length is invalid.",
                parameterName);
        }

        internal void RequireEnd()
        {
            if (offset != payload.Length)
            {
                throw new ArgumentException(
                    $"{context} contains trailing fields or bytes.",
                    parameterName);
            }
        }
    }
}
