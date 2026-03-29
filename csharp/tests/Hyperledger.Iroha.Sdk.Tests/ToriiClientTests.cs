using System.Net;
using System.Text.Json;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Http;
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
