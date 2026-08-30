using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Sccp;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class SccpReplayV1Tests
{
    [Fact]
    public void SharedReplayForestGoldenAndCanonicalWitnessesMatch()
    {
        var domainHash = SccpReplayV1.DomainHash(
            SccpNetworkV1.SoraTaira,
            SccpNetworkV1.EthereumMainnet,
            SccpReplayBoundaryV1.SoraOutboundLock,
            7,
            Repeated(0x44, 32),
            SccpReplayActorV1.Route());
        Assert.Equal(
            "de11cbd183f55063fe715fcf120773d799dfb1185e057f758c126306832fdc3d",
            SccpV1.LowerHex(domainHash));

        var key = SccpReplayV1.ReplayKey(domainHash, Repeated(0x11, 32));
        Assert.Equal(
            "139f57881d055a13ecf390d7441dadfc065ded40181c42a7aa3ab0a27469f17b",
            SccpV1.LowerHex(key));
        Assert.Equal(19, key[0]);

        var recordDigest = SccpReplayV1.RecordDigest(
            SccpReplayBoundaryV1.SoraOutboundLock,
            Repeated(0x11, 32),
            Repeated(0x22, 32),
            9,
            SccpReplayPrincipalV1.Evm(Repeated(0x33, 20)),
            Repeated(0x55, 32));
        Assert.Equal(
            "31e4f2267d63d21101ab070e04aefe660df9681d3e12b263b61676e07c6f4aa5",
            SccpV1.LowerHex(recordDigest));

        var empty = SccpReplayV1.EmptyHashes();
        Assert.Equal(
            "6841d062186b649a505eb694ebce936fe978c5530596882a70c6e04303c88d43",
            SccpV1.LowerHex(empty[0]));
        Assert.Equal(
            "cefd4f39c0d2ba5c33835008c6c3e7bca47d6ea1c4da5bfc8a63f09dbc66651f",
            SccpV1.LowerHex(empty[SccpReplayV1.Depth]));

        var emptyWitness = new SccpSparseMerkleWitnessV1(
            empty[SccpReplayV1.Depth],
            new byte[32],
            new byte[32],
            []);
        Assert.True(SccpReplayV1.RootFromWitness(key, null, emptyWitness).MatchesExpectedRoot);

        var occupiedRoot = SccpV1.DecodeLowerHex(
            "d9c75ee102ec40076d903d6d5a0c3b0f9a9fa006ea9a2638274be11712ffb849");
        var occupiedWitness = new SccpSparseMerkleWitnessV1(
            occupiedRoot,
            recordDigest,
            new byte[32],
            []);
        Assert.True(SccpReplayV1.RootFromWitness(
            key, recordDigest, occupiedWitness).MatchesExpectedRoot);

        var reservedBitmap = new byte[32];
        reservedBitmap[0] = 1;
        var reservedWitness = new SccpSparseMerkleWitnessV1(
            empty[SccpReplayV1.Depth],
            new byte[32],
            reservedBitmap,
            [Repeated(0xaa, 32)]);
        Assert.Throws<ArgumentException>(() =>
            SccpReplayV1.RootFromWitness(key, null, reservedWitness));

        var explicitDefaultBitmap = new byte[32];
        explicitDefaultBitmap[31] = 1;
        var explicitDefaultWitness = new SccpSparseMerkleWitnessV1(
            empty[SccpReplayV1.Depth],
            new byte[32],
            explicitDefaultBitmap,
            [empty[0]]);
        Assert.Throws<ArgumentException>(() =>
            SccpReplayV1.RootFromWitness(key, null, explicitDefaultWitness));
    }

    [Fact]
    public void SoraPrincipalAcceptsExactCompactAccountId()
    {
        var publicKey = Ed25519KeyPair.FromSeed(Repeated(0x42, 32)).PublicKey;
        var accountId = AccountAddress.FromPublicKey(publicKey).ToI105();
        var payload = new TransactionEncodingContext(accountId).EncodeAccountId(accountId);

        var principal = SccpReplayPrincipalV1.SoraAccount(payload);

        Assert.Equal(payload, principal.Bytes);
    }

    [Fact]
    public void SoraPrincipalRejectsMalformedOrNoncanonicalAccountId()
    {
        var publicKey = Ed25519KeyPair.FromSeed(Repeated(0x43, 32)).PublicKey;
        var accountId = AccountAddress.FromPublicKey(publicKey).ToI105();
        var canonical = new TransactionEncodingContext(accountId).EncodeAccountId(accountId);
        var trailing = canonical.Concat(new byte[] { 0 }).ToArray();
        var unknownController = canonical.ToArray();
        unknownController[0] = 2;
        var overlongLength = new byte[canonical.Length + 1];
        canonical[..4].CopyTo(overlongLength, 0);
        overlongLength[4] = (byte)(canonical[4] | 0x80);
        overlongLength[5] = 0;
        canonical[5..].CopyTo(overlongLength, 6);
        var shortEd25519Key = CompactSingleAccount(new byte[31]);

        foreach (var malformed in new[]
        {
            Array.Empty<byte>(),
            new byte[] { 0 },
            canonical[..^1],
            trailing,
            unknownController,
            overlongLength,
            shortEd25519Key,
        })
        {
            Assert.Throws<ArgumentException>(() =>
                SccpReplayPrincipalV1.SoraAccount(malformed));
        }
    }

    [Fact]
    public void SoraPrincipalAcceptsCanonicalMultisigAndRejectsNoncanonicalMembers()
    {
        var keys = new[]
        {
            Ed25519KeyPair.FromSeed(Repeated(0x51, 32)).PublicKey,
            Ed25519KeyPair.FromSeed(Repeated(0x52, 32)).PublicKey,
        };
        Array.Sort(keys, static (left, right) =>
            left.AsSpan().SequenceCompareTo(right));
        var canonical = CompactMultisigAccount(
            (keys[0], (ushort)1),
            (keys[1], (ushort)1));

        var principal = SccpReplayPrincipalV1.SoraAccount(canonical);

        Assert.Equal(canonical, principal.Bytes);
        Assert.Throws<ArgumentException>(() =>
            SccpReplayPrincipalV1.SoraAccount(CompactMultisigAccount(
                (keys[1], (ushort)1),
                (keys[0], (ushort)1))));
        Assert.Throws<ArgumentException>(() =>
            SccpReplayPrincipalV1.SoraAccount(CompactMultisigAccount(
                (keys[0], (ushort)1),
                (keys[0], (ushort)1))));
    }

    [Fact]
    public void RecordDigestRejectsUnknownReplayBoundary()
    {
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            SccpReplayV1.RecordDigest(
                (SccpReplayBoundaryV1)0xFF,
                Repeated(0x11, 32),
                Repeated(0x22, 32),
                9,
                SccpReplayPrincipalV1.Evm(Repeated(0x33, 20)),
                Repeated(0x55, 32)));
    }

    private static byte[] CompactMultisigAccount(
        params (byte[] PublicKey, ushort Weight)[] members)
    {
        var policy = new CanonicalNoritoWriter();
        var version = new CanonicalNoritoWriter();
        version.WriteByte(1);
        policy.WriteField(version.ToArray());
        var threshold = new CanonicalNoritoWriter();
        threshold.WriteUInt16LittleEndian(2);
        policy.WriteField(threshold.ToArray());
        var encodedMembers = new CanonicalNoritoWriter();
        encodedMembers.WriteSequenceLength((ulong)members.Length);
        foreach (var (publicKey, weight) in members)
        {
            var member = new CanonicalNoritoWriter();
            member.WriteField(CompactPublicKey(publicKey));
            var encodedWeight = new CanonicalNoritoWriter();
            encodedWeight.WriteUInt16LittleEndian(weight);
            member.WriteField(encodedWeight.ToArray());
            encodedMembers.WriteField(member.ToArray());
        }
        policy.WriteField(encodedMembers.ToArray());

        var account = new CanonicalNoritoWriter();
        account.WriteUInt32LittleEndian(1);
        account.WriteField(policy.ToArray());
        return account.ToArray();
    }

    private static byte[] CompactSingleAccount(ReadOnlySpan<byte> publicKey)
    {
        var account = new CanonicalNoritoWriter();
        account.WriteUInt32LittleEndian(0);
        account.WriteField(CompactPublicKey(publicKey));
        return account.ToArray();
    }

    private static byte[] CompactPublicKey(ReadOnlySpan<byte> publicKey)
    {
        var encoded = new byte[publicKey.Length + 1];
        publicKey.CopyTo(encoded.AsSpan(1));
        var writer = new CanonicalNoritoWriter();
        writer.WriteSequenceLength((ulong)encoded.Length);
        writer.WriteByteElements(encoded);
        return writer.ToArray();
    }

    private static byte[] Repeated(byte value, int count) =>
        Enumerable.Repeat(value, count).ToArray();
}
