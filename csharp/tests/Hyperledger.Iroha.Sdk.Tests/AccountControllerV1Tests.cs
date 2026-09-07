using System.Buffers.Binary;
using System.Text.Json;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Sccp;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class AccountControllerV1Tests
{
    [Fact]
    public void AllRustOwnedFullControllerFixturesMatchCanonicalWire()
    {
        var directory = new DirectoryInfo(AppContext.BaseDirectory);
        while (directory is not null && !File.Exists(Path.Combine(directory.FullName, "Cargo.toml"))) directory = directory.Parent;
        Assert.NotNull(directory);
        using var fixture = JsonDocument.Parse(File.ReadAllBytes(Path.Combine(directory.FullName, "fixtures/account/multisig_wire_v1.json")));
        var root = fixture.RootElement;
        Assert.Equal("iroha.account.multisig-wire.v1", root.GetProperty("schema").GetString());
        Assert.Equal(753, root.GetProperty("chain_discriminant").GetInt32());
        var positives = root.GetProperty("positive");
        Assert.Equal(16, positives.GetArrayLength());
        foreach (var item in positives.EnumerateArray())
        {
            Assert.Equal(2, item.GetProperty("layout_flags").GetInt32());
            var address = AccountAddress.Parse(item.GetProperty("i105").GetString()!, 753);
            Assert.Equal(Convert.FromHexString(item.GetProperty("canonical_address_hex").GetString()!), address.CanonicalBytes());
            var payload = Convert.FromHexString(item.GetProperty("account_id_payload_hex").GetString()!);
            var actual = new TransactionEncodingContext(address.ToI105()).EncodeAccountId(address.ToI105());
            Assert.Equal(payload, actual);
            var frame = Convert.FromHexString(item.GetProperty("account_id_frame_hex").GetString()!);
            var decoded = NoritoCodec.DecodeWithSchemaHash(frame.AsSpan(6, 16), frame);
            Assert.Equal(2, decoded.Flags); Assert.Equal(payload, decoded.Payload);
            var policy = item.GetProperty("policy");
            if (policy.ValueKind == JsonValueKind.Null) Assert.Null(address.GetMultisigPolicy());
            else
            {
                var parsed = Assert.IsType<AccountAddress.MultisigPolicy>(address.GetMultisigPolicy());
                Assert.Equal(policy.GetProperty("version").GetByte(), parsed.Version);
                Assert.Equal(policy.GetProperty("threshold").GetUInt16(), parsed.Threshold);
                var members = policy.GetProperty("members").EnumerateArray().ToArray();
                Assert.Equal(members.Length, parsed.Members.Count);
                for (var index = 0; index < members.Length; index++)
                {
                    Assert.Equal(members[index].GetProperty("curve_id").GetByte(), (byte)parsed.Members[index].Curve);
                    Assert.Equal(members[index].GetProperty("weight").GetUInt16(), parsed.Members[index].Weight);
                    Assert.Equal(Convert.FromHexString(members[index].GetProperty("public_key_hex").GetString()!), parsed.Members[index].PublicKey);
                }
            }
        }
        var negatives = root.GetProperty("negative");
        Assert.Equal(7, negatives.GetArrayLength());
        foreach (var item in negatives.EnumerateArray())
        {
            Assert.Equal(2, item.GetProperty("layout_flags").GetInt32());
            var payload = Convert.FromHexString(item.GetProperty("account_id_payload_hex").GetString()!);
            Assert.ThrowsAny<ArgumentException>(() => SccpReplayPrincipalV1.SoraAccount(payload));
        }
    }

    [Theory]
    [InlineData(CurveId.Ed25519, 1, 0, "ed25519", 32)]
    [InlineData(CurveId.Secp256k1, 4, 1, "secp256k1", 33)]
    [InlineData(CurveId.BlsNormal, 3, 2, "bls_normal", 48)]
    [InlineData(CurveId.BlsSmall, 5, 3, "bls_small", 96)]
    [InlineData(CurveId.MlDsa, 2, 4, "ml-dsa", 1952)]
    [InlineData(CurveId.Gost256A, 10, 5, "gost3410-2012-256-paramset-a", 64)]
    [InlineData(CurveId.Gost256B, 11, 6, "gost3410-2012-256-paramset-b", 64)]
    [InlineData(CurveId.Gost256C, 12, 7, "gost3410-2012-256-paramset-c", 64)]
    [InlineData(CurveId.Gost512A, 13, 8, "gost3410-2012-512-paramset-a", 128)]
    [InlineData(CurveId.Gost512B, 14, 9, "gost3410-2012-512-paramset-b", 128)]
    [InlineData(CurveId.Sm2, 15, 10, "sm2", 67)]
    public void PublishedCurveEnvelopesMapToFinalNoritoTags(CurveId curve, int curveId, int tag, string name, int length)
    {
        // These synthetic envelope values test representation, not group admission.
        var key = Enumerable.Repeat((byte)1, length).ToArray();
        if (curve == CurveId.Secp256k1) key[0] = 2;
        if (curve == CurveId.Sm2) { key[0] = 0; key[1] = 0; key[2] = 4; }
        var address = AccountAddress.FromPublicKey(key, curve);
        Assert.Equal(curveId, (byte)curve); Assert.Equal(name, address.Algorithm);
        var encoded = new TransactionEncodingContext(address.ToI105()).EncodeAccountId(address.ToI105());
        var reader = new CanonicalNoritoReader(encoded, "account", nameof(encoded));
        Assert.Equal(0u, reader.ReadUInt32LittleEndian("single"));
        var publicKey = new CanonicalNoritoReader(reader.ReadField("key"), "public key", nameof(encoded));
        Assert.Equal((ulong)length + 1, publicKey.ReadSequenceLength("key length"));
        Assert.Equal((byte)tag, Assert.Single(publicKey.ReadField("algorithm").ToArray()));
        foreach (var value in key) Assert.Equal(value, Assert.Single(publicKey.ReadField("byte").ToArray()));
        reader.RequireEnd(); publicKey.RequireEnd();
        foreach (var invalid in new[] { new byte[length], new byte[length - 1], new byte[length + 1] })
            Assert.Throws<AccountAddressException>(() => AccountAddress.FromPublicKey(invalid, curve));
    }

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
