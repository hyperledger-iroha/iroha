using System.Text.Json;
using System.Text.Json.Serialization;
using Hyperledger.Iroha;
using Hyperledger.Iroha.Address;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class AddressFixtureTests
{
    private static readonly Lazy<AddressFixtureRoot> Fixture = new(LoadFixture);

    [Fact]
    public void PositiveVectorsRoundTrip()
    {
        foreach (var testCase in Fixture.Value.Cases.Positive)
        {
            var address = AccountAddress.Parse(testCase.Encodings.I105.String, (ushort)testCase.Encodings.I105.Prefix);
            Assert.Equal(testCase.Encodings.CanonicalHex, address.CanonicalHex);
            Assert.Equal(testCase.Encodings.I105.String, address.ToI105((ushort)testCase.Encodings.I105.Prefix));

            if (string.Equals(testCase.Category, "single", StringComparison.Ordinal))
            {
                Assert.Equal(AddressClass.SingleKey, address.AddressClass);
                Assert.Equal(testCase.Controller.PublicKeyHex, Convert.ToHexString(address.PublicKey));
            }
            else
            {
                Assert.Equal(AddressClass.MultiSig, address.AddressClass);
                Assert.NotEmpty(address.ControllerBytes());
            }
        }
    }

    [Fact]
    public void NegativeVectorsFailWithExpectedErrorCodes()
    {
        foreach (var testCase in Fixture.Value.Cases.Negative)
        {
            var exception = Assert.Throws<AccountAddressException>(() =>
            {
                var expectedPrefix = testCase.ExpectedPrefix;
                _ = AccountAddress.Parse(testCase.Input, expectedPrefix is null ? null : (ushort?)expectedPrefix.Value);
            });

            Assert.Equal(ParseExpectedCode(testCase.ExpectedError.Kind), exception.Code);
        }
    }

    [Fact]
    public void FullwidthSentinelLiteralFails()
    {
        var literal = Fixture.Value.Cases.Positive[0].Encodings.I105.String;
        Assert.StartsWith("sora", literal, StringComparison.Ordinal);

        var noncanonical = $"ｓｏｒａ{literal["sora".Length..]}";
        var exception = Assert.Throws<AccountAddressException>(() =>
            AccountAddress.Parse(noncanonical, AccountAddress.DefaultChainDiscriminant));

        Assert.Equal(AccountAddressErrorCode.MissingI105Sentinel, exception.Code);
    }

    [Fact]
    public void LegacyFullwidthKanaPayloadFails()
    {
        var literal = Fixture.Value.Cases.Positive
            .Select(testCase => testCase.Encodings.I105.String)
            .First(value => value.Contains("ﾛ", StringComparison.Ordinal));
        var noncanonical = literal.Replace("ﾛ", "ロ", StringComparison.Ordinal);

        var exception = Assert.Throws<AccountAddressException>(() =>
            AccountAddress.Parse(noncanonical, AccountAddress.DefaultChainDiscriminant));

        Assert.Equal(AccountAddressErrorCode.InvalidI105Char, exception.Code);
    }

    [Fact]
    public void CurveAlgorithmAliasesRejectConfusableOrControlLabels()
    {
        var publicKey = Enumerable.Repeat((byte)0x11, 32).ToArray();
        var algorithms = new (string Algorithm, string ExpectedMessage)[]
        {
            ("", "non-empty string"),
            ("   ", "non-empty string"),
            ("\u00A0", "non-empty string"),
            (" ed25519", "surrounding whitespace"),
            ("ed25519 ", "surrounding whitespace"),
            ("\ted25519", "surrounding whitespace"),
            ("ed25519\t", "surrounding whitespace"),
            ("ed25519\n", "surrounding whitespace"),
            ("\u00A0ed25519", "surrounding whitespace"),
            ("ed25519\u00A0", "surrounding whitespace"),
            ("future-curve", "future-curve"),
            ("ed\t25519", "ed\t25519"),
            ("ed\u000025519", "unsupported signing algorithm"),
            ("ed\u001F25519", "unsupported signing algorithm"),
            ("ed\u007F25519", "unsupported signing algorithm"),
            ("ed\u200B25519", "ed\u200B25519"),
            ("\u0435d25519", "\u0435d25519"),
            ("ml\uFF0Ddsa", "ml\uFF0Ddsa"),
            ("gost256\u0430", "gost256\u0430"),
        };

        foreach (var (algorithm, expectedMessage) in algorithms)
        {
            var exception = Assert.Throws<AccountAddressException>(() =>
                AccountAddress.FromPublicKey(publicKey, algorithm));

            Assert.Equal(AccountAddressErrorCode.UnsupportedAlgorithm, exception.Code);
            Assert.Contains(expectedMessage, exception.Message, StringComparison.Ordinal);
        }
    }

    private static AddressFixtureRoot LoadFixture()
    {
        var path = Path.Combine(AppContext.BaseDirectory, "Fixtures", "address_vectors.json");
        var json = File.ReadAllText(path);
        return JsonSerializer.Deserialize(json, AddressFixtureJsonContext.Default.AddressFixtureRoot)
            ?? throw new InvalidOperationException("failed to deserialize address fixture");
    }

    private static AccountAddressErrorCode ParseExpectedCode(string value)
    {
        return value switch
        {
            "ChecksumMismatch" => AccountAddressErrorCode.ChecksumMismatch,
            "InvalidI105Char" => AccountAddressErrorCode.InvalidI105Char,
            "MissingI105Sentinel" => AccountAddressErrorCode.MissingI105Sentinel,
            "UnexpectedNetworkPrefix" => AccountAddressErrorCode.UnexpectedNetworkPrefix,
            "UnsupportedAddressFormat" => AccountAddressErrorCode.UnsupportedAddressFormat,
            _ => throw new InvalidOperationException($"unsupported fixture error kind: {value}"),
        };
    }
}

