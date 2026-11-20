using System.Buffers.Binary;
using System.Text.Json;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>
/// The shared dense-layout vectors (<c>fixtures/norito_dense/vectors.json</c>): every value
/// decodes and re-encodes byte-identically in both the compact and the dense layout, through the
/// layout-aware reader and writer the SDK's typed codecs use. Needs no native artifact.
/// </summary>
public sealed class NoritoDenseLayoutVectorTests
{
    private const string WideSchemaName = "norito.vectors.dense.Wide";
    private const int WideAlignment = 16;
    private const string WideValue = "{ value: 0x0102030405060708090a0b0c0d0e0f10, tag: [0xca, 0xfe] }";
    private const string RecordValue =
        "{ id: 7, key: [1, 2, 3, 4], digests: [[0xaa; 4], [0xbb; 4]], label: Some(\"hi\"), choice: Far { value: 1 } }";

    private static readonly IReadOnlyDictionary<string, VectorCase> Cases = new[]
    {
        Case("u8_vec", "Vec<u8>", "[1, 2, 3]", DecodeByteVec, EncodeByteVec, RenderBytes),
        Case("u8_vec_128", "Vec<u8>", string.Concat(Enumerable.Repeat("11", 128)), DecodeByteVec, EncodeByteVec, Convert.ToHexStringLower),
        Case("u8_array", "[u8; 4]", "[1, 2, 3, 4]", static (bytes, layout) => DecodeByteArray(bytes, 4, layout), EncodeByteArray, RenderBytes),
        Case("option_u8_array", "Option<[u8; 4]>", "Some([9, 9, 9, 9])", DecodeOptionByteArray, EncodeOptionByteArray, RenderOptionByteArray),
        Case("option_none", "Option<[u8; 4]>", "None", DecodeOptionByteArray, EncodeOptionByteArray, RenderOptionByteArray),
        Case("vec_u8_array", "Vec<[u8; 2]>", "[[7, 7], [8, 8]]", DecodeByteArraySequence, EncodeByteArraySequence, RenderByteArrays),
        Case("vec_string", "Vec<String>", "[\"ab\"]", DecodeStringSequence, EncodeStringSequence, RenderStrings),
        Case("vec_string_empty", "Vec<String>", "[]", DecodeStringSequence, EncodeStringSequence, RenderStrings),
        Case("map_string_u16", "BTreeMap<String, u16>", "{\"k\": 5}", DecodeStringUInt16Map, EncodeStringUInt16Map, RenderStringUInt16Map),
        Case("enum_unit", "Choice (Unit = 0)", "Unit", DecodeChoice, EncodeChoice, RenderChoice),
        Case("enum_tuple", "Choice (One(String) = 1)", "One(\"hi\")", DecodeChoice, EncodeChoice, RenderChoice),
        Case("enum_far", "Choice (Far { value: u64 } = 300)", "Far { value: 1 }", DecodeChoice, EncodeChoice, RenderChoice),
        Case("result_err", "Result<u8, u8>", "Err(7)", DecodeByteResult, EncodeByteResult, RenderByteResult),
        Case(
            "record",
            "Record { id: u32, key: [u8; 4], digests: Vec<[u8; 4]>, label: Option<String>, choice: Choice }",
            RecordValue,
            DecodeRecord,
            EncodeRecord,
            RenderRecord),
    }.ToDictionary(static item => item.Name, StringComparer.Ordinal);

