using System.Numerics;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>
/// The transaction wire codec on hand-built values: every modeled envelope shape round-trips in
/// both layouts and each strict-decoding rule refuses its counterexample. Needs no native
/// artifact.
/// </summary>
public sealed class TransactionWireCodecTests
{
    private static readonly byte[] NetworkIdBytes = Enumerable.Range(1, 32).Select(static value => (byte)value).ToArray();
    private static readonly byte[] KeyA = [0, .. Enumerable.Repeat((byte)0xA1, 32)];
    private static readonly byte[] KeyB = [0, .. Enumerable.Repeat((byte)0xB2, 32)];
    private static readonly byte[] AssetDefinition = Enumerable.Range(0x10, 16).Select(static value => (byte)value).ToArray();
    private static readonly byte[] CodeHash = Enumerable.Repeat((byte)0x0f, 32).ToArray();

    public static TheoryData<NoritoWireLayout> TypedLayouts => [NoritoWireLayout.Compact, NoritoWireLayout.Dense];

    [Theory]
    [MemberData(nameof(TypedLayouts))]
    public void EveryModeledShapeRoundTripsThroughTheStrictDecoder(NoritoWireLayout layout)
    {
        foreach (var payload in ModeledPayloads(layout))
        {
            var encoded = TransactionWireCodec.EncodePayload(payload);
            var decoded = TransactionWireCodec.DecodePayload(encoded, layout, "payload");
            Assert.Equal(layout, decoded.Layout);
            Assert.Equal(encoded, TransactionWireCodec.EncodePayload(decoded));
            Assert.Equal(payload.IsGenesisDomain, decoded.IsGenesisDomain);
            Assert.Equal(payload.NetworkId, decoded.NetworkId);
            Assert.Equal(payload.CreationTimeMilliseconds, decoded.CreationTimeMilliseconds);
            Assert.Equal(payload.TimeToLiveMilliseconds, decoded.TimeToLiveMilliseconds);
            Assert.Equal(payload.Nonce, decoded.Nonce);
            Assert.Equal(payload.Metadata, decoded.Metadata);
            Assert.Equal(payload.FeePayment.GetType(), decoded.FeePayment.GetType());
            Assert.Equal(payload.Executable.GetType(), decoded.Executable.GetType());
            Assert.Equal(payload.Authority.GetType(), decoded.Authority.GetType());

            var signature = Enumerable.Range(0, 64).Select(static value => (byte)(value + 1)).ToArray();
            var signed = TransactionWireCodec.EncodeSignedTransaction(new WireSignedTransaction(signature, payload));
            Assert.Equal(signed, TransactionWireCodec.EncodeSignedTransaction(layout, signature, encoded));
            var transaction = BareSignedTransaction.Decode(signed, layout);
            Assert.Equal(layout, transaction.Layout);
            Assert.Equal(signature, transaction.Signature);
            Assert.Equal(encoded, transaction.PayloadBytes);
            Assert.Equal(IrohaHash.Hash(encoded), transaction.SigningHash);
            Assert.Equal(signed, transaction.SignedTransactionBytes);
            Assert.Equal(signed, BareSignedTransaction.DecodeFrame(transaction.EncodeFrame()).SignedTransactionBytes);

            // Returned arrays are copies.
            transaction.SignedTransactionBytes[0] ^= 0xff;
            transaction.PayloadBytes[0] ^= 0xff;
            transaction.SigningHash[0] ^= 0xff;
            transaction.Signature[0] ^= 0xff;
            Assert.Equal(signed, transaction.SignedTransactionBytes);
            Assert.Equal(encoded, transaction.PayloadBytes);
            Assert.Equal(signature, transaction.Signature);
        }
    }

    [Fact]
    public void ValuesWithoutInstructionPayloadsEncodeDifferentlyInEachLayoutAndOnlyDecodeInTheirOwn()
    {
        foreach (var index in Enumerable.Range(0, ModeledPayloads(NoritoWireLayout.Compact).Length))
        {
            var compact = TransactionWireCodec.EncodePayload(ModeledPayloads(NoritoWireLayout.Compact)[index]);
            var dense = TransactionWireCodec.EncodePayload(ModeledPayloads(NoritoWireLayout.Dense)[index]);

            Assert.True(dense.Length < compact.Length);
            Assert.Throws<ArgumentException>(
                () => TransactionWireCodec.DecodePayload(dense, NoritoWireLayout.Compact, "payload"));
            Assert.Throws<ArgumentException>(
                () => TransactionWireCodec.DecodePayload(compact, NoritoWireLayout.Dense, "payload"));
        }
    }

