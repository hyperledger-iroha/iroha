using System.Buffers.Binary;
using System.Text;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Offline;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class OfflineNotePaymentTokenTests
{
    private const byte CompactLenFlag = 0x02;
    private const string AssetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
    private const string SeedHex = "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032";

    [Fact]
    public void ConstructorValidatesAuditBindingAndDefensivelyCopiesMutableInputs()
    {
        var tokenNonce = FixedBytes(0x30, 32, oddLastByte: false);
        var tokenId = Hash(0x44);
        var audit = AuditArchive(tokenId);
        var trail = new[] { AuditArchive(tokenId, proofSeed: 0x91), audit };
        var token = new OfflineNotePaymentToken(
            "iroha-mainnet",
            "payment-request-7",
            1_706_000_000_222,
            tokenNonce,
            tokenId,
            audit,
            trail);

        tokenNonce[0] ^= 0xff;
        tokenId[0] ^= 0xff;
        audit[0] ^= 0xff;
        trail[0][0] ^= 0xff;
        Assert.NotEqual(tokenNonce[0], token.TokenNonce[0]);
        Assert.NotEqual(tokenId[0], token.TokenId[0]);
        Assert.NotEqual(audit[0], token.AuditNorito[0]);
        Assert.NotEqual(trail[0][0], token.BearerAuditTrailNorito[0][0]);

        var returnedTrail = token.BearerAuditTrailNorito[0];
        returnedTrail[1] ^= 0xff;
        Assert.NotEqual(returnedTrail[1], token.BearerAuditTrailNorito[0][1]);
        Assert.True(token.ContainsRecipientAccountId(AccountId()));
        Assert.False(token.ContainsRecipientAccountId(OtherAccountId()));
        Assert.ThrowsAny<ArgumentException>(() => token.ContainsRecipientAccountId("merchant@sora"));
        Assert.ThrowsAny<ArgumentException>(() => token.ContainsRecipientAccountId(AccountId() + " "));
        Assert.ThrowsAny<ArgumentException>(() => token.ContainsRecipientAccountId(AccountId() + "\u0000"));

        Assert.ThrowsAny<ArgumentException>(() => new OfflineNotePaymentToken(
            "iroha mainnet",
            "payment-request-7",
            1,
            FixedBytes(0x30, 32, oddLastByte: false),
            Hash(0x44),
            AuditArchive(Hash(0x44))));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNotePaymentToken(
            "iroha-mainnet",
            "payment request-7",
            1,
            FixedBytes(0x30, 32, oddLastByte: false),
            Hash(0x44),
            AuditArchive(Hash(0x44))));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNotePaymentToken(
            "iroha-mainnet",
            "payment-request-7",
            0,
            FixedBytes(0x30, 32, oddLastByte: false),
            Hash(0x44),
            AuditArchive(Hash(0x44))));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNotePaymentToken(
            "iroha-mainnet",
            "payment-request-7",
            1,
            FixedBytes(0x30, 31, oddLastByte: false),
            Hash(0x44),
            AuditArchive(Hash(0x44))));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNotePaymentToken(
            "iroha-mainnet",
            "payment-request-7",
            1,
            FixedBytes(0x30, 32, oddLastByte: false),
            Hash(0x44),
            AuditArchive(Hash(0x45))));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNotePaymentToken(
            "iroha-mainnet",
            "payment-request-7",
            1,
            FixedBytes(0x30, 32, oddLastByte: false),
            Hash(0x44),
            AuditArchive(Hash(0x44)),
            Array.Empty<byte[]>()));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNotePaymentToken(
            "iroha-mainnet",
            "payment-request-7",
            1,
            FixedBytes(0x30, 32, oddLastByte: false),
            Hash(0x44),
            AuditArchive(Hash(0x44)),
            new[] { AuditArchive(Hash(0x44), proofSeed: 0x91) }));
    }

    [Fact]
    public void CodecRoundTripsNoritoJsonTextAndQrPayload()
    {
        var token = ValidToken();

        var encoded = OfflineNotePaymentTokenCodec.EncodeNorito(token);
        AssertArchiveHeader(encoded, OfflineNotePaymentTokenCodec.EnvelopeTypeName);
        var decoded = OfflineNotePaymentTokenCodec.DecodeNorito(encoded);
        Assert.Equal(token.ChainId, decoded.ChainId);
        Assert.Equal(token.PaymentRequestId, decoded.PaymentRequestId);
        Assert.Equal(token.CreatedAtMs, decoded.CreatedAtMs);
        Assert.Equal(token.TokenNonce, decoded.TokenNonce);
        Assert.Equal(token.TokenId, decoded.TokenId);
        Assert.Equal(token.AuditNorito, decoded.AuditNorito);
        Assert.Equal(token.BearerAuditTrailNorito[0], decoded.BearerAuditTrailNorito[0]);
        Assert.Equal(token.TokenIdHex, decoded.TokenIdHex);

        Assert.Equal(token.TokenId, OfflineNotePaymentTokenCodec.DecodeJson(
            OfflineNotePaymentTokenCodec.EncodeJson(token)).TokenId);
        Assert.Equal(token.TokenId, OfflineNotePaymentTokenCodec.DecodeQrPayload(encoded).TokenId);

        var text = OfflineNotePaymentTokenCodec.EncodeText(token);
        Assert.StartsWith(OfflineNotePaymentTokenCodec.TextPrefix, text);
        Assert.DoesNotContain("=", text);
        Assert.Equal(token.TokenId, OfflineNotePaymentTokenCodec.DecodeText(text).TokenId);
    }

    [Fact]
    public void ReceiptAckMatchesPaymentTokenRecipientOutputs()
    {
        var token = ValidToken();
        var ack = OfflineNoteReceiptAck.FromPaymentToken(
            token,
            AccountId(),
            1_706_000_000_333);

        Assert.True(ack.MatchesPaymentToken(token));
        ack.RequireMatchesPaymentToken(token);
        Assert.Equal(token.ChainId, ack.ChainId);
        Assert.Equal(token.PaymentRequestId, ack.PaymentRequestId);
        Assert.Equal(token.TokenId, ack.TokenId);

        Assert.ThrowsAny<ArgumentException>(() => OfflineNoteReceiptAck.FromPaymentToken(
            token,
            OtherAccountId(),
            1));
        Assert.False(new OfflineNoteReceiptAck(
            "iroha-mainnet",
            "payment-request-7",
            token.TokenId,
            OtherAccountId(),
            1).MatchesPaymentToken(token));
        Assert.ThrowsAny<ArgumentException>(() => new OfflineNoteReceiptAck(
            "iroha-mainnet",
            "wrong-request",
            token.TokenId,
            AccountId(),
            1).RequireMatchesPaymentToken(token));
    }

    [Fact]
    public void DecoderRejectsMalformedPaymentTokenEnvelope()
    {
        var token = ValidToken();
        var encoded = OfflineNotePaymentTokenCodec.EncodeNorito(token);

        AssertRejects(TokenArchiveFromPayload(new byte[] { 0 }));
        AssertRejects(ReplaceFieldChild(encoded, OfflineNotePaymentTokenCodec.EnvelopeTypeName, 0, UInt64Payload(1)));
        AssertRejects(ReplaceFieldChild(
            encoded,
            OfflineNotePaymentTokenCodec.EnvelopeTypeName,
            1,
            StringPayload(" iroha-mainnet")));
        AssertRejects(ReplaceFieldChild(
            encoded,
            OfflineNotePaymentTokenCodec.EnvelopeTypeName,
            2,
            StringPayload("payment request-7")));
        AssertRejects(ReplaceFieldChild(
            encoded,
            OfflineNotePaymentTokenCodec.EnvelopeTypeName,
            3,
            UInt64Payload(0)));
        AssertRejects(ReplaceFieldChild(
            encoded,
            OfflineNotePaymentTokenCodec.EnvelopeTypeName,
            4,
            BytesVec(FixedBytes(0x30, 31, oddLastByte: false))));
        AssertRejects(ReplaceFieldChild(
            encoded,
            OfflineNotePaymentTokenCodec.EnvelopeTypeName,
            5,
            EvenHash(0x44)));
        AssertRejects(ReplaceFieldChild(
            encoded,
            OfflineNotePaymentTokenCodec.EnvelopeTypeName,
            6,
            BytesVec(AuditArchive(Hash(0x45)))));
        AssertRejects(ReplaceFieldChild(
            encoded,
            OfflineNotePaymentTokenCodec.EnvelopeTypeName,
            7,
            UInt64Payload(0)));
        AssertRejects(ReplaceFieldChild(
            encoded,
            OfflineNotePaymentTokenCodec.EnvelopeTypeName,
            7,
            Vec(FieldPayload(BytesVec(token.AuditNorito).Concat(new byte[] { 0 }).ToArray()))));

        var wrongSchema = encoded.ToArray();
        wrongSchema[6] ^= 0xff;
        AssertRejects(wrongSchema);
        var missingCompact = encoded.ToArray();
        missingCompact[39] = 0;
        AssertRejects(missingCompact);
        foreach (var forgedKnownLayoutFlags in new byte[] { 0x03, 0x06, 0x26 })
        {
            AssertRejects(WithHeaderFlags(encoded, forgedKnownLayoutFlags));
        }

        var badChecksum = encoded.ToArray();
        badChecksum[31] ^= 0xff;
        AssertRejects(badChecksum);
    }

    [Fact]
    public void DecoderRejectsMalformedAuditBundleInsideToken()
    {
        var tokenId = Hash(0x44);
        AssertRejects(TokenArchive(audit: AuditArchive(tokenId, outputClaims: Array.Empty<byte[]>())));
        AssertRejects(TokenArchive(audit: AuditArchive(
            tokenId,
            outputCommitment: Hash(0x90),
            outputClaimNoteCommitment: Hash(0x91))));
        AssertRejects(TokenArchive(audit: AuditArchive(
            tokenId,
            certificate: KeyCertificatePayload(issuerSignatureLength: 63))));
        AssertRejects(TokenArchive(audit: AuditArchive(
            tokenId,
            recursiveProof: RecursiveProofPayload(proofBackend: "halo2/kzg"))));
        AssertRejects(TokenArchive(audit: AuditArchive(
            tokenId,
            recursiveProof: RecursiveProofPayload(proofBytes: Array.Empty<byte>()))));
        foreach (var forgedKnownLayoutFlags in new byte[] { 0x03, 0x06, 0x26 })
        {
            AssertRejects(TokenArchive(audit: WithHeaderFlags(AuditArchive(tokenId), forgedKnownLayoutFlags)));
        }

        AssertRejects(TokenArchive(audit: NoritoCodec.Encode(
            OfflineNoteCanonicalPayloadCodec.IssuedClaimTypeName,
            Payload(OfflineNoteCanonicalPayloadCodec.EncodeIssuedClaim(ValidClaim(Hash(0x20)))),
            CompactLenFlag)));
    }

    [Fact]
    public void TextCodecRejectsAmbiguousPayloads()
    {
        var text = OfflineNotePaymentTokenCodec.EncodeText(ValidToken());

        Assert.ThrowsAny<ArgumentException>(() => OfflineNotePaymentTokenCodec.DecodeText(
            text.Replace(OfflineNotePaymentTokenCodec.TextPrefix, "wallet-offline-payment:")));
        Assert.ThrowsAny<ArgumentException>(() => OfflineNotePaymentTokenCodec.DecodeText($" \n{text}\t"));
        Assert.ThrowsAny<ArgumentException>(() => OfflineNotePaymentTokenCodec.DecodeText(
            text.Insert(OfflineNotePaymentTokenCodec.TextPrefix.Length, "\n")));
        Assert.ThrowsAny<ArgumentException>(() => OfflineNotePaymentTokenCodec.DecodeText(text + "="));
        Assert.ThrowsAny<ArgumentException>(() => OfflineNotePaymentTokenCodec.DecodeText(
            OfflineNotePaymentTokenCodec.TextPrefix + "abc$"));
        Assert.ThrowsAny<ArgumentException>(() => OfflineNotePaymentTokenCodec.DecodeText(
            WithNonCanonicalBase64UrlPadBits(
                text,
                OfflineNotePaymentTokenCodec.EncodeText(ValidToken(paymentRequestId: "payment-request-77")),
                OfflineNotePaymentTokenCodec.EncodeText(ValidToken(paymentRequestId: "payment-request-777")))));
    }

    private static OfflineNotePaymentToken ValidToken(
        byte[]? tokenId = null,
        string paymentRequestId = "payment-request-7")
    {
        var checkedTokenId = tokenId ?? Hash(0x44);
        var audit = AuditArchive(checkedTokenId);
        return new OfflineNotePaymentToken(
            "iroha-mainnet",
            paymentRequestId,
            1_706_000_000_222,
            FixedBytes(0x30, 32, oddLastByte: false),
            checkedTokenId,
            audit,
            new[] { audit });
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

    private static byte[] TokenArchive(byte[]? audit = null)
    {
        var tokenId = Hash(0x44);
        return TokenArchiveFromPayload(TokenPayload(tokenId, audit ?? AuditArchive(tokenId)));
    }

    private static byte[] TokenPayload(byte[] tokenId, byte[] audit)
    {
        return PayloadFields(
            FieldPayload(UInt64Payload(OfflineNotePaymentTokenCodec.EnvelopeVersion)),
            FieldPayload(StringPayload("iroha-mainnet")),
            FieldPayload(StringPayload("payment-request-7")),
            FieldPayload(UInt64Payload(1_706_000_000_222)),
            FieldPayload(BytesVec(FixedBytes(0x30, 32, oddLastByte: false))),
            FieldPayload(tokenId),
            FieldPayload(BytesVec(audit)),
            FieldPayload(Vec(FieldPayload(BytesVec(audit)))));
    }

    private static byte[] AuditArchive(
        byte[] tokenId,
        byte[]? certificate = null,
        byte[]? recursiveProof = null,
        byte[]? outputCommitment = null,
        byte[]? outputClaimNoteCommitment = null,
        byte[][]? outputClaims = null,
        byte proofSeed = 0x90)
    {
        var noteCommitment = outputClaimNoteCommitment ?? outputCommitment ?? Hash(0x20);
        var commitment = outputCommitment ?? noteCommitment;
        var claim = ValidClaim(Hash(0x20));
        var claimPayload = Payload(OfflineNoteCanonicalPayloadCodec.EncodeIssuedClaim(claim));
        var claimAsset = LocateField(claimPayload, 3);
        var claimAmount = LocateField(claimPayload, 4);
        var outputClaim = PayloadFields(
            FieldPayload(noteCommitment),
            FieldPayload(certificate ?? KeyCertificatePayload()),
            FieldPayload(claimPayload[claimAsset.ChildStart..claimAsset.Next]),
            FieldPayload(claimPayload[claimAmount.ChildStart..claimAmount.Next]));
        var checkedOutputClaims = outputClaims ?? new[] { FieldPayload(outputClaim) };
        var auditPayload = PayloadFields(
            FieldPayload(tokenId),
            FieldPayload(certificate ?? KeyCertificatePayload()),
            FieldPayload(Vec(FieldPayload(Hash(0x80)))),
            FieldPayload(Vec(FieldPayload(claimPayload))),
            FieldPayload(Vec(FieldPayload(commitment))),
            FieldPayload(Vec(checkedOutputClaims)),
            FieldPayload(recursiveProof ?? RecursiveProofPayload(proofBytes: FixedBytes(proofSeed, 48, oddLastByte: false))));
        return NoritoCodec.Encode(OfflineNotePaymentTokenCodec.AuditBundleTypeName, auditPayload, CompactLenFlag);
    }

    private static byte[] KeyCertificatePayload(int issuerSignatureLength = 64)
    {
        var payloadArchive = OfflineNoteCanonicalPayloadCodec.EncodeKeyCertificatePayload(
            new OfflineNoteKeyCertificatePayload(
                OfflineNoteCanonicalPayloadCodec.KeyCertificateVersion,
                "ios-app-attest",
                Convert.ToBase64String(FixedBytes(0x77, 16, oddLastByte: false)),
                "device-7",
                AccountId(),
                FixedBytes(0x10, 32, oddLastByte: false),
                "apple-app-attest-v1",
                "ecdsa-p256-sha256",
                FixedBytes(0x04, 65, oddLastByte: false),
                1,
                true));
        var payload = Payload(payloadArchive);
        var domain = LocateField(payload, 0);
        return payload[domain.Next..]
            .Concat(FieldPayload(ConstVec(FixedBytes(0xa0, issuerSignatureLength, oddLastByte: false))))
            .ToArray();
    }

    private static byte[] RecursiveProofPayload(
        string verifierBackend = "halo2/ipa",
        string verifierName = "offline-note-recursive",
        string proofBackend = "halo2/ipa",
        byte[]? proofBytes = null)
    {
        return PayloadFields(
            FieldPayload(PayloadFields(
                FieldPayload(StringPayload(verifierBackend)),
                FieldPayload(StringPayload(verifierName)))),
            FieldPayload(Hash(0xc0)),
            FieldPayload(PayloadFields(
                FieldPayload(StringPayload(proofBackend)),
                FieldPayload(BytesVec(proofBytes ?? FixedBytes(0x90, 48, oddLastByte: false))))));
    }

    private static OfflineNoteIssuedClaim ValidClaim(byte[] noteCommitment)
    {
        return new OfflineNoteIssuedClaim(noteCommitment, Hash(0x60), AssetId(), "15.7500");
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

    private static string AssetId()
    {
        return AssetDefinitionId + "#" + AccountId();
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
        Assert.ThrowsAny<ArgumentException>(() => OfflineNotePaymentTokenCodec.DecodeNorito(payload));
    }

    private static byte[] ReplaceFieldChild(byte[] archive, string typeName, int fieldIndex, byte[] childPayload)
    {
        var payload = Payload(archive);
        var field = LocateField(payload, fieldIndex);
        return NoritoCodec.Encode(
            typeName,
            payload[..field.FieldStart]
                .Concat(FieldPayload(childPayload))
                .Concat(payload[field.Next..])
                .ToArray(),
            CompactLenFlag);
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

    private static byte[] TokenArchiveFromPayload(byte[] payload)
    {
        return NoritoCodec.Encode(OfflineNotePaymentTokenCodec.EnvelopeTypeName, payload, CompactLenFlag);
    }

    private static byte[] WithHeaderFlags(byte[] archive, byte flags)
    {
        var mutated = archive.ToArray();
        mutated[39] = flags;
        return mutated;
    }

    private static byte[] PayloadFields(params byte[][] fields)
    {
        using var writer = new MemoryStream();
        foreach (var field in fields)
        {
            writer.Write(field);
        }

        return writer.ToArray();
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

    private static byte[] Vec(params byte[][] fields)
    {
        return UInt64Payload((ulong)fields.Length)
            .Concat(fields.SelectMany(static field => field))
            .ToArray();
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
