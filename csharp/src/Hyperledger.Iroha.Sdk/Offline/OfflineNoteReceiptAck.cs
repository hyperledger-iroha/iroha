using System.Buffers.Binary;
using System.Text;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Offline;

public sealed class OfflineNoteReceiptAck
{
    private readonly byte[] tokenId;

    public OfflineNoteReceiptAck(
        string chainId,
        string paymentRequestId,
        byte[] tokenId,
        string recipientAccountId,
        ulong acceptedAtMs)
    {
        ChainId = RequireExactNonBlankField(chainId, "chain_id", nameof(chainId));
        PaymentRequestId = RequireExactNonBlankField(
            paymentRequestId,
            "payment_request_id",
            nameof(paymentRequestId));
        ArgumentNullException.ThrowIfNull(tokenId);
        if (tokenId.Length != OfflineNoteReceiptAckCodec.HashLength)
        {
            throw new ArgumentException("token_id must be 32 bytes.", nameof(tokenId));
        }

        this.tokenId = tokenId.ToArray();
        RecipientAccountId = OfflineNoteCanonicalPayloadCodec.CanonicalAccountId(
            recipientAccountId,
            "recipient_account_id",
            nameof(recipientAccountId));
        if (acceptedAtMs == 0)
        {
            throw new ArgumentException("accepted_at_ms must be positive.", nameof(acceptedAtMs));
        }

        AcceptedAtMs = acceptedAtMs;
    }

    public string ChainId { get; }

    public string PaymentRequestId { get; }

    public byte[] TokenId => tokenId.ToArray();

    public string TokenIdHex => Convert.ToHexString(tokenId).ToLowerInvariant();

    public string RecipientAccountId { get; }

    public ulong AcceptedAtMs { get; }

    public static OfflineNoteReceiptAck FromPaymentToken(
        OfflineNotePaymentToken token,
        string recipientAccountId,
        ulong acceptedAtMs)
    {
        ArgumentNullException.ThrowIfNull(token);
        var exactRecipient = OfflineNoteCanonicalPayloadCodec.CanonicalAccountId(
            recipientAccountId,
            "recipient_account_id",
            nameof(recipientAccountId));
        if (!token.ContainsRecipientAccountId(exactRecipient))
        {
            throw new ArgumentException(
                "payment token does not contain recipient output.",
                nameof(recipientAccountId));
        }

        return new OfflineNoteReceiptAck(
            token.ChainId,
            token.PaymentRequestId,
            token.TokenId,
            exactRecipient,
            acceptedAtMs);
    }

    public bool MatchesPaymentToken(OfflineNotePaymentToken token)
    {
        ArgumentNullException.ThrowIfNull(token);
        return string.Equals(ChainId, token.ChainId, StringComparison.Ordinal)
            && string.Equals(PaymentRequestId, token.PaymentRequestId, StringComparison.Ordinal)
            && tokenId.SequenceEqual(token.TokenId)
            && token.ContainsRecipientAccountId(RecipientAccountId);
    }

    public void RequireMatchesPaymentToken(OfflineNotePaymentToken token)
    {
        if (!MatchesPaymentToken(token))
        {
            throw new ArgumentException("receipt ACK does not match payment token.", nameof(token));
        }
    }

    private static string RequireExactNonBlankField(
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
            OfflineNoteReceiptAckCodec.StrictUtf8.GetByteCount(value);
        }
        catch (EncoderFallbackException exception)
        {
            throw new ArgumentException($"{field} must be valid UTF-8 text.", parameterName, exception);
        }

        return value;
    }
}

public static class OfflineNoteReceiptAckCodec
{
    public const string Type = "offline_receipt_ack";
    public const string TextPrefix = "wallet-offline-bearer-cash-ack:";
    public const int HashLength = 32;

    private const string EnvelopeTypeName =
        "iroha_data_model::offline::model::OfflineNoteReceiptAckEnvelope";
    private const byte CompactLenFlag = 0x02;
    private const int MaxNoritoHeaderPaddingBytes = 64;

    internal static readonly Encoding StrictUtf8 = new UTF8Encoding(false, true);

    public static byte[] EncodeNorito(OfflineNoteReceiptAck ack)
    {
        ArgumentNullException.ThrowIfNull(ack);

        using var writer = new MemoryStream();
        WriteField(writer, child => WriteString(child, ack.ChainId));
        WriteField(writer, child => WriteString(child, ack.PaymentRequestId));
        WriteField(writer, child => child.Write(ack.TokenId));
        WriteField(writer, child => WriteString(child, ack.RecipientAccountId));
        WriteField(writer, child =>
        {
            Span<byte> acceptedAt = stackalloc byte[sizeof(ulong)];
            BinaryPrimitives.WriteUInt64LittleEndian(acceptedAt, ack.AcceptedAtMs);
            child.Write(acceptedAt);
        });
        return NoritoCodec.Encode(EnvelopeTypeName, writer.ToArray(), CompactLenFlag);
    }