    [Fact]
    public void DenseEnvelopeShapesFollowTheDenseRules()
    {
        const NoritoWireLayout dense = NoritoWireLayout.Dense;
        const NoritoWireLayout compact = NoritoWireLayout.Compact;

        // Rule 5: an enum discriminant is a varint; rule 4: a fixed array is its raw bytes.
        Assert.Equal(
            Convert.ToHexString([0x00, 0x20, .. NetworkIdBytes]),
            Convert.ToHexString(TransactionWireCodec.EncodeDomain(dense, NetworkIdBytes)));
        Assert.Equal(
            Convert.ToHexString([0x00, 0x00, 0x00, 0x00, 0x20, .. NetworkIdBytes]),
            Convert.ToHexString(TransactionWireCodec.EncodeDomain(compact, NetworkIdBytes)));
        Assert.Equal("01", Convert.ToHexString(TransactionWireCodec.EncodeDomain(dense, [], genesis: true)));
        Assert.Equal("01000000", Convert.ToHexString(TransactionWireCodec.EncodeDomain(compact, [], genesis: true)));
        Assert.Throws<ArgumentException>(() => TransactionWireCodec.EncodeDomain(dense, NetworkIdBytes[..31]));

        // Rules 2 and 3: counts are varints and byte sequences are `[count][raw bytes]`.
        Assert.Equal("03010203", Convert.ToHexString(TransactionWireCodec.EncodeConstVec(dense, [1, 2, 3])));
        Assert.Equal(
            "0300000000000000010101020103",
            Convert.ToHexString(TransactionWireCodec.EncodeConstVec(compact, [1, 2, 3])));
        Assert.Equal("03010203", Convert.ToHexString(TransactionWireCodec.EncodeBytesVec(dense, [1, 2, 3])));
        Assert.Equal(
            "0300000000000000010203",
            Convert.ToHexString(TransactionWireCodec.EncodeBytesVec(compact, [1, 2, 3])));
        Assert.Equal(new byte[] { 1, 2, 3 }, TransactionWireCodec.DecodeConstVec([3, 1, 2, 3], "bytes", "bytes", dense));
        Assert.Equal(
            new byte[] { 1, 2, 3 },
            TransactionWireCodec.DecodeConstVec([3, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 2, 1, 3], "bytes", "bytes", compact));
        Assert.Equal(new byte[] { 1, 2, 3 }, TransactionWireCodec.DecodeBytesVec([3, 1, 2, 3], "bytes", "bytes", dense));
        Assert.Throws<ArgumentException>(() => TransactionWireCodec.DecodeConstVec([3, 1, 2], "bytes", "bytes", dense));
        Assert.Throws<ArgumentException>(() => TransactionWireCodec.DecodeBytesVec([3, 1, 2, 3, 4], "bytes", "bytes", dense));

        // Rule 6: the instruction envelope is the wire id and a byte vector of the bare payload.
        var instruction = new WireInstruction("iroha.log", [0xAA, 0xBB]);
        Assert.Equal(
            "0A0969726F68612E6C6F670302AABB",
            Convert.ToHexString(TransactionWireCodec.EncodeInstruction(dense, instruction)));
        Assert.Equal(
            "0A0969726F68612E6C6F670A0200000000000000AABB",
            Convert.ToHexString(TransactionWireCodec.EncodeInstruction(compact, instruction)));

        // Unchanged by dense: strings, fixed-width scalars, `Option` tags and the `u32` mantissa length.
        foreach (var layout in new[] { compact, dense })
        {
            Assert.Equal("026869", Convert.ToHexString(TransactionWireCodec.EncodeString(layout, "hi")));
            Assert.Equal("hi", TransactionWireCodec.DecodeString([2, 0x68, 0x69], "text", "bytes", layout));
            Assert.Equal("0500", Convert.ToHexString(TransactionWireCodec.EncodeUInt16(layout, 5)));
            Assert.Equal("05000000", Convert.ToHexString(TransactionWireCodec.EncodeUInt32(layout, 5)));
            Assert.Equal("0500000000000000", Convert.ToHexString(TransactionWireCodec.EncodeUInt64(layout, 5)));
            Assert.Equal("00", Convert.ToHexString(TransactionWireCodec.EncodeNone(layout)));
            Assert.Equal("0102AABB", Convert.ToHexString(TransactionWireCodec.EncodeSome(layout, [0xAA, 0xBB])));
            Assert.Equal("00", Convert.ToHexString(TransactionWireCodec.EncodeOptionUInt64(layout, null)));
            Assert.Equal("01080700000000000000", Convert.ToHexString(TransactionWireCodec.EncodeOptionUInt64(layout, 7)));
            Assert.Equal("010407000000", Convert.ToHexString(TransactionWireCodec.EncodeOptionUInt32(layout, 7)));
            Assert.Equal("00", Convert.ToHexString(TransactionWireCodec.EncodeOptionUInt32(layout, null)));
            Assert.Equal(
                "0602000000E8030403000000",
                Convert.ToHexString(TransactionWireCodec.EncodeNumeric(layout, new WireNumeric(1000, 3))));
            Assert.Equal(
                "04000000000400000000",
                Convert.ToHexString(TransactionWireCodec.EncodeNumeric(layout, new WireNumeric(BigInteger.Zero, 0))));
            Assert.Equal("050474727565", Convert.ToHexString(TransactionWireCodec.EncodeJsonText(layout, "true")));
            Assert.Throws<ArgumentException>(
                () => TransactionWireCodec.DecodeString([2, 0xff, 0xfe], "text", "bytes", layout));
        }

        Assert.Equal("00", Convert.ToHexString(TransactionWireCodec.EncodeMetadata(dense, [])));
        Assert.Equal("0000000000000000", Convert.ToHexString(TransactionWireCodec.EncodeMetadata(compact, [])));
    }