    [Fact]
    public void EveryBareVectorDecodesAndReEncodesByteIdenticallyInBothLayouts()
    {
        using var document = LoadVectors();
        Assert.Equal(
            "norito.md, Dense layout (header minor 1)",
            document.RootElement.GetProperty("layout").GetString());

        var seen = new HashSet<string>(StringComparer.Ordinal);
        foreach (var vector in document.RootElement.GetProperty("bare").EnumerateArray())
        {
            var name = vector.GetProperty("name").GetString()!;
            Assert.True(seen.Add(name), $"duplicate vector {name}");
            Assert.True(Cases.TryGetValue(name, out var vectorCase), $"vector {name} has no typed codec in this test");
            Assert.Equal(vectorCase.Type, vector.GetProperty("type").GetString());

            var compact = RequireLowerHex(vector, "compact", name);
            var dense = RequireLowerHex(vector, "dense", name);

            // Each layout decodes to the named value and re-encodes to the exact fixture bytes.
            var (compactValue, compactBytes) = vectorCase.RoundTrip(compact, NoritoWireLayout.Compact);
            var (denseValue, denseBytes) = vectorCase.RoundTrip(dense, NoritoWireLayout.Dense);
            Assert.Equal(vectorCase.Value, compactValue);
            Assert.Equal(vectorCase.Value, denseValue);
            Assert.Equal(Convert.ToHexStringLower(compact), Convert.ToHexStringLower(compactBytes));
            Assert.Equal(Convert.ToHexStringLower(dense), Convert.ToHexStringLower(denseBytes));
            if (name != "u8_vec_128")
            {
                Assert.Equal(vectorCase.Value, vector.GetProperty("value").GetString());
            }

            // The decoded value transcodes: a typed value owns both of its encodings.
            Assert.Equal(dense, vectorCase.Transcode(compact, NoritoWireLayout.Compact, NoritoWireLayout.Dense));
            Assert.Equal(compact, vectorCase.Transcode(dense, NoritoWireLayout.Dense, NoritoWireLayout.Compact));
        }

        Assert.Equal(Cases.Keys.Order(StringComparer.Ordinal), seen.Order(StringComparer.Ordinal));
    }

    [Fact]
    public void BareVectorsNeverDecodeAsTheOtherLayout()
    {
        using var document = LoadVectors();
        var refused = 0;
        foreach (var vector in document.RootElement.GetProperty("bare").EnumerateArray())
        {
            var name = vector.GetProperty("name").GetString()!;
            var vectorCase = Cases[name];
            var compact = RequireLowerHex(vector, "compact", name);
            var dense = RequireLowerHex(vector, "dense", name);
            if (compact.AsSpan().SequenceEqual(dense))
            {
                // `Option`, `Result` and `bool` tags are identical in both layouts.
                Assert.Contains(name, new[] { "option_none", "result_err" });
                continue;
            }

            Assert.Throws<ArgumentException>(() => vectorCase.RoundTrip(dense, NoritoWireLayout.Compact));
            Assert.Throws<ArgumentException>(() => vectorCase.RoundTrip(compact, NoritoWireLayout.Dense));
            refused++;
        }

        Assert.Equal(Cases.Count - 2, refused);
    }

    [Fact]
    public void BareVectorsRejectTruncationAndTrailingBytesInBothLayouts()
    {
        using var document = LoadVectors();
        foreach (var vector in document.RootElement.GetProperty("bare").EnumerateArray())
        {
            var name = vector.GetProperty("name").GetString()!;
            var vectorCase = Cases[name];
            foreach (var (field, layout) in new[]
            {
                ("compact", NoritoWireLayout.Compact),
                ("dense", NoritoWireLayout.Dense),
            })
            {
                var bytes = RequireLowerHex(vector, field, name);
                for (var length = 0; length < bytes.Length; length++)
                {
                    var truncated = bytes[..length];
                    Assert.Throws<ArgumentException>(() => vectorCase.RoundTrip(truncated, layout));
                }

                Assert.Throws<ArgumentException>(() => vectorCase.RoundTrip([.. bytes, 0], layout));
            }
        }
    }

