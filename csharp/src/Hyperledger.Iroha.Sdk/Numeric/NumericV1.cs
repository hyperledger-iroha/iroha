using System.Buffers.Binary;
using System.Numerics;
using System.Security.Cryptography;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Text.RegularExpressions;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Numeric;

/// <summary>Lossless values and strict wire codecs for Kotodama V1 exact numerics.</summary>
public static class NumericV1
{
    /// <summary>Stable strict-decoder failure category.</summary>
    public enum ErrorCode
    {
        MantissaOverflow,
        NoncanonicalMantissa,
        InvalidScale,
        NoncanonicalDecimal,
        NegativeQuantity,
        InvalidText,
        FrameTooShort,
        FrameTooLarge,
        InvalidHeader,
        SchemaMismatch,
        CompressionNotAllowed,
        LayoutFlagsNotAllowed,
        LengthMismatch,
        ChecksumMismatch,
        TruncatedEnvelope,
        UnknownType,
        TypeNotAllowed,
        WrongType,
        InvalidEnvelopeVersion,
        OversizedLength,
        PayloadHashMismatch,
    }

    /// <summary>Strict Kotodama V1 numeric validation failure.</summary>
    public sealed class NumericException : ArgumentException
    {
        internal NumericException(ErrorCode code, string message)
            : base(message)
        {
            Code = code;
        }

        /// <summary>Gets the stable failure category.</summary>
        public ErrorCode Code { get; }
    }

    /// <summary>Lossless signed 512-bit integer.</summary>
    [JsonConverter(typeof(IntValueJsonConverter))]
    public sealed class IntValue : IEquatable<IntValue>
    {
        private IntValue(BigInteger value)
        {
            Value = CheckedMantissa(value);
        }

        /// <summary>Gets the exact arbitrary-precision value.</summary>
        public BigInteger Value { get; }

        /// <summary>Constructs an integer after enforcing the signed 512-bit domain.</summary>
        public static IntValue FromBigInteger(BigInteger value)
        {
            return new IntValue(value);
        }

        /// <summary>Parses a canonical base-10 integer string.</summary>
        public static IntValue Parse(string value)
        {
            ArgumentNullException.ThrowIfNull(value);
            if (!CanonicalIntegerRegex.IsMatch(value) || value == "-0")
            {
                Fail(ErrorCode.InvalidText, "int must use canonical base-10 syntax");
            }

            if (value.Length > MaxIntTextBytes)
            {
                Fail(ErrorCode.MantissaOverflow, "integer text exceeds the signed 512-bit input bound");
            }

            return FromBigInteger(BigInteger.Parse(value, System.Globalization.CultureInfo.InvariantCulture));
        }

        /// <inheritdoc />
        public bool Equals(IntValue? other)
        {
            return other is not null && Value == other.Value;
        }

        /// <inheritdoc />
        public override bool Equals(object? obj)
        {
            return obj is IntValue other && Equals(other);
        }

        /// <inheritdoc />
        public override int GetHashCode()
        {
            return Value.GetHashCode();
        }

        /// <inheritdoc />
        public override string ToString()
        {
            return Value.ToString(System.Globalization.CultureInfo.InvariantCulture);
        }
    }

    /// <summary>Lossless exact decimal with canonical scale.</summary>
    [JsonConverter(typeof(DecimalValueJsonConverter))]
    public sealed class DecimalValue : IEquatable<DecimalValue>
    {
        private DecimalValue(BigInteger mantissa, int scale)
        {
            Mantissa = mantissa;
            Scale = scale;
        }

        /// <summary>Gets the exact signed mantissa.</summary>
        public BigInteger Mantissa { get; }

        /// <summary>Gets the canonical base-10 scale.</summary>
        public int Scale { get; }

        /// <summary>Constructs and canonicalizes a mantissa/scale pair.</summary>
        public static DecimalValue FromMantissa(BigInteger mantissa, int scale)
        {
            var normalized = NormalizeScaled(mantissa, scale, quantity: false);
            return new DecimalValue(normalized.Mantissa, normalized.Scale);
        }

