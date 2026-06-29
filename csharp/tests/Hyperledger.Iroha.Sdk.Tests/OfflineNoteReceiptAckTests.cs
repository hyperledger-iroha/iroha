using System.Buffers.Binary;
using System.Text;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Offline;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class OfflineNoteReceiptAckTests
{
    private const string EnvelopeTypeName =
        "iroha_data_model::offline::model::OfflineNoteReceiptAckEnvelope";
    private const byte CompactLenFlag = 0x02;
    private const string SeedHex = "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032";

    [Fact]
    public void ConstructorDefensivelyCopiesTokenIdAndRejectsInvalidFields()
    {
        var tokenId = Fixed32(0x21);
        var ack = new OfflineNoteReceiptAck(
            "iroha-mainnet",
            "payment-request-7",
            tokenId,
            AccountId(),
            1_706_000_000_333);

        tokenId[0] ^= 0xff;
        Assert.NotEqual(tokenId[0], ack.TokenId[0]);

        var returned = ack.TokenId;
        returned[1] ^= 0xff;
        Assert.NotEqual(returned[1], ack.TokenId[1]);
        Assert.Equal(Convert.ToHexString(ack.TokenId).ToLowerInvariant(), ack.TokenIdHex);

        Assert.ThrowsAny<ArgumentException>(() => new OfflineNoteReceiptAck(
            "",
            "payment-request-7",
            Fixed32(0x22),
            AccountId(),
            1));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNoteReceiptAck(
            " iroha-mainnet",
            "payment-request-7",
            Fixed32(0x23),
            AccountId(),
            1));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNoteReceiptAck(
            "iroha mainnet",
            "payment-request-7",
            Fixed32(0x23),
            AccountId(),
            1));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNoteReceiptAck(
            "iroha-mainnet",
            "\tpayment-request-7",
            Fixed32(0x24),
            AccountId(),
            1));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNoteReceiptAck(
            "iroha-mainnet",
            "payment request-7",
            Fixed32(0x24),
            AccountId(),
            1));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNoteReceiptAck(
            "iroha-mainnet",
            "payment-request-7\u0000",
            Fixed32(0x24),
            AccountId(),
            1));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNoteReceiptAck(
            "iroha-mainnet",
            "payment-request-7",
            Fixed32(0x25),
            AccountId() + " ",
            1));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNoteReceiptAck(
            "iroha-mainnet",
            "payment-request-7",
            Fixed32(0x25),
            AccountId().Insert(8, " "),
            1));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNoteReceiptAck(
            "iroha-mainnet",
            "payment-request-7",
            Fixed32(0x25),
            "merchant@sora",
            1));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNoteReceiptAck(
            "iroha-mainnet",
            "payment-request-7",
            Fixed32(0x26)[..31],
            AccountId(),
            1));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNoteReceiptAck(
            "iroha-mainnet",
            "payment-request-7",
            Fixed32(0x27),
            AccountId(),
            0));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNoteReceiptAck(
            "iroha-mainnet\ud800",
            "payment-request-7",
            Fixed32(0x28),
            AccountId(),
            1));
    }

    [Fact]
    public void NoritoCodecRoundTripsWithCompactEnvelope()
    {
        var ack = ValidAck();

        var encoded = OfflineNoteReceiptAckCodec.EncodeNorito(ack);
        Assert.Equal("NRT0"u8.ToArray(), encoded[..4]);
        Assert.Equal(0, encoded[4]);
        Assert.Equal(0, encoded[5]);
        Assert.Equal(
            NoritoCodec.SchemaHash(EnvelopeTypeName),
            encoded.AsSpan(6, 16).ToArray());
        Assert.Equal(0, encoded[22]);
        Assert.Equal(CompactLenFlag, encoded[39]);

        var payloadLength = BinaryPrimitives.ReadUInt64LittleEndian(encoded.AsSpan(23, sizeof(ulong)));
        Assert.Equal((ulong)(encoded.Length - NoritoHeader.EncodedLength), payloadLength);

        var decoded = OfflineNoteReceiptAckCodec.DecodeNorito(encoded);
        Assert.Equal(ack.ChainId, decoded.ChainId);
        Assert.Equal(ack.PaymentRequestId, decoded.PaymentRequestId);
        Assert.Equal(ack.TokenId, decoded.TokenId);
        Assert.Equal(ack.RecipientAccountId, decoded.RecipientAccountId);
        Assert.Equal(ack.AcceptedAtMs, decoded.AcceptedAtMs);

        var zeroPadded = WithHeaderPadding(encoded, new byte[] { 0, 0, 0 });
        Assert.Equal(ack.TokenIdHex, OfflineNoteReceiptAckCodec.DecodeNorito(zeroPadded).TokenIdHex);
    }

    [Fact]
    public void TextCodecRoundTripsAndRejectsAmbiguousPayloads()
    {
        var ack = ValidAck();

        var text = OfflineNoteReceiptAckCodec.EncodeText(ack);
        Assert.StartsWith(OfflineNoteReceiptAckCodec.TextPrefix, text);
        Assert.DoesNotContain("=", text);
        Assert.Equal(ack.TokenIdHex, OfflineNoteReceiptAckCodec.DecodeText(text).TokenIdHex);

        Assert.ThrowsAny<ArgumentException>(() => OfflineNoteReceiptAckCodec.DecodeText(
            text.Replace(OfflineNoteReceiptAckCodec.TextPrefix, "wallet-offline-ack-v2:")));
        Assert.ThrowsAny<ArgumentException>(() => OfflineNoteReceiptAckCodec.DecodeText($" \n{text}\t"));
        Assert.ThrowsAny<ArgumentException>(() => OfflineNoteReceiptAckCodec.DecodeText(
            text.Insert(OfflineNoteReceiptAckCodec.TextPrefix.Length, "\n")));
        Assert.ThrowsAny<ArgumentException>(() => OfflineNoteReceiptAckCodec.DecodeText(text + "="));
        Assert.ThrowsAny<ArgumentException>(() => OfflineNoteReceiptAckCodec.DecodeText(
            OfflineNoteReceiptAckCodec.TextPrefix + "abc$"));
        Assert.ThrowsAny<ArgumentException>(() => OfflineNoteReceiptAckCodec.DecodeText(
            WithNonCanonicalBase64UrlPadBits(
                text,
                OfflineNoteReceiptAckCodec.EncodeText(ValidAck(paymentRequestId: "payment-request-77")),
                OfflineNoteReceiptAckCodec.EncodeText(ValidAck(paymentRequestId: "payment-request-777")))));
    }

    [Fact]
    public void DecoderRejectsMalformedNoritoArchive()
    {
        var encoded = OfflineNoteReceiptAckCodec.EncodeNorito(ValidAck());

        AssertRejects(encoded[..(NoritoHeader.EncodedLength - 1)]);

        var wrongMagic = encoded.ToArray();
        wrongMagic[0] = 0;
        AssertRejects(wrongMagic);

        var wrongSchema = encoded.ToArray();
        wrongSchema[6] ^= 0xff;
        AssertRejects(wrongSchema);

        var compressed = encoded.ToArray();
        compressed[22] = 1;
        AssertRejects(compressed);

        var missingCompactFlag = encoded.ToArray();
        missingCompactFlag[39] = 0;
        AssertRejects(missingCompactFlag);

        foreach (var forgedKnownLayoutFlags in new byte[] { 0x03, 0x06, 0x26 })
        {
            var forgedKnownLayout = encoded.ToArray();
            forgedKnownLayout[39] = forgedKnownLayoutFlags;
            AssertRejects(forgedKnownLayout);
        }

        var unsupportedFlag = encoded.ToArray();
        unsupportedFlag[39] = 0x08;
        AssertRejects(unsupportedFlag);

        var invalidFieldBitsetCombination = encoded.ToArray();
        invalidFieldBitsetCombination[39] = 0x22;
        AssertRejects(invalidFieldBitsetCombination);

        var badChecksum = encoded.ToArray();
        badChecksum[31] ^= 0xff;
        AssertRejects(badChecksum);

        AssertRejects(WithHeaderPadding(encoded, new byte[] { 0, 1 }));
        AssertRejects(WithHeaderPadding(encoded, new byte[65]));
    }

    [Fact]
    public void DecoderRejectsFieldBoundaryAndValueMutations()
    {
        AssertRejects(AckArchive(chainId: " iroha-mainnet"));
        AssertRejects(AckArchive(chainId: "iroha mainnet"));
        AssertRejects(AckArchive(paymentRequestId: " "));
        AssertRejects(AckArchive(paymentRequestId: "payment request-7"));
        AssertRejects(AckArchive(recipientAccountId: AccountId() + "\n"));
        AssertRejects(AckArchive(recipientAccountId: AccountId().Insert(8, " ")));
        AssertRejects(AckArchive(recipientAccountId: AccountId() + "\u0000"));
        AssertRejects(AckArchive(recipientAccountId: "merchant@sora"));
        AssertRejects(AckArchive(tokenId: Fixed32(0x42)[..31]));
        AssertRejects(AckArchive(acceptedAtMs: 0));

        AssertRejects(AckArchiveFromFields(
            FieldPayload(StringPayload("iroha-mainnet").Concat(new byte[] { 0 }).ToArray()),
            FieldPayload(StringPayload("payment-request-7")),
            FieldPayload(Fixed32(0x42)),
            FieldPayload(StringPayload(AccountId())),
            FieldPayload(UInt64Payload(1_706_000_000_333))));

        AssertRejects(AckArchiveFromPayload(
            ValidAckPayload().Concat(new byte[] { 0 }).ToArray()));

        AssertRejects(AckArchiveFromPayload(new byte[] { 0x80 }));
        AssertRejects(AckArchiveFromPayload(
            new byte[] { 0x81, 0x00, 0x00 }.Concat(ValidAckPayload()).ToArray()));
        AssertRejects(AckArchiveFromFields(
            FieldPayload(new byte[] { 0x81, 0x00, (byte)'a' }),
            FieldPayload(StringPayload("payment-request-7")),
            FieldPayload(Fixed32(0x42)),
            FieldPayload(StringPayload(AccountId())),
            FieldPayload(UInt64Payload(1_706_000_000_333))));
        AssertRejects(AckArchiveFromFields(
            FieldPayload(new byte[] { 0x01, 0xff }),
            FieldPayload(StringPayload("payment-request-7")),
            FieldPayload(Fixed32(0x42)),
            FieldPayload(StringPayload(AccountId())),
            FieldPayload(UInt64Payload(1_706_000_000_333))));
    }

    private static OfflineNoteReceiptAck ValidAck(string paymentRequestId = "payment-request-7")
    {
        return new OfflineNoteReceiptAck(
            "iroha-mainnet",
            paymentRequestId,
            Fixed32(0x42),
            AccountId(),
            1_706_000_000_333);
    }

    private static string WithNonCanonicalBase64UrlPadBits(params string[] texts)
    {
        const string alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        foreach (var text in texts)
        {
            var prefixLength = text.IndexOf(':') + 1;
            Assert.True(prefixLength > 0);
            var payload = text[prefixLength..];
            if (payload.Length % 4 is not (2 or 3))
            {
                continue;
            }

            var chars = payload.ToCharArray();
            var value = alphabet.IndexOf(chars[^1]);
            Assert.True(value >= 0);
            chars[^1] = alphabet[value | 1];
            return text[..prefixLength] + new string(chars);
        }

        throw new InvalidOperationException("Test fixture text did not include base64url pad bits.");
    }

    private static void AssertRejects(byte[] payload)
    {
        Assert.ThrowsAny<ArgumentException>(() => OfflineNoteReceiptAckCodec.DecodeNorito(payload));
    }

    private static byte[] AckArchive(
        string chainId = "iroha-mainnet",
        string paymentRequestId = "payment-request-7",
        byte[]? tokenId = null,
        string? recipientAccountId = null,
        ulong acceptedAtMs = 1_706_000_000_333)
    {
        return AckArchiveFromPayload(ValidAckPayload(
            chainId,
            paymentRequestId,
            tokenId ?? Fixed32(0x42),
            recipientAccountId ?? AccountId(),
            acceptedAtMs));
    }

    private static byte[] ValidAckPayload(
        string chainId = "iroha-mainnet",
        string paymentRequestId = "payment-request-7",
        byte[]? tokenId = null,
        string? recipientAccountId = null,
        ulong acceptedAtMs = 1_706_000_000_333)
    {
        return AckArchivePayloadFields(
            FieldPayload(StringPayload(chainId)),
            FieldPayload(StringPayload(paymentRequestId)),
            FieldPayload(tokenId ?? Fixed32(0x42)),
            FieldPayload(StringPayload(recipientAccountId ?? AccountId())),
            FieldPayload(UInt64Payload(acceptedAtMs)));
    }

    private static string AccountId()
    {
        return Ed25519KeyPair.FromSeed(Convert.FromHexString(SeedHex))
            .ToAccountAddress()
            .ToI105(AccountAddress.DefaultChainDiscriminant);
    }

    private static byte[] AckArchiveFromFields(params byte[][] fields)
    {
        return AckArchiveFromPayload(AckArchivePayloadFields(fields));
    }

    private static byte[] AckArchivePayloadFields(params byte[][] fields)
    {
        using var writer = new MemoryStream();
        foreach (var field in fields)
        {
            writer.Write(field);
        }

        return writer.ToArray();
    }

    private static byte[] AckArchiveFromPayload(byte[] payload)
    {
        return NoritoCodec.Encode(EnvelopeTypeName, payload, CompactLenFlag);
    }

    private static byte[] FieldPayload(byte[] payload)
    {
        using var writer = new MemoryStream();
        WriteCompactLength(writer, (ulong)payload.Length);
        writer.Write(payload);
        return writer.ToArray();
    }

    private static byte[] StringPayload(string value)
    {
        using var writer = new MemoryStream();
        var bytes = Encoding.UTF8.GetBytes(value);
        WriteCompactLength(writer, (ulong)bytes.Length);
        writer.Write(bytes);
        return writer.ToArray();
    }

    private static byte[] UInt64Payload(ulong value)
    {
        var bytes = new byte[sizeof(ulong)];
        BinaryPrimitives.WriteUInt64LittleEndian(bytes, value);
        return bytes;
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

    private static byte[] WithHeaderPadding(byte[] archive, byte[] padding)
    {
        var padded = new byte[archive.Length + padding.Length];
        Array.Copy(archive, 0, padded, 0, NoritoHeader.EncodedLength);
        Array.Copy(padding, 0, padded, NoritoHeader.EncodedLength, padding.Length);
        Array.Copy(
            archive,
            NoritoHeader.EncodedLength,
            padded,
            NoritoHeader.EncodedLength + padding.Length,
            archive.Length - NoritoHeader.EncodedLength);
        return padded;
    }

    private static byte[] Fixed32(byte seed)
    {
        var bytes = new byte[32];
        for (var index = 0; index < bytes.Length; index++)
        {
            bytes[index] = (byte)(seed + index);
        }

        return bytes;
    }
}
