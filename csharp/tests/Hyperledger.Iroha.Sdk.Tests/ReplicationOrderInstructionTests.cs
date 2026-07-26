using System.Buffers.Binary;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class ReplicationOrderInstructionTests
{
    private const string OrderId =
        "abababababababababababababababababababababababababababababababab";
    private const string ProviderId =
        "1010101010101010101010101010101010101010101010101010101010101010";
    private const string FixtureAccountId =
        "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53";
    private const string ReplicationOrderTypeName =
        "sorafs_manifest::capacity::ReplicationOrderV1";

    [Fact]
    public void IssueEncodesTheExactFourFieldNativePayload()
    {
        Assert.Equal(1_048_576, IssueReplicationOrderInstruction.MaximumOrderPayloadBytesV1);
        var fixture = Fixture();
        var instruction = TransactionInstruction.IssueReplicationOrder(
            OrderId,
            fixture,
            issuedEpoch: 20,
            deadlineEpoch: 28);

        Assert.Equal(
            "iroha_data_model::isi::sorafs::IssueReplicationOrder",
            instruction.WireId);
        Assert.Equal(instruction.WireId, instruction.TypeName);
        Assert.Equal(OrderId, instruction.OrderId);
        Assert.Equal(Convert.ToBase64String(fixture), instruction.OrderPayloadBase64);

        var context = new TransactionEncodingContext(FixtureAccountId);
        var encoded = instruction.EncodePayload(context);
        var fields = new FixedFieldReader(encoded);
        Assert.Equal(Convert.FromHexString(OrderId), fields.ReadField().ToArray());
        var orderPayload = fields.ReadField();
        Assert.Equal(
            (ulong)fixture.Length,
            BinaryPrimitives.ReadUInt64LittleEndian(orderPayload));
        Assert.Equal(fixture, orderPayload[sizeof(ulong)..].ToArray());
        Assert.Equal(20ul, BinaryPrimitives.ReadUInt64LittleEndian(fields.ReadField()));
        Assert.Equal(28ul, BinaryPrimitives.ReadUInt64LittleEndian(fields.ReadField()));
        fields.RequireEnd();

        var copy = instruction.OrderPayload;
        copy[0] ^= 0xff;
        Assert.Equal(fixture, instruction.OrderPayload);
        Assert.NotEmpty(instruction.EncodeInstructionBox(FixtureAccountId));
    }

    [Fact]
    public void CompleteRequiresProviderAndEncodesExactlyThreeFields()
    {
        var instruction = TransactionInstruction.CompleteReplicationOrder(
            OrderId,
            ProviderId,
            completionEpoch: 27);
        var context = new TransactionEncodingContext(FixtureAccountId);
        var fields = new FixedFieldReader(instruction.EncodePayload(context));

        Assert.Equal(
            "iroha_data_model::isi::sorafs::CompleteReplicationOrder",
            instruction.WireId);
        Assert.Equal(Convert.FromHexString(OrderId), fields.ReadField().ToArray());
        Assert.Equal(Convert.FromHexString(ProviderId), fields.ReadField().ToArray());
        Assert.Equal(27ul, BinaryPrimitives.ReadUInt64LittleEndian(fields.ReadField()));
        fields.RequireEnd();

        Assert.DoesNotContain(
            typeof(CompleteReplicationOrderInstruction).GetConstructors(),
            constructor => constructor.GetParameters().Length == 2);
        Assert.Throws<ArgumentException>(() =>
            TransactionInstruction.CompleteReplicationOrder(
                OrderId,
                new string('0', 64),
                27));
    }

    [Fact]
    public void ExpireEncodesTheExactTwoFieldNativePayload()
    {
        var instruction = TransactionInstruction.ExpireReplicationOrder(
            OrderId,
            expirationEpoch: 29);
        var context = new TransactionEncodingContext(FixtureAccountId);
        var fields = new FixedFieldReader(instruction.EncodePayload(context));

        Assert.Equal(
            "iroha_data_model::isi::sorafs::ExpireReplicationOrder",
            instruction.WireId);
        Assert.Equal(Convert.FromHexString(OrderId), fields.ReadField().ToArray());
        Assert.Equal(29ul, BinaryPrimitives.ReadUInt64LittleEndian(fields.ReadField()));
        fields.RequireEnd();
    }

    [Fact]
    public void BuildersRejectInvalidIdentifiersBase64AndEpochWindows()
    {
        var fixture = Fixture();
        var canonical = Convert.ToBase64String(fixture);

        Assert.Throws<ArgumentException>(() =>
            TransactionInstruction.IssueReplicationOrder(
                OrderId.ToUpperInvariant(),
                fixture,
                1,
                2));
        Assert.Throws<ArgumentException>(() =>
            TransactionInstruction.IssueReplicationOrder(
                OrderId,
                canonical + "\n",
                1,
                2));
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            TransactionInstruction.IssueReplicationOrder(
                OrderId,
                fixture,
                2,
                2));
        Assert.Throws<ArgumentException>(() =>
            TransactionInstruction.IssueReplicationOrder(
                new string('c', 64),
                fixture,
                1,
                2));
        Assert.Throws<ArgumentException>(() =>
            TransactionInstruction.IssueReplicationOrder(
                OrderId,
                new byte[IssueReplicationOrderInstruction.MaximumOrderPayloadBytesV1 + 1],
                1,
                2));
        Assert.Throws<ArgumentException>(() =>
            TransactionInstruction.ExpireReplicationOrder(new string('0', 64), 1));
    }

    [Fact]
    public void IssueRejectsInvalidTargetDuplicateProvidersAndEmbeddedDeadline()
    {
        var fixture = Fixture();
        var duplicateProvider = Mutate(
            fixture,
            Enumerable.Repeat((byte)0x11, 32).ToArray(),
            Enumerable.Repeat((byte)0x10, 32).ToArray());
        Assert.Throws<ArgumentException>(() =>
            TransactionInstruction.IssueReplicationOrder(
                OrderId,
                duplicateProvider,
                1,
                2));

        var zeroTarget = Mutate(
            fixture,
            [0x02, 0x02, 0x00],
            [0x02, 0x00, 0x00]);
        Assert.Throws<ArgumentException>(() =>
            TransactionInstruction.IssueReplicationOrder(
                OrderId,
                zeroTarget,
                1,
                2));

        var invalidDeadline = Mutate(
            fixture,
            LittleEndian(1_700_086_400),
            LittleEndian(1_700_000_000));
        Assert.Throws<ArgumentException>(() =>
            TransactionInstruction.IssueReplicationOrder(
                OrderId,
                invalidDeadline,
                1,
                2));
    }

    [Fact]
    public void TransactionBuilderExposesAllThreeCanonicalInstructions()
    {
        var builder = new TransactionBuilder(
                "00000042",
                FixtureAccountId,
                FeePaymentIntent.Authority(Array.Empty<FeeChargeLimit>()))
            .IssueReplicationOrder(OrderId, Fixture(), 20, 28)
            .CompleteReplicationOrder(OrderId, ProviderId, 27)
            .ExpireReplicationOrder(OrderId, 29);

        Assert.Collection(
            builder.Instructions,
            instruction => Assert.IsType<IssueReplicationOrderInstruction>(instruction),
            instruction => Assert.IsType<CompleteReplicationOrderInstruction>(instruction),
            instruction => Assert.IsType<ExpireReplicationOrderInstruction>(instruction));
    }

    private static byte[] Fixture()
    {
        return File.ReadAllBytes(Path.Combine(
            AppContext.BaseDirectory,
            "Fixtures",
            "sorafs_manifest",
            "replication_order",
            "order_v1.to"));
    }

    private static byte[] Mutate(
        ReadOnlySpan<byte> fixture,
        ReadOnlySpan<byte> needle,
        ReadOnlySpan<byte> replacement)
    {
        Assert.Equal(needle.Length, replacement.Length);
        var body = fixture[NoritoHeader.EncodedLength..].ToArray();
        var offset = Find(body, needle, 0);
        Assert.True(offset >= 0);
        Assert.Equal(-1, Find(body, needle, offset + 1));
        replacement.CopyTo(body.AsSpan(offset, replacement.Length));
        return NoritoCodec.Encode(ReplicationOrderTypeName, body, flags: 0x02);
    }

    private static int Find(ReadOnlySpan<byte> haystack, ReadOnlySpan<byte> needle, int start)
    {
        for (var index = start; index <= haystack.Length - needle.Length; index++)
        {
            if (haystack[index..].StartsWith(needle))
            {
                return index;
            }
        }
        return -1;
    }

    private static byte[] LittleEndian(ulong value)
    {
        var bytes = new byte[sizeof(ulong)];
        BinaryPrimitives.WriteUInt64LittleEndian(bytes, value);
        return bytes;
    }

    private ref struct FixedFieldReader
    {
        private readonly ReadOnlySpan<byte> payload;
        private int offset;

        internal FixedFieldReader(ReadOnlySpan<byte> payload)
        {
            this.payload = payload;
            offset = 0;
        }

        internal ReadOnlySpan<byte> ReadField()
        {
            Assert.True(offset + sizeof(ulong) <= payload.Length);
            var length = BinaryPrimitives.ReadUInt64LittleEndian(
                payload.Slice(offset, sizeof(ulong)));
            offset += sizeof(ulong);
            Assert.True(length <= int.MaxValue);
            Assert.True((int)length <= payload.Length - offset);
            var result = payload.Slice(offset, (int)length);
            offset += (int)length;
            return result;
        }

        internal void RequireEnd()
        {
            Assert.Equal(payload.Length, offset);
        }
    }
}