        /// <summary>Parses and canonicalizes an exact decimal string.</summary>
        public static DecimalValue Parse(string value)
        {
            var normalized = ParseScaled(value, quantity: false);
            return new DecimalValue(normalized.Mantissa, normalized.Scale);
        }

        /// <inheritdoc />
        public bool Equals(DecimalValue? other)
        {
            return other is not null && Mantissa == other.Mantissa && Scale == other.Scale;
        }

        /// <inheritdoc />
        public override bool Equals(object? obj)
        {
            return obj is DecimalValue other && Equals(other);
        }

        /// <inheritdoc />
        public override int GetHashCode()
        {
            return HashCode.Combine(Mantissa, Scale);
        }

        /// <inheritdoc />
        public override string ToString()
        {
            return ScaledText(Mantissa, Scale);
        }
    }

    /// <summary>Lossless nominal non-negative asset quantity.</summary>
    [JsonConverter(typeof(QuantityValueJsonConverter))]
    public sealed class QuantityValue : IEquatable<QuantityValue>
    {
        private QuantityValue(BigInteger mantissa, int scale)
        {
            Mantissa = mantissa;
            Scale = scale;
        }

        /// <summary>Gets the exact non-negative mantissa.</summary>
        public BigInteger Mantissa { get; }

        /// <summary>Gets the canonical base-10 scale.</summary>
        public int Scale { get; }

        /// <summary>Constructs and canonicalizes a non-negative mantissa/scale pair.</summary>
        public static QuantityValue FromMantissa(BigInteger mantissa, int scale)
        {
            var normalized = NormalizeScaled(mantissa, scale, quantity: true);
            return new QuantityValue(normalized.Mantissa, normalized.Scale);
        }

        /// <summary>Parses and canonicalizes an exact non-negative quantity string.</summary>
        public static QuantityValue Parse(string value)
        {
            var normalized = ParseScaled(value, quantity: true);
            return new QuantityValue(normalized.Mantissa, normalized.Scale);
        }

        /// <summary>Parses an exact non-negative quantity and rejects alternate spellings.</summary>
        public static QuantityValue ParseCanonical(string value)
        {
            var decoded = Parse(value);
            if (!string.Equals(decoded.ToString(), value, StringComparison.Ordinal))
            {
                Fail(ErrorCode.InvalidText, "quantity must use canonical spelling");
            }

            return decoded;
        }

        /// <inheritdoc />
        public bool Equals(QuantityValue? other)
        {
            return other is not null && Mantissa == other.Mantissa && Scale == other.Scale;
        }

        /// <inheritdoc />
        public override bool Equals(object? obj)
        {
            return obj is QuantityValue other && Equals(other);
        }

        /// <inheritdoc />
        public override int GetHashCode()
        {
            return HashCode.Combine(Mantissa, Scale);
        }

        /// <inheritdoc />
        public override string ToString()
        {
            return ScaledText(Mantissa, Scale);
        }
    }

    /// <summary>Serializes <see cref="IntValue"/> only as a canonical JSON string.</summary>
    public sealed class IntValueJsonConverter : JsonConverter<IntValue>
    {
        /// <inheritdoc />
        public override bool HandleNull => true;

        /// <inheritdoc />
        public override IntValue Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
        {
            return IntValue.Parse(RequiredJsonString(ref reader, "int"));
        }

        /// <inheritdoc />
        public override void Write(Utf8JsonWriter writer, IntValue value, JsonSerializerOptions options)
        {
            ArgumentNullException.ThrowIfNull(value);
            writer.WriteStringValue(value.ToString());
        }
    }

    /// <summary>Serializes <see cref="DecimalValue"/> only as a canonical JSON string.</summary>
    public sealed class DecimalValueJsonConverter : JsonConverter<DecimalValue>
    {
        /// <inheritdoc />
        public override bool HandleNull => true;

        /// <inheritdoc />
        public override DecimalValue Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
        {
            return DecodeDecimalJson(RequiredJsonString(ref reader, "decimal"));
        }

