using System.Text;
using Hyperledger.Iroha.Http;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class CanonicalRequestTests
{
    [Fact]
    public void CanonicalQueryStringSortsAndEncodes()
    {
        var canonical = CanonicalRequest.BuildCanonicalQueryString("gas_units=100&cursor_mode=stored&note=hello world");
        Assert.Equal("cursor_mode=stored&gas_units=100&note=hello+world", canonical);
    }

    [Fact]
    public void BuildHeadersMatchesDeterministicNodeVector()
    {
        var privateKeySeed = Convert.FromHexString("616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032");
        var body = Encoding.UTF8.GetBytes("{\"selector\":\"assets\"}");
        var headers = CanonicalRequest.BuildHeaders(
            accountId: "sorauロ1NイリウdPBeシRoクQ2ヤgシQqeカヘスチhRW2コソZ9ユヲUナRX5NJYH53",
            privateKeySeed: privateKeySeed,
            method: "post",
            path: "/v1/query",
            query: "gas_units=100&cursor_mode=stored",
            body: body,
            timestampMs: 1735000000123,
            nonce: "abcdef0123456789abcdef0123456789");

        Assert.Equal("RdaUygjFPFHDlzL5VQpz0m5L5MYN1MDJzY4I87+6LgzxA3VrnoAmSWqfrvgh2+tB2+pqqyZVEVstNZN86Px1Cw==", headers.SignatureBase64);
        Assert.Equal(1735000000123, headers.TimestampMs);
        Assert.Equal("abcdef0123456789abcdef0123456789", headers.Nonce);
    }
}
