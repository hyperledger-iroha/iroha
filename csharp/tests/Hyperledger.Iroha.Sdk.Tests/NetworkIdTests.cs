using System.Text.Json;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class NetworkIdTests
{
    private const string Canonical =
        "32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149";

    [Fact]
    public void NetworkIdParsesCanonicalLowercaseGenesisHashLiteral()
    {
        var first = NetworkId.Parse(Canonical);
        var second = NetworkId.Parse(Canonical);

        Assert.Equal(Canonical, first.ToString());
        Assert.Equal(32, first.ToBytes().Length);
        Assert.Equal(first, second);
        Assert.True(first == second);

        var copy = first.ToBytes();
        copy[0] ^= 0xff;
        Assert.Equal(0x32, first.ToBytes()[0]);

        var json = JsonSerializer.Serialize(first);
        Assert.Equal($"\"{Canonical}\"", json);
        Assert.Equal(first, JsonSerializer.Deserialize<NetworkId>(json));
    }

    [Theory]
    [InlineData("00000042")]
    [InlineData("genesis")]
    [InlineData("32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149")]
    [InlineData("32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91148")]
    [InlineData("32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f9114")]
    [InlineData("g2c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149")]
    [InlineData("hash:32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149#a2f0")]
    [InlineData("hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0")]
    public void NetworkIdRejectsLabelsAliasesAndNoncanonicalLiterals(string value)
    {
        Assert.Throws<FormatException>(() => NetworkId.Parse(value));
    }
}