        /// <inheritdoc />
        public override void Write(Utf8JsonWriter writer, DecimalValue value, JsonSerializerOptions options)
        {
            ArgumentNullException.ThrowIfNull(value);
            writer.WriteStringValue(value.ToString());
        }
    }

    /// <summary>Serializes <see cref="QuantityValue"/> only as a canonical JSON string.</summary>
    public sealed class QuantityValueJsonConverter : JsonConverter<QuantityValue>
    {
        /// <inheritdoc />
        public override bool HandleNull => true;

        /// <inheritdoc />
        public override QuantityValue Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
        {
            return DecodeQuantityJson(RequiredJsonString(ref reader, "quantity"));
        }

        /// <inheritdoc />
        public override void Write(Utf8JsonWriter writer, QuantityValue value, JsonSerializerOptions options)
        {
            ArgumentNullException.ThrowIfNull(value);
            writer.WriteStringValue(value.ToString());
        }
    }

    /// <summary>Minimum signed V1 integer.</summary>
    public static BigInteger IntMin { get; } = -(BigInteger.One << 511);

    /// <summary>Maximum signed V1 integer.</summary>
    public static BigInteger IntMax { get; } = (BigInteger.One << 511) - BigInteger.One;

    /// <summary>Maximum canonical decimal scale.</summary>
    public const int MaxScale = 28;

    /// <summary>Encodes an integer as its canonical lossless JSON string value.</summary>
    public static string EncodeIntJson(IntValue value)
    {
        ArgumentNullException.ThrowIfNull(value);
        return value.ToString();
    }

    /// <summary>Encodes a decimal as its canonical lossless JSON string value.</summary>
    public static string EncodeDecimalJson(DecimalValue value)
    {
        ArgumentNullException.ThrowIfNull(value);
        return value.ToString();
    }

    /// <summary>Encodes a quantity as its canonical lossless JSON string value.</summary>
    public static string EncodeQuantityJson(QuantityValue value)
    {
        ArgumentNullException.ThrowIfNull(value);
        return value.ToString();
    }

    /// <summary>Decodes a canonical integer JSON string.</summary>
    public static IntValue DecodeIntJson(string value)
    {
        return IntValue.Parse(value);
    }

    /// <summary>Decodes a canonical decimal JSON string, rejecting alternate spellings.</summary>
    public static DecimalValue DecodeDecimalJson(string value)
    {
        var decoded = DecimalValue.Parse(value);
        if (!string.Equals(decoded.ToString(), value, StringComparison.Ordinal))
        {
            Fail(ErrorCode.InvalidText, "decimal JSON must use canonical spelling");
        }

        return decoded;
    }

    /// <summary>Decodes a canonical quantity JSON string, rejecting alternate spellings.</summary>
    public static QuantityValue DecodeQuantityJson(string value)
    {
        return QuantityValue.ParseCanonical(value);
    }

    /// <summary>Encodes a canonical integer Norito frame.</summary>
    public static byte[] EncodeIntFrame(IntValue value)
    {
        ArgumentNullException.ThrowIfNull(value);
        return EncodeFrame(IntKind, value.Value, 0);
    }

    /// <summary>Encodes a canonical decimal Norito frame.</summary>
    public static byte[] EncodeDecimalFrame(DecimalValue value)
    {
        ArgumentNullException.ThrowIfNull(value);
        return EncodeFrame(DecimalKind, value.Mantissa, value.Scale);
    }

    /// <summary>Encodes a canonical quantity Norito frame.</summary>
    public static byte[] EncodeQuantityFrame(QuantityValue value)
    {
        ArgumentNullException.ThrowIfNull(value);
        return EncodeFrame(QuantityKind, value.Mantissa, value.Scale);
    }

    /// <summary>Strictly decodes an integer Norito frame.</summary>
    public static IntValue DecodeIntFrame(ReadOnlySpan<byte> frame)
    {
        return IntValue.FromBigInteger(DecodeFrame(IntKind, frame).Mantissa);
    }

