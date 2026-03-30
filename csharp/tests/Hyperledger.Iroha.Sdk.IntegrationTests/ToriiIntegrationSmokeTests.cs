using Hyperledger.Iroha.Torii;
using Hyperledger.Iroha.Http;

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

        var explorerAccounts = await client.GetExplorerAccountsAsync(new ToriiExplorerAccountsQuery
        {
            Page = 1,
            PerPage = 1,
        });
        Assert.True(explorerAccounts.Pagination.TotalItems >= (ulong)explorerAccounts.Items.Count);
        if (explorerAccounts.Items.Count > 0)
        {
            var explorerAccount = await client.GetExplorerAccountAsync(explorerAccounts.Items[0].Id);
            Assert.Equal(explorerAccounts.Items[0].Id, explorerAccount.Id);
        }

        var explorerDomains = await client.GetExplorerDomainsAsync(new ToriiExplorerDomainsQuery
        {
            Page = 1,
            PerPage = 1,
        });
        Assert.True(explorerDomains.Pagination.TotalItems >= (ulong)explorerDomains.Items.Count);
        if (explorerDomains.Items.Count > 0)
        {
            var explorerDomain = await client.GetExplorerDomainAsync(explorerDomains.Items[0].Id);
            Assert.Equal(explorerDomains.Items[0].Id, explorerDomain.Id);
        }

        var explorerAssetDefinitions = await client.GetExplorerAssetDefinitionsAsync(new ToriiExplorerAssetDefinitionsQuery
        {
            Page = 1,
            PerPage = 1,
        });
        Assert.True(explorerAssetDefinitions.Pagination.TotalItems >= (ulong)explorerAssetDefinitions.Items.Count);
        if (explorerAssetDefinitions.Items.Count > 0)
        {
            var explorerAssetDefinition = await client.GetExplorerAssetDefinitionAsync(explorerAssetDefinitions.Items[0].Id);
            Assert.Equal(explorerAssetDefinitions.Items[0].Id, explorerAssetDefinition.Id);
        }

        var explorerAssets = await client.GetExplorerAssetsAsync(new ToriiExplorerAssetsQuery
        {
            Page = 1,
            PerPage = 1,
        });
        Assert.True(explorerAssets.Pagination.TotalItems >= (ulong)explorerAssets.Items.Count);
        if (explorerAssets.Items.Count > 0)
        {
            var explorerAsset = await client.GetExplorerAssetAsync(explorerAssets.Items[0].Id);
            Assert.Equal(explorerAssets.Items[0].Id, explorerAsset.Id);
        }

        var explorerNfts = await client.GetExplorerNftsAsync(new ToriiExplorerNftsQuery
        {
            Page = 1,
            PerPage = 1,
        });
        Assert.True(explorerNfts.Pagination.TotalItems >= (ulong)explorerNfts.Items.Count);
        if (explorerNfts.Items.Count > 0)
        {
            var explorerNft = await client.GetExplorerNftAsync(explorerNfts.Items[0].Id);
            Assert.Equal(explorerNfts.Items[0].Id, explorerNft.Id);
        }

        var explorerRwas = await client.GetExplorerRwasAsync(new ToriiExplorerRwasQuery
        {
            Page = 1,
            PerPage = 1,
        });
        Assert.True(explorerRwas.Pagination.TotalItems >= (ulong)explorerRwas.Items.Count);
        if (explorerRwas.Items.Count > 0)
        {
            var explorerRwa = await client.GetExplorerRwaAsync(explorerRwas.Items[0].Id);
            Assert.Equal(explorerRwas.Items[0].Id, explorerRwa.Id);
        }

        var faucetPuzzle = await client.GetAccountFaucetPuzzleAsync();
        Assert.Equal("scrypt-leading-zero-bits-v1", faucetPuzzle.Algorithm);
        Assert.True(faucetPuzzle.AnchorHeight > 0);
        Assert.True(faucetPuzzle.MaxAnchorAgeBlocks > 0);

        var identifierPolicies = await client.GetIdentifierPoliciesAsync();
        Assert.True(identifierPolicies.Total >= identifierPolicies.Items.Count);

        var vpnProfile = await client.GetVpnProfileAsync();
        Assert.True(vpnProfile.SupportedExitClasses.Count >= 0);
        Assert.True(vpnProfile.LeaseSeconds > 0 || !vpnProfile.Available);

        var denylistCatalog = await client.GetSoraFsDenylistCatalogAsync();
        Assert.True(denylistCatalog.Version >= 1);
        Assert.True(denylistCatalog.Packs.Count >= 0);

        if (denylistCatalog.Packs.Count > 0)
        {
            var denylistPack = await client.GetSoraFsDenylistPackAsync(denylistCatalog.Packs[0].PackId);
            Assert.Equal(denylistCatalog.Packs[0].PackId, denylistPack.PackId);
        }

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

        var smokeContractNamespace = Environment.GetEnvironmentVariable("IROHA_CSHARP_SMOKE_CONTRACT_NAMESPACE");
        var contractNamespace = string.IsNullOrWhiteSpace(smokeContractNamespace)
            ? "universal"
            : smokeContractNamespace.Trim();
        var contractInstances = await client.GetContractInstancesAsync(
            contractNamespace,
            new ToriiContractInstancesQuery
            {
                Limit = 1,
            });
        Assert.True(contractInstances.Total >= (ulong)contractInstances.Instances.Count);

        var contractCodeHash = Environment.GetEnvironmentVariable("IROHA_CSHARP_SMOKE_CONTRACT_CODE_HASH")?.Trim();
        if (string.IsNullOrWhiteSpace(contractCodeHash) && contractInstances.Instances.Count > 0)
        {
            contractCodeHash = contractInstances.Instances[0].CodeHashHex;
        }

        if (!string.IsNullOrWhiteSpace(contractCodeHash))
        {
            var contractCode = await client.GetContractCodeAsync(contractCodeHash);
            Assert.Equal(contractCodeHash, contractCode.Manifest.CodeHash);

            var contractBytes = await client.GetContractCodeBytesAsync(contractCodeHash);
            Assert.NotEmpty(contractBytes);

            var contractView = await client.GetContractCodeViewAsync(contractCodeHash);
            Assert.False(string.IsNullOrWhiteSpace(contractView.CodeHash));
            Assert.False(string.IsNullOrWhiteSpace(contractView.RenderedSourceKind));
        }

        var smokeCid = Environment.GetEnvironmentVariable("IROHA_CSHARP_SMOKE_SORAFS_CID");
        if (!string.IsNullOrWhiteSpace(smokeCid))
        {
            var normalizedCid = smokeCid.Trim();
            var cidLookup = await client.GetSoraFsCidLookupAsync(normalizedCid);
            Assert.Equal(normalizedCid, cidLookup.ContentCid);

            var smokePath = Environment.GetEnvironmentVariable("IROHA_CSHARP_SMOKE_SORAFS_PATH");
            var content = await client.GetSoraFsCidContentAsync(normalizedCid, smokePath);
            Assert.NotEmpty(content.Bytes);
        }

        var canonicalCredentials = TryGetCanonicalRequestCredentials();
        if (canonicalCredentials is not null)
        {
            using var signedClient = new ToriiClient(
                new Uri(baseUrl, UriKind.Absolute),
                options: new ToriiClientOptions
                {
                    CanonicalRequestCredentials = canonicalCredentials,
                });

            var session = await signedClient.CreateVpnSessionAsync();
            Assert.False(string.IsNullOrWhiteSpace(session.SessionId));

            var deleted = await signedClient.DeleteVpnSessionAsync(session.SessionId);
            Assert.Equal(session.SessionId, deleted.SessionId);
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

    private static CanonicalRequestCredentials? TryGetCanonicalRequestCredentials()
    {
        var accountId = Environment.GetEnvironmentVariable("IROHA_CSHARP_CANONICAL_ACCOUNT_ID");
        var seedHex = Environment.GetEnvironmentVariable("IROHA_CSHARP_PRIVATE_KEY_SEED_HEX");
        if (string.IsNullOrWhiteSpace(accountId) || string.IsNullOrWhiteSpace(seedHex))
        {
            return null;
        }

        return new CanonicalRequestCredentials(accountId.Trim(), Convert.FromHexString(seedHex.Trim()));
    }
}
