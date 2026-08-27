using System.Buffers.Binary;
using System.Numerics;
using System.Text;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Numeric;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class CancelAssetLockInstructionTests
{
    private const string NetworkIdLiteral = "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0";
    private const string AccountId =
        "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53";
    private const string MerchantEscrowId =
        "hash:996264C84790C64086AAB0EF693A1D33EC18FC0B1C1229774C461A00939A6687#F2BD";
    private const string AppealFinanceCanonicalNoritoHex =
        "4e5254300000b5c8a665a7de80e2eef75ccb287078fa002d00000000000000"
        + "d5f0a9bf0af707a1022073ccd4e0dd69ad434db75056b600aa4f74c8fc5556b11bdc"
        + "799dfdb7ea29851f0b0501000000140400000000";
    private const string AppealFinanceNestedEscrowIdNoritoHex =
        "4e5254300000b5c8a665a7de80e2eef75ccb287078fa002e00000000000000"
        + "0e55fb7ed463b87302212073ccd4e0dd69ad434db75056b600aa4f74c8fc5556b11b"
        + "dc799dfdb7ea29851f0b0501000000140400000000";
    private const string JavascriptInstructionBoxV1 =
        "TlJUMAAAhip9dwddTSP/bBJh2wJ4EQCNAAAAAAAAAHyPhQgH6/N3Ai4taXJvaGEuaW5zdHJ1Y3Rpb24udjE6OmVzY3Jvdzo6Q2FuY2VsQXNzZXRMb2NrXVUAAAAAAAAATlJUMAAAtcimZafegOLu91zLKHB4+gAtAAAAAAAAAJ+oc+u4RieVAiCZYmTIR5DGQIaqsO9pOh0z7Bj8CxwSKXdMRhoAk5pmhwsFAQAAAH0EAgAAAA==";

    [Fact]
    public void BuilderEmitsTheExactTwoFieldV1Shape()
    {
        var instruction = TransactionInstruction.CancelAssetLock(
            "merchant-lock-001",
            "1500");

        Assert.Equal(CancelAssetLockInstruction.NativeWireId, instruction.WireId);
        Assert.Equal(CancelAssetLockInstruction.NativeTypeName, instruction.TypeName);
        Assert.Equal(MerchantEscrowId, instruction.EscrowId);
        Assert.Equal("1500", instruction.ExpectedRemainingAmount);
        Assert.Equal(
            """
            {
              "CancelAssetLock": {
                "escrow_id": "hash:996264C84790C64086AAB0EF693A1D33EC18FC0B1C1229774C461A00939A6687#F2BD",
                "expected_remaining_amount": "1500"
              }
            }

            """,
            Encoding.UTF8.GetString(instruction.EncodeInstructionJson()));

        var payload = new FixedFieldReader(
            instruction.EncodePayload(new TransactionEncodingContext(AccountId)));
        Assert.Equal(
            Convert.FromHexString(
                "996264C84790C64086AAB0EF693A1D33EC18FC0B1C1229774C461A00939A6687"),
            payload.ReadField().ToArray());
        Assert.Equal(
            NumericV1.QuantityValue.ParseCanonical("1500"),
            DecodeQuantity(payload.ReadField()));
        payload.RequireEnd();

        var parityInstruction = TransactionInstruction.CancelAssetLock(
            "merchant-lock-001",
            "1.25");
        Assert.Equal(
            JavascriptInstructionBoxV1,
            parityInstruction.EncodeInstructionBoxBase64(AccountId));
    }

    [Fact]
    public void NativeAndJsonCodecsRoundTripCanonically()
    {
        var expected = CancelAssetLockInstruction.FromEscrowId(
            MerchantEscrowId,
            "1.25");

        var norito = expected.EncodeNorito();
        var fromNorito = CancelAssetLockInstruction.DecodeNorito(norito);
        Assert.Equal(expected.EscrowId, fromNorito.EscrowId);
        Assert.Equal(
            expected.ExpectedRemainingAmount,
            fromNorito.ExpectedRemainingAmount);
        Assert.Equal(norito, fromNorito.EncodeNorito());

        var json = expected.EncodePayloadJson();
        var fromJson = CancelAssetLockInstruction.DecodePayloadJson(json);
        Assert.Equal(expected.EscrowId, fromJson.EscrowId);
        Assert.Equal(
            expected.ExpectedRemainingAmount,
            fromJson.ExpectedRemainingAmount);
        Assert.Equal(json, fromJson.EncodePayloadJson());

        var fromInstructionJson =
            CancelAssetLockInstruction.DecodeInstructionJson(
                expected.EncodeInstructionJson());
        Assert.Equal(expected.EscrowId, fromInstructionJson.EscrowId);
        Assert.Equal(
            expected.ExpectedRemainingAmount,
            fromInstructionJson.ExpectedRemainingAmount);
    }

    [Fact]
    public void BuilderRejectsLegacyZeroAndNoncanonicalInputs()
    {
        Assert.DoesNotContain(
            typeof(CancelAssetLockInstruction).GetConstructors(),
            constructor => constructor.GetParameters().Length == 1);
        Assert.Throws<ArgumentException>(() =>
            TransactionInstruction.CancelAssetLock("", "1"));
        Assert.Throws<ArgumentException>(() =>
            TransactionInstruction.CancelAssetLock(" merchant-lock-001", "1"));
        Assert.Throws<ArgumentException>(() =>
            TransactionInstruction.CancelAssetLock(
                "\uFEFFmerchant-lock-001",
                "1"));
        foreach (var malformedUtf16 in new[]
        {
            "\uD800",
            "\uDC00",
            "lock\uD800id",
        })
        {
            Assert.Throws<ArgumentException>(() =>
                TransactionInstruction.CancelAssetLock(
                    malformedUtf16,
                    "1"));
        }

        Assert.Equal(
            4_096,
            CancelAssetLockInstruction.MaximumLockIdUtf8BytesV1);
        var exactUtf8Boundary = string.Concat(
            Enumerable.Repeat("\U0001F512", 1_024));
        Assert.Equal(
            CancelAssetLockInstruction.MaximumLockIdUtf8BytesV1,
            Encoding.UTF8.GetByteCount(exactUtf8Boundary));
        Assert.NotNull(
            TransactionInstruction.CancelAssetLock(
                exactUtf8Boundary,
                "1"));
        Assert.Throws<ArgumentException>(() =>
            TransactionInstruction.CancelAssetLock(
                exactUtf8Boundary + "a",
                "1"));

        foreach (var quantity in new[]
        {
            "0",
            "-1",
            "01",
            "1.0",
            "+1",
        })
        {
            Assert.ThrowsAny<ArgumentException>(() =>
                TransactionInstruction.CancelAssetLock(
                    "merchant-lock-001",
                    quantity));
        }
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            TransactionInstruction.CancelAssetLock(
                "merchant-lock-001",
                NumericV1.QuantityValue.ParseCanonical("0")));
    }

    [Fact]
    public void DecoderRejectsLegacyNoncanonicalAndTrailingNorito()
    {
        Assert.Throws<ArgumentException>(() =>
            CancelAssetLockInstruction.DecodeNorito(
                Encoding.ASCII.GetBytes(AppealFinanceCanonicalNoritoHex)));

        var canonical = TransactionInstruction.CancelAssetLock(
            "merchant-lock-001",
            "20");
        var canonicalPayload = canonical.EncodePayload(
            new TransactionEncodingContext(AccountId));
        var payload = new FixedFieldReader(canonicalPayload);
        var escrowField = payload.ReadField().ToArray();
        _ = payload.ReadField();
        payload.RequireEnd();

        Assert.Throws<ArgumentException>(() =>
            CancelAssetLockInstruction.DecodeNorito(
                NoritoCodec.Encode(
                    CancelAssetLockInstruction.NativeTypeName,
                    canonicalPayload)));

        Assert.Throws<ArgumentException>(() =>
            CancelAssetLockInstruction.DecodeNorito(
                NoritoCodec.Encode(
                    CancelAssetLockInstruction.NativeTypeName,
                    EncodeCompactFields(escrowField),
                    flags: 0x02)));

        var noncanonicalQuantity = EncodeCompactQuantity(
            mantissa: 20,
            scale: 1);
        Assert.Throws<ArgumentException>(() =>
            CancelAssetLockInstruction.DecodeNorito(
                NoritoCodec.Encode(
                    CancelAssetLockInstruction.NativeTypeName,
                    EncodeCompactFields(
                        escrowField,
                        noncanonicalQuantity),
                    flags: 0x02)));

        Assert.ThrowsAny<ArgumentException>(() =>
            CancelAssetLockInstruction.DecodeNorito(
                NoritoCodec.Encode(
                    CancelAssetLockInstruction.NativeTypeName,
                    EncodeCompactFields(
                        escrowField,
                        EncodeCompactQuantity(mantissa: 0, scale: 0)),
                    flags: 0x02)));

        var trailing = canonical.EncodeNorito().Concat(new byte[] { 0 }).ToArray();
        Assert.Throws<ArgumentException>(() =>
            CancelAssetLockInstruction.DecodeNorito(trailing));

        var retiredNestedEscrowId = NoritoCodec.Encode(
            CancelAssetLockInstruction.NativeTypeName,
            EncodeCompactFields(
                EncodeCompactFields(escrowField),
                EncodeCompactQuantity(mantissa: 20, scale: 0)),
            flags: 0x02);
        Assert.Equal(86, retiredNestedEscrowId.Length);
        Assert.Equal(new byte[] { 0x21, 0x20 }, retiredNestedEscrowId[40..42]);
        Assert.Throws<ArgumentException>(() =>
            CancelAssetLockInstruction.DecodeNorito(retiredNestedEscrowId));
    }

    [Fact]
    public void JsonDecoderRejectsLegacyUnknownDuplicateAndInvalidQuantities()
    {
        var prefix =
            $$"""{"escrow_id":"{{MerchantEscrowId}}","expected_remaining_amount":""";

        Assert.Throws<ArgumentException>(() =>
            CancelAssetLockInstruction.DecodePayloadJson(
                new byte[]
                {
                    (byte)'{',
                    (byte)'"',
                    0x80,
                    (byte)'"',
                    (byte)':',
                    (byte)'1',
                    (byte)'}',
                }));
        Assert.Throws<ArgumentException>(() =>
            CancelAssetLockInstruction.DecodePayloadJson(
                Encoding.UTF8.GetBytes(
                    $$"""{"escrow_id":"{{MerchantEscrowId}}"}""")));
        Assert.Throws<ArgumentException>(() =>
            CancelAssetLockInstruction.DecodePayloadJson(
                Encoding.UTF8.GetBytes(prefix + "\"20\",\"legacy\":true}")));
        Assert.Throws<ArgumentException>(() =>
            CancelAssetLockInstruction.DecodePayloadJson(
                Encoding.UTF8.GetBytes(
                    $$"""{"escrow_id":"{{MerchantEscrowId}}","escrow_id":"{{MerchantEscrowId}}","expected_remaining_amount":"20"}""")));
        Assert.Throws<ArgumentException>(() =>
            CancelAssetLockInstruction.DecodePayloadJson(
                Encoding.UTF8.GetBytes(
                    $$"""{"escrow_id":["{{MerchantEscrowId}}"],"expected_remaining_amount":"20"}""")));
        Assert.Throws<ArgumentException>(() =>
            CancelAssetLockInstruction.DecodeInstructionJson(
                Encoding.UTF8.GetBytes(
                    "{\"CancelAssetLock\":{\"escrow_id\":\""
                    + MerchantEscrowId
                    + "\",\"expected_remaining_amount\":\"20\"},\"CancelAssetLock\":{\"escrow_id\":\""
                    + MerchantEscrowId
                    + "\",\"expected_remaining_amount\":\"20\"}}")));
        Assert.Throws<ArgumentException>(() =>
            CancelAssetLockInstruction.DecodeInstructionJson(
                Encoding.UTF8.GetBytes(
                    "{\"CancelAssetEscrow\":{\"escrow_id\":\""
                    + MerchantEscrowId
                    + "\",\"expected_remaining_amount\":\"20\"}}")));
        foreach (var quantity in new[] { "0", "-1", "01", "20.0" })
        {
            Assert.ThrowsAny<ArgumentException>(() =>
                CancelAssetLockInstruction.DecodePayloadJson(
                    Encoding.UTF8.GetBytes(prefix + $"\"{quantity}\"}}")));
        }
    }

    [Fact]
    public void FluentBuilderRequiresAndRetainsTheCasPrecondition()
    {
        var builder = new TransactionBuilder(
                NetworkId.Parse(NetworkIdLiteral),
                AccountId,
                FeePaymentIntent.Authority(Array.Empty<FeeChargeLimit>()))
            .CancelAssetLock("merchant-lock-001", "20");

        var instruction = Assert.IsType<CancelAssetLockInstruction>(
            Assert.Single(builder.Instructions));
        Assert.Equal(MerchantEscrowId, instruction.EscrowId);
        Assert.Equal("20", instruction.ExpectedRemainingAmount);
    }

    [Fact]
    public void SharedFixturesEnforceTheV1HardCut()
    {
        var root = Path.Combine(
            AppContext.BaseDirectory,
            "Fixtures",
            "sorafs_manifest",
            "appeal_finance");
        byte[] Read(string relative)
        {
            var path = Path.Combine(root, relative);
            Assert.True(
                File.Exists(path),
                $"Missing generated CancelAssetLock fixture `{relative}`.");
            return File.ReadAllBytes(path);
        }

        var fromJson = CancelAssetLockInstruction.DecodePayloadJson(
            Read("cancel_asset_lock_v1.json"));
        var fromNorito = CancelAssetLockInstruction.DecodeNorito(
            Read("cancel_asset_lock_v1.to"));
        Assert.Equal(fromJson.EscrowId, fromNorito.EscrowId);
        Assert.Equal(
            fromJson.ExpectedRemainingAmount,
            fromNorito.ExpectedRemainingAmount);
        Assert.Equal(
            Read("cancel_asset_lock_v1.json"),
            fromJson.EncodePayloadJson());
        Assert.Equal(
            Read("cancel_asset_lock_v1.to"),
            fromNorito.EncodeNorito());
        var canonicalNorito = Read("cancel_asset_lock_v1.to");
        Assert.Equal(85, canonicalNorito.Length);
        Assert.Equal(
            Convert.FromHexString(AppealFinanceCanonicalNoritoHex),
            canonicalNorito);
        var nestedEscrowIdNorito =
            Read("negative/cancel_asset_lock_nested_escrow_id_v1.to");
        Assert.Equal(86, nestedEscrowIdNorito.Length);
        Assert.Equal(
            Convert.FromHexString(AppealFinanceNestedEscrowIdNoritoHex),
            nestedEscrowIdNorito);
        Assert.Equal(new byte[] { 0x21, 0x20 }, nestedEscrowIdNorito[40..42]);

        foreach (var relative in new[]
        {
            "negative/cancel_asset_lock_legacy_missing_expected_v1.json",
            "negative/cancel_asset_lock_noncanonical_quantity_v1.json",
            "negative/cancel_asset_lock_zero_expected_v1.json",
        })
        {
            Assert.ThrowsAny<ArgumentException>(() =>
                CancelAssetLockInstruction.DecodePayloadJson(Read(relative)));
        }
        foreach (var relative in new[]
        {
            "negative/cancel_asset_lock_legacy_missing_expected_v1.to",
            "negative/cancel_asset_lock_nested_escrow_id_v1.to",
            "negative/cancel_asset_lock_zero_expected_v1.to",
        })
        {
            Assert.ThrowsAny<ArgumentException>(() =>
                CancelAssetLockInstruction.DecodeNorito(Read(relative)));
        }
    }

    private static byte[] EncodeCompactQuantity(int mantissa, uint scale)
    {
        var mantissaBytes = mantissa == 0
            ? Array.Empty<byte>()
            : new BigInteger(mantissa).ToByteArray(
                isUnsigned: false,
                isBigEndian: false);
        var mantissaPayload = new OfflineNoritoWriter();
        mantissaPayload.WriteUInt32LittleEndian((uint)mantissaBytes.Length);
        mantissaPayload.WriteBytes(mantissaBytes);

        var scalePayload = new byte[sizeof(uint)];
        BinaryPrimitives.WriteUInt32LittleEndian(scalePayload, scale);
        return EncodeCompactFields(mantissaPayload.ToArray(), scalePayload);
    }

    private static byte[] EncodeCompactFields(params byte[][] fields)
    {
        var output = new List<byte>();
        foreach (var field in fields)
        {
            var length = checked((ulong)field.Length);
            do
            {
                var next = (byte)(length & 0x7f);
                length >>= 7;
                if (length != 0)
                {
                    next |= 0x80;
                }
                output.Add(next);
            }
            while (length != 0);
            output.AddRange(field);
        }
        return [.. output];
    }

    private static NumericV1.QuantityValue DecodeQuantity(
        ReadOnlySpan<byte> payload)
    {
        var fields = new FixedFieldReader(payload);
        var mantissaPayload = fields.ReadField();
        var scalePayload = fields.ReadField();
        fields.RequireEnd();
        var length = BinaryPrimitives.ReadUInt32LittleEndian(mantissaPayload);
        Assert.Equal((uint)(mantissaPayload.Length - sizeof(uint)), length);
        var mantissaBytes = mantissaPayload[sizeof(uint)..];
        var mantissa = mantissaBytes.IsEmpty
            ? BigInteger.Zero
            : new BigInteger(
                mantissaBytes,
                isUnsigned: false,
                isBigEndian: false);
        var scale = BinaryPrimitives.ReadUInt32LittleEndian(scalePayload);
        return NumericV1.QuantityValue.FromMantissa(
            mantissa,
            checked((int)scale));
    }

    private ref struct FixedFieldReader
    {
        private CanonicalNoritoReader reader;

        internal FixedFieldReader(ReadOnlySpan<byte> payload)
        {
            reader = new CanonicalNoritoReader(
                payload,
                "cancel-asset-lock test payload",
                nameof(payload));
        }

        internal ReadOnlySpan<byte> ReadField() => reader.ReadField("field");

        internal void RequireEnd() => reader.RequireEnd();
    }
}