    /// <summary>Strictly decodes a decimal Norito frame.</summary>
    public static DecimalValue DecodeDecimalFrame(ReadOnlySpan<byte> frame)
    {
        var value = DecodeFrame(DecimalKind, frame);
        return DecimalValue.FromMantissa(value.Mantissa, value.Scale);
    }

    /// <summary>Strictly decodes a quantity Norito frame.</summary>
    public static QuantityValue DecodeQuantityFrame(ReadOnlySpan<byte> frame)
    {
        var value = DecodeFrame(QuantityKind, frame);
        return QuantityValue.FromMantissa(value.Mantissa, value.Scale);
    }

    /// <summary>Encodes an integer pointer envelope.</summary>
    public static byte[] EncodeIntEnvelope(IntValue value)
    {
        return EncodeEnvelope(IntKind, EncodeIntFrame(value));
    }

    /// <summary>Encodes a decimal pointer envelope.</summary>
    public static byte[] EncodeDecimalEnvelope(DecimalValue value)
    {
        return EncodeEnvelope(DecimalKind, EncodeDecimalFrame(value));
    }

    /// <summary>Encodes a quantity pointer envelope.</summary>
    public static byte[] EncodeQuantityEnvelope(QuantityValue value)
    {
        return EncodeEnvelope(QuantityKind, EncodeQuantityFrame(value));
    }

    /// <summary>Strictly decodes an integer pointer envelope.</summary>
    public static IntValue DecodeIntEnvelope(ReadOnlySpan<byte> envelope)
    {
        return DecodeIntFrame(DecodeEnvelope(IntKind, envelope));
    }

    /// <summary>Strictly decodes a decimal pointer envelope.</summary>
    public static DecimalValue DecodeDecimalEnvelope(ReadOnlySpan<byte> envelope)
    {
        return DecodeDecimalFrame(DecodeEnvelope(DecimalKind, envelope));
    }

    /// <summary>Strictly decodes a quantity pointer envelope.</summary>
    public static QuantityValue DecodeQuantityEnvelope(ReadOnlySpan<byte> envelope)
    {
        return DecodeQuantityFrame(DecodeEnvelope(QuantityKind, envelope));
    }

    private static byte[] EncodeFrame(NumericKind kind, BigInteger mantissa, int scale)
    {
        var twos = EncodeTwos(mantissa);
        var body = new byte[sizeof(uint) + twos.Length + (kind.Scaled ? 1 : 0)];
        BinaryPrimitives.WriteUInt32LittleEndian(body, (uint)twos.Length);
        twos.CopyTo(body, sizeof(uint));
        if (kind.Scaled)
        {
            body[^1] = checked((byte)scale);
        }

        var frame = new byte[FrameHeaderBytes + body.Length];
        Magic.CopyTo(frame, 0);
        kind.SchemaHash.CopyTo(frame, 6);
        BinaryPrimitives.WriteUInt64LittleEndian(frame.AsSpan(23, sizeof(ulong)), (ulong)body.Length);
        BinaryPrimitives.WriteUInt64LittleEndian(
            frame.AsSpan(31, sizeof(ulong)),
            Crc64Ecma.Compute(body));
        body.CopyTo(frame, FrameHeaderBytes);
        return frame;
    }

