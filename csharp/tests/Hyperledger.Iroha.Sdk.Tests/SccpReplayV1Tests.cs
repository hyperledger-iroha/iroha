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

    private static byte[] Repeated(byte value, int count) =>
        Enumerable.Repeat(value, count).ToArray();
}
