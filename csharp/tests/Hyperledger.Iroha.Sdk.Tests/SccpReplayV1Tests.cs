using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Sccp;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class SccpReplayV1Tests
{
    [Fact]
    public void LocalFinalV1ReplayVectorAndCanonicalWitnessesMatch()
    {
        var domainHash = SccpReplayV1.DomainHash(
            SccpNetworkV1.SoraTaira,
            SccpNetworkV1.EthereumMainnet,
            SccpReplayBoundaryV1.EvmDestinationMint,
            7,
            Repeated(0x44, 32),
            SccpReplayActorV1.Evm(Repeated(0x33, 20)));
        Assert.Equal(
            "ebc495541ef2265beebe7ee9e4e8764595c2a55ed67dc6d0a8ff69ccd3ff3228",
            SccpV1.LowerHex(domainHash));

        var key = SccpReplayV1.ReplayKey(domainHash, Repeated(0x11, 32));
        Assert.Equal(
            "035bcebe9423edd4f1b945bae54905e0f0860bcc54718d372b1a58797ce614d4",
            SccpV1.LowerHex(key));
        Assert.Equal(3, key[0]);

        var recordDigest = SccpReplayV1.RecordDigest(
            SccpReplayBoundaryV1.EvmDestinationMint,
            Repeated(0x11, 32),
            Repeated(0x22, 32),
            9,
            SccpReplayPrincipalV1.Evm(Repeated(0x33, 20)),
            Repeated(0x55, 32));
        Assert.Equal(
            "bb0a7e99f5d2d136375e46ba231903611366ea85ec0e10130488a085fa05bf4f",
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
        Assert.True(SccpReplayV1.RootFromWitness(key, new byte[32], emptyWitness).MatchesExpectedRoot);

        var occupiedRoot = SccpV1.DecodeLowerHex(
            "ec10fe878a6429557c7af279b8cb6fa5cc51165f4e6a54fb27ed6ad8525caf91");
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
            SccpReplayV1.RootFromWitness(key, new byte[32], reservedWitness));

        var explicitDefaultBitmap = new byte[32];
        explicitDefaultBitmap[31] = 1;
        var explicitDefaultWitness = new SccpSparseMerkleWitnessV1(
            empty[SccpReplayV1.Depth],
            new byte[32],
            explicitDefaultBitmap,
            [empty[0]]);
        Assert.Throws<ArgumentException>(() =>
            SccpReplayV1.RootFromWitness(key, new byte[32], explicitDefaultWitness));

        Assert.True(SccpReplayV1.VerifyAgainstCurrentRoot(
            new byte[32],
            new byte[32],
            emptyWitness,
            empty[SccpReplayV1.Depth]).MatchesExpectedRoot);
        var zeroExpectedWitness = new SccpSparseMerkleWitnessV1(
            new byte[32], new byte[32], new byte[32], []);
        Assert.False(SccpReplayV1.RootFromWitness(
            new byte[32], new byte[32], zeroExpectedWitness).MatchesExpectedRoot);
        Assert.Throws<ArgumentException>(() => SccpReplayV1.VerifyAgainstCurrentRoot(
            new byte[32], new byte[32], zeroExpectedWitness, new byte[32]));

        var zeroSiblingBitmap = new byte[32];
        zeroSiblingBitmap[31] = 1;
        var zeroSiblingWitness = new SccpSparseMerkleWitnessV1(
            empty[SccpReplayV1.Depth],
            new byte[32],
            zeroSiblingBitmap,
            [new byte[32]]);
        var zeroSiblingRoot = SccpReplayV1.RootFromWitness(
            new byte[32], new byte[32], zeroSiblingWitness).Root;
        var boundZeroSiblingWitness = new SccpSparseMerkleWitnessV1(
            zeroSiblingRoot,
            new byte[32],
            zeroSiblingBitmap,
            [new byte[32]]);
        Assert.True(SccpReplayV1.VerifyAgainstCurrentRoot(
            new byte[32], new byte[32], boundZeroSiblingWitness, zeroSiblingRoot).MatchesExpectedRoot);
        Assert.Throws<ArgumentException>(() => SccpReplayV1.VerifyAgainstCurrentRoot(
            new byte[32], new byte[32], boundZeroSiblingWitness, Repeated(0x77, 32)));
    }

    [Fact]
    public void ReplayOperationsBindPrincipalKindsDirectionsAndDefinedEnums()
    {
        Assert.Equal(0x35, (byte)SccpReplayBoundaryV1.TonWalletBurnAuthorization);
        Assert.Equal(0x36, (byte)SccpReplayBoundaryV1.TonWalletBurnLock);
        Assert.Equal(0x37, (byte)SccpReplayBoundaryV1.TonWalletBurnRefund);

        var tonActor = SccpReplayActorV1.Ton(0, Repeated(0x66, 32));
        foreach (var operation in new[]
        {
            SccpReplayBoundaryV1.TonBridgeInboundMint,
            SccpReplayBoundaryV1.TonMasterMint,
            SccpReplayBoundaryV1.TonWalletMintCredit,
        })
        {
            _ = SccpReplayV1.DomainHash(
                SccpNetworkV1.SoraTaira,
                SccpNetworkV1.TonMainnet,
                operation,
                7,
                Repeated(0x44, 32),
                tonActor);
        }
        foreach (var operation in new[]
        {
            SccpReplayBoundaryV1.TonBridgeOutboundBurn,
            SccpReplayBoundaryV1.TonMasterBurn,
            SccpReplayBoundaryV1.TonWalletBurnAuthorization,
            SccpReplayBoundaryV1.TonWalletBurnLock,
            SccpReplayBoundaryV1.TonWalletBurnRefund,
        })
        {
            _ = SccpReplayV1.DomainHash(
                SccpNetworkV1.TonMainnet,
                SccpNetworkV1.SoraTaira,
                operation,
                7,
                Repeated(0x44, 32),
                tonActor);
            Assert.Throws<ArgumentException>(() => SccpReplayV1.DomainHash(
                SccpNetworkV1.SoraTaira,
                SccpNetworkV1.TonMainnet,
                operation,
                7,
                Repeated(0x44, 32),
                tonActor));
        }

        Assert.Throws<ArgumentException>(() => SccpReplayV1.RecordDigest(
            SccpReplayBoundaryV1.SoraOutboundLock,
            Repeated(0x11, 32),
            Repeated(0x22, 32),
            9,
            SccpReplayPrincipalV1.Evm(Repeated(0x33, 20)),
            Repeated(0x55, 32)));
        Assert.Throws<ArgumentException>(() => SccpReplayV1.DomainHash(
            SccpNetworkV1.SoraTaira,
            SccpNetworkV1.EthereumMainnet,
            (SccpReplayBoundaryV1)0xff,
            7,
            Repeated(0x44, 32),
            SccpReplayActorV1.Route()));
        Assert.Throws<ArgumentOutOfRangeException>(() => SccpReplayV1.RecordDigest(
            (SccpReplayBoundaryV1)0xff,
            Repeated(0x11, 32),
            Repeated(0x22, 32),
            9,
            SccpReplayPrincipalV1.Evm(Repeated(0x33, 20)),
            Repeated(0x55, 32)));
    }

    [Fact]
    public void SoraReplayPrincipalRequiresCanonicalCompactNoritoBytes()
    {
        const string account =
            "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
        var canonical = new TransactionEncodingContext(account).EncodeAccountId(account);
        _ = SccpReplayPrincipalV1.SoraAccount(canonical);
        Assert.Throws<ArgumentException>(() =>
            SccpReplayPrincipalV1.SoraAccount([.. canonical, 0]));
        var overlongLength = new byte[canonical.Length + 1];
        canonical.AsSpan(0, 4).CopyTo(overlongLength);
        overlongLength[4] = (byte)(canonical[4] | 0x80);
        overlongLength[5] = 0;
        canonical.AsSpan(5).CopyTo(overlongLength.AsSpan(6));
        Assert.Throws<ArgumentException>(() =>
            SccpReplayPrincipalV1.SoraAccount(overlongLength));

        var compactEd25519 = new byte[33];
        compactEd25519[0] = 0;
        compactEd25519.AsSpan(1).Fill(0x33);
        _ = SccpReplayPrincipalV1.SoraAccount(
            EncodeMultisigAccountId(compactEd25519));

        Assert.Throws<ArgumentException>(() =>
            SccpReplayPrincipalV1.SoraAccount(
                EncodeSingleAccountId([0, .. Repeated(0x33, 31)])));
        Assert.Throws<ArgumentException>(() =>
            SccpReplayPrincipalV1.SoraAccount(
                EncodeSingleAccountId([1, 0x04, .. Repeated(0x33, 32)])));
        Assert.Throws<ArgumentException>(() =>
            SccpReplayPrincipalV1.SoraAccount(
                EncodeMultisigAccountId([0, .. new byte[32]])));
    }

    private static byte[] EncodeSingleAccountId(byte[] compactPublicKey)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteUInt32LittleEndian(0);
        writer.WriteField(EncodeCompactPublicKey(compactPublicKey));
        return writer.ToArray();
    }

    private static byte[] EncodeMultisigAccountId(byte[] compactPublicKey)
    {
        var member = new CanonicalNoritoWriter();
        member.WriteField(EncodeCompactPublicKey(compactPublicKey));
        var weight = new CanonicalNoritoWriter();
        weight.WriteUInt16LittleEndian(1);
        member.WriteField(weight.ToArray());

        var policy = new CanonicalNoritoWriter();
        policy.WriteField([1]);
        var threshold = new CanonicalNoritoWriter();
        threshold.WriteUInt16LittleEndian(1);
        policy.WriteField(threshold.ToArray());
        var members = new CanonicalNoritoWriter();
        members.WriteSequenceLength(1);
        members.WriteField(member.ToArray());
        policy.WriteField(members.ToArray());

        var writer = new CanonicalNoritoWriter();
        writer.WriteUInt32LittleEndian(1);
        writer.WriteField(policy.ToArray());
        return writer.ToArray();
    }

    private static byte[] EncodeCompactPublicKey(byte[] compactPublicKey)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteSequenceLength(checked((ulong)compactPublicKey.Length));
        writer.WriteByteElements(compactPublicKey);
        return writer.ToArray();
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