public sealed class AddressFixtureRoot
{
    [JsonPropertyName("cases")]
    public required AddressFixtureCases Cases { get; init; }
}

public sealed class AddressFixtureCases
{
    [JsonPropertyName("negative")]
    public required List<AddressNegativeCase> Negative { get; init; }

    [JsonPropertyName("positive")]
    public required List<AddressPositiveCase> Positive { get; init; }
}

public sealed class AddressPositiveCase
{
    [JsonPropertyName("category")]
    public required string Category { get; init; }

    [JsonPropertyName("controller")]
    public required AddressControllerFixture Controller { get; init; }

    [JsonPropertyName("encodings")]
    public required AddressEncodingFixture Encodings { get; init; }
}

public sealed class AddressControllerFixture
{
    [JsonPropertyName("public_key_hex")]
    public string? PublicKeyHex { get; init; }
}

public sealed class AddressEncodingFixture
{
    [JsonPropertyName("canonical_hex")]
    public required string CanonicalHex { get; init; }

    [JsonPropertyName("i105")]
    public required AddressI105EncodingFixture I105 { get; init; }
}

public sealed class AddressI105EncodingFixture
{
    [JsonPropertyName("prefix")]
    public required int Prefix { get; init; }

    [JsonPropertyName("string")]
    public required string String { get; init; }
}

public sealed class AddressNegativeCase
{
    [JsonPropertyName("expected_error")]
    public required AddressExpectedError ExpectedError { get; init; }

    [JsonPropertyName("expected_prefix")]
    public int? ExpectedPrefix { get; init; }

    [JsonPropertyName("format")]
    public required string Format { get; init; }

    [JsonPropertyName("input")]
    public required string Input { get; init; }
}

public sealed class AddressExpectedError
{
    [JsonPropertyName("kind")]
    public required string Kind { get; init; }
}

[JsonSourceGenerationOptions(PropertyNamingPolicy = JsonKnownNamingPolicy.CamelCase)]
[JsonSerializable(typeof(AddressFixtureRoot))]
internal partial class AddressFixtureJsonContext : JsonSerializerContext;
