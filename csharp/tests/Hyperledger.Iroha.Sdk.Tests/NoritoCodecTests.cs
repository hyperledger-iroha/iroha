using System.Buffers.Binary;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class NoritoCodecTests
{
    private const string TestTypeName = "iroha.test.Header";
    private const byte CompactLenFlag = 0x02;

    [Fact]
    public void SchemaHashUsesCanonicalTypeNameDomain()
    {
        var schemaHash = Convert.ToHexString(NoritoCodec.SchemaHash(TestTypeName)).ToLowerInvariant();

        Assert.Equal("ae7a510c0b77d4b4c8edb67c99d83baa", schemaHash);
    }

    [Fact]
    public void EncodeWithSchemaHashUsesProvidedSchemaHash()
    {
        var schemaHash = Convert.FromHexString("862a7d77075d4d23ff6c1261db027811");
        var payload = new byte[] { 1, 2, 3 };

        var encoded = NoritoCodec.EncodeWithSchemaHash(schemaHash, payload);

        Assert.Equal("862a7d77075d4d23ff6c1261db027811", Convert.ToHexString(encoded.AsSpan(6, 16)).ToLowerInvariant());
        Assert.Equal(payload, encoded[NoritoHeader.EncodedLength..]);
        Assert.Throws<ArgumentException>(() => NoritoCodec.EncodeWithSchemaHash(schemaHash[..15], payload));
    }

    [Theory]
    [InlineData(0x00)]
    [InlineData(0x01)]
    [InlineData(CompactLenFlag)]
    [InlineData(0x03)]
    [InlineData(0x04)]
    [InlineData(0x06)]
    [InlineData(0x26)]
    [InlineData(0x27)]
    public void EncodeDecodeAcceptsSupportedNoritoV1Flags(byte flags)
    {
        var payload = new byte[] { 1, 2, 3 };
        var encoded = NoritoCodec.Encode(TestTypeName, payload, flags);

        var (decodedPayload, decodedFlags) = NoritoCodec.Decode(TestTypeName, encoded);

        Assert.Equal(payload, decodedPayload);
        Assert.Equal(flags, decodedFlags);
    }

    [Theory]
    [InlineData(0x08)]
    [InlineData(0x10)]
    [InlineData(0x20)]
    [InlineData(0x22)]
    [InlineData(0x40)]
    [InlineData(0x80)]
    public void EncodeDecodeRejectUnsupportedNoritoV1Flags(byte flags)
    {
        var payload = new byte[] { 1, 2, 3 };

        Assert.Throws<ArgumentOutOfRangeException>(() => NoritoCodec.Encode(TestTypeName, payload, flags));

        var encoded = NoritoCodec.Encode(TestTypeName, payload, CompactLenFlag);
        encoded[39] = flags;
        Assert.Throws<ArgumentException>(() => NoritoCodec.Decode(TestTypeName, encoded));
    }

    [Fact]
    public void DecodeAcceptsZeroPaddingAndSnapshotsPayload()
    {
        var payload = new byte[] { 1, 2, 3, 4 };
        var encoded = NoritoCodec.Encode(TestTypeName, payload, CompactLenFlag);
        var padded = InsertPadding(encoded, 3);

        var (decodedPayload, decodedFlags) = NoritoCodec.Decode(TestTypeName, padded);
        decodedPayload[0] = 0xFF;

        Assert.Equal(CompactLenFlag, decodedFlags);
        Assert.Equal(payload, NoritoCodec.Decode(TestTypeName, padded).Payload);
    }

    [Fact]
    public void DecodeRejectsMalformedNoritoV1Frame()
    {
        var encoded = NoritoCodec.Encode(TestTypeName, new byte[] { 1, 2, 3 }, CompactLenFlag);

        AssertRejects(encoded[..(NoritoHeader.EncodedLength - 1)]);
        AssertRejects(Mutate(encoded, 0, (byte)'X'));
        AssertRejects(Mutate(encoded, 4, 1));
        AssertRejects(Mutate(encoded, 5, 1));
        AssertRejects(Mutate(encoded, 6, (byte)(encoded[6] ^ 0xFF)));
        AssertRejects(Mutate(encoded, 22, 1));

        var tooLong = encoded.ToArray();
        BinaryPrimitives.WriteUInt64LittleEndian(
            tooLong.AsSpan(23, sizeof(ulong)),
            (ulong)(encoded.Length - NoritoHeader.EncodedLength + 1));
        AssertRejects(tooLong);

        AssertRejects(Mutate(encoded, 31, (byte)(encoded[31] ^ 0x01)));

        var nonZeroPadding = InsertPadding(encoded, 1);
        nonZeroPadding[NoritoHeader.EncodedLength] = 0x01;
        AssertRejects(nonZeroPadding);

        AssertRejects(InsertPadding(encoded, 65));
        Assert.Throws<ArgumentException>(() => NoritoCodec.DecodeWithSchemaHash(new byte[15], encoded));
    }

    [Fact]
    public void NoritoHeaderSnapshotsSchemaHashConstructorGetterAndInitValues()
    {
        var schemaHash = Convert.FromHexString("862a7d77075d4d23ff6c1261db027811");
        var header = new NoritoHeader(schemaHash, NoritoCompression.None, 3, 4, 5);
        var encoded = header.Encode();

        schemaHash[0] = 0xFF;
        var getterHash = header.SchemaHash;
        getterHash[1] = 0xEE;

        Assert.Equal("862a7d77075d4d23ff6c1261db027811", Convert.ToHexString(header.SchemaHash).ToLowerInvariant());
        Assert.NotSame(getterHash, header.SchemaHash);
        Assert.Equal(encoded, header.Encode());

        var replacement = Convert.FromHexString("00112233445566778899aabbccddeeff");
        var updated = header with { SchemaHash = replacement };
        replacement[0] = 0xFF;
        var updatedGetterHash = updated.SchemaHash;
        updatedGetterHash[15] = 0xEE;

        Assert.Equal("00112233445566778899aabbccddeeff", Convert.ToHexString(updated.SchemaHash).ToLowerInvariant());
        Assert.NotSame(updatedGetterHash, updated.SchemaHash);
        Assert.Equal("00112233445566778899aabbccddeeff", Convert.ToHexString(updated.Encode().AsSpan(6, 16)).ToLowerInvariant());
    }

    [Fact]
    public void NoritoHeaderEqualityUsesSchemaHashContents()
    {
        var first = new NoritoHeader(new byte[16], NoritoCompression.None, 3, 4, 5);
        var second = new NoritoHeader(new byte[16], NoritoCompression.None, 3, 4, 5);

        Assert.Equal(first, second);
        Assert.True(first == second);
        Assert.Equal(first.GetHashCode(), second.GetHashCode());
        Assert.Single(new HashSet<NoritoHeader> { first, second });
    }

    private static void AssertRejects(byte[] archive)
    {
        Assert.Throws<ArgumentException>(() => NoritoCodec.Decode(TestTypeName, archive));
    }

    private static byte[] Mutate(byte[] archive, int index, byte value)
    {
        var copy = archive.ToArray();
        copy[index] = value;
        return copy;
    }

    private static byte[] InsertPadding(byte[] archive, int paddingLength)
    {
        var padded = new byte[archive.Length + paddingLength];
        Array.Copy(archive, 0, padded, 0, NoritoHeader.EncodedLength);
        Array.Copy(
            archive,
            NoritoHeader.EncodedLength,
            padded,
            NoritoHeader.EncodedLength + paddingLength,
            archive.Length - NoritoHeader.EncodedLength);
        return padded;
    }
}
