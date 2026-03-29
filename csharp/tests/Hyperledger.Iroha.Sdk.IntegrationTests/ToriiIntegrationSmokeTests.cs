using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Sdk.IntegrationTests;

public sealed class ToriiIntegrationSmokeTests
{
    private const string SmokeUaidLiteral = "uaid:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    [Fact]
    public async Task LiveToriiSmoke()
    {
        if (!ShouldRunLiveTests())
        {
            return;
        }

        var baseUrl = Environment.GetEnvironmentVariable("IROHA_CSHARP_TORII_BASE_URL")
            ?? "https://taira.sora.org";

        using var client = new ToriiClient(new Uri(baseUrl, UriKind.Absolute));

        var capabilities = await client.GetNodeCapabilitiesAsync();
        Assert.Equal(1, capabilities.AbiVersion);
        Assert.Equal(1, capabilities.DataModelVersion);
        Assert.NotEmpty(capabilities.Crypto.Curves.AllowedCurveIds);

        var activeAbi = await client.GetRuntimeAbiActiveAsync();
        Assert.Equal(1, activeAbi.AbiVersion);

        var accounts = await client.GetAccountsAsync(limit: 5);
        Assert.NotEmpty(accounts.Items);
        Assert.True(accounts.Total >= accounts.Items.Count);

        var qrSnapshot = await client.GetExplorerAccountQrAsync(accounts.Items[0].Id);
        Assert.Equal(accounts.Items[0].Id, qrSnapshot.CanonicalId);
        Assert.Contains("<svg", qrSnapshot.Svg, StringComparison.Ordinal);

        var faucetPuzzle = await client.GetAccountFaucetPuzzleAsync();
        Assert.Equal("scrypt-leading-zero-bits-v1", faucetPuzzle.Algorithm);
        Assert.True(faucetPuzzle.AnchorHeight > 0);
        Assert.True(faucetPuzzle.MaxAnchorAgeBlocks > 0);

        var identifierPolicies = await client.GetIdentifierPoliciesAsync();
        Assert.True(identifierPolicies.Total >= identifierPolicies.Items.Count);

        var bindings = await client.GetUaidBindingsAsync(SmokeUaidLiteral);
        Assert.Equal(SmokeUaidLiteral, bindings.Uaid);
        Assert.True(bindings.Dataspaces.Count >= 0);

        var manifests = await client.GetUaidManifestsAsync(SmokeUaidLiteral);
        Assert.Equal(SmokeUaidLiteral, manifests.Uaid);
        Assert.True(manifests.Total >= manifests.Manifests.Count);

        var aliases = await client.LookupAliasesByAccountAsync(accounts.Items[0].Id);
        Assert.NotNull(aliases);
        Assert.Equal(accounts.Items[0].Id, aliases!.AccountId);
        Assert.True(aliases.Total >= aliases.Items.Count);

        if (aliases.Items.Count > 0)
        {
            var resolvedAlias = await client.ResolveAccountAliasAsync(aliases.Items[0].Alias);
            Assert.NotNull(resolvedAlias);
            Assert.Equal(accounts.Items[0].Id, resolvedAlias!.AccountId);
        }
    }

    private static bool ShouldRunLiveTests()
    {
        var raw = Environment.GetEnvironmentVariable("IROHA_CSHARP_RUN_LIVE_TESTS");
        if (string.IsNullOrWhiteSpace(raw))
        {
            return false;
        }

        return raw.Equals("1", StringComparison.Ordinal)
            || raw.Equals("true", StringComparison.OrdinalIgnoreCase)
            || raw.Equals("yes", StringComparison.OrdinalIgnoreCase);
    }
}
