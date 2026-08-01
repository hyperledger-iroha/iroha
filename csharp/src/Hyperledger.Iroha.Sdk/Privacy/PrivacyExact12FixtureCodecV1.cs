using System.Buffers;
using System.Buffers.Binary;
using System.Text;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Privacy;

/// <summary>
/// Strict native-independent codec for the canonical first-release exact-12 fixture bundle.
/// </summary>
public static class PrivacyExact12FixtureCodecV1
{
    public const string SchemaName = "iroha.privacy.exact12-typed-fixture-bundle.v1";
    public const string SubmitProofWireId = "iroha.privacy.submit_proof.v1";
    public const uint Version = 1;
    public const int RowCount = 12;
    public const int HashBytes = 32;
    public const int MaxArchiveBytes = 2 * 1024 * 1024;
    public const long MaxAggregateNestedBytes = 2L * 1024 * 1024;
    public const int MaxStatementBytes = 256 * 1024;
    public const int MaxEnvelopeBytes = 512 * 1024;
    public const int MaxInstructionBytes = 512 * 1024;
    public const int MaxIntentProjectionBytes = 512 * 1024;
    public const int MaxUnsignedTransactionBytes = 768 * 1024;
    public const int MaxSignedTransactionBytes = 1024 * 1024;

    private const byte CanonicalFlags = 0x02;
    private const int HeaderCompressionOffset = 22;
    private const int HeaderPayloadLengthOffset = 23;
    private const int HeaderFlagsOffset = NoritoHeader.EncodedLength - 1;
    private const int MaximumRowEncodedBytes = MaxArchiveBytes;
    private const int MaximumWireIdEncodedBytes = 128;
    private const int MaximumPayloadBytes = MaxArchiveBytes - NoritoHeader.EncodedLength;

    private static readonly UTF8Encoding StrictUtf8 = new(
        encoderShouldEmitUTF8Identifier: false,
        throwOnInvalidBytes: true);

    /// <summary>
    /// Decode a complete canonical archive using only managed code.
    /// </summary>
    /// <remarks>
    /// The decoder accepts only schema-bound, checksum-valid, uncompressed Norito with the exact
    /// compact-length flag. Every declared size is bounded before slicing or allocating, every
    /// compact integer is minimally encoded, and all nested fields must be consumed exactly.
    /// </remarks>
    public static PrivacyExact12FixtureBundleV1 DecodeCanonical(byte[] archive)
    {
        ArgumentNullException.ThrowIfNull(archive);
        if (archive.Length == 0)
        {
            throw new ArgumentException("Exact-12 fixture archive must not be empty.", nameof(archive));
        }

        if (archive.Length > MaxArchiveBytes)
        {
            throw new ArgumentException(
                $"Exact-12 fixture archive exceeds {MaxArchiveBytes} bytes.",
                nameof(archive));
        }

        var snapshot = (byte[])archive.Clone();
        if (snapshot.Length < NoritoHeader.EncodedLength)
        {
            throw new ArgumentException(
                "Exact-12 fixture archive is truncated before the Norito header.",
                nameof(archive));
        }

        if (snapshot[HeaderCompressionOffset] != (byte)NoritoCompression.None)
        {
            throw new ArgumentException(
                "Exact-12 fixture must use uncompressed Norito.",
                nameof(archive));
        }

        if (snapshot[HeaderFlagsOffset] != CanonicalFlags)
        {
            throw new ArgumentException(
                "Exact-12 fixture must use only the canonical compact-length flag.",
                nameof(archive));
        }

        var declaredPayloadLength = BinaryPrimitives.ReadUInt64LittleEndian(
            snapshot.AsSpan(HeaderPayloadLengthOffset, sizeof(ulong)));
        if (declaredPayloadLength > MaximumPayloadBytes
            || declaredPayloadLength != (ulong)(snapshot.Length - NoritoHeader.EncodedLength))
        {
            throw new ArgumentException(
                "Exact-12 fixture payload length does not cover one bounded complete archive.",
                nameof(archive));
        }

        var decoded = NoritoCodec.Decode(SchemaName, snapshot);
        if (decoded.Flags != CanonicalFlags)
        {
            throw new ArgumentException(
                "Exact-12 fixture layout flags changed during header validation.",
                nameof(archive));
        }

        var reader = new CanonicalReader(decoded.Payload);
        var versionReader = reader.ReadSizedField(sizeof(uint), "bundle version");
        var version = versionReader.ReadUInt32("bundle version");
        versionReader.RequireEnd("bundle version");

        var aggregateBudget = new DecodeBudget(MaxAggregateNestedBytes);
        var rowsReader = reader.ReadSizedField(MaximumPayloadBytes, "bundle rows");
        var declaredRows = rowsReader.ReadUInt64("bundle row count");
        if (declaredRows != RowCount)
        {
            throw new ArgumentException($"Exact-12 fixture must declare exactly {RowCount} rows.");
        }

        var rows = new PrivacyExact12TypedFixtureRowV1[RowCount];
        for (var index = 0; index < RowCount; index++)
        {
            var rowReader = rowsReader.ReadSizedField(
                MaximumRowEncodedBytes,
                $"row {index}");
            rows[index] = DecodeRow(ref rowReader, aggregateBudget, index);
            rowReader.RequireEnd($"row {index}");
        }

        rowsReader.RequireEnd("bundle rows");
        reader.RequireEnd("exact-12 bundle payload");
        var bundle = new PrivacyExact12FixtureBundleV1(version, rows);
        var canonical = EncodeCanonical(bundle);
        if (!snapshot.AsSpan().SequenceEqual(canonical))
        {
            throw new ArgumentException(
                "Exact-12 fixture is not byte-canonical Norito.",
                nameof(archive));
        }

        return bundle;
    }