    public static OfflineNoteReceiptAck DecodeNorito(byte[] payload)
    {
        ArgumentNullException.ThrowIfNull(payload);

        var framePayload = DecodeArchivePayload(payload);
        var offset = 0;
        var chainId = ReadField(framePayload, ref offset, "chain_id", ReadString);
        var paymentRequestId = ReadField(framePayload, ref offset, "payment_request_id", ReadString);
        var tokenId = ReadField(framePayload, ref offset, "token_id", (byte[] fieldPayload, ref int fieldOffset) =>
            ReadBytes(fieldPayload, ref fieldOffset, HashLength, "token_id"));
        var recipientAccountId = ReadField(framePayload, ref offset, "recipient_account_id", ReadString);
        var acceptedAtMs = ReadField(framePayload, ref offset, "accepted_at_ms", ReadUInt64LittleEndian);
        if (offset != framePayload.Length)
        {
            throw InvalidField("trailing_bytes");
        }

        return new OfflineNoteReceiptAck(
            chainId,
            paymentRequestId,
            tokenId,
            recipientAccountId,
            acceptedAtMs);
    }

    public static string EncodeText(OfflineNoteReceiptAck ack)
    {
        return TextPrefix + Base64UrlEncode(EncodeNorito(ack));
    }

    public static OfflineNoteReceiptAck DecodeText(string text)
    {
        ArgumentNullException.ThrowIfNull(text);

        if (!string.Equals(text.Trim(), text, StringComparison.Ordinal) || text.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException("Offline Note receipt ACK text must be exact.", nameof(text));
        }

        if (!text.StartsWith(TextPrefix, StringComparison.Ordinal))
        {
            throw new ArgumentException("Offline Note receipt ACK prefix missing.", nameof(text));
        }

        return DecodeNorito(Base64UrlDecode(text[TextPrefix.Length..]));
    }

    public static OfflineNoteReceiptAck DecodeQrPayload(byte[] payload)
    {
        return DecodeNorito(payload);
    }

    private delegate T FieldReader<T>(byte[] payload, ref int offset);

    private static T ReadField<T>(
        byte[] payload,
        ref int offset,
        string field,
        FieldReader<T> read)
    {
        var length = ReadCompactLength(payload, ref offset, field);
        if (length > int.MaxValue || length > (ulong)(payload.Length - offset))
        {
            throw InvalidField(field);
        }

        var child = payload.AsSpan(offset, (int)length).ToArray();
        offset += (int)length;
        var childOffset = 0;
        var value = read(child, ref childOffset);
        if (childOffset != child.Length)
        {
            throw InvalidField(field);
        }

        return value;
    }

    private static string ReadString(byte[] payload, ref int offset)
    {
        var length = ReadCompactLength(payload, ref offset, "string");
        if (length > int.MaxValue || length > (ulong)(payload.Length - offset))
        {
            throw InvalidField("string");
        }

        try
        {
            var value = StrictUtf8.GetString(payload, offset, (int)length);
            offset += (int)length;
            if (string.IsNullOrWhiteSpace(value))
            {
                throw InvalidField("string");
            }

            return value;
        }
        catch (DecoderFallbackException exception)
        {
            throw new ArgumentException("Offline Note receipt ACK field string is invalid.", nameof(payload), exception);
        }
    }

    private static byte[] ReadBytes(byte[] payload, ref int offset, int length, string field)
    {
        if (length < 0 || length > payload.Length - offset)
        {
            throw InvalidField(field);
        }

        var bytes = payload.AsSpan(offset, length).ToArray();
        offset += length;
        return bytes;
    }

    private static ulong ReadUInt64LittleEndian(byte[] payload, ref int offset)
    {
        if (payload.Length - offset < sizeof(ulong))
        {
            throw InvalidField("accepted_at_ms");
        }

        var value = BinaryPrimitives.ReadUInt64LittleEndian(payload.AsSpan(offset, sizeof(ulong)));
        offset += sizeof(ulong);
        return value;
    }

