using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class CanonicalNoritoCodecTests
{
    [Theory]
    [InlineData(0UL, "00")]
    [InlineData(127UL, "7F")]
    [InlineData(128UL, "8001")]
    [InlineData(16_383UL, "FF7F")]
    [InlineData(16_384UL, "808001")]
    [InlineData(ulong.MaxValue, "FFFFFFFFFFFFFFFFFF01")]
    public void CanonicalWriterUsesMinimalCompactFieldLengthsAtBoundaries(
        ulong value,
        string expectedHex)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteCompactLength(value);

        Assert.Equal(expectedHex, Convert.ToHexString(writer.ToArray()));
    }

    [Fact]
    public void CanonicalWriterKeepsSequenceCountsFixedUInt64()
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteSequenceLength(128);

        Assert.Equal("8000000000000000", Convert.ToHexString(writer.ToArray()));
    }

    [Theory]
    [InlineData("80")]
    [InlineData("8000")]
    [InlineData("FFFFFFFFFFFFFFFFFF02")]
    [InlineData("FFFFFFFFFFFFFFFFFFFF")]
    public void CanonicalReaderRejectsTruncatedOverflowingOrOverlongLengths(string encodedHex)
    {
        var bytes = Convert.FromHexString(encodedHex);

        Assert.Throws<ArgumentException>(() => ReadCompactLength(bytes));
    }

    [Fact]
    public void CanonicalReaderRejectsTrailingBytes()
    {
        Assert.Throws<ArgumentException>(() => RequireEnd([0x00]));
    }

    [Theory]
    [InlineData(0UL, "00")]
    [InlineData(127UL, "7F")]
    [InlineData(128UL, "8001")]
    [InlineData(16_384UL, "808001")]
    [InlineData(ulong.MaxValue, "FFFFFFFFFFFFFFFFFF01")]
    public void DenseWriterAndReaderUseShortestVarintSequenceCounts(ulong value, string expectedHex)
    {
        var writer = new CanonicalNoritoWriter(NoritoWireLayout.Dense);
        Assert.Equal(NoritoWireLayout.Dense, writer.Layout);
        writer.WriteSequenceLength(value);
        var encoded = writer.ToArray();
        Assert.Equal(expectedHex, Convert.ToHexString(encoded));

        var reader = new CanonicalNoritoReader(encoded, "test", "bytes", NoritoWireLayout.Dense);
        Assert.Equal(NoritoWireLayout.Dense, reader.Layout);
        Assert.Equal(value, reader.ReadSequenceLength("count"));
        reader.RequireEnd();
    }

    [Theory]
    [InlineData(0U, "00000000", "00")]
    [InlineData(1U, "01000000", "01")]
    [InlineData(127U, "7F000000", "7F")]
    [InlineData(128U, "80000000", "8001")]
    [InlineData(300U, "2C010000", "AC02")]
    [InlineData(uint.MaxValue, "FFFFFFFF", "FFFFFFFF0F")]
    public void EnumDiscriminantsAreWordsInCompactAndVarintsInDense(uint tag, string compactHex, string denseHex)
    {
        foreach (var (layout, expectedHex) in new[]
        {
            (NoritoWireLayout.Compact, compactHex),
            (NoritoWireLayout.Dense, denseHex),
        })
        {
            var writer = new CanonicalNoritoWriter(layout);
            writer.WriteEnumTag(tag);
            var encoded = writer.ToArray();
            Assert.Equal(expectedHex, Convert.ToHexString(encoded));

            var reader = new CanonicalNoritoReader(encoded, "test", "bytes", layout);
            Assert.Equal(tag, reader.ReadEnumTag("discriminant"));
            reader.RequireEnd();
        }

        // The default writer and reader are the compact layout.
        var compact = new CanonicalNoritoWriter();
        Assert.Equal(NoritoWireLayout.Compact, compact.Layout);
        compact.WriteEnumTag(tag);
        Assert.Equal(compactHex, Convert.ToHexString(compact.ToArray()));
        var compactReader = new CanonicalNoritoReader(Convert.FromHexString(compactHex), "test", "bytes");
        Assert.Equal(NoritoWireLayout.Compact, compactReader.Layout);
        Assert.Equal(tag, compactReader.ReadEnumTag("discriminant"));
    }

    [Theory]
    [InlineData("8000")]
    [InlineData("FFFFFFFF10")]
    [InlineData("8080808010")]
    [InlineData("80")]
    [InlineData("")]
    public void DenseReaderRejectsOverlongOversizedOrTruncatedDiscriminants(string encodedHex)
    {
        var bytes = Convert.FromHexString(encodedHex);

        Assert.Throws<ArgumentException>(() =>
        {
            var reader = new CanonicalNoritoReader(bytes, "test", "bytes", NoritoWireLayout.Dense);
            _ = reader.ReadEnumTag("discriminant");
        });
    }

    [Fact]
    public void ByteElementsAreFramedPerByteInCompactAndRawInDense()
    {
        var values = new byte[] { 0x00, 0x01, 0x7f, 0xff };

        var compact = new CanonicalNoritoWriter();
        compact.WriteByteElements(values);
        Assert.Equal("01000101017F01FF", Convert.ToHexString(compact.ToArray()));
        var dense = new CanonicalNoritoWriter(NoritoWireLayout.Dense);
        dense.WriteByteElements(values);
        Assert.Equal("00017FFF", Convert.ToHexString(dense.ToArray()));

        Assert.Equal(values, ReadByteElements(compact.ToArray(), values.Length, NoritoWireLayout.Compact));
        Assert.Equal(values, ReadByteElements(dense.ToArray(), values.Length, NoritoWireLayout.Dense));

        // Neither layout reads the other's bytes, short input or a wrong element length.
        Assert.Throws<ArgumentException>(() => ReadByteElements(dense.ToArray(), values.Length, NoritoWireLayout.Compact));
        Assert.Throws<ArgumentException>(() => ReadByteElements(compact.ToArray(), values.Length, NoritoWireLayout.Dense));
        Assert.Throws<ArgumentException>(() => ReadByteElements([0x01, 0x00, 0x01], 2, NoritoWireLayout.Compact));
        Assert.Throws<ArgumentException>(() => ReadByteElements([0x02, 0x00, 0x01, 0x01], 2, NoritoWireLayout.Compact));
        Assert.Throws<ArgumentException>(() => ReadByteElements([0x00], 2, NoritoWireLayout.Dense));
        Assert.Throws<ArgumentException>(() => ReadByteElements([], -1, NoritoWireLayout.Dense));
        Assert.Throws<ArgumentException>(() => ReadByteElements([], -1, NoritoWireLayout.Compact));
    }

    [Fact]
    public void BoundedSequenceCountsAreRefusedBeforeTheRemainingInputCanHoldThem()
    {
        // Two one-byte elements fit; three do not.
        Assert.Equal(2, ReadBoundedCount([0x02, 0xAA, 0xBB], 1, NoritoWireLayout.Dense));
        Assert.Throws<ArgumentException>(() => ReadBoundedCount([0x03, 0xAA, 0xBB], 1, NoritoWireLayout.Dense));
        Assert.Throws<ArgumentException>(() => ReadBoundedCount([0x02, 0xAA, 0xBB], 2, NoritoWireLayout.Dense));
        Assert.Throws<ArgumentException>(
            () => ReadBoundedCount([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01], 1, NoritoWireLayout.Dense));

        Assert.Equal(1, ReadBoundedCount([1, 0, 0, 0, 0, 0, 0, 0, 0xAA], 1, NoritoWireLayout.Compact));
        Assert.Throws<ArgumentException>(
            () => ReadBoundedCount([2, 0, 0, 0, 0, 0, 0, 0, 0xAA], 1, NoritoWireLayout.Compact));
        Assert.Throws<ArgumentException>(
            () => ReadBoundedCount([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF], 1, NoritoWireLayout.Compact));
        Assert.Throws<ArgumentOutOfRangeException>(() => ReadBoundedCount([0x00], 0, NoritoWireLayout.Dense));
    }

    [Fact]
    public void TypedReadersAndWritersSpeakOnlyTheCompactAndDenseLayouts()
    {
        Assert.Throws<ArgumentOutOfRangeException>(() => new CanonicalNoritoWriter(NoritoWireLayout.Fixed));
        Assert.Throws<ArgumentOutOfRangeException>(() => new CanonicalNoritoWriter((NoritoWireLayout)99));
        Assert.Throws<ArgumentOutOfRangeException>(() =>
        {
            _ = new CanonicalNoritoReader([], "test", "bytes", NoritoWireLayout.Fixed);
        });
        Assert.Throws<ArgumentOutOfRangeException>(() =>
        {
            _ = new CanonicalNoritoReader([], "test", "bytes", (NoritoWireLayout)99);
        });
    }

    private static byte[] ReadByteElements(byte[] bytes, int count, NoritoWireLayout layout)
    {
        var reader = new CanonicalNoritoReader(bytes, "test", "bytes", layout);
        var values = reader.ReadByteElements(count, "elements");
        reader.RequireEnd();
        return values;
    }

    private static int ReadBoundedCount(byte[] bytes, int minimumElementBytes, NoritoWireLayout layout)
    {
        var reader = new CanonicalNoritoReader(bytes, "test", "bytes", layout);
        return reader.ReadBoundedSequenceLength("count", minimumElementBytes);
    }

    private static void ReadCompactLength(byte[] bytes)
    {
        var reader = new CanonicalNoritoReader(bytes, "test", "bytes");
        _ = reader.ReadCompactLength("field");
    }

    private static void RequireEnd(byte[] bytes)
    {
        var reader = new CanonicalNoritoReader(bytes, "test", "bytes");
        reader.RequireEnd();
    }
}