    /// <summary>Encode one validated bundle with the exact first-release schema and flags.</summary>
    public static byte[] EncodeCanonical(PrivacyExact12FixtureBundleV1 bundle)
    {
        ArgumentNullException.ThrowIfNull(bundle);
        var payload = new CanonicalWriter(MaximumPayloadBytes);
        payload.WriteSizedField(
            writer => writer.WriteUInt32(bundle.Version),
            "bundle version");
        payload.WriteSizedField(
            rowsWriter =>
            {
                rowsWriter.WriteUInt64(RowCount);
                foreach (var row in bundle.Rows)
                {
                    rowsWriter.WriteSizedField(
                        rowWriter => EncodeRow(rowWriter, row),
                        "exact-12 row");
                }
            },
            "bundle rows");

        var archive = NoritoCodec.Encode(SchemaName, payload.ToArray(), CanonicalFlags);
        if (archive.Length > MaxArchiveBytes)
        {
            throw new ArgumentException(
                $"Exact-12 fixture archive exceeds {MaxArchiveBytes} bytes.",
                nameof(bundle));
        }

        return archive;
    }

    /// <summary>Decode canonical padded standard Base64 without accepting whitespace.</summary>
    public static PrivacyExact12FixtureBundleV1 DecodeCanonicalBase64(string encoded)
    {
        ArgumentNullException.ThrowIfNull(encoded);
        if (encoded.Length == 0)
        {
            throw new ArgumentException("Exact-12 fixture Base64 must not be empty.", nameof(encoded));
        }

        if ((long)encoded.Length > CanonicalBase64EncodedLength(MaxArchiveBytes))
        {
            throw new ArgumentException(
                "Exact-12 fixture Base64 exceeds the archive limit.",
                nameof(encoded));
        }

        var decodedLength = ValidateCanonicalBase64Shape(encoded);
        if (decodedLength > MaxArchiveBytes)
        {
            throw new ArgumentException(
                "Exact-12 fixture Base64 declares an oversized decoded archive.",
                nameof(encoded));
        }

        byte[] archive;
        try
        {
            archive = Convert.FromBase64String(encoded);
        }
        catch (FormatException error)
        {
            throw new ArgumentException(
                "Exact-12 fixture must use canonical standard Base64.",
                nameof(encoded),
                error);
        }

        if (!string.Equals(Convert.ToBase64String(archive), encoded, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "Exact-12 fixture must use canonical padded standard Base64 without whitespace.",
                nameof(encoded));
        }

        return DecodeCanonical(archive);
    }