    [Fact]
    public void DenseCountsTagsAndArraysRejectNonCanonicalAndShortInput()
    {
        // The cases of the Rust codec's `dense_counts_tags_and_arrays_reject_noncanonical_and_short_input`.
        Assert.Equal(new byte[] { 1, 2, 3 }, DecodeByteVec([3, 1, 2, 3], NoritoWireLayout.Dense));
        // Overlong count, truncated body, trailing byte.
        Assert.Throws<ArgumentException>(() => DecodeByteVec([0x83, 0x00, 1, 2, 3], NoritoWireLayout.Dense));
        Assert.Throws<ArgumentException>(() => DecodeByteVec([3, 1, 2], NoritoWireLayout.Dense));
        Assert.Throws<ArgumentException>(() => DecodeByteVec([3, 1, 2, 3, 0], NoritoWireLayout.Dense));

        Assert.Equal(new Choice(0, null, null), DecodeChoice([0], NoritoWireLayout.Dense));
        // Overlong tag, unknown tag, tag beyond `u32`, compact-layout tag width.
        Assert.Throws<ArgumentException>(() => DecodeChoice([0x80, 0x00], NoritoWireLayout.Dense));
        Assert.Throws<ArgumentException>(() => DecodeChoice([2], NoritoWireLayout.Dense));
        Assert.Throws<ArgumentException>(() => DecodeChoice([0x80, 0x80, 0x80, 0x80, 0x10], NoritoWireLayout.Dense));
        Assert.Throws<ArgumentException>(() => DecodeChoice([0, 0, 0, 0], NoritoWireLayout.Dense));

        Assert.Equal(new byte[] { 1, 2, 3, 4 }, DecodeByteArray([1, 2, 3, 4], 4, NoritoWireLayout.Dense));
        Assert.Throws<ArgumentException>(() => DecodeByteArray([1, 2, 3], 4, NoritoWireLayout.Dense));
        Assert.Throws<ArgumentException>(() => DecodeByteArray([1, 1, 1, 2, 1, 3, 1, 4], 4, NoritoWireLayout.Dense));
    }

    [Fact]
    public void FrameVectorsDecodeAndReEncodeByteIdenticallyWithTheHeaderSelectingTheLayout()
    {
        var (compact, dense) = LoadWideFrames();

        // Minor 0 with COMPACT_LEN and the type's 16-byte alignment padding.
        Assert.Equal(NoritoWireLayout.Compact, NoritoCodec.ReadLayout(compact));
        Assert.Equal(0, compact[5]);
        Assert.Equal(0x02, compact[NoritoHeader.EncodedLength - 1]);
        Assert.Equal(8, NoritoCodec.PayloadPadding(NoritoWireLayout.Compact, WideAlignment));
        var compactPayload = NoritoCodec.DecodeWithLayout(WideSchemaName, compact, NoritoWireLayout.Compact, WideAlignment);
        var compactValue = DecodeWide(compactPayload, NoritoWireLayout.Compact);
        Assert.Equal(WideValue, RenderWide(compactValue));
        Assert.Equal(
            compact,
            NoritoCodec.EncodeWithLayout(
                WideSchemaName,
                EncodeWide(compactValue, NoritoWireLayout.Compact),
                NoritoWireLayout.Compact,
                WideAlignment));

        // Minor 1, no flags, no padding: the payload starts at byte 40.
        Assert.Equal(NoritoWireLayout.Dense, NoritoCodec.ReadLayout(dense));
        Assert.Equal(1, dense[5]);
        Assert.Equal(0, dense[NoritoHeader.EncodedLength - 1]);
        Assert.Equal(0, NoritoCodec.PayloadPadding(NoritoWireLayout.Dense, WideAlignment));
        var densePayload = NoritoCodec.DecodeWithLayout(WideSchemaName, dense, NoritoWireLayout.Dense, WideAlignment);
        Assert.Equal(dense[NoritoHeader.EncodedLength..], densePayload);
        var denseValue = DecodeWide(densePayload, NoritoWireLayout.Dense);
        Assert.Equal(compactValue, denseValue);
        Assert.Equal(
            dense,
            NoritoCodec.EncodeWithLayout(
                WideSchemaName,
                EncodeWide(denseValue, NoritoWireLayout.Dense),
                NoritoWireLayout.Dense,
                WideAlignment));

        // Schema hash and CRC64 are the same in both layouts.
        Assert.Equal(NoritoCodec.SchemaHash(WideSchemaName), dense[6..22]);
        Assert.Equal(compact[6..22], dense[6..22]);
        Assert.Equal(compact[31..39], dense[31..39]);
    }