    private static byte[] DecodeArchivePayload(byte[] archive)
    {
        if (archive.Length < NoritoHeader.EncodedLength)
        {
            throw InvalidField("payload");
        }

        if (archive[0] != (byte)'N'
            || archive[1] != (byte)'R'
            || archive[2] != (byte)'T'
            || archive[3] != (byte)'0'
            || archive[4] != 0
            || archive[5] != 0)
        {
            throw InvalidField("payload");
        }

        var expectedSchema = NoritoCodec.SchemaHash(EnvelopeTypeName);
        if (!archive.AsSpan(6, expectedSchema.Length).SequenceEqual(expectedSchema))
        {
            throw InvalidField("schema");
        }

        if (archive[22] != (byte)NoritoCompression.None)
        {
            throw InvalidField("layout");
        }

        var payloadLength = BinaryPrimitives.ReadUInt64LittleEndian(archive.AsSpan(23, sizeof(ulong)));
        if (payloadLength > int.MaxValue
            || payloadLength > (ulong)(archive.Length - NoritoHeader.EncodedLength))
        {
            throw InvalidField("payload");
        }

        var flags = archive[39];
        if (flags != CompactLenFlag)
        {
            throw InvalidField("layout");
        }

        var payloadLengthInt = (int)payloadLength;
        var minimumLength = NoritoHeader.EncodedLength + payloadLengthInt;
        if (minimumLength > archive.Length)
        {
            throw InvalidField("payload");
        }

        var paddingLength = archive.Length - minimumLength;
        if (paddingLength > MaxNoritoHeaderPaddingBytes)
        {
            throw InvalidField("payload");
        }

        for (var index = 0; index < paddingLength; index++)
        {
            if (archive[NoritoHeader.EncodedLength + index] != 0)
            {
                throw InvalidField("payload");
            }
        }

        var payloadOffset = NoritoHeader.EncodedLength + paddingLength;
        var decodedPayload = archive.AsSpan(payloadOffset, payloadLengthInt).ToArray();
        var expectedChecksum = BinaryPrimitives.ReadUInt64LittleEndian(archive.AsSpan(31, sizeof(ulong)));
        if (Crc64Ecma.Compute(decodedPayload) != expectedChecksum)
        {
            throw InvalidField("checksum");
        }

        return decodedPayload;
    }

    private static void WriteField(MemoryStream writer, Action<MemoryStream> write)
    {
        using var child = new MemoryStream();
        write(child);
        var payload = child.ToArray();
        WriteCompactLength(writer, (ulong)payload.Length);
        writer.Write(payload);
    }

    private static void WriteString(MemoryStream writer, string value)
    {
        var bytes = StrictUtf8.GetBytes(value);
        WriteCompactLength(writer, (ulong)bytes.Length);
        writer.Write(bytes);
    }

    private static void WriteCompactLength(MemoryStream writer, ulong value)
    {
        while (value >= 0x80)
        {
            writer.WriteByte((byte)((value & 0x7f) | 0x80));
            value >>= 7;
        }

        writer.WriteByte((byte)value);
    }

    private static ulong ReadCompactLength(byte[] payload, ref int offset, string field)
    {
        var startOffset = offset;
        ulong value = 0;
        var shift = 0;
        while (offset < payload.Length && offset - startOffset < 10)
        {
            var current = payload[offset++];
            var currentValue = current & 0x7f;
            if (shift >= 63 && currentValue > 1)
            {
                throw InvalidField(field);
            }

            value |= (ulong)currentValue << shift;
            if ((current & 0x80) == 0)
            {
                var encodedLength = offset - startOffset;
                if (encodedLength > 1 && value < (1UL << (7 * (encodedLength - 1))))
                {
                    throw InvalidField(field);
                }

                return value;
            }

            shift += 7;
        }

        throw InvalidField(field);
    }

    private static string Base64UrlEncode(byte[] payload)
    {
        return Convert.ToBase64String(payload)
            .Replace('+', '-')
            .Replace('/', '_')
            .TrimEnd('=');
    }

    private static byte[] Base64UrlDecode(string payload)
    {
        if (string.IsNullOrWhiteSpace(payload) || payload.Contains('='))
        {
            throw InvalidField("payload");
        }

        foreach (var ch in payload)
        {
            if (!((ch >= 'A' && ch <= 'Z')
                || (ch >= 'a' && ch <= 'z')
                || (ch >= '0' && ch <= '9')
                || ch == '-'
                || ch == '_'))
            {
                throw InvalidField("payload");
            }
        }

        byte[] decoded;
        try
        {
            var normalized = payload.Replace('-', '+').Replace('_', '/');
            var padding = (4 - (normalized.Length % 4)) % 4;
            normalized += new string('=', padding);
            decoded = Convert.FromBase64String(normalized);
        }
        catch (FormatException exception)
        {
            throw new ArgumentException("Offline Note receipt ACK field payload is invalid.", nameof(payload), exception);
        }

        if (!string.Equals(Base64UrlEncode(decoded), payload, StringComparison.Ordinal))
        {
            throw InvalidField("payload");
        }

        return decoded;
    }

    private static ArgumentException InvalidField(string field)
    {
        return new ArgumentException($"Offline Note receipt ACK field {field} is invalid.");
    }
}
