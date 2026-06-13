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

    [Theory]
    [InlineData("")]
    [InlineData(" ")]
    [InlineData(" sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")]
    [InlineData("sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53 ")]
    [InlineData("\tsorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")]
    [InlineData("sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53\n")]
    [InlineData("\u00A0sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")]
    [InlineData("sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53\u00A0")]
    [InlineData("sorauﾛ1N\u0000ｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")]
    [InlineData("sorauﾛ1N\u001FｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")]
    [InlineData("sorauﾛ1N\u007FｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")]
    public void BuildHeadersRejectsNonExactAccountIds(string accountId)
    {
        Assert.Throws<ArgumentException>(() => CanonicalRequest.BuildHeaders(
            accountId: accountId,
            privateKeySeed: FixturePrivateKeySeed,
            method: "post",
            path: "/v1/query",
            body: Encoding.UTF8.GetBytes("{}"),
            timestampMs: 1735000000123,
            nonce: "abcdef0123456789abcdef0123456789"));
    }

    [Theory]
    [InlineData("")]
    [InlineData(" ")]
    [InlineData(" post")]
    [InlineData("post ")]
    [InlineData("\tpost")]
    [InlineData("post\n")]
    [InlineData("\u00A0post")]
    [InlineData("po\u0000st")]
    [InlineData("po\u001Fst")]
    public void BuildHeadersRejectsNonExactMethods(string method)
    {
        Assert.Throws<ArgumentException>(() => CanonicalRequest.BuildHeaders(
            accountId: FixtureAccountId,
            privateKeySeed: FixturePrivateKeySeed,
            method: method,
            path: "/v1/query",
            body: Encoding.UTF8.GetBytes("{}"),
            timestampMs: 1735000000123,
            nonce: "abcdef0123456789abcdef0123456789"));
    }

    [Theory]
    [InlineData("")]
    [InlineData(" ")]
    [InlineData(" /v1/query")]
    [InlineData("/v1/query ")]
    [InlineData("\t/v1/query")]
    [InlineData("/v1/query\n")]
    [InlineData("\u00A0/v1/query")]
    [InlineData("/v1\u0000/query")]
    [InlineData("/v1\u001F/query")]
    public void BuildHeadersRejectsNonExactPaths(string path)
    {
        Assert.Throws<ArgumentException>(() => CanonicalRequest.BuildHeaders(
            accountId: FixtureAccountId,
            privateKeySeed: FixturePrivateKeySeed,
            method: "post",
            path: path,
            body: Encoding.UTF8.GetBytes("{}"),
            timestampMs: 1735000000123,
            nonce: "abcdef0123456789abcdef0123456789"));
    }

    [Theory]
    [InlineData("")]
    [InlineData(" post")]
    [InlineData("post ")]
    [InlineData("po\u0000st")]
    public void BuildMessageRejectsNonExactMethods(string method)
    {
        Assert.Throws<ArgumentException>(() => CanonicalRequest.BuildMessage(
            method: method,
            path: "/v1/query",
            body: Encoding.UTF8.GetBytes("{}")));
    }

    [Theory]
    [InlineData("")]
    [InlineData(" /v1/query")]
    [InlineData("/v1/query ")]
    [InlineData("/v1\u0000/query")]
    public void BuildMessageRejectsNonExactPaths(string path)
    {
        Assert.Throws<ArgumentException>(() => CanonicalRequest.BuildMessage(
            method: "post",
            path: path,
            body: Encoding.UTF8.GetBytes("{}")));
    }

    [Theory]
    [InlineData("")]
    [InlineData(" ")]
    [InlineData(" abcdef0123456789abcdef0123456789")]
    [InlineData("abcdef0123456789abcdef0123456789 ")]
    [InlineData("\tabcdef0123456789abcdef0123456789")]
    [InlineData("abcdef0123456789abcdef0123456789\n")]
    [InlineData("\u00A0abcdef0123456789abcdef0123456789")]
    [InlineData("abcdef0123456789abcdef0123456789\u00A0")]
    [InlineData("abcdef0123456789\u0000abcdef0123456789")]
    [InlineData("abcdef0123456789\u001Fabcdef0123456789")]
    [InlineData("abcdef0123456789\u007Fabcdef0123456789")]
    public void BuildHeadersRejectsNonExactCallerProvidedNonces(string nonce)
    {
        Assert.Throws<ArgumentException>(() => CanonicalRequest.BuildHeaders(
            accountId: FixtureAccountId,
            privateKeySeed: FixturePrivateKeySeed,
            method: "post",
            path: "/v1/query",
            body: Encoding.UTF8.GetBytes("{}"),
            timestampMs: 1735000000123,
            nonce: nonce));
    }

    [Theory]
    [InlineData("")]
    [InlineData(" ")]
    [InlineData(" abcdef0123456789abcdef0123456789")]
    [InlineData("abcdef0123456789abcdef0123456789 ")]
    [InlineData("abcdef0123456789\u0000abcdef0123456789")]
    public void BuildSignatureMessageRejectsNonExactCallerProvidedNonces(string nonce)
    {
        Assert.Throws<ArgumentException>(() => CanonicalRequest.BuildSignatureMessage(
            method: "post",
            path: "/v1/query",
            body: Encoding.UTF8.GetBytes("{}"),
            timestampMs: 1735000000123,
            nonce: nonce));
    }

    [Theory]
    [InlineData(" post", "/v1/query")]
    [InlineData("post ", "/v1/query")]
    [InlineData("post", " /v1/query")]
    [InlineData("post", "/v1/query ")]
    [InlineData("po\u0000st", "/v1/query")]
    [InlineData("post", "/v1\u0000/query")]
    public void BuildSignatureMessageRejectsNonExactMethodsAndPaths(string method, string path)
    {
        Assert.Throws<ArgumentException>(() => CanonicalRequest.BuildSignatureMessage(
            method: method,
            path: path,
            body: Encoding.UTF8.GetBytes("{}"),
            timestampMs: 1735000000123,
            nonce: "abcdef0123456789abcdef0123456789"));
    }

    [Theory]
    [InlineData("")]
    [InlineData(" ")]
    [InlineData(" sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")]
    [InlineData("sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53 ")]
    [InlineData("sorauﾛ1N\u0000ｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")]
    [InlineData("sorauﾛ1N\u001FｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")]
    public void CanonicalRequestCredentialsRejectsNonExactAccountIds(string accountId)
    {
        Assert.Throws<ArgumentException>(() => new CanonicalRequestCredentials(accountId, FixturePrivateKeySeed));
    }
}