    private static Scaled DecodeFrame(NumericKind kind, ReadOnlySpan<byte> frame)
    {
        var maximum = MaximumFrameBytes(kind);
        if (frame.Length < FrameHeaderBytes)
        {
            Fail(ErrorCode.FrameTooShort, "frame is truncated");
        }

        if (frame.Length > maximum)
        {
            Fail(ErrorCode.FrameTooLarge, "frame is oversized");
        }

        if (!frame[..4].SequenceEqual(Magic) || frame[4] != 0 || frame[5] != 0)
        {
            Fail(ErrorCode.InvalidHeader, "frame has the wrong magic or version");
        }

        if (!frame.Slice(6, 16).SequenceEqual(kind.SchemaHash))
        {
            Fail(ErrorCode.SchemaMismatch, "frame schema does not match");
        }

        if (frame[22] != 0)
        {
            Fail(ErrorCode.CompressionNotAllowed, "compression is forbidden");
        }

        if (frame[39] != 0)
        {
            Fail(ErrorCode.LayoutFlagsNotAllowed, "layout flags must be zero");
        }

        var bodyLength = BinaryPrimitives.ReadUInt64LittleEndian(frame.Slice(23, sizeof(ulong)));
        if (bodyLength != (ulong)(frame.Length - FrameHeaderBytes))
        {
            Fail(ErrorCode.LengthMismatch, "frame length is inconsistent");
        }

        var body = frame[FrameHeaderBytes..];
        var suppliedChecksum = BinaryPrimitives.ReadUInt64LittleEndian(frame.Slice(31, sizeof(ulong)));
        if (suppliedChecksum != Crc64Ecma.Compute(body))
        {
            Fail(ErrorCode.ChecksumMismatch, "frame checksum failed");
        }

        if (body.Length < sizeof(uint))
        {
            Fail(ErrorCode.LengthMismatch, "body has no mantissa length");
        }

        var mantissaLength = BinaryPrimitives.ReadUInt32LittleEndian(body);
        if (mantissaLength > MaxMantissaBytes)
        {
            Fail(ErrorCode.MantissaOverflow, "mantissa length exceeds 64 bytes");
        }

        var expected = sizeof(uint) + checked((int)mantissaLength) + (kind.Scaled ? 1 : 0);
        if (expected != body.Length)
        {
            Fail(ErrorCode.LengthMismatch, "body length is inconsistent");
        }

        var mantissa = DecodeTwos(body.Slice(sizeof(uint), (int)mantissaLength));
        if (!kind.Scaled)
        {
            return new Scaled(mantissa, 0);
        }

        var scale = body[^1];
        if (scale > MaxScale)
        {
            Fail(ErrorCode.InvalidScale, "scale exceeds 28");
        }

        if ((mantissa.IsZero && scale != 0)
            || (scale > 0 && BigInteger.Remainder(mantissa, 10).IsZero))
        {
            Fail(ErrorCode.NoncanonicalDecimal, "scaled value is not canonical");
        }

        if (kind == QuantityKind && mantissa.Sign < 0)
        {
            Fail(ErrorCode.NegativeQuantity, "quantity cannot be negative");
        }

        return new Scaled(mantissa, scale);
    }

    private static byte[] EncodeEnvelope(NumericKind kind, ReadOnlySpan<byte> frame)
    {
        var envelope = new byte[EnvelopeHeaderBytes + frame.Length + HashBytes];
        BinaryPrimitives.WriteUInt16BigEndian(envelope, kind.PointerType);
        envelope[2] = 1;
        BinaryPrimitives.WriteUInt32BigEndian(envelope.AsSpan(3, sizeof(uint)), (uint)frame.Length);
        frame.CopyTo(envelope.AsSpan(EnvelopeHeaderBytes));
        PayloadHash(frame).CopyTo(envelope, EnvelopeHeaderBytes + frame.Length);
        return envelope;
    }