    /// <summary>Encode one validated bundle as canonical padded standard Base64.</summary>
    public static string EncodeCanonicalBase64(PrivacyExact12FixtureBundleV1 bundle) =>
        Convert.ToBase64String(EncodeCanonical(bundle));

    /// <summary>
    /// Decode a candidate and require byte identity with an independently trusted canonical archive.
    /// </summary>
    /// <remarks>
    /// Structural decoding alone cannot assign semantics to opaque nested statement, envelope, or
    /// transaction archives. This path first validates immutable snapshots of both inputs and then
    /// compares them byte-for-byte, rejecting row reordering, same-shape substitution, and any byte
    /// mutation even when an attacker recomputes the outer checksum.
    /// </remarks>
    public static PrivacyExact12FixtureBundleV1 RequireTrustedCanonical(
        byte[] candidate,
        byte[] trustedCanonicalArchive)
    {
        ArgumentNullException.ThrowIfNull(candidate);
        ArgumentNullException.ThrowIfNull(trustedCanonicalArchive);
        var candidateSnapshot = (byte[])candidate.Clone();
        var trustedSnapshot = (byte[])trustedCanonicalArchive.Clone();
        _ = DecodeCanonical(trustedSnapshot);
        var decoded = DecodeCanonical(candidateSnapshot);
        if (!candidateSnapshot.AsSpan().SequenceEqual(trustedSnapshot))
        {
            throw new ArgumentException(
                "Exact-12 fixture differs from the supplied trusted canonical archive.",
                nameof(candidate));
        }

        return decoded;
    }

    /// <summary>Compute the padded standard Base64 length without allocating an archive.</summary>
    public static long CanonicalBase64EncodedLength(long decodedByteCount)
    {
        if (decodedByteCount < 0)
        {
            throw new ArgumentOutOfRangeException(nameof(decodedByteCount));
        }

        try
        {
            var groups = checked(
                decodedByteCount / 3 + (decodedByteCount % 3 == 0 ? 0 : 1));
            return checked(groups * 4);
        }
        catch (OverflowException)
        {
            throw new ArgumentOutOfRangeException(
                nameof(decodedByteCount),
                decodedByteCount,
                "Canonical Base64 length exceeds the supported range.");
        }
    }

    private static PrivacyExact12TypedFixtureRowV1 DecodeRow(
        ref CanonicalReader reader,
        DecodeBudget budget,
        int expectedIndex)
    {
        var protocolReader = reader.ReadSizedField(sizeof(uint), "protocol ID");
        var protocolTag = protocolReader.ReadUInt32("protocol ID");
        protocolReader.RequireEnd("protocol ID");
        if (protocolTag >= RowCount || protocolTag != (uint)expectedIndex)
        {
            throw new ArgumentException(
                $"Exact-12 row {expectedIndex} has an unknown or out-of-order protocol ID.");
        }

        var statement = ReadRawBytesField(
            ref reader,
            MaxStatementBytes,
            budget,
            "statement");
        var envelope = ReadRawBytesField(
            ref reader,
            MaxEnvelopeBytes,
            budget,
            "envelope");
        var wireId = ReadWireId(ref reader, budget);
        var instruction = ReadRawBytesField(
            ref reader,
            MaxInstructionBytes,
            budget,
            "submit-proof instruction");
        var projection = ReadRawBytesField(
            ref reader,
            MaxIntentProjectionBytes,
            budget,
            "transaction-intent projection");
        var intentDigest = ReadFixedHashField(
            ref reader,
            budget,
            "transaction-intent digest");
        var unsignedTransaction = ReadRawBytesField(
            ref reader,
            MaxUnsignedTransactionBytes,
            budget,
            "unsigned transaction payload");
        var signedTransaction = ReadRawBytesField(
            ref reader,
            MaxSignedTransactionBytes,
            budget,
            "signed transaction");
        var transactionHash = ReadFixedHashField(
            ref reader,
            budget,
            "signed transaction hash");

        return new PrivacyExact12TypedFixtureRowV1(
            (PrivacyProtocolIdV1)protocolTag,
            statement,
            envelope,
            wireId,
            instruction,
            projection,
            intentDigest,
            unsignedTransaction,
            signedTransaction,
            transactionHash);
    }