    [Fact]
    public void AFrameNeverDecodesAsTheOtherLayout()
    {
        var (compact, dense) = LoadWideFrames();

        // The current-layout decoder refuses a dense frame; a dense read refuses a current frame.
        Assert.Throws<ArgumentException>(() => NoritoCodec.Decode(WideSchemaName, dense));
        Assert.Throws<ArgumentException>(
            () => NoritoCodec.DecodeWithLayout(WideSchemaName, dense, NoritoWireLayout.Compact, WideAlignment));
        Assert.Throws<ArgumentException>(
            () => NoritoCodec.DecodeWithLayout(WideSchemaName, dense, NoritoWireLayout.Fixed, WideAlignment));
        Assert.Throws<ArgumentException>(
            () => NoritoCodec.DecodeWithLayout(WideSchemaName, compact, NoritoWireLayout.Dense, WideAlignment));
        Assert.Throws<ArgumentException>(
            () => NoritoCodec.DecodeWithLayout(WideSchemaName, compact, NoritoWireLayout.Fixed, WideAlignment));

        // Relabelling a dense frame as minor 0 does not make it a current frame: the 16-aligned
        // type requires its alignment padding there.
        var relabelled = dense.ToArray();
        relabelled[5] = 0;
        relabelled[NoritoHeader.EncodedLength - 1] = 0x02;
        Assert.Throws<ArgumentException>(
            () => NoritoCodec.DecodeWithLayout(WideSchemaName, relabelled, NoritoWireLayout.Compact, WideAlignment));

        // Relabelling a current frame as dense leaves padding that a dense frame never carries.
        var densified = compact.ToArray();
        densified[5] = 1;
        densified[NoritoHeader.EncodedLength - 1] = 0;
        Assert.Equal(NoritoWireLayout.Dense, NoritoCodec.ReadLayout(densified));
        Assert.Throws<ArgumentException>(
            () => NoritoCodec.DecodeWithLayout(WideSchemaName, densified, NoritoWireLayout.Dense, WideAlignment));
    }

    [Fact]
    public void DenseHeadersRejectFlagsUnknownMinorsPaddingTruncationAndTrailingBytes()
    {
        var (_, dense) = LoadWideFrames();

        // Any flag with minor 1.
        for (var flags = 1; flags <= byte.MaxValue; flags++)
        {
            var flagged = dense.ToArray();
            flagged[NoritoHeader.EncodedLength - 1] = (byte)flags;
            Assert.Throws<ArgumentException>(() => NoritoCodec.ReadLayout(flagged));
            Assert.Throws<ArgumentException>(
                () => NoritoCodec.DecodeWithLayout(WideSchemaName, flagged, NoritoWireLayout.Dense, WideAlignment));
            Assert.Throws<ArgumentException>(() => NoritoCodec.Decode(WideSchemaName, flagged));
        }

        // Every minor other than 0 and 1.
        for (var minor = 2; minor <= byte.MaxValue; minor++)
        {
            var future = dense.ToArray();
            future[5] = (byte)minor;
            Assert.Throws<ArgumentException>(() => NoritoCodec.ReadLayout(future));
            Assert.Throws<ArgumentException>(
                () => NoritoCodec.DecodeWithLayout(WideSchemaName, future, NoritoWireLayout.Dense, WideAlignment));
            Assert.Throws<ArgumentException>(() => NoritoCodec.Decode(WideSchemaName, future));
        }

        // Leading padding, with the header length unchanged.
        byte[] padded =
        [
            .. dense[..NoritoHeader.EncodedLength],
            .. new byte[8],
            .. dense[NoritoHeader.EncodedLength..],
        ];
        Assert.Throws<ArgumentException>(
            () => NoritoCodec.DecodeWithLayout(WideSchemaName, padded, NoritoWireLayout.Dense, WideAlignment));

        // Truncated at every length, and one trailing byte.
        for (var length = 0; length < dense.Length; length++)
        {
            var truncated = dense[..length];
            Assert.Throws<ArgumentException>(
                () => NoritoCodec.DecodeWithLayout(WideSchemaName, truncated, NoritoWireLayout.Dense, WideAlignment));
        }

        Assert.Throws<ArgumentException>(
            () => NoritoCodec.DecodeWithLayout(WideSchemaName, [.. dense, 0], NoritoWireLayout.Dense, WideAlignment));

        // Wrong schema and a corrupted payload byte.
        Assert.Throws<ArgumentException>(
            () => NoritoCodec.DecodeWithLayout("norito.vectors.dense.Other", dense, NoritoWireLayout.Dense));
        var corrupted = dense.ToArray();
        corrupted[^1] ^= 0x01;
        Assert.Throws<ArgumentException>(
            () => NoritoCodec.DecodeWithLayout(WideSchemaName, corrupted, NoritoWireLayout.Dense, WideAlignment));
    }