    private static byte[] DecodeEnvelope(NumericKind kind, ReadOnlySpan<byte> envelope)
    {
        if (envelope.Length < EnvelopeHeaderBytes)
        {
            Fail(ErrorCode.TruncatedEnvelope, "envelope is truncated");
        }

        var pointerType = BinaryPrimitives.ReadUInt16BigEndian(envelope);
        if (pointerType == RetiredAmountPointerType)
        {
            Fail(ErrorCode.TypeNotAllowed, "retired Amount pointer type is permanently reserved");
        }

        var knownAllowedType = pointerType is >= 0x0001 and <= 0x000F
            || pointerType is >= 0x0011 and <= 0x0013;
        if (!knownAllowedType)
        {
            Fail(ErrorCode.UnknownType, "unknown pointer type");
        }

        if (pointerType != kind.PointerType)
        {
            Fail(ErrorCode.WrongType, "pointer type does not match");
        }

        if (envelope[2] != 1)
        {
            Fail(ErrorCode.InvalidEnvelopeVersion, "version must be 1");
        }

        var frameLength = BinaryPrimitives.ReadUInt32BigEndian(envelope.Slice(3, sizeof(uint)));
        if (frameLength > MaximumFrameBytes(kind))
        {
            Fail(ErrorCode.OversizedLength, "declared frame is oversized");
        }

        var expectedLength = (ulong)EnvelopeHeaderBytes + frameLength + HashBytes;
        if (expectedLength != (ulong)envelope.Length)
        {
            Fail(ErrorCode.TruncatedEnvelope, "envelope length is inconsistent");
        }

        var frame = envelope.Slice(EnvelopeHeaderBytes, (int)frameLength);
        var suppliedHash = envelope.Slice(EnvelopeHeaderBytes + (int)frameLength, HashBytes);
        if (!CryptographicOperations.FixedTimeEquals(PayloadHash(frame), suppliedHash))
        {
            Fail(ErrorCode.PayloadHashMismatch, "payload hash failed");
        }

        return frame.ToArray();
    }

    private static byte[] EncodeTwos(BigInteger value)
    {
        CheckedMantissa(value);
        if (value.IsZero)
        {
            return [];
        }

        var bytes = value.ToByteArray(isUnsigned: false, isBigEndian: false);
        if (bytes.Length > MaxMantissaBytes)
        {
            Fail(ErrorCode.MantissaOverflow, "mantissa is too wide");
        }

        return bytes;
    }

    private static BigInteger DecodeTwos(ReadOnlySpan<byte> bytes)
    {
        if (bytes.Length > MaxMantissaBytes)
        {
            Fail(ErrorCode.MantissaOverflow, "mantissa is too wide");
        }

        if (bytes.IsEmpty)
        {
            return BigInteger.Zero;
        }

        var last = bytes[^1];
        if (bytes.Length == 1 && last == 0)
        {
            Fail(ErrorCode.NoncanonicalMantissa, "zero must use an empty mantissa");
        }

        if (bytes.Length > 1)
        {
            var previous = bytes[^2];
            if ((last == 0 && (previous & 0x80) == 0)
                || (last == 0xFF && (previous & 0x80) != 0))
            {
                Fail(ErrorCode.NoncanonicalMantissa, "mantissa has redundant sign extension");
            }
        }

        return CheckedMantissa(new BigInteger(bytes, isUnsigned: false, isBigEndian: false));
    }

    private static byte[] PayloadHash(ReadOnlySpan<byte> frame)
    {
        var digest = Blake2b.Hash256(frame);
        digest[^1] |= 1;
        return digest;
    }

    private static Scaled ParseScaled(string raw, bool quantity)
    {
        ArgumentNullException.ThrowIfNull(raw);
        var match = ExactDecimalRegex.Match(raw);
        if (!match.Success || raw == "-0")
        {
            Fail(ErrorCode.InvalidText, "value must use exact decimal syntax");
        }

        var fraction = match.Groups[3].Success ? match.Groups[3].Value : string.Empty;
        var rawDigits = string.Concat(match.Groups[2].Value, fraction);
        var first = 0;
        while (first < rawDigits.Length && rawDigits[first] == '0')
        {
            first++;
        }

        if (first == rawDigits.Length)
        {
            return NormalizeScaled(BigInteger.Zero, 0, quantity);
        }

        var end = rawDigits.Length;
        var scale = fraction.Length;
        while (scale > 0 && rawDigits[end - 1] == '0')
        {
            end--;
            scale--;
        }

        if (scale > MaxScale)
        {
            Fail(ErrorCode.InvalidScale, "canonical scale exceeds 28");
        }

        if (end - first > MaxSignificantDigits)
        {
            Fail(ErrorCode.MantissaOverflow, "decimal mantissa exceeds the signed 512-bit input bound");
        }

        var magnitude = BigInteger.Parse(
            rawDigits.AsSpan(first, end - first),
            System.Globalization.CultureInfo.InvariantCulture);
        var mantissa = match.Groups[1].Value == "-" ? -magnitude : magnitude;
        return NormalizeScaled(mantissa, scale, quantity);
    }

