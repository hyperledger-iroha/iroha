using System.Numerics;
using System.Text.Json;
using Hyperledger.Iroha.Numeric;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class NumericV1Tests
{
    [Fact]
    public void ExactValuesCanonicalizeWithoutLossyHostNumbers()
    {
        Assert.Equal(NumericV1.IntMin.ToString(), NumericV1.IntValue.FromBigInteger(NumericV1.IntMin).ToString());
        Assert.Equal(NumericV1.IntMax.ToString(), NumericV1.IntValue.FromBigInteger(NumericV1.IntMax).ToString());
        Assert.Equal("1.23", NumericV1.DecimalValue.Parse("1.2300").ToString());
        Assert.Equal("0", NumericV1.DecimalValue.Parse("0.000").ToString());
        Assert.Equal("12.5", NumericV1.QuantityValue.Parse("12.50").ToString());
        AssertNumericError(
            NumericV1.ErrorCode.NegativeQuantity,
            () => NumericV1.QuantityValue.Parse("-0.1"));
        AssertNumericError(
            NumericV1.ErrorCode.MantissaOverflow,
            () => NumericV1.QuantityValue.Parse("-" + new string('9', 154)));
        AssertNumericError(
            NumericV1.ErrorCode.MantissaOverflow,
            () => NumericV1.IntValue.FromBigInteger(NumericV1.IntMax + BigInteger.One));
        AssertNumericError(
            NumericV1.ErrorCode.MantissaOverflow,
            () => NumericV1.IntValue.FromBigInteger(NumericV1.IntMin - BigInteger.One));
        AssertNumericError(
            NumericV1.ErrorCode.MantissaOverflow,
            () => NumericV1.IntValue.Parse(new string('1', 10_000)));
        AssertNumericError(
            NumericV1.ErrorCode.InvalidText,
            () => NumericV1.IntValue.Parse(new string('x', 10_000)));
        AssertNumericError(
            NumericV1.ErrorCode.MantissaOverflow,
            () => NumericV1.DecimalValue.Parse(new string('1', 10_000)));
        Assert.Equal(
            "1",
            NumericV1.DecimalValue.Parse("1.00000000000000000000000000000").ToString());
        Assert.Equal("1", NumericV1.DecimalValue.Parse("1." + new string('0', 10_000)).ToString());
        Assert.Equal(
            NumericV1.IntMax.ToString(),
            NumericV1.DecimalValue.Parse(NumericV1.IntMax + ".0").ToString());
        Assert.Equal(
            NumericV1.IntMax.ToString(),
            NumericV1.DecimalValue.FromMantissa(NumericV1.IntMax * 10, 1).ToString());
        AssertNumericError(
            NumericV1.ErrorCode.MantissaOverflow,
            () => NumericV1.DecimalValue.Parse(NumericV1.IntMax + ".1"));
        AssertNumericError(
            NumericV1.ErrorCode.InvalidScale,
            () => NumericV1.DecimalValue.Parse("0.00000000000000000000000000001"));

        Assert.Equal("\"1.23\"", JsonSerializer.Serialize(NumericV1.DecimalValue.Parse("1.23")));
        Assert.Equal("\"7\"", JsonSerializer.Serialize(NumericV1.IntValue.Parse("7")));
        Assert.Equal("\"12.5\"", JsonSerializer.Serialize(NumericV1.QuantityValue.Parse("12.5")));
        Assert.Equal(
            NumericV1.IntValue.Parse("7"),
            JsonSerializer.Deserialize<NumericV1.IntValue>("\"7\""));
        AssertNumericError(
            NumericV1.ErrorCode.InvalidText,
            () => JsonSerializer.Deserialize<NumericV1.IntValue>("7"));
        AssertNumericError(
            NumericV1.ErrorCode.InvalidText,
            () => JsonSerializer.Deserialize<NumericV1.IntValue>("null"));
        AssertNumericError(
            NumericV1.ErrorCode.InvalidText,
            () => JsonSerializer.Deserialize<NumericV1.DecimalValue>("1.25"));

        foreach (var alternate in new[]
        {
            "+1", "01", "1.", ".5", "1e0", "-0", "-0.0", "1.0", "1.2300", "0.0",
        })
        {
            AssertNumericError(
                NumericV1.ErrorCode.InvalidText,
                () => NumericV1.DecodeDecimalJson(alternate));
        }

        foreach (var alternate in new[] { "+1", "01", "-0", "1.0", "1e0" })
        {
            AssertNumericError(
                NumericV1.ErrorCode.InvalidText,
                () => NumericV1.DecodeIntJson(alternate));
        }

        AssertNumericError(
            NumericV1.ErrorCode.InvalidText,
            () => NumericV1.DecodeQuantityJson("1.0"));
        AssertNumericError(
            NumericV1.ErrorCode.NegativeQuantity,
            () => NumericV1.DecodeQuantityJson("-1"));
    }

    [Fact]
    public void CanonicalFramesAndEnvelopesRoundTrip()
    {
        var integer = NumericV1.IntValue.Parse("-129");
        var integerEnvelope = NumericV1.EncodeIntEnvelope(integer);
        Assert.Equal(0x00, integerEnvelope[0]);
        Assert.Equal(0x11, integerEnvelope[1]);
        Assert.Equal(integer, NumericV1.DecodeIntFrame(NumericV1.EncodeIntFrame(integer)));
        Assert.Equal(integer, NumericV1.DecodeIntEnvelope(integerEnvelope));

        var decimalValue = NumericV1.DecimalValue.Parse("-1.25");
        var decimalEnvelope = NumericV1.EncodeDecimalEnvelope(decimalValue);
        Assert.Equal(0x00, decimalEnvelope[0]);
        Assert.Equal(0x12, decimalEnvelope[1]);
        Assert.Equal(decimalValue, NumericV1.DecodeDecimalFrame(NumericV1.EncodeDecimalFrame(decimalValue)));
        Assert.Equal(decimalValue, NumericV1.DecodeDecimalEnvelope(decimalEnvelope));

        var quantity = NumericV1.QuantityValue.Parse("1.25");
        var quantityEnvelope = NumericV1.EncodeQuantityEnvelope(quantity);
        Assert.Equal(0x00, quantityEnvelope[0]);
        Assert.Equal(0x13, quantityEnvelope[1]);
        Assert.Equal(quantity, NumericV1.DecodeQuantityFrame(NumericV1.EncodeQuantityFrame(quantity)));
        Assert.Equal(quantity, NumericV1.DecodeQuantityEnvelope(quantityEnvelope));

        AssertNumericError(
            NumericV1.ErrorCode.WrongType,
            () => NumericV1.DecodeDecimalEnvelope(NumericV1.EncodeIntEnvelope(NumericV1.IntValue.Parse("1"))));
    }

    [Fact]
    public void MalformedAuthenticatedInputsAreRejected()
    {
        var frame = NumericV1.EncodeIntFrame(NumericV1.IntValue.Parse("128"));
        for (var length = 0; length < frame.Length; length++)
        {
            AssertAnyNumericError(() => NumericV1.DecodeIntFrame(frame.AsSpan(0, length)));
        }

        var badChecksum = frame.ToArray();
        badChecksum[^1] ^= 1;
        AssertNumericError(
            NumericV1.ErrorCode.ChecksumMismatch,
            () => NumericV1.DecodeIntFrame(badChecksum));

        var badHash = NumericV1.EncodeIntEnvelope(NumericV1.IntValue.Parse("1"));
        badHash[^1] ^= 1;
        AssertNumericError(
            NumericV1.ErrorCode.PayloadHashMismatch,
            () => NumericV1.DecodeIntEnvelope(badHash));

        var retired = NumericV1.EncodeIntEnvelope(NumericV1.IntValue.Parse("1"));
        retired[0] = 0;
        retired[1] = 0x10;
        retired[2] = 2;
        AssertNumericError(
            NumericV1.ErrorCode.TypeNotAllowed,
            () => NumericV1.DecodeIntEnvelope(retired));

        var knownWrong = NumericV1.EncodeIntEnvelope(NumericV1.IntValue.Parse("1"));
        knownWrong[0] = 0;
        knownWrong[1] = 0x01;
        knownWrong[2] = 2;
        AssertNumericError(
            NumericV1.ErrorCode.WrongType,
            () => NumericV1.DecodeIntEnvelope(knownWrong));

        var unknown = NumericV1.EncodeIntEnvelope(NumericV1.IntValue.Parse("1"));
        unknown[0] = 0;
        unknown[1] = 0x14;
        unknown[2] = 2;
        AssertNumericError(
            NumericV1.ErrorCode.UnknownType,
            () => NumericV1.DecodeIntEnvelope(unknown));
    }

    [Fact]
    public void ConsumesRustAuthoredSharedGoldenFixture()
    {
        using var fixture = JsonDocument.Parse(File.ReadAllText(SharedFixturePath()));
        var root = fixture.RootElement;
        Assert.Equal("iroha.numeric.v1", root.GetProperty("format").GetString());
        Assert.Equal(512, root.GetProperty("signed_bits").GetInt32());
        Assert.Equal(28, root.GetProperty("maximum_scale").GetInt32());

        foreach (var vector in root.GetProperty("text").EnumerateArray())
        {
            var input = vector.GetProperty("input").GetString()!;
            var canonical = vector.GetProperty("kind").GetString() switch
            {
                "decimal" => NumericV1.DecimalValue.Parse(input).ToString(),
                "quantity" => NumericV1.QuantityValue.Parse(input).ToString(),
                var kind => throw new Xunit.Sdk.XunitException($"unknown text fixture kind {kind}"),
            };
            Assert.Equal(vector.GetProperty("canonical").GetString(), canonical);
        }

        foreach (var vector in root.GetProperty("valid").EnumerateArray())
        {
            var id = vector.GetProperty("id").GetString()!;
            var kind = vector.GetProperty("kind").GetString()!;
            var canonical = vector.GetProperty("canonical").GetString()!;
            var fixtureFrame = Convert.FromHexString(vector.GetProperty("frame_hex").GetString()!);
            var fixtureEnvelope = Convert.FromHexString(vector.GetProperty("envelope_hex").GetString()!);
            byte[] frame;
            byte[] envelope;
            string decodedFrame;
            string decodedEnvelope;
            switch (kind)
            {
                case "int":
                    var integer = NumericV1.DecodeIntJson(canonical);
                    frame = NumericV1.EncodeIntFrame(integer);
                    envelope = NumericV1.EncodeIntEnvelope(integer);
                    decodedFrame = NumericV1.DecodeIntFrame(fixtureFrame).ToString();
                    decodedEnvelope = NumericV1.DecodeIntEnvelope(fixtureEnvelope).ToString();
                    break;
                case "decimal":
                    var decimalValue = NumericV1.DecodeDecimalJson(canonical);
                    frame = NumericV1.EncodeDecimalFrame(decimalValue);
                    envelope = NumericV1.EncodeDecimalEnvelope(decimalValue);
                    decodedFrame = NumericV1.DecodeDecimalFrame(fixtureFrame).ToString();
                    decodedEnvelope = NumericV1.DecodeDecimalEnvelope(fixtureEnvelope).ToString();
                    break;
                case "quantity":
                    var quantity = NumericV1.DecodeQuantityJson(canonical);
                    frame = NumericV1.EncodeQuantityFrame(quantity);
                    envelope = NumericV1.EncodeQuantityEnvelope(quantity);
                    decodedFrame = NumericV1.DecodeQuantityFrame(fixtureFrame).ToString();
                    decodedEnvelope = NumericV1.DecodeQuantityEnvelope(fixtureEnvelope).ToString();
                    break;
                default:
                    throw new Xunit.Sdk.XunitException($"unknown fixture kind {kind}");
            }

            Assert.Equal(
                vector.GetProperty("body_hex").GetString(),
                Hex(frame.AsSpan(40)));
            Assert.Equal(vector.GetProperty("frame_hex").GetString(), Hex(frame));
            Assert.Equal(vector.GetProperty("envelope_hex").GetString(), Hex(envelope));
            Assert.Equal(canonical, decodedFrame);
            Assert.Equal(canonical, decodedEnvelope);
        }

        foreach (var vector in root.GetProperty("invalid").EnumerateArray())
        {
            var input = vector.GetProperty("input").GetString()!;
            var decodeAs = vector.GetProperty("decode_as").GetString()!;
            var expected = FixtureErrorCode(vector.GetProperty("expected").GetString()!);
            var bytes = Convert.FromHexString(vector.GetProperty("hex").GetString()!);
            AssertNumericError(expected, () => DecodeFixtureInput(input, decodeAs, bytes));
        }

        foreach (var vector in root.GetProperty("invalid_text").EnumerateArray())
        {
            var kind = vector.GetProperty("kind").GetString()!;
            var input = vector.GetProperty("input");
            var expected = FixtureErrorCode(vector.GetProperty("expected").GetString()!);
            AssertNumericError(expected, () => DecodeInvalidText(kind, input));
        }
    }

    private static void DecodeInvalidText(string kind, JsonElement input)
    {
        if (input.ValueKind == JsonValueKind.String)
        {
            var value = input.GetString()!;
            switch (kind)
            {
                case "int": _ = NumericV1.DecodeIntJson(value); break;
                case "decimal": _ = NumericV1.DecodeDecimalJson(value); break;
                case "quantity": _ = NumericV1.DecodeQuantityJson(value); break;
                default: throw new Xunit.Sdk.XunitException($"unknown numeric text kind {kind}");
            }
            return;
        }

        var json = input.GetRawText();
        switch (kind)
        {
            case "int": _ = JsonSerializer.Deserialize<NumericV1.IntValue>(json); break;
            case "decimal": _ = JsonSerializer.Deserialize<NumericV1.DecimalValue>(json); break;
            case "quantity": _ = JsonSerializer.Deserialize<NumericV1.QuantityValue>(json); break;
            default: throw new Xunit.Sdk.XunitException($"unknown numeric text kind {kind}");
        }
    }

    private static void DecodeFixtureInput(string input, string decodeAs, byte[] bytes)
    {
        switch (input, decodeAs)
        {
            case ("frame", "int"):
                _ = NumericV1.DecodeIntFrame(bytes);
                break;
            case ("frame", "decimal"):
                _ = NumericV1.DecodeDecimalFrame(bytes);
                break;
            case ("frame", "quantity"):
                _ = NumericV1.DecodeQuantityFrame(bytes);
                break;
            case ("envelope", "int"):
                _ = NumericV1.DecodeIntEnvelope(bytes);
                break;
            case ("envelope", "decimal"):
                _ = NumericV1.DecodeDecimalEnvelope(bytes);
                break;
            case ("envelope", "quantity"):
                _ = NumericV1.DecodeQuantityEnvelope(bytes);
                break;
            default:
                throw new Xunit.Sdk.XunitException($"unknown fixture decoder {input}/{decodeAs}");
        }
    }

    private static NumericV1.ErrorCode FixtureErrorCode(string value)
    {
        return value switch
        {
            "mantissa_overflow" => NumericV1.ErrorCode.MantissaOverflow,
            "noncanonical_mantissa" => NumericV1.ErrorCode.NoncanonicalMantissa,
            "invalid_scale" => NumericV1.ErrorCode.InvalidScale,
            "noncanonical_decimal" => NumericV1.ErrorCode.NoncanonicalDecimal,
            "negative_quantity" => NumericV1.ErrorCode.NegativeQuantity,
            "invalid_text" => NumericV1.ErrorCode.InvalidText,
            "frame_too_short" => NumericV1.ErrorCode.FrameTooShort,
            "frame_too_large" => NumericV1.ErrorCode.FrameTooLarge,
            "invalid_header" => NumericV1.ErrorCode.InvalidHeader,
            "schema_mismatch" => NumericV1.ErrorCode.SchemaMismatch,
            "compression_not_allowed" => NumericV1.ErrorCode.CompressionNotAllowed,
            "layout_flags_not_allowed" => NumericV1.ErrorCode.LayoutFlagsNotAllowed,
            "length_mismatch" => NumericV1.ErrorCode.LengthMismatch,
            "checksum_mismatch" => NumericV1.ErrorCode.ChecksumMismatch,
            "truncated_envelope" => NumericV1.ErrorCode.TruncatedEnvelope,
            "unknown_type" => NumericV1.ErrorCode.UnknownType,
            "type_not_allowed" => NumericV1.ErrorCode.TypeNotAllowed,
            "wrong_type" => NumericV1.ErrorCode.WrongType,
            "invalid_envelope_version" => NumericV1.ErrorCode.InvalidEnvelopeVersion,
            "oversized_length" => NumericV1.ErrorCode.OversizedLength,
            "payload_hash_mismatch" => NumericV1.ErrorCode.PayloadHashMismatch,
            _ => throw new Xunit.Sdk.XunitException($"unknown fixture error code {value}"),
        };
    }

    private static void AssertAnyNumericError(Action action)
    {
        Assert.IsType<NumericV1.NumericException>(Record.Exception(action));
    }

    private static void AssertNumericError(NumericV1.ErrorCode expected, Action action)
    {
        var exception = Assert.Throws<NumericV1.NumericException>(action);
        Assert.Equal(expected, exception.Code);
    }

    private static string SharedFixturePath()
    {
        var path = Path.Combine(AppContext.BaseDirectory, "Fixtures", "numeric_v1_golden.json");
        Assert.True(File.Exists(path), $"shared numeric fixture was not copied to {path}");
        return path;
    }

    private static string Hex(ReadOnlySpan<byte> bytes)
    {
        return Convert.ToHexString(bytes).ToLowerInvariant();
    }
}