    [Theory]
    [MemberData(nameof(TypedLayouts))]
    public void StrictDecodingRefusesNonCanonicalAndUnmodeledPayloads(NoritoWireLayout layout)
    {
        var valid = ModeledPayloads(layout)[0];
        _ = TransactionWireCodec.DecodePayload(TransactionWireCodec.EncodePayload(valid), layout, "payload");

        // Metadata keys must be unique and ascending by UTF-8 bytes.
        AssertRefused(valid with { Metadata = [new("b", "1"), new("a", "2")] });
        AssertRefused(valid with { Metadata = [new("a", "1"), new("a", "2")] });
        // U+FF5E sorts before U+1F600 by UTF-8 bytes although its UTF-16 unit is larger.
        _ = TransactionWireCodec.DecodePayload(
            TransactionWireCodec.EncodePayload(valid with { Metadata = [new("～", "1"), new("\U0001F600", "2")] }),
            layout,
            "payload");
        AssertRefused(valid with { Metadata = [new("\U0001F600", "1"), new("～", "2")] });

        // Zero is not a time to live or a nonce.
        AssertRefused(valid with { TimeToLiveMilliseconds = 0 });
        AssertRefused(valid with { Nonce = 0 });
        _ = TransactionWireCodec.DecodePayload(
            TransactionWireCodec.EncodePayload(valid with { TimeToLiveMilliseconds = null, Nonce = null }),
            layout,
            "payload");

        // Wire ids are 1 to 192 bytes of ASCII letters, digits and `. _ : - @ /`.
        foreach (var wireId in new[] { "", "iroha log", "iroha.lög", "iroha#log", new string('a', 193) })
        {
            Assert.False(TransactionWireCodec.IsWellFormedWireId(wireId));
            AssertRefused(valid with { Executable = new WireInstructionsExecutable([new WireInstruction(wireId, [1])]) });
        }

        foreach (var wireId in new[] { "a", "iroha.instruction.v1::transparent::SetAssetKeyValue", "A_b-c@d/e:9", new string('a', 192) })
        {
            Assert.True(TransactionWireCodec.IsWellFormedWireId(wireId));
            var accepted = valid with { Executable = new WireInstructionsExecutable([new WireInstruction(wireId, [1])]) };
            _ = TransactionWireCodec.DecodePayload(TransactionWireCodec.EncodePayload(accepted), layout, "payload");
        }

        // A numeric mantissa has one encoding: minimal two's-complement bytes, none for zero.
        var limit = new WireFeeChargeLimit(0, AssetDefinition, new WireNumeric(5, 0));
        var canonicalFee = FeeWithMantissa([0x05]);
        Assert.Equal(
            TransactionWireCodec.EncodeFeePayment(layout, new WireAuthorityFeePayment([limit], null)),
            canonicalFee);
        _ = TransactionWireCodec.DecodePayload(PayloadWithFee(canonicalFee), layout, "payload");
        foreach (var mantissa in new byte[][] { [0x05, 0x00], [0x05, 0x00, 0x00] })
        {
            var error = Assert.Throws<ArgumentException>(
                () => TransactionWireCodec.DecodePayload(PayloadWithFee(FeeWithMantissa(mantissa)), layout, "payload"));
            Assert.Contains("not canonically encoded", error.Message, StringComparison.Ordinal);
        }

        var zero = Assert.Throws<ArgumentException>(
            () => TransactionWireCodec.DecodePayload(PayloadWithFee(FeeWithMantissa([0x00])), layout, "payload"));
        Assert.Contains("not canonically encoded", zero.Message, StringComparison.Ordinal);
        _ = TransactionWireCodec.DecodePayload(PayloadWithFee(FeeWithMantissa([])), layout, "payload");

        byte[] FeeWithMantissa(byte[] mantissaBytes)
        {
            var mantissa = new CanonicalNoritoWriter(layout);
            mantissa.WriteUInt32LittleEndian((uint)mantissaBytes.Length);
            mantissa.WriteBytes(mantissaBytes);
            var numeric = new CanonicalNoritoWriter(layout);
            numeric.WriteField(mantissa.ToArray());
            numeric.WriteField(TransactionWireCodec.EncodeUInt32(layout, 0));

            var kind = new CanonicalNoritoWriter(layout);
            kind.WriteEnumTag(0);
            var assetDefinition = new CanonicalNoritoWriter(layout);
            assetDefinition.WriteByteElements(AssetDefinition);
            var chargeLimit = new CanonicalNoritoWriter(layout);
            chargeLimit.WriteField(kind.ToArray());
            chargeLimit.WriteField(assetDefinition.ToArray());
            chargeLimit.WriteField(numeric.ToArray());

            var limits = new CanonicalNoritoWriter(layout);
            limits.WriteSequenceLength(1);
            limits.WriteField(chargeLimit.ToArray());
            var body = new CanonicalNoritoWriter(layout);
            body.WriteField(limits.ToArray());
            body.WriteField(TransactionWireCodec.EncodeNone(layout));
            var fee = new CanonicalNoritoWriter(layout);
            fee.WriteEnumTag(0);
            fee.WriteField(body.ToArray());
            return fee.ToArray();
        }

        byte[] PayloadWithFee(byte[] fee) => RawPayload(layout, fee: fee);

        // Unknown charge kinds and wrong fixed lengths are refused by the encoder itself.
        Assert.Throws<ArgumentOutOfRangeException>(
            () => TransactionWireCodec.EncodeFeePayment(
                layout,
                new WireAuthorityFeePayment([limit with { Kind = 2 }], null)));
        Assert.Throws<ArgumentException>(
            () => TransactionWireCodec.EncodeFeePayment(
                layout,
                new WireAuthorityFeePayment([limit with { AssetDefinitionId = AssetDefinition[..15] }], null)));
        Assert.Throws<ArgumentException>(
            () => TransactionWireCodec.EncodeContractInvocation(
                layout,
                new WireContractInvocation("irohac1q", CodeHash[..31], "run", null)));
        Assert.Throws<ArgumentException>(
            () => TransactionWireCodec.EncodeExecutable(layout, new WireBatchExecutable([])));

        // An IvmProved executable (discriminant 3) and any unknown discriminant are refused.
        var ivm = TransactionWireCodec.EncodePayload(valid with { Executable = new WireIvmExecutable([1, 2, 3]) });
        var tagOffset = ivm.AsSpan().IndexOf(layout == NoritoWireLayout.Dense
            ? new byte[] { 0x02, 0x05, 0x04, 0x03, 1, 2, 3 }
            : new byte[] { 0x02, 0x00, 0x00, 0x00, 0x0c, 0x0b, 0x03 });
        Assert.True(tagOffset > 0);
        foreach (var tag in new byte[] { 3, 5, 0x7f })
        {
            var retagged = ivm.ToArray();
            retagged[tagOffset] = tag;
            Assert.Throws<ArgumentException>(() => TransactionWireCodec.DecodePayload(retagged, layout, "payload"));
        }

        void AssertRefused(WireTransactionPayload payload) =>
            Assert.Throws<ArgumentException>(
                () => TransactionWireCodec.DecodePayload(TransactionWireCodec.EncodePayload(payload), layout, "payload"));
    }

