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
    public void PaddedI105LiteralsFailBeforeCanonicalParsing()
    {
        var literal = Fixture.Value.Cases.Positive[0].Encodings.I105.String;
        foreach (var padded in new[]
        {
            " " + literal,
            literal + " ",
            "\t" + literal,
            literal + "\n",
            "\u00A0" + literal,
            literal + "\u00A0",
        })
        {
            var exception = Assert.Throws<AccountAddressException>(() =>
                AccountAddress.Parse(padded, AccountAddress.DefaultChainDiscriminant));

            Assert.Equal(AccountAddressErrorCode.UnsupportedAddressFormat, exception.Code);
            Assert.Contains("surrounding whitespace", exception.Message, StringComparison.Ordinal);
        }
    }

    [Fact]
    public void NumericI105SentinelRejectsOverflowAndNonAsciiDigitForms()
    {
        var literal = AccountAddress
            .FromPublicKey(Enumerable.Repeat((byte)0x22, 32).ToArray())
            .ToI105(5);
        Assert.StartsWith("n5", literal, StringComparison.Ordinal);

        var overflow = $"n65536{literal["n5".Length..]}";
        var overflowException = Assert.Throws<AccountAddressException>(() =>
            AccountAddress.Parse(overflow));

        Assert.Equal(AccountAddressErrorCode.InvalidI105Discriminant, overflowException.Code);
        Assert.Contains("unsigned 16-bit integer", overflowException.Message, StringComparison.Ordinal);

        foreach (var nonAsciiOrSigned in new[]
        {
            $"n+5{literal["n5".Length..]}",
            $"n５{literal["n5".Length..]}",
        })
        {
            var exception = Assert.Throws<AccountAddressException>(() =>
                AccountAddress.Parse(nonAsciiOrSigned));

            Assert.Equal(AccountAddressErrorCode.MissingI105Sentinel, exception.Code);
        }
    }

    [Fact]
    public void NumericI105SentinelRejectsLeadingZeroAliases()
    {
        var customLiteral = AccountAddress
            .FromPublicKey(Enumerable.Repeat((byte)0x33, 32).ToArray())
            .ToI105(5);
        var devLiteral = AccountAddress
            .FromPublicKey(Enumerable.Repeat((byte)0x44, 32).ToArray())
            .ToI105(AccountAddress.DevChainDiscriminant);

        foreach (var noncanonical in new[]
        {
            $"n0005{customLiteral["n5".Length..]}",
            $"n0{devLiteral["dev".Length..]}",
        })
        {
            var exception = Assert.Throws<AccountAddressException>(() =>
                AccountAddress.Parse(noncanonical));

            Assert.Equal(AccountAddressErrorCode.UnsupportedAddressFormat, exception.Code);
            Assert.Contains("canonical I105 form", exception.Message, StringComparison.Ordinal);
        }
    }

    [Fact]
    public void AccountAddressSnapshotsInputAndReturnedByteArrays()
    {
        var publicKey = Enumerable.Range(1, 32).Select(static value => (byte)value).ToArray();
        var expectedPublicKeyHex = Convert.ToHexString(publicKey);
        var address = AccountAddress.FromPublicKey(publicKey);
        var expectedCanonicalBytes = address.CanonicalBytes();
        var expectedControllerBytes = address.ControllerBytes();
        var expectedLiteral = address.ToI105();

        publicKey[0] = 0xFF;
        var returnedPublicKey = address.PublicKey;
        returnedPublicKey[1] = 0xEE;
        var returnedCanonicalBytes = address.CanonicalBytes();
        returnedCanonicalBytes[0] = 0xFF;
        var returnedControllerBytes = address.ControllerBytes();
        returnedControllerBytes[0] = 0xFF;

        Assert.Equal(expectedPublicKeyHex, Convert.ToHexString(address.PublicKey));
        Assert.Equal(expectedCanonicalBytes, address.CanonicalBytes());
        Assert.Equal(expectedControllerBytes, address.ControllerBytes());
        Assert.Equal(expectedLiteral, address.ToI105());
        Assert.NotSame(returnedPublicKey, address.PublicKey);
        Assert.NotSame(returnedCanonicalBytes, address.CanonicalBytes());
        Assert.NotSame(returnedControllerBytes, address.ControllerBytes());

        var canonicalSource = address.CanonicalBytes();
        var parsed = AccountAddress.FromCanonicalBytes(canonicalSource);
        canonicalSource[0] = 0xFF;

        Assert.Equal(expectedLiteral, parsed.ToI105());
        Assert.Equal(expectedCanonicalBytes, parsed.CanonicalBytes());
    }

    [Fact]
    public void ParsedAccountAddressesHaveCanonicalValueEquality()
    {
        var literal = AccountAddress
            .FromPublicKey(Enumerable.Repeat((byte)0x42, 32).ToArray())
            .ToI105();
        var first = AccountAddress.Parse(literal);
        var second = AccountAddress.FromCanonicalBytes(first.CanonicalBytes());

        Assert.NotSame(first, second);
        Assert.Equal(first, second);
        Assert.True(first == second);
        Assert.False(first != second);
        Assert.Equal(first.GetHashCode(), second.GetHashCode());
        Assert.Single(new HashSet<AccountAddress> { first, second });
    }

    [Fact]
    public void MlDsaAccountAddressesUseCanonicalExtendedSuite65Keys()
    {
        var publicKey = Enumerable.Repeat((byte)0xA5, 1_952).ToArray();
        var address = AccountAddress.FromPublicKey(publicKey, CurveId.MlDsa);
        var canonical = address.CanonicalBytes();

        Assert.Equal("ml-dsa", address.Algorithm);
        Assert.Equal(new byte[] { 0x02, 0x02, 0x02, 0x07, 0xA0 }, canonical[..5]);
        Assert.Equal(publicKey, canonical[5..]);
        Assert.Equal(canonical, AccountAddress.FromCanonicalBytes(canonical).CanonicalBytes());
        var literal = address.ToI105(AccountAddress.DevChainDiscriminant);
        Assert.Equal(canonical, AccountAddress.Parse(literal).CanonicalBytes());
    }

    [Fact]
    public void MlDsaAccountAddressesRejectNonSuite65KeyMaterial()
    {
        foreach (var publicKey in new[]
        {
            Array.Empty<byte>(),
            Enumerable.Repeat((byte)0x20, 32).ToArray(),
            Enumerable.Repeat((byte)0x44, 1_312).ToArray(),
            Enumerable.Repeat((byte)0x65, 1_951).ToArray(),
            Enumerable.Repeat((byte)0x65, 1_953).ToArray(),
            Enumerable.Repeat((byte)0x87, 2_592).ToArray(),
            new byte[1_952],
        })
        {
            var exception = Assert.Throws<AccountAddressException>(() =>
                AccountAddress.FromPublicKey(publicKey, CurveId.MlDsa));
            Assert.Equal(AccountAddressErrorCode.InvalidPublicKey, exception.Code);
            Assert.Contains("1952-byte ML-DSA-65", exception.Message, StringComparison.Ordinal);
        }
    }

    [Fact]
    public void MlDsaCanonicalDecodeRejectsShortAndAllZeroControllers()
    {
        var shortCompact = new byte[4 + 32];
        shortCompact[0] = 0x02;
        shortCompact[1] = 0x00;
        shortCompact[2] = 0x02;
        shortCompact[3] = 32;
        Array.Fill(shortCompact, (byte)0x44, 4, 32);
        Assert.Equal(
            AccountAddressErrorCode.InvalidPublicKey,
            Assert.Throws<AccountAddressException>(() => AccountAddress.FromCanonicalBytes(shortCompact)).Code);

        var allZeroExtended = new byte[5 + 1_952];
        allZeroExtended[0] = 0x02;
        allZeroExtended[1] = 0x02;
        allZeroExtended[2] = 0x02;
        allZeroExtended[3] = 0x07;
        allZeroExtended[4] = 0xA0;
        Assert.Equal(
            AccountAddressErrorCode.InvalidPublicKey,
            Assert.Throws<AccountAddressException>(() => AccountAddress.FromCanonicalBytes(allZeroExtended)).Code);

        var shortExtended = new byte[5 + 32];
        shortExtended[0] = 0x02;
        shortExtended[1] = 0x02;
        shortExtended[2] = 0x01;
        shortExtended[3] = 0x00;
        shortExtended[4] = 0x20;
        Array.Fill(shortExtended, (byte)0x11, 5, 32);
        Assert.Equal(
            AccountAddressErrorCode.InvalidLength,
            Assert.Throws<AccountAddressException>(() => AccountAddress.FromCanonicalBytes(shortExtended)).Code);
    }

    [Fact]
    public void MlDsaMultisigDecodeRequiresSuite65KeyMaterial()
    {
        var validKey = Enumerable.Repeat((byte)0xA5, 1_952).ToArray();
        var valid = BuildSingleMemberMultisig(0x02, validKey);
        Assert.Equal(valid, AccountAddress.FromCanonicalBytes(valid).CanonicalBytes());

        foreach (var invalidKey in new[]
        {
            Enumerable.Repeat((byte)0x44, 1_312).ToArray(),
            Enumerable.Repeat((byte)0x87, 2_592).ToArray(),
            new byte[1_952],
        })
        {
            Assert.Equal(
                AccountAddressErrorCode.InvalidPublicKey,
                Assert.Throws<AccountAddressException>(() =>
                    AccountAddress.FromCanonicalBytes(BuildSingleMemberMultisig(0x02, invalidKey))).Code);
        }
    }

    private static byte[] BuildSingleMemberMultisig(byte curveId, byte[] publicKey)
    {
        var canonical = new byte[12 + publicKey.Length];
        canonical[0] = 0x0A;
        canonical[1] = 0x01;
        canonical[2] = 0x01;
        canonical[3] = 0x00;
        canonical[4] = 0x01;
        canonical[5] = 0x00;
        canonical[6] = 0x01;
        canonical[7] = curveId;
        canonical[8] = 0x00;
        canonical[9] = 0x01;
        canonical[10] = (byte)(publicKey.Length >> 8);
        canonical[11] = (byte)publicKey.Length;
        publicKey.CopyTo(canonical, 12);
        return canonical;
    }

    [Fact]
    public void RetiredDomainSelectorPrefixIsRejected()
    {
        var canonical = AccountAddress
            .FromPublicKey(Enumerable.Repeat((byte)0x01, 32).ToArray())
            .CanonicalBytes();
        var selectorPrefixed = new byte[canonical.Length + 13];
        selectorPrefixed[0] = canonical[0];
        selectorPrefixed[1] = 0x01;
        for (var index = 0; index < 12; index++)
        {
            selectorPrefixed[index + 2] = (byte)(index + 1);
        }

        canonical.AsSpan(1).CopyTo(selectorPrefixed.AsSpan(14));

        Assert.Throws<AccountAddressException>(() =>
            AccountAddress.FromCanonicalBytes(selectorPrefixed));
    }

    [Fact]
    public void UnknownCurveIdentifiersAreRejected()
    {
        var publicKey = Enumerable.Repeat((byte)0x11, 32).ToArray();
        foreach (var curveIdentifier in new byte[] { 0, 3, 4, 5, 9, 16, byte.MaxValue })
        {
            var exception = Assert.Throws<AccountAddressException>(() =>
                AccountAddress.FromPublicKey(publicKey, (CurveId)curveIdentifier));

            Assert.Equal(AccountAddressErrorCode.UnknownCurve, exception.Code);
            Assert.Contains(curveIdentifier.ToString(), exception.Message, StringComparison.Ordinal);
        }
    }

    [Fact]
    public void Ed25519AccountAddressesRejectMalformedKeyMaterial()
    {
        foreach (var publicKey in new[]
        {
            Array.Empty<byte>(),
            new byte[31],
            new byte[32],
            new byte[33],
        })
        {
            var exception = Assert.Throws<AccountAddressException>(() =>
                AccountAddress.FromPublicKey(publicKey));

            Assert.Equal(AccountAddressErrorCode.InvalidPublicKey, exception.Code);
            Assert.Contains("nonzero 32-byte key", exception.Message, StringComparison.Ordinal);
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
