using System.Text;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Http;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class CanonicalRequestTests
{
    private const string FixtureAccountId =
        "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53";

    private const string FixtureSignatureBase64 =
        "ZDp26XaHf2XldnSLiM5ICzmrQS4lCkc3jO5vpJBmLsCR9ivR8dF4ll14L6+m9tPX429ovNYG6mZYc6ggql0kDQ==";

    private static readonly byte[] FixturePrivateKeySeed =
        Convert.FromHexString("616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032");

    private static readonly NetworkId FixtureNetworkId = NetworkId.Parse(
        "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0");

    [Fact]
    public void CanonicalQueryStringSortsAndEncodes()
    {
        var canonical = CanonicalRequest.BuildCanonicalQueryString("gas_units=100&cursor_mode=stored&note=hello world");
        Assert.Equal("cursor_mode=stored&gas_units=100&note=hello+world", canonical);
    }

    [Fact]
    public void CanonicalQueryStringAcceptsValidPercentEscapes()
    {
        var canonical = CanonicalRequest.BuildCanonicalQueryString("slash=%2f&note=hello%20world");
        Assert.Equal("note=hello+world&slash=%2F", canonical);
    }

    [Theory]
    [InlineData("note=%")]
    [InlineData("note=%2")]
    [InlineData("note=%GG")]
    [InlineData("%GG=value")]
    [InlineData("note=%FF")]
    [InlineData("note=%C3%28")]
    [InlineData("note=%00")]
    [InlineData("note=%1F")]
    [InlineData("note=line%0Abreak")]
    public void CanonicalQueryStringRejectsMalformedOrControlPercentEscapes(string query)
    {
        Assert.Throws<ArgumentException>(() => CanonicalRequest.BuildCanonicalQueryString(query));
    }

    [Theory]
    [InlineData("?")]
    [InlineData("&gas=1")]
    [InlineData("gas=1&")]
    [InlineData("gas=1&&cursor=2")]
    [InlineData("=value")]
    [InlineData("%20=value")]
    [InlineData("gas=1&%20=value")]
    [InlineData("?&gas=1")]
    [InlineData("?=value")]
    public void CanonicalQueryStringRejectsAmbiguousSegmentsAndBlankNames(string query)
    {
        Assert.Throws<ArgumentException>(() => CanonicalRequest.BuildCanonicalQueryString(query));
    }

    [Fact]
    public void BuildMessageAcceptsHttpTokenMethodCharacters()
    {
        var message = Encoding.UTF8.GetString(CanonicalRequest.BuildMessage(
            method: "m-search+safe",
            path: "/v1/query"));

        Assert.StartsWith("M-SEARCH+SAFE\n/v1/query\n", message, StringComparison.Ordinal);
    }

    [Fact]
    public void BuildHeadersMatchesDeterministicNodeVector()
    {
        var body = Encoding.UTF8.GetBytes("{\"selector\":\"assets\"}");
        var headers = CanonicalRequest.BuildHeaders(
            FixtureNetworkId,
            accountId: FixtureAccountId,
            privateKeySeed: FixturePrivateKeySeed,
            method: "post",
            path: "/v1/query",
            query: "gas_units=100&cursor_mode=stored",
            body: body,
            timestampMs: 1735000000123,
            nonce: "abcdef0123456789abcdef0123456789");

        Assert.Equal(FixtureSignatureBase64, headers.SignatureBase64);
        Assert.Equal(FixtureAccountId, headers.AccountId);
        Assert.Equal(1735000000123, headers.TimestampMs);
        Assert.Equal("abcdef0123456789abcdef0123456789", headers.Nonce);
    }

    [Fact]
    public void BuildHeadersGeneratesCanonicalNonce()
    {
        var headers = CanonicalRequest.BuildHeaders(
            FixtureNetworkId,
            accountId: FixtureAccountId,
            privateKeySeed: FixturePrivateKeySeed,
            method: "post",
            path: "/v1/query",
            body: Encoding.UTF8.GetBytes("{}"));

        Assert.Equal(32, headers.Nonce.Length);
        Assert.All(headers.Nonce, static value =>
            Assert.True(value is >= '0' and <= '9' or >= 'a' and <= 'f'));
    }

    [Fact]
    public void CanonicalRequestCredentialsSnapshotsPrivateKeySeedConstructorAndGetterBytes()
    {
        var seed = FixturePrivateKeySeed.ToArray();
        var credentials = new CanonicalRequestCredentials(FixtureAccountId, seed);
        var expectedSeed = credentials.PrivateKeySeed;
        var expectedSignature = CanonicalRequest.BuildHeaders(
            FixtureNetworkId,
            credentials.AccountId,
            credentials.PrivateKeySeed,
            "post",
            "/v1/query",
            timestampMs: 1735000000123,
            nonce: "abcdef0123456789abcdef0123456789").SignatureBase64;

        seed[0] ^= 0xff;
        var returnedSeed = credentials.PrivateKeySeed;
        returnedSeed[1] ^= 0xff;

        Assert.Equal(expectedSeed, credentials.PrivateKeySeed);
        Assert.NotSame(returnedSeed, credentials.PrivateKeySeed);

        var headers = CanonicalRequest.BuildHeaders(
            FixtureNetworkId,
            credentials.AccountId,
            credentials.PrivateKeySeed,
            "post",
            "/v1/query",
            timestampMs: 1735000000123,
            nonce: "abcdef0123456789abcdef0123456789");
        Assert.Equal(expectedSignature, headers.SignatureBase64);
    }

    [Fact]
    public void Ed25519KeyPairSnapshotsSeedAndPublicKeyBytes()
    {
        var seed = FixturePrivateKeySeed.ToArray();
        var expectedPublicKey = Ed25519Signer.GetPublicKey(seed);
        var keyPair = Ed25519KeyPair.FromSeed(seed);
        var expectedSeed = keyPair.PrivateKeySeed;
        var expectedAddress = keyPair.ToAccountAddress().ToI105();

        seed[0] ^= 0xff;
        var returnedSeed = keyPair.PrivateKeySeed;
        returnedSeed[1] ^= 0xff;
        var returnedPublicKey = keyPair.PublicKey;
        returnedPublicKey[2] ^= 0xff;

        Assert.Equal(expectedSeed, keyPair.PrivateKeySeed);
        Assert.Equal(expectedPublicKey, keyPair.PublicKey);
        Assert.Equal(expectedAddress, keyPair.ToAccountAddress().ToI105());
        Assert.NotSame(returnedSeed, keyPair.PrivateKeySeed);
        Assert.NotSame(returnedPublicKey, keyPair.PublicKey);
    }

    [Theory]
    [InlineData(0)]
    [InlineData(1)]
    [InlineData(31)]
    [InlineData(33)]
    [InlineData(64)]
    public void Ed25519SeedConsumersRejectInvalidPrivateKeySeedLengths(int seedLength)
    {
        var seed = new byte[seedLength];

        AssertArgumentException("privateKeySeed", () => Ed25519Signer.GetPublicKey(seed));
        AssertArgumentException("privateKeySeed", () => Ed25519Signer.Sign([], seed));
        AssertArgumentException("privateKeySeed", () => Ed25519KeyPair.FromSeed(seed));
        AssertArgumentException("privateKeySeed", () => new CanonicalRequestCredentials(FixtureAccountId, seed));
        AssertArgumentException(
            "privateKeySeed",
            () => CanonicalRequest.BuildHeaders(
            FixtureNetworkId,
                accountId: FixtureAccountId,
                privateKeySeed: seed,
                method: "post",
                path: "/v1/query",
                timestampMs: 1735000000123,
                nonce: "abcdef0123456789abcdef0123456789"));
    }

    [Fact]
    public void Ed25519SignerSnapshotsTemporaryInputsWithoutMutatingCallerBuffers()
    {
        var seed = FixturePrivateKeySeed.ToArray();
        var seedSnapshot = seed.ToArray();
        var message = Encoding.UTF8.GetBytes("canonical request signer caller-owned message");
        var messageSnapshot = message.ToArray();

        var publicKey = Ed25519Signer.GetPublicKey(seed);
        var publicKeySnapshot = publicKey.ToArray();
        var signature = Ed25519Signer.Sign(message, seed);
        var signatureSnapshot = signature.ToArray();
        var verified = Ed25519Signer.Verify(message, signature, publicKey);

        Assert.True(verified);
        Assert.Equal(seedSnapshot, seed);
        Assert.Equal(messageSnapshot, message);
        Assert.Equal(publicKeySnapshot, publicKey);
        Assert.Equal(signatureSnapshot, signature);
    }

    [Theory]
    [InlineData(0, 32, "signature")]
    [InlineData(63, 32, "signature")]
    [InlineData(65, 32, "signature")]
    [InlineData(64, 0, "publicKey")]
    [InlineData(64, 31, "publicKey")]
    [InlineData(64, 33, "publicKey")]
    public void Ed25519VerifyRejectsInvalidSignatureAndPublicKeyLengths(
        int signatureLength,
        int publicKeyLength,
        string expectedParamName)
    {
        var error = Assert.Throws<ArgumentException>(() => Ed25519Signer.Verify(
            Encoding.UTF8.GetBytes("message"),
            new byte[signatureLength],
            new byte[publicKeyLength]));

        Assert.Equal(expectedParamName, error.ParamName);
    }

    [Fact]
    public void CanonicalRequestSigningRejectsPrivateKeysThatDoNotDeriveAccount()
    {
        var mismatchedSeed = FixturePrivateKeySeed.ToArray();
        mismatchedSeed[0] ^= 0xff;
        var mismatchedAccountId = Ed25519KeyPair.FromSeed(mismatchedSeed)
            .ToAccountAddress()
            .ToI105();

        Assert.NotEqual(FixtureAccountId, mismatchedAccountId);
        AssertArgumentException("accountId", () => new CanonicalRequestCredentials(FixtureAccountId, mismatchedSeed));
        AssertArgumentException(
            "accountId",
            () => CanonicalRequest.BuildHeaders(
            FixtureNetworkId,
                accountId: FixtureAccountId,
                privateKeySeed: mismatchedSeed,
                method: "post",
                path: "/v1/query",
                timestampMs: 1735000000123,
                nonce: "abcdef0123456789abcdef0123456789"));
    }

    [Fact]
    public void CanonicalRequestAuthAcceptsPositiveTimestampBoundary()
    {
        var headers = CanonicalRequest.BuildHeaders(
            FixtureNetworkId,
            accountId: FixtureAccountId,
            privateKeySeed: FixturePrivateKeySeed,
            method: "post",
            path: "/v1/query",
            timestampMs: 1,
            nonce: "abcdef0123456789abcdef0123456789");
        var message = CanonicalRequest.BuildSignatureMessage(
            FixtureNetworkId,
            "post",
            "/v1/query",
            timestampMs: 1,
            nonce: "abcdef0123456789abcdef0123456789");
        var manualHeaders = new CanonicalRequestHeaders(
            FixtureAccountId,
            headers.SignatureBase64,
            1,
            headers.Nonce);

        Assert.Equal(1, headers.TimestampMs);
        Assert.Contains("\n1\n", Encoding.UTF8.GetString(message), StringComparison.Ordinal);
        Assert.Equal("1", manualHeaders.ToDictionary()["X-Iroha-Timestamp-Ms"]);
    }

    [Fact]
    public void CanonicalRequestAuthCannotReplayAcrossSameLabelNetworks()
    {
        var foreignNetworkId = NetworkId.Parse(
            "hash:0E5751C026E543B2E8AB2EB06099DAA1D1E5DF47778F7787FAAB45CDF12FE3A9#6A22");
        var canonical = CanonicalRequest.BuildSignatureMessage(
            FixtureNetworkId,
            "GET",
            "/v1/accounts",
            query: "label=same",
            timestampMs: 1735000000123,
            nonce: "abcdef0123456789abcdef0123456789");
        var foreign = CanonicalRequest.BuildSignatureMessage(
            foreignNetworkId,
            "GET",
            "/v1/accounts",
            query: "label=same",
            timestampMs: 1735000000123,
            nonce: "abcdef0123456789abcdef0123456789");

        Assert.NotEqual(canonical, foreign);
    }

    [Theory]
    [InlineData(-1L)]
    [InlineData(0L)]
    public void CanonicalRequestAuthRejectsNonPositiveTimestamps(long timestampMs)
    {
        Assert.Throws<ArgumentOutOfRangeException>(() => CanonicalRequest.BuildHeaders(
            FixtureNetworkId,
            accountId: FixtureAccountId,
            privateKeySeed: FixturePrivateKeySeed,
            method: "post",
            path: "/v1/query",
            timestampMs: timestampMs,
            nonce: "abcdef0123456789abcdef0123456789"));
        Assert.Throws<ArgumentOutOfRangeException>(() => CanonicalRequest.BuildSignatureMessage(
            FixtureNetworkId,
            method: "post",
            path: "/v1/query",
            timestampMs: timestampMs,
            nonce: "abcdef0123456789abcdef0123456789"));
        Assert.Throws<ArgumentOutOfRangeException>(() => new CanonicalRequestHeaders(
            FixtureAccountId,
            FixtureSignatureBase64,
            timestampMs,
            "abcdef0123456789abcdef0123456789"));
    }

    [Fact]
    public void CanonicalRequestAuthRejectsPaddedAndBlankFields()
    {
        Assert.Throws<ArgumentException>(
            () => new CanonicalRequestCredentials($" {FixtureAccountId}", FixturePrivateKeySeed));
        Assert.Throws<ArgumentException>(
            () => CanonicalRequest.BuildHeaders(
            FixtureNetworkId,
                accountId: $"{FixtureAccountId} ",
                privateKeySeed: FixturePrivateKeySeed,
                method: "post",
                path: "/v1/query",
                nonce: "abcdef0123456789abcdef0123456789"));
        Assert.Throws<ArgumentException>(
            () => CanonicalRequest.BuildHeaders(
            FixtureNetworkId,
                accountId: FixtureAccountId,
                privateKeySeed: FixturePrivateKeySeed,
                method: "post",
                path: "/v1/query",
                nonce: " abcdef0123456789abcdef0123456789 "));
        Assert.Throws<ArgumentException>(
            () => CanonicalRequest.BuildHeaders(
            FixtureNetworkId,
                accountId: FixtureAccountId,
                privateKeySeed: FixturePrivateKeySeed,
                method: "post",
                path: "/v1/query",
                nonce: " "));
        Assert.Throws<ArgumentException>(
            () => CanonicalRequest.BuildSignatureMessage(
            FixtureNetworkId,
                "post",
                "/v1/query",
                timestampMs: 1735000000123,
                nonce: " abcdef0123456789abcdef0123456789 "));
        Assert.Throws<ArgumentException>(
            () => new CanonicalRequestHeaders(
                $" {FixtureAccountId}",
                FixtureSignatureBase64,
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
                FixtureSignatureBase64,
                1735000000123,
                " abcdef0123456789abcdef0123456789 "));
    }

    [Fact]
    public void CanonicalRequestAuthRejectsInternalWhitespaceFields()
    {
        AssertArgumentException(
            "accountId",
            () => new CanonicalRequestCredentials(FixtureAccountId.Insert(8, " "), FixturePrivateKeySeed));
        AssertArgumentException(
            "accountId",
            () => CanonicalRequest.BuildHeaders(
            FixtureNetworkId,
                accountId: FixtureAccountId.Insert(8, "\u00A0"),
                privateKeySeed: FixturePrivateKeySeed,
                method: "post",
                path: "/v1/query",
                nonce: "abcdef0123456789abcdef0123456789"));
        AssertArgumentException(
            "method",
            () => CanonicalRequest.BuildHeaders(
            FixtureNetworkId,
                accountId: FixtureAccountId,
                privateKeySeed: FixturePrivateKeySeed,
                method: "po st",
                path: "/v1/query",
                nonce: "abcdef0123456789abcdef0123456789"));
        AssertArgumentException(
            "path",
            () => CanonicalRequest.BuildHeaders(
            FixtureNetworkId,
                accountId: FixtureAccountId,
                privateKeySeed: FixturePrivateKeySeed,
                method: "post",
                path: "/v1/qu ery",
                nonce: "abcdef0123456789abcdef0123456789"));
        AssertArgumentException(
            "nonce",
            () => CanonicalRequest.BuildHeaders(
            FixtureNetworkId,
                accountId: FixtureAccountId,
                privateKeySeed: FixturePrivateKeySeed,
                method: "post",
                path: "/v1/query",
                nonce: "abcdef0123456789 abcdef0123456789"));
        AssertArgumentException(
            "method",
            () => CanonicalRequest.BuildMessage("po st", "/v1/query"));
        AssertArgumentException(
            "path",
            () => CanonicalRequest.BuildMessage("post", "/v1/qu ery"));
        AssertArgumentException(
            "nonce",
            () => CanonicalRequest.BuildSignatureMessage(
            FixtureNetworkId,
                "post",
                "/v1/query",
                timestampMs: 1735000000123,
                nonce: "abcdef0123456789\u00A0abcdef0123456789"));
        AssertArgumentException(
            "signatureBase64",
            () => new CanonicalRequestHeaders(
                FixtureAccountId,
                "sig nature",
                1735000000123,
                "abcdef0123456789abcdef0123456789"));
        AssertArgumentException(
            "nonce",
            () => new CanonicalRequestHeaders(
                FixtureAccountId,
                FixtureSignatureBase64,
                1735000000123,
                "abcdef0123456789 abcdef0123456789"));
    }

    [Theory]
    [InlineData("signature", "base64")]
    [InlineData("AR==", "canonical base64")]
    [InlineData("AA==", "64 bytes")]
    public void CanonicalRequestHeadersRejectMalformedSignatureBase64(
        string signatureBase64,
        string expectedMessage)
    {
        var error = Assert.Throws<ArgumentException>(() => new CanonicalRequestHeaders(
            FixtureAccountId,
            signatureBase64,
            1735000000123,
            "abcdef0123456789abcdef0123456789"));

        Assert.Equal("signatureBase64", error.ParamName);
        Assert.Contains(expectedMessage, error.Message);
    }

    [Theory]
    [InlineData("")]
    [InlineData(" ")]
    [InlineData("abcdef0123456789abcdef012345678")]
    [InlineData("abcdef0123456789abcdef01234567890")]
    [InlineData("ABCDEF0123456789abcdef0123456789")]
    [InlineData("abcdef0123456789abcdef01234567g")]
    [InlineData("abcdef0123456789abcdef01234567-")]
    [InlineData("abcdef0123456789abcdef01234567_")]
    public void CanonicalRequestHeadersRejectsNonCanonicalNonces(string nonce)
    {
        var error = Assert.Throws<ArgumentException>(() => new CanonicalRequestHeaders(
            FixtureAccountId,
            FixtureSignatureBase64,
            1735000000123,
            nonce));

        Assert.Equal("nonce", error.ParamName);
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
    [InlineData("merchant@sora")]
    [InlineData("0x0a00012022d3c25e96fa1178ae08b3d30081a31a0d09e8f7321b1e015140cd37b332109ca")]
    [InlineData("n753uﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")]
    public void BuildHeadersRejectsNonExactAccountIds(string accountId)
    {
        Assert.Throws<ArgumentException>(() => CanonicalRequest.BuildHeaders(
            FixtureNetworkId,
            accountId: accountId,
            privateKeySeed: FixturePrivateKeySeed,
            method: "post",
            path: "/v1/query",
            body: Encoding.UTF8.GetBytes("{}"),
            timestampMs: 1735000000123,
            nonce: "abcdef0123456789abcdef0123456789"));
        Assert.Throws<ArgumentException>(() => new CanonicalRequestHeaders(
            accountId,
            FixtureSignatureBase64,
            1735000000123,
            "abcdef0123456789abcdef0123456789"));
    }

    [Theory]
    [InlineData("")]
    [InlineData(" ")]
    [InlineData(" post")]
    [InlineData("post ")]
    [InlineData("\tpost")]
    [InlineData("post\n")]
    [InlineData("\u00A0post")]
    [InlineData("po st")]
    [InlineData("po\u00A0st")]
    [InlineData("po\u0000st")]
    [InlineData("po\u001Fst")]
    [InlineData("post/get")]
    [InlineData("post:get")]
    [InlineData("post?get")]
    [InlineData("post@get")]
    [InlineData("post,get")]
    [InlineData("post;get")]
    [InlineData("post(get)")]
    [InlineData("post[get]")]
    [InlineData("post\\get")]
    [InlineData("post\"get")]
    [InlineData("post\u007Fget")]
    [InlineData("posté")]
    public void BuildHeadersRejectsNonExactMethods(string method)
    {
        Assert.Throws<ArgumentException>(() => CanonicalRequest.BuildHeaders(
            FixtureNetworkId,
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
    [InlineData("/v1/qu ery")]
    [InlineData("/v1/qu\u00A0ery")]
    [InlineData("/v1\u0000/query")]
    [InlineData("/v1\u001F/query")]
    [InlineData("v1/query")]
    [InlineData("https://evil.example/v1/query")]
    [InlineData("http://evil.example/v1/query")]
    [InlineData("//evil.example/v1/query")]
    [InlineData("/v1/query:http")]
    [InlineData("/v1/../admin")]
    [InlineData("/v1/%2e%2e/admin")]
    [InlineData("/v1/%2E%2E/admin")]
    [InlineData("/v1/./health")]
    [InlineData("/v1/%2e/health")]
    [InlineData("/v1\\health")]
    [InlineData("/v1/%")]
    [InlineData("/v1/%2")]
    [InlineData("/v1/%GG")]
    [InlineData("/v1/%FF")]
    [InlineData("/v1/%00")]
    public void BuildHeadersRejectsNonExactPaths(string path)
    {
        Assert.Throws<ArgumentException>(() => CanonicalRequest.BuildHeaders(
            FixtureNetworkId,
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
    [InlineData("po st")]
    [InlineData("po\u0000st")]
    [InlineData("post/get")]
    [InlineData("post:get")]
    [InlineData("post?get")]
    [InlineData("post@get")]
    [InlineData("post,get")]
    [InlineData("post;get")]
    [InlineData("post(get)")]
    [InlineData("post[get]")]
    [InlineData("post\\get")]
    [InlineData("post\"get")]
    [InlineData("post\u007Fget")]
    [InlineData("posté")]
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
    [InlineData("/v1/qu ery")]
    [InlineData("/v1\u0000/query")]
    [InlineData("v1/query")]
    [InlineData("https://evil.example/v1/query")]
    [InlineData("http://evil.example/v1/query")]
    [InlineData("//evil.example/v1/query")]
    [InlineData("/v1/query:http")]
    [InlineData("/v1/../admin")]
    [InlineData("/v1/%2e%2e/admin")]
    [InlineData("/v1/%2E%2E/admin")]
    [InlineData("/v1/./health")]
    [InlineData("/v1/%2e/health")]
    [InlineData("/v1\\health")]
    [InlineData("/v1/%")]
    [InlineData("/v1/%2")]
    [InlineData("/v1/%GG")]
    [InlineData("/v1/%FF")]
    [InlineData("/v1/%00")]
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
    [InlineData("abcdef0123456789 abcdef0123456789")]
    [InlineData("abcdef0123456789\u00A0abcdef0123456789")]
    [InlineData("abcdef0123456789\u0000abcdef0123456789")]
    [InlineData("abcdef0123456789\u001Fabcdef0123456789")]
    [InlineData("abcdef0123456789\u007Fabcdef0123456789")]
    [InlineData("abcdef0123456789abcdef012345678")]
    [InlineData("abcdef0123456789abcdef01234567890")]
    [InlineData("ABCDEF0123456789abcdef0123456789")]
    [InlineData("abcdef0123456789abcdef01234567g")]
    [InlineData("abcdef0123456789abcdef01234567-")]
    [InlineData("abcdef0123456789abcdef01234567_")]
    public void BuildHeadersRejectsNonExactCallerProvidedNonces(string nonce)
    {
        Assert.Throws<ArgumentException>(() => CanonicalRequest.BuildHeaders(
            FixtureNetworkId,
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
    [InlineData("abcdef0123456789 abcdef0123456789")]
    [InlineData("abcdef0123456789\u0000abcdef0123456789")]
    [InlineData("abcdef0123456789abcdef012345678")]
    [InlineData("abcdef0123456789abcdef01234567890")]
    [InlineData("ABCDEF0123456789abcdef0123456789")]
    [InlineData("abcdef0123456789abcdef01234567g")]
    [InlineData("abcdef0123456789abcdef01234567-")]
    [InlineData("abcdef0123456789abcdef01234567_")]
    public void BuildSignatureMessageRejectsNonExactCallerProvidedNonces(string nonce)
    {
        Assert.Throws<ArgumentException>(() => CanonicalRequest.BuildSignatureMessage(
            FixtureNetworkId,
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
    [InlineData("po st", "/v1/query")]
    [InlineData("post/get", "/v1/query")]
    [InlineData("post:get", "/v1/query")]
    [InlineData("post?get", "/v1/query")]
    [InlineData("post@get", "/v1/query")]
    [InlineData("post,get", "/v1/query")]
    [InlineData("post;get", "/v1/query")]
    [InlineData("post(get)", "/v1/query")]
    [InlineData("post[get]", "/v1/query")]
    [InlineData("post\\get", "/v1/query")]
    [InlineData("post\"get", "/v1/query")]
    [InlineData("post\u007Fget", "/v1/query")]
    [InlineData("posté", "/v1/query")]
    [InlineData("post", "/v1/qu ery")]
    [InlineData("po\u0000st", "/v1/query")]
    [InlineData("post", "/v1\u0000/query")]
    [InlineData("post", "v1/query")]
    [InlineData("post", "https://evil.example/v1/query")]
    [InlineData("post", "http://evil.example/v1/query")]
    [InlineData("post", "//evil.example/v1/query")]
    [InlineData("post", "/v1/query:http")]
    [InlineData("post", "/v1/../admin")]
    [InlineData("post", "/v1/%2e%2e/admin")]
    [InlineData("post", "/v1/%2E%2E/admin")]
    [InlineData("post", "/v1/./health")]
    [InlineData("post", "/v1/%2e/health")]
    [InlineData("post", "/v1\\health")]
    [InlineData("post", "/v1/%")]
    [InlineData("post", "/v1/%2")]
    [InlineData("post", "/v1/%GG")]
    [InlineData("post", "/v1/%FF")]
    [InlineData("post", "/v1/%00")]
    public void BuildSignatureMessageRejectsNonExactMethodsAndPaths(string method, string path)
    {
        Assert.Throws<ArgumentException>(() => CanonicalRequest.BuildSignatureMessage(
            FixtureNetworkId,
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
    [InlineData("merchant@sora")]
    [InlineData("0x0a00012022d3c25e96fa1178ae08b3d30081a31a0d09e8f7321b1e015140cd37b332109ca")]
    [InlineData("n753uﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")]
    public void CanonicalRequestCredentialsRejectsNonExactAccountIds(string accountId)
    {
        Assert.Throws<ArgumentException>(() => new CanonicalRequestCredentials(accountId, FixturePrivateKeySeed));
    }

    private static void AssertArgumentException(string paramName, Action action)
    {
        var exception = Assert.Throws<ArgumentException>(action);
        Assert.Equal(paramName, exception.ParamName);
    }
}
