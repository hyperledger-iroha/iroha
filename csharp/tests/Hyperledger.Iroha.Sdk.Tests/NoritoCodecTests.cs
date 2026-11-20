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
    [InlineData(CompactLenFlag)]
    public void EncodeDecodeAcceptsSupportedNoritoV1Flags(byte flags)
    {
        var payload = new byte[] { 1, 2, 3 };
        var encoded = NoritoCodec.Encode(TestTypeName, payload, flags);

        var (decodedPayload, decodedFlags) = NoritoCodec.Decode(TestTypeName, encoded);

        Assert.Equal(payload, decodedPayload);
        Assert.Equal(flags, decodedFlags);
    }

    [Fact]
    public void EncodeDecodeRejectEveryNonCanonicalNoritoV1Flag()
    {
        var payload = new byte[] { 1, 2, 3 };
        var valid = NoritoCodec.Encode(TestTypeName, payload, CompactLenFlag);

        for (var value = 0; value <= byte.MaxValue; value++)
        {
            var flags = (byte)value;
            if (flags is 0x00 or CompactLenFlag)
            {
                continue;
            }

            Assert.Throws<ArgumentOutOfRangeException>(() => NoritoCodec.Encode(TestTypeName, payload, flags));
            Assert.Throws<ArgumentOutOfRangeException>(
                () => new NoritoHeader(new byte[16], NoritoCompression.None, 0, 0, flags).Encode());

            var encoded = Mutate(valid, 39, flags);
            Assert.Throws<ArgumentException>(() => NoritoCodec.Decode(TestTypeName, encoded));
        }
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
        var header = new NoritoHeader(schemaHash, NoritoCompression.None, 3, 4, CompactLenFlag);
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
        var first = new NoritoHeader(new byte[16], NoritoCompression.None, 3, 4, CompactLenFlag);
        var second = new NoritoHeader(new byte[16], NoritoCompression.None, 3, 4, CompactLenFlag);

        Assert.Equal(first, second);
        Assert.True(first == second);
        Assert.Equal(first.GetHashCode(), second.GetHashCode());
        Assert.Single(new HashSet<NoritoHeader> { first, second });
    }

    [Theory]
    [InlineData(NoritoWireLayout.Compact, 0, 0x02)]
    [InlineData(NoritoWireLayout.Fixed, 0, 0x00)]
    [InlineData(NoritoWireLayout.Dense, 1, 0x00)]
    public void EncodeWithLayoutDeclaresTheLayoutInTheHeaderMinorAndFlags(
        NoritoWireLayout layout,
        byte expectedMinor,
        byte expectedFlags)
    {
        var payload = new byte[] { 1, 2, 3 };
        var encoded = NoritoCodec.EncodeWithLayout(TestTypeName, payload, layout);

        Assert.Equal(NoritoHeader.EncodedLength + payload.Length, encoded.Length);
        Assert.Equal(0, encoded[4]);
        Assert.Equal(expectedMinor, encoded[5]);
        Assert.Equal(expectedFlags, encoded[NoritoHeader.EncodedLength - 1]);
        Assert.Equal(layout, NoritoCodec.ReadLayout(encoded));
        Assert.Equal(payload, NoritoCodec.DecodeWithLayout(TestTypeName, encoded, layout));

        // A frame is read only as the layout its header declares.
        foreach (var other in Enum.GetValues<NoritoWireLayout>())
        {
            if (other != layout)
            {
                Assert.Throws<ArgumentException>(() => NoritoCodec.DecodeWithLayout(TestTypeName, encoded, other));
            }
        }

        if (layout == NoritoWireLayout.Dense)
        {
            // The current-layout reader refuses a dense frame instead of returning flags 0x00.
            Assert.Throws<ArgumentException>(() => NoritoCodec.Decode(TestTypeName, encoded));
            Assert.Throws<ArgumentException>(
                () => NoritoCodec.DecodeWithSchemaHash(NoritoCodec.SchemaHash(TestTypeName), encoded));
        }
        else
        {
            // The current-layout entry points produce and read the same frame.
            Assert.Equal(encoded, NoritoCodec.Encode(TestTypeName, payload, expectedFlags));
            Assert.Equal(expectedFlags, NoritoCodec.Decode(TestTypeName, encoded).Flags);
        }
    }

    [Fact]
    public void LayoutFramesCarryExactlyTheTypeAlignmentPaddingAndDenseCarriesNone()
    {
        var payload = new byte[] { 1, 2, 3, 4 };
        foreach (var (alignment, padding) in new[] { (1, 0), (2, 0), (4, 0), (8, 0), (16, 8), (32, 24), (64, 24) })
        {
            Assert.Equal(padding, NoritoCodec.PayloadPadding(NoritoWireLayout.Compact, alignment));
            Assert.Equal(padding, NoritoCodec.PayloadPadding(NoritoWireLayout.Fixed, alignment));
            Assert.Equal(0, NoritoCodec.PayloadPadding(NoritoWireLayout.Dense, alignment));

            var compact = NoritoCodec.EncodeWithLayout(TestTypeName, payload, NoritoWireLayout.Compact, alignment);
            Assert.Equal(NoritoHeader.EncodedLength + padding + payload.Length, compact.Length);
            Assert.All(compact[NoritoHeader.EncodedLength..^payload.Length], static value => Assert.Equal(0, value));
            Assert.Equal(payload, NoritoCodec.DecodeWithLayout(TestTypeName, compact, NoritoWireLayout.Compact, alignment));
            // The untyped current-layout reader accepts the zero padding.
            Assert.Equal(payload, NoritoCodec.Decode(TestTypeName, compact).Payload);

            var dense = NoritoCodec.EncodeWithLayout(TestTypeName, payload, NoritoWireLayout.Dense, alignment);
            Assert.Equal(NoritoHeader.EncodedLength + payload.Length, dense.Length);
            Assert.Equal(payload, NoritoCodec.DecodeWithLayout(TestTypeName, dense, NoritoWireLayout.Dense, alignment));

            if (padding != 0)
            {
                // A typed read requires the exact padding of its alignment.
                Assert.Throws<ArgumentException>(
                    () => NoritoCodec.DecodeWithLayout(TestTypeName, compact, NoritoWireLayout.Compact));
                Assert.Throws<ArgumentException>(
                    () => NoritoCodec.DecodeWithLayout(TestTypeName, InsertPadding(dense, padding), NoritoWireLayout.Dense, alignment));
            }
        }

        foreach (var alignment in new[] { 0, -1, 3, 12, 128 })
        {
            Assert.Throws<ArgumentOutOfRangeException>(
                () => NoritoCodec.PayloadPadding(NoritoWireLayout.Compact, alignment));
            Assert.Throws<ArgumentOutOfRangeException>(
                () => NoritoCodec.EncodeWithLayout(TestTypeName, payload, NoritoWireLayout.Dense, alignment));
        }

        Assert.Throws<ArgumentOutOfRangeException>(
            () => NoritoCodec.EncodeWithLayout(TestTypeName, payload, (NoritoWireLayout)99));
        Assert.Throws<ArgumentOutOfRangeException>(
            () => NoritoCodec.DecodeWithLayout(TestTypeName, new byte[NoritoHeader.EncodedLength], (NoritoWireLayout)99));
    }

    [Fact]
    public void ReadLayoutUsesOnlyTheHeaderMinorAndFlags()
    {
        var encoded = NoritoCodec.Encode(TestTypeName, new byte[] { 1, 2, 3 }, CompactLenFlag);

        for (var minor = 0; minor <= byte.MaxValue; minor++)
        {
            for (var flags = 0; flags <= byte.MaxValue; flags++)
            {
                var header = Mutate(Mutate(encoded, 5, (byte)minor), NoritoHeader.EncodedLength - 1, (byte)flags);
                NoritoWireLayout? expected = (minor, flags) switch
                {
                    (0, 0x02) => NoritoWireLayout.Compact,
                    (0, 0x00) => NoritoWireLayout.Fixed,
                    (1, 0x00) => NoritoWireLayout.Dense,
                    _ => null,
                };
                if (expected is { } layout)
                {
                    Assert.Equal(layout, NoritoCodec.ReadLayout(header));
                }
                else
                {
                    Assert.Throws<ArgumentException>(() => NoritoCodec.ReadLayout(header));
                }
            }
        }

        Assert.Throws<ArgumentException>(() => NoritoCodec.ReadLayout(encoded[..(NoritoHeader.EncodedLength - 1)]));
        Assert.Throws<ArgumentException>(() => NoritoCodec.ReadLayout(Mutate(encoded, 0, (byte)'X')));
        Assert.Throws<ArgumentException>(() => NoritoCodec.ReadLayout(Mutate(encoded, 4, 1)));
    }

    [Fact]
    public void NoritoHeaderEncodesAndComparesTheMinorVersion()
    {
        var current = new NoritoHeader(new byte[16], NoritoCompression.None, 3, 4, CompactLenFlag);
        var dense = new NoritoHeader(new byte[16], NoritoCompression.None, 3, 4, 0) { Minor = 1 };

        Assert.Equal(0, current.Minor);
        Assert.Equal(0, current.Encode()[5]);
        Assert.Equal(1, dense.Encode()[5]);
        Assert.Equal(0, dense.Encode()[NoritoHeader.EncodedLength - 1]);
        Assert.NotEqual(dense, dense with { Minor = 0 });
        Assert.Equal(dense, new NoritoHeader(new byte[16], NoritoCompression.None, 3, 4, 0) { Minor = 1 });
        Assert.Equal(dense.GetHashCode(), (dense with { }).GetHashCode());

        // Dense headers carry no flags; no minor beyond 1 exists.
        Assert.Throws<ArgumentOutOfRangeException>(() => (current with { Minor = 1 }).Encode());
        for (var minor = 2; minor <= byte.MaxValue; minor++)
        {
            var value = (byte)minor;
            Assert.Throws<ArgumentOutOfRangeException>(() => (dense with { Minor = value }).Encode());
        }
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