    private static JsonDocument LoadVectors() =>
        JsonDocument.Parse(File.ReadAllText(
            Path.Combine(AppContext.BaseDirectory, "Fixtures", "norito_dense", "vectors.json")));

    private static (byte[] Compact, byte[] Dense) LoadWideFrames()
    {
        using var document = LoadVectors();
        var frames = document.RootElement.GetProperty("frames");
        Assert.Equal(1, frames.GetArrayLength());
        var wide = frames[0];
        Assert.Equal("wide", wide.GetProperty("name").GetString());
        Assert.Equal(WideSchemaName, wide.GetProperty("schema_name").GetString());
        Assert.Equal("Wide { value: u128, tag: [u8; 2] }", wide.GetProperty("type").GetString());
        Assert.Equal(WideValue, wide.GetProperty("value").GetString());
        return (RequireLowerHex(wide, "compact", "wide"), RequireLowerHex(wide, "dense", "wide"));
    }

    private static byte[] RequireLowerHex(JsonElement vector, string field, string name)
    {
        var text = vector.GetProperty(field).GetString()
            ?? throw new InvalidDataException($"{name}.{field} must be a string");
        if (text.Length % 2 != 0 || text.Any(static character => character is not (>= '0' and <= '9' or >= 'a' and <= 'f')))
        {
            throw new InvalidDataException($"{name}.{field} must be lowercase hex bytes");
        }

        return Convert.FromHexString(text);
    }

    private static VectorCase Case<T>(
        string name,
        string type,
        string value,
        Func<byte[], NoritoWireLayout, T> decode,
        Func<T, NoritoWireLayout, byte[]> encode,
        Func<T, string> render) =>
        new(
            name,
            type,
            value,
            (bytes, layout) =>
            {
                var decoded = decode(bytes, layout);
                return (render(decoded), encode(decoded, layout));
            },
            (bytes, from, to) => encode(decode(bytes, from), to));

    private sealed record VectorCase(
        string Name,
        string Type,
        string Value,
        Func<byte[], NoritoWireLayout, (string Value, byte[] Bytes)> RoundTrip,
        Func<byte[], NoritoWireLayout, NoritoWireLayout, byte[]> Transcode);

    private sealed record Choice(uint Tag, string? One, ulong? Far);

    private sealed record Record(uint Id, byte[] Key, byte[][] Digests, string? Label, Choice Choice);

    private sealed record Wide(UInt128 Value, byte[] Tag)
    {
        public bool Equals(Wide? other) =>
            other is not null && Value == other.Value && Tag.AsSpan().SequenceEqual(other.Tag);

        public override int GetHashCode() => Value.GetHashCode();
    }

    // ---- Typed codecs over the SDK's layout-aware reader and writer ----

    private static CanonicalNoritoReader Reader(ReadOnlySpan<byte> bytes, string context, NoritoWireLayout layout) =>
        new(bytes, context, nameof(bytes), layout);

    private static byte[] DecodeByteVec(byte[] bytes, NoritoWireLayout layout) =>
        TransactionWireCodec.DecodeBytesVec(bytes, "Vec<u8>", nameof(bytes), layout);

    private static byte[] EncodeByteVec(byte[] value, NoritoWireLayout layout) =>
        TransactionWireCodec.EncodeBytesVec(layout, value);

