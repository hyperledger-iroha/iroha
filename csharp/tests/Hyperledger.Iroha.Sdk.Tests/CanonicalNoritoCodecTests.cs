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
