using System.Net;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Torii;
using Hyperledger.Iroha.Http;
using Hyperledger.Iroha.Crypto;

namespace Hyperledger.Iroha.Sdk.IntegrationTests;

public sealed class ToriiIntegrationSmokeTests
{
    private const string SmokeUaidLiteral = "uaid:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    [Fact]
    public async Task OptionalReadTreatsNotFoundAsUnavailable()
    {
        var result = await TryGetOptionalReadAsync<ToriiSoraFsCidLookupResponse>(
            () => throw new ToriiApiException(
                HttpStatusCode.NotFound,
                new Uri("https://torii.example/v1/optional"),
                responseBody: null,
                reasonPhrase: "Not Found"));

        Assert.Null(result);
    }

    [Fact]
    public async Task LiveToriiSmoke()
    {
        if (!ShouldRunLiveTests())
        {
            return;
        }

        var baseUrl = Environment.GetEnvironmentVariable("IROHA_CSHARP_TORII_BASE_URL")
            ?? "https://taira.sora.org";
        var canonicalCredentials = RequireCanonicalRequestCredentials();
        var networkId = Environment.GetEnvironmentVariable("IROHA_CSHARP_NETWORK_ID");
        if (string.IsNullOrWhiteSpace(networkId))
        {
            throw new InvalidOperationException(
                "IROHA_CSHARP_NETWORK_ID is required for authenticated runtime reads.");
        }

        using var client = new ToriiClient(
            new Uri(baseUrl, UriKind.Absolute),
            options: new ToriiClientOptions
            {
                LocalSigningContext = new ToriiLocalSigningContext(
                    NetworkId.Parse(networkId)),
                CanonicalRequestCredentials = canonicalCredentials,
            });

        var capabilities = await client.GetNodeCapabilitiesAsync(cancellationToken: TestContext.Current.CancellationToken);
        Assert.Equal(1, capabilities.AbiVersion);
        Assert.Equal(1, capabilities.DataModelVersion);
        Assert.NotEmpty(capabilities.Crypto.Curves.AllowedCurveIds);

        var activeAbi = await client.GetRuntimeAbiActiveAsync(cancellationToken: TestContext.Current.CancellationToken);
        Assert.Equal(1, activeAbi.AbiVersion);

        var accounts = await client.GetAccountsAsync(limit: 5, cancellationToken: TestContext.Current.CancellationToken);
        Assert.NotEmpty(accounts.Items);
        Assert.True(accounts.Total >= accounts.Items.Count);

        var qrSnapshot = await client.GetExplorerAccountQrAsync(accounts.Items[0].Id, cancellationToken: TestContext.Current.CancellationToken);
        Assert.Equal(accounts.Items[0].Id, qrSnapshot.CanonicalId);
        Assert.Contains("<svg", qrSnapshot.Svg, StringComparison.Ordinal);

        var explorerAccounts = await client.GetExplorerAccountsAsync(new ToriiExplorerAccountsQuery
        {
            Limit = 1,
        }, cancellationToken: TestContext.Current.CancellationToken);
        Assert.True(explorerAccounts.Items.Count <= explorerAccounts.Pagination.Limit);
        if (explorerAccounts.Items.Count > 0)
        {
            var explorerAccount = await client.GetExplorerAccountAsync(explorerAccounts.Items[0].Id, cancellationToken: TestContext.Current.CancellationToken);
            Assert.Equal(explorerAccounts.Items[0].Id, explorerAccount.Id);
        }

        var explorerDomains = await client.GetExplorerDomainsAsync(new ToriiExplorerDomainsQuery
        {
            Limit = 1,
        }, cancellationToken: TestContext.Current.CancellationToken);
        Assert.True(explorerDomains.Items.Count <= explorerDomains.Pagination.Limit);
        if (explorerDomains.Items.Count > 0)
        {
            var explorerDomain = await client.GetExplorerDomainAsync(explorerDomains.Items[0].Id, cancellationToken: TestContext.Current.CancellationToken);
            Assert.Equal(explorerDomains.Items[0].Id, explorerDomain.Id);
        }

        var explorerAssetDefinitions = await client.GetExplorerAssetDefinitionsAsync(new ToriiExplorerAssetDefinitionsQuery
        {
            Limit = 1,
        }, cancellationToken: TestContext.Current.CancellationToken);
        Assert.True(explorerAssetDefinitions.Items.Count <= explorerAssetDefinitions.Pagination.Limit);
        if (explorerAssetDefinitions.Items.Count > 0)
        {
            var explorerAssetDefinition = await client.GetExplorerAssetDefinitionAsync(explorerAssetDefinitions.Items[0].Id, cancellationToken: TestContext.Current.CancellationToken);
            Assert.Equal(explorerAssetDefinitions.Items[0].Id, explorerAssetDefinition.Id);
        }

        var explorerAssets = await client.GetExplorerAssetsAsync(new ToriiExplorerAssetsQuery
        {
            Limit = 1,
        }, cancellationToken: TestContext.Current.CancellationToken);
        Assert.True(explorerAssets.Items.Count <= explorerAssets.Pagination.Limit);
        if (explorerAssets.Items.Count > 0)
        {
            var explorerAsset = await client.GetExplorerAssetAsync(explorerAssets.Items[0].Id, cancellationToken: TestContext.Current.CancellationToken);
            Assert.Equal(explorerAssets.Items[0].Id, explorerAsset.Id);
        }

        var explorerNfts = await client.GetExplorerNftsAsync(new ToriiExplorerNftsQuery
        {
            Limit = 1,
        }, cancellationToken: TestContext.Current.CancellationToken);
        Assert.True(explorerNfts.Items.Count <= explorerNfts.Pagination.Limit);
        if (explorerNfts.Items.Count > 0)
        {
            var explorerNft = await client.GetExplorerNftAsync(explorerNfts.Items[0].Id, cancellationToken: TestContext.Current.CancellationToken);
            Assert.Equal(explorerNfts.Items[0].Id, explorerNft.Id);
        }

        var explorerRwas = await client.GetExplorerRwasAsync(new ToriiExplorerRwasQuery
        {
            Limit = 1,
        }, cancellationToken: TestContext.Current.CancellationToken);
        Assert.True(explorerRwas.Items.Count <= explorerRwas.Pagination.Limit);
        if (explorerRwas.Items.Count > 0)
        {
            var explorerRwa = await client.GetExplorerRwaAsync(explorerRwas.Items[0].Id, cancellationToken: TestContext.Current.CancellationToken);
            Assert.Equal(explorerRwas.Items[0].Id, explorerRwa.Id);
        }

        var faucetPuzzle = await client.GetAccountFaucetPuzzleAsync(cancellationToken: TestContext.Current.CancellationToken);
        Assert.Equal("scrypt-leading-zero-bits-v1", faucetPuzzle.Algorithm);
        Assert.True(faucetPuzzle.AnchorHeight > 0);
        Assert.True(faucetPuzzle.MaxAnchorAgeBlocks > 0);

        var identifierPolicies = await client.GetIdentifierPoliciesAsync(cancellationToken: TestContext.Current.CancellationToken);
        Assert.True(identifierPolicies.Total >= identifierPolicies.Items.Count);

        var vpnProfile = await client.GetVpnProfileAsync(cancellationToken: TestContext.Current.CancellationToken);
        var supportedExitClasses = Assert.IsAssignableFrom<IReadOnlyList<string>>(vpnProfile.SupportedExitClasses);
        Assert.True(supportedExitClasses.Count >= 0);
        Assert.True(vpnProfile.LeaseSeconds > 0 || !vpnProfile.Available);

        var bindings = await client.GetUaidBindingsAsync(SmokeUaidLiteral, cancellationToken: TestContext.Current.CancellationToken);
        Assert.Equal(SmokeUaidLiteral, bindings.Uaid);
        Assert.True(bindings.Dataspaces.Count >= 0);

        var manifests = await client.GetUaidManifestsAsync(SmokeUaidLiteral, cancellationToken: TestContext.Current.CancellationToken);
        Assert.Equal(SmokeUaidLiteral, manifests.Uaid);
        Assert.True(manifests.Total >= manifests.Manifests.Count);

        var aliases = await client.LookupAliasesByAccountAsync(accounts.Items[0].Id, cancellationToken: TestContext.Current.CancellationToken);
        Assert.NotNull(aliases);
        Assert.Equal(accounts.Items[0].Id, aliases!.AccountId);
        Assert.True(aliases.Total >= aliases.Items.Count);

        if (aliases.Items.Count > 0)
        {
            var resolvedAlias = await client.ResolveAccountAliasAsync(aliases.Items[0].Alias, cancellationToken: TestContext.Current.CancellationToken);
            Assert.NotNull(resolvedAlias);
            Assert.Equal(accounts.Items[0].Id, resolvedAlias!.AccountId);
        }

        var smokeContractNamespace = Environment.GetEnvironmentVariable("IROHA_CSHARP_SMOKE_CONTRACT_NAMESPACE");
        var contractNamespace = string.IsNullOrWhiteSpace(smokeContractNamespace)
            ? "universal"
            : smokeContractNamespace.Trim();
        var contractInstances = await TryGetOptionalReadAsync<ToriiContractInstancesResponse>(
            () => client.GetContractInstancesAsync(
                contractNamespace,
                new ToriiContractInstancesQuery
                {
                    Limit = 1,
                }));
        if (contractInstances is not null)
        {
            Assert.True(contractInstances.Total >= (ulong)contractInstances.Instances.Count);
        }

        var contractCodeHash = Environment.GetEnvironmentVariable("IROHA_CSHARP_SMOKE_CONTRACT_CODE_HASH")?.Trim();
        if (string.IsNullOrWhiteSpace(contractCodeHash)
            && contractInstances is not null
            && contractInstances.Instances.Count > 0)
        {
            contractCodeHash = contractInstances.Instances[0].CodeHashHex;
        }

        if (!string.IsNullOrWhiteSpace(contractCodeHash))
        {
            var contractCode = await client.GetContractCodeAsync(contractCodeHash, cancellationToken: TestContext.Current.CancellationToken);
            Assert.Equal(contractCodeHash, contractCode.Manifest.CodeHash);

            var contractBytes = await client.GetContractCodeBytesAsync(contractCodeHash, cancellationToken: TestContext.Current.CancellationToken);
            Assert.NotEmpty(contractBytes);

            var contractView = await client.GetContractCodeViewAsync(contractCodeHash, cancellationToken: TestContext.Current.CancellationToken);
            Assert.False(string.IsNullOrWhiteSpace(contractView.CodeHash));
            Assert.False(string.IsNullOrWhiteSpace(contractView.RenderedSourceKind));
        }

        var smokeCid = Environment.GetEnvironmentVariable("IROHA_CSHARP_SMOKE_SORAFS_CID");
        if (!string.IsNullOrWhiteSpace(smokeCid))
        {
            var normalizedCid = smokeCid.Trim();
            var cidLookup = await client.GetSoraFsCidLookupAsync(normalizedCid, cancellationToken: TestContext.Current.CancellationToken);
            Assert.Equal(normalizedCid, cidLookup.ContentCid);

            var smokePath = Environment.GetEnvironmentVariable("IROHA_CSHARP_SMOKE_SORAFS_PATH");
            var content = await client.GetSoraFsCidContentAsync(normalizedCid, smokePath, cancellationToken: TestContext.Current.CancellationToken);
            Assert.NotEmpty(content.Bytes);
        }

        var meteringPublicKeyHex = Convert.ToHexString(
            Ed25519Signer.GetPublicKey(canonicalCredentials.PrivateKeySeed)).ToLowerInvariant();
        var quote = await client.CreateVpnQuoteAsync(new ToriiVpnQuoteCreateRequest
        {
            MeteringPublicKeyHex = meteringPublicKeyHex,
        }, cancellationToken: TestContext.Current.CancellationToken);
        Assert.False(string.IsNullOrWhiteSpace(quote.QuoteId));
        Assert.Equal("OpenVpnLeaseEscrow", quote.OpenLeaseInstruction.WireId);
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

    private static async Task<T?> TryGetOptionalReadAsync<T>(Func<Task<T>> read)
        where T : class
    {
        try
        {
            return await read();
        }
        catch (ToriiApiException exception) when (exception.StatusCode == HttpStatusCode.NotFound)
        {
            return null;
        }
    }

    private static CanonicalRequestCredentials RequireCanonicalRequestCredentials()
    {
        var accountId = Environment.GetEnvironmentVariable("IROHA_CSHARP_CANONICAL_ACCOUNT_ID");
        var seedHex = Environment.GetEnvironmentVariable("IROHA_CSHARP_PRIVATE_KEY_SEED_HEX");
        if (string.IsNullOrWhiteSpace(accountId) || string.IsNullOrWhiteSpace(seedHex))
        {
            throw new InvalidOperationException(
                "IROHA_CSHARP_CANONICAL_ACCOUNT_ID and IROHA_CSHARP_PRIVATE_KEY_SEED_HEX "
                + "are required for authenticated runtime reads.");
        }

        return new CanonicalRequestCredentials(accountId, Convert.FromHexString(seedHex));
    }
}