    private static byte[] DecodeByteArray(ReadOnlySpan<byte> bytes, int length, NoritoWireLayout layout)
    {
        var reader = Reader(bytes, $"[u8; {length}]", layout);
        var value = reader.ReadByteElements(length, "bytes");
        reader.RequireEnd();
        return value;
    }

    private static byte[] EncodeByteArray(byte[] value, NoritoWireLayout layout)
    {
        var writer = new CanonicalNoritoWriter(layout);
        writer.WriteByteElements(value);
        return writer.ToArray();
    }

    private static byte[]? DecodeOptionByteArray(byte[] bytes, NoritoWireLayout layout)
    {
        var reader = Reader(bytes, "Option<[u8; 4]>", layout);
        byte[]? value = null;
        switch (reader.ReadByte("option"))
        {
            case 0:
                break;
            case 1:
                value = DecodeByteArray(reader.ReadField("value"), 4, layout);
                break;
            default:
                throw new ArgumentException("invalid Option tag", nameof(bytes));
        }

        reader.RequireEnd();
        return value;
    }

    private static byte[] EncodeOptionByteArray(byte[]? value, NoritoWireLayout layout) =>
        value is null
            ? TransactionWireCodec.EncodeNone(layout)
            : TransactionWireCodec.EncodeSome(layout, EncodeByteArray(value, layout));

    private static byte[][] DecodeByteArraySequence(byte[] bytes, NoritoWireLayout layout) =>
        DecodeByteArraySequence(bytes, 2, layout);

    private static byte[][] DecodeByteArraySequence(ReadOnlySpan<byte> bytes, int length, NoritoWireLayout layout)
    {
        var reader = Reader(bytes, $"Vec<[u8; {length}]>", layout);
        var count = reader.ReadBoundedSequenceLength("count", minimumElementBytes: 1);
        var values = new byte[count][];
        for (var index = 0; index < count; index++)
        {
            values[index] = DecodeByteArray(reader.ReadField($"[{index}]"), length, layout);
        }

        reader.RequireEnd();
        return values;
    }

    private static byte[] EncodeByteArraySequence(byte[][] values, NoritoWireLayout layout)
    {
        var writer = new CanonicalNoritoWriter(layout);
        writer.WriteSequenceLength((ulong)values.Length);
        foreach (var value in values)
        {
            writer.WriteField(EncodeByteArray(value, layout));
        }

        return writer.ToArray();
    }

    private static string[] DecodeStringSequence(byte[] bytes, NoritoWireLayout layout)
    {
        var reader = Reader(bytes, "Vec<String>", layout);
        var count = reader.ReadBoundedSequenceLength("count", minimumElementBytes: 1);
        var values = new string[count];
        for (var index = 0; index < count; index++)
        {
            values[index] = TransactionWireCodec.DecodeString(
                reader.ReadField($"[{index}]"), "Vec<String> element", nameof(bytes), layout);
        }

        reader.RequireEnd();
        return values;
    }

    private static byte[] EncodeStringSequence(string[] values, NoritoWireLayout layout)
    {
        var writer = new CanonicalNoritoWriter(layout);
        writer.WriteSequenceLength((ulong)values.Length);
        foreach (var value in values)
        {
            writer.WriteField(TransactionWireCodec.EncodeString(layout, value));
        }

        return writer.ToArray();
    }

    private static KeyValuePair<string, ushort>[] DecodeStringUInt16Map(byte[] bytes, NoritoWireLayout layout)
    {
        var reader = Reader(bytes, "BTreeMap<String, u16>", layout);
        var count = reader.ReadBoundedSequenceLength("count", minimumElementBytes: 2);
        var entries = new KeyValuePair<string, ushort>[count];
        for (var index = 0; index < count; index++)
        {
            var key = TransactionWireCodec.DecodeString(
                reader.ReadField($"[{index}].key"), "map key", nameof(bytes), layout);
            var value = reader.ReadField($"[{index}].value");
            if (value.Length != sizeof(ushort))
            {
                throw new ArgumentException("map value must be two bytes", nameof(bytes));
            }

            entries[index] = new(key, BinaryPrimitives.ReadUInt16LittleEndian(value));
        }

        reader.RequireEnd();
        return entries;
    }

