using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class TairaTestnetProfileTests
{
    private const string NetworkIdLiteral =
        "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0";

    [Fact]
    public void ExposesExactPublicMetadata()
    {
        Assert.Equal("https://taira.sora.org/", TairaTestnetProfile.ToriiBaseUri.AbsoluteUri);
        Assert.Equal("fc56984b-2be7-431d-840e-21514d1883f0", TairaTestnetProfile.ChainId);
        Assert.Equal((ushort)369, TairaTestnetProfile.I105Discriminant);
        Assert.Equal("7ZepsJTHCVLKsrFFNZGSRGZgvBhv", TairaTestnetProfile.OfflineCashAssetDefinitionId);
        Assert.Equal("ds#boi.is", TairaTestnetProfile.OfflineCashAssetAlias);
        Assert.Equal((uint)2, TairaTestnetProfile.OfflineCashAssetScale);
        Assert.Equal("6TEAJqbb8oEPmLncoNiMRbLEK6tw", TairaTestnetProfile.XorAssetDefinitionId);
        Assert.Equal("xor#universal", TairaTestnetProfile.XorAssetAlias);
        Assert.Equal((uint)9, TairaTestnetProfile.XorAssetScale);
    }

    [Fact]
    public void ClientRequiresAndPreservesDeployedNetworkId()
    {
        var networkId = NetworkId.Parse(NetworkIdLiteral);
        using var client = TairaTestnetProfile.CreateClient(networkId);

        Assert.Equal(TairaTestnetProfile.ToriiBaseUri, client.BaseUri);
        Assert.Equal(networkId, client.Options.NetworkId);
        Assert.Throws<ArgumentNullException>(() => TairaTestnetProfile.CreateClientOptions(null!));
    }
}
