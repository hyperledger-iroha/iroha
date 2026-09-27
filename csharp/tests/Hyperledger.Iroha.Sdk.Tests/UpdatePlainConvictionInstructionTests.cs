using System.Text.Json;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class UpdatePlainConvictionInstructionTests
{
    private const string AccountId =
        "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53";
    private const string OtherAccountId =
        "sorauﾛ1P2PMｲbjRｦ2jrLFﾁｽｸFjjBヱYﾜｴ3ﾋNRjﾌｸﾆｺNXcfﾒXSKXAW";
    private const string NetworkIdLiteral =
        "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0";

    [Fact]
    public void RegisteredFrameHasExactlyTheFourChoiceFreeFields()
    {
        var instruction = TransactionInstruction.UpdatePlainConviction(
            "referendum_1", AccountId, "125.5", ulong.MaxValue);
        var context = new TransactionEncodingContext(AccountId);

        Assert.Equal(UpdatePlainConvictionInstruction.NativeWireId, instruction.WireId);
        Assert.Equal(UpdatePlainConvictionInstruction.NativeTypeName, instruction.TypeName);
        Assert.Equal("125.5", instruction.Amount);
        Assert.Equal(ulong.MaxValue, instruction.DurationBlocks);

        var frame = instruction.EncodeFramedPayload(context);
        var (payload, flags) = NoritoCodec.Decode(
            UpdatePlainConvictionInstruction.NativeTypeName, frame);
        Assert.Equal(NoritoCodec.CanonicalLayoutFlags, flags);
        var fields = new CanonicalNoritoReader(payload, "UpdatePlainConviction", nameof(frame));
        Assert.Equal(context.EncodeString("referendum_1"), fields.ReadField("referendum_id").ToArray());
        Assert.Equal(context.EncodeAccountId(AccountId), fields.ReadField("owner").ToArray());
        Assert.Equal(context.EncodeQuantity(Hyperledger.Iroha.Numeric.NumericV1.QuantityValue.ParseCanonical("125.5")),
            fields.ReadField("amount").ToArray());
        Assert.Equal(context.EncodeUInt64(ulong.MaxValue), fields.ReadField("duration_blocks").ToArray());
        fields.RequireEnd();

        var boxed = instruction.EncodeInstructionBox(AccountId);
        var (boxPayload, boxFlags) = NoritoCodec.DecodeWithSchemaHash(
            Convert.FromHexString("862a7d77075d4d23ff6c1261db027811"), boxed);
        Assert.Equal(NoritoCodec.CanonicalLayoutFlags, boxFlags);
        var box = new CanonicalNoritoReader(boxPayload, "InstructionBox", nameof(boxed));
        Assert.Equal(context.EncodeString(UpdatePlainConvictionInstruction.NativeWireId),
            box.ReadField("wire_id").ToArray());
        var vec = box.ReadField("payload");
        Assert.Equal((ulong)frame.Length, System.Buffers.Binary.BinaryPrimitives.ReadUInt64LittleEndian(vec));
        Assert.Equal(frame, vec[sizeof(ulong)..].ToArray());
        box.RequireEnd();
    }

    [Fact]
    public void DirectConvictionFrameMatchesRustOwnedGoldenAndRoundTrips()
    {
        using var document = JsonDocument.Parse(File.ReadAllBytes(RepositoryGolden()));
        var fixture = document.RootElement;
        Assert.Equal(1, fixture.GetProperty("version").GetInt32());
        var inputs = fixture.GetProperty("inputs");
        Assert.Equal(4, inputs.EnumerateObject().Count());
        var fields = new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["referendum_id"] = inputs.GetProperty("referendum_id").GetString()!,
            ["owner"] = inputs.GetProperty("owner").GetString()!,
            ["amount"] = inputs.GetProperty("amount").GetString()!,
            ["duration_blocks"] = inputs.GetProperty("duration_blocks").GetUInt64()
                .ToString(System.Globalization.CultureInfo.InvariantCulture),
        };
        var instruction = UpdatePlainConvictionInstruction.FromCanonicalFields(fields);
        Assert.Equal(fields["referendum_id"], instruction.ReferendumId);
        Assert.Equal(fields["owner"], instruction.OwnerAccountId);
        Assert.Equal(fields["amount"], instruction.Amount);
        Assert.Equal(ulong.Parse(fields["duration_blocks"], System.Globalization.CultureInfo.InvariantCulture),
            instruction.DurationBlocks);
        Assert.Equal(UpdatePlainConvictionInstruction.NativeWireId,
            fixture.GetProperty("wire_id").GetString());
        var schema = fixture.GetProperty("concrete_schema_name").GetString()!;
        Assert.Equal(UpdatePlainConvictionInstruction.NativeTypeName, schema);
        Assert.Equal(Convert.ToHexString(NoritoCodec.SchemaHash(schema)).ToLowerInvariant(),
            fixture.GetProperty("concrete_schema_hash").GetString());

        var context = new TransactionEncodingContext(fields["owner"]);
        var frame = instruction.EncodeFramedPayload(context);
        Assert.Equal(Convert.FromHexString(fixture.GetProperty("concrete_frame_hex").GetString()!), frame);
        Assert.Equal(Convert.FromBase64String(fixture.GetProperty("framed_instruction_base64").GetString()!),
            frame);
        Assert.Equal(fixture.GetProperty("framed_instruction_len").GetInt32(), frame.Length);
        var (payload, flags) = NoritoCodec.Decode(schema, frame);
        Assert.Equal(fixture.GetProperty("header_flags").GetByte(), flags);
        Assert.Equal(Convert.FromHexString(fixture.GetProperty("bare_payload_hex").GetString()!), payload);
        Assert.Equal(frame, NoritoCodec.Encode(schema, payload, flags));
        var reader = new CanonicalNoritoReader(payload, "UpdatePlainConviction", nameof(frame));
        Assert.Equal(context.EncodeString(fields["referendum_id"]), reader.ReadField("referendum_id").ToArray());
        Assert.Equal(context.EncodeAccountId(fields["owner"]), reader.ReadField("owner").ToArray());
        Assert.Equal(context.EncodeQuantity(Hyperledger.Iroha.Numeric.NumericV1.QuantityValue.ParseCanonical(
            fields["amount"])), reader.ReadField("amount").ToArray());
        Assert.Equal(context.EncodeUInt64(instruction.DurationBlocks),
            reader.ReadField("duration_blocks").ToArray());
        reader.RequireEnd();

        var standalone = instruction.EncodeInstructionBox(fields["owner"]);
        Assert.Equal(Convert.FromHexString(
            fixture.GetProperty("standalone_instruction_box_frame_hex").GetString()!), standalone);
        var boxSchema = NoritoCodec.SchemaHash("(alloc::string::String, alloc::vec::Vec<u8>)");
        Assert.Equal(boxSchema, standalone.AsSpan(6, boxSchema.Length).ToArray());
        var (pair, boxFlags) = NoritoCodec.DecodeWithSchemaHash(boxSchema, standalone);
        Assert.Equal(fixture.GetProperty("header_flags").GetByte(), boxFlags);
        Assert.Equal(Convert.FromHexString(fixture.GetProperty("instruction_box_pair_hex").GetString()!),
            pair);
        Assert.Equal(standalone, NoritoCodec.EncodeWithSchemaHash(boxSchema, pair, boxFlags));
        var box = new CanonicalNoritoReader(pair, "InstructionBox", nameof(standalone));
        Assert.Equal(context.EncodeString(UpdatePlainConvictionInstruction.NativeWireId),
            box.ReadField("wire_id").ToArray());
        var embedded = box.ReadField("payload");
        Assert.Equal((ulong)frame.Length,
            System.Buffers.Binary.BinaryPrimitives.ReadUInt64LittleEndian(embedded));
        Assert.Equal(frame, embedded[sizeof(ulong)..].ToArray());
        box.RequireEnd();
    }

    [Fact]
    public void StrictCanonicalFieldsRejectChoiceAliasesAndAlternateNumbers()
    {
        var fields = new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["referendum_id"] = "referendum_1",
            ["owner"] = AccountId,
            ["amount"] = "20",
            ["duration_blocks"] = ulong.MaxValue.ToString(System.Globalization.CultureInfo.InvariantCulture),
        };
        var instruction = UpdatePlainConvictionInstruction.FromCanonicalFields(fields);
        Assert.Equal(ulong.MaxValue, instruction.DurationBlocks);

        foreach (var extra in new[] { "direction", "choice", "action" })
        {
            var forged = new Dictionary<string, string>(fields) { [extra] = "1" };
            Assert.Throws<ArgumentException>(() =>
                UpdatePlainConvictionInstruction.FromCanonicalFields(forged));
        }
        var missing = new Dictionary<string, string>(fields);
        missing.Remove("owner");
        Assert.Throws<ArgumentException>(() =>
            UpdatePlainConvictionInstruction.FromCanonicalFields(missing));

        foreach (var duration in new[] { "01", "+1", "-1", "18446744073709551616", " 1", "1 " })
        {
            var malformed = new Dictionary<string, string>(fields) { ["duration_blocks"] = duration };
            Assert.Throws<ArgumentException>(() =>
                UpdatePlainConvictionInstruction.FromCanonicalFields(malformed));
        }
    }

    [Fact]
    public void ConstructorAndAuthorityBoundaryRejectMalformedOrStaleOwnerInputs()
    {
        foreach (var selector in new[] { "", ".hidden", "bad/path", "bad%20", "x y", new string('a', 129) })
        {
            Assert.Throws<ArgumentException>(() =>
                TransactionInstruction.UpdatePlainConviction(selector, AccountId, "20", 10));
        }
        foreach (var amount in new[] { "0", "-1", "01", "1.0", "+1" })
        {
            Assert.ThrowsAny<ArgumentException>(() =>
                TransactionInstruction.UpdatePlainConviction("referendum_1", AccountId, amount, 10));
        }
        Assert.Throws<ArgumentException>(() =>
            TransactionInstruction.UpdatePlainConviction("referendum_1", AccountId + "@domain", "20", 10));
        var instruction = TransactionInstruction.UpdatePlainConviction(
            "referendum_1", AccountId, "20", 10);
        Assert.Throws<ArgumentException>(() => instruction.EncodeInstructionBox(OtherAccountId));
    }

    [Fact]
    public void BuilderUsesTheSameRegisteredDirectInstruction()
    {
        var builder = new TransactionBuilder(
            NetworkId.Parse(NetworkIdLiteral),
            AccountId,
            FeePaymentIntent.Authority(Array.Empty<FeeChargeLimit>()))
            .UpdatePlainConviction("referendum_1", "20", 10);

        var instruction = Assert.IsType<UpdatePlainConvictionInstruction>(
            Assert.Single(builder.Instructions));
        Assert.Equal(AccountId, instruction.OwnerAccountId);
        Assert.Equal(UpdatePlainConvictionInstruction.NativeWireId, instruction.WireId);
        Assert.NotEmpty(builder.BuildUnsignedPayload().Executable.ToJsonString());
    }
    private static string RepositoryGolden()
    {
        const string relative = "fixtures/governance/plain_v1/update_plain_conviction_instruction_v1.json";
        for (var directory = new DirectoryInfo(AppContext.BaseDirectory);
             directory is not null;
             directory = directory.Parent)
        {
            if (File.Exists(Path.Combine(directory.FullName, "Cargo.toml")))
            {
                return Path.Combine(directory.FullName, relative);
            }
        }
        throw new InvalidOperationException("Rust-owned conviction golden requires the repository checkout.");
    }
}
