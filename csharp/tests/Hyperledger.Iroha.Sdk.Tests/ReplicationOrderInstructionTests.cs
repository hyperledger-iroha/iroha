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
    private const string PolicyId =
        "2121212121212121212121212121212121212121212121212121212121212121";
    private const string PredecessorDigest =
        "3232323232323232323232323232323232323232323232323232323232323232";
    private const string PolicyDigest =
        "4343434343434343434343434343434343434343434343434343434343434343";
    private const string BlockHash =
        "5454545454545454545454545454545454545454545454545454545454545454";
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
        Assert.Equal(
            Convert.FromHexString(OrderId),
            ReadIdentifier(fields.ReadField()));
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
    public void CompleteEncodesTheExactSixFieldAuthorityHardCut()
    {
        var instruction = TransactionInstruction.CompleteReplicationOrder(
            OrderId,
            ProviderId,
            completionEpoch: 27,
            expectedAuthority: ExpectedAuthority(),
            expectedAssignmentRevision: 3,
            finalizedAnchor: FinalizedAnchor());
        var context = new TransactionEncodingContext(FixtureAccountId);
        var fields = new FixedFieldReader(instruction.EncodePayload(context));

        Assert.Equal(
            "iroha_data_model::isi::sorafs::CompleteReplicationOrder",
            instruction.WireId);
        Assert.Equal(Convert.FromHexString(OrderId), ReadIdentifier(fields.ReadField()));
        Assert.Equal(Convert.FromHexString(ProviderId), ReadIdentifier(fields.ReadField()));
        Assert.Equal(27ul, BinaryPrimitives.ReadUInt64LittleEndian(fields.ReadField()));

        var authority = new FixedFieldReader(fields.ReadField());
        Assert.Equal(
            context.EncodeAccountId(FixtureAccountId),
            authority.ReadField().ToArray());
        var signerPolicy = new FixedFieldReader(authority.ReadField());
        Assert.Equal(Convert.FromHexString(PolicyId), signerPolicy.ReadField().ToArray());
        Assert.Equal(2ul, BinaryPrimitives.ReadUInt64LittleEndian(signerPolicy.ReadField()));
        Assert.Equal(
            Convert.FromHexString(PredecessorDigest),
            ReadOptionalFixedByteArray(signerPolicy.ReadField()));
        Assert.Equal(
            Convert.FromHexString(PolicyDigest),
            signerPolicy.ReadField().ToArray());
        signerPolicy.RequireEnd();
        authority.RequireEnd();

        Assert.Equal(3ul, BinaryPrimitives.ReadUInt64LittleEndian(fields.ReadField()));
        var anchor = new FixedFieldReader(fields.ReadField());
        Assert.Equal(41ul, BinaryPrimitives.ReadUInt64LittleEndian(anchor.ReadField()));
        Assert.Equal(Convert.FromHexString(BlockHash), anchor.ReadField().ToArray());
        anchor.RequireEnd();
        fields.RequireEnd();

        Assert.DoesNotContain(
            typeof(CompleteReplicationOrderInstruction).GetConstructors(),
            constructor => constructor.GetParameters().Length < 6);
        Assert.Throws<ArgumentException>(() =>
            TransactionInstruction.CompleteReplicationOrder(
                OrderId,
                new string('0', 64),
                27,
                expectedAuthority: ExpectedAuthority(),
                expectedAssignmentRevision: 3,
                finalizedAnchor: FinalizedAnchor()));
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            TransactionInstruction.CompleteReplicationOrder(
                OrderId,
                ProviderId,
                27,
                expectedAuthority: ExpectedAuthority(),
                expectedAssignmentRevision: 0,
                finalizedAnchor: FinalizedAnchor()));
        Assert.Throws<ArgumentException>(() =>
            new ProviderIngestCompletionSignerPolicyV1(
                PolicyId,
                2,
                predecessorDigest: null,
                policyDigest: PolicyDigest));
        Assert.Throws<ArgumentException>(() =>
            new ProviderIngestCompletionAuthorityV1(
                $" {FixtureAccountId}",
                ExpectedAuthority().SignerPolicy));
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            new ProviderIngestFinalizedAnchorV1(0, BlockHash));
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
        Assert.Equal(Convert.FromHexString(OrderId), ReadIdentifier(fields.ReadField()));
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
            .CompleteReplicationOrder(
                OrderId,
                ProviderId,
                27,
                ExpectedAuthority(),
                3,
                FinalizedAnchor())
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

    private static ProviderIngestCompletionAuthorityV1 ExpectedAuthority()
    {
        return new ProviderIngestCompletionAuthorityV1(
            FixtureAccountId,
            new ProviderIngestCompletionSignerPolicyV1(
                PolicyId,
                2,
                PredecessorDigest,
                PolicyDigest));
    }

    private static ProviderIngestFinalizedAnchorV1 FinalizedAnchor()
    {
        return new ProviderIngestFinalizedAnchorV1(41, BlockHash);
    }

    private static byte[] ReadIdentifier(ReadOnlySpan<byte> payload)
    {
        var identifier = new FixedFieldReader(payload);
        var bytes = identifier.ReadField().ToArray();
        identifier.RequireEnd();
        return bytes;
    }

    private static byte[] ReadOptionalFixedByteArray(ReadOnlySpan<byte> payload)
    {
        Assert.False(payload.IsEmpty);
        Assert.Equal(1, payload[0]);
        var some = new FixedFieldReader(payload[1..]);
        var array = new FixedFieldReader(some.ReadField());
        var bytes = new byte[32];
        for (var index = 0; index < bytes.Length; index++)
        {
            var item = array.ReadField();
            Assert.Single(item.ToArray());
            bytes[index] = item[0];
        }
        array.RequireEnd();
        some.RequireEnd();
        return bytes;
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
