using System.Buffers.Binary;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Sccp;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class AccountControllerV1Tests
{
    [Fact]
    public void FullMultisigControllerEncodesCanonicalMembersThresholdAndWeights()
    {
        var keys = Keys(2);
        var address = Address(2, [(keys[0], 1), (keys[1], 2)]);
        var encoded = new TransactionEncodingContext(address.ToI105()).EncodeAccountId(address.ToI105());
        Assert.Equal(encoded, SccpReplayPrincipalV1.SoraAccount(encoded).Bytes);
        Assert.Equal(1u, BinaryPrimitives.ReadUInt32LittleEndian(encoded));
        var weighted = Address(2, [(keys[0], 2), (keys[1], 1)]);
        var threshold = Address(1, [(keys[0], 1), (keys[1], 2)]);
        Assert.NotEqual(encoded, new TransactionEncodingContext(weighted.ToI105()).EncodeAccountId(weighted.ToI105()));
        Assert.NotEqual(encoded, new TransactionEncodingContext(threshold.ToI105()).EncodeAccountId(threshold.ToI105()));
        Assert.Equal(encoded, new TransactionEncodingContext(address.ToI105(42)).EncodeAccountId(address.ToI105(42)));
    }

    [Fact]
    public void AddressRejectsNoncanonicalOrderDuplicatesAndDegeneratePolicies()
    {
        var keys = Keys(2);
        Assert.Throws<AccountAddressException>(() => Address(1, [(keys[1], 1), (keys[0], 1)]));
        Assert.Throws<AccountAddressException>(() => Address(1, [(keys[0], 1), (keys[0], 2)]));
        Assert.Throws<AccountAddressException>(() => Address(1, [(keys[0], 0)]));
        Assert.Throws<AccountAddressException>(() => Address(2, [(keys[0], 1)]));
        Assert.Throws<AccountAddressException>(() => Address(0, [(keys[0], 1)]));
        var bytes = Address(1, [(keys[0], 1)]).CanonicalBytes();
        bytes[2] = 2;
        Assert.Throws<AccountAddressException>(() => AccountAddress.FromCanonicalBytes(bytes));
        bytes[2] = 1;
        var retiredCount = bytes.Where((_, index) => index != 5).ToArray();
        Assert.Throws<AccountAddressException>(() => AccountAddress.FromCanonicalBytes(retiredCount));
    }

    [Fact]
    public void AccountControllerRetainsMoreThanOneByteOfMemberCount()
    {
        var keys = Keys(256);
        var address = Address(256, keys.Select(static key => (key, (ushort)1)).ToArray());
        Assert.Equal(new byte[] { 1, 0 }, address.CanonicalBytes()[5..7]);
        var encoded = new TransactionEncodingContext(address.ToI105()).EncodeAccountId(address.ToI105());
        Assert.Equal(encoded, SccpReplayPrincipalV1.SoraAccount(encoded).Bytes);
        Assert.Equal(256, address.GetMultisigPolicy()!.Members.Count);
    }

    private static byte[][] Keys(int count) => Enumerable.Range(1, count).Select(static index =>
    {
        var seed = new byte[32]; BinaryPrimitives.WriteInt32LittleEndian(seed, index);
        using var keyPair = Ed25519KeyPair.FromSeed(seed);
        return keyPair.PublicKey;
    }).OrderBy(static key => Convert.ToHexString(key), StringComparer.Ordinal).ToArray();

    private static AccountAddress Address(ushort threshold, (byte[] Key, ushort Weight)[] members)
    {
        using var stream = new MemoryStream();
        stream.Write([0x0a, 1, 1]);
        Span<byte> value = stackalloc byte[2];
        BinaryPrimitives.WriteUInt16BigEndian(value, threshold); stream.Write(value);
        BinaryPrimitives.WriteUInt16BigEndian(value, checked((ushort)members.Length)); stream.Write(value);
        foreach (var (key, weight) in members)
        {
            stream.WriteByte(1);
            BinaryPrimitives.WriteUInt16BigEndian(value, weight); stream.Write(value);
            BinaryPrimitives.WriteUInt16BigEndian(value, checked((ushort)key.Length)); stream.Write(value);
            stream.Write(key);
        }
        return AccountAddress.FromCanonicalBytes(stream.ToArray());
    }
}