    [Theory]
    [MemberData(nameof(TypedLayouts))]
    public void HostileSequenceCountsAreRefusedBeforeAnyElementIsCollected(NoritoWireLayout layout)
    {
        foreach (var count in new ulong[] { 1, 2, 1UL << 20, uint.MaxValue, ulong.MaxValue })
        {
            // A count with no elements behind it, as the instruction list and as the metadata map.
            var sequence = new CanonicalNoritoWriter(layout);
            sequence.WriteSequenceLength(count);
            var executable = new CanonicalNoritoWriter(layout);
            executable.WriteEnumTag(0);
            executable.WriteField(sequence.ToArray());

            var instructions = Assert.Throws<ArgumentException>(
                () => TransactionWireCodec.DecodePayload(
                    RawPayload(layout, executable: executable.ToArray()), layout, "payload"));
            Assert.Contains("exceeds the remaining input", instructions.Message, StringComparison.Ordinal);

            var metadata = Assert.Throws<ArgumentException>(
                () => TransactionWireCodec.DecodePayload(
                    RawPayload(layout, metadata: sequence.ToArray()), layout, "payload"));
            Assert.Contains("exceeds the remaining input", metadata.Message, StringComparison.Ordinal);
        }

        // The same frames with an honest zero count decode.
        _ = TransactionWireCodec.DecodePayload(RawPayload(layout), layout, "payload");
    }