    private static byte[] EncodeStringUInt16Map(KeyValuePair<string, ushort>[] entries, NoritoWireLayout layout)
    {
        var writer = new CanonicalNoritoWriter(layout);
        writer.WriteSequenceLength((ulong)entries.Length);
        foreach (var (key, value) in entries)
        {
            writer.WriteField(TransactionWireCodec.EncodeString(layout, key));
            writer.WriteField(TransactionWireCodec.EncodeUInt16(layout, value));
        }

        return writer.ToArray();
    }

    private static Choice DecodeChoice(byte[] bytes, NoritoWireLayout layout) =>
        DecodeChoice((ReadOnlySpan<byte>)bytes, layout);

    private static Choice DecodeChoice(ReadOnlySpan<byte> bytes, NoritoWireLayout layout)
    {
        var reader = Reader(bytes, "Choice", layout);
        var tag = reader.ReadEnumTag("discriminant");
        Choice value;
        switch (tag)
        {
            case 0:
                value = new Choice(tag, null, null);
                break;
            case 1:
                value = new Choice(
                    tag,
                    TransactionWireCodec.DecodeString(reader.ReadField("0"), "Choice::One", nameof(bytes), layout),
                    null);
                break;
            case 300:
                var far = reader.ReadField("value");
                if (far.Length != sizeof(ulong))
                {
                    throw new ArgumentException("Choice::Far value must be eight bytes", nameof(bytes));
                }

                value = new Choice(tag, null, BinaryPrimitives.ReadUInt64LittleEndian(far));
                break;
            default:
                throw new ArgumentException($"unknown Choice discriminant {tag}", nameof(bytes));
        }

        reader.RequireEnd();
        return value;
    }

    private static byte[] EncodeChoice(Choice value, NoritoWireLayout layout)
    {
        var writer = new CanonicalNoritoWriter(layout);
        writer.WriteEnumTag(value.Tag);
        if (value.One is { } one)
        {
            writer.WriteField(TransactionWireCodec.EncodeString(layout, one));
        }

        if (value.Far is { } far)
        {
            writer.WriteField(TransactionWireCodec.EncodeUInt64(layout, far));
        }

        return writer.ToArray();
    }

    private static (bool Ok, byte Value) DecodeByteResult(byte[] bytes, NoritoWireLayout layout)
    {
        var reader = Reader(bytes, "Result<u8, u8>", layout);
        var tag = reader.ReadByte("result");
        if (tag > 1)
        {
            throw new ArgumentException("invalid Result tag", nameof(bytes));
        }

        var value = reader.ReadField("value");
        reader.RequireEnd();
        if (value.Length != 1)
        {
            throw new ArgumentException("Result value must be one byte", nameof(bytes));
        }

        return (tag == 0, value[0]);
    }

    private static byte[] EncodeByteResult((bool Ok, byte Value) value, NoritoWireLayout layout)
    {
        var writer = new CanonicalNoritoWriter(layout);
        writer.WriteByte(value.Ok ? (byte)0 : (byte)1);
        writer.WriteField([value.Value]);
        return writer.ToArray();
    }

    private static Record DecodeRecord(byte[] bytes, NoritoWireLayout layout)
    {
        var reader = Reader(bytes, "Record", layout);
        var id = reader.ReadField("id");
        // A record field of `[u8; N]` is the raw bytes behind its field prefix in both layouts.
        var key = reader.ReadField("key");
        var digests = reader.ReadField("digests");
        var labelField = reader.ReadField("label");
        var choice = reader.ReadField("choice");
        reader.RequireEnd();
        if (id.Length != sizeof(uint) || key.Length != 4)
        {
            throw new ArgumentException("Record fixed-width field has the wrong length", nameof(bytes));
        }

        var labelReader = Reader(labelField, "Record.label", layout);
        string? label = null;
        switch (labelReader.ReadByte("option"))
        {
            case 0:
                break;
            case 1:
                label = TransactionWireCodec.DecodeString(
                    labelReader.ReadField("value"), "Record.label", nameof(bytes), layout);
                break;
            default:
                throw new ArgumentException("invalid Option tag", nameof(bytes));
        }

        labelReader.RequireEnd();
        return new Record(
            BinaryPrimitives.ReadUInt32LittleEndian(id),
            key.ToArray(),
            DecodeByteArraySequence(digests, 4, layout),
            label,
            DecodeChoice(choice, layout));
    }

