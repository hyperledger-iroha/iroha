using System.Net;
using System.Text.Json;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Http;
using Hyperledger.Iroha.Queries;
using Hyperledger.Iroha.Torii;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class ToriiClientTests
{
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
                "sorauロ1NイリウdPBeシRoクQ2ヤgシQqeカヘスチhRW2コソZ9ユヲUナRX5NJYH53",
                Convert.FromHexString("616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032")),
        };

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler), options);
        using var document = await client.GetJsonDocumentAsync("/v1/query", "gas_units=100&cursor_mode=stored");

        Assert.True(document.RootElement.GetProperty("ok").GetBoolean());
        Assert.Equal("Bearer", handler.LastRequest!.Headers.Authorization?.Scheme);
        Assert.Equal("dev-token", handler.LastRequest.Headers.Authorization?.Parameter);
        Assert.True(handler.LastRequest.Headers.Contains("X-Iroha-Account"));
        Assert.True(handler.LastRequest.Headers.Contains("X-Iroha-Signature"));
        Assert.Equal("/v1/query?gas_units=100&cursor_mode=stored", handler.LastRequest.RequestUri!.PathAndQuery);
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
            Domain = " wonderland.sbp ",
            WithAsset = " rose#wonderland.sbp ",
        });

        Assert.Equal((ulong)11, page.Pagination.TotalItems);
        Assert.Single(page.Items);
        Assert.Equal("i105:sorauロ1Nholder", page.Items[0].I105Address);
        Assert.Equal("gold", page.Items[0].Metadata!["tier"]!.GetValue<string>());
        Assert.Equal("/v1/explorer/accounts?page=2&per_page=5&domain=wonderland.sbp&with_asset=rose%23wonderland.sbp", handler.LastRequest!.RequestUri!.PathAndQuery);
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
                      "id": "wonderland.sbp",
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
        Assert.Equal("wonderland.sbp", page.Items[0].Id);
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
                  "id": "wonderland.sbp",
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
        var domain = await client.GetExplorerDomainAsync(" wonderland.sbp ");

        Assert.Equal("sorauロ1Nowner", domain.OwnedBy);
        Assert.Equal((uint)4, domain.Accounts);
        Assert.Equal("/v1/explorer/domains/wonderland.sbp", handler.LastRequest!.RequestUri!.AbsolutePath);
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
                      "id": "rose#wonderland.sbp",
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
            Domain = " wonderland.sbp ",
            OwnedBy = " issuer@universal ",
        });

        Assert.Equal((ulong)8, page.Pagination.TotalItems);
        Assert.Single(page.Items);
        Assert.Equal("975", page.Items[0].CirculatingQuantity);
        Assert.Equal("flora", page.Items[0].Metadata!["category"]!.GetValue<string>());
        Assert.Equal("/v1/explorer/asset-definitions?page=3&per_page=2&domain=wonderland.sbp&owned_by=issuer%40universal", handler.LastRequest!.RequestUri!.PathAndQuery);
    }

    [Fact]
    public async Task GetExplorerAssetDefinitionAsyncEncodesIdentifierAndDeserializesDetail()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "id": "rose#wonderland.sbp",
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
        var definition = await client.GetExplorerAssetDefinitionAsync(" rose#wonderland.sbp ");

        Assert.Equal("Infinitely", definition.Mintable);
        Assert.Equal("975", definition.CirculatingQuantity);
        Assert.Equal("/v1/explorer/asset-definitions/rose%23wonderland.sbp", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task GetExplorerAssetDefinitionEconometricsAsyncEncodesIdentifierAndDeserializesSnapshot()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "definition_id": "rose#wonderland.sbp",
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
        var snapshot = await client.GetExplorerAssetDefinitionEconometricsAsync(" rose#wonderland.sbp ");

        Assert.Equal((ulong)123456, snapshot.ComputedAtMilliseconds);
        Assert.Single(snapshot.VelocityWindows);
        Assert.Equal((ulong)2, snapshot.VelocityWindows[0].Transfers);
        Assert.Equal("100", snapshot.IssuanceWindows[0].Net);
        Assert.Equal("/v1/explorer/asset-definitions/rose%23wonderland.sbp/econometrics", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task GetExplorerAssetDefinitionSnapshotAsyncEncodesIdentifierAndDeserializesSnapshot()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "definition_id": "rose#wonderland.sbp",
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
        var snapshot = await client.GetExplorerAssetDefinitionSnapshotAsync(" rose#wonderland.sbp ");

        Assert.Equal((ulong)2, snapshot.HoldersTotal);
        Assert.Equal("700", snapshot.TopHolders[0].Balance);
        Assert.Equal(0.7, snapshot.Distribution.Top1);
        Assert.Equal(2, snapshot.Distribution.Lorenz.Count);
        Assert.Equal("/v1/explorer/asset-definitions/rose%23wonderland.sbp/snapshot", handler.LastRequest!.RequestUri!.AbsolutePath);
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
                      "definition_id": "rose#wonderland.sbp",
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
            Definition = " rose#wonderland.sbp ",
            AssetId = " asset-001 ",
        });

        Assert.Equal((ulong)9, page.Pagination.TotalItems);
        Assert.Single(page.Items);
        Assert.Equal("25", page.Items[0].Value);
        Assert.Equal("/v1/explorer/assets?page=4&per_page=1&owned_by=holder%40universal&definition=rose%23wonderland.sbp&asset_id=asset-001", handler.LastRequest!.RequestUri!.PathAndQuery);
    }

    [Fact]
    public async Task GetExplorerAssetAsyncEncodesIdentifierAndDeserializesDetail()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "id": "asset-001",
                  "definition_id": "rose#wonderland.sbp",
                  "account_id": "sorauロ1Nholder",
                  "value": "25"
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var asset = await client.GetExplorerAssetAsync(" asset-001 ");

        Assert.Equal("rose#wonderland.sbp", asset.DefinitionId);
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
            Domain = " wonderland.sbp ",
        });

        Assert.Single(page.Items);
        Assert.Equal("A1", page.Items[0].Metadata!["seat"]!.GetValue<string>());
        Assert.Equal("/v1/explorer/nfts?page=2&per_page=2&owned_by=holder%40universal&domain=wonderland.sbp", handler.LastRequest!.RequestUri!.PathAndQuery);
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
                      "owned_by": "sorauロ1Ncustodian",
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
            Domain = " wonderland.sbp ",
        });

        Assert.Single(page.Items);
        Assert.Equal("vault-7", page.Items[0].PrimaryReference);
        Assert.Equal("ore$mine#wonderland", page.Items[0].Parents[0].Rwa);
        Assert.Equal("/v1/explorer/rwas?page=1&per_page=5&owned_by=custodian%40universal&domain=wonderland.sbp", handler.LastRequest!.RequestUri!.PathAndQuery);
    }

    [Fact]
    public async Task GetExplorerRwaAsyncEncodesIdentifierAndDeserializesDetail()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "id": "lot$gold#wonderland",
                  "owned_by": "sorauロ1Ncustodian",
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
                      "asset": "rose#wonderland.sbp",
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
        var balances = await client.GetAccountAssetsAsync("sorauロ1Nholder", limit: 10, offset: 1, asset: "rose#wonderland.sbp", scope: "global");

        Assert.Single(balances.Items);
        Assert.Equal("rose#wonderland.sbp", balances.Items[0].Asset);
        Assert.Equal("10", balances.Items[0].Quantity);
        Assert.Contains("/v1/accounts/", handler.LastRequest!.RequestUri!.AbsoluteUri);
        Assert.Contains("/assets", handler.LastRequest.RequestUri.AbsoluteUri);
        Assert.Equal("limit=10&offset=1&asset=rose%23wonderland.sbp&scope=global", handler.LastRequest.RequestUri.Query.TrimStart('?'));
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
        var transactions = await client.GetAccountTransactionsAsync("sorauロ1Nholder", limit: 50, offset: 3, assetId: "rose#wonderland.sbp");

        Assert.Single(transactions.Items);
        Assert.Equal("hash", transactions.Items[0].EntrypointHash);
        Assert.True(transactions.Items[0].ResultOk);
        Assert.Contains("/v1/accounts/", handler.LastRequest!.RequestUri!.AbsoluteUri);
        Assert.Contains("/transactions", handler.LastRequest.RequestUri.AbsoluteUri);
        Assert.Equal("limit=50&offset=3&asset_id=rose%23wonderland.sbp", handler.LastRequest.RequestUri.Query.TrimStart('?'));
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
            AssetId = " rose#wonderland.sbp ",
        });

        Assert.Equal((ulong)21, page.Pagination.TotalItems);
        Assert.Single(page.Items);
        Assert.Equal("tx-1", page.Items[0].Hash);
        Assert.Equal("Committed", page.Items[0].Status);
        Assert.Equal(
            "page=1&per_page=20&authority=sorau%E3%83%AD1Nholder&block=5&status=committed&asset_id=rose%23wonderland.sbp",
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
                      "authority": "sorauロ1Nauthority",
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
            Authority = " sorauロ1Nauthority ",
            Account = " sorauロ1Naccount ",
            TransactionHash = " tx-ins ",
            TransactionStatus = ToriiExplorerTransactionStatusFilter.Committed,
            Block = 12,
            Kind = " transfer ",
            AssetId = " rose#wonderland.sbp ",
        });

        Assert.Equal((ulong)48, page.Pagination.TotalItems);
        Assert.Single(page.Items);
        Assert.Equal("Transfer", page.Items[0].Kind);
        Assert.Equal("tx-ins", page.Items[0].TransactionHash);
        Assert.Equal(
            "page=3&per_page=10&authority=sorau%E3%83%AD1Nauthority&account=sorau%E3%83%AD1Naccount&transaction_hash=tx-ins&transaction_status=committed&block=12&kind=transfer&asset_id=rose%23wonderland.sbp",
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
                      "authority": "sorauロ1Nauthority",
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
                  "authority": "sorauロ1Nauthority",
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
            Assert.Equal("sbp", payload.RootElement.GetProperty("domain").GetString());

            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("""
                    {
                      "account_id": "sorauロ1Nholder",
                      "total": 2,
                      "source": "on_chain",
                      "items": [
                        {
                          "alias": "merchant@sbp.universal",
                          "dataspace": "universal",
                          "domain": "sbp",
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
        var aliases = await client.LookupAliasesByAccountAsync("sorauロ1Nholder", dataspace: "universal", domain: "sbp");

        Assert.NotNull(aliases);
        Assert.Equal("sorauロ1Nholder", aliases!.AccountId);
        Assert.Equal(2, aliases.Total);
        Assert.Equal("merchant@sbp.universal", aliases.Items[0].Alias);
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
                  "dns_servers": ["1.1.1.1", "8.8.8.8"],
                  "display_billing_label": "standard · vpn-standard"
                }
                """),
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var profile = await client.GetVpnProfileAsync();

        Assert.True(profile.Available);
        Assert.Equal("standard", profile.DefaultExitClass);
        Assert.Equal((ulong)3600, profile.LeaseSeconds);
        Assert.Equal(3, profile.SupportedExitClasses.Count);
        Assert.Equal("/v1/vpn/profile", handler.LastRequest!.RequestUri!.AbsolutePath);
    }

    [Fact]
    public async Task CreateVpnSessionAsyncPostsExitClassAndDeserializesSession()
    {
        using var handler = new RecordingHandler(request =>
        {
            var payload = ReadBodyAsJson(request);
            Assert.Equal("/v1/vpn/sessions", request.RequestUri!.AbsolutePath);
            Assert.Equal("low-latency", payload.RootElement.GetProperty("exit_class").GetString());

            return new HttpResponseMessage(HttpStatusCode.Created)
            {
                Content = new StringContent("""
                    {
                      "session_id": "session-1",
                      "account_id": "sorauロ1Nholder",
                      "exit_class": "low-latency",
                      "relay_endpoint": "/dns4/vpn.sora.org/tcp/443/wss",
                      "lease_secs": 3600,
                      "expires_at_ms": 1700000000000,
                      "meter_family": "vpn-standard",
                      "route_pushes": ["10.0.0.0/8"],
                      "dns_servers": ["1.1.1.1"],
                      "helper_ticket_hex": "deadbeef",
                      "status": "active"
                    }
                    """),
            };
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var session = await client.CreateVpnSessionAsync(new ToriiVpnSessionCreateRequest
        {
            ExitClass = "low-latency",
        });

        Assert.Equal("session-1", session.SessionId);
        Assert.Equal("low-latency", session.ExitClass);
        Assert.Equal("active", session.Status);
        Assert.Equal((ulong)1700000000000, session.ExpiresAtMilliseconds);
    }

    [Fact]
    public async Task DeleteVpnSessionAsyncAllowsNotFoundAndDeserializesResponse()
    {
        using var handler = new RecordingHandler(request =>
        {
            Assert.Equal(HttpMethod.Delete, request.Method);
            Assert.Equal("/v1/vpn/sessions/session-404", request.RequestUri!.AbsolutePath);

            return new HttpResponseMessage(HttpStatusCode.NotFound)
            {
                Content = new StringContent("""
                    {
                      "session_id": "session-404",
                      "status": "not_found",
                      "disconnected_at_ms": 1700000000500
                    }
                    """),
            };
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var response = await client.DeleteVpnSessionAsync(" session-404 ");

        Assert.Equal("session-404", response.SessionId);
        Assert.Equal("not_found", response.Status);
        Assert.Equal((ulong)1700000000500, response.DisconnectedAtMilliseconds);
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
                        "id": "sorauロ1Nmerchant"
                      }
                    }
                    """),
            };
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        using var response = await client.SubmitSignedQueryAsync(queryBytes, query: "limit=1");

        Assert.Equal("Singular", response.RootElement.GetProperty("kind").GetString());
        Assert.Equal("sorauロ1Nmerchant", response.RootElement.GetProperty("value").GetProperty("id").GetString());
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
        seenEnvelope = new SignedQueryBuilder("sorauロ1NイリウdPBeシRoクQ2ヤgシQqeカヘスチhRW2コソZ9ユヲUナRX5NJYH53")
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
                      "account_id": "sorauロ1Nmerchant",
                      "resolved_at_ms": 1710000000000,
                      "expires_at_ms": 1710003600000,
                      "backend": "bfv-programmed-sha3-256-v1",
                      "signature": "ABCD",
                      "signature_payload_hex": "DEADBEEF",
                      "signature_payload": {
                        "policy_id": "phone#retail",
                        "account_id": "sorauロ1Nmerchant"
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
        Assert.Equal("sorauロ1Nmerchant", resolved.AccountId);
        Assert.Equal("bfv-programmed-sha3-256-v1", resolved.Backend);
        Assert.NotNull(resolved.SignaturePayload);
        Assert.Equal("phone#retail", resolved.SignaturePayload!["policy_id"]!.GetValue<string>());
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
                      "alias": "merchant@sbp.universal",
                      "account_id": "sorauロ1Nmerchant",
                      "source": "on_chain"
                    }
                    """),
            };
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var resolved = await client.ResolveAccountAliasIndexAsync(7);

        Assert.NotNull(resolved);
        Assert.Equal((ulong)7, resolved!.Index);
        Assert.Equal("merchant@sbp.universal", resolved.Alias);
        Assert.Equal("sorauロ1Nmerchant", resolved.AccountId);
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
                      "accounts": ["sorauロ1Nmerchant", "sorauロ1Nissuer"]
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
        Assert.Equal("sorauロ1Nmerchant", response.Dataspaces[0].Accounts[0]);
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
                      "accounts": ["sorauロ1Nmerchant"],
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
        var transaction = new TransactionBuilder("00000042", "sorauロ1NイリウdPBeシRoクQ2ヤgシQqeカヘスチhRW2コソZ9ユヲUナRX5NJYH53")
            .TransferAsset("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", "15.7500", "sorauロ1NイリウdPBeシRoクQ2ヤgシQqeカヘスチhRW2コソZ9ユヲUナRX5NJYH53")
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
            Assert.Equal("merchant@sbp", payload.RootElement.GetProperty("alias").GetString());
            Assert.Equal("sorauロ1Nmerchant", payload.RootElement.GetProperty("account_id").GetString());
            Assert.Equal("merchant@example.com", payload.RootElement.GetProperty("identity").GetProperty("email").GetString());

            return new HttpResponseMessage(HttpStatusCode.Accepted)
            {
                Content = new StringContent("""
                    {
                      "account_id": "sorauロ1Nmerchant",
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
            Alias = "merchant@sbp",
            AccountId = "sorauロ1Nmerchant",
            Identity = new JsonObject
            {
                ["email"] = "merchant@example.com",
            },
            Permissions = ["CanResolveAccountAlias"],
        });

        Assert.Equal("sorauロ1Nmerchant", response.AccountId);
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
            "sorauロ1Nmerchant",
            68,
            "d5c0016a6345e8ea379da42aab1fdc16ba82756e19e0b63c48c14735e8caf7ef");

        Assert.Equal(
            "8fedfb3e73b08653203dfedc046fe38e523503453d0efb639cfa0e9870550adf",
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
            "sorauロ1Nmerchant",
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

        Assert.Equal("000000000000008c", solution.NonceHex);
        Assert.Equal("00ddc37a0c89354df654f5a0f09a591d6bcd343ce23f5b621049be4ebac8f559", solution.DigestHex);
        Assert.Equal(8, solution.LeadingZeroBits);
        Assert.Equal(141, solution.Attempts);
    }

    [Fact]
    public async Task ClaimAccountFaucetAsyncPostsPowFieldsAndDeserializesQueuedResponse()
    {
        using var handler = new RecordingHandler(request =>
        {
            var payload = ReadBodyAsJson(request);
            Assert.Equal("/v1/accounts/faucet", request.RequestUri!.AbsolutePath);
            Assert.Equal("sorauロ1Nmerchant", payload.RootElement.GetProperty("account_id").GetString());
            Assert.Equal<ulong>(68, payload.RootElement.GetProperty("pow_anchor_height").GetUInt64());
            Assert.Equal("abcdef", payload.RootElement.GetProperty("pow_nonce_hex").GetString());

            return new HttpResponseMessage(HttpStatusCode.Accepted)
            {
                Content = new StringContent("""
                    {
                      "account_id": "sorauロ1Nmerchant",
                      "asset_definition_id": "rose#wonderland",
                      "asset_id": "rose#wonderland#sorauロ1Nmerchant",
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
            AccountId = "sorauロ1Nmerchant",
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
        var solution = await client.SolveAccountFaucetAsync("sorauロ1Nmerchant", new ToriiAccountFaucetSolveOptions { MaxAttempts = 200 });

        Assert.Equal("000000000000008c", solution.NonceHex);
        Assert.Equal("sorauロ1Nmerchant", solution.AccountId);
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
            Assert.Equal("sorauロ1Nmerchant", payload.RootElement.GetProperty("account_id").GetString());
            Assert.Equal("000000000000008c", payload.RootElement.GetProperty("pow_nonce_hex").GetString());
            Assert.Equal<ulong>(68, payload.RootElement.GetProperty("pow_anchor_height").GetUInt64());

            return new HttpResponseMessage(HttpStatusCode.Accepted)
            {
                Content = new StringContent("""
                    {
                      "account_id": "sorauロ1Nmerchant",
                      "asset_definition_id": "rose#wonderland",
                      "asset_id": "rose#wonderland#sorauロ1Nmerchant",
                      "amount": "100",
                      "tx_hash_hex": "feedface",
                      "status": "QUEUED"
                    }
                    """),
            };
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var response = await client.ClaimAccountFaucetAsync("sorauロ1Nmerchant", new ToriiAccountFaucetSolveOptions { MaxAttempts = 200 });

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
            Assert.Equal("treasury@sbp", payload.RootElement.GetProperty("alias").GetString());
            Assert.Equal(2, payload.RootElement.GetProperty("required_signers").GetInt32());
            Assert.Equal(2, payload.RootElement.GetProperty("member_weights")[1].GetInt32());

            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("""
                    {
                      "account_id": "sorauロ1Nmultisig",
                      "tx_hash_hex": "",
                      "status": "EXISTS"
                    }
                    """),
            };
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var response = await client.RegisterMultisigAccountAsync(new ToriiMultisigAccountOnboardingRequest
        {
            Alias = "treasury@sbp",
            RequiredSigners = 2,
            MemberAccountIds = ["sorauロ1Nmember1", "sorauロ1Nmember2"],
            MemberWeights = [1, 2],
            TransactionTtlMilliseconds = 60_000,
        });

        Assert.Equal("sorauロ1Nmultisig", response.AccountId);
        Assert.Equal(string.Empty, response.TransactionHashHex);
        Assert.Equal("EXISTS", response.Status);
    }

    [Fact]
    public async Task ResolveAccountAliasAsyncReturnsTypedResponse()
    {
        using var handler = new RecordingHandler(request =>
        {
            var payload = ReadBodyAsJson(request);
            Assert.Equal("merchant@sbp.universal", payload.RootElement.GetProperty("alias").GetString());

            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("""
                    {
                      "alias": "merchant@sbp.universal",
                      "account_id": "sorauロ1Nmerchant",
                      "index": 7,
                      "source": "on_chain"
                    }
                    """),
            };
        });

        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        var resolved = await client.ResolveAccountAliasAsync("merchant@sbp.universal");

        Assert.NotNull(resolved);
        Assert.Equal("merchant@sbp.universal", resolved!.Alias);
        Assert.Equal("sorauロ1Nmerchant", resolved.AccountId);
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
                  "resolved_multisig_account_id": "sorauロ1Nmultisig",
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
    public async Task ApproveMultisigContractCallAsyncDeserializesScaffoldResponse()
    {
        using var handler = new RecordingHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent("""
                {
                  "ok": true,
                  "resolved_multisig_account_id": "sorauロ1Nmultisig",
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
