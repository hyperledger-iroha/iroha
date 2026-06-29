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
        foreach (var paddedExpectedKey in new[]
        {
            " " + binding.Key,
            binding.Key + " ",
            binding.Key + "\n",
        })
        {
            Assert.Throws<ArgumentException>(
                () => BscTestnetSccp.DestinationBinding(
                    "0x" + new string('1', 40),
                    "0x" + new string('2', 40),
                    "0x" + new string('b', 64),
                    "0x" + new string('c', 64),
                    expectedKey: paddedExpectedKey));
        }
    }

    [Fact]
    public void LocalAdmissionSubmissionWrapsNativeBscTestnetOutput()
    {
        var proofBytes = new byte[] { 1, 2, 3 };
        var publicInputsBytes = new byte[] { 4, 5, 6 };
        var bundleBytes = new byte[] { 7, 8, 9 };
        var envelopeBytes = new byte[] { 10, 11, 12 };
        var input = new BscTestnetLocalAdmissionSubmissionInput(
            ProofBytes: proofBytes,
            PublicInputsBytes: publicInputsBytes,
            BundleBytes: bundleBytes,
            EnvelopeBytes: envelopeBytes,
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

        proofBytes[0] = 99;
        publicInputsBytes[0] = 99;
        bundleBytes[0] = 99;
        envelopeBytes[0] = 99;
        input.ProofBytes[0] = 98;
        input.PublicInputsBytes[0] = 98;
        input.BundleBytes[0] = 98;
        input.EnvelopeBytes[0] = 98;
        submission.ProofBytes[0] = 97;
        submission.PublicInputsBytes[0] = 97;
        submission.BundleBytes[0] = 97;
        submission.EnvelopeBytes[0] = 97;
        submission.LocalAdmission.ProofBytes[0] = 96;
        submission.LocalAdmission.PublicInputsBytes[0] = 96;
        submission.LocalAdmission.BundleBytes[0] = 96;
        Assert.Equal([1, 2, 3], input.ProofBytes);
        Assert.Equal([4, 5, 6], input.PublicInputsBytes);
        Assert.Equal([7, 8, 9], input.BundleBytes);
        Assert.Equal([10, 11, 12], input.EnvelopeBytes);
        Assert.Equal([1, 2, 3], submission.ProofBytes);
        Assert.Equal([4, 5, 6], submission.PublicInputsBytes);
        Assert.Equal([7, 8, 9], submission.BundleBytes);
        Assert.Equal([10, 11, 12], submission.EnvelopeBytes);
        Assert.Equal([1, 2, 3], submission.LocalAdmission.ProofBytes);
        Assert.Equal([4, 5, 6], submission.LocalAdmission.PublicInputsBytes);
        Assert.Equal([7, 8, 9], submission.LocalAdmission.BundleBytes);

        var updatedPayload = submission.LocalAdmission with
        {
            ProofBytes = [0xaa],
            PublicInputsBytes = [0xbb, 0xcc],
            BundleBytes = [0xdd, 0xee, 0xff],
        };
        var updatedSubmission = submission with
        {
            ProofBytes = [0x11],
            PublicInputsBytes = [0x22, 0x33],
            BundleBytes = [0x44, 0x55, 0x66],
            EnvelopeBytes = [0x77, 0x88],
        };
        Assert.Equal("0xaa", updatedPayload.ProofBytesHex);
        Assert.Equal("0xbbcc", updatedPayload.PublicInputsBytesHex);
        Assert.Equal("0xddeeff", updatedPayload.BundleBytesHex);
        Assert.Equal("0x11", updatedSubmission.ProofBytesHex);
        Assert.Equal("0x2233", updatedSubmission.PublicInputsBytesHex);
        Assert.Equal("0x445566", updatedSubmission.BundleBytesHex);
        Assert.Equal("0x7788", updatedSubmission.EnvelopeHex);

        Assert.Throws<ArgumentNullException>(
            () => new BscTestnetLocalAdmissionSubmissionInput(
                ProofBytes: null!,
                PublicInputsBytes: publicInputsBytes,
                BundleBytes: bundleBytes,
                EnvelopeBytes: envelopeBytes,
                StatementHash: input.StatementHash,
                SourceVerifierMaterialHash: input.SourceVerifierMaterialHash,
                SourceAdapterEngineDeploymentHash: input.SourceAdapterEngineDeploymentHash));
        Assert.Throws<ArgumentNullException>(() => input with { EnvelopeBytes = null! });
        Assert.Throws<ArgumentNullException>(
            () => new BscTestnetLocalAdmissionPayload(
                ProofBytes: null!,
                PublicInputsBytes: publicInputsBytes,
                BundleBytes: bundleBytes,
                StatementHash: input.StatementHash,
                SourceVerifierMaterialHash: input.SourceVerifierMaterialHash,
                SourceAdapterEngineDeploymentHash: input.SourceAdapterEngineDeploymentHash));
        Assert.Throws<ArgumentNullException>(() => updatedPayload with { BundleBytes = null! });
        Assert.Throws<ArgumentNullException>(
            () => new BscTestnetLocalAdmissionSubmission(
                submission.ProofFamily,
                submission.VerifierBackend,
                submission.SourceDomain,
                submission.TargetDomain,
                submission.StatementHash,
                submission.SourceVerifierMaterialHash,
                submission.SourceAdapterEngineDeploymentHash,
                submission.LocalAdmission,
                ProofBytes: null!,
                PublicInputsBytes: publicInputsBytes,
                BundleBytes: bundleBytes,
                EnvelopeBytes: envelopeBytes));
        Assert.Throws<ArgumentNullException>(() => updatedSubmission with { EnvelopeBytes = null! });

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
