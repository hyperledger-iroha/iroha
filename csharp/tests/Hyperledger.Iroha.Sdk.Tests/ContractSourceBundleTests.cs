using System.Net;
using System.Text.Json;
using Hyperledger.Iroha.Torii;
using Hyperledger.Iroha.Http;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class ContractSourceBundleTests
{
    [Fact]
    public void VerifiedSourceCompanionsRoundtripWithTheirFileIdentities()
    {
        var view = JsonSerializer.Deserialize<ToriiContractCodeView>("""
            {"network_id":"hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0","artifact_id":{"dataspace_id":0,"code_hash":"hash:BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB#ABA2"},"code_hash":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
             "permissions":[],"entrypoints":[],"warnings":[],"rendered_source_kind":"verified_source",
             "rendered_source_text":"seiyaku App { include \"view.ko\"; }",
             "source_files":[{"source_name":"view.ko","source_text":"view fn value() -> int { 7 }"}]}
            """)!;
        Assert.Equal("view.ko", Assert.Single(view.SourceFiles).SourceName);
        var roundtrip = JsonSerializer.Deserialize<ToriiContractCodeView>(JsonSerializer.Serialize(view))!;
        Assert.Equal(view.SourceFiles[0], Assert.Single(roundtrip.SourceFiles));
    }

    [Theory]
    [InlineData("../outside.ko")]
    [InlineData("/absolute.ko")]
    [InlineData("other/../app.ko")]
    public async Task InvalidCompanionPathsFailBeforeDispatch(string path)
    {
        using var handler = new NoDispatchHandler();
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        await Assert.ThrowsAnyAsync<ArgumentException>(() => client.SubmitContractVerifiedSourceJobAsync(new ContractArtifactId(0, new string('b', 64)), new ToriiContractVerifiedSourceSubmission {
                Language = "kotodama", SourceName = "app.ko", SourceText = "seiyaku App {}",
                Sources = new[] { new ToriiContractSourceFile { SourceName = path, SourceText = "state int total;" } },
            }, cancellationToken: TestContext.Current.CancellationToken));
        Assert.Equal(0, handler.Requests);
    }

    [Fact]
    public void VerifiedSourceLimitsDoNotConstrainGeneratedPseudoSource()
    {
        var view = new ToriiContractCodeView {
            NetworkId = NetworkId.Parse("hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"),
            ArtifactId = new ContractArtifactId(0, new string('b', 64)),
            CodeHash = new string('b', 64), RenderedSourceKind = "pseudo_source",
            RenderedSourceText = new string('x', 1024 * 1024 + 1),
        };
        Assert.NotEmpty(JsonSerializer.Serialize(view));
        Assert.Throws<JsonException>(() => JsonSerializer.Serialize(view with { RenderedSourceKind = "verified_source" }));
    }

    [Fact]
    public void SourceFileRequiresBothOriginalPathAndText()
    {
        Assert.Throws<JsonException>(() => JsonSerializer.Deserialize<ToriiContractSourceFile>("""{"source_name":"app.ko"}"""));
        Assert.Throws<JsonException>(() => JsonSerializer.Deserialize<ToriiContractSourceFile>("""{"source_text":""}"""));
    }

    private static ToriiContractVerifiedSourceSubmission Bundle()
    {
        return new ToriiContractVerifiedSourceSubmission
        {
            Language = "kotodama", SourceName = "src/../app.ko", SourceText = "seiyaku App {}",
            Imports = new[] { new ToriiContractSourceImport { Alias = "math", Package = "math@1.0.0" } },
            Packages = new[] {
                new ToriiContractSourcePackage {
                    Identity = "math@1.0.0",
                    Modules = new[] { new ToriiContractSourceFile { SourceName = "lib/../app.ko", SourceText = "module Math {}" } },
                    Sources = new[] { new ToriiContractSourceFile { SourceName = "include/part.ko", SourceText = "" } },
                    Exports = new[] { "Math::add" }, Imports = Array.Empty<ToriiContractSourceImport>(),
                },
            },
        };
    }

    [Fact]
    public async Task LockedSourceClosurePreservesOwnersAndNormalizesPaths()
    {
        using var handler = new RecordingHandler();
        using var credentials = new CanonicalRequestCredentials(
            "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53",
            Convert.FromHexString("616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032"));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler),
            new ToriiClientOptions {
                NetworkId = NetworkId.Parse("hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"),
                CanonicalRequestCredentials = credentials,
            }, TransactionSubmissionTransportAssurance.OneShotWithoutRedirectsOrRetries);
        await client.SubmitContractVerifiedSourceJobAsync(new ContractArtifactId(0, new string('b', 64)), Bundle(), cancellationToken: TestContext.Current.CancellationToken);
        using var body = JsonDocument.Parse(handler.Body!);
        Assert.Equal("app.ko", body.RootElement.GetProperty("source_name").GetString());
        Assert.Equal("math@1.0.0", body.RootElement.GetProperty("imports")[0].GetProperty("package").GetString());
        var package = body.RootElement.GetProperty("packages")[0];
        Assert.Equal("app.ko", package.GetProperty("modules")[0].GetProperty("source_name").GetString());
        Assert.Equal("include/part.ko", package.GetProperty("sources")[0].GetProperty("source_name").GetString());
        Assert.Equal("Math::add", package.GetProperty("exports")[0].GetString());

        var view = new ToriiContractCodeView {
            NetworkId = NetworkId.Parse("hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"),
            ArtifactId = new ContractArtifactId(0, new string('b', 64)),
            CodeHash = new string('b', 64), RenderedSourceKind = "verified_source", RenderedSourceText = Bundle().SourceText,
            SourceImports = Bundle().Imports, SourcePackages = Bundle().Packages,
        };
        var restored = JsonSerializer.Deserialize<ToriiContractCodeView>(JsonSerializer.Serialize(view))!;
        Assert.Equal("math", restored.SourceImports.Single().Alias);
        Assert.Equal("math@1.0.0", restored.SourcePackages.Single().Identity);
        Assert.Equal("lib/../app.ko", restored.SourcePackages.Single().Modules.Single().SourceName);
    }

    [Fact]
    public async Task InvalidLockedClosuresFailBeforeDispatch()
    {
        var valid = Bundle();
        var package = valid.Packages.Single();
        var megabyte = new string('x', 1024 * 1024);
        var invalid = new[] {
            valid with { SourceName = null },
            valid with { SourceText = megabyte + "x" },
            valid with { Packages = new[] { package, package } },
            valid with { Imports = new[] { valid.Imports[0], valid.Imports[0] } },
            valid with { Packages = Array.Empty<ToriiContractSourcePackage>() },
            valid with { Packages = new[] { package with { Exports = new[] { "Math::add", "Math::add" } } } },
            valid with { Packages = new[] { package with { Imports = new[] { valid.Imports[0], valid.Imports[0] } } } },
            valid with { Packages = new[] { package with { Sources = new[] { new ToriiContractSourceFile { SourceName = "app.ko", SourceText = "" } } } } },
            valid with { Sources = Enumerable.Range(0, 512).Select(index => new ToriiContractSourceFile { SourceName = $"{index}.ko", SourceText = "" }).ToArray() },
            valid with { Sources = Enumerable.Range(0, 16).Select(index => new ToriiContractSourceFile { SourceName = $"{index}.ko", SourceText = megabyte }).ToArray() },
        };
        using var handler = new NoDispatchHandler();
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));
        foreach (var request in invalid)
            await Assert.ThrowsAnyAsync<ArgumentException>(() => client.SubmitContractVerifiedSourceJobAsync(new ContractArtifactId(0, new string('b', 64)), request, cancellationToken: TestContext.Current.CancellationToken));
        Assert.Equal(0, handler.Requests);
    }

    [Fact]
    public void CodeViewRejectsDuplicateNormalizedPackageFiles()
    {
        var request = Bundle();
        var package = request.Packages.Single();
        var view = new ToriiContractCodeView {
            NetworkId = NetworkId.Parse("hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"),
            ArtifactId = new ContractArtifactId(0, new string('b', 64)),
            CodeHash = new string('b', 64), RenderedSourceKind = "verified_source", RenderedSourceText = request.SourceText,
            SourceImports = request.Imports,
            SourcePackages = new[] { package with { Sources = new[] { new ToriiContractSourceFile { SourceName = "app.ko", SourceText = "" } } } },
        };
        Assert.Throws<JsonException>(() => JsonSerializer.Serialize(view));
    }

    private sealed class RecordingHandler : HttpMessageHandler
    {
        public string? Body { get; private set; }
        protected override async Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
        {
            Body = await request.Content!.ReadAsStringAsync(cancellationToken);
            return new HttpResponseMessage(HttpStatusCode.OK) {
                Content = new StringContent("""
                    {"network_id":"hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0","artifact_id":{"dataspace_id":0,"code_hash":"hash:BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB#ABA2"},"job_id":"job-1","code_hash":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                     "status":"queued","submitted_at":"2026-09-30T00:00:00Z"}
                    """),
            };
        }
    }

    private sealed class NoDispatchHandler : HttpMessageHandler
    {
        public int Requests { get; private set; }
        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
        {
            Requests++;
            return Task.FromResult(new HttpResponseMessage(HttpStatusCode.InternalServerError));
        }
    }
}