    private static int ValidateCanonicalBase64Shape(string encoded)
    {
        if ((encoded.Length & 3) != 0)
        {
            throw new ArgumentException(
                "Exact-12 fixture Base64 length must be divisible by four.",
                nameof(encoded));
        }

        var padding = 0;
        if (encoded.Length > 0 && encoded[^1] == '=')
        {
            padding++;
        }

        if (encoded.Length > 1 && encoded[^2] == '=')
        {
            padding++;
        }

        var payloadLength = encoded.Length - padding;
        for (var index = 0; index < encoded.Length; index++)
        {
            var value = encoded[index];
            var alphabet = (value >= 'A' && value <= 'Z')
                || (value >= 'a' && value <= 'z')
                || (value >= '0' && value <= '9')
                || value == '+'
                || value == '/';
            if (index < payloadLength ? !alphabet : value != '=')
            {
                throw new ArgumentException(
                    "Exact-12 fixture must use only the padded standard Base64 alphabet.",
                    nameof(encoded));
            }
        }

        var decodedLength = checked(encoded.Length / 4 * 3 - padding);
        if (decodedLength == 0)
        {
            throw new ArgumentException(
                "Exact-12 fixture Base64 must decode to a non-empty archive.",
                nameof(encoded));
        }

        return decodedLength;
    }

    private static void EncodeRow(
        CanonicalWriter writer,
        PrivacyExact12TypedFixtureRowV1 row)
    {
        writer.WriteSizedField(
            child => child.WriteUInt32((uint)row.ProtocolId),
            "protocol ID");
        WriteRawBytesField(writer, row.StatementNoritoSpan, "statement");
        WriteRawBytesField(writer, row.EnvelopeNoritoSpan, "envelope");
        writer.WriteSizedField(
            child =>
            {
                var bytes = StrictUtf8.GetBytes(row.SubmitProofWireId);
                child.WriteCompactLength((ulong)bytes.Length);
                child.WriteBytes(bytes);
            },
            "submit-proof wire ID");
        WriteRawBytesField(
            writer,
            row.SubmitProofInstructionNoritoSpan,
            "submit-proof instruction");
        WriteRawBytesField(
            writer,
            row.TransactionIntentProjectionNoritoSpan,
            "transaction-intent projection");
        var intentDigest = row.TransactionIntentDigestSpan.ToArray();
        writer.WriteSizedField(
            child => child.WriteBytes(intentDigest),
            "transaction-intent digest");
        WriteRawBytesField(
            writer,
            row.UnsignedTransactionPayloadNoritoSpan,
            "unsigned transaction payload");
        WriteRawBytesField(
            writer,
            row.SignedTransactionVersionedNoritoSpan,
            "signed transaction");
        var transactionHash = row.SignedTransactionHashSpan.ToArray();
        writer.WriteSizedField(
            child => child.WriteBytes(transactionHash),
            "signed transaction hash");
    }

    private static void WriteRawBytesField(
        CanonicalWriter writer,
        ReadOnlySpan<byte> bytes,
        string fieldName)
    {
        var snapshot = bytes.ToArray();
        writer.WriteSizedField(
            child =>
            {
                child.WriteUInt64((ulong)snapshot.Length);
                child.WriteBytes(snapshot);
            },
            fieldName);
    }

