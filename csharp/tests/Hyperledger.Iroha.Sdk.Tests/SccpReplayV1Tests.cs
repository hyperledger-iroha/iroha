using Hyperledger.Iroha.Sccp;

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
            "4ed1f7c9f024bc628c66656b6314a1f45677c68487f78cdfb636eed6c9b51985",
            SccpV1.LowerHex(domainHash));

        var key = SccpReplayV1.ReplayKey(domainHash, Repeated(0x11, 32));
        Assert.Equal(
            "7aff291d1bad14cd1349ba4d73609de9e42f6fe4df4ce509952a2a8352b33582",
            SccpV1.LowerHex(key));
        Assert.Equal(122, key[0]);

        var recordDigest = SccpReplayV1.RecordDigest(
            SccpReplayBoundaryV1.SoraOutboundLock,
            Repeated(0x11, 32),
            Repeated(0x22, 32),
            9,
            SccpReplayPrincipalV1.Evm(Repeated(0x33, 20)),
            Repeated(0x55, 32));
        Assert.Equal(
            "35ab8613a0be06397609861d3cb3383770948b24b1cf098f4006c232240a2c07",
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
            "b19ed784f9998252402594bab82e0256c27fbaa0a50c9fe95c6f6c7457076a77");
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

    private static byte[] Repeated(byte value, int count) =>
        Enumerable.Repeat(value, count).ToArray();
}