    // A payload assembled field by field, so a test can substitute hand-written field bytes.
    private static byte[] RawPayload(
        NoritoWireLayout layout,
        byte[]? executable = null,
        byte[]? fee = null,
        byte[]? metadata = null)
    {
        var writer = new CanonicalNoritoWriter(layout);
        writer.WriteField(TransactionWireCodec.EncodeDomain(layout, NetworkIdBytes));
        writer.WriteField(TransactionWireCodec.EncodeAccountController(layout, new WireSingleKeyController(KeyA)));
        writer.WriteField(TransactionWireCodec.EncodeUInt64(layout, 1));
        writer.WriteField(executable
            ?? TransactionWireCodec.EncodeExecutable(layout, new WireInstructionsExecutable([])));
        writer.WriteField(TransactionWireCodec.EncodeOptionUInt64(layout, 5));
        writer.WriteField(TransactionWireCodec.EncodeOptionUInt32(layout, null));
        writer.WriteField(fee
            ?? TransactionWireCodec.EncodeFeePayment(layout, new WireAuthorityFeePayment([], null)));
        writer.WriteField(metadata ?? TransactionWireCodec.EncodeMetadata(layout, []));
        writer.WriteField(TransactionWireCodec.EncodeNone(layout));
        return writer.ToArray();
    }

    // One payload per modeled alternative: both domains, both controllers, every executable and
    // both fee payers. None carries an instruction payload that would bind it to a layout.
    private static WireTransactionPayload[] ModeledPayloads(NoritoWireLayout layout)
    {
        var single = new WireSingleKeyController(KeyA);
        var multisig = new WireMultisigController(
            1,
            2,
            [new WireMultisigMember(KeyA, 1), new WireMultisigMember(KeyB, 2)]);
        var call = new WireContractInvocation("irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh", CodeHash, "run", [1, 2, 3, 4]);
        var bareCall = call with { Arguments = null };
        WireFeeChargeLimit[] limits =
        [
            new(0, AssetDefinition, new WireNumeric(1000, 0)),
            new(1, AssetDefinition, new WireNumeric(BigInteger.Parse("340282366920938463463374607431768211456"), 18)),
        ];
        WireMetadataEntry[] metadata = [new("checked", "true"), new("note", "{\"a\":[1,null]}")];

        return
        [
            new WireTransactionPayload(
                layout,
                NetworkIdBytes,
                single,
                1_735_000_300_000,
                new WireContractCallExecutable(call),
                4000,
                13,
                new WireAuthorityFeePayment(limits, 100_000),
                metadata),
            new WireTransactionPayload(
                layout,
                null,
                multisig,
                1,
                new WireIvmExecutable(Enumerable.Range(0, 200).Select(static value => (byte)value).ToArray()),
                ulong.MaxValue,
                uint.MaxValue,
                new WireSponsorFeePayment(multisig, "program", 7, limits, null),
                []),
            new WireTransactionPayload(
                layout,
                NetworkIdBytes,
                multisig,
                ulong.MaxValue,
                new WireBatchExecutable([new WireBatchContractCall(call), new WireBatchContractCall(bareCall)]),
                1,
                1,
                new WireSponsorFeePayment(single, "p", 1, [], 1),
                metadata),
            new WireTransactionPayload(
                layout,
                NetworkIdBytes,
                single,
                0,
                new WireInstructionsExecutable([]),
                5,
                null,
                new WireAuthorityFeePayment([], null),
                []),
        ];
    }
}