    private static byte[] ReadRawBytesField(
        ref CanonicalReader reader,
        int maximum,
        DecodeBudget budget,
        string fieldName)
    {
        var fieldReader = reader.ReadSizedField(
            checked(maximum + sizeof(ulong)),
            fieldName,
            minimum: sizeof(ulong) + 1);
        var declaredLength = fieldReader.ReadUInt64($"{fieldName} byte length");
        if (declaredLength == 0
            || declaredLength > (ulong)maximum
            || declaredLength != (ulong)fieldReader.Remaining)
        {
            throw new ArgumentException(
                $"{fieldName} declares an invalid or incomplete byte length.");
        }

        budget.Claim(declaredLength, fieldName);
        var value = fieldReader.ReadBytes(checked((int)declaredLength), fieldName);
        fieldReader.RequireEnd(fieldName);
        return value;
    }

    private static byte[] ReadFixedHashField(
        ref CanonicalReader reader,
        DecodeBudget budget,
        string fieldName)
    {
        var fieldReader = reader.ReadSizedField(HashBytes, fieldName, minimum: HashBytes);
        budget.Claim(HashBytes, fieldName);
        var hash = fieldReader.ReadBytes(HashBytes, fieldName);
        fieldReader.RequireEnd(fieldName);
        return hash;
    }

    private static string ReadWireId(ref CanonicalReader reader, DecodeBudget budget)
    {
        var fieldReader = reader.ReadSizedField(
            MaximumWireIdEncodedBytes,
            "submit-proof wire ID",
            minimum: 1);
        var byteLength = fieldReader.ReadCompactLength("submit-proof wire ID byte length");
        if (byteLength == 0
            || byteLength > MaximumWireIdEncodedBytes
            || byteLength != (ulong)fieldReader.Remaining)
        {
            throw new ArgumentException(
                "Submit-proof wire ID declares an invalid or incomplete byte length.");
        }

        var bytes = fieldReader.ReadBytes(checked((int)byteLength), "submit-proof wire ID");
        fieldReader.RequireEnd("submit-proof wire ID");
        string wireId;
        try
        {
            wireId = StrictUtf8.GetString(bytes);
        }
        catch (DecoderFallbackException error)
        {
            throw new ArgumentException("Submit-proof wire ID is not valid UTF-8.", error);
        }

        if (!string.Equals(wireId, SubmitProofWireId, StringComparison.Ordinal))
        {
            throw new ArgumentException("Unknown or retired submit-proof wire ID.");
        }

        budget.Claim(byteLength, "submit-proof wire ID");
        return wireId;
    }

    private sealed class DecodeBudget
    {
        private readonly ulong _maximum;
        private ulong _used;

        internal DecodeBudget(long maximum)
        {
            _maximum = checked((ulong)maximum);
        }

        internal void Claim(ulong bytes, string fieldName)
        {
            try
            {
                _used = checked(_used + bytes);
            }
            catch (OverflowException error)
            {
                throw new ArgumentException(
                    "Exact-12 aggregate byte count overflowed.",
                    fieldName,
                    error);
            }

            if (_used > _maximum)
            {
                throw new ArgumentException(
                    $"Exact-12 aggregate nested-byte limit exceeded at {fieldName}.");
            }
        }
    }