    private static byte[] EncodeRecord(Record value, NoritoWireLayout layout)
    {
        var writer = new CanonicalNoritoWriter(layout);
        writer.WriteField(TransactionWireCodec.EncodeUInt32(layout, value.Id));
        writer.WriteField(value.Key);
        writer.WriteField(EncodeByteArraySequence(value.Digests, layout));
        writer.WriteField(value.Label is { } label
            ? TransactionWireCodec.EncodeSome(layout, TransactionWireCodec.EncodeString(layout, label))
            : TransactionWireCodec.EncodeNone(layout));
        writer.WriteField(EncodeChoice(value.Choice, layout));
        return writer.ToArray();
    }

    private static Wide DecodeWide(byte[] bytes, NoritoWireLayout layout)
    {
        var reader = Reader(bytes, "Wide", layout);
        var value = reader.ReadField("value");
        var tag = reader.ReadField("tag");
        reader.RequireEnd();
        if (value.Length != 16 || tag.Length != 2)
        {
            throw new ArgumentException("Wide fixed-width field has the wrong length", nameof(bytes));
        }

        return new Wide(BinaryPrimitives.ReadUInt128LittleEndian(value), tag.ToArray());
    }

    private static byte[] EncodeWide(Wide value, NoritoWireLayout layout)
    {
        Span<byte> word = stackalloc byte[16];
        BinaryPrimitives.WriteUInt128LittleEndian(word, value.Value);
        var writer = new CanonicalNoritoWriter(layout);
        writer.WriteField(word);
        writer.WriteField(value.Tag);
        return writer.ToArray();
    }

    // ---- Renderings in the fixture's `value` notation ----

    private static string RenderBytes(byte[] value) => $"[{string.Join(", ", value)}]";

    private static string RenderOptionByteArray(byte[]? value) => value is null ? "None" : $"Some({RenderBytes(value)})";

    private static string RenderByteArrays(byte[][] values) => $"[{string.Join(", ", values.Select(RenderBytes))}]";

    private static string RenderStrings(string[] values) =>
        $"[{string.Join(", ", values.Select(static value => $"\"{value}\""))}]";

    private static string RenderStringUInt16Map(KeyValuePair<string, ushort>[] entries) =>
        $"{{{string.Join(", ", entries.Select(static entry => $"\"{entry.Key}\": {entry.Value}"))}}}";

    private static string RenderChoice(Choice value) => value switch
    {
        { Tag: 0 } => "Unit",
        { Tag: 1, One: { } one } => $"One(\"{one}\")",
        { Tag: 300, Far: { } far } => $"Far {{ value: {far} }}",
        _ => throw new InvalidOperationException("unrenderable Choice"),
    };

    private static string RenderByteResult((bool Ok, byte Value) value) =>
        value.Ok ? $"Ok({value.Value})" : $"Err({value.Value})";

    private static string RenderRecord(Record value) =>
        $"{{ id: {value.Id}, key: {RenderBytes(value.Key)}, digests: [{string.Join(", ", value.Digests.Select(RenderRepeated))}], "
        + $"label: {(value.Label is { } label ? $"Some(\"{label}\")" : "None")}, choice: {RenderChoice(value.Choice)} }}";

    private static string RenderRepeated(byte[] value) =>
        value.Distinct().Count() == 1 ? $"[0x{value[0]:x2}; {value.Length}]" : RenderBytes(value);

    private static string RenderWide(Wide value) =>
        $"{{ value: 0x{value.Value:x32}, tag: [{string.Join(", ", value.Tag.Select(static item => $"0x{item:x2}"))}] }}";
}
