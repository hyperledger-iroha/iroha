using System.Buffers.Binary;
using System.Text;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Offline;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class OfflineNoteReceiveRequestTests
{
    private const byte CompactLenFlag = 0x02;
    private const string AssetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
    private const string SeedHex = "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032";

    [Fact]
    public void ConstructorCanonicalizesAndDefensivelyCopiesMutableInputs()
    {
        var certificate = KeyCertificateArchive();
        var outputCommitment = Hash(0x90);
        var request = new OfflineNoteReceiveRequest(
            "iroha-mainnet",
            "payment-request-7",
            AccountId(),
            AssetDefinitionId,
            AssetId(),
            "001.2300",
            certificate,
            outputCommitment);

        certificate[0] ^= 0xff;
        outputCommitment[0] ^= 0xff;
        Assert.NotEqual(certificate[0], request.KeyCertificateNorito[0]);
        Assert.NotEqual(outputCommitment[0], request.OutputCommitment[0]);
        Assert.Equal("1.2300", request.Amount);
        Assert.Equal(AssetDefinitionId, request.AssetDefinitionId);
        Assert.Equal(AssetId(), request.AssetId);
        Assert.Equal(Convert.ToHexString(request.OutputCommitment).ToLowerInvariant(), request.OutputCommitmentHex);

        var scopedRequest = ValidRequest(assetId: AssetId("7"));
        Assert.Equal(AssetId("7"), scopedRequest.AssetId);
        Assert.Equal(AssetDefinitionId, scopedRequest.AssetDefinitionId);

        var returnedCertificate = request.KeyCertificateNorito;
        returnedCertificate[1] ^= 0xff;
        Assert.NotEqual(returnedCertificate[1], request.KeyCertificateNorito[1]);
    }

    [Fact]
    public void ConstructorRejectsMismatchedAndNonExactInputs()
    {
        AssertRejectsInput(() => ValidRequest(chainId: " iroha-mainnet"));
        AssertRejectsInput(() => ValidRequest(chainId: "iroha mainnet"));
        AssertRejectsInput(() => ValidRequest(paymentRequestId: "payment-request-7\n"));
        AssertRejectsInput(() => ValidRequest(paymentRequestId: "payment request-7"));
        AssertRejectsInput(() => ValidRequest(accountId: "merchant@sora"));
        AssertRejectsInput(() => ValidRequest(assetDefinitionId: " " + AssetDefinitionId));
        AssertRejectsInput(() => ValidRequest(assetDefinitionId: AssetDefinitionId.Insert(4, " ")));
        AssertRejectsInput(() => ValidRequest(assetDefinitionId: AssetDefinitionId + "#" + AccountId()));
        AssertRejectsInput(() => ValidRequest(assetId: AssetDefinitionId + "#" + OtherAccountId()));
        AssertRejectsInput(() => ValidRequest(assetId: AssetId("007")));
        AssertRejectsInput(() => ValidRequest(assetId: AssetId("+7")));
        AssertRejectsInput(() => ValidRequest(assetId: AssetId("18446744073709551616")));
        AssertRejectsInput(() => ValidRequest(amount: "15 7500"));
        AssertRejectsInput(() => ValidRequest(amount: "1." + new string('0', 29)));
        AssertRejectsInput(() => ValidRequest(
            keyCertificateNorito: KeyCertificateArchive(OtherAccountId())));
        AssertRejectsInput(() => ValidRequest(
            keyCertificateNorito: KeyCertificateArchive(issuerSignatureLength: 63)));
        AssertRejectsInput(() => ValidRequest(outputCommitment: EvenHash(0x90)));
        AssertRejectsInput(() => ValidRequest(outputCommitment: FixedBytes(0x90, 31, oddLastByte: true)));
    }

    [Fact]
    public void CodecRoundTripsNoritoJsonTextAndQrPayload()
    {
        var request = ValidRequest();
        var encoded = OfflineNoteReceiveRequestCodec.EncodeNorito(request);
        AssertArchiveHeader(encoded, OfflineNoteReceiveRequestCodec.EnvelopeTypeName);

        var decoded = OfflineNoteReceiveRequestCodec.DecodeNorito(encoded);
        Assert.Equal(request.ChainId, decoded.ChainId);
        Assert.Equal(request.PaymentRequestId, decoded.PaymentRequestId);
        Assert.Equal(request.AccountId, decoded.AccountId);
        Assert.Equal(request.AssetDefinitionId, decoded.AssetDefinitionId);
        Assert.Equal(request.AssetId, decoded.AssetId);
        Assert.Equal(request.Amount, decoded.Amount);
        Assert.Equal(request.KeyCertificateNorito, decoded.KeyCertificateNorito);
        Assert.Equal(request.OutputCommitment, decoded.OutputCommitment);

        Assert.Equal(request.OutputCommitment, OfflineNoteReceiveRequestCodec.DecodeJson(
            OfflineNoteReceiveRequestCodec.EncodeJson(request)).OutputCommitment);
        Assert.Equal(request.OutputCommitment, OfflineNoteReceiveRequestCodec.DecodeQrPayload(encoded).OutputCommitment);

        var text = OfflineNoteReceiveRequestCodec.EncodeText(request);
        Assert.StartsWith(OfflineNoteReceiveRequestCodec.TextPrefix, text);
        Assert.DoesNotContain("=", text);
        Assert.Equal(request.OutputCommitment, OfflineNoteReceiveRequestCodec.DecodeText(text).OutputCommitment);
    }

    [Fact]
    public void DecoderRejectsMalformedNoritoArchiveAndFields()
    {
        var encoded = OfflineNoteReceiveRequestCodec.EncodeNorito(ValidRequest());

        AssertRejects(encoded[..(NoritoHeader.EncodedLength - 1)]);

        var wrongSchema = encoded.ToArray();
        wrongSchema[6] ^= 0xff;
        AssertRejects(wrongSchema);

        var missingCompact = encoded.ToArray();
        missingCompact[39] = 0;
        AssertRejects(missingCompact);

        foreach (var forgedKnownLayoutFlags in new byte[] { 0x03, 0x06, 0x26 })
        {
            AssertRejects(WithHeaderFlags(encoded, forgedKnownLayoutFlags));
            AssertRejects(ReplaceFieldChild(
                encoded,
                6,
                BytesVec(WithHeaderFlags(KeyCertificateArchive(), forgedKnownLayoutFlags))));
        }

        var badChecksum = encoded.ToArray();
        badChecksum[31] ^= 0xff;
        AssertRejects(badChecksum);

        AssertRejects(ReplaceFieldChild(
            encoded,
            0,
            StringPayload(" iroha-mainnet")));
        AssertRejects(ReplaceFieldChild(
            encoded,
            1,
            StringPayload("payment request-7")));
        AssertRejects(ReplaceFieldChild(
            encoded,
            2,
            StringPayload("merchant@sora")));
        AssertRejects(ReplaceFieldChild(
            encoded,
            4,
            StringPayload(AssetDefinitionId + "#" + OtherAccountId())));
        AssertRejects(ReplaceFieldChild(
            encoded,
            6,
            BytesVec(KeyCertificateArchive(OtherAccountId()))));
        AssertRejects(ReplaceFieldChild(
            encoded,
            7,
            EvenHash(0x90)));
        AssertRejects(ReplaceFieldChild(
            encoded,
            7,
            Hash(0x90).Concat(new byte[] { 0 }).ToArray()));
        AssertRejects(ReceiveRequestArchiveFromPayload(Payload(encoded).Concat(new byte[] { 0 }).ToArray()));
        AssertRejects(ReceiveRequestArchiveFromPayload(new byte[] { 0x81, 0x00 }));
    }

    [Fact]
    public void TextCodecRejectsAmbiguousPayloads()
    {
        var text = OfflineNoteReceiveRequestCodec.EncodeText(ValidRequest());

        Assert.ThrowsAny<ArgumentException>(() => OfflineNoteReceiveRequestCodec.DecodeText(
            text.Replace(OfflineNoteReceiveRequestCodec.TextPrefix, "wallet-offline-receive:")));
        Assert.ThrowsAny<ArgumentException>(() => OfflineNoteReceiveRequestCodec.DecodeText($" \n{text}\t"));
        Assert.ThrowsAny<ArgumentException>(() => OfflineNoteReceiveRequestCodec.DecodeText(
            text.Insert(OfflineNoteReceiveRequestCodec.TextPrefix.Length, "\n")));
        Assert.ThrowsAny<ArgumentException>(() => OfflineNoteReceiveRequestCodec.DecodeText(text + "="));
        Assert.ThrowsAny<ArgumentException>(() => OfflineNoteReceiveRequestCodec.DecodeText(
            OfflineNoteReceiveRequestCodec.TextPrefix + "abc$"));
        Assert.ThrowsAny<ArgumentException>(() => OfflineNoteReceiveRequestCodec.DecodeText(
            WithNonCanonicalBase64UrlPadBits(
                text,
                OfflineNoteReceiveRequestCodec.EncodeText(ValidRequest(paymentRequestId: "payment-request-77")),
                OfflineNoteReceiveRequestCodec.EncodeText(ValidRequest(paymentRequestId: "payment-request-777")))));
    }

    private static OfflineNoteReceiveRequest ValidRequest(
        string chainId = "iroha-mainnet",
        string paymentRequestId = "payment-request-7",
        string? accountId = null,
        string assetDefinitionId = AssetDefinitionId,
        string? assetId = null,
        string amount = "15.7500",
        byte[]? keyCertificateNorito = null,
        byte[]? outputCommitment = null)
    {
        return new OfflineNoteReceiveRequest(
            chainId,
            paymentRequestId,
            accountId ?? AccountId(),
            assetDefinitionId,
            assetId ?? AssetId(),
            amount,
            keyCertificateNorito ?? KeyCertificateArchive(),
            outputCommitment ?? Hash(0x90));
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

    private static byte[] KeyCertificateArchive(string? accountId = null, int issuerSignatureLength = 64)
    {
        var payloadArchive = OfflineNoteCanonicalPayloadCodec.EncodeKeyCertificatePayload(
            new OfflineNoteKeyCertificatePayload(
                OfflineNoteCanonicalPayloadCodec.KeyCertificateVersion,
                "ios-app-attest",
                Convert.ToBase64String(FixedBytes(0x77, 16, oddLastByte: false)),
                "device-7",
                accountId ?? AccountId(),
                FixedBytes(0x10, 32, oddLastByte: false),
                "apple-app-attest-v1",
                "ecdsa-p256-sha256",
                FixedBytes(0x04, 65, oddLastByte: false),
                1,
                true));
        var payload = Payload(payloadArchive);
        var domain = LocateField(payload, 0);
        var certificatePayload = payload[domain.Next..]
            .Concat(FieldPayload(ConstVec(FixedBytes(0xa0, issuerSignatureLength, oddLastByte: false))))
            .ToArray();
        return NoritoCodec.Encode(OfflineNotePaymentTokenCodec.KeyCertificateTypeName, certificatePayload, CompactLenFlag);
    }

    private static string AccountId()
    {
        return Ed25519KeyPair.FromSeed(Convert.FromHexString(SeedHex))
            .ToAccountAddress()
            .ToI105(AccountAddress.DefaultChainDiscriminant);
    }

    private static string OtherAccountId()
    {
        return Ed25519KeyPair.FromSeed(FixedBytes(0x55, 32, oddLastByte: false))
            .ToAccountAddress()
            .ToI105(AccountAddress.DefaultChainDiscriminant);
    }

    private static string AssetId(string? dataspaceId = null)
    {
        var baseId = AssetDefinitionId + "#" + AccountId();
        return dataspaceId is null ? baseId : baseId + "#dataspace:" + dataspaceId;
    }

    private static void AssertArchiveHeader(byte[] archive, string typeName)
    {
        Assert.Equal("NRT0"u8.ToArray(), archive[..4]);
        Assert.Equal(NoritoCodec.SchemaHash(typeName), archive.AsSpan(6, 16).ToArray());
        Assert.Equal(0, archive[22]);
        Assert.Equal(CompactLenFlag, archive[39]);
        var payloadLength = BinaryPrimitives.ReadUInt64LittleEndian(archive.AsSpan(23, sizeof(ulong)));
        Assert.Equal((ulong)(archive.Length - NoritoHeader.EncodedLength), payloadLength);
    }

    private static void AssertRejects(byte[] payload)
    {
        AssertRejectsInput(() => OfflineNoteReceiveRequestCodec.DecodeNorito(payload));
    }

    private static void AssertRejectsInput(Action action)
    {
        var exception = Record.Exception(action);
        Assert.NotNull(exception);
        Assert.True(
            exception is ArgumentException or FormatException,
            $"Expected argument or format validation failure, got {exception.GetType().FullName}: {exception.Message}");
    }

    private static byte[] ReplaceFieldChild(byte[] archive, int fieldIndex, byte[] childPayload)
    {
        var payload = Payload(archive);
        var field = LocateField(payload, fieldIndex);
        return ReceiveRequestArchiveFromPayload(
            payload[..field.FieldStart]
                .Concat(FieldPayload(childPayload))
                .Concat(payload[field.Next..])
                .ToArray());
    }

    private static (int FieldStart, int ChildStart, int ChildLength, int Next) LocateField(
        byte[] payload,
        int fieldIndex)
    {
        var offset = 0;
        for (var index = 0; index <= fieldIndex; index++)
        {
            var fieldStart = offset;
            var childLength = (int)ReadCompactLength(payload, ref offset);
            var childStart = offset;
            var next = childStart + childLength;
            if (next > payload.Length)
            {
                throw new InvalidOperationException("invalid fixture payload");
            }

            if (index == fieldIndex)
            {
                return (fieldStart, childStart, childLength, next);
            }

            offset = next;
        }

        throw new InvalidOperationException("field not found");
    }

    private static byte[] Payload(byte[] archive)
    {
        return archive[NoritoHeader.EncodedLength..];
    }

    private static byte[] ReceiveRequestArchiveFromPayload(byte[] payload)
    {
        return NoritoCodec.Encode(OfflineNoteReceiveRequestCodec.EnvelopeTypeName, payload, CompactLenFlag);
    }

    private static byte[] WithHeaderFlags(byte[] archive, byte flags)
    {
        var mutated = archive.ToArray();
        mutated[39] = flags;
        return mutated;
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

    private static byte[] BytesVec(byte[] value)
    {
        return UInt64Payload((ulong)value.Length).Concat(value).ToArray();
    }

    private static byte[] ConstVec(byte[] value)
    {
        using var writer = new MemoryStream();
        writer.Write(UInt64Payload((ulong)value.Length));
        foreach (var b in value)
        {
            WriteCompactLength(writer, 1);
            writer.WriteByte(b);
        }

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

    private static ulong ReadCompactLength(byte[] payload, ref int offset)
    {
        ulong value = 0;
        var shift = 0;
        while (offset < payload.Length)
        {
            var current = payload[offset++];
            value |= (ulong)(current & 0x7f) << shift;
            if ((current & 0x80) == 0)
            {
                return value;
            }

            shift += 7;
        }

        throw new InvalidOperationException("invalid compact length");
    }

    private static byte[] Hash(byte seed)
    {
        return FixedBytes(seed, 32, oddLastByte: true);
    }

    private static byte[] EvenHash(byte seed)
    {
        var bytes = Hash(seed);
        bytes[^1] &= 0xfe;
        return bytes;
    }

    private static byte[] FixedBytes(byte seed, int length, bool oddLastByte)
    {
        var bytes = new byte[length];
        for (var index = 0; index < bytes.Length; index++)
        {
            bytes[index] = (byte)(seed + index);
        }

        if (oddLastByte && bytes.Length > 0)
        {
            bytes[^1] |= 1;
        }

        return bytes;
    }
}