    private ref struct CanonicalReader
    {
        private readonly ReadOnlySpan<byte> _bytes;
        private int _offset;

        internal CanonicalReader(ReadOnlySpan<byte> bytes)
        {
            _bytes = bytes;
            _offset = 0;
        }

        internal int Remaining => _bytes.Length - _offset;

        internal CanonicalReader ReadSizedField(
            int maximum,
            string fieldName,
            int minimum = 0)
        {
            var length = ReadCompactLength($"{fieldName} encoded length");
            if (length > (ulong)maximum || length < (ulong)minimum || length > (ulong)Remaining)
            {
                throw new ArgumentException(
                    $"{fieldName} declares an invalid or oversized encoded length.");
            }

            var count = checked((int)length);
            var field = new CanonicalReader(_bytes.Slice(_offset, count));
            _offset += count;
            return field;
        }

        internal byte[] ReadBytes(int count, string fieldName)
        {
            if (count < 0 || count > Remaining)
            {
                throw new ArgumentException($"{fieldName} is truncated.");
            }

            var result = _bytes.Slice(_offset, count).ToArray();
            _offset += count;
            return result;
        }

        internal uint ReadUInt32(string fieldName)
        {
            if (Remaining < sizeof(uint))
            {
                throw new ArgumentException($"{fieldName} is truncated.");
            }

            var value = BinaryPrimitives.ReadUInt32LittleEndian(
                _bytes.Slice(_offset, sizeof(uint)));
            _offset += sizeof(uint);
            return value;
        }

        internal ulong ReadUInt64(string fieldName)
        {
            if (Remaining < sizeof(ulong))
            {
                throw new ArgumentException($"{fieldName} is truncated.");
            }

            var value = BinaryPrimitives.ReadUInt64LittleEndian(
                _bytes.Slice(_offset, sizeof(ulong)));
            _offset += sizeof(ulong);
            return value;
        }

        internal ulong ReadCompactLength(string fieldName)
        {
            ulong value = 0;
            var shift = 0;
            for (var index = 0; index < 10; index++)
            {
                if (Remaining == 0)
                {
                    throw new ArgumentException($"{fieldName} is truncated.");
                }

                var current = _bytes[_offset++];
                var chunk = (ulong)(current & 0x7F);
                if (shift == 63 && chunk > 1)
                {
                    throw new ArgumentException($"{fieldName} exceeds 64 bits.");
                }

                value |= chunk << shift;
                if ((current & 0x80) == 0)
                {
                    if (index > 0 && chunk == 0)
                    {
                        throw new ArgumentException(
                            $"{fieldName} is not minimally encoded.");
                    }

                    return value;
                }

                shift += 7;
            }

            throw new ArgumentException($"{fieldName} exceeds 64 bits.");
        }

        internal void RequireEnd(string fieldName)
        {
            if (Remaining != 0)
            {
                throw new ArgumentException($"{fieldName} contains trailing or unknown data.");
            }
        }
    }

    private sealed class CanonicalWriter
    {
        private readonly ArrayBufferWriter<byte> _buffer = new();
        private readonly int _maximum;

        internal CanonicalWriter(int maximum)
        {
            if (maximum < 0)
            {
                throw new ArgumentOutOfRangeException(nameof(maximum));
            }

            _maximum = maximum;
        }

        internal void WriteUInt32(uint value)
        {
            Span<byte> encoded = stackalloc byte[sizeof(uint)];
            BinaryPrimitives.WriteUInt32LittleEndian(encoded, value);
            WriteBytes(encoded);
        }

        internal void WriteUInt64(ulong value)
        {
            Span<byte> encoded = stackalloc byte[sizeof(ulong)];
            BinaryPrimitives.WriteUInt64LittleEndian(encoded, value);
            WriteBytes(encoded);
        }

        internal void WriteCompactLength(ulong value)
        {
            Span<byte> encoded = stackalloc byte[10];
            var count = 0;
            do
            {
                var next = (byte)(value & 0x7F);
                value >>= 7;
                if (value != 0)
                {
                    next |= 0x80;
                }

                encoded[count++] = next;
            }
            while (value != 0);

            WriteBytes(encoded[..count]);
        }

        internal void WriteBytes(ReadOnlySpan<byte> bytes)
        {
            if (bytes.Length > _maximum - _buffer.WrittenCount)
            {
                throw new ArgumentException(
                    "Exact-12 canonical encoding exceeds its bounded output size.");
            }

            bytes.CopyTo(_buffer.GetSpan(bytes.Length));
            _buffer.Advance(bytes.Length);
        }

        internal void WriteSizedField(Action<CanonicalWriter> encode, string fieldName)
        {
            ArgumentNullException.ThrowIfNull(encode);
            var child = new CanonicalWriter(_maximum);
            encode(child);
            var bytes = child.ToArray();
            try
            {
                WriteCompactLength((ulong)bytes.Length);
                WriteBytes(bytes);
            }
            catch (ArgumentException error)
            {
                throw new ArgumentException(
                    $"{fieldName} exceeds the bounded exact-12 output size.",
                    fieldName,
                    error);
            }
        }

        internal byte[] ToArray() => _buffer.WrittenSpan.ToArray();
    }
}