    private static Scaled NormalizeScaled(BigInteger rawMantissa, int rawScale, bool quantity)
    {
        if (rawScale < 0)
        {
            Fail(ErrorCode.InvalidScale, "scale cannot be negative");
        }

        var mantissa = rawMantissa;
        var scale = rawScale;
        if (mantissa.IsZero)
        {
            scale = 0;
        }
        else
        {
            while (scale > 0 && BigInteger.Remainder(mantissa, 10).IsZero)
            {
                mantissa /= 10;
                scale--;
            }
        }

        if (scale > MaxScale)
        {
            Fail(ErrorCode.InvalidScale, "canonical scale exceeds 28");
        }

        CheckedMantissa(mantissa);
        if (quantity && mantissa.Sign < 0)
        {
            Fail(ErrorCode.NegativeQuantity, "quantity cannot be negative");
        }

        return new Scaled(mantissa, scale);
    }

    private static string ScaledText(BigInteger mantissa, int scale)
    {
        if (scale == 0)
        {
            return mantissa.ToString(System.Globalization.CultureInfo.InvariantCulture);
        }

        var digits = BigInteger.Abs(mantissa).ToString(System.Globalization.CultureInfo.InvariantCulture);
        if (digits.Length <= scale)
        {
            digits = digits.PadLeft(scale + 1, '0');
        }

        var split = digits.Length - scale;
        var sign = mantissa.Sign < 0 ? "-" : string.Empty;
        return sign + digits[..split] + "." + digits[split..];
    }

    private static BigInteger CheckedMantissa(BigInteger value)
    {
        if (value < IntMin || value > IntMax)
        {
            Fail(ErrorCode.MantissaOverflow, "mantissa is outside the signed 512-bit domain");
        }

        return value;
    }

    private static string RequiredJsonString(ref Utf8JsonReader reader, string typeName)
    {
        if (reader.TokenType != JsonTokenType.String)
        {
            Fail(ErrorCode.InvalidText, $"{typeName} JSON must be a canonical string");
        }

        return reader.GetString()!;
    }

    private static int MaximumFrameBytes(NumericKind kind)
    {
        return FrameHeaderBytes + sizeof(uint) + MaxMantissaBytes + (kind.Scaled ? 1 : 0);
    }

    private static void Fail(ErrorCode code, string message)
    {
        throw new NumericException(code, message);
    }

    private sealed record NumericKind(byte[] SchemaHash, ushort PointerType, bool Scaled);

    private readonly record struct Scaled(BigInteger Mantissa, int Scale);

    private static readonly Regex CanonicalIntegerRegex = new(
        "^-?(?:0|[1-9][0-9]*)$",
        RegexOptions.CultureInvariant | RegexOptions.NonBacktracking);

    private static readonly Regex ExactDecimalRegex = new(
        "^(-?)(0|[1-9][0-9]*)(?:\\.([0-9]+))?$",
        RegexOptions.CultureInvariant | RegexOptions.NonBacktracking);

    private static readonly byte[] Magic = "NRT0"u8.ToArray();

    private static readonly NumericKind IntKind = new(
        Convert.FromHexString("07c039457363b9e1d36bbd31d93dec4a"),
        0x0011,
        false);

    private static readonly NumericKind DecimalKind = new(
        Convert.FromHexString("ba2ffed52e4d8ee16f17efefe1828524"),
        0x0012,
        true);

    private static readonly NumericKind QuantityKind = new(
        Convert.FromHexString("e4769984c81ce0e8b678f2eb06274ee3"),
        0x0013,
        true);

    private const ushort RetiredAmountPointerType = 0x0010;
    private const int MaxMantissaBytes = 64;
    private const int MaxIntTextBytes = 155;
    private const int MaxSignificantDigits = 154;
    private const int FrameHeaderBytes = 40;
    private const int EnvelopeHeaderBytes = 7;
    private const int HashBytes = 32;
}
