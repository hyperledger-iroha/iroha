using System.Net;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Http;
using Hyperledger.Iroha.Queries;
using Hyperledger.Iroha.Torii;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class ToriiClientTests
{
    private static readonly byte[] CanonicalPrivateKeySeed = Convert.FromHexString("616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032");
    private const string CanonicalAccountId = "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53";

    [Fact]
    public async Task GetHealthAsyncReturnsTextResponse()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("ok"),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var health = await client.GetHealthAsync();

        Assert.Equal("ok", health);
        Assert.Equal("/v1/health", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task GetJsonDocumentAsyncAddsBearerAndCanonicalHeaders()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("{\"ok\":true}"),
        });

        var options = new ToriiClientOptions
        {
            BearerToken = "dev-token",
            CanonicalRequestCredentials = new CanonicalRequestCredentials(
                CanonicalAccountId,
                CanonicalPrivateKeySeed),
        };

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler), options);
        using var document = await client.GetJsonDocumentAsync("/v1/query", "gas_units=100&cursor_mode=stored");

        Assert.True(document.RootElement.GetProperty("ok").GetBoolean());
        Assert.Equal("Bearer", handler.LastRequest!.Headers.Authorization?.Scheme);
        Assert.Equal("dev-token", handler.LastRequest.Headers.Authorization?.Parameter);
        Assert.True(handler.LastRequest.Headers.Contains("X-Iroha-Account"));
        Assert.True(handler.LastRequest.Headers.Contains("X-Iroha-Signature"));
        Assert.Equal(CanonicalAccountId, Assert.Single(handler.LastRequest.Headers.GetValues("X-Iroha-Account")));
        Assert.All(
            Assert.Single(handler.LastRequest.Headers.GetValues("X-Iroha-Nonce")),
            character => Assert.False(char.IsWhiteSpace(character)));
        Assert.Equal("/v1/query?gas_units=100&cursor_mode=stored", handler.LastRequest.RequestUri!.PathAndQuery);
    }

    [Theory]
    [InlineData(" ")]
    [InlineData(" sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")]
    [InlineData("sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53 ")]
    [InlineData("\u00A0sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")]
    [InlineData("sorauﾛ1N\u0000ｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")]
    public void CanonicalRequestCredentialsRejectNonExactAccountIdsBeforeToriiRequestSetup(string accountId)
    {
        Assert.Throws<ArgumentException>(() => new CanonicalRequestCredentials(
            accountId,
            CanonicalPrivateKeySeed));
    }

    [Fact]
    public async Task GetNodeCapabilitiesAsyncDeserializesTypedResponse()
    {
        const string responseBody = """
            {
              "abi_version": 1,
              "data_model_version": 1,
              "crypto": {
                "curves": {
                  "allowed_curve_bitmap": [26],
                  "allowed_curve_ids": [1, 3, 4],
                  "registry_version": 1
                },
                "sm": {
                  "acceleration": {
                    "neon_sm3": false,
                    "neon_sm4": false,
                    "policy": "scalar-only",
                    "scalar": true
                  },
                  "allowed_signing": ["ed25519", "secp256k1", "bls_normal"],
                  "default_hash": "blake2b-256",
                  "enabled": false,
                  "openssl_preview": false,
                  "sm2_distid_default": "1234567812345678"
                }
              }
            }
            """;

        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent(responseBody),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var capabilities = await client.GetNodeCapabilitiesAsync();

        Assert.Equal(1, capabilities.AbiVersion);
        Assert.Equal(1, capabilities.DataModelVersion);
        Assert.Equal(3, capabilities.Crypto.Curves.AllowedCurveIds.Count);
        Assert.Contains("ed25519", capabilities.Crypto.Sm.AllowedSigning);
        Assert.Equal("/v1/node/capabilities", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task GetAccountsAsyncAddsPaginationAndDeserializesPage()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "items": [
                    { "id": "sorauロ1Ntest" }
                  ],
                  "total": 7
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var page = await client.GetAccountsAsync(limit: 5, offset: 2);

        Assert.Single(page.Items);
        Assert.Equal("sorauロ1Ntest", page.Items[0].Id);
        Assert.Equal(7, page.Total);
        Assert.Equal("/v1/accounts?limit=5&offset=2", handler.LastRequest!.RequestUri!.PathAndQuery);
    }

    [Fact]
    public async Task GetExplorerAccountQrAsyncEncodesAccountIdAndDeserializesSvgSnapshot()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "canonical_id": "sorauロ1Nholder",
                  "literal": "sorauロ1Nholder",
                  "network_prefix": 753,
                  "error_correction": "M",
                  "modules": 192,
                  "qr_version": 5,
                  "svg": "<svg/>"
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var snapshot = await client.GetExplorerAccountQrAsync("  sorauロ1Nholder  ");

        Assert.Equal("sorauロ1Nholder", snapshot.CanonicalId);
        Assert.Equal(753, snapshot.NetworkPrefix);
        Assert.Equal("<svg/>", snapshot.Svg);
        Assert.Equal("/v1/explorer/accounts/sorau%E3%83%AD1Nholder/qr", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task GetExplorerAccountsAsyncAddsFiltersAndDeserializesPage()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "pagination": { "page": 2, "per_page": 5, "total_pages": 3, "total_items": 11 },
                  "items": [
                    {
                      "id": "sorauロ1Nholder",
                      "i105_address": "i105:sorauロ1Nholder",
                      "network_prefix": 753,
                      "metadata": { "tier": "gold" },
                      "owned_domains": 1,
                      "owned_assets": 2,
                      "owned_nfts": 3
                    }
                  ]
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var page = await client.GetExplorerAccountsAsync(new ToriiExplorerAccountsQuery
        {
            Page = 2,
            PerPage = 5,
            Domain = " wonderland.paynet ",
            WithAsset = " rose#wonderland.paynet ",
        });

        Assert.Equal((ulong)11, page.Pagination.TotalItems);
        Assert.Single(page.Items);
        Assert.Equal("i105:sorauロ1Nholder", page.Items[0].I105Address);
        Assert.Equal("gold", page.Items[0].Metadata!["tier"]!.GetValue<string>());
        Assert.Equal("/v1/explorer/accounts?page=2&per_page=5&domain=wonderland.paynet&with_asset=rose%23wonderland.paynet", handler.LastRequest!.RequestUri!.PathAndQuery);
    }

    [Fact]
    public async Task GetExplorerAccountAsyncEncodesIdentifierAndDeserializesDetail()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "id": "sorauロ1Nholder",
                  "i105_address": "i105:sorauロ1Nholder",
                  "network_prefix": 753,
                  "metadata": { "tier": "gold" },
                  "owned_domains": 1,
                  "owned_assets": 2,
                  "owned_nfts": 3
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var account = await client.GetExplorerAccountAsync(" member@universal ");

        Assert.Equal("sorauロ1Nholder", account.Id);
        Assert.Equal((uint)2, account.OwnedAssets);
        Assert.Equal("gold", account.Metadata!["tier"]!.GetValue<string>());
        Assert.Equal("/v1/explorer/accounts/member%40universal", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task GetExplorerDomainsAsyncAddsFiltersAndDeserializesPage()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "pagination": { "page": 1, "per_page": 10, "total_pages": 1, "total_items": 1 },
                  "items": [
                    {
                      "id": "wonderland.paynet",
                      "logo": "https://cdn.example/logo.svg",
                      "metadata": { "region": "jp" },
                      "owned_by": "sorauロ1Nowner",
                      "accounts": 4,
                      "assets": 7,
                      "nfts": 2
                    }
                  ]
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var page = await client.GetExplorerDomainsAsync(new ToriiExplorerDomainsQuery
        {
            Page = 1,
            PerPage = 10,
            OwnedBy = " owner@universal ",
        });

        Assert.Single(page.Items);
        Assert.Equal("wonderland.paynet", page.Items[0].Id);
        Assert.Equal("jp", page.Items[0].Metadata!["region"]!.GetValue<string>());
        Assert.Equal("/v1/explorer/domains?page=1&per_page=10&owned_by=owner%40universal", handler.LastRequest!.RequestUri!.PathAndQuery);
    }

    [Fact]
    public async Task GetExplorerDomainAsyncEncodesIdentifierAndDeserializesDetail()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "id": "wonderland.paynet",
                  "logo": null,
                  "metadata": { "region": "jp" },
                  "owned_by": "sorauロ1Nowner",
                  "accounts": 4,
                  "assets": 7,
                  "nfts": 2
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var domain = await client.GetExplorerDomainAsync(" wonderland.paynet ");

        Assert.Equal("sorauロ1Nowner", domain.OwnedBy);
        Assert.Equal((uint)4, domain.Accounts);
        Assert.Equal("/v1/explorer/domains/wonderland.paynet", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task GetExplorerAssetDefinitionsAsyncAddsFiltersAndDeserializesPage()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "pagination": { "page": 3, "per_page": 2, "total_pages": 4, "total_items": 8 },
                  "items": [
                    {
                      "id": "rose#wonderland.paynet",
                      "mintable": "Infinitely",
                      "logo": "https://cdn.example/rose.svg",
                      "metadata": { "category": "flora" },
                      "owned_by": "sorauロ1Nissuer",
                      "assets": 12,
                      "total_quantity": "1000",
                      "locked_quantity": "25",
                      "circulating_quantity": "975"
                    }
                  ]
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var page = await client.GetExplorerAssetDefinitionsAsync(new ToriiExplorerAssetDefinitionsQuery
        {
            Page = 3,
            PerPage = 2,
            Domain = " wonderland.paynet ",
            OwnedBy = " issuer@universal ",
        });

        Assert.Equal((ulong)8, page.Pagination.TotalItems);
        Assert.Single(page.Items);
        Assert.Equal("975", page.Items[0].CirculatingQuantity);
        Assert.Equal("flora", page.Items[0].Metadata!["category"]!.GetValue<string>());
        Assert.Equal("/v1/explorer/asset-definitions?page=3&per_page=2&domain=wonderland.paynet&owned_by=issuer%40universal", handler.LastRequest!.RequestUri!.PathAndQuery);
    }

    [Fact]
    public async Task GetExplorerAssetDefinitionAsyncEncodesIdentifierAndDeserializesDetail()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "id": "rose#wonderland.paynet",
                  "mintable": "Infinitely",
                  "logo": null,
                  "metadata": { "category": "flora" },
                  "owned_by": "sorauロ1Nissuer",
                  "assets": 12,
                  "total_quantity": "1000",
                  "locked_quantity": "25",
                  "circulating_quantity": "975"
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var definition = await client.GetExplorerAssetDefinitionAsync(" rose#wonderland.paynet ");

        Assert.Equal("Infinitely", definition.Mintable);
        Assert.Equal("975", definition.CirculatingQuantity);
        Assert.Equal("/v1/explorer/asset-definitions/rose%23wonderland.paynet", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task GetExplorerAssetDefinitionEconometricsAsyncEncodesIdentifierAndDeserializesSnapshot()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "definition_id": "rose#wonderland.paynet",
                  "computed_at_ms": 123456,
                  "velocity_windows": [
                    {
                      "key": "1h",
                      "start_ms": 120000,
                      "end_ms": 123456,
                      "transfers": 2,
                      "unique_senders": 1,
                      "unique_receivers": 2,
                      "amount": "10"
                    }
                  ],
                  "issuance_windows": [
                    {
                      "key": "24h",
                      "start_ms": 100000,
                      "end_ms": 123456,
                      "mint_count": 1,
                      "burn_count": 0,
                      "minted": "100",
                      "burned": "0",
                      "net": "100"
                    }
                  ],
                  "issuance_series": [
                    {
                      "bucket_start_ms": 86400,
                      "minted": "100",
                      "burned": "0",
                      "net": "100"
                    }
                  ]
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var snapshot = await client.GetExplorerAssetDefinitionEconometricsAsync(" rose#wonderland.paynet ");

        Assert.Equal((ulong)123456, snapshot.ComputedAtMilliseconds);
        Assert.Single(snapshot.VelocityWindows);
        Assert.Equal((ulong)2, snapshot.VelocityWindows[0].Transfers);
        Assert.Equal("100", snapshot.IssuanceWindows[0].Net);
        Assert.Equal("/v1/explorer/asset-definitions/rose%23wonderland.paynet/econometrics", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task GetExplorerAssetDefinitionSnapshotAsyncEncodesIdentifierAndDeserializesSnapshot()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "definition_id": "rose#wonderland.paynet",
                  "computed_at_ms": 123456,
                  "holders_total": 2,
                  "total_supply": "1000",
                  "top_holders": [
                    { "account_id": "sorauロ1Nholder", "balance": "700" }
                  ],
                  "distribution": {
                    "gini": 0.3,
                    "hhi": 0.58,
                    "theil": 0.1,
                    "entropy": 0.6,
                    "entropy_normalized": 0.8,
                    "nakamoto_33": 1,
                    "nakamoto_51": 1,
                    "nakamoto_67": 2,
                    "top1": 0.7,
                    "top5": 1.0,
                    "top10": 1.0,
                    "median": "300",
                    "p90": "700",
                    "p99": "700",
                    "lorenz": [
                      { "population": 0.0, "share": 0.0 },
                      { "population": 1.0, "share": 1.0 }
                    ]
                  }
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var snapshot = await client.GetExplorerAssetDefinitionSnapshotAsync(" rose#wonderland.paynet ");

        Assert.Equal((ulong)2, snapshot.HoldersTotal);
        Assert.Equal("700", snapshot.TopHolders[0].Balance);
        Assert.Equal(0.7, snapshot.Distribution.Top1);
        Assert.Equal(2, snapshot.Distribution.Lorenz.Count);
        Assert.Equal("/v1/explorer/asset-definitions/rose%23wonderland.paynet/snapshot", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task GetExplorerAssetsAsyncAddsFiltersAndDeserializesPage()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "pagination": { "page": 4, "per_page": 1, "total_pages": 9, "total_items": 9 },
                  "items": [
                    {
                      "id": "asset-001",
                      "definition_id": "rose#wonderland.paynet",
                      "account_id": "sorauロ1Nholder",
                      "value": "25"
                    }
                  ]
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var page = await client.GetExplorerAssetsAsync(new ToriiExplorerAssetsQuery
        {
            Page = 4,
            PerPage = 1,
            OwnedBy = " holder@universal ",
            Definition = " rose#wonderland.paynet ",
            AssetId = " asset-001 ",
        });

        Assert.Equal((ulong)9, page.Pagination.TotalItems);
        Assert.Single(page.Items);
        Assert.Equal("25", page.Items[0].Value);
        Assert.Equal("/v1/explorer/assets?page=4&per_page=1&owned_by=holder%40universal&definition=rose%23wonderland.paynet&asset_id=asset-001", handler.LastRequest!.RequestUri!.PathAndQuery);
    }

    [Fact]
    public async Task GetExplorerAssetAsyncEncodesIdentifierAndDeserializesDetail()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "id": "asset-001",
                  "definition_id": "rose#wonderland.paynet",
                  "account_id": "sorauロ1Nholder",
                  "value": "25"
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var asset = await client.GetExplorerAssetAsync(" asset-001 ");

        Assert.Equal("rose#wonderland.paynet", asset.DefinitionId);
        Assert.Equal("25", asset.Value);
        Assert.Equal("/v1/explorer/assets/asset-001", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task GetExplorerNftsAsyncAddsFiltersAndDeserializesPage()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "pagination": { "page": 2, "per_page": 2, "total_pages": 2, "total_items": 3 },
                  "items": [
                    {
                      "id": "ticket$wonderland",
                      "owned_by": "sorauロ1Nholder",
                      "metadata": { "seat": "A1" }
                    }
                  ]
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var page = await client.GetExplorerNftsAsync(new ToriiExplorerNftsQuery
        {
            Page = 2,
            PerPage = 2,
            OwnedBy = " holder@universal ",
            Domain = " wonderland.paynet ",
        });

        Assert.Single(page.Items);
        Assert.Equal("A1", page.Items[0].Metadata!["seat"]!.GetValue<string>());
        Assert.Equal("/v1/explorer/nfts?page=2&per_page=2&owned_by=holder%40universal&domain=wonderland.paynet", handler.LastRequest!.RequestUri!.PathAndQuery);
    }

    [Fact]
    public async Task GetExplorerNftAsyncEncodesIdentifierAndDeserializesDetail()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "id": "ticket$wonderland",
                  "owned_by": "sorauロ1Nholder",
                  "metadata": { "seat": "A1" }
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var nft = await client.GetExplorerNftAsync(" ticket$wonderland ");

        Assert.Equal("sorauロ1Nholder", nft.OwnedBy);
        Assert.Equal("A1", nft.Metadata!["seat"]!.GetValue<string>());
        Assert.Equal("/v1/explorer/nfts/ticket%24wonderland", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task GetExplorerRwasAsyncAddsFiltersAndDeserializesPage()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "pagination": { "page": 1, "per_page": 5, "total_pages": 1, "total_items": 1 },
                  "items": [
                    {
                      "id": "lot$gold#wonderland",
                      "owned_by": "sorauﾛ1Ncustodian",
                      "quantity": "100",
                      "held_quantity": "5",
                      "primary_reference": "vault-7",
                      "status": "Verified",
                      "is_frozen": false,
                      "metadata": { "grade": "A" },
                      "parents": [
                        { "rwa": "ore$mine#wonderland", "quantity": "10" }
                      ]
                    }
                  ]
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var page = await client.GetExplorerRwasAsync(new ToriiExplorerRwasQuery
        {
            Page = 1,
            PerPage = 5,
            OwnedBy = " custodian@universal ",
            Domain = " wonderland.paynet ",
        });

        Assert.Single(page.Items);
        Assert.Equal("vault-7", page.Items[0].PrimaryReference);
        Assert.Equal("ore$mine#wonderland", page.Items[0].Parents[0].Rwa);
        Assert.Equal("/v1/explorer/rwas?page=1&per_page=5&owned_by=custodian%40universal&domain=wonderland.paynet", handler.LastRequest!.RequestUri!.PathAndQuery);
    }

    [Fact]
    public async Task GetExplorerRwaAsyncEncodesIdentifierAndDeserializesDetail()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "id": "lot$gold#wonderland",
                  "owned_by": "sorauﾛ1Ncustodian",
                  "quantity": "100",
                  "held_quantity": "5",
                  "primary_reference": "vault-7",
                  "status": "Verified",
                  "is_frozen": false,
                  "metadata": { "grade": "A" },
                  "parents": [
                    { "rwa": "ore$mine#wonderland", "quantity": "10" }
                  ]
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var rwa = await client.GetExplorerRwaAsync(" lot$gold#wonderland ");

        Assert.Equal("100", rwa.Quantity);
        Assert.Equal("A", rwa.Metadata!["grade"]!.GetValue<string>());
        Assert.Equal("/v1/explorer/rwas/lot%24gold%23wonderland", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task GetExplorerBlocksAsyncAddsPaginationAndDeserializesPage()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "pagination": { "page": 2, "per_page": 5, "total_pages": 3, "total_items": 12 },
                  "items": [
                    {
                      "hash": "block-1",
                      "height": 42,
                      "created_at": "2026-03-29T00:00:00Z",
                      "prev_block_hash": "block-0",
                      "transactions_hash": "root-1",
                      "transactions_rejected": 1,
                      "transactions_total": 4
                    }
                  ]
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var page = await client.GetExplorerBlocksAsync(new ToriiExplorerPaginationQuery { Page = 2, PerPage = 5 });

        Assert.Equal((ulong)2, page.Pagination.Page);
        Assert.Equal((ulong)5, page.Pagination.PerPage);
        Assert.Equal((ulong)3, page.Pagination.TotalPages);
        Assert.Equal((ulong)12, page.Pagination.TotalItems);
        Assert.Single(page.Items);
        Assert.Equal("block-1", page.Items[0].Hash);
        Assert.Equal((ulong)42, page.Items[0].Height);
        Assert.Equal("/v1/explorer/blocks?page=2&per_page=5", handler.LastRequest!.RequestUri!.PathAndQuery);
    }

    [Fact]
    public async Task GetExplorerBlockAsyncEncodesIdentifierAndDeserializesBlock()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "hash": "block-hash",
                  "height": 7,
                  "created_at": "2026-03-29T02:00:00Z",
                  "prev_block_hash": null,
                  "transactions_hash": "tx-root",
                  "transactions_rejected": 0,
                  "transactions_total": 2
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var block = await client.GetExplorerBlockAsync(" 0007 ");

        Assert.Equal("block-hash", block.Hash);
        Assert.Equal((ulong)7, block.Height);
        Assert.Equal("tx-root", block.TransactionsHash);
        Assert.Equal("/v1/explorer/blocks/0007", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task GetAccountAssetsAsyncEncodesFiltersAndDeserializesBalances()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "items": [
                    {
                      "asset": "rose#wonderland.paynet",
                      "account_id": "sorauロ1Nholder",
                      "scope": "global",
                      "asset_name": "rose",
                      "asset_alias": null,
                      "quantity": "10"
                    }
                  ],
                  "total": 1
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var balances = await client.GetAccountAssetsAsync("sorauロ1Nholder", limit: 10, offset: 1, asset: "rose#wonderland.paynet", scope: "global");

        Assert.Single(balances.Items);
        Assert.Equal("rose#wonderland.paynet", balances.Items[0].Asset);
        Assert.Equal("10", balances.Items[0].Quantity);
        Assert.Contains("/v1/accounts/", handler.LastRequest!.RequestUri!.AbsoluteUri);
        Assert.Contains("/assets", handler.LastRequest.RequestUri.AbsoluteUri);
        Assert.Equal("limit=10&offset=1&asset=rose%23wonderland.paynet&scope=global", handler.LastRequest.RequestUri.Query.TrimStart('?'));
    }

    [Fact]
    public async Task GetAccountTransactionsAsyncUsesServerAssetIdQueryParameter()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "items": [
                    {
                      "authority": "sorauロ1Nholder",
                      "timestamp_ms": 1,
                      "entrypoint_hash": "hash",
                      "result_ok": true
                    }
                  ],
                  "total": 1
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var transactions = await client.GetAccountTransactionsAsync("sorauロ1Nholder", limit: 50, offset: 3, assetId: "rose#wonderland.paynet");

        Assert.Single(transactions.Items);
        Assert.Equal("hash", transactions.Items[0].EntrypointHash);
        Assert.True(transactions.Items[0].ResultOk);
        Assert.Contains("/v1/accounts/", handler.LastRequest!.RequestUri!.AbsoluteUri);
        Assert.Contains("/transactions", handler.LastRequest.RequestUri.AbsoluteUri);
        Assert.Equal("limit=50&offset=3&asset_id=rose%23wonderland.paynet", handler.LastRequest.RequestUri.Query.TrimStart('?'));
    }

    [Fact]
    public async Task GetExplorerTransactionsAsyncAddsFiltersAndDeserializesPage()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "pagination": { "page": 1, "per_page": 20, "total_pages": 2, "total_items": 21 },
                  "items": [
                    {
                      "authority": "sorauロ1Nholder",
                      "hash": "tx-1",
                      "block": 5,
                      "created_at": "2026-03-29T03:00:00Z",
                      "executable": "Instructions",
                      "status": "Committed"
                    }
                  ]
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var page = await client.GetExplorerTransactionsAsync(new ToriiExplorerTransactionsQuery
        {
            Page = 1,
            PerPage = 20,
            Authority = " sorauロ1Nholder ",
            Block = 5,
            Status = ToriiExplorerTransactionStatusFilter.Committed,
            AssetId = " rose#wonderland.paynet ",
        });

        Assert.Equal((ulong)21, page.Pagination.TotalItems);
        Assert.Single(page.Items);
        Assert.Equal("tx-1", page.Items[0].Hash);
        Assert.Equal("Committed", page.Items[0].Status);
        Assert.Equal(
            "page=1&per_page=20&authority=sorau%E3%83%AD1Nholder&block=5&status=committed&asset_id=rose%23wonderland.paynet",
            handler.LastRequest!.RequestUri!.Query.TrimStart('?'));
    }

    [Fact]
    public async Task GetExplorerLatestTransactionsAsyncUsesLatestPathAndDeserializesResponse()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "sampled_at": "2026-03-29T07:00:00Z",
                  "items": [
                    {
                      "authority": "sorauロ1Nholder",
                      "hash": "tx-latest",
                      "block": 9,
                      "created_at": "2026-03-29T06:59:00Z",
                      "executable": "Instructions",
                      "status": "Committed"
                    }
                  ]
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var response = await client.GetExplorerLatestTransactionsAsync(new ToriiExplorerTransactionsQuery
        {
            PerPage = 3,
            Status = ToriiExplorerTransactionStatusFilter.Committed,
        });

        Assert.Equal("2026-03-29T07:00:00Z", response.SampledAt);
        Assert.Single(response.Items);
        Assert.Equal("tx-latest", response.Items[0].Hash);
        Assert.Equal("/v1/explorer/transactions/latest?per_page=3&status=committed", handler.LastRequest!.RequestUri!.PathAndQuery);
    }

    [Fact]
    public async Task GetExplorerTransactionAsyncEncodesHashAndDeserializesDetail()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "authority": "sorauロ1Nholder",
                  "hash": "tx-detail",
                  "block": 8,
                  "created_at": "2026-03-29T04:00:00Z",
                  "executable": "Instructions",
                  "status": "Rejected",
                  "rejection_reason": {
                    "encoded": "0x01",
                    "json": { "kind": "ValidationFail" },
                    "message": "validation failed"
                  },
                  "metadata": { "trace": "abc" },
                  "nonce": 9,
                  "signature": "cafebabe",
                  "time_to_live": { "ms": 5000 }
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var transaction = await client.GetExplorerTransactionAsync(" tx-detail ");

        Assert.Equal("tx-detail", transaction.Hash);
        Assert.Equal((ulong)8, transaction.Block);
        Assert.Equal("Rejected", transaction.Status);
        Assert.NotNull(transaction.RejectionReason);
        Assert.Equal("validation failed", transaction.RejectionReason!.Message);
        Assert.Equal("ValidationFail", transaction.RejectionReason.Json!["kind"]!.GetValue<string>());
        Assert.Equal("abc", transaction.Metadata!["trace"]!.GetValue<string>());
        Assert.Equal((ulong)9, transaction.Nonce);
        Assert.Equal((ulong)5000, transaction.TimeToLive!.Milliseconds);
        Assert.Equal("/v1/explorer/transactions/tx-detail", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task GetAccountPermissionsAsyncDeserializesPayloadPage()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "items": [
                    {
                      "name": "CanResolveAccountAlias",
                      "payload": { "dataspace": 7 }
                    }
                  ],
                  "total": 1
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var permissions = await client.GetAccountPermissionsAsync("sorauロ1Nholder", limit: 5);

        Assert.Single(permissions.Items);
        Assert.Equal("CanResolveAccountAlias", permissions.Items[0].Name);
        Assert.NotNull(permissions.Items[0].Payload);
        Assert.Equal(7, permissions.Items[0].Payload!["dataspace"]!.GetValue<int>());
        Assert.Equal("/v1/accounts/sorau%E3%83%AD1Nholder/permissions?limit=5", handler.LastRequest!.RequestUri!.PathAndQuery);
    }

    [Fact]
    public async Task GetExplorerInstructionsAsyncAddsFiltersAndDeserializesPage()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "pagination": { "page": 3, "per_page": 10, "total_pages": 5, "total_items": 48 },
                  "items": [
                    {
                      "authority": "sorauﾛ1Nauthority",
                      "created_at": "2026-03-29T05:00:00Z",
                      "kind": "Transfer",
                      "box": {
                        "encoded": "0x11",
                        "json": {
                          "kind": "Transfer",
                          "payload": { "object": "asset" },
                          "wire_id": "iroha.transfer",
                          "encoded": "11"
                        }
                      },
                      "transaction_hash": "tx-ins",
                      "transaction_status": "Committed",
                      "block": 12,
                      "index": 1
                    }
                  ]
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var page = await client.GetExplorerInstructionsAsync(new ToriiExplorerInstructionsQuery
        {
            Page = 3,
            PerPage = 10,
            Authority = " sorauﾛ1Nauthority ",
            Account = " sorauロ1Naccount ",
            TransactionHash = " tx-ins ",
            TransactionStatus = ToriiExplorerTransactionStatusFilter.Committed,
            Block = 12,
            Kind = " transfer ",
            AssetId = " rose#wonderland.paynet ",
        });

        Assert.Equal((ulong)48, page.Pagination.TotalItems);
        Assert.Single(page.Items);
        Assert.Equal("Transfer", page.Items[0].Kind);
        Assert.Equal("tx-ins", page.Items[0].TransactionHash);
        Assert.Equal(
            "page=3&per_page=10&authority=sorau%EF%BE%9B1Nauthority&account=sorau%E3%83%AD1Naccount&transaction_hash=tx-ins&transaction_status=committed&block=12&kind=transfer&asset_id=rose%23wonderland.paynet",
            handler.LastRequest!.RequestUri!.Query.TrimStart('?'));
    }

    [Fact]
    public async Task GetExplorerLatestInstructionsAsyncUsesLatestPathAndDeserializesResponse()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "sampled_at": "2026-03-29T08:00:00Z",
                  "items": [
                    {
                      "authority": "sorauﾛ1Nauthority",
                      "created_at": "2026-03-29T07:59:00Z",
                      "kind": "Transfer",
                      "box": {
                        "encoded": "0x33",
                        "json": {
                          "kind": "Transfer",
                          "payload": { "object": "asset" },
                          "wire_id": "iroha.transfer",
                          "encoded": "33"
                        }
                      },
                      "transaction_hash": "tx-latest-ins",
                      "transaction_status": "Committed",
                      "block": 14,
                      "index": 0
                    }
                  ]
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var response = await client.GetExplorerLatestInstructionsAsync(new ToriiExplorerInstructionsQuery
        {
            PerPage = 4,
            TransactionStatus = ToriiExplorerTransactionStatusFilter.Committed,
        });

        Assert.Equal("2026-03-29T08:00:00Z", response.SampledAt);
        Assert.Single(response.Items);
        Assert.Equal("tx-latest-ins", response.Items[0].TransactionHash);
        Assert.Equal("/v1/explorer/instructions/latest?per_page=4&transaction_status=committed", handler.LastRequest!.RequestUri!.PathAndQuery);
    }

    [Fact]
    public async Task GetExplorerInstructionAsyncEncodesHashAndIndexAndDeserializesDetail()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "authority": "sorauﾛ1Nauthority",
                  "created_at": "2026-03-29T06:00:00Z",
                  "kind": "SetKeyValue",
                  "box": {
                    "encoded": "0x22",
                    "json": {
                      "kind": "SetKeyValue",
                      "payload": { "object": "domain", "key": "flag" },
                      "wire_id": "iroha.set_key_value",
                      "encoded": "22"
                    }
                  },
                  "transaction_hash": "tx-detail",
                  "transaction_status": "Rejected",
                  "block": 13,
                  "index": 2
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var instruction = await client.GetExplorerInstructionAsync(" tx-detail ", 2);

        Assert.Equal("SetKeyValue", instruction.Kind);
        Assert.Equal("tx-detail", instruction.TransactionHash);
        Assert.Equal("Rejected", instruction.TransactionStatus);
        Assert.Equal("iroha.set_key_value", instruction.InstructionBox.Json!.WireId);
        Assert.Equal("domain", instruction.InstructionBox.Json.Payload!["object"]!.GetValue<string>());
        Assert.Equal((uint)2, instruction.Index);
        Assert.Equal("/v1/explorer/instructions/tx-detail/2", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task GetExplorerInstructionContractViewAsyncEncodesHashAndIndexAndDeserializesView()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "code_hash": "ab12",
                  "declared_code_hash": "cd34",
                  "abi_hash": "ef56",
                  "compiler_fingerprint": "kotodama-1",
                  "byte_len": 64,
                  "permissions": ["CanTransferUserAssets"],
                  "access_hints": {
                    "read_keys": ["balances"],
                    "write_keys": ["ledger"]
                  },
                  "entrypoints": [
                    {
                      "name": "main",
                      "kind": "Execute",
                      "params": [
                        { "name": "amount", "type_name": "u64" }
                      ],
                      "return_type": "bool",
                      "permission": "CanTransferUserAssets",
                      "read_keys": ["balances"],
                      "write_keys": ["ledger"],
                      "access_hints_complete": true,
                      "access_hints_skipped": [],
                      "triggers": ["after_transfer"]
                    }
                  ],
                  "analysis": {
                    "instruction_count": 42,
                    "memory": {
                      "load64": 1,
                      "store64": 2,
                      "load128": 3,
                      "store128": 4
                    },
                    "syscalls": [
                      { "number": 7, "name": "emit", "count": 3 }
                    ]
                  },
                  "warnings": ["historical bytes"],
                  "rendered_source_kind": "pseudo_source",
                  "rendered_source_text": "public fn main() {}",
                  "verified_source_ref": {
                    "language": "kotodama",
                    "source_name": "demo.ko",
                    "submitted_at": "2026-03-29T12:00:00Z",
                    "manifest_id_hex": "aa55",
                    "payload_digest_hex": "bb66",
                    "content_length": 128
                  }
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var view = await client.GetExplorerInstructionContractViewAsync(" tx-detail ", 2);

        Assert.Equal("ab12", view.CodeHash);
        Assert.Equal("cd34", view.DeclaredCodeHash);
        Assert.Equal("ledger", view.AccessHints!.WriteKeys[0]);
        Assert.Equal("amount", view.Entrypoints[0].Parameters[0].Name);
        Assert.Equal((ulong)42, view.Analysis!.InstructionCount);
        Assert.Equal("kotodama", view.VerifiedSourceReference!.Language);
        Assert.Equal("/v1/explorer/instructions/tx-detail/2/contract-view", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task GetExplorerHealthAsyncDeserializesSnapshot()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "head_height": 55,
                  "head_created_at": "2026-03-29T09:00:00Z",
                  "sampled_at": "2026-03-29T09:00:05Z"
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var snapshot = await client.GetExplorerHealthAsync();

        Assert.Equal((ulong)55, snapshot.HeadHeight);
        Assert.Equal("2026-03-29T09:00:00Z", snapshot.HeadCreatedAt);
        Assert.Equal("2026-03-29T09:00:05Z", snapshot.SampledAt);
        Assert.Equal("/v1/explorer/health", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task GetExplorerMetricsAsyncDeserializesSnapshot()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "peers": 4,
                  "domains": 10,
                  "accounts": 20,
                  "assets": 30,
                  "transactions_accepted": 40,
                  "transactions_rejected": 2,
                  "block": 99,
                  "block_created_at": "2026-03-29T10:00:00Z",
                  "finalized_block": 99,
                  "avg_commit_time": { "ms": 850 },
                  "avg_block_time": { "ms": 1200 }
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var snapshot = await client.GetExplorerMetricsAsync();

        Assert.Equal((ulong)4, snapshot.Peers);
        Assert.Equal((ulong)30, snapshot.Assets);
        Assert.Equal((ulong)99, snapshot.FinalizedBlock);
        Assert.Equal((ulong)850, snapshot.AverageCommitTime!.Milliseconds);
        Assert.Equal((ulong)1200, snapshot.AverageBlockTime!.Milliseconds);
        Assert.Equal("/v1/explorer/metrics", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task LookupAliasesByAccountAsyncPostsOptionalScopeFilters()
    {
        using var handler = new RecordingHandler(request =>
        {
            var payload = ReadBodyAsJson(request);
            Assert.Equal("/v1/aliases/by_account", request.RequestUri!.AbsolutePath);
            Assert.Equal("sorauロ1Nholder", payload.RootElement.GetProperty("account_id").GetString());
            Assert.Equal("universal", payload.RootElement.GetProperty("dataspace").GetString());
            Assert.Equal("paynet", payload.RootElement.GetProperty("domain").GetString());

            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("""
                    {
                      "account_id": "sorauロ1Nholder",
                      "total": 2,
                      "source": "on_chain",
                      "items": [
                        {
                          "alias": "merchant@paynet.universal",
                          "dataspace": "universal",
                          "domain": "paynet",
                          "is_primary": true
                        },
                        {
                          "alias": "merchant@universal",
                          "dataspace": "universal",
                          "domain": null,
                          "is_primary": false
                        }
                      ]
                    }
                    """),
            };
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var aliases = await client.LookupAliasesByAccountAsync("sorauロ1Nholder", dataspace: "universal", domain: "paynet");

        Assert.NotNull(aliases);
        Assert.Equal("sorauロ1Nholder", aliases!.AccountId);
        Assert.Equal(2, aliases.Total);
        Assert.Equal("merchant@paynet.universal", aliases.Items[0].Alias);
        Assert.True(aliases.Items[0].IsPrimary);
        Assert.Equal("on_chain", aliases.Source);
    }

    [Fact]
    public async Task LookupAliasesByAccountAsyncReturnsNullOnNotFound()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.NotFound));

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var aliases = await client.LookupAliasesByAccountAsync("sorauロ1Nmissing");

        Assert.Null(aliases);
    }

    [Fact]
    public async Task GetVpnProfileAsyncDeserializesSnapshot()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "available": true,
                  "relay_endpoint": "/dns4/vpn.sora.org/tcp/443/wss",
                  "supported_exit_classes": ["standard", "low-latency", "high-security"],
                  "default_exit_class": "standard",
                  "lease_secs": 3600,
                  "dns_push_interval_secs": 30,
                  "meter_family": "vpn-standard",
                  "route_pushes": ["10.0.0.0/8"],
                  "excluded_routes": ["127.0.0.0/8"],
                  "dns_servers": ["1.1.1.1", "8.8.8.8"],
                  "tunnel_addresses": ["10.208.0.2/32"],
                  "mtu_bytes": 1280,
                  "display_billing_label": "standard · vpn-standard · 1000000 nano-XOR",
                  "fee_asset_id": "xor#universal.universal",
                  "escrow_account_id": "vpn_escrow",
                  "operator_account_id": "sorauロ1Noperator",
                  "lease_fee_nanos": 1000000,
                  "settlement_grace_secs": 120,
                  "flow_label_bits": 20,
                  "padding_budget_ms": 80,
                  "relay_tls_spki_sha256_hex": "1111111111111111111111111111111111111111111111111111111111111111"
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var profile = await client.GetVpnProfileAsync();

        Assert.True(profile.Available);
        Assert.Equal("standard", profile.DefaultExitClass);
        Assert.Equal((ulong)3600, profile.LeaseSeconds);
        Assert.Equal(3, profile.SupportedExitClasses.Count);
        Assert.Equal("xor#universal.universal", profile.FeeAssetId);
        Assert.Equal((ulong)1000000, profile.LeaseFeeNanos);
        Assert.Equal((ushort)80, profile.PaddingBudgetMilliseconds);
        Assert.Equal("/v1/vpn/profile", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task CreateVpnQuoteAsyncPostsMeteringKeyAndDeserializesNativeInstruction()
    {
        const string quoteId = "1212121212121212121212121212121212121212121212121212121212121212";
        const string meteringPublicKeyHex = "3434343434343434343434343434343434343434343434343434343434343434";

        using var handler = new RecordingHandler(request =>
        {
            var payload = ReadBodyAsJson(request);
            Assert.Equal("/v1/vpn/quotes", request.RequestUri!.AbsolutePath);
            Assert.Equal("low-latency", payload.RootElement.GetProperty("exit_class").GetString());
            Assert.Equal(meteringPublicKeyHex, payload.RootElement.GetProperty("metering_public_key_hex").GetString());

            return new HttpResponseMessage(HttpStatusCode.Created)
            {
                Content = new StringContent($$"""
                    {
                      "quote_id": "{{quoteId}}",
                      "lease_id_hex": "{{quoteId}}",
                      "session_id_hex": "56565656565656565656565656565656",
                      "payment_reference": "{{quoteId}}",
                      "account_id": "sorauロ1Nholder",
                      "exit_class": "low-latency",
                      "relay_endpoint": "/dns4/vpn.sora.org/tcp/443/wss",
                      "lease_secs": 3600,
                      "quote_expires_at_ms": 1700000000000,
                      "fee_asset_id": "xor#universal.universal",
                      "escrow_account_id": "vpn_escrow",
                      "operator_account_id": "sorauロ1Noperator",
                      "lease_fee_nanos": 1000000,
                      "route_pushes": ["10.0.0.0/8"],
                      "excluded_routes": ["127.0.0.0/8"],
                      "dns_servers": ["1.1.1.1"],
                      "tunnel_addresses": ["10.208.0.2/32"],
                      "mtu_bytes": 1280,
                      "meter_family": "vpn-standard",
                      "flow_label_bits": 20,
                      "padding_budget_ms": 80,
                      "relay_tls_spki_sha256_hex": "7878787878787878787878787878787878787878787878787878787878787878",
                      "metering_public_key_hex": "{{meteringPublicKeyHex}}",
                      "open_lease_instruction": {
                        "wire_id": "OpenVpnLeaseEscrow",
                        "payload_hex": "abcd"
                      },
                      "tx_instructions": [
                        {
                          "wire_id": "OpenVpnLeaseEscrow",
                          "payload_hex": "abcd"
                        }
                      ]
                    }
                    """),
            };
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var quote = await client.CreateVpnQuoteAsync(new ToriiVpnQuoteCreateRequest
        {
            ExitClass = "low-latency",
            MeteringPublicKeyHex = meteringPublicKeyHex,
        });

        Assert.Equal(quoteId, quote.QuoteId);
        Assert.Equal("OpenVpnLeaseEscrow", quote.OpenLeaseInstruction?.WireId);
        Assert.Single(quote.TxInstructions);
        Assert.Equal("xor#universal.universal", quote.FeeAssetId);
    }

    [Fact]
    public async Task CreateVpnSessionAsyncPostsQuotePaymentAndDeserializesSession()
    {
        const string quoteId = "abababababababababababababababababababababababababababababababab";
        const string paymentTxHash = "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd";
        const string meteringPublicKeyHex = "efefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefef";

        using var handler = new RecordingHandler(request =>
        {
            var payload = ReadBodyAsJson(request);
            Assert.Equal("/v1/vpn/sessions", request.RequestUri!.AbsolutePath);
            Assert.Equal("low-latency", payload.RootElement.GetProperty("exit_class").GetString());
            Assert.Equal(quoteId, payload.RootElement.GetProperty("quote_id").GetString());
            Assert.Equal(paymentTxHash, payload.RootElement.GetProperty("payment_tx_hash").GetString());
            Assert.Equal(meteringPublicKeyHex, payload.RootElement.GetProperty("metering_public_key_hex").GetString());

            return new HttpResponseMessage(HttpStatusCode.Created)
            {
                Content = new StringContent($$"""
                    {
                      "session_id": "session-1",
                      "account_id": "sorauロ1Nholder",
                      "exit_class": "low-latency",
                      "relay_endpoint": "/dns4/vpn.sora.org/tcp/443/wss",
                      "lease_secs": 3600,
                      "expires_at_ms": 1700000000000,
                      "connected_at_ms": 1699999400000,
                      "meter_family": "vpn-standard",
                      "quote_id": "{{quoteId}}",
                      "payment_reference": "{{quoteId}}",
                      "payment_tx_hash": "{{paymentTxHash}}",
                      "fee_asset_id": "xor#universal.universal",
                      "escrow_account_id": "vpn_escrow",
                      "operator_account_id": "sorauロ1Noperator",
                      "lease_fee_nanos": 1000000,
                      "flow_label_bits": 20,
                      "padding_budget_ms": 80,
                      "relay_tls_spki_sha256_hex": "7878787878787878787878787878787878787878787878787878787878787878",
                      "route_pushes": ["10.0.0.0/8"],
                      "excluded_routes": ["127.0.0.0/8"],
                      "dns_servers": ["1.1.1.1"],
                      "tunnel_addresses": ["10.208.0.2/32"],
                      "mtu_bytes": 1280,
                      "helper_ticket_hex": "deadbeef",
                      "bytes_in": 123,
                      "bytes_out": 456,
                      "status": "active"
                    }
                    """),
            };
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var session = await client.CreateVpnSessionAsync(new ToriiVpnSessionCreateRequest
        {
            ExitClass = "low-latency",
            QuoteId = quoteId,
            PaymentTransactionHash = paymentTxHash,
            MeteringPublicKeyHex = meteringPublicKeyHex,
        });

        Assert.Equal("session-1", session.SessionId);
        Assert.Equal("low-latency", session.ExitClass);
        Assert.Equal("active", session.Status);
        Assert.Equal((ulong)1700000000000, session.ExpiresAtMilliseconds);
        Assert.Equal(quoteId, session.QuoteId);
        Assert.Equal(paymentTxHash, session.PaymentTransactionHash);
        Assert.Equal((ulong)1000000, session.LeaseFeeNanos);
    }

    [Fact]
    public async Task GetVpnSessionAsyncReturnsNullOnNotFoundAndDeserializesActiveSession()
    {
        const string quoteId = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

        var calls = 0;
        using var handler = new RecordingHandler(request =>
        {
            calls += 1;
            if (calls == 1)
            {
                Assert.Equal("/v1/vpn/sessions/missing", request.RequestUri!.AbsolutePath);
                return new HttpResponseMessage(HttpStatusCode.NotFound);
            }

            Assert.Equal("/v1/vpn/sessions/session-1", request.RequestUri!.AbsolutePath);
            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent($$"""
                    {
                      "session_id": "session-1",
                      "account_id": "sorauロ1Nholder",
                      "exit_class": "standard",
                      "relay_endpoint": "/dns4/vpn.sora.org/tcp/443/wss",
                      "lease_secs": 3600,
                      "expires_at_ms": 1700000000000,
                      "connected_at_ms": 1699999400000,
                      "meter_family": "vpn-standard",
                      "quote_id": "{{quoteId}}",
                      "payment_reference": "{{quoteId}}",
                      "payment_tx_hash": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                      "fee_asset_id": "xor#universal.universal",
                      "escrow_account_id": "vpn_escrow",
                      "operator_account_id": "sorauロ1Noperator",
                      "lease_fee_nanos": 1000000,
                      "flow_label_bits": 20,
                      "padding_budget_ms": 80,
                      "relay_tls_spki_sha256_hex": null,
                      "route_pushes": [],
                      "excluded_routes": [],
                      "dns_servers": [],
                      "tunnel_addresses": [],
                      "mtu_bytes": 1280,
                      "helper_ticket_hex": "deadbeef",
                      "bytes_in": 0,
                      "bytes_out": 0,
                      "status": "active"
                    }
                    """),
            };
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var missing = await client.GetVpnSessionAsync("missing");
        var active = await client.GetVpnSessionAsync("session-1");

        Assert.Null(missing);
        Assert.NotNull(active);
        Assert.Equal(quoteId, active.QuoteId);
    }

    [Fact]
    public async Task SubmitVpnReceiptAsyncPostsEvidenceAndDeserializesSettlementInstruction()
    {
        const string quoteId = "1111111111111111111111111111111111111111111111111111111111111111";

        using var handler = new RecordingHandler(request =>
        {
            var payload = ReadBodyAsJson(request);
            Assert.Equal(HttpMethod.Post, request.Method);
            Assert.Equal("/v1/vpn/receipts", request.RequestUri!.AbsolutePath);
            Assert.Equal("abcd", payload.RootElement.GetProperty("relay_receipt_hex").GetString());
            Assert.Equal("beef", payload.RootElement.GetProperty("client_voucher_hex").GetString());
            Assert.Equal(quoteId, payload.RootElement.GetProperty("lease_id_hex").GetString());

            return new HttpResponseMessage(HttpStatusCode.Created)
            {
                Content = new StringContent($$"""
                    {
                      "session_id": "session-1",
                      "account_id": "sorauロ1Nholder",
                      "exit_class": "standard",
                      "relay_endpoint": "/dns4/vpn.sora.org/tcp/443/wss",
                      "meter_family": "vpn-standard",
                      "connected_at_ms": 1699999400000,
                      "disconnected_at_ms": 1700000000000,
                      "duration_ms": 600000,
                      "bytes_in": 123,
                      "bytes_out": 456,
                      "status": "settled",
                      "receipt_source": "relay",
                      "quote_id": "{{quoteId}}",
                      "payment_tx_hash": "2222222222222222222222222222222222222222222222222222222222222222",
                      "fee_asset_id": "xor#universal.universal",
                      "escrow_account_id": "vpn_escrow",
                      "operator_account_id": "sorauロ1Noperator",
                      "lease_fee_nanos": 1000000,
                      "earned_fee_nanos": 500000,
                      "refunded_fee_nanos": 500000,
                      "lease_id_hex": "{{quoteId}}",
                      "settle_lease_instruction": {
                        "wire_id": "SettleVpnLease",
                        "payload_hex": "cafe"
                      },
                      "tx_instructions": [
                        {
                          "wire_id": "SettleVpnLease",
                          "payload_hex": "cafe"
                        }
                      ]
                    }
                    """),
            };
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var receipt = await client.SubmitVpnReceiptAsync(new ToriiVpnReceiptSubmitRequest
        {
            RelayReceiptHex = "abcd",
            ClientVoucherHex = "beef",
            LeaseIdHex = quoteId,
        });

        Assert.Equal("settled", receipt.Status);
        Assert.Equal("SettleVpnLease", receipt.SettleLeaseInstruction?.WireId);
        Assert.Single(receipt.TxInstructions);
        Assert.Equal((ulong)500000, receipt.EarnedFeeNanos);
    }

    [Fact]
    public async Task ListVpnReceiptsAsyncDeserializesNativeSettlementItems()
    {
        const string quoteId = "3333333333333333333333333333333333333333333333333333333333333333";

        using var handler = new RecordingHandler(request =>
        {
            Assert.Equal(HttpMethod.Get, request.Method);
            Assert.Equal("/v1/vpn/receipts", request.RequestUri!.AbsolutePath);
            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent($$"""
                    {
                      "items": [
                        {
                          "session_id": "session-1",
                          "account_id": "sorauロ1Nholder",
                          "exit_class": "standard",
                          "relay_endpoint": "/dns4/vpn.sora.org/tcp/443/wss",
                          "meter_family": "vpn-standard",
                          "connected_at_ms": 1699999400000,
                          "disconnected_at_ms": 1700000000000,
                          "duration_ms": 600000,
                          "bytes_in": 123,
                          "bytes_out": 456,
                          "status": "settled",
                          "receipt_source": "relay",
                          "quote_id": "{{quoteId}}",
                          "payment_tx_hash": "4444444444444444444444444444444444444444444444444444444444444444",
                          "fee_asset_id": "xor#universal.universal",
                          "escrow_account_id": "vpn_escrow",
                          "operator_account_id": "sorauロ1Noperator",
                          "lease_fee_nanos": 1000000,
                          "earned_fee_nanos": 500000,
                          "refunded_fee_nanos": 500000,
                          "lease_id_hex": "{{quoteId}}",
                          "settle_lease_instruction": {
                            "wire_id": "SettleVpnLease",
                            "payload_hex": "cafe"
                          },
                          "tx_instructions": []
                        }
                      ],
                      "total": 1
                    }
                    """),
            };
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var receipts = await client.ListVpnReceiptsAsync();

        Assert.Equal((ulong)1, receipts.Total);
        Assert.Single(receipts.Items);
        Assert.Equal(quoteId, receipts.Items[0].LeaseIdHex);
    }

    [Fact]
    public async Task DeleteVpnSessionAsyncReturnsNullForNotFound()
    {
        using var handler = new RecordingHandler(request =>
        {
            Assert.Equal(HttpMethod.Delete, request.Method);
            Assert.Equal("/v1/vpn/sessions/session-404", request.RequestUri!.AbsolutePath);

            return new HttpResponseMessage(HttpStatusCode.NotFound);
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var response = await client.DeleteVpnSessionAsync(" session-404 ");

        Assert.Null(response);
    }

    [Fact]
    public async Task DeleteVpnSessionAsyncDeserializesDisconnectedReceipt()
    {
        var quoteId = new string('6', 64);
        using var handler = new RecordingHandler(request =>
        {
            Assert.Equal(HttpMethod.Delete, request.Method);
            Assert.Equal("/v1/vpn/sessions/session-1", request.RequestUri!.AbsolutePath);

            return JsonResponse("""
                {
                  "session_id": "session-1",
                  "account_id": "sorauAlice",
                  "exit_class": "standard",
                  "relay_endpoint": "/dns4/vpn.sora.org/tcp/443/wss",
                  "meter_family": "vpn-standard",
                  "connected_at_ms": 1700000000000,
                  "disconnected_at_ms": 1700000000500,
                  "duration_ms": 500,
                  "bytes_in": 10,
                  "bytes_out": 20,
                  "status": "disconnected",
                  "receipt_source": "torii",
                  "quote_id": "{{quoteId}}",
                  "payment_tx_hash": "7777777777777777777777777777777777777777777777777777777777777777",
                  "fee_asset_id": "xor#universal.universal",
                  "escrow_account_id": "vpn_escrow",
                  "operator_account_id": "vpn_operator",
                  "lease_fee_nanos": 1000000,
                  "earned_fee_nanos": 0,
                  "refunded_fee_nanos": 1000000,
                  "lease_id_hex": "{{quoteId}}",
                  "settle_lease_instruction": null,
                  "tx_instructions": []
                }
                """.Replace("{{quoteId}}", quoteId));
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var response = await client.DeleteVpnSessionAsync(" session-1 ");

        Assert.NotNull(response);
        Assert.Equal("session-1", response.SessionId);
        Assert.Equal("disconnected", response.Status);
        Assert.Equal((ulong)1000000, response.RefundedFeeNanos);
    }

    [Fact]
    public async Task GetIdentifierPoliciesAsyncDeserializesPolicySummaries()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "total": 1,
                  "items": [
                    {
                      "policy_id": "phone#retail",
                      "owner": "sorauロ1Nowner",
                      "active": true,
                      "normalization": "phone_e164",
                      "resolver_public_key": "ed0120abcd",
                      "backend": "bfv-affine-sha3-256-v1",
                      "input_encryption": "bfv-v1",
                      "input_encryption_public_parameters": "DEADBEEF",
                      "input_encryption_public_parameters_decoded": {
                        "degree": 2048
                      },
                      "ram_fhe_profile": null,
                      "note": "retail phone resolver"
                    }
                  ]
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var policies = await client.GetIdentifierPoliciesAsync();

        Assert.Equal(1, policies.Total);
        Assert.Single(policies.Items);
        Assert.Equal("phone#retail", policies.Items[0].PolicyId);
        Assert.True(policies.Items[0].Active);
        Assert.Equal("bfv-affine-sha3-256-v1", policies.Items[0].Backend);
        Assert.Equal(2048, policies.Items[0].InputEncryptionPublicParametersDecoded!["degree"]!.GetValue<int>());
        Assert.Equal("retail phone resolver", policies.Items[0].Note);
        Assert.Equal("/v1/identifier-policies", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Theory]
    [InlineData(" phone#retail", "ed0120abcd", "identifier policies response.items[0].policy_id", "whitespace")]
    [InlineData("phone#retail ", "ed0120abcd", "identifier policies response.items[0].policy_id", "whitespace")]
    [InlineData("phone# retail", "ed0120abcd", "identifier policies response.items[0].policy_id.rule", "whitespace")]
    [InlineData("phone", "ed0120abcd", "identifier policies response.items[0].policy_id", "kind#rule")]
    [InlineData("phone#retail\u0001", "ed0120abcd", "identifier policies response.items[0].policy_id", "control")]
    [InlineData("phone#retail", "", "identifier policies response.items[0].resolver_public_key", "non-empty")]
    [InlineData("phone#retail", " ed0120abcd", "identifier policies response.items[0].resolver_public_key", "whitespace")]
    [InlineData("phone#retail", "ed0120abcd ", "identifier policies response.items[0].resolver_public_key", "whitespace")]
    [InlineData("phone#retail", "ed0120abcd\u0001", "identifier policies response.items[0].resolver_public_key", "control")]
    public async Task GetIdentifierPoliciesAsyncRejectsNonExactPolicySummaryFields(
        string policyId,
        string resolverPublicKey,
        string expectedField,
        string expectedReason)
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent($$"""
                {
                  "total": 1,
                  "items": [
                    {
                      "policy_id": {{JsonSerializer.Serialize(policyId)}},
                      "owner": "sorauロ1Nowner",
                      "active": true,
                      "normalization": "phone_e164",
                      "resolver_public_key": {{JsonSerializer.Serialize(resolverPublicKey)}},
                      "backend": "bfv-affine-sha3-256-v1"
                    }
                  ]
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var error = await Assert.ThrowsAsync<JsonException>(() => client.GetIdentifierPoliciesAsync());

        Assert.Contains(expectedField, error.Message);
        Assert.Contains(expectedReason, error.Message);
    }

    [Fact]
    public async Task GetSoraFsCidLookupAsyncDeserializesListing()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "content_cid": "bafylookup",
                  "manifest_digest_hex": "aa55",
                  "manifest_id_hex": "cc33",
                  "index_document": "index.html",
                  "files": [
                    {
                      "path": ["assets", "app.js"],
                      "offset": 1,
                      "size": 2,
                      "first_chunk": 3,
                      "chunk_count": 4
                    }
                  ]
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var lookup = await client.GetSoraFsCidLookupAsync(" bafylookup ");

        Assert.Equal("bafylookup", lookup.ContentCid);
        Assert.Equal("index.html", lookup.IndexDocument);
        Assert.Single(lookup.Files);
        Assert.Equal("assets", lookup.Files[0].Path[0]);
        Assert.Equal("/v1/sorafs/cid/bafylookup", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task RegisterSoraFsPinManifestAsyncPostsNormalizedPayloadAndDeserializesResponse()
    {
        var manifestHex = new string('a', 64);
        var chunkHex = new string('b', 64);
        var successorHex = new string('c', 64);
        var aliasProof = Convert.ToBase64String("alias-proof"u8.ToArray());

        using var handler = new RecordingHandler(request =>
        {
            var payload = ReadBodyAsJson(request);
            var root = payload.RootElement;
            Assert.Equal(HttpMethod.Post, request.Method);
            Assert.Equal("/v1/sorafs/pin/register", request.RequestUri!.AbsolutePath);
            Assert.Equal("application/json", request.Content!.Headers.ContentType!.MediaType);
            Assert.Equal("alice@boi", root.GetProperty("authority").GetString());
            Assert.Equal("ed25519:deadbeef", root.GetProperty("private_key").GetString());
            Assert.False(root.TryGetProperty("chunker", out _));
            Assert.Equal((uint)1, root.GetProperty("chunker_profile_id").GetUInt32());
            Assert.Equal("sorafs", root.GetProperty("chunker_namespace").GetString());
            Assert.Equal("sf1", root.GetProperty("chunker_name").GetString());
            Assert.Equal("1.0.0", root.GetProperty("chunker_semver").GetString());
            Assert.Equal((uint)0, root.GetProperty("chunker_multihash_code").GetUInt32());
            var pinPolicy = root.GetProperty("pin_policy");
            Assert.Equal((uint)3, pinPolicy.GetProperty("min_replicas").GetUInt32());
            Assert.Equal("Hot", pinPolicy.GetProperty("storage_class").GetProperty("type").GetString());
            Assert.Equal((ulong)72, pinPolicy.GetProperty("retention_epoch").GetUInt64());
            Assert.Equal(manifestHex, root.GetProperty("manifest_digest_hex").GetString());
            Assert.Equal(chunkHex, root.GetProperty("chunk_digest_sha3_256_hex").GetString());
            Assert.Equal((ulong)4096, root.GetProperty("content_length").GetUInt64());
            Assert.Equal((ulong)42, root.GetProperty("submitted_epoch").GetUInt64());
            Assert.Equal("docs", root.GetProperty("alias").GetProperty("namespace").GetString());
            Assert.Equal("main", root.GetProperty("alias").GetProperty("name").GetString());
            Assert.Equal(aliasProof, root.GetProperty("alias").GetProperty("proof_base64").GetString());
            Assert.Equal(successorHex, root.GetProperty("successor_of_hex").GetString());

            return JsonResponse($$"""
                {
                  "manifest_digest_hex": "{{manifestHex.ToUpperInvariant()}}",
                  "chunker_handle": "sorafs.sf1@1.0.0",
                  "submitted_epoch": 42,
                  "content_length": 4096,
                  "pin_fee_nano": 500000000,
                  "pin_fee_asset_id": "xor#universal",
                  "pin_fee_treasury_account_id": "treasury@boi",
                  "alias": {
                    "namespace": "docs",
                    "name": "main",
                    "proof_base64": "{{aliasProof}}"
                  },
                  "successor_of_hex": "0x{{successorHex.ToUpperInvariant()}}"
                }
                """);
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var response = await client.RegisterSoraFsPinManifestAsync(ValidSoraFsPinRegisterRequest());

        Assert.Equal(manifestHex, response.ManifestDigestHex);
        Assert.Equal("sorafs.sf1@1.0.0", response.ChunkerHandle);
        Assert.Equal((ulong)42, response.SubmittedEpoch);
        Assert.Equal((ulong)4096, response.ContentLength);
        Assert.Equal((ulong)500000000, response.PinFeeNano);
        Assert.Equal("xor#universal", response.PinFeeAssetId);
        Assert.Equal("treasury@boi", response.PinFeeTreasuryAccountId);
        Assert.Equal("docs", response.Alias!.Namespace);
        Assert.Equal("main", response.Alias.Name);
        Assert.Equal(aliasProof, response.Alias.ProofBase64);
        Assert.Equal(successorHex, response.SuccessorOfHex);
    }

    [Fact]
    public async Task RegisterSoraFsPinManifestAsyncRejectsMalformedInputsBeforeRequest()
    {
        var valid = ValidSoraFsPinRegisterRequest();
        var invalidRequests = new[]
        {
            valid with { ManifestDigestHex = "abc123" },
            valid with { ChunkDigestSha3_256Hex = new string('z', 64) },
            valid with { SuccessorOfHex = new string('c', 63) },
            valid with { ContentLength = null },
            valid with { SubmittedEpoch = null },
            valid with { Chunker = null },
            valid with { Chunker = valid.Chunker! with { ProfileId = null } },
            valid with { Chunker = valid.Chunker! with { ProfileId = 0 } },
            valid with { Chunker = valid.Chunker! with { Namespace = " " } },
            valid with { Chunker = valid.Chunker! with { Semver = "" } },
            valid with { PinPolicy = null },
            valid with { PinPolicy = valid.PinPolicy! with { MinReplicas = null } },
            valid with { PinPolicy = valid.PinPolicy! with { MinReplicas = 0 } },
            valid with { PinPolicy = valid.PinPolicy! with { StorageClass = null } },
            valid with
            {
                PinPolicy = valid.PinPolicy! with
                {
                    StorageClass = ToriiSoraFsStorageClass.From("lava"),
                },
            },
            valid with { Alias = valid.Alias! with { Namespace = "" } },
            valid with { Alias = valid.Alias! with { ProofBase64 = null } },
            valid with { Alias = valid.Alias! with { ProofBase64 = "not base64!" } },
            valid with { Alias = valid.Alias! with { ProofBase64 = Convert.ToBase64String(Array.Empty<byte>()) } },
        };

        using var handler = new RecordingHandler(_ => throw new InvalidOperationException("request should not be sent"));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        foreach (var request in invalidRequests)
        {
            await Assert.ThrowsAnyAsync<ArgumentException>(() => client.RegisterSoraFsPinManifestAsync(request));
            Assert.Null(handler.LastRequest);
        }
    }

    [Fact]
    public async Task RegisterSoraFsPinManifestAsyncRejectsMalformedResponse()
    {
        using var handler = new RecordingHandler(_ => JsonResponse("""
            {
              "manifest_digest_hex": "abc123",
              "chunker_handle": "sorafs.sf1@1.0.0",
              "submitted_epoch": 42,
              "content_length": 4096,
              "pin_fee_nano": 500000000,
              "pin_fee_asset_id": "xor#universal",
              "pin_fee_treasury_account_id": "treasury@boi"
            }
            """));

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        await Assert.ThrowsAnyAsync<ArgumentException>(() =>
            client.RegisterSoraFsPinManifestAsync(ValidSoraFsPinRegisterRequest()));
        Assert.Equal("/v1/sorafs/pin/register", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task RegisterVerifyingKeyAsyncCanonicalizesInlineCommitmentPayload()
    {
        var vkBytes = "abc"u8.ToArray();
        var commitmentHex = VerifyingKeyCommitmentHex("halo2/ipa", vkBytes);
        using var handler = new RecordingHandler(request =>
        {
            Assert.Equal("/v1/zk/vk/register", request.RequestUri!.AbsolutePath);
            using var body = ReadBodyAsJson(request);
            var root = body.RootElement;
            Assert.Equal("alice", root.GetProperty("authority").GetString());
            Assert.Equal("ed25519:deadbeef", root.GetProperty("private_key").GetString());
            Assert.Equal("halo2/ipa", root.GetProperty("backend").GetString());
            Assert.Equal("vk_main", root.GetProperty("name").GetString());
            Assert.Equal(1u, root.GetProperty("version").GetUInt32());
            Assert.Equal(new string('a', 64), root.GetProperty("public_inputs_schema_hash_hex").GetString());
            Assert.Equal(commitmentHex, root.GetProperty("commitment_hex").GetString());
            Assert.Equal(Convert.ToBase64String(vkBytes), root.GetProperty("vk_bytes").GetString());
            Assert.Equal(3u, root.GetProperty("vk_len").GetUInt32());
            Assert.Equal("Active", root.GetProperty("status").GetString());
            return JsonResponse("""{"accepted":true}""", HttpStatusCode.Accepted);
        });
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        using var response = await client.RegisterVerifyingKeyAsync(new ToriiVerifyingKeyRegisterRequest
        {
            Authority = " alice ",
            PrivateKey = " ed25519:deadbeef ",
            Backend = "halo2/ipa",
            Name = " vk_main ",
            Version = 1,
            CircuitId = " halo2/ipa::transfer_v1 ",
            PublicInputsSchemaHashHex = "0x" + new string('A', 64),
            GasScheduleId = " halo2_default ",
            VerifyingKeyBytes = vkBytes,
            CommitmentHex = commitmentHex.ToUpperInvariant(),
            Status = "active",
        });

        Assert.True(response.RootElement.GetProperty("accepted").GetBoolean());
    }

    [Fact]
    public async Task UpdateVerifyingKeyAsyncCanonicalizesPrivateKeyAndInlineCommitmentPayload()
    {
        var vkBytes = "abcd"u8.ToArray();
        var commitmentHex = VerifyingKeyCommitmentHex("halo2/ipa", vkBytes);
        using var handler = new RecordingHandler(request =>
        {
            Assert.Equal("/v1/zk/vk/update", request.RequestUri!.AbsolutePath);
            using var body = ReadBodyAsJson(request);
            var root = body.RootElement;
            Assert.Equal("alice", root.GetProperty("authority").GetString());
            Assert.Equal("ed25519:deadbeef", root.GetProperty("private_key").GetString());
            Assert.Equal("halo2/ipa", root.GetProperty("backend").GetString());
            Assert.Equal("vk_main", root.GetProperty("name").GetString());
            Assert.Equal(2u, root.GetProperty("version").GetUInt32());
            Assert.Equal(new string('b', 64), root.GetProperty("public_inputs_schema_hash_hex").GetString());
            Assert.Equal(commitmentHex, root.GetProperty("commitment_hex").GetString());
            Assert.Equal(Convert.ToBase64String(vkBytes), root.GetProperty("vk_bytes").GetString());
            Assert.Equal(4u, root.GetProperty("vk_len").GetUInt32());
            Assert.Equal("Withdrawn", root.GetProperty("status").GetString());
            return JsonResponse("""{"accepted":true}""", HttpStatusCode.Accepted);
        });
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        using var response = await client.UpdateVerifyingKeyAsync(new ToriiVerifyingKeyUpdateRequest
        {
            Authority = " alice ",
            PrivateKey = " ed25519:deadbeef ",
            Backend = "halo2/ipa",
            Name = " vk_main ",
            Version = 2,
            CircuitId = " halo2/ipa::transfer_v2 ",
            PublicInputsSchemaHashHex = "0x" + new string('B', 64),
            VerifyingKeyBytes = vkBytes,
            CommitmentHex = commitmentHex.ToUpperInvariant(),
            Status = "withdrawn",
        });

        Assert.True(response.RootElement.GetProperty("accepted").GetBoolean());
    }

    [Fact]
    public async Task UpdateVerifyingKeyAsyncRejectsMismatchedInlineCommitmentBeforeRequest()
    {
        using var handler = new RecordingHandler(_ => throw new InvalidOperationException("request must not be sent"));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        var error = await Assert.ThrowsAsync<ArgumentException>(() =>
            client.UpdateVerifyingKeyAsync(new ToriiVerifyingKeyUpdateRequest
            {
                Authority = "alice",
                PrivateKey = "ed25519:deadbeef",
                Backend = "halo2/ipa",
                Name = "vk_main",
                Version = 2,
                CircuitId = "halo2/ipa::transfer_v2",
                PublicInputsSchemaHashHex = new string('b', 64),
                VerifyingKeyBytes = "abc"u8.ToArray(),
                CommitmentHex = new string('0', 64),
            }));
        Assert.Contains("commitment_hex must match domain-separated SHA-256", error.Message);
        Assert.Null(handler.LastRequest);
    }

    [Fact]
    public async Task VerifyingKeyRequestsRejectMalformedInputsBeforeRequest()
    {
        using var handler = new RecordingHandler(_ => throw new InvalidOperationException("request must not be sent"));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        var valid = ValidVerifyingKeyRegisterRequest();
        ToriiVerifyingKeyRegisterRequest[] invalid =
        {
            valid with { Backend = " halo2/ipa" },
            valid with { PrivateKey = "" },
            valid with { Backend = "halo2/ipa/orchard" },
            valid with { Backend = "halo2/\u200Bipa" },
            valid with { Name = "vk:main" },
            valid with { Name = " " },
            valid with { Version = 0 },
            valid with { PublicInputsSchemaHashHex = "abc123" },
            valid with { GasScheduleId = "" },
            valid with { VerifyingKeyBytes = Array.Empty<byte>() },
            valid with { VerifyingKeyLength = 99 },
            valid with { VerifyingKeyBytes = null, VerifyingKeyLength = 3, CommitmentHex = null },
            valid with { ActivationHeight = 10, WithdrawHeight = 9 },
            valid with { Status = "production-ready" },
        };

        foreach (var request in invalid)
        {
            await Assert.ThrowsAnyAsync<ArgumentException>(() => client.RegisterVerifyingKeyAsync(request));
            Assert.Null(handler.LastRequest);
        }

        foreach (var backend in new[] { "halo2/ipa ", "halo2\uFF0Fipa", "mock/dev" })
        {
            await Assert.ThrowsAnyAsync<ArgumentException>(() => client.GetVerifyingKeyAsync(backend, "vk_main"));
            Assert.Null(handler.LastRequest);
        }

        await Assert.ThrowsAnyAsync<ArgumentException>(() =>
            client.UpdateVerifyingKeyAsync(new ToriiVerifyingKeyUpdateRequest
            {
                Authority = "alice",
                PrivateKey = "ed25519:deadbeef",
                Backend = "halo2/ipa",
                Name = "vk_main",
                Version = 2,
                CircuitId = "halo2/ipa::transfer_v2",
                PublicInputsSchemaHashHex = new string('b', 64),
                ActivationHeight = 10,
                WithdrawHeight = 9,
            }));
        Assert.Null(handler.LastRequest);

        await Assert.ThrowsAnyAsync<ArgumentException>(() =>
            client.UpdateVerifyingKeyAsync(new ToriiVerifyingKeyUpdateRequest
            {
                Authority = "alice",
                PrivateKey = "",
                Backend = "halo2/ipa",
                Name = "vk_main",
                Version = 2,
                CircuitId = "halo2/ipa::transfer_v2",
                PublicInputsSchemaHashHex = new string('b', 64),
            }));
        Assert.Null(handler.LastRequest);
    }

    [Fact]
    public async Task GetSoraFsDenylistCatalogAsyncDeserializesPackSummaries()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "version": 1,
                  "jurisdiction": "global",
                  "opt_out_packs": ["global-emergency"],
                  "extra_packs": [],
                  "packs": [
                    {
                      "pack_id": "global-core",
                      "version": "2026-03-29",
                      "default_enabled": true,
                      "active": true,
                      "policy_tier": "standard",
                      "manifest_cid": "bafycorepack",
                      "merkle_root": "root",
                      "issued_by_proposal_id": null,
                      "review_reference": "ref-1",
                      "jurisdiction": "global",
                      "issued_at": "2026-03-29T00:00:00Z",
                      "expires_at": null,
                      "entry_count": 10
                    }
                  ]
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var catalog = await client.GetSoraFsDenylistCatalogAsync();

        Assert.Equal(1, catalog.Version);
        Assert.Single(catalog.Packs);
        Assert.Equal("global-core", catalog.Packs[0].PackId);
        Assert.True(catalog.Packs[0].Active);
        Assert.Equal("/v1/sorafs/denylist/catalog", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task GetSoraFsDenylistPackAsyncEncodesPackIdAndDeserializesResponse()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "pack_id": "global core",
                  "version": "2026-03-29",
                  "default_enabled": true,
                  "active": true,
                  "policy_tier": "standard",
                  "manifest_cid": "bafycorepack",
                  "merkle_root": "root",
                  "issued_by_proposal_id": "proposal-1",
                  "review_reference": "ref-1",
                  "jurisdiction": "global",
                  "issued_at": "2026-03-29T00:00:00Z",
                  "expires_at": null,
                  "entry_count": 10,
                  "source_path": "/tmp/global-core.json"
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var pack = await client.GetSoraFsDenylistPackAsync(" global core ");

        Assert.Equal("global core", pack.PackId);
        Assert.Equal("/tmp/global-core.json", pack.SourcePath);
        Assert.Equal("/v1/sorafs/denylist/packs/global%20core", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task OpenSoraFsCidContentAsyncUsesRootGatewayPath()
    {
        using var handler = new RecordingHandler(_ =>
        {
            var response = new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new ByteArrayContent("<html/>"u8.ToArray()),
            };
            response.Content.Headers.ContentType = new("text/html");
            response.Headers.TryAddWithoutValidation("sora-content-cid", "bafyroot");
            return response;
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        using var response = await client.OpenSoraFsCidContentAsync(" bafyroot ");
        var bytes = await response.Content.ReadAsByteArrayAsync();

        Assert.Equal("/sorafs/cid/bafyroot/", handler.LastRequest!.RequestUri!.AbsolutePath);
        Assert.Equal("<html/>", System.Text.Encoding.UTF8.GetString(bytes));
        Assert.Equal("bafyroot", response.Headers.GetValues("sora-content-cid").Single());
    }

    [Fact]
    public async Task GetSoraFsCidContentAsyncEncodesNestedPathAndReturnsHeaders()
    {
        using var handler = new RecordingHandler(_ =>
        {
            var response = new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new ByteArrayContent("console.log('ok');"u8.ToArray()),
            };
            response.Content.Headers.ContentType = new("text/javascript")
            {
                CharSet = "utf-8",
            };
            response.Headers.TryAddWithoutValidation("sora-content-cid", "bafynested");
            return response;
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var content = await client.GetSoraFsCidContentAsync("bafynested", "assets/app main.js");

        Assert.Equal("/sorafs/cid/bafynested/assets/app%20main.js", handler.LastRequest!.RequestUri!.AbsolutePath);
        Assert.Equal("text/javascript; charset=utf-8", content.ContentType);
        Assert.Equal("bafynested", content.ContentCid);
        Assert.Equal("console.log('ok');", System.Text.Encoding.UTF8.GetString(content.Bytes));
        Assert.Equal(content.Bytes.Length, content.ContentLength);
    }

    [Fact]
    public async Task SubmitSignedQueryAsyncPostsVersionedNoritoPayload()
    {
        var queryBytes = new byte[] { 1, 2, 3, 4, 5 };

        using var handler = new RecordingHandler(request =>
        {
            Assert.Equal(HttpMethod.Post, request.Method);
            Assert.Equal("/query", request.RequestUri!.AbsolutePath);
            Assert.Equal("limit=1", request.RequestUri.Query.TrimStart('?'));
            Assert.Equal("application/x-norito", request.Content!.Headers.ContentType!.MediaType);

            using var stream = request.Content.ReadAsStream();
            using var buffer = new MemoryStream();
            stream.CopyTo(buffer);
            Assert.Equal(queryBytes, buffer.ToArray());

            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("""
                    {
                      "kind": "Singular",
                      "value": {
                        "id": "sorauﾛ1Nmerchant"
                      }
                    }
                    """),
            };
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        using var response = await client.SubmitSignedQueryAsync(queryBytes, query: "limit=1");

        Assert.Equal("Singular", response.RootElement.GetProperty("kind").GetString());
        Assert.Equal("sorauﾛ1Nmerchant", response.RootElement.GetProperty("value").GetProperty("id").GetString());
    }

    [Fact]
    public async Task SubmitSignedQueryAsyncAcceptsManagedEnvelope()
    {
        SignedQueryEnvelope? seenEnvelope = null;
        using var handler = new RecordingHandler(request =>
        {
            Assert.Equal(HttpMethod.Post, request.Method);
            Assert.Equal("/query", request.RequestUri!.AbsolutePath);
            Assert.Equal("limit=1", request.RequestUri.Query.TrimStart('?'));
            Assert.Equal("application/x-norito", request.Content!.Headers.ContentType!.MediaType);

            using var stream = request.Content.ReadAsStream();
            using var buffer = new MemoryStream();
            stream.CopyTo(buffer);
            Assert.Equal(seenEnvelope!.VersionedNoritoBytes, buffer.ToArray());

            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("""
                    {
                      "kind": "Singular",
                      "value": {
                        "kind": "FindParameters"
                      }
                    }
                    """),
            };
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        seenEnvelope = new SignedQueryBuilder("sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")
            .FindParameters()
            .BuildSigned(Convert.FromHexString("616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032"));

        using var response = await client.SubmitSignedQueryAsync(seenEnvelope, query: "limit=1");

        Assert.Equal("Singular", response.RootElement.GetProperty("kind").GetString());
        Assert.Equal("FindParameters", response.RootElement.GetProperty("value").GetProperty("kind").GetString());
    }

    [Fact]
    public async Task OpenEventSseAsyncRequestsEventStreamAcceptHeader()
    {
        using var handler = new RecordingHandler(_ =>
        {
            var response = new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("event: pipeline\ndata: {\"kind\":\"Queued\"}\n\n"),
            };
            response.Content.Headers.ContentType = new("text/event-stream");
            return response;
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        using var response = await client.OpenEventSseAsync("scope=auto", "resume-id");

        Assert.Equal("/v1/events/sse", handler.LastRequest!.RequestUri!.AbsolutePath);
        Assert.Equal("scope=auto", handler.LastRequest.RequestUri.Query.TrimStart('?'));
        Assert.Equal("resume-id", handler.LastRequest.Headers.GetValues("Last-Event-ID").Single());
        Assert.Contains(handler.LastRequest.Headers.Accept, static value => value.MediaType == "text/event-stream");
        Assert.Equal("text/event-stream", response.Content.Headers.ContentType?.MediaType);
    }

    [Fact]
    public async Task StreamEventsAsyncParsesCommentAndJsonFrames()
    {
        using var handler = new RecordingHandler(_ =>
        {
            var response = new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("""
                    : keepalive

                    id: block-1
                    event: pipeline.block
                    retry: 1500
                    data: {"height":1,"status":"Applied"}

                    """),
            };
            response.Content.Headers.ContentType = new("text/event-stream");
            return response;
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var events = new List<ToriiServerSentEvent>();

        await foreach (var sseEvent in client.StreamEventsAsync("scope=auto"))
        {
            events.Add(sseEvent);
        }

        Assert.Equal(2, events.Count);
        Assert.True(events[0].IsComment);
        Assert.Equal("keepalive", events[0].Comment);
        Assert.Null(events[0].RawData);

        Assert.Equal("pipeline.block", events[1].Event);
        Assert.Equal("block-1", events[1].Id);
        Assert.Equal(1500, events[1].RetryMilliseconds);
        Assert.Equal("{\"height\":1,\"status\":\"Applied\"}", events[1].RawData);
        Assert.NotNull(events[1].JsonData);
        Assert.Equal(1, events[1].JsonData!["height"]!.GetValue<int>());
        Assert.Equal("Applied", events[1].JsonData!["status"]!.GetValue<string>());
    }

    [Fact]
    public async Task OpenEventSseAsyncRejectsUnsupportedProductionBackendEventFiltersBeforeRequest()
    {
        using var handler = new RecordingHandler(_ => throw new InvalidOperationException("request must not be sent"));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        foreach (var filterJson in new[]
        {
            """{"VerifyingKey":{"id_matcher":{"backend":"halo2/ipa/orchard","name":"vk"},"event_set":{"Registered":true}}}""",
            """{"VerifyingKey":{"id_matcher":{"backend":" halo2/ipa","name":"vk"},"event_set":{"Registered":true}}}""",
            """{"Proof":{"id_matcher":{"backend":"mock/dev","hash_hex":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},"event_set":{"Verified":true}}}""",
            """{"Proof":{"id_matcher":{"backend":"groth16/bls12-377","hash_hex":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},"event_set":{"Verified":true}}}""",
        })
        {
            var error = await Assert.ThrowsAsync<ArgumentException>(
                () => client.OpenEventSseAsync(EventFilterQuery(filterJson)));
            var expected = filterJson.Contains("\"backend\":\" ")
                ? "surrounding whitespace"
                : "unsupported production verifier backend";
            Assert.Contains(expected, error.Message);
            Assert.Null(handler.LastRequest);
        }
    }

    [Fact]
    public async Task OpenEventSseAsyncRejectsMalformedVerifyingKeyEventNamesBeforeRequest()
    {
        using var handler = new RecordingHandler(_ => throw new InvalidOperationException("request must not be sent"));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        foreach (var nameJson in new[]
        {
            "\"\"",
            "\"   \"",
            "\"\\t\"",
            "\"\\n\"",
            "\"vk:main\"",
            "42",
        })
        {
            var filterJson =
                "{\"VerifyingKey\":{\"id_matcher\":{\"backend\":\"halo2/ipa\",\"name\":"
                + nameJson
                + "},\"event_set\":{\"Registered\":true}}}";
            var error = await Assert.ThrowsAsync<ArgumentException>(
                () => client.OpenEventSseAsync(EventFilterQuery(filterJson)));
            Assert.True(
                error.Message.Contains("non-empty string", StringComparison.Ordinal)
                    || error.Message.Contains("must not contain ':'", StringComparison.Ordinal)
                    || error.Message.Contains("must be a string", StringComparison.Ordinal),
                $"unexpected error: {error.Message}");
            Assert.Null(handler.LastRequest);
        }
    }

    [Fact]
    public async Task OpenEventSseAsyncCanonicalizesVerifyingKeyEventNamesBeforeRequest()
    {
        using var handler = new RecordingHandler(_ =>
        {
            var response = new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent(": keepalive\n\n"),
            };
            response.Content.Headers.ContentType = new("text/event-stream");
            return response;
        });
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        using var response = await client.OpenEventSseAsync(EventFilterQuery(
            """{"VerifyingKey":{"id_matcher":{"backend":"halo2/ipa","name":" vk_main "},"event_set":{"Registered":true}}}"""));

        var filter = QueryParameter(handler.LastRequest!.RequestUri!.Query, "filter");
        Assert.Contains("\"name\":\"vk_main\"", filter);
    }

    [Fact]
    public async Task StreamEventsAsyncRejectsMalformedProofEventHashesBeforeRequest()
    {
        using var handler = new RecordingHandler(_ => throw new InvalidOperationException("request must not be sent"));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        foreach (var hashJson in new[]
        {
            "\"\"",
            "\"abc\"",
            "\"" + new string('z', 64) + "\"",
            "\"0x0x" + new string('a', 64) + "\"",
            "42",
        })
        {
            var filterJson =
                "{\"Proof\":{\"id_matcher\":{\"backend\":\"halo2/ipa\",\"hash_hex\":"
                + hashJson
                + "},\"event_set\":{\"Verified\":true}}}";
            var error = await Assert.ThrowsAsync<ArgumentException>(async () =>
            {
                await foreach (var _ in client.StreamEventsAsync(EventFilterQuery(filterJson)))
                {
                }
            });
            Assert.True(
                error.Message.Contains("32-byte hex string", StringComparison.Ordinal)
                    || error.Message.Contains("must be a string", StringComparison.Ordinal),
                $"unexpected error: {error.Message}");
            Assert.Null(handler.LastRequest);
        }
    }

    [Fact]
    public async Task OpenEventSseAsyncCanonicalizesProofHashEventFiltersBeforeRequest()
    {
        using var handler = new RecordingHandler(_ =>
        {
            var response = new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent(": keepalive\n\n"),
            };
            response.Content.Headers.ContentType = new("text/event-stream");
            return response;
        });
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        var hashHex = new string('A', 64);
        var proofHashHex = new string('B', 64);
        var filterJson =
            "{\"Proof\":{\"id_matcher\":{\"backend\":\"halo2/ipa\",\"hash_hex\":\"0x"
            + hashHex
            + "\",\"proof_hash_hex\":\""
            + proofHashHex
            + "\"},\"event_set\":{\"Verified\":true}}}";

        using var response = await client.OpenEventSseAsync(EventFilterQuery(filterJson));

        var filter = QueryParameter(handler.LastRequest!.RequestUri!.Query, "filter");
        Assert.Contains("\"hash_hex\":\"" + new string('a', 64) + "\"", filter);
        Assert.Contains("\"proof_hash_hex\":\"" + new string('b', 64) + "\"", filter);
    }

    [Fact]
    public async Task StreamPipelineEventsAsyncDeserializesPipelinePayloadsAndSkipsNonPipelineEvents()
    {
        using var handler = new RecordingHandler(_ =>
        {
            var response = new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("""
                    : keepalive

                    id: tx-1
                    event: pipeline.transaction
                    retry: 1500
                    data: {"category":"Pipeline","event":"Transaction","hash":"abc123","lane_id":3,"dataspace_id":7,"block_height":11,"status":"Approved"}

                    data: {"category":"Data","event":"ProofVerified","backend":"groth16"}

                    """),
            };
            response.Content.Headers.ContentType = new("text/event-stream");
            return response;
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var events = new List<ToriiPipelineEvent>();

        await foreach (var pipelineEvent in client.StreamPipelineEventsAsync("scope=auto"))
        {
            events.Add(pipelineEvent);
        }

        var typed = Assert.Single(events);
        Assert.Equal("Pipeline", typed.Category);
        Assert.Equal("Transaction", typed.Event);
        Assert.Equal("Approved", typed.Status);
        Assert.Equal("abc123", typed.Hash);
        Assert.Equal((ulong)3, typed.LaneId);
        Assert.Equal((ulong)7, typed.DataspaceId);
        Assert.Equal((ulong)11, typed.BlockHeight);
        Assert.Equal("tx-1", typed.LastEventId);
        Assert.Equal("pipeline.transaction", typed.SseEventName);
        Assert.Equal(1500, typed.RetryMilliseconds);
    }

    [Fact]
    public async Task StreamProofEventsAsyncDeserializesProofPayloadsAndSkipsOtherEvents()
    {
        using var handler = new RecordingHandler(_ =>
        {
            var response = new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("""
                    : keepalive

                    id: proof-1
                    event: data.proof
                    retry: 2500
                    data: {"category":"Data","event":"ProofVerified","backend":"halo2/ipa","proof_hash":"33","call_hash":"aa","envelope_hash":"10","vk_ref":"halo2/ipa::vk_name","vk_commitment":"55"}

                    data: {"category":"Pipeline","event":"Transaction","hash":"abc123"}

                    data: {"category":"Data","event":"ProofPruned","backend":"halo2/ipa","removed_count":1,"remaining":3,"cap":32,"grace_blocks":64,"prune_batch":8,"pruned_at_height":777,"pruned_by":"peer-1","origin":"Automatic","removed":[{"backend":"halo2/ipa","proof_hash":"44"}]}

                    """),
            };
            response.Content.Headers.ContentType = new("text/event-stream");
            return response;
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var events = new List<ToriiProofEvent>();

        await foreach (var proofEvent in client.StreamProofEventsAsync("scope=auto"))
        {
            events.Add(proofEvent);
        }

        Assert.Equal(2, events.Count);

        var verified = events[0];
        Assert.Equal("Data", verified.Category);
        Assert.Equal("ProofVerified", verified.Event);
        Assert.Equal("halo2/ipa", verified.Backend);
        Assert.Equal("33", verified.ProofHash);
        Assert.Equal("aa", verified.CallHash);
        Assert.Equal("10", verified.EnvelopeHash);
        Assert.Equal("halo2/ipa::vk_name", verified.VerificationKeyReference);
        Assert.Equal("55", verified.VerificationKeyCommitment);
        Assert.Equal("proof-1", verified.LastEventId);
        Assert.Equal("data.proof", verified.SseEventName);
        Assert.Equal(2500, verified.RetryMilliseconds);

        var pruned = events[1];
        Assert.Equal("ProofPruned", pruned.Event);
        Assert.Equal((ulong)1, pruned.RemovedCount);
        Assert.Equal((ulong)3, pruned.Remaining);
        Assert.Equal((ulong)32, pruned.Cap);
        Assert.Equal((ulong)64, pruned.GraceBlocks);
        Assert.Equal((ulong)8, pruned.PruneBatch);
        Assert.Equal((ulong)777, pruned.PrunedAtHeight);
        Assert.Equal("peer-1", pruned.PrunedBy);
        Assert.Equal("Automatic", pruned.Origin);
        var removed = Assert.Single(pruned.Removed!);
        Assert.Equal("halo2/ipa", removed.Backend);
        Assert.Equal("44", removed.ProofHash);
    }

    [Fact]
    public async Task OpenExplorerBlocksSseAsyncRequestsEventStreamAcceptHeader()
    {
        using var handler = new RecordingHandler(_ =>
        {
            var response = new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("data: {\"hash\":\"abc\",\"height\":1}\n\n"),
            };
            response.Content.Headers.ContentType = new("text/event-stream");
            return response;
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        using var response = await client.OpenExplorerBlocksSseAsync("resume-block");

        Assert.Equal("/v1/explorer/blocks/stream", handler.LastRequest!.RequestUri!.AbsolutePath);
        Assert.Equal(string.Empty, handler.LastRequest.RequestUri.Query);
        Assert.Equal("resume-block", handler.LastRequest.Headers.GetValues("Last-Event-ID").Single());
        Assert.Contains(handler.LastRequest.Headers.Accept, static value => value.MediaType == "text/event-stream");
        Assert.Equal("text/event-stream", response.Content.Headers.ContentType?.MediaType);
    }

    [Fact]
    public async Task StreamExplorerBlocksAsyncDeserializesBlockPayloadsAndSkipsComments()
    {
        using var handler = new RecordingHandler(_ =>
        {
            var response = new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("""
                    : keepalive

                    data: {"hash":"block-abc","height":42,"created_at":"2026-03-29T00:00:00Z","prev_block_hash":"block-prev","transactions_hash":"tx-root","transactions_rejected":1,"transactions_total":3}

                    """),
            };
            response.Content.Headers.ContentType = new("text/event-stream");
            return response;
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var events = new List<ToriiExplorerBlock>();

        await foreach (var block in client.StreamExplorerBlocksAsync())
        {
            events.Add(block);
        }

        var typed = Assert.Single(events);
        Assert.Equal("block-abc", typed.Hash);
        Assert.Equal((ulong)42, typed.Height);
        Assert.Equal("2026-03-29T00:00:00Z", typed.CreatedAt);
        Assert.Equal("block-prev", typed.PreviousBlockHash);
        Assert.Equal("tx-root", typed.TransactionsHash);
        Assert.Equal((uint)1, typed.TransactionsRejected);
        Assert.Equal((uint)3, typed.TransactionsTotal);
    }

    [Fact]
    public async Task OpenExplorerTransactionsSseAsyncRequestsEventStreamAcceptHeader()
    {
        using var handler = new RecordingHandler(_ =>
        {
            var response = new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("data: {\"authority\":\"alice\",\"hash\":\"tx1\",\"block\":1,\"created_at\":\"2026-03-29T00:00:00Z\",\"executable\":\"Instructions\",\"status\":\"Committed\"}\n\n"),
            };
            response.Content.Headers.ContentType = new("text/event-stream");
            return response;
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        using var response = await client.OpenExplorerTransactionsSseAsync("resume-tx");

        Assert.Equal("/v1/explorer/transactions/stream", handler.LastRequest!.RequestUri!.AbsolutePath);
        Assert.Equal(string.Empty, handler.LastRequest.RequestUri.Query);
        Assert.Equal("resume-tx", handler.LastRequest.Headers.GetValues("Last-Event-ID").Single());
        Assert.Contains(handler.LastRequest.Headers.Accept, static value => value.MediaType == "text/event-stream");
        Assert.Equal("text/event-stream", response.Content.Headers.ContentType?.MediaType);
    }

    [Fact]
    public async Task StreamExplorerTransactionsAsyncDeserializesTransactionPayloads()
    {
        using var handler = new RecordingHandler(_ =>
        {
            var response = new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("""
                    : keepalive

                    data: {"authority":"sorau123","hash":"tx-abc","block":99,"created_at":"2026-03-29T01:02:03Z","executable":"Instructions","status":"Rejected"}

                    """),
            };
            response.Content.Headers.ContentType = new("text/event-stream");
            return response;
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var events = new List<ToriiExplorerTransaction>();

        await foreach (var transaction in client.StreamExplorerTransactionsAsync())
        {
            events.Add(transaction);
        }

        var typed = Assert.Single(events);
        Assert.Equal("sorau123", typed.Authority);
        Assert.Equal("tx-abc", typed.Hash);
        Assert.Equal((ulong)99, typed.Block);
        Assert.Equal("2026-03-29T01:02:03Z", typed.CreatedAt);
        Assert.Equal("Instructions", typed.Executable);
        Assert.Equal("Rejected", typed.Status);
    }

    [Fact]
    public async Task OpenExplorerInstructionsSseAsyncRequestsEventStreamAcceptHeader()
    {
        using var handler = new RecordingHandler(_ =>
        {
            var response = new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("data: {\"authority\":\"alice\",\"created_at\":\"2026-03-29T00:00:00Z\",\"kind\":\"Transfer\",\"box\":{\"encoded\":\"0x11\",\"json\":{\"kind\":\"Transfer\",\"payload\":{},\"wire_id\":\"iroha.transfer\",\"encoded\":\"11\"}},\"transaction_hash\":\"tx1\",\"transaction_status\":\"Committed\",\"block\":1,\"index\":0}\n\n"),
            };
            response.Content.Headers.ContentType = new("text/event-stream");
            return response;
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        using var response = await client.OpenExplorerInstructionsSseAsync("resume-instruction");

        Assert.Equal("/v1/explorer/instructions/stream", handler.LastRequest!.RequestUri!.AbsolutePath);
        Assert.Equal(string.Empty, handler.LastRequest.RequestUri.Query);
        Assert.Equal("resume-instruction", handler.LastRequest.Headers.GetValues("Last-Event-ID").Single());
        Assert.Contains(handler.LastRequest.Headers.Accept, static value => value.MediaType == "text/event-stream");
        Assert.Equal("text/event-stream", response.Content.Headers.ContentType?.MediaType);
    }

    [Fact]
    public async Task StreamExplorerInstructionsAsyncDeserializesInstructionPayloads()
    {
        using var handler = new RecordingHandler(_ =>
        {
            var response = new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("""
                    : keepalive

                    data: {"authority":"sorau456","created_at":"2026-03-29T04:05:06Z","kind":"SetKeyValue","box":{"encoded":"0x1234","json":{"kind":"SetKeyValue","payload":{"object":"domain","key":"flag","value":{"enabled":true}},"wire_id":"iroha.set_key_value","encoded":"1234"}},"transaction_hash":"tx-set-kv","transaction_status":"Committed","block":7,"index":2}

                    """),
            };
            response.Content.Headers.ContentType = new("text/event-stream");
            return response;
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var events = new List<ToriiExplorerInstruction>();

        await foreach (var instruction in client.StreamExplorerInstructionsAsync())
        {
            events.Add(instruction);
        }

        var typed = Assert.Single(events);
        Assert.Equal("sorau456", typed.Authority);
        Assert.Equal("2026-03-29T04:05:06Z", typed.CreatedAt);
        Assert.Equal("SetKeyValue", typed.Kind);
        Assert.Equal("0x1234", typed.InstructionBox.Encoded);
        Assert.NotNull(typed.InstructionBox.Json);
        Assert.Equal("SetKeyValue", typed.InstructionBox.Json!.Kind);
        Assert.Equal("iroha.set_key_value", typed.InstructionBox.Json.WireId);
        Assert.Equal("1234", typed.InstructionBox.Json.Encoded);
        Assert.Equal("domain", typed.InstructionBox.Json.Payload!["object"]!.GetValue<string>());
        Assert.Equal("flag", typed.InstructionBox.Json.Payload!["key"]!.GetValue<string>());
        Assert.True(typed.InstructionBox.Json.Payload!["value"]!["enabled"]!.GetValue<bool>());
        Assert.Equal("tx-set-kv", typed.TransactionHash);
        Assert.Equal("Committed", typed.TransactionStatus);
        Assert.Equal((ulong)7, typed.Block);
        Assert.Equal((uint)2, typed.Index);
    }

    [Fact]
    public async Task ResolveIdentifierAsyncPostsRequestAndDeserializesSignedReceipt()
    {
        using var handler = new RecordingHandler(request =>
        {
            var payload = ReadBodyAsJson(request);
            Assert.Equal("/v1/identifiers/resolve", request.RequestUri!.AbsolutePath);
            Assert.Equal("phone#retail", payload.RootElement.GetProperty("policy_id").GetString());
            Assert.Equal("+15551234567", payload.RootElement.GetProperty("input").GetString());

            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("""
                    {
                      "policy_id": "phone#retail",
                      "opaque_id": "opaque-1",
                      "receipt_hash": "receipt-1",
                      "uaid": "uaid:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                      "account_id": "sorauﾛ1Nmerchant",
                      "resolved_at_ms": 1710000000000,
                      "expires_at_ms": 1710003600000,
                      "backend": "bfv-programmed-sha3-256-v1",
                      "signature": "ABCD",
                      "signature_payload_hex": "DEADBEEF",
                      "signature_payload": {
                        "policy_id": "phone#retail",
                        "account_id": "sorauﾛ1Nmerchant"
                      }
                    }
                    """),
            };
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var resolved = await client.ResolveIdentifierAsync(new ToriiIdentifierResolveRequest
        {
            PolicyId = "phone#retail",
            Input = "+15551234567",
        });

        Assert.Equal("opaque-1", resolved.OpaqueId);
        Assert.Equal("receipt-1", resolved.ReceiptHash);
        Assert.Equal("sorauﾛ1Nmerchant", resolved.AccountId);
        Assert.Equal("bfv-programmed-sha3-256-v1", resolved.Backend);
        Assert.NotNull(resolved.SignaturePayload);
        Assert.Equal("phone#retail", resolved.SignaturePayload!["policy_id"]!.GetValue<string>());
    }

    [Theory]
    [InlineData("")]
    [InlineData(" phone#retail")]
    [InlineData("phone#retail ")]
    [InlineData("phone# retail")]
    [InlineData("phone")]
    [InlineData("phone#retail#extra")]
    [InlineData("phone#retail\u0001")]
    public async Task ResolveIdentifierAsyncRejectsNonExactPolicyIdBeforeDispatch(string policyId)
    {
        using var handler = new RecordingHandler(_ =>
            throw new InvalidOperationException("non-exact identifier policy id reached HTTP dispatch"));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        var error = await Assert.ThrowsAsync<ArgumentException>(() => client.ResolveIdentifierAsync(
            new ToriiIdentifierResolveRequest
            {
                PolicyId = policyId,
                Input = "+15551234567",
            }));

        Assert.Contains("identifier resolve request.policy_id", error.Message);
        Assert.Null(handler.LastRequest);
    }

    [Theory]
    [InlineData(" ABCD", "DEADBEEF", """{"policy_id":"phone#retail","account_id":"sorauﾛ1Nmerchant"}""", "identifier resolve response.signature")]
    [InlineData("ABCD", " DEADBEEF", """{"policy_id":"phone#retail","account_id":"sorauﾛ1Nmerchant"}""", "identifier resolve response.signature_payload_hex")]
    [InlineData("ABCD", "DEADBEEF", """{"opening":{"signature":" ABCD"}}""", "identifier resolve response.signature_payload.opening.signature")]
    [InlineData("ABCD", "DEADBEEF", """{"payload":{"opening":{"signature":"ABCD "}}}""", "identifier resolve response.signature_payload.payload.opening.signature")]
    [InlineData("ABCD", "DEADBEEF", """{"attestation":{"signature":" ABCD"}}""", "identifier resolve response.signature_payload.attestation.signature")]
    public async Task ResolveIdentifierAsyncRejectsPaddedSignatureReceiptFields(
        string signature,
        string signaturePayloadHex,
        string signaturePayloadJson,
        string expectedField)
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent(IdentifierResolveResponseJson(
                signature,
                signaturePayloadHex,
                signaturePayloadJson)),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var error = await Assert.ThrowsAsync<JsonException>(() => client.ResolveIdentifierAsync(
            new ToriiIdentifierResolveRequest
            {
                PolicyId = "phone#retail",
                Input = "+15551234567",
            }));

        Assert.Contains(expectedField, error.Message);
        Assert.Contains("whitespace", error.Message);
    }

    [Theory]
    [InlineData(" phone#retail", "identifier resolve response.policy_id", "whitespace")]
    [InlineData("phone#retail ", "identifier resolve response.policy_id", "whitespace")]
    [InlineData("phone# retail", "identifier resolve response.policy_id.rule", "whitespace")]
    [InlineData("phone", "identifier resolve response.policy_id", "kind#rule")]
    [InlineData("phone#retail#extra", "identifier resolve response.policy_id", "kind#rule")]
    [InlineData("phone#retail\u0001", "identifier resolve response.policy_id", "control")]
    public async Task ResolveIdentifierAsyncRejectsNonExactTopLevelPolicyId(
        string policyId,
        string expectedField,
        string expectedReason)
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent(IdentifierResolveResponseJson(
                "ABCD",
                "DEADBEEF",
                """{"policy_id":"phone#retail","account_id":"sorauﾛ1Nmerchant"}""",
                policyId)),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var error = await Assert.ThrowsAsync<JsonException>(() => client.ResolveIdentifierAsync(
            new ToriiIdentifierResolveRequest
            {
                PolicyId = "phone#retail",
                Input = "+15551234567",
            }));

        Assert.Contains(expectedField, error.Message);
        Assert.Contains(expectedReason, error.Message);
    }

    [Theory]
    [InlineData("""{"policy_id":" phone#retail","account_id":"sorauﾛ1Nmerchant"}""", "identifier resolve response.signature_payload.policy_id", "whitespace")]
    [InlineData("""{"policy_id":"phone#retail ","account_id":"sorauﾛ1Nmerchant"}""", "identifier resolve response.signature_payload.policy_id", "whitespace")]
    [InlineData("""{"policy_id":"phone# retail","account_id":"sorauﾛ1Nmerchant"}""", "identifier resolve response.signature_payload.policy_id.rule", "whitespace")]
    [InlineData("""{"policy_id":"phone","account_id":"sorauﾛ1Nmerchant"}""", "identifier resolve response.signature_payload.policy_id", "kind#rule")]
    [InlineData("""{"policy_id":"phone#retail#extra","account_id":"sorauﾛ1Nmerchant"}""", "identifier resolve response.signature_payload.policy_id", "kind#rule")]
    [InlineData("""{"policy_id":"phone#retail\u0001","account_id":"sorauﾛ1Nmerchant"}""", "identifier resolve response.signature_payload.policy_id", "control")]
    [InlineData("""{"payload":{"policy_id":" phone#retail"}}""", "identifier resolve response.signature_payload.payload.policy_id", "whitespace")]
    [InlineData("""{"payload":{"policy_id":"phone#retail "}}""", "identifier resolve response.signature_payload.payload.policy_id", "whitespace")]
    [InlineData("""{"payload":{"policy_id":"phone# retail"}}""", "identifier resolve response.signature_payload.payload.policy_id.rule", "whitespace")]
    [InlineData("""{"payload":{"policy_id":"phone"}}""", "identifier resolve response.signature_payload.payload.policy_id", "kind#rule")]
    [InlineData("""{"payload":{"policy_id":"phone#retail#extra"}}""", "identifier resolve response.signature_payload.payload.policy_id", "kind#rule")]
    [InlineData("""{"payload":{"policy_id":"phone#retail\u0001"}}""", "identifier resolve response.signature_payload.payload.policy_id", "control")]
    public async Task ResolveIdentifierAsyncRejectsNonExactSignaturePayloadPolicyIds(
        string signaturePayloadJson,
        string expectedField,
        string expectedReason)
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent(IdentifierResolveResponseJson(
                "ABCD",
                "DEADBEEF",
                signaturePayloadJson)),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var error = await Assert.ThrowsAsync<JsonException>(() => client.ResolveIdentifierAsync(
            new ToriiIdentifierResolveRequest
            {
                PolicyId = "phone#retail",
                Input = "+15551234567",
            }));

        Assert.Contains(expectedField, error.Message);
        Assert.Contains(expectedReason, error.Message);
    }

    [Fact]
    public async Task ResolveIdentifierAsyncAcceptsExactProofAttestationReceipt()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent(IdentifierResolveResponseJson(
                "ABCD",
                "DEADBEEF",
                """
                {
                  "policy_id": "phone#retail",
                  "attestation": {
                    "kind": "proof",
                    "proof_backend": "halo2/ipa",
                    "proof_b64": "AQID"
                  }
                }
                """)),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var resolved = await client.ResolveIdentifierAsync(new ToriiIdentifierResolveRequest
        {
            PolicyId = "phone#retail",
            Input = "+15551234567",
        });

        Assert.Equal("proof", resolved.SignaturePayload!["attestation"]!["kind"]!.GetValue<string>());
    }

    [Theory]
    [InlineData("""{"attestation":{"kind":" signed","signature":"ABCD"}}""", "identifier resolve response.signature_payload.attestation.kind", "whitespace")]
    [InlineData("""{"attestation":{"kind":"signed ","signature":"ABCD"}}""", "identifier resolve response.signature_payload.attestation.kind", "whitespace")]
    [InlineData("""{"attestation":{"kind":"Signed","signature":"ABCD"}}""", "identifier resolve response.signature_payload.attestation.kind", "signed or proof")]
    [InlineData("""{"attestation":{"kind":"signed","signature":"ABCD","proof_b64":"AQID"}}""", "identifier resolve response.signature_payload.attestation signed attestations", "proof fields")]
    [InlineData("""{"attestation":{"proof_b64":"AQID"}}""", "identifier resolve response.signature_payload.attestation.kind", "required")]
    [InlineData("""{"attestation":{"kind":"proof","proof_backend":" halo2/ipa","proof_b64":"AQID"}}""", "identifier resolve response.signature_payload.attestation.proof_backend", "whitespace")]
    [InlineData("""{"attestation":{"kind":"proof","proof_backend":"halo2/ipa","proof_b64":" AQID"}}""", "identifier resolve response.signature_payload.attestation.proof_b64", "whitespace")]
    [InlineData("""{"attestation":{"kind":"proof","proof_backend":"halo2/ipa","proof_b64":"AQID "}}""", "identifier resolve response.signature_payload.attestation.proof_b64", "whitespace")]
    [InlineData("""{"attestation":{"kind":"proof","proof_backend":"halo2/ipa","proof_b64":"@@@"}}""", "identifier resolve response.signature_payload.attestation.proof_b64", "valid base64")]
    public async Task ResolveIdentifierAsyncRejectsNonExactAttestationSelectors(
        string signaturePayloadJson,
        string expectedField,
        string expectedReason)
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent(IdentifierResolveResponseJson(
                "ABCD",
                "DEADBEEF",
                signaturePayloadJson)),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var error = await Assert.ThrowsAsync<JsonException>(() => client.ResolveIdentifierAsync(
            new ToriiIdentifierResolveRequest
            {
                PolicyId = "phone#retail",
                Input = "+15551234567",
            }));

        Assert.Contains(expectedField, error.Message);
        Assert.Contains(expectedReason, error.Message);
    }

    [Fact]
    public async Task ResolveIdentifierAsyncAcceptsExactNestedReceiptPayloadFields()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent(IdentifierResolveResponseJson(
                "ABCD",
                "DEADBEEF",
                """
                {
                  "payload": {
                    "policy_id": "phone#retail",
                    "account_id": "sorauﾛ1Nmerchant",
                    "opaque_id": "opaque-1",
                    "receipt_hash": "receipt-1",
                    "uaid": "uaid:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                    "execution": {
                      "program_id": "identifier_lookup_retail",
                      "program_digest": "program-digest",
                      "backend": "hkdf-sha3-512-prf-v1",
                      "verification_mode": "signed",
                      "input_ciphertext_hash": "input-hash",
                      "output_ciphertext_hash": "output-hash",
                      "parameter_digest": "parameter-digest",
                      "evaluation_key_digest": "evaluation-key-digest",
                      "output_hash": "output-open-hash",
                      "associated_data_hash": "associated-data-hash",
                      "executed_at_ms": 1710000000000,
                      "expires_at_ms": "1710003600000"
                    },
                    "opening": {
                      "payload": {
                        "program_id": "identifier_lookup_retail",
                        "input_ciphertext_hash": "input-hash",
                        "output_ciphertext_hash": "output-hash",
                        "parameter_digest": "parameter-digest",
                        "evaluation_key_digest": "evaluation-key-digest",
                        "opened_output_hash": "opened-output-hash",
                        "opened_at_ms": 1710000000000,
                        "expires_at_ms": "1710003600000"
                      },
                      "signature": "ABCD"
                    }
                  }
                }
                """)),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var resolved = await client.ResolveIdentifierAsync(new ToriiIdentifierResolveRequest
        {
            PolicyId = "phone#retail",
            Input = "+15551234567",
        });

        Assert.Equal(
            "identifier_lookup_retail",
            resolved.SignaturePayload!["payload"]!["execution"]!["program_id"]!.GetValue<string>());
    }

    [Theory]
    [InlineData("""{"payload":{"account_id":" sorauﾛ1Nmerchant"}}""", "identifier resolve response.signature_payload.payload.account_id", "whitespace")]
    [InlineData("""{"payload":{"account_id":"sorauﾛ1Nmerchant\u0001"}}""", "identifier resolve response.signature_payload.payload.account_id", "control")]
    [InlineData("""{"payload":{"opaque_id":" opaque-1"}}""", "identifier resolve response.signature_payload.payload.opaque_id", "whitespace")]
    [InlineData("""{"payload":{"receipt_hash":"receipt-1 "}}""", "identifier resolve response.signature_payload.payload.receipt_hash", "whitespace")]
    [InlineData("""{"payload":{"uaid":" uaid:0123456789abcdef"}}""", "identifier resolve response.signature_payload.payload.uaid", "whitespace")]
    [InlineData("""{"payload":{"execution":{"program_id":" identifier_lookup_retail"}}}""", "identifier resolve response.signature_payload.payload.execution.program_id", "whitespace")]
    [InlineData("""{"payload":{"execution":{"program_digest":"program-digest "}}}""", "identifier resolve response.signature_payload.payload.execution.program_digest", "whitespace")]
    [InlineData("""{"payload":{"execution":{"backend":" hkdf-sha3-512-prf-v1"}}}""", "identifier resolve response.signature_payload.payload.execution.backend", "whitespace")]
    [InlineData("""{"payload":{"execution":{"verification_mode":"signed "}}}""", "identifier resolve response.signature_payload.payload.execution.verification_mode", "whitespace")]
    [InlineData("""{"payload":{"execution":{"input_ciphertext_hash":" input-hash"}}}""", "identifier resolve response.signature_payload.payload.execution.input_ciphertext_hash", "whitespace")]
    [InlineData("""{"payload":{"execution":{"output_ciphertext_hash":"output-hash "}}}""", "identifier resolve response.signature_payload.payload.execution.output_ciphertext_hash", "whitespace")]
    [InlineData("""{"payload":{"execution":{"parameter_digest":" parameter-digest"}}}""", "identifier resolve response.signature_payload.payload.execution.parameter_digest", "whitespace")]
    [InlineData("""{"payload":{"execution":{"evaluation_key_digest":"evaluation-key-digest "}}}""", "identifier resolve response.signature_payload.payload.execution.evaluation_key_digest", "whitespace")]
    [InlineData("""{"payload":{"execution":{"output_hash":" output-open-hash"}}}""", "identifier resolve response.signature_payload.payload.execution.output_hash", "whitespace")]
    [InlineData("""{"payload":{"execution":{"associated_data_hash":"associated-data-hash "}}}""", "identifier resolve response.signature_payload.payload.execution.associated_data_hash", "whitespace")]
    [InlineData("""{"payload":{"execution":{"executed_at_ms":" 1710000000000"}}}""", "identifier resolve response.signature_payload.payload.execution.executed_at_ms", "whitespace")]
    [InlineData("""{"payload":{"execution":{"expires_at_ms":-1}}}""", "identifier resolve response.signature_payload.payload.execution.expires_at_ms", "non-negative")]
    [InlineData("""{"payload":{"opening":{"payload":{"program_id":"identifier_lookup_retail "}}}}""", "identifier resolve response.signature_payload.payload.opening.payload.program_id", "whitespace")]
    [InlineData("""{"payload":{"opening":{"payload":{"input_ciphertext_hash":" input-hash"}}}}""", "identifier resolve response.signature_payload.payload.opening.payload.input_ciphertext_hash", "whitespace")]
    [InlineData("""{"payload":{"opening":{"payload":{"output_ciphertext_hash":"output-hash "}}}}""", "identifier resolve response.signature_payload.payload.opening.payload.output_ciphertext_hash", "whitespace")]
    [InlineData("""{"payload":{"opening":{"payload":{"parameter_digest":" parameter-digest"}}}}""", "identifier resolve response.signature_payload.payload.opening.payload.parameter_digest", "whitespace")]
    [InlineData("""{"payload":{"opening":{"payload":{"evaluation_key_digest":"evaluation-key-digest "}}}}""", "identifier resolve response.signature_payload.payload.opening.payload.evaluation_key_digest", "whitespace")]
    [InlineData("""{"payload":{"opening":{"payload":{"opened_output_hash":" opened-output-hash"}}}}""", "identifier resolve response.signature_payload.payload.opening.payload.opened_output_hash", "whitespace")]
    [InlineData("""{"payload":{"opening":{"payload":{"opened_at_ms":"1710000000000 "}}}}""", "identifier resolve response.signature_payload.payload.opening.payload.opened_at_ms", "whitespace")]
    [InlineData("""{"payload":{"opening":{"payload":{"expires_at_ms":"-1"}}}}""", "identifier resolve response.signature_payload.payload.opening.payload.expires_at_ms", "non-negative")]
    public async Task ResolveIdentifierAsyncRejectsNonExactNestedReceiptPayloadFields(
        string signaturePayloadJson,
        string expectedField,
        string expectedReason)
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent(IdentifierResolveResponseJson(
                "ABCD",
                "DEADBEEF",
                signaturePayloadJson)),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var error = await Assert.ThrowsAsync<JsonException>(() => client.ResolveIdentifierAsync(
            new ToriiIdentifierResolveRequest
            {
                PolicyId = "phone#retail",
                Input = "+15551234567",
            }));

        Assert.Contains(expectedField, error.Message);
        Assert.Contains(expectedReason, error.Message);
    }

    [Fact]
    public async Task ResolveAccountAliasIndexAsyncReturnsTypedResponse()
    {
        using var handler = new RecordingHandler(request =>
        {
            var payload = ReadBodyAsJson(request);
            Assert.Equal<ulong>(7, payload.RootElement.GetProperty("index").GetUInt64());

            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("""
                    {
                      "index": 7,
                      "alias": "merchant@paynet.universal",
                      "account_id": "sorauﾛ1Nmerchant",
                      "source": "on_chain"
                    }
                    """),
            };
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var resolved = await client.ResolveAccountAliasIndexAsync(7);

        Assert.NotNull(resolved);
        Assert.Equal((ulong)7, resolved!.Index);
        Assert.Equal("merchant@paynet.universal", resolved.Alias);
        Assert.Equal("sorauﾛ1Nmerchant", resolved.AccountId);
        Assert.Equal("on_chain", resolved.Source);
    }

    [Fact]
    public async Task ResolveAccountAliasIndexAsyncReturnsNullOnNotFound()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.NotFound));

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var resolved = await client.ResolveAccountAliasIndexAsync(0);

        Assert.Null(resolved);
    }

    [Fact]
    public async Task GetUaidPortfolioAsyncNormalizesLiteralAndAddsAssetIdQuery()
    {
        const string uaidHex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "uaid": "uaid:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                  "totals": { "accounts": 2, "positions": 3 },
                  "dataspaces": [
                    {
                      "dataspace_id": 0,
                      "dataspace_alias": "universal",
                      "accounts": [
                        {
                          "account_id": "sorauロ1Nholder",
                          "label": null,
                          "assets": [
                            {
                              "asset_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                              "asset_definition_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                              "quantity": "500"
                            }
                          ]
                        }
                      ]
                    }
                  ]
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var response = await client.GetUaidPortfolioAsync(
            $"  UAID:{uaidHex.ToUpperInvariant()}  ",
            new ToriiUaidPortfolioQuery { AssetId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM" });

        Assert.Equal($"uaid:{uaidHex}", response.Uaid);
        Assert.Equal(2, response.Totals.Accounts);
        Assert.Equal("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", response.Dataspaces[0].Accounts[0].Assets[0].AssetId);
        Assert.Contains("/v1/accounts/uaid%3A0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef/portfolio", handler.LastRequest!.RequestUri!.AbsoluteUri);
        Assert.Equal("asset_id=62Fk4FPcMuLvW5QjDGNF2a4jAmjM", handler.LastRequest.RequestUri.Query.TrimStart('?'));
    }

    [Fact]
    public async Task GetUaidBindingsAsyncNormalizesLiteralAndDeserializesDataspaces()
    {
        const string uaidHex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "uaid": "uaid:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                  "dataspaces": [
                    {
                      "dataspace_id": 42,
                      "dataspace_alias": "payments",
                      "accounts": ["sorauﾛ1Nmerchant", "sorauロ1Nissuer"]
                    }
                  ]
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var response = await client.GetUaidBindingsAsync($"  {uaidHex.ToUpperInvariant()}  ");

        Assert.Equal($"uaid:{uaidHex}", response.Uaid);
        Assert.Single(response.Dataspaces);
        Assert.Equal(42, response.Dataspaces[0].DataspaceId);
        Assert.Equal("payments", response.Dataspaces[0].DataspaceAlias);
        Assert.Equal("sorauﾛ1Nmerchant", response.Dataspaces[0].Accounts[0]);
        Assert.Contains("/v1/space-directory/uaids/uaid%3A0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef", handler.LastRequest!.RequestUri!.AbsoluteUri);
    }

    [Fact]
    public async Task GetUaidManifestsAsyncAddsQueryAndPreservesManifestPayload()
    {
        const string uaidHex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "uaid": "uaid:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                  "total": 1,
                  "manifests": [
                    {
                      "dataspace_id": 42,
                      "dataspace_alias": "payments",
                      "manifest_hash": "deadbeef",
                      "status": "Active",
                      "lifecycle": {
                        "activated_epoch": 121,
                        "expired_epoch": 240,
                        "revocation": null
                      },
                      "accounts": ["sorauﾛ1Nmerchant"],
                      "manifest": {
                        "version": "V1",
                        "uaid": "uaid:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                        "dataspace": 42,
                        "issued_ms": 1710000000000,
                        "activation_epoch": 120,
                        "expiry_epoch": 240,
                        "entries": []
                      }
                    }
                  ]
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var response = await client.GetUaidManifestsAsync(
            $"UAID:{uaidHex.ToUpperInvariant()}",
            new ToriiUaidManifestQuery
            {
                DataspaceId = 42,
                Status = ToriiUaidManifestStatusFilter.Inactive,
                Limit = 5,
                Offset = 2,
            });

        Assert.Equal($"uaid:{uaidHex}", response.Uaid);
        Assert.Equal(1, response.Total);
        Assert.Single(response.Manifests);
        Assert.Equal("deadbeef", response.Manifests[0].ManifestHash);
        Assert.Equal("Active", response.Manifests[0].Status);
        Assert.Equal(121, response.Manifests[0].Lifecycle.ActivatedEpoch);
        Assert.NotNull(response.Manifests[0].Manifest);
        Assert.Equal(42, response.Manifests[0].Manifest!["dataspace"]!.GetValue<int>());
        Assert.Contains("/v1/space-directory/uaids/uaid%3A0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef/manifests", handler.LastRequest!.RequestUri!.AbsoluteUri);
        Assert.Equal("limit=5&offset=2&dataspace=42&status=inactive", handler.LastRequest.RequestUri.Query.TrimStart('?'));
    }

    [Fact]
    public async Task SubmitTransactionAsyncPostsNoritoPayload()
    {
        var transaction = new TransactionBuilder("00000042", "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")
            .TransferAsset("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", "15.7500", "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")
            .SetCreationTimeMilliseconds(1736000000000)
            .SetTimeToLiveMilliseconds(3500)
            .SetNonce(17)
            .BuildSigned(Convert.FromHexString("616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032"));

        using var handler = new RecordingHandler(request =>
        {
            Assert.Equal(HttpMethod.Post, request.Method);
            Assert.Equal("/transaction", request.RequestUri!.AbsolutePath);
            Assert.Equal("application/x-norito", request.Content!.Headers.ContentType!.MediaType);
            using var stream = request.Content.ReadAsStream();
            using var buffer = new MemoryStream();
            stream.CopyTo(buffer);
            Assert.Equal(transaction.NoritoBytes, buffer.ToArray());

            return new HttpResponseMessage(HttpStatusCode.Accepted)
            {
                Content = new ByteArrayContent(Array.Empty<byte>()),
            };
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        await client.SubmitTransactionAsync(transaction);
    }

    [Fact]
    public async Task GetPipelineTransactionStatusAsyncParsesEnvelopePayload()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "kind": "Transaction",
                  "content": {
                    "hash": "da01f3a369d10e6ad78f241c86f4fe2d5481ff13ace97e6fb5db5c30240bdb3b",
                    "status": {
                      "kind": "Applied",
                      "block_height": 9,
                      "content": null
                    },
                    "scope": "auto",
                    "resolved_from": "state"
                  }
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var status = await client.GetPipelineTransactionStatusAsync("da01f3a369d10e6ad78f241c86f4fe2d5481ff13ace97e6fb5db5c30240bdb3b");

        Assert.NotNull(status);
        Assert.Equal(PipelineTransactionState.Applied, status!.State);
        Assert.Equal((ulong)9, status.BlockHeight);
        Assert.Equal("auto", status.Scope);
        Assert.Equal("state", status.ResolvedFrom);
    }

    [Fact]
    public async Task RegisterAccountAsyncPostsJsonAndDeserializesQueuedResponse()
    {
        using var handler = new RecordingHandler(request =>
        {
            var payload = ReadBodyAsJson(request);
            Assert.Equal("/v1/accounts/onboard", request.RequestUri!.AbsolutePath);
            Assert.Equal(HttpMethod.Post, request.Method);
            Assert.Equal("merchant@paynet", payload.RootElement.GetProperty("alias").GetString());
            Assert.Equal("sorauﾛ1Nmerchant", payload.RootElement.GetProperty("account_id").GetString());
            Assert.Equal("merchant@example.com", payload.RootElement.GetProperty("identity").GetProperty("email").GetString());

            return new HttpResponseMessage(HttpStatusCode.Accepted)
            {
                Content = new StringContent("""
                    {
                      "account_id": "sorauﾛ1Nmerchant",
                      "uaid": "uaid:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                      "tx_hash_hex": "deadbeef",
                      "status": "QUEUED"
                    }
                    """),
            };
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var response = await client.RegisterAccountAsync(new ToriiAccountOnboardingRequest
        {
            Alias = "merchant@paynet",
            AccountId = "sorauﾛ1Nmerchant",
            Identity = new JsonObject
            {
                ["email"] = "merchant@example.com",
            },
            Permissions = ["CanResolveAccountAlias"],
        });

        Assert.Equal("sorauﾛ1Nmerchant", response.AccountId);
        Assert.Equal("deadbeef", response.TransactionHashHex);
        Assert.Equal("QUEUED", response.Status);
    }

    [Fact]
    public async Task GetAccountFaucetPuzzleAsyncDeserializesTypedResponse()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "algorithm": "scrypt-leading-zero-bits-v1",
                  "difficulty_bits": 10,
                  "anchor_height": 68,
                  "anchor_block_hash_hex": "d5c0016a6345e8ea379da42aab1fdc16ba82756e19e0b63c48c14735e8caf7ef",
                  "challenge_salt_hex": null,
                  "scrypt_log_n": 13,
                  "scrypt_r": 8,
                  "scrypt_p": 1,
                  "max_anchor_age_blocks": 6
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var puzzle = await client.GetAccountFaucetPuzzleAsync();

        Assert.Equal("scrypt-leading-zero-bits-v1", puzzle.Algorithm);
        Assert.Equal((byte)10, puzzle.DifficultyBits);
        Assert.Equal((ulong)68, puzzle.AnchorHeight);
        Assert.Equal((uint)8, puzzle.ScryptR);
        Assert.Equal("/v1/accounts/faucet/puzzle", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public void FaucetPowComputeChallengeMatchesDeterministicVector()
    {
        var challenge = ToriiAccountFaucetPow.ComputeChallenge(
            "sorauﾛ1Nmerchant",
            68,
            "d5c0016a6345e8ea379da42aab1fdc16ba82756e19e0b63c48c14735e8caf7ef");

        Assert.Equal(
            "0ea18a1c23d3d8d323fed2ebbcb5a5372d96c81a3383c138dbfbe7c6562a8f81",
            Convert.ToHexString(challenge).ToLowerInvariant());
    }

    [Fact]
    public void FaucetPowComputeDigestMatchesManagedScryptVector()
    {
        var challenge = Convert.FromHexString("8fedfb3e73b08653203dfedc046fe38e523503453d0efb639cfa0e9870550adf");
        var digest = ToriiAccountFaucetPow.ComputeDigest(
            Convert.FromHexString("0000000000000001"),
            challenge,
            scryptLogN: 4,
            scryptR: 1,
            scryptP: 1);

        Assert.Equal(
            "d9dd0907aba2a70b6bdf9b5a9f5b4ef621397e5f637190e80848384b0ac1745c",
            Convert.ToHexString(digest).ToLowerInvariant());
    }

    [Fact]
    public void FaucetPowSolveFindsExpectedNonceForDeterministicPuzzle()
    {
        var solution = ToriiAccountFaucetPow.Solve(
            "sorauﾛ1Nmerchant",
            new ToriiAccountFaucetPuzzle
            {
                Algorithm = "scrypt-leading-zero-bits-v1",
                DifficultyBits = 8,
                AnchorHeight = 68,
                AnchorBlockHashHex = "d5c0016a6345e8ea379da42aab1fdc16ba82756e19e0b63c48c14735e8caf7ef",
                ScryptLogN = 4,
                ScryptR = 1,
                ScryptP = 1,
                MaxAnchorAgeBlocks = 6,
            },
            new ToriiAccountFaucetSolveOptions
            {
                MaxAttempts = 200,
            });

        Assert.Equal("00000000000000c0", solution.NonceHex);
        Assert.Equal("005bbe67022aa322d672010caed4df8de6eca7c3204139bcb8ef409883ab2dbb", solution.DigestHex);
        Assert.Equal(9, solution.LeadingZeroBits);
        Assert.Equal(193, solution.Attempts);
    }

    [Fact]
    public async Task ClaimAccountFaucetAsyncPostsPowFieldsAndDeserializesQueuedResponse()
    {
        using var handler = new RecordingHandler(request =>
        {
            var payload = ReadBodyAsJson(request);
            Assert.Equal("/v1/accounts/faucet", request.RequestUri!.AbsolutePath);
            Assert.Equal("sorauﾛ1Nmerchant", payload.RootElement.GetProperty("account_id").GetString());
            Assert.Equal<ulong>(68, payload.RootElement.GetProperty("pow_anchor_height").GetUInt64());
            Assert.Equal("abcdef", payload.RootElement.GetProperty("pow_nonce_hex").GetString());

            return new HttpResponseMessage(HttpStatusCode.Accepted)
            {
                Content = new StringContent("""
                    {
                      "account_id": "sorauﾛ1Nmerchant",
                      "asset_definition_id": "rose#wonderland",
                      "asset_id": "rose#wonderland#sorauﾛ1Nmerchant",
                      "amount": "100",
                      "tx_hash_hex": "feedface",
                      "status": "QUEUED"
                    }
                    """),
            };
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var response = await client.ClaimAccountFaucetAsync(new ToriiAccountFaucetRequest
        {
            AccountId = "sorauﾛ1Nmerchant",
            PowAnchorHeight = 68,
            PowNonceHex = "abcdef",
        });

        Assert.Equal("rose#wonderland", response.AssetDefinitionId);
        Assert.Equal("100", response.Amount);
        Assert.Equal("feedface", response.TransactionHashHex);
        Assert.Equal("QUEUED", response.Status);
    }

    [Fact]
    public async Task SolveAccountFaucetAsyncFetchesPuzzleAndReturnsPreparedSolution()
    {
        using var handler = new RecordingHandler(request =>
        {
            Assert.Equal(HttpMethod.Get, request.Method);
            Assert.Equal("/v1/accounts/faucet/puzzle", request.RequestUri!.AbsolutePath);

            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("""
                    {
                      "algorithm": "scrypt-leading-zero-bits-v1",
                      "difficulty_bits": 8,
                      "anchor_height": 68,
                      "anchor_block_hash_hex": "d5c0016a6345e8ea379da42aab1fdc16ba82756e19e0b63c48c14735e8caf7ef",
                      "challenge_salt_hex": null,
                      "scrypt_log_n": 4,
                      "scrypt_r": 1,
                      "scrypt_p": 1,
                      "max_anchor_age_blocks": 6
                    }
                    """),
            };
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var solution = await client.SolveAccountFaucetAsync("sorauﾛ1Nmerchant", new ToriiAccountFaucetSolveOptions { MaxAttempts = 200 });

        Assert.Equal("00000000000000c0", solution.NonceHex);
        Assert.Equal("sorauﾛ1Nmerchant", solution.AccountId);
        Assert.Equal((ulong)68, solution.AnchorHeight);
    }

    [Fact]
    public async Task ClaimAccountFaucetAsyncWithAccountIdSolvesPuzzleBeforePosting()
    {
        var requestCount = 0;
        using var handler = new RecordingHandler(request =>
        {
            requestCount++;
            if (request.Method == HttpMethod.Get)
            {
                Assert.Equal("/v1/accounts/faucet/puzzle", request.RequestUri!.AbsolutePath);
                return new HttpResponseMessage(HttpStatusCode.OK)
                {
                    Content = new StringContent("""
                        {
                          "algorithm": "scrypt-leading-zero-bits-v1",
                          "difficulty_bits": 8,
                          "anchor_height": 68,
                          "anchor_block_hash_hex": "d5c0016a6345e8ea379da42aab1fdc16ba82756e19e0b63c48c14735e8caf7ef",
                          "challenge_salt_hex": null,
                          "scrypt_log_n": 4,
                          "scrypt_r": 1,
                          "scrypt_p": 1,
                          "max_anchor_age_blocks": 6
                        }
                        """),
                };
            }

            var payload = ReadBodyAsJson(request);
            Assert.Equal("/v1/accounts/faucet", request.RequestUri!.AbsolutePath);
            Assert.Equal("sorauﾛ1Nmerchant", payload.RootElement.GetProperty("account_id").GetString());
            Assert.Equal("00000000000000c0", payload.RootElement.GetProperty("pow_nonce_hex").GetString());
            Assert.Equal<ulong>(68, payload.RootElement.GetProperty("pow_anchor_height").GetUInt64());

            return new HttpResponseMessage(HttpStatusCode.Accepted)
            {
                Content = new StringContent("""
                    {
                      "account_id": "sorauﾛ1Nmerchant",
                      "asset_definition_id": "rose#wonderland",
                      "asset_id": "rose#wonderland#sorauﾛ1Nmerchant",
                      "amount": "100",
                      "tx_hash_hex": "feedface",
                      "status": "QUEUED"
                    }
                    """),
            };
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var response = await client.ClaimAccountFaucetAsync("sorauﾛ1Nmerchant", new ToriiAccountFaucetSolveOptions { MaxAttempts = 200 });

        Assert.Equal(2, requestCount);
        Assert.Equal("feedface", response.TransactionHashHex);
        Assert.Equal("QUEUED", response.Status);
    }

    [Fact]
    public async Task RegisterMultisigAccountAsyncPostsMembersAndAcceptsExistsResponse()
    {
        using var handler = new RecordingHandler(request =>
        {
            var payload = ReadBodyAsJson(request);
            Assert.Equal("/v1/accounts/onboard/multisig", request.RequestUri!.AbsolutePath);
            Assert.Equal("treasury@paynet", payload.RootElement.GetProperty("alias").GetString());
            Assert.Equal(2, payload.RootElement.GetProperty("required_signers").GetInt32());
            Assert.Equal(2, payload.RootElement.GetProperty("member_weights")[1].GetInt32());

            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("""
                    {
                      "account_id": "sorauﾛ1Nmultisig",
                      "tx_hash_hex": "",
                      "status": "EXISTS"
                    }
                    """),
            };
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var response = await client.RegisterMultisigAccountAsync(new ToriiMultisigAccountOnboardingRequest
        {
            Alias = "treasury@paynet",
            RequiredSigners = 2,
            MemberAccountIds = ["sorauロ1Nmember1", "sorauロ1Nmember2"],
            MemberWeights = [1, 2],
            TransactionTtlMilliseconds = 60_000,
        });

        Assert.Equal("sorauﾛ1Nmultisig", response.AccountId);
        Assert.Equal(string.Empty, response.TransactionHashHex);
        Assert.Equal("EXISTS", response.Status);
    }

    [Fact]
    public async Task ResolveAccountAliasAsyncReturnsTypedResponse()
    {
        using var handler = new RecordingHandler(request =>
        {
            var payload = ReadBodyAsJson(request);
            Assert.Equal("merchant@paynet.universal", payload.RootElement.GetProperty("alias").GetString());

            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("""
                    {
                      "alias": "merchant@paynet.universal",
                      "account_id": "sorauﾛ1Nmerchant",
                      "index": 7,
                      "source": "on_chain"
                    }
                    """),
            };
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var resolved = await client.ResolveAccountAliasAsync("merchant@paynet.universal");

        Assert.NotNull(resolved);
        Assert.Equal("merchant@paynet.universal", resolved!.Alias);
        Assert.Equal("sorauﾛ1Nmerchant", resolved.AccountId);
        Assert.Equal((long)7, resolved.Index);
        Assert.Equal("on_chain", resolved.Source);
    }

    [Fact]
    public async Task ResolveAssetAliasAsyncReturnsTypedResponse()
    {
        using var handler = new RecordingHandler(request =>
        {
            var payload = ReadBodyAsJson(request);
            Assert.Equal("usd#issuer.main", payload.RootElement.GetProperty("alias").GetString());

            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("""
                    {
                      "alias": "usd#issuer.main",
                      "asset_definition_id": "66owaQmAQMuHxPzxUN3bqZ6FJfDa",
                      "asset_name": "USD",
                      "description": "United States Dollar",
                      "logo": "sorafs://logos/usd.png",
                      "source": "world_state",
                      "alias_binding": {
                        "alias": "usd#issuer.main",
                        "status": "permanent",
                        "bound_at_ms": 1
                      }
                    }
                    """),
            };
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var resolved = await client.ResolveAssetAliasAsync("usd#issuer.main");

        Assert.NotNull(resolved);
        Assert.Equal("66owaQmAQMuHxPzxUN3bqZ6FJfDa", resolved!.AssetDefinitionId);
        Assert.Equal("USD", resolved.AssetName);
        Assert.Equal("permanent", resolved.AliasBinding!.Status);
    }

    [Fact]
    public async Task ResolveContractAliasAsyncReturnsTypedResponse()
    {
        using var handler = new RecordingHandler(request =>
        {
            var payload = ReadBodyAsJson(request);
            Assert.Equal("router::dex.universal", payload.RootElement.GetProperty("contract_alias").GetString());

            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("""
                    {
                      "contract_alias": "router::dex.universal",
                      "contract_address": "iroha1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq",
                      "dataspace": "universal",
                      "source": "world_state",
                      "contract_alias_binding": {
                        "alias": "router::dex.universal",
                        "status": "permanent",
                        "bound_at_ms": 1
                      }
                    }
                    """),
            };
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var resolved = await client.ResolveContractAliasAsync("router::dex.universal");

        Assert.NotNull(resolved);
        Assert.Equal("router::dex.universal", resolved!.ContractAlias);
        Assert.Equal("universal", resolved.Dataspace);
        Assert.Equal("permanent", resolved.ContractAliasBinding!.Status);
        Assert.Equal("world_state", resolved.Source);
    }

    [Fact]
    public async Task GetContractCodeAsyncEncodesCodeHashAndDeserializesManifest()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "manifest": {
                    "code_hash": "0011aa",
                    "abi_hash": "99ff"
                  }
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var record = await client.GetContractCodeAsync(" 0011aa ");

        Assert.Equal("0011aa", record.Manifest.CodeHash);
        Assert.Equal("99ff", record.Manifest.AbiHash);
        Assert.Equal("/v1/contracts/code/0011aa", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task DeployContractAsyncEncodesRouteAndDeserializesResponse()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "ok": true,
                  "contract_address": "iroha1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq",
                  "dataspace": "universal",
                  "deploy_nonce": 4,
                  "code_hash_hex": "aa55",
                  "abi_hash_hex": "bb66"
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var response = await client.DeployContractAsync(new ToriiDeployContractRequest
        {
            Authority = "sorauロ1Ncaller",
            PrivateKey = "ed0120AABB",
            CodeBase64 = "AQID",
        });

        Assert.True(response.Ok);
        Assert.Equal("universal", response.Dataspace);
        Assert.Equal((ulong)4, response.DeployNonce);
        Assert.Equal("/v1/contracts/deploy", handler.LastRequest!.RequestUri!.AbsolutePath);
        Assert.Equal(HttpMethod.Post, handler.LastRequest.Method);
    }

    [Fact]
    public async Task GetContractCodeBytesAsyncEncodesCodeHashAndDecodesBase64()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "code_b64": "AQIDBA=="
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var bytes = await client.GetContractCodeBytesAsync(" 0011aa ");

        Assert.Equal(new byte[] { 1, 2, 3, 4 }, bytes);
        Assert.Equal("/v1/contracts/code-bytes/0011aa", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task DeployAndActivateContractInstanceAsyncEncodesRouteAndDeserializesResponse()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "ok": true,
                  "namespace": "apps",
                  "contract_id": "calc.v1",
                  "code_hash_hex": "aa55",
                  "abi_hash_hex": "bb66"
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var response = await client.DeployAndActivateContractInstanceAsync(new ToriiDeployAndActivateContractInstanceRequest
        {
            Authority = "sorauロ1Ncaller",
            PrivateKey = "ed0120AABB",
            Namespace = "apps",
            ContractId = "calc.v1",
            CodeBase64 = "AQID",
        });

        Assert.True(response.Ok);
        Assert.Equal("apps", response.Namespace);
        Assert.Equal("calc.v1", response.ContractId);
        Assert.Equal("/v1/contracts/instance", handler.LastRequest!.RequestUri!.AbsolutePath);
        Assert.Equal(HttpMethod.Post, handler.LastRequest.Method);
    }

    [Fact]
    public async Task ActivateContractInstanceAsyncEncodesRouteAndDeserializesResponse()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""{ "ok": true }"""),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var response = await client.ActivateContractInstanceAsync(new ToriiActivateContractInstanceRequest
        {
            Authority = "sorauロ1Ncaller",
            PrivateKey = "ed0120AABB",
            Namespace = "apps",
            ContractId = "calc.v1",
            CodeHash = "aa55",
        });

        Assert.True(response.Ok);
        Assert.Equal("/v1/contracts/instance/activate", handler.LastRequest!.RequestUri!.AbsolutePath);
        Assert.Equal(HttpMethod.Post, handler.LastRequest.Method);
    }

    [Fact]
    public async Task GetContractInstancesAsyncAddsFiltersAndDeserializesResponse()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "namespace": "universal",
                  "instances": [
                    {
                      "contract_id": "router::dex.universal",
                      "code_hash_hex": "aa55"
                    }
                  ],
                  "total": 3,
                  "offset": 2,
                  "limit": 5
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var response = await client.GetContractInstancesAsync(
            " universal ",
            new ToriiContractInstancesQuery
            {
                Contains = " dex ",
                HashPrefix = " aa ",
                Offset = 2,
                Limit = 5,
                Order = " hash_desc ",
            });

        Assert.Equal("universal", response.Namespace);
        Assert.Single(response.Instances);
        Assert.Equal("router::dex.universal", response.Instances[0].ContractId);
        Assert.Equal((ulong)3, response.Total);
        Assert.Equal("/v1/contracts/instances/universal?contains=dex&hash_prefix=aa&offset=2&limit=5&order=hash_desc", handler.LastRequest!.RequestUri!.PathAndQuery);
    }

    [Fact]
    public async Task GetContractStateAsyncAddsQueryAndDeserializesResponse()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "path": null,
                  "paths": ["balances/alice", "balances/bob"],
                  "prefix": null,
                  "entries": [
                    {
                      "path": "balances/alice",
                      "found": true,
                      "value_json": { "quantity": "1" }
                    },
                    {
                      "path": "balances/bob",
                      "found": false
                    }
                  ],
                  "offset": 0,
                  "limit": 2,
                  "next_offset": null
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var response = await client.GetContractStateAsync(new ToriiContractStateQuery
        {
            Paths = [" balances/alice ", " balances/bob "],
            IncludeValue = false,
            Decode = " json ",
        });

        Assert.Equal(2, response.Paths!.Count);
        Assert.Equal("1", response.Entries[0].ValueJson!["quantity"]!.GetValue<string>());
        Assert.False(response.Entries[1].Found);
        Assert.Equal("/v1/contracts/state?paths=balances%2Falice%2Cbalances%2Fbob&include_value=false&decode=json", handler.LastRequest!.RequestUri!.PathAndQuery);
    }

    [Fact]
    public async Task CallContractAsyncDeserializesScaffoldResponse()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "ok": true,
                  "submitted": false,
                  "dataspace": "universal",
                  "contract_id": "router::dex.universal",
                  "contract_address": "iroha1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq",
                  "code_hash_hex": "aa55",
                  "abi_hash_hex": "bb66",
                  "creation_time_ms": 123456,
                  "tx_hash_hex": null,
                  "transaction_scaffold_b64": "c2NhZmZvbGQ=",
                  "signed_transaction_b64": "c2lnbmVk",
                  "signing_message_b64": "bWVzc2FnZQ==",
                  "entrypoint": "main"
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var response = await client.CallContractAsync(new ToriiContractCallRequest
        {
            Authority = "sorauロ1Ncaller",
            ContractAlias = "router::dex.universal",
            Payload = JsonNode.Parse("""{ "amount": "1" }"""),
            GasLimit = 500_000,
        });

        Assert.True(response.Ok);
        Assert.False(response.Submitted);
        Assert.Equal("router::dex.universal", response.ContractId);
        Assert.Equal("c2NhZmZvbGQ=", response.TransactionScaffoldBase64);
        Assert.Equal("/v1/contracts/call", handler.LastRequest!.RequestUri!.AbsolutePath);
        Assert.Equal(HttpMethod.Post, handler.LastRequest.Method);
    }

    [Fact]
    public async Task GetContractCodeViewAsyncEncodesCodeHashAndDeserializesView()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "code_hash": "0011aa",
                  "declared_code_hash": null,
                  "abi_hash": "99ff",
                  "compiler_fingerprint": "torii-tests",
                  "byte_len": 256,
                  "permissions": [],
                  "access_hints": null,
                  "entrypoints": [],
                  "analysis": null,
                  "warnings": ["verified source record loaded"],
                  "rendered_source_kind": "verified_source",
                  "rendered_source_text": "kotoage fn main() {}",
                  "verified_source_ref": {
                    "language": "kotodama",
                    "source_name": "demo.ko",
                    "submitted_at": "2026-03-29T12:30:00Z",
                    "manifest_id_hex": "abcd",
                    "payload_digest_hex": "efgh",
                    "content_length": 24
                  }
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var view = await client.GetContractCodeViewAsync(" 0011aa ");

        Assert.Equal("0011aa", view.CodeHash);
        Assert.Equal("verified_source", view.RenderedSourceKind);
        Assert.Equal("kotoage fn main() {}", view.RenderedSourceText);
        Assert.Equal((ulong)24, view.VerifiedSourceReference!.ContentLength);
        Assert.Equal("/v1/contracts/code/0011aa/contract-view", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task ExecuteContractViewAsyncEncodesRequestAndDeserializesSuccess()
    {
        using var handler = new RecordingHandler(_ =>
        {
            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("""
                {
                  "ok": true,
                  "dataspace": "universal",
                  "contract_id": "router::dex.universal",
                  "contract_address": "iroha1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq",
                  "code_hash_hex": "aa55",
                  "abi_hash_hex": "bb66",
                  "entrypoint": "main",
                  "result": {
                    "status": "ok",
                    "matched": 2
                  }
                }
                """),
            };
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var result = await client.ExecuteContractViewAsync(new ToriiContractViewRequest
        {
            Authority = "sorauロ1Ncaller",
            ContractAlias = "router::dex.universal",
            Entrypoint = "main",
            Payload = JsonNode.Parse("""{ "amount": "1" }"""),
            GasLimit = 500_000,
        });

        Assert.True(result.IsSuccess);
        Assert.NotNull(result.Success);
        Assert.Null(result.Error);
        Assert.Equal("router::dex.universal", result.Success!.ContractId);
        Assert.Equal("ok", result.Success.Result!["status"]!.GetValue<string>());
        Assert.Equal("/v1/contracts/view", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task ExecuteContractViewAsyncDeserializesValidationFailure()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.UnprocessableEntity)
        {
            Content = new StringContent("""
                {
                  "ok": false,
                  "dataspace": "universal",
                  "contract_id": "router::dex.universal",
                  "contract_address": null,
                  "code_hash_hex": "aa55",
                  "abi_hash_hex": "bb66",
                  "entrypoint": "main",
                  "error": "view entrypoint rejected payload",
                  "vm_diagnostic": {
                    "trap_kind": "Validation",
                    "message": "missing field `amount`",
                    "pc": 12,
                    "function": "main",
                    "source_path": "contracts/router.ko",
                    "line": 4,
                    "column": 9,
                    "gas_limit": 500000,
                    "gas_remaining": 499900,
                    "gas_used": 100,
                    "cycles": 20,
                    "max_cycles": 500000,
                    "stack_limit_bytes": 8192,
                    "stack_bytes_used": 256,
                    "entrypoint_pc": 10,
                    "current_function": "main",
                    "opcode": 34,
                    "syscall": 7,
                    "predecoded_loaded": true,
                    "predecoded_hit": false
                  }
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var result = await client.ExecuteContractViewAsync(new ToriiContractViewRequest
        {
            Authority = "sorauロ1Ncaller",
            ContractAlias = "router::dex.universal",
            GasLimit = 500_000,
        });

        Assert.False(result.IsSuccess);
        Assert.Null(result.Success);
        Assert.NotNull(result.Error);
        Assert.Equal("view entrypoint rejected payload", result.Error!.Error);
        Assert.Equal((ulong)100, result.Error.VmDiagnostic!.GasUsed);
        Assert.Equal((ushort)34, result.Error.VmDiagnostic.Opcode);
    }

    [Fact]
    public async Task ProposeMultisigContractCallAsyncDeserializesScaffoldResponse()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "ok": true,
                  "resolved_multisig_account_id": "sorauﾛ1Nmultisig",
                  "submitted": false,
                  "proposal_id": "aa55",
                  "instructions_hash": "aa55",
                  "tx_hash_hex": null,
                  "executed_tx_hash_hex": null,
                  "creation_time_ms": 321,
                  "signing_message_b64": "bXVsdGlzaWc="
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var response = await client.ProposeMultisigContractCallAsync(new ToriiMultisigContractCallProposeRequest
        {
            MultisigAccountAlias = "ops@universal",
            SignerAccountId = "sorauロ1Nsigner",
            Namespace = "apps",
            ContractId = "calc.v1",
            Entrypoint = "main",
        });

        Assert.True(response.Ok);
        Assert.False(response.Submitted);
        Assert.Equal("aa55", response.ProposalId);
        Assert.Equal("/v1/contracts/call/multisig/propose", handler.LastRequest!.RequestUri!.AbsolutePath);
        Assert.Equal(HttpMethod.Post, handler.LastRequest.Method);
    }

    [Fact]
    public async Task ProposeMultisigAsyncPostsNativeNoritoInstructionFrames()
    {
        const string accountId = "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53";
        var instructionBase64 = TransactionInstruction
            .ExecuteTrigger("daily-close")
            .EncodeInstructionBoxBase64(accountId);

        using var handler = new RecordingHandler(request =>
        {
            var payload = ReadBodyAsJson(request);
            Assert.Equal("/v1/multisig/propose", request.RequestUri!.AbsolutePath);
            Assert.Equal(HttpMethod.Post, request.Method);
            Assert.Equal("ops@universal", payload.RootElement.GetProperty("multisig_account_alias").GetString());
            Assert.Equal(accountId, payload.RootElement.GetProperty("signer_account_id").GetString());
            Assert.Equal((ulong)123, payload.RootElement.GetProperty("creation_time_ms").GetUInt64());

            var encodedInstruction = payload.RootElement.GetProperty("instructions")[0].GetString()!;
            Assert.Equal(instructionBase64, encodedInstruction);
            var instructionBytes = Convert.FromBase64String(encodedInstruction);
            Assert.Equal("862a7d77075d4d23ff6c1261db027811", Convert.ToHexString(instructionBytes.AsSpan(6, 16)).ToLowerInvariant());

            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("""
                    {
                      "ok": true,
                      "resolved_multisig_account_id": "sorauﾛ1Nmultisig",
                      "submitted": false,
                      "proposal_id": "aa55",
                      "instructions_hash": "aa55",
                      "tx_hash_hex": null,
                      "executed_tx_hash_hex": null,
                      "creation_time_ms": 123,
                      "signing_message_b64": "bXVsdGlzaWc="
                    }
                    """),
            };
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var response = await client.ProposeMultisigAsync(new ToriiMultisigProposeRequest
        {
            MultisigAccountAlias = "ops@universal",
            SignerAccountId = accountId,
            CreationTimeMilliseconds = 123,
            Instructions = [instructionBase64],
        });

        Assert.True(response.Ok);
        Assert.False(response.Submitted);
        Assert.Equal("aa55", response.ProposalId);
        Assert.Equal("bXVsdGlzaWc=", response.SigningMessageBase64);
    }

    [Fact]
    public void EncodeInstructionBoxBase64RejectsMissingAuthority()
    {
        var instruction = TransactionInstruction.ExecuteTrigger("daily-close");

        Assert.Throws<ArgumentException>(() => instruction.EncodeInstructionBoxBase64(""));
        Assert.Throws<ArgumentNullException>(() => instruction.EncodeInstructionBoxBase64(null!));
    }

    [Fact]
    public async Task ProposeMultisigAsyncPropagatesToriiRejection()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.UnprocessableEntity)
        {
            Content = new StringContent("""{"error":"malformed native instruction frame"}"""),
            ReasonPhrase = "Unprocessable Entity",
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var ex = await Assert.ThrowsAsync<ToriiApiException>(() =>
            client.ProposeMultisigAsync(new ToriiMultisigProposeRequest
            {
                MultisigAccountAlias = "ops@universal",
                SignerAccountId = "sorauﾛ1Nmultisig",
                Instructions = ["AQID"],
            }));

        Assert.Equal(HttpStatusCode.UnprocessableEntity, ex.StatusCode);
        var responseBody = Assert.IsType<string>(ex.ResponseBody);
        Assert.Contains("malformed native instruction frame", responseBody);
        Assert.Equal("/v1/multisig/propose", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task ProposeMultisigAsyncRejectsMalformedSuccessResponse()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "ok": true,
                  "resolved_multisig_account_id": "sorauﾛ1Nmultisig",
                  "instructions_hash": {}
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        await Assert.ThrowsAsync<JsonException>(() =>
            client.ProposeMultisigAsync(new ToriiMultisigProposeRequest
            {
                MultisigAccountAlias = "ops@universal",
                SignerAccountId = "sorauﾛ1Nmultisig",
                Instructions = ["AQID"],
            }));
        Assert.Equal("/v1/multisig/propose", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task ProposeMultisigAsyncRejectsFalseOkResponse()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "ok": false,
                  "resolved_multisig_account_id": "sorauﾛ1Nmultisig"
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        await Assert.ThrowsAsync<JsonException>(() =>
            client.ProposeMultisigAsync(new ToriiMultisigProposeRequest
            {
                MultisigAccountAlias = "ops@universal",
                SignerAccountId = "sorauﾛ1Nmultisig",
                Instructions = ["AQID"],
            }));
        Assert.Equal("/v1/multisig/propose", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task ProposeMultisigAsyncRejectsEmptySigningMessageResponse()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "ok": true,
                  "resolved_multisig_account_id": "sorauﾛ1Nmultisig",
                  "signing_message_b64": ""
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        await Assert.ThrowsAsync<JsonException>(() =>
            client.ProposeMultisigAsync(new ToriiMultisigProposeRequest
            {
                MultisigAccountAlias = "ops@universal",
                SignerAccountId = "sorauﾛ1Nmultisig",
                Instructions = ["AQID"],
            }));
        Assert.Equal("/v1/multisig/propose", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task ProposeMultisigAsyncRejectsNegativeCreationTimeResponse()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "ok": true,
                  "resolved_multisig_account_id": "sorauﾛ1Nmultisig",
                  "creation_time_ms": -1
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        await Assert.ThrowsAsync<JsonException>(() =>
            client.ProposeMultisigAsync(new ToriiMultisigProposeRequest
            {
                MultisigAccountAlias = "ops@universal",
                SignerAccountId = "sorauﾛ1Nmultisig",
                Instructions = ["AQID"],
            }));
        Assert.Equal("/v1/multisig/propose", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task ApproveMultisigContractCallAsyncDeserializesScaffoldResponse()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "ok": true,
                  "resolved_multisig_account_id": "sorauﾛ1Nmultisig",
                  "submitted": false,
                  "proposal_id": "aa55",
                  "instructions_hash": "aa55",
                  "tx_hash_hex": null,
                  "executed_tx_hash_hex": null,
                  "creation_time_ms": 654,
                  "signing_message_b64": "YXBwcm92ZQ=="
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var response = await client.ApproveMultisigContractCallAsync(new ToriiMultisigContractCallApproveRequest
        {
            MultisigAccountAlias = "ops@universal",
            SignerAccountId = "sorauロ1Nsigner",
            ProposalId = "aa55",
        });

        Assert.True(response.Ok);
        Assert.False(response.Submitted);
        Assert.Equal("aa55", response.InstructionsHash);
        Assert.Equal("/v1/contracts/call/multisig/approve", handler.LastRequest!.RequestUri!.AbsolutePath);
        Assert.Equal(HttpMethod.Post, handler.LastRequest.Method);
    }

    [Fact]
    public async Task SubmitContractVerifiedSourceJobAsyncEncodesCodeHashAndRequest()
    {
        using var handler = new RecordingHandler(_ =>
        {
            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("""
                {
                  "job_id": "job-1",
                  "code_hash": "0011aa",
                  "status": "verified",
                  "submitted_at": "2026-03-29T13:00:00Z",
                  "completed_at": "2026-03-29T13:00:01Z",
                  "message": "source matched",
                  "actual_code_hash": "0011aa",
                  "verified_source_ref": {
                    "language": "kotodama",
                    "source_name": "demo.ko",
                    "submitted_at": "2026-03-29T13:00:01Z",
                    "manifest_id_hex": "abcd",
                    "payload_digest_hex": "ef01",
                    "content_length": 22
                  }
                }
                """),
            };
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var job = await client.SubmitContractVerifiedSourceJobAsync(
            " 0011aa ",
            new ToriiContractVerifiedSourceSubmission
            {
                Language = "kotodama",
                SourceName = "demo.ko",
                SourceText = "public fn main() {}",
            });

        Assert.Equal("job-1", job.JobId);
        Assert.Equal("verified", job.Status);
        Assert.Equal((ulong)22, job.VerifiedSourceReference!.ContentLength);
        Assert.Equal("/v1/contracts/code/0011aa/verified-source/jobs", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task GetContractVerifiedSourceJobAsyncEncodesPathAndDeserializesResponse()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "job_id": "job-1",
                  "code_hash": "0011aa",
                  "status": "mismatch",
                  "submitted_at": "2026-03-29T13:00:00Z",
                  "completed_at": "2026-03-29T13:00:02Z",
                  "message": "declared hash differs",
                  "actual_code_hash": "deadbeef"
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var job = await client.GetContractVerifiedSourceJobAsync(" 0011aa ", " job-1 ");

        Assert.NotNull(job);
        Assert.Equal("mismatch", job!.Status);
        Assert.Equal("deadbeef", job.ActualCodeHash);
        Assert.Equal("/v1/contracts/code/0011aa/verified-source-jobs/job-1", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task GetContractVerifiedSourceJobAsyncReturnsNullOnNotFound()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.NotFound));

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var job = await client.GetContractVerifiedSourceJobAsync("0011aa", "missing-job");

        Assert.Null(job);
    }

    [Fact]
    public async Task ResolveContractAliasAsyncReturnsNullOnNotFound()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.NotFound));

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var resolved = await client.ResolveContractAliasAsync("missing::universal");

        Assert.Null(resolved);
    }

    [Fact]
    public async Task ResolveAccountAliasAsyncReturnsNullOnNotFound()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.NotFound));

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var resolved = await client.ResolveAccountAliasAsync("missing-alias");

        Assert.Null(resolved);
    }

    [Fact]
    public async Task GetRuntimeEndpointsDeserializeTypedPayloads()
    {
        using var handler = new RecordingHandler(request =>
        {
            var body = request.RequestUri!.AbsolutePath switch
            {
                "/v1/runtime/abi/active" => """{ "abi_version": 1 }""",
                "/v1/runtime/abi/hash" => """{ "policy": "V1", "abi_hash_hex": "deadbeef" }""",
                "/v1/runtime/metrics" => """
                    {
                      "abi_version": 1,
                      "upgrade_events_total": {
                        "proposed": 2,
                        "activated": 1,
                        "canceled": 0
                      }
                    }
                    """,
                _ => throw new InvalidOperationException("Unexpected route."),
            };

            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent(body),
            };
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        var active = await client.GetRuntimeAbiActiveAsync();
        var hash = await client.GetRuntimeAbiHashAsync();
        var metrics = await client.GetRuntimeMetricsAsync();

        Assert.Equal(1, active.AbiVersion);
        Assert.Equal("V1", hash.Policy);
        Assert.Equal("deadbeef", hash.AbiHashHex);
        Assert.Equal(2, metrics.UpgradeEventsTotal.Proposed);
        Assert.Equal(1, metrics.UpgradeEventsTotal.Activated);
    }

    [Fact]
    public async Task SendAsyncThrowsToriiApiExceptionWithStatusBodyAndUri()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.UnprocessableEntity)
        {
            Content = new StringContent("""{ "error": "invalid account id literal" }"""),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        var exception = await Assert.ThrowsAsync<ToriiApiException>(() => client.GetNodeCapabilitiesAsync());

        Assert.Equal(HttpStatusCode.UnprocessableEntity, exception.StatusCode);
        Assert.Equal("https://torii.example/v1/node/capabilities", exception.RequestUri?.ToString());
        Assert.Contains("invalid account id literal", exception.ResponseBody);
    }

    private static JsonDocument ReadBodyAsJson(HttpRequestMessage request)
    {
        var body = request.Content!.ReadAsStringAsync().GetAwaiter().GetResult();
        return JsonDocument.Parse(body);
    }

    private static HttpResponseMessage JsonResponse(string json, HttpStatusCode statusCode = HttpStatusCode.OK)
    {
        return new HttpResponseMessage(statusCode)
        {
            Content = new StringContent(json, System.Text.Encoding.UTF8, "application/json"),
        };
    }

    private static ToriiSoraFsPinRegisterRequest ValidSoraFsPinRegisterRequest()
    {
        return new ToriiSoraFsPinRegisterRequest
        {
            Authority = " alice@boi ",
            PrivateKey = " ed25519:deadbeef ",
            Chunker = new ToriiSoraFsChunkerHandle
            {
                ProfileId = 1,
                Namespace = " sorafs ",
                Name = " sf1 ",
                Semver = " 1.0.0 ",
                MultihashCode = 0,
            },
            PinPolicy = new ToriiSoraFsPinPolicy
            {
                MinReplicas = 3,
                StorageClass = ToriiSoraFsStorageClass.From("hot"),
                RetentionEpoch = 72,
            },
            ManifestDigestHex = "0x" + new string('A', 64),
            ChunkDigestSha3_256Hex = "0x" + new string('B', 64),
            ContentLength = 4096,
            SubmittedEpoch = 42,
            Alias = new ToriiSoraFsPinAlias
            {
                Namespace = " docs ",
                Name = " main ",
                ProofBase64 = Convert.ToBase64String("alias-proof"u8.ToArray()),
            },
            SuccessorOfHex = new string('C', 64),
        };
    }

    private static ToriiVerifyingKeyRegisterRequest ValidVerifyingKeyRegisterRequest()
    {
        var vkBytes = "abc"u8.ToArray();
        return new ToriiVerifyingKeyRegisterRequest
        {
            Authority = "alice",
            PrivateKey = "ed25519:deadbeef",
            Backend = "halo2/ipa",
            Name = "vk_main",
            Version = 1,
            CircuitId = "halo2/ipa::transfer_v1",
            PublicInputsSchemaHashHex = new string('a', 64),
            GasScheduleId = "halo2_default",
            VerifyingKeyBytes = vkBytes,
            CommitmentHex = VerifyingKeyCommitmentHex("halo2/ipa", vkBytes),
            Status = "Active",
        };
    }

    private static string IdentifierResolveResponseJson(
        string signature,
        string signaturePayloadHex,
        string signaturePayloadJson,
        string policyId = "phone#retail")
    {
        return $$"""
            {
              "policy_id": {{JsonSerializer.Serialize(policyId)}},
              "opaque_id": "opaque-1",
              "receipt_hash": "receipt-1",
              "uaid": "uaid:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
              "account_id": "sorauﾛ1Nmerchant",
              "resolved_at_ms": 1710000000000,
              "expires_at_ms": 1710003600000,
              "backend": "bfv-programmed-sha3-256-v1",
              "signature": {{JsonSerializer.Serialize(signature)}},
              "signature_payload_hex": {{JsonSerializer.Serialize(signaturePayloadHex)}},
              "signature_payload": {{signaturePayloadJson}}
            }
            """;
    }

    private static string VerifyingKeyCommitmentHex(string backend, byte[] bytes)
    {
        var domainBytes = Encoding.UTF8.GetBytes("iroha:zk:v1:vk");
        var backendBytes = Encoding.UTF8.GetBytes(backend);
        var preimage = new byte[domainBytes.Length + 8 + backendBytes.Length + 8 + bytes.Length];
        var offset = 0;
        Buffer.BlockCopy(domainBytes, 0, preimage, offset, domainBytes.Length);
        offset += domainBytes.Length;
        WriteUInt64BigEndian(preimage, offset, (ulong)backendBytes.Length);
        offset += 8;
        Buffer.BlockCopy(backendBytes, 0, preimage, offset, backendBytes.Length);
        offset += backendBytes.Length;
        WriteUInt64BigEndian(preimage, offset, (ulong)bytes.Length);
        offset += 8;
        Buffer.BlockCopy(bytes, 0, preimage, offset, bytes.Length);
        return Convert.ToHexString(SHA256.HashData(preimage)).ToLowerInvariant();
    }

    private static void WriteUInt64BigEndian(byte[] target, int offset, ulong value)
    {
        for (var index = 7; index >= 0; index--)
        {
            target[offset + index] = (byte)(value & 0xff);
            value >>= 8;
        }
    }

    private static string EventFilterQuery(string filterJson)
    {
        return "filter=" + Uri.EscapeDataString(filterJson);
    }

    private static string QueryParameter(string query, string name)
    {
        var queryText = query.StartsWith("?", StringComparison.Ordinal) ? query[1..] : query;
        foreach (var segment in queryText.Split('&'))
        {
            var equalsIndex = segment.IndexOf('=');
            var rawName = equalsIndex >= 0 ? segment[..equalsIndex] : segment;
            if (!string.Equals(Uri.UnescapeDataString(rawName), name, StringComparison.Ordinal))
            {
                continue;
            }

            var rawValue = equalsIndex >= 0 ? segment[(equalsIndex + 1)..] : string.Empty;
            return Uri.UnescapeDataString(rawValue.Replace("+", " ", StringComparison.Ordinal));
        }

        throw new InvalidOperationException($"Query parameter {name} was not present.");
    }

    private sealed class RecordingHandler : HttpMessageHandler
    {
        private readonly Func<HttpRequestMessage, HttpResponseMessage> responder;

        public RecordingHandler(Func<HttpRequestMessage, HttpResponseMessage> responder)
        {
            this.responder = responder;
        }

        public HttpRequestMessage? LastRequest { get; private set; }

        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
        {
            LastRequest = request;
            var response = responder(request);
            response.RequestMessage ??= request;
            return Task.FromResult(response);
        }
    }
}
