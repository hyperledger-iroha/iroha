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
