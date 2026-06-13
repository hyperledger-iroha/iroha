using System.Text;
using Hyperledger.Iroha.Http;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class CanonicalRequestTests
{
    private const string FixtureAccountId =
        "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53";

    private static readonly byte[] FixturePrivateKeySeed =
        Convert.FromHexString("616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032");

    [Fact]
    public void CanonicalQueryStringSortsAndEncodes()
    {
        var canonical = CanonicalRequest.BuildCanonicalQueryString("gas_units=100&cursor_mode=stored&note=hello world");
        Assert.Equal("cursor_mode=stored&gas_units=100&note=hello+world", canonical);
    }

    [Fact]
    public void BuildHeadersMatchesDeterministicNodeVector()
    {
        var body = Encoding.UTF8.GetBytes("{\"selector\":\"assets\"}");
        var headers = CanonicalRequest.BuildHeaders(
            accountId: FixtureAccountId,
            privateKeySeed: FixturePrivateKeySeed,
            method: "post",
            path: "/v1/query",
            query: "gas_units=100&cursor_mode=stored",
            body: body,
            timestampMs: 1735000000123,
            nonce: "abcdef0123456789abcdef0123456789");

        Assert.Equal("RdaUygjFPFHDlzL5VQpz0m5L5MYN1MDJzY4I87+6LgzxA3VrnoAmSWqfrvgh2+tB2+pqqyZVEVstNZN86Px1Cw==", headers.SignatureBase64);
        Assert.Equal(FixtureAccountId, headers.AccountId);
        Assert.Equal(1735000000123, headers.TimestampMs);
        Assert.Equal("abcdef0123456789abcdef0123456789", headers.Nonce);
    }

    [Fact]
    public void CanonicalRequestAuthRejectsPaddedAndBlankFields()
    {
        Assert.Throws<ArgumentException>(
            () => new CanonicalRequestCredentials($" {FixtureAccountId}", FixturePrivateKeySeed));
        Assert.Throws<ArgumentException>(
            () => CanonicalRequest.BuildHeaders(
                accountId: $"{FixtureAccountId} ",
                privateKeySeed: FixturePrivateKeySeed,
                method: "post",
                path: "/v1/query",
                nonce: "abcdef0123456789abcdef0123456789"));
        Assert.Throws<ArgumentException>(
            () => CanonicalRequest.BuildHeaders(
                accountId: FixtureAccountId,
                privateKeySeed: FixturePrivateKeySeed,
                method: "post",
                path: "/v1/query",
                nonce: " abcdef0123456789abcdef0123456789 "));
        Assert.Throws<ArgumentException>(
            () => CanonicalRequest.BuildHeaders(
                accountId: FixtureAccountId,
                privateKeySeed: FixturePrivateKeySeed,
                method: "post",
                path: "/v1/query",
                nonce: " "));
        Assert.Throws<ArgumentException>(
            () => CanonicalRequest.BuildSignatureMessage(
                "post",
                "/v1/query",
                timestampMs: 1735000000123,
                nonce: " abcdef0123456789abcdef0123456789 "));
        Assert.Throws<ArgumentException>(
            () => new CanonicalRequestHeaders(
                $" {FixtureAccountId}",
                "signature",
                1735000000123,
                "abcdef0123456789abcdef0123456789"));
        Assert.Throws<ArgumentException>(
            () => new CanonicalRequestHeaders(
                FixtureAccountId,
                " signature ",
                1735000000123,
                "abcdef0123456789abcdef0123456789"));
        Assert.Throws<ArgumentException>(
            () => new CanonicalRequestHeaders(
                FixtureAccountId,
                "signature",
                1735000000123,
                " abcdef0123456789abcdef0123456789 "));
    }
}
