using Hyperledger.Iroha.Sccp;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class SccpBscTestnetTests
{
    [Fact]
    public void TestnetGuardsAcceptBscAndRejectOtherRoutes()
    {
        BscTestnetSccp.RequireTestnetChainId(97);
        BscTestnetSccp.RequireTestnetNetworkId(BscTestnetSccp.TestnetNetworkId);
        BscTestnetSccp.RequireInboundRoute(
            BscTestnetSccp.DomainBsc,
            BscTestnetSccp.DomainSora);
        BscTestnetSccp.RequireOutboundRoute(
            BscTestnetSccp.DomainSora,
            BscTestnetSccp.DomainBsc);

        var binding = BscTestnetSccp.DestinationBinding(
            "0x" + new string('1', 40),
            "0x" + new string('2', 40),
            "0x" + new string('b', 64),
            "0x" + new string('c', 64));
        Assert.Equal(1, binding.Version);
        Assert.Equal(BscTestnetSccp.DomainSora, binding.SourceDomain);
        Assert.Equal(BscTestnetSccp.DomainBsc, binding.TargetDomain);
        Assert.Equal(BscTestnetSccp.TestnetNetworkId, binding.NetworkId);
        Assert.Equal(BscTestnetSccp.EvmGroth16Bn254ProofBackend, binding.VerifierBackend);
        Assert.Equal(BscTestnetSccp.StarkFriProofFamily, binding.ProofFamily);
        Assert.Equal(
            "evm:0:2:0000000000000000000000000000000000000000000000000000000000000061:"
                + "0x1111111111111111111111111111111111111111:"
                + "0x2222222222222222222222222222222222222222:"
                + "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb:"
                + "0xcccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            binding.Key);
        Assert.Equal(
            "0x16eb6817844e492f8fea4fc4742b9e464a80ae392f25d5e6fad9960d49414dcc",
            binding.BindingHash);
        Assert.Equal(
            binding.BindingHash,
            BscTestnetSccp.DestinationBindingHash(
                "0x" + new string('1', 40),
                "0x" + new string('2', 40),
                "0x" + new string('b', 64),
                "0x" + new string('c', 64)));
        Assert.Equal(
            binding.BindingHash,
            BscTestnetSccp.DestinationBinding(
                "0X" + new string('1', 40).ToUpperInvariant(),
                "0X" + new string('2', 40).ToUpperInvariant(),
                "0x" + new string('b', 64),
                "0x" + new string('c', 64),
                expectedBindingHash: binding.BindingHash,
                expectedKey: binding.Key).BindingHash);
        Assert.Throws<ArgumentException>(
            () => BscTestnetSccp.DestinationBinding(
                "0x" + new string('1', 40),
                "0x" + new string('2', 40),
                "0X" + new string('b', 64).ToUpperInvariant(),
                "0x" + new string('c', 64)));
        Assert.Throws<ArgumentException>(
            () => BscTestnetSccp.DestinationBinding(
                "0x" + new string('1', 40),
                "0x" + new string('2', 40),
                "0x" + new string('b', 64),
                "0X" + new string('c', 64).ToUpperInvariant()));

        Assert.Throws<ArgumentOutOfRangeException>(
            () => BscTestnetSccp.RequireTestnetChainId(56));
        Assert.Throws<ArgumentException>(
            () => BscTestnetSccp.RequireTestnetNetworkId("0x61"));
        Assert.Throws<ArgumentException>(
            () => BscTestnetSccp.RequireTestnetNetworkId(BscMainnetSccp.MainnetNetworkId));
        Assert.Throws<ArgumentException>(() => BscTestnetSccp.RequireInboundRoute(1, 0));
        Assert.Throws<ArgumentException>(() => BscTestnetSccp.RequireOutboundRoute(0, 1));
        Assert.Throws<ArgumentException>(
            () => BscTestnetSccp.DestinationBinding(
                "0x" + new string('1', 40),
                "0x" + new string('2', 40),
                "0x" + new string('b', 64),
                "0x" + new string('c', 64),
                networkId: BscMainnetSccp.MainnetNetworkId));
        Assert.Throws<ArgumentException>(
            () => BscTestnetSccp.DestinationBinding(
                "0x" + new string('1', 40),
                "0x" + new string('1', 40),
                "0x" + new string('b', 64),
                "0x" + new string('c', 64)));
        Assert.Throws<ArgumentException>(
            () => BscTestnetSccp.DestinationBinding(
                "0x" + new string('0', 40),
                "0x" + new string('2', 40),
                "0x" + new string('b', 64),
                "0x" + new string('c', 64)));
        Assert.Throws<ArgumentException>(
            () => BscTestnetSccp.DestinationBinding(
                "0x" + new string('1', 40),
                "0x" + new string('2', 40),
                "0x" + new string('b', 64),
                "0x" + new string('c', 64),
                targetDomain: 1));
        Assert.Throws<ArgumentException>(
            () => BscTestnetSccp.DestinationBinding(
                "0x" + new string('1', 40),
                "0x" + new string('2', 40),
                "0x" + new string('b', 64),
                "0x" + new string('c', 64),
                expectedBindingHash: "0x" + new string('9', 64)));
        Assert.Throws<ArgumentException>(
            () => BscTestnetSccp.DestinationBinding(
                "0x" + new string('1', 40),
                "0x" + new string('2', 40),
                "0x" + new string('b', 64),
                "0x" + new string('c', 64),
                expectedKey: binding.Key + "-wrong"));
    }

    [Fact]
    public void LocalAdmissionSubmissionWrapsNativeBscTestnetOutput()
    {
        var input = new BscTestnetLocalAdmissionSubmissionInput(
            ProofBytes: [1, 2, 3],
            PublicInputsBytes: [4, 5, 6],
            BundleBytes: [7, 8, 9],
            EnvelopeBytes: [10, 11, 12],
            StatementHash: "0x" + new string('6', 64),
            SourceVerifierMaterialHash: "0x" + new string('7', 64),
            SourceAdapterEngineDeploymentHash: "0x" + new string('8', 64));
        var submission = BscTestnetSccp.BuildLocalAdmissionSubmission(input);

        Assert.Equal(BscTestnetSccp.LocalAdmissionSubmissionKind, submission.PlatformPayload);
        Assert.Equal(BscTestnetSccp.LocalAdmissionEnvelopeEncoding, submission.EnvelopeEncoding);
        Assert.Equal(BscTestnetSccp.LocalAdmissionEntrypoint, submission.VerifierEntrypoint);
        Assert.Equal(BscTestnetSccp.DomainBsc, submission.SourceDomain);
        Assert.Equal(BscTestnetSccp.DomainSora, submission.TargetDomain);
        Assert.Empty(submission.Arguments);
        Assert.Equal([1, 2, 3], submission.ProofBytes);
        Assert.Equal([4, 5, 6], submission.PublicInputsBytes);
        Assert.Equal([7, 8, 9], submission.BundleBytes);
        Assert.Equal([10, 11, 12], submission.EnvelopeBytes);
        Assert.Equal([1, 2, 3], submission.LocalAdmission.ProofBytes);
        Assert.Equal("0x0a0b0c", submission.EnvelopeHex);

        input.ProofBytes[0] = 99;
        Assert.Equal([1, 2, 3], submission.ProofBytes);

        Assert.Throws<ArgumentException>(
            () => BscTestnetSccp.BuildLocalAdmissionSubmission(input with
            {
                SourceDomain = EthereumMainnetSccp.DomainEthereum,
            }));
        Assert.Throws<ArgumentException>(
            () => BscTestnetSccp.BuildLocalAdmissionSubmission(input with
            {
                ProofBytes = [0, 0],
            }));
        Assert.Throws<ArgumentException>(
            () => BscTestnetSccp.BuildLocalAdmissionSubmission(input with
            {
                EnvelopeBytes = [],
            }));
        Assert.Throws<ArgumentException>(
            () => BscTestnetSccp.BuildLocalAdmissionSubmission(input with
            {
                EnvelopeEncoding = "abi_tuple_v1",
            }));
        Assert.Throws<ArgumentException>(
            () => BscTestnetSccp.BuildLocalAdmissionSubmission(input with
            {
                ProofFamily = "debug-proof-family",
            }));
    }
}
