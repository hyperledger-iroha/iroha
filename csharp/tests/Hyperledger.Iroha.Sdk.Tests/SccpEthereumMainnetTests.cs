using System.Buffers.Binary;
using System.IO;
using System.Net;
using System.Net.Http;
using System.Security.Cryptography;
using System.Text;
using Hyperledger.Iroha.Sccp;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class SccpEthereumMainnetTests
{
    private const string ExpectedBindingHash =
        "0xc86f9d904df50c4522d01da3773916ebecce816f3fdfa664e2dff7cfbe697c45";
    private const string ExpectedRequestHash =
        "0xa1b5d005fcf7e8ba427e6423a061edd1106f5b11a0be4a01832ddfa1dce9347d";
    private const string ExpectedEnvelopeHash =
        "0x5ca5c9b4a45dc0de3a22ad16e1fc0ac7aebdcdb4d8abfee8109f8fd97fe99fe4";
    private const string ExpectedSourceBridgeConfigHash =
        "0x871a910500648c68576f7d8fb044de1c494ae24c74f435c87dd451e6ae169c6b";
    private const string ExpectedSourceAdapterVerifierVkHash =
        "0x2140903293411cad0f0eb217d8beb18d3a188edf7bba455098589a2409445e46";
    private const string ExpectedSourceVerifierMaterialHash =
        "0x4d1e9d15bc59c0a2157aa967eb033f5778c805aea4707785a31ef6b60f694d77";
    private const string ExpectedSourceAdapterEngineDeploymentHash =
        "0xfeb62925410b1376a2cd3704c3822e335da96c3dcc283b041a559d7b08ab1cc4";
    private static readonly string ExpectedSyncCommitteeRoot =
        EthereumMainnetSccp.EthSyncCommitteeHashFromPayload(SampleSyncCommitteePayload());
    private static readonly string EthereumSyncCommitteeSupermajorityBits =
        "0x" + string.Concat(Enumerable.Repeat("ff", 42)) + "3f" + string.Concat(Enumerable.Repeat("00", 21));
    private const string EthereumSyncCommitteeSupermajorityParticipation = "342";
    private static readonly string[] EthereumFinalityBranch =
        Enumerable.Range(0, 6)
            .Select(index => "0x" + string.Concat(Enumerable.Repeat((0x50 + index).ToString("x2"), 32)))
            .ToArray();
    private const string BeaconHeaderRootSlot64 =
        "0xbb44a971e8c280f585ba430bfabfe87d9c59adf38bf9f77266b69687a148048c";
    private const string ExpectedPublicInputsBytes =
        "01afc78cca06ba8f5ed66573680617a9e1c7ce2c139f65296e24e13a8e310d2619"
        + "e5cd179754797d2dc49089ea1b4a847c185139c631da4b1fbbda224d446fd399"
        + "01000000"
        + "c67f32d1a81ecd49e1b921a8696853ab6bb92bf171fb595e5041199802f1a65f"
        + "2a00000000000000"
        + "5555555555555555555555555555555555555555555555555555555555555555";
    private const string ExpectedCallDataHex =
        "0xbd57826c0000000000000000000000000000000000000000000000000000000000000100"
        + "afc78cca06ba8f5ed66573680617a9e1c7ce2c139f65296e24e13a8e310d2619"
        + "e5cd179754797d2dc49089ea1b4a847c185139c631da4b1fbbda224d446fd399"
        + "0000000000000000000000000000000000000000000000000000000000000001"
        + "c67f32d1a81ecd49e1b921a8696853ab6bb92bf171fb595e5041199802f1a65f"
        + "000000000000000000000000000000000000000000000000000000000000002a"
        + "5555555555555555555555555555555555555555555555555555555555555555"
        + "5555555555555555555555555555555555555555555555555555555555555555"
        + "0000000000000000000000000000000000000000000000000000000000000180"
        + "0000000000000000000000000000000000000000000000000000000000000001"
        + "afc78cca06ba8f5ed66573680617a9e1c7ce2c139f65296e24e13a8e310d2619"
        + "0000000000000000000000000000000000000000000000000000000000000000"
        + "c67f32d1a81ecd49e1b921a8696853ab6bb92bf171fb595e5041199802f1a65f"
        + "0000000000000000000000000000000000000000000000000000000000000001"
        + "0000000000000000000000000000000000000000000000000000000000000002"
        + "1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed"
        + "198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2"
        + "12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa"
        + "090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b"
        + "0000000000000000000000000000000000000000000000000000000000000001"
        + "0000000000000000000000000000000000000000000000000000000000000002";
    private const string ExpectedReceiptProofBytes =
        "0101000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
        + "20000000000000003412000000000000"
        + "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        + "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
        + "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
        + "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        + "030000000000000002000000010000000102000000020301000000"
        + "1111111111111111111111111111111111111111111111111111111111111111";
    private const string ExpectedReceiptProofHash =
        "0x39f014e3f5f8d38b44d59f1afdf72ceb71d10d6d937f268c404b046f092b38f0";
    private const string SampleOutboundMessageId =
        "0xafc78cca06ba8f5ed66573680617a9e1c7ce2c139f65296e24e13a8e310d2619";
    private const string SampleOutboundPayloadHash =
        "0xe5cd179754797d2dc49089ea1b4a847c185139c631da4b1fbbda224d446fd399";
    private const string SampleOutboundCommitmentRoot =
        "0xc67f32d1a81ecd49e1b921a8696853ab6bb92bf171fb595e5041199802f1a65f";
    private const string SampleOutboundFinalityBlockHash =
        "0x5555555555555555555555555555555555555555555555555555555555555555";
    private const string SampleOutboundBundleHex =
        "01c67f32d1a81ecd49e1b921a8696853ab6bb92bf171fb595e5041199802f1a6"
        + "5f46000000010601000000afc78cca06ba8f5ed66573680617a9e1c7ce2c139f"
        + "65296e24e13a8e310d2619e5cd179754797d2dc49089ea1b4a847c185139c631"
        + "da4b1fbbda224d446fd39904000000000000008e000000020100000000010000"
        + "000100000000000000000000000103000000786f72e803000000000000000000"
        + "0000000000010a000000616c69636540736f7261022a00000030783131313131"
        + "3131313131313131313131313131313131313131313131313131313131313131"
        + "313131011d000000736363702d6574682d6d61696e6e65742d786f722d726f75"
        + "74652d763103000000010203";
    private const string NonSoraProofBundleMessageId =
        "0xf9e8853047d4f99212c8c18fc11a5d1f46fb94e74da9230e4f8e2303eebbfcb6";
    private const string NonSoraProofBundlePayloadHash =
        "0x8f8ccf13aa4cbed43aa51d086ad6637e5ba145b138b4c8a75e3411aaf43cd396";
    private const string NonSoraProofBundleCommitmentRoot =
        "0xada0e465d843f2b3659950e244235642029342bb2dd56b637ba94fc533af3cb3";
    private const string NonSoraProofBundleFinalityHex =
        "4e52543000007a27db10248ac178129ff7397f9a1ce70051010000000000000d3b4d2ca6"
        + "61e9400201010401000000040200000004036574680401000000040000000020f9e88530"
        + "47d4f99212c8c18fc11a5d1f46fb94e74da9230e4f8e2303eebbfcb6208f8ccf13aa4cbe"
        + "d43aa51d086ad6637e5ba145b138b4c8a75e3411aaf43cd39620d0013a9d1d77df90587f"
        + "2281ba76360e51188d2cd811408edb81e377c5f67db920ada0e465d843f2b3659950e244"
        + "235642029342bb2dd56b637ba94fc533af3cb3082a000000000000002055555555555555"
        + "555555555555555555555555555555555555555555555555552011111111111111111111"
        + "111111111111111111111111111111111111111111112022222222222222222222222222"
        + "222222222222222222222222222222222222220901000000000000000109010000000000"
        + "000002310100000000000000282000000000000000333333333333333333333333333333"
        + "3333333333333333333333333333333333";
    private const string NonSoraProofBundleHex =
        "01ada0e465d843f2b3659950e244235642029342bb2dd56b637ba94fc533af3cb3460000"
        + "00010602000000f9e8853047d4f99212c8c18fc11a5d1f46fb94e74da9230e4f8e2303ee"
        + "bbfcb68f8ccf13aa4cbed43aa51d086ad6637e5ba145b138b4c8a75e3411aaf43cd39604"
        + "00000000000000b800000002010100000002000000070000000000000000000000062000"
        + "0000aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa15cd"
        + "5b07000000000000000000000000022a0000003078313131313131313131313131313131"
        + "31313131313131313131313131313131313131313131313131022a000000307832323232"
        + "323232323232323232323232323232323232323232323232323232323232323232323232"
        + "010a000000726f7574652d6d61696e790100004e52543000007a27db10248ac178129ff7"
        + "397f9a1ce70051010000000000000d3b4d2ca661e9400201010401000000040200000004"
        + "036574680401000000040000000020f9e8853047d4f99212c8c18fc11a5d1f46fb94e74d"
        + "a9230e4f8e2303eebbfcb6208f8ccf13aa4cbed43aa51d086ad6637e5ba145b138b4c8a7"
        + "5e3411aaf43cd39620d0013a9d1d77df90587f2281ba76360e51188d2cd811408edb81e3"
        + "77c5f67db920ada0e465d843f2b3659950e244235642029342bb2dd56b637ba94fc533af"
        + "3cb3082a0000000000000020555555555555555555555555555555555555555555555555"
        + "555555555555555520111111111111111111111111111111111111111111111111111111"
        + "111111111120222222222222222222222222222222222222222222222222222222222222"
        + "222209010000000000000001090100000000000000023101000000000000002820000000"
        + "000000003333333333333333333333333333333333333333333333333333333333333333";

    private static readonly string[] ExpectedPublicSignalWords =
    [
        "0x117703cc9b95de6bd7a49c01b4d0b05154726e58c223e881e9135e5b9b82512b",
        "0x126d2e7b1a2abb57fc6c8617f84ab8d568365e3055c69a6fe5f15bb0051e3cb2",
        "0x2eb6b5dbab56255a979f433862429637ba1e8251106271606f0a279f593d7a39",
        "0x0e1ea63f789fc9c586b65f8de6b018e098463fed5d30a333043498f3dd5059b5",
        "0x220a98afe36b6d6828e7e852988c8595f0ad6d128e845e74e0161cb0fa2f642f",
        "0x22bb7c2a83959fc7daf7a788292fda0bc36b0ba60b32e9a2c26bcd41849677ef",
        "0x2697e4e42f34b673b4aa254c6a92de09304e84c1a667c7d266777775a231efb4",
        "0x16fbe0c1d659f142b3e7815b24df66da3cfd89cc42d051b04bc31aae6925c396",
        "0x02dcef873274cccbb6bde309daaabeec707adc38755c2d118518ecd716151da3",
    ];

    private static readonly string[] ExpectedPublicInputWords =
    [
        "0xafc78cca06ba8f5ed66573680617a9e1c7ce2c139f65296e24e13a8e310d2619",
        "0xe5cd179754797d2dc49089ea1b4a847c185139c631da4b1fbbda224d446fd399",
        "0x0000000000000000000000000000000000000000000000000000000000000001",
        "0xc67f32d1a81ecd49e1b921a8696853ab6bb92bf171fb595e5041199802f1a65f",
        "0x000000000000000000000000000000000000000000000000000000000000002a",
        "0x5555555555555555555555555555555555555555555555555555555555555555",
    ];

    private sealed class ExecutionProviderStub(
        object chainId,
        IReadOnlyDictionary<string, object?> receipt,
        IReadOnlyDictionary<string, object?> block,
        IReadOnlyList<IReadOnlyDictionary<string, object?>>? blockReceipts = null) : IEthereumMainnetExecutionProvider
    {
        public List<string> Calls { get; } = [];

        public ValueTask<object?> RequestAsync(
            string method,
            IReadOnlyList<object?> parameters,
            CancellationToken cancellationToken = default)
        {
            Calls.Add(method);
            return method switch
            {
                "eth_chainId" => ValueTask.FromResult<object?>(chainId),
                "eth_getTransactionReceipt" => ReceiptResult(parameters),
                "eth_getBlockByHash" => BlockResult(parameters),
                "eth_getBlockReceipts" => BlockReceiptsResult(parameters),
                _ => throw new ArgumentException($"unexpected method {method}", nameof(method)),
            };
        }

        private ValueTask<object?> ReceiptResult(IReadOnlyList<object?> parameters)
        {
            Assert.Single(parameters);
            Assert.Equal(receipt["transactionHash"], parameters[0]);
            return ValueTask.FromResult<object?>(receipt);
        }

        private ValueTask<object?> BlockResult(IReadOnlyList<object?> parameters)
        {
            Assert.Equal(2, parameters.Count);
            Assert.Equal(block["hash"], parameters[0]);
            Assert.False((bool)parameters[1]!);
            return ValueTask.FromResult<object?>(block);
        }

        private ValueTask<object?> BlockReceiptsResult(IReadOnlyList<object?> parameters)
        {
            Assert.Single(parameters);
            Assert.Equal(block["number"], parameters[0]);
            Assert.NotNull(blockReceipts);
            return ValueTask.FromResult<object?>(blockReceipts);
        }
    }

    private sealed class InboundProverStub(
        string expectedTransactionHash,
        string? expectedReceiptProofHash = null,
        string? expectedSourceEventDigest = null)
        : IEthereumMainnetInboundProver
    {
        public ValueTask<byte[]> ProveAsync(
            EthereumMainnetInboundEvidence evidence,
            CancellationToken cancellationToken = default)
        {
            Assert.Equal(EthereumMainnetSccp.DomainEthereum, evidence.SourceDomain);
            Assert.Equal(EthereumMainnetSccp.DomainSora, evidence.TargetDomain);
            Assert.Equal(expectedTransactionHash, evidence.TransactionHash);
            if (expectedReceiptProofHash is not null)
            {
                Assert.Equal(expectedReceiptProofHash, evidence.ReceiptProofHash);
                Assert.NotNull(evidence.ReceiptProof);
            }

            if (expectedSourceEventDigest is not null)
            {
                Assert.Equal(expectedSourceEventDigest, evidence.ReceiptProof?.SourceEventDigest);
                Assert.Equal(expectedSourceEventDigest, evidence.SourceEventDigest);
            }

            return ValueTask.FromResult(new byte[] { 1, 2, 3 });
        }
    }

    private sealed class DelegateInboundProver(Func<EthereumMainnetInboundEvidence, byte[]> prove)
        : IEthereumMainnetInboundProver
    {
        public ValueTask<byte[]> ProveAsync(
            EthereumMainnetInboundEvidence evidence,
            CancellationToken cancellationToken = default) =>
            ValueTask.FromResult(prove(evidence));
    }

    private sealed class ConsensusProviderStub(
        IReadOnlyDictionary<string, object?> expectedReceipt,
        IReadOnlyDictionary<string, object?> expectedBlock,
        string expectedTransactionHash,
        IReadOnlyDictionary<string, object?> finality) : IEthereumMainnetConsensusProvider
    {
        public int Calls { get; private set; }

        public ValueTask<IReadOnlyDictionary<string, object?>?> CollectFinalityEvidenceAsync(
            IReadOnlyDictionary<string, object?>? receipt,
            IReadOnlyDictionary<string, object?>? block,
            string? transactionHash,
            CancellationToken cancellationToken = default)
        {
            Calls++;
            Assert.NotSame(expectedReceipt, receipt);
            Assert.NotSame(expectedBlock, block);
            Assert.Equal(expectedReceipt["blockHash"], receipt?["blockHash"]);
            Assert.Equal(expectedReceipt["transactionHash"], receipt?["transactionHash"]);
            Assert.Equal(expectedBlock["hash"], block?["hash"]);
            Assert.Equal(expectedTransactionHash, transactionHash);
            return ValueTask.FromResult<IReadOnlyDictionary<string, object?>?>(finality);
        }
    }

    private sealed class DelegateConsensusProvider(
        Func<IReadOnlyDictionary<string, object?>?,
            IReadOnlyDictionary<string, object?>?,
            string?,
            IReadOnlyDictionary<string, object?>?> collect) : IEthereumMainnetConsensusProvider
    {
        public ValueTask<IReadOnlyDictionary<string, object?>?> CollectFinalityEvidenceAsync(
            IReadOnlyDictionary<string, object?>? receipt,
            IReadOnlyDictionary<string, object?>? block,
            string? transactionHash,
            CancellationToken cancellationToken = default) =>
            ValueTask.FromResult(collect(receipt, block, transactionHash));
    }

    private sealed class DelegateBscConsensusProvider(
        Func<IReadOnlyDictionary<string, object?>?,
            IReadOnlyDictionary<string, object?>?,
            string?,
            IReadOnlyDictionary<string, object?>> collect) : IBscMainnetConsensusProvider
    {
        public ValueTask<IReadOnlyDictionary<string, object?>> CollectFinalityEvidenceAsync(
            IReadOnlyDictionary<string, object?>? receipt,
            IReadOnlyDictionary<string, object?>? block,
            string? transactionHash,
            CancellationToken cancellationToken = default) =>
            ValueTask.FromResult(collect(receipt, block, transactionHash));
    }

    private sealed class BeaconRestTransportStub(
        Func<string, IReadOnlyDictionary<string, string>, EthereumMainnetBeaconRestResponse> handler)
        : IEthereumMainnetBeaconRestTransport
    {
        public List<string> Calls { get; } = [];

        public List<IReadOnlyDictionary<string, string>> HeaderCalls { get; } = [];

        public ValueTask<EthereumMainnetBeaconRestResponse> GetAsync(
            string url,
            IReadOnlyDictionary<string, string> headers,
            CancellationToken cancellationToken = default)
        {
            Calls.Add(url);
            HeaderCalls.Add(new Dictionary<string, string>(headers, StringComparer.Ordinal));
            return ValueTask.FromResult(handler(url, headers));
        }
    }

    private sealed class BeaconRestHttpHandlerStub(Func<HttpRequestMessage, HttpResponseMessage> handler)
        : HttpMessageHandler
    {
        protected override Task<HttpResponseMessage> SendAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken) =>
            Task.FromResult(handler(request));
    }

    private sealed class UnknownLengthContent(byte[] body) : HttpContent
    {
        protected override Task SerializeToStreamAsync(Stream stream, TransportContext? context) =>
            stream.WriteAsync(body, 0, body.Length);

        protected override Task<Stream> CreateContentReadStreamAsync() =>
            Task.FromResult<Stream>(new MemoryStream(body, writable: false));

        protected override bool TryComputeLength(out long length)
        {
            length = 0;
            return false;
        }
    }

    private sealed class InboundSubmitterStub : IEthereumMainnetInboundSubmitter
    {
        public ValueTask<object?> SubmitAsync(
            byte[] proofBytes,
            CancellationToken cancellationToken = default)
        {
            Assert.Equal(new byte[] { 1, 2, 3 }, proofBytes);
            return ValueTask.FromResult<object?>("submitted");
        }
    }

    private sealed class CountingInboundProver : IEthereumMainnetInboundProver
    {
        public int Calls { get; private set; }

        public ValueTask<byte[]> ProveAsync(
            EthereumMainnetInboundEvidence evidence,
            CancellationToken cancellationToken = default)
        {
            Calls++;
            return ValueTask.FromResult(new byte[] { 1, 2, 3 });
        }
    }

    private sealed class MutatingConsensusProviderStub(
        IReadOnlyDictionary<string, object?> expectedReceipt,
        IReadOnlyDictionary<string, object?> expectedBlock,
        IReadOnlyDictionary<string, object?> finality) : IEthereumMainnetConsensusProvider
    {
        public ValueTask<IReadOnlyDictionary<string, object?>?> CollectFinalityEvidenceAsync(
            IReadOnlyDictionary<string, object?>? receipt,
            IReadOnlyDictionary<string, object?>? block,
            string? transactionHash,
            CancellationToken cancellationToken = default)
        {
            Assert.NotSame(expectedReceipt, receipt);
            Assert.NotSame(expectedBlock, block);
            Assert.Equal(expectedReceipt["transactionHash"], receipt?["transactionHash"]);
            Assert.Equal(expectedBlock["hash"], block?["hash"]);
            Assert.True(receipt is IDictionary<string, object?>);
            Assert.True(block is IDictionary<string, object?>);
            ((IDictionary<string, object?>)receipt!)["status"] = "0x0";
            MutateNestedReceiptSnapshot(receipt);
            ((IDictionary<string, object?>)block!)["receiptsRoot"] = "0x" + new string('e', 64);
            return ValueTask.FromResult<IReadOnlyDictionary<string, object?>?>(finality);
        }
    }

    private sealed class MutatingInboundProverStub(
        IReadOnlyDictionary<string, object?> originalReceipt,
        IReadOnlyDictionary<string, object?> originalBlock,
        IReadOnlyDictionary<string, object?> originalFinality,
        string expectedTransactionHash) : IEthereumMainnetInboundProver
    {
        public ValueTask<byte[]> ProveAsync(
            EthereumMainnetInboundEvidence evidence,
            CancellationToken cancellationToken = default)
        {
            Assert.NotSame(originalReceipt, evidence.Receipt);
            Assert.NotSame(originalBlock, evidence.Block);
            Assert.NotSame(originalFinality, evidence.BeaconFinality);
            Assert.Equal(expectedTransactionHash, evidence.TransactionHash);
            Assert.True(evidence.Receipt is IDictionary<string, object?>);
            Assert.True(evidence.Block is IDictionary<string, object?>);
            Assert.True(evidence.BeaconFinality is IDictionary<string, object?>);
            ((IDictionary<string, object?>)evidence.Receipt!)["status"] = "0x0";
            MutateNestedReceiptSnapshot(evidence.Receipt);
            ((IDictionary<string, object?>)evidence.Block!)["receiptsRoot"] =
                "0x" + new string('e', 64);
            ((IDictionary<string, object?>)evidence.BeaconFinality!)["executionBlockHash"] =
                "0x" + new string('e', 64);
            return ValueTask.FromResult(new byte[] { 1, 2, 3 });
        }
    }

    private static void MutateNestedReceiptSnapshot(
        IReadOnlyDictionary<string, object?>? receipt)
    {
        var logs = Assert.IsType<object?[]>(receipt?["logs"]);
        var metadata = Assert.IsAssignableFrom<IDictionary<string, object?>>(logs[0]);
        var topics = Assert.IsType<object?[]>(metadata["topics"]);

        metadata["address"] = "0x" + new string('e', 40);
        topics[0] = "0x" + new string('e', 64);
    }

    private sealed class OutboundProverStub(
        byte[] proofBytes,
        Action<EthereumMainnetOutboundProofRequest>? onRequest = null,
        string expectedRequestHash = ExpectedRequestHash,
        string expectedBindingHash = ExpectedBindingHash,
        IReadOnlyList<string>? expectedPublicSignalWords = null) : IEthereumMainnetOutboundProver
    {
        public EthereumMainnetOutboundProofRequest? Request { get; private set; }

        public ValueTask<byte[]> ProveAsync(
            EthereumMainnetOutboundProofRequest request,
            CancellationToken cancellationToken = default)
        {
            Request = request;
            Assert.Equal(expectedRequestHash, request.RequestHash);
            Assert.Equal(expectedBindingHash, request.DestinationBindingHash);
            Assert.Equal(expectedPublicSignalWords ?? ExpectedPublicSignalWords, request.PublicSignalWords);
            onRequest?.Invoke(request);
            return ValueTask.FromResult(proofBytes);
        }
    }

    private sealed class NativeProverSelfTestStub(
        Func<
            EthereumMainnetNativeEvmProverSelfTestFixture,
            EthereumMainnetNativeEvmProverSelfTestSdkResult,
            EthereumMainnetNativeEvmProverArtifacts,
            EthereumMainnetNativeEvmProverSelfTestSdkResult>? run = null)
        : IEthereumMainnetNativeProverSelfTest
    {
        public bool Called { get; private set; }

        public ValueTask<EthereumMainnetNativeEvmProverSelfTestSdkResult> RunAsync(
            EthereumMainnetNativeEvmProverSelfTestFixture fixture,
            EthereumMainnetNativeEvmProverSelfTestSdkResult expectedResult,
            EthereumMainnetNativeEvmProverArtifacts artifacts,
            CancellationToken cancellationToken = default)
        {
            Called = true;
            Assert.Equal(expectedResult.ProofHash, fixture.ProofHash);
            return ValueTask.FromResult(run?.Invoke(fixture, expectedResult, artifacts) ?? expectedResult);
        }
    }

    private sealed class MutatingOutboundProverStub(byte[] proofBytes) : IEthereumMainnetOutboundProver
    {
        public EthereumMainnetOutboundProofRequest? Request { get; private set; }

        public ValueTask<byte[]> ProveAsync(
            EthereumMainnetOutboundProofRequest request,
            CancellationToken cancellationToken = default)
        {
            Request = request;
            request.BundleBytes[0] = (byte)(request.BundleBytes[0] ^ 0xff);
            if (request.SourceProofBytes.Length > 0)
            {
                request.SourceProofBytes[0] = (byte)(request.SourceProofBytes[0] ^ 0xff);
            }
            request.PublicInputsBytes[0] = (byte)(request.PublicInputsBytes[0] ^ 0xff);
            if (request.PublicSignalWords is string[] publicSignalWords)
            {
                publicSignalWords[0] = "0x" + new string('9', 64);
            }

            return ValueTask.FromResult(proofBytes);
        }
    }

    private sealed class OutboundSubmitterStub : IEthereumMainnetOutboundSubmitter
    {
        public EthereumMainnetSccpSubmission? Submission { get; private set; }

        public ValueTask<object?> SubmitAsync(
            EthereumMainnetSccpSubmission submission,
            CancellationToken cancellationToken = default)
        {
            Submission = submission;
            return ValueTask.FromResult<object?>("eth-submitted");
        }
    }

    private static EthereumMainnetSccpDestinationBinding SampleDestinationBinding()
        => EthereumMainnetSccp.DestinationBinding(
            "0x" + new string('1', 40),
            "0x" + new string('2', 40),
            "0x" + new string('b', 64),
            "0x" + new string('c', 64));

    private static EthereumMainnetTransparentPublicInputs SamplePublicInputs()
        => new(
            Version: 1,
            MessageId: SampleOutboundMessageId,
            PayloadHash: SampleOutboundPayloadHash,
            TargetDomain: EthereumMainnetSccp.DomainEthereum,
            CommitmentRoot: SampleOutboundCommitmentRoot,
            FinalityHeight: 42,
            FinalityBlockHash: SampleOutboundFinalityBlockHash);

    private static EthereumMainnetSourceVerifierMaterialInput SampleSourceMaterial()
        => new(
            SourceTrustAnchorHash: "0x" + new string('4', 64),
            ConsensusVerifierHash: "0x" + new string('5', 64),
            MessageInclusionVerifierHash: "0x" + new string('6', 64),
            FinalityPolicyHash: "0x" + new string('8', 64),
            BridgeAddress: "0x" + string.Concat(Enumerable.Repeat("11", 20)),
            SourceBridgeEmitterCodeHash: "0x" + new string('7', 64));

    private static EthereumMainnetSourceAdapterDeploymentInput SampleSourceAdapterDeployment()
    {
        var material = SampleSourceMaterial();
        return new EthereumMainnetSourceAdapterDeploymentInput(
            SourceTrustAnchorHash: material.SourceTrustAnchorHash,
            ConsensusVerifierHash: material.ConsensusVerifierHash,
            MessageInclusionVerifierHash: material.MessageInclusionVerifierHash,
            FinalityPolicyHash: material.FinalityPolicyHash,
            BridgeAddress: material.BridgeAddress,
            SourceBridgeEmitterCodeHash: material.SourceBridgeEmitterCodeHash,
            DeploymentReceiptHash: "0x" + new string('a', 64));
    }

    private static EthereumMainnetOutboundProofRequestInput SampleOutboundInput(
        EthereumMainnetSccpDestinationBinding? binding = null,
        EthereumMainnetTransparentPublicInputs? publicInputs = null)
        => new()
        {
            PublicInputs = publicInputs ?? SamplePublicInputs(),
            BundleBytes = SampleOutboundBundleBytes(),
            SourceProofBytes = [],
            StatementHash = "0x" + new string('5', 64),
            DestinationBinding = binding ?? SampleDestinationBinding(),
            DestinationBindingHash = (binding ?? SampleDestinationBinding()).BindingHash,
            SourceDomain = EthereumMainnetSccp.DomainSora,
        };

    private static EthereumMainnetNativeEvmProverBundle SampleNativeEvmProverBundle(
        string destinationBindingHash,
        bool noWasm = true,
        bool remoteProverRequired = false,
        string? expectedDestinationBindingHash = null)
    {
        var proofArtifactHash = "0x" + new string('9', 64);
        var provingKeyHash = "0x" + new string('a', 64);
        var artifacts = EthereumMainnetSccp.EthNativeEvmProverRequiredImplementationsV1
            .OrderBy(entry => entry.Key, StringComparer.Ordinal)
            .Select((entry, index) => new EthereumMainnetNativeEvmProverBundleSdkArtifact(
                entry.Key,
                entry.Value,
                proofArtifactHash,
                provingKeyHash,
                "0x" + string.Concat(Enumerable.Repeat((index + 1).ToString("x2"), 32)),
                implementationArtifact: $"artifacts/eth-mainnet/{entry.Key}-implementation.bin"))
            .ToArray();
        return new EthereumMainnetNativeEvmProverBundle(
            proofArtifactHash,
            provingKeyHash,
            "0x" + new string('c', 64),
            destinationBindingHash,
            artifacts,
            SampleNativeAuditHashes(),
            noWasm: noWasm,
            remoteProverRequired: remoteProverRequired,
            expectedDestinationBindingHash: expectedDestinationBindingHash,
            proofArtifact: "artifacts/eth-mainnet/proof-artifact.bin",
            provingKey: "artifacts/eth-mainnet/proving-key.bin",
            verifierKey: "artifacts/eth-mainnet/verifier-key.bin",
            crossSdkFixtureParityArtifact: "artifacts/eth-mainnet/cross-sdk-fixture-parity.json",
            nativeProverSelfTestArtifact: "artifacts/eth-mainnet/native-prover-self-test.json");
    }

    private static IReadOnlyDictionary<string, string> SampleNativeAuditHashes()
        => new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["circuit_security_audit"] = "0x" + new string('d', 64),
            ["native_implementation_audit"] = "0x" + new string('e', 64),
            ["reproducible_build_attestation"] = "0x" + new string('f', 64),
            ["cross_sdk_fixture_parity"] = "0x" + new string('1', 64),
            ["native_prover_self_test"] = "0x" + new string('2', 64),
            ["no_wasm_no_remote_scan"] = "0x" + new string('3', 64),
        };

    private static string Sha256Hex(byte[] value) =>
        "0x" + Convert.ToHexString(SHA256.HashData(value)).ToLowerInvariant();

    private static byte[] NativeEvmProverArtifactBytes(string label, int size = 64 * 1024)
    {
        var labelBytes = Encoding.UTF8.GetBytes(label);
        var bytes = new byte[size];
        for (var index = 0; index < bytes.Length; index++)
        {
            bytes[index] = (byte)((index * 37 + labelBytes.Length * 11) & 0xff);
        }

        Array.Copy(labelBytes, bytes, Math.Min(labelBytes.Length, bytes.Length));
        return bytes;
    }

    private static string SampleNativeEvmProverBundleJson(
        string destinationBindingHash,
        bool noWasm = true,
        bool remoteProverRequired = false,
        string proofArtifact = "artifacts/eth-mainnet/proof-artifact.bin")
    {
        var proofArtifactHash = "0x" + new string('9', 64);
        var provingKeyHash = "0x" + new string('a', 64);
        var artifacts = string.Join(
            ",",
            EthereumMainnetSccp.EthNativeEvmProverRequiredImplementationsV1
                .OrderBy(entry => entry.Key, StringComparer.Ordinal)
                .Select((entry, index) =>
                    $$"""
                    {
                      "sdk": "{{entry.Key}}",
                      "implementation": "{{entry.Value}}",
                      "prover_artifact_hash": "{{proofArtifactHash}}",
                      "proving_key_hash": "{{provingKeyHash}}",
                      "implementation_artifact": "artifacts/eth-mainnet/{{entry.Key}}-implementation.bin",
                      "implementation_hash": "0x{{string.Concat(Enumerable.Repeat((index + 1).ToString("x2"), 32))}}"
                    }
                    """));
        return $$"""
        {
          "schema": "{{EthereumMainnetSccp.NativeEvmProverBundleSchemaV1}}",
          "bundle_id": "{{EthereumMainnetSccp.EthNativeEvmProverBundleIdV1}}",
          "domain": {{EthereumMainnetSccp.DomainEthereum}},
          "chain": "eth",
          "proof_backend": "{{EthereumMainnetSccp.EvmGroth16Bn254ProofBackend}}",
          "proof_artifact": "{{proofArtifact}}",
          "proof_artifact_hash": "{{proofArtifactHash}}",
          "proving_key": "artifacts/eth-mainnet/proving-key.bin",
          "proving_key_hash": "{{provingKeyHash}}",
          "verifier_key": "artifacts/eth-mainnet/verifier-key.bin",
          "verifier_key_hash": "0x{{new string('c', 64)}}",
          "destination_binding_hash": "{{destinationBindingHash}}",
          "no_wasm": {{noWasm.ToString().ToLowerInvariant()}},
          "remote_prover_required": {{remoteProverRequired.ToString().ToLowerInvariant()}},
          "browser_implementation": "pure-typescript",
          "native_sdk_artifacts": [{{artifacts}}],
          "cross_sdk_fixture_parity_artifact": "artifacts/eth-mainnet/cross-sdk-fixture-parity.json",
          "native_prover_self_test_artifact": "artifacts/eth-mainnet/native-prover-self-test.json",
          "audit_hashes": {
            "circuit_security_audit": "0x{{new string('d', 64)}}",
            "native_implementation_audit": "0x{{new string('e', 64)}}",
            "reproducible_build_attestation": "0x{{new string('f', 64)}}",
            "cross_sdk_fixture_parity": "0x{{new string('1', 64)}}",
            "native_prover_self_test": "0x{{new string('2', 64)}}",
            "no_wasm_no_remote_scan": "0x{{new string('3', 64)}}"
          }
        }
        """;
    }

    private static string SampleNativeEvmProverParityFixtureJson(
        EthereumMainnetNativeEvmProverBundle nativeProverBundle,
        string? dotnetCalldataHash = null)
    {
        var defaultCalldataHash = "0x" + new string('3', 64);
        var publicSignalWords = string.Join(
            ",",
            Enumerable.Range(0, 9)
                .Select(index => $"\"0x{string.Concat(Enumerable.Repeat((index + 0x10).ToString("x2"), 32))}\""));
        var sdkResults = string.Join(
            ",",
            EthereumMainnetSccp.EthNativeEvmProverRequiredImplementationsV1
                .OrderBy(entry => entry.Key, StringComparer.Ordinal)
                .Select(entry =>
                {
                    var calldataHash = entry.Key == "dotnet"
                        ? dotnetCalldataHash ?? defaultCalldataHash
                        : defaultCalldataHash;
                    return $$"""
                    "{{entry.Key}}": {
                      "receipt_proof_hash": "0x{{new string('1', 64)}}",
                      "source_proof_hash": "0x{{new string('2', 64)}}",
                      "destination_binding_hash": "{{nativeProverBundle.DestinationBindingHash}}",
                      "public_signal_words": [{{publicSignalWords}}],
                      "calldata_hash": "{{calldataHash}}",
                      "torii_submit_payload_hash": "0x{{new string('4', 64)}}"
                    }
                    """;
                }));
        return $$"""
        {
          "schema": "{{EthereumMainnetSccp.EthNativeEvmProverParityFixtureSchemaV1}}",
          "domain": {{EthereumMainnetSccp.DomainEthereum}},
          "chain": "eth",
          "proof_backend": "{{EthereumMainnetSccp.EvmGroth16Bn254ProofBackend}}",
          "proof_artifact_hash": "{{nativeProverBundle.ProofArtifactHash}}",
          "proving_key_hash": "{{nativeProverBundle.ProvingKeyHash}}",
          "verifier_key_hash": "{{nativeProverBundle.VerifierKeyHash}}",
          "destination_binding_hash": "{{nativeProverBundle.DestinationBindingHash}}",
          "receipt_proof_hash": "0x{{new string('1', 64)}}",
          "source_proof_hash": "0x{{new string('2', 64)}}",
          "public_signal_words": [{{publicSignalWords}}],
          "calldata_hash": "{{defaultCalldataHash}}",
          "torii_submit_payload_hash": "0x{{new string('4', 64)}}",
          "sdk_results": {
            {{sdkResults}}
          }
        }
        """;
    }

    private static string SampleNativeEvmProverSelfTestFixtureJson(
        EthereumMainnetNativeEvmProverBundle nativeProverBundle,
        string? dotnetProofHash = null)
    {
        var defaultProofHash = "0x" + new string('8', 64);
        var publicSignalWords = string.Join(
            ",",
            Enumerable.Range(0, 9)
                .Select(index => $"\"0x{string.Concat(Enumerable.Repeat((index + 0x20).ToString("x2"), 32))}\""));
        var sdkResults = string.Join(
            ",",
            EthereumMainnetSccp.EthNativeEvmProverRequiredImplementationsV1
                .OrderBy(entry => entry.Key, StringComparer.Ordinal)
                .Select(entry =>
                {
                    var proofHash = entry.Key == "dotnet"
                        ? dotnetProofHash ?? defaultProofHash
                        : defaultProofHash;
                    return $$"""
                    "{{entry.Key}}": {
                      "request_hash": "0x{{new string('5', 64)}}",
                      "witness_hash": "0x{{new string('6', 64)}}",
                      "source_proof_hash": "0x{{new string('7', 64)}}",
                      "proof_hash": "{{proofHash}}",
                      "public_signal_words": [{{publicSignalWords}}],
                      "calldata_hash": "0x{{new string('9', 64)}}",
                      "torii_submit_payload_hash": "0x{{new string('a', 64)}}"
                    }
                    """;
                }));
        return $$"""
        {
          "schema": "{{EthereumMainnetSccp.EthNativeEvmProverSelfTestSchemaV1}}",
          "domain": {{EthereumMainnetSccp.DomainEthereum}},
          "chain": "eth",
          "proof_backend": "{{EthereumMainnetSccp.EvmGroth16Bn254ProofBackend}}",
          "proof_artifact_hash": "{{nativeProverBundle.ProofArtifactHash}}",
          "proving_key_hash": "{{nativeProverBundle.ProvingKeyHash}}",
          "verifier_key_hash": "{{nativeProverBundle.VerifierKeyHash}}",
          "destination_binding_hash": "{{nativeProverBundle.DestinationBindingHash}}",
          "request_hash": "0x{{new string('5', 64)}}",
          "witness_hash": "0x{{new string('6', 64)}}",
          "source_proof_hash": "0x{{new string('7', 64)}}",
          "proof_hash": "{{defaultProofHash}}",
          "public_signal_words": [{{publicSignalWords}}],
          "calldata_hash": "0x{{new string('9', 64)}}",
          "torii_submit_payload_hash": "0x{{new string('a', 64)}}",
          "sdk_results": {
            {{sdkResults}}
          }
        }
        """;
    }

    private static byte[] Groth16ProofBytes()
        => Concat(
            AbiWord(1),
            HexWord(SampleOutboundMessageId[2..]),
            AbiWord((ulong)EthereumMainnetSccp.DomainSora),
            HexWord(SampleOutboundCommitmentRoot[2..]),
            AbiWord(1),
            AbiWord(2),
            HexWord("1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed"),
            HexWord("198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2"),
            HexWord("12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa"),
            HexWord("090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b"),
            AbiWord(1),
            AbiWord(2));

    private static byte[] AbiWord(ulong value)
    {
        var bytes = new byte[32];
        BinaryPrimitives.WriteUInt64BigEndian(bytes.AsSpan(24, 8), value);
        return bytes;
    }

    private static byte[] LeU32(uint value)
    {
        var bytes = new byte[4];
        BinaryPrimitives.WriteUInt32LittleEndian(bytes, value);
        return bytes;
    }

    private static byte[] LeU64(ulong value)
    {
        var bytes = new byte[8];
        BinaryPrimitives.WriteUInt64LittleEndian(bytes, value);
        return bytes;
    }

    private static byte[] HexWord(string hex)
        => Convert.FromHexString(hex);

    private static byte[] SampleOutboundBundleBytes()
        => Convert.FromHexString(SampleOutboundBundleHex);

    private static byte[] NonSoraProofBundleBytes()
        => Convert.FromHexString(NonSoraProofBundleHex);

    private static byte[] NonSoraProofBundleFinalityBytes()
        => Convert.FromHexString(NonSoraProofBundleFinalityHex);

    private static byte[] MutatedNonSoraProofBundle(int offset, byte xorMask)
    {
        var bytes = NonSoraProofBundleBytes();
        bytes[offset] ^= xorMask;
        return bytes;
    }

    private static byte[] RepeatByte(byte value, int count)
    {
        var bytes = new byte[count];
        Array.Fill(bytes, value);
        return bytes;
    }

    private static byte[] IndexedRepeatByte(byte value, int count, int index)
    {
        var bytes = RepeatByte(value, count);
        bytes[count - 2] = (byte)((index >> 8) & 0xff);
        bytes[count - 1] = (byte)(index & 0xff);
        return bytes;
    }

    private static void WriteBytes(Stream stream, byte[] bytes)
        => stream.Write(bytes, 0, bytes.Length);

    private static byte[] Concat(params byte[][] chunks)
    {
        var output = new byte[chunks.Sum(static chunk => chunk.Length)];
        var offset = 0;
        foreach (var chunk in chunks)
        {
            chunk.CopyTo(output.AsSpan(offset));
            offset += chunk.Length;
        }

        return output;
    }

    private static byte[] RlpString(byte[] value)
    {
        if (value.Length == 1 && value[0] < 0x80)
        {
            return value.ToArray();
        }

        return Concat(RlpLengthPrefix(value.Length, 0x80, 0xb7), value);
    }

    private static byte[] RlpList(IEnumerable<byte[]> fields)
    {
        var payload = Concat(fields.ToArray());
        return Concat(RlpLengthPrefix(payload.Length, 0xc0, 0xf7), payload);
    }

    private static byte[] RlpLengthPrefix(int length, int shortOffset, int longOffset)
    {
        if (length < 56)
        {
            return [(byte)(shortOffset + length)];
        }

        var remaining = length;
        var lengthBytes = new List<byte>();
        while (remaining > 0)
        {
            lengthBytes.Insert(0, (byte)(remaining & 0xff));
            remaining >>= 8;
        }

        return Concat([(byte)(longOffset + lengthBytes.Count)], lengthBytes.ToArray());
    }

    private static byte[] SampleEthExecutionHeaderRlp()
        => RlpList(
        [
            RlpString(RepeatByte(0x10, 32)),
            RlpString(RepeatByte(0x11, 32)),
            RlpString(RepeatByte(0x12, 20)),
            RlpString(RepeatByte(0x13, 32)),
            RlpString(RepeatByte(0x14, 32)),
            RlpString(RepeatByte(0x15, 32)),
            RlpString(RepeatByte(0x00, 256)),
            RlpString([]),
            RlpString([0x2a]),
            RlpString([0x01, 0xc9, 0xc3, 0x80]),
            RlpString([0x52, 0x08]),
            RlpString([0x65, 0x53, 0xf1, 0x00]),
            RlpString(Encoding.UTF8.GetBytes("iroha-sccp-test")),
            RlpString(RepeatByte(0x16, 32)),
            RlpString(RepeatByte(0x00, 8)),
            RlpString([0x3b, 0x9a, 0xca, 0x00]),
            RlpString(RepeatByte(0x17, 32)),
            RlpString([]),
            RlpString([]),
            RlpString(RepeatByte(0x18, 32)),
        ]);

    private static byte[] SampleSyncCommitteePayload()
    {
        using var payload = new MemoryStream();
        payload.WriteByte(0x01);
        WriteBytes(payload, LeU32(512));
        for (var index = 0; index < 512; index++)
        {
            WriteBytes(payload, LeU32(48));
            WriteBytes(payload, IndexedRepeatByte(0x33, 48, index));
            WriteBytes(payload, LeU64(1));
            WriteBytes(payload, LeU32(96));
            WriteBytes(payload, IndexedRepeatByte(0xcc, 96, index));
        }

        return payload.ToArray();
    }

    private static byte[] CompressedSyncCommitteePayload()
        => Concat(
            [0x01],
            LeU32(2),
            LeU32(48),
            RepeatByte(0x33, 48),
            LeU64(1),
            LeU32(96),
            RepeatByte(0xcc, 96),
            LeU32(48),
            RepeatByte(0x44, 48),
            LeU64(1),
            LeU32(96),
            RepeatByte(0xdd, 96));

    private static byte[] WeightedSyncCommitteePayload()
    {
        var payload = SampleSyncCommitteePayload();
        var firstWeightOffset = 1 + 4 + 4 + 48;
        BinaryPrimitives.WriteUInt64LittleEndian(payload.AsSpan(firstWeightOffset, 8), 2);
        return payload;
    }

    private static EthereumMainnetBeaconRestResponse BeaconResponse(string json)
        => new(200, Encoding.UTF8.GetBytes(json));

    private static EthereumMainnetBeaconRestConsensusProvider BeaconRestProvider(
        EthereumMainnetBeaconRestResponse header,
        EthereumMainnetBeaconRestResponse checkpoint)
        => BeaconRestProvider(header, BeaconResponse(BeaconBlockRootJson()), BeaconResponse(BeaconBlockJson()), checkpoint);

    private static EthereumMainnetBeaconRestConsensusProvider BeaconRestProvider(
        EthereumMainnetBeaconRestResponse header,
        EthereumMainnetBeaconRestResponse finalizedBlockRoot,
        EthereumMainnetBeaconRestResponse finalizedBlock,
        EthereumMainnetBeaconRestResponse checkpoint)
        => BeaconRestProvider(
            header,
            finalizedBlockRoot,
            finalizedBlock,
            checkpoint,
            BeaconResponse(BeaconFinalityUpdateJson()));

    private static EthereumMainnetBeaconRestConsensusProvider BeaconRestProvider(
        EthereumMainnetBeaconRestResponse header,
        EthereumMainnetBeaconRestResponse finalizedBlockRoot,
        EthereumMainnetBeaconRestResponse finalizedBlock,
        EthereumMainnetBeaconRestResponse checkpoint,
        EthereumMainnetBeaconRestResponse finalityUpdate)
    {
        var transport = new BeaconRestTransportStub((url, _) => url switch
        {
            var value when value.EndsWith("/eth/v1/beacon/headers/finalized", StringComparison.Ordinal) => header,
            var value when value.EndsWith("/eth/v1/beacon/blocks/finalized/root", StringComparison.Ordinal) => finalizedBlockRoot,
            var value when value.EndsWith("/eth/v2/beacon/blocks/finalized", StringComparison.Ordinal) => finalizedBlock,
            var value when value.EndsWith(
                "/eth/v1/beacon/states/finalized/finality_checkpoints",
                StringComparison.Ordinal) => checkpoint,
            var value when value.EndsWith(
                "/eth/v1/beacon/light_client/finality_update",
                StringComparison.Ordinal) => finalityUpdate,
            _ => throw new InvalidOperationException($"unexpected Beacon REST URL {url}"),
        });
        return new EthereumMainnetBeaconRestConsensusProvider(
            "https://beacon.example",
            "0x" + new string('e', 64),
            transport: transport);
    }

    private static EthereumMainnetBeaconRestConsensusProvider BeaconRestProvider(
        EthereumMainnetBeaconRestResponse header,
        EthereumMainnetBeaconRestResponse finalizedBlock,
        EthereumMainnetBeaconRestResponse checkpoint)
        => BeaconRestProvider(header, BeaconResponse(BeaconBlockRootJson()), finalizedBlock, checkpoint);

    private static string BeaconHeaderJson(
        bool executionOptimistic = false,
        bool finalized = true,
        char? rootNibble = null,
        string slot = "64")
    {
        var root = rootNibble is null ? BeaconHeaderRootSlot64 : "0x" + new string(rootNibble.Value, 64);
        return $$"""
        {
          "execution_optimistic": {{executionOptimistic.ToString().ToLowerInvariant()}},
          "finalized": {{finalized.ToString().ToLowerInvariant()}},
          "data": {
            "root": "{{root}}",
            "canonical": true,
            "header": {
              "message": {
                "slot": "{{slot}}",
                "proposer_index": "1",
                "parent_root": "0x{{string.Concat(Enumerable.Repeat("01", 32))}}",
                "state_root": "0x{{string.Concat(Enumerable.Repeat("02", 32))}}",
                "body_root": "0x{{string.Concat(Enumerable.Repeat("03", 32))}}"
              },
              "signature": "0x{{string.Concat(Enumerable.Repeat("12", 96))}}"
            }
          }
        }
        """;
    }

    private static string BeaconBlockJson(
        string slot = "64",
        string? blockHash = null,
        string blockNumber = "4660",
        string? receiptsRoot = null)
        => $$"""
        {
          "execution_optimistic": false,
          "finalized": true,
          "data": {
            "message": {
              "slot": "{{slot}}",
              "body": {
                "execution_payload": {
                  "block_hash": "{{blockHash ?? ("0x" + new string('b', 64))}}",
                  "block_number": "{{blockNumber}}",
                  "receipts_root": "{{receiptsRoot ?? ("0x" + new string('c', 64))}}"
                }
              }
            }
          }
        }
        """;

    private static string BeaconCheckpointJson(char? rootNibble = null)
    {
        var root = rootNibble is null ? BeaconHeaderRootSlot64 : "0x" + new string(rootNibble.Value, 64);
        return $$"""
        {
          "execution_optimistic": false,
          "finalized": true,
          "data": {
            "finalized": {
              "root": "{{root}}",
              "epoch": "2"
            }
          }
        }
        """;
    }

    private static string BeaconGenesisJson(string genesisTime = "100")
        => $$"""
        {
          "data": {
            "genesis_time": "{{genesisTime}}",
            "genesis_validators_root": "0x{{new string('a', 64)}}",
            "genesis_fork_version": "0x00000000"
          }
        }
        """;

    private static string BeaconFinalityUpdateJson(
        string slot = "64",
        string signatureSlot = "65",
        string? syncCommitteeBits = null,
        string? syncCommitteeSignature = null,
        bool includeFinalityBranch = true,
        IReadOnlyList<string>? finalityBranch = null)
    {
        var selectedBranch = finalityBranch ?? EthereumFinalityBranch;
        var quotedFinalityBranch = string.Join(",", selectedBranch.Select(value => "\"" + value + "\""));
        var finalityBranchField = includeFinalityBranch
            ? $"""
            "finality_branch": [{quotedFinalityBranch}],
"""
            : "";
        return $$"""
        {
          "execution_optimistic": false,
          "data": {
            "finalized_header": {
              "beacon": {
                "slot": "{{slot}}",
                "proposer_index": "1",
                "parent_root": "0x{{string.Concat(Enumerable.Repeat("01", 32))}}",
                "state_root": "0x{{string.Concat(Enumerable.Repeat("02", 32))}}",
                "body_root": "0x{{string.Concat(Enumerable.Repeat("03", 32))}}"
              }
            },
            {{finalityBranchField}}
            "sync_aggregate": {
              "sync_committee_bits": "{{syncCommitteeBits ?? EthereumSyncCommitteeSupermajorityBits}}",
              "sync_committee_signature": "{{syncCommitteeSignature ?? ("0x" + string.Concat(Enumerable.Repeat("34", 96)))}}"
            },
            "signature_slot": "{{signatureSlot}}"
          }
        }
        """;
    }

    private static string BeaconBlockRootJson(char? rootNibble = null)
    {
        var root = rootNibble is null ? BeaconHeaderRootSlot64 : "0x" + new string(rootNibble.Value, 64);
        return $$"""
        {
          "execution_optimistic": false,
          "finalized": true,
          "data": {
            "root": "{{root}}"
          }
        }
        """;
    }

    [Fact]
    public void MainnetGuardsAcceptEthereumAndRejectOtherRoutes()
    {
        EthereumMainnetSccp.RequireMainnetChainId(1);
        EthereumMainnetSccp.RequireMainnetNetworkId(EthereumMainnetSccp.MainnetNetworkId);
        EthereumMainnetSccp.RequireInboundRoute(
            EthereumMainnetSccp.DomainEthereum,
            EthereumMainnetSccp.DomainSora);
        Assert.Equal(
            "0x577b41c65ffbce226de59f224b464797257063747891b88ebec1bcd57af82727",
            EthereumMainnetSccp.SourceEventTopic);
        EthereumMainnetSccp.RequireOutboundRoute(
            EthereumMainnetSccp.DomainSora,
            EthereumMainnetSccp.DomainEthereum);

        var binding = EthereumMainnetSccp.DestinationBinding(
            "0x" + new string('1', 40),
            "0x" + new string('2', 40),
            "0x" + new string('b', 64),
            "0x" + new string('c', 64));
        Assert.Equal(1, binding.Version);
        Assert.Equal(EthereumMainnetSccp.DomainSora, binding.SourceDomain);
        Assert.Equal(EthereumMainnetSccp.DomainEthereum, binding.TargetDomain);
        Assert.Equal(EthereumMainnetSccp.MainnetNetworkId, binding.NetworkId);
        Assert.Equal(EthereumMainnetSccp.EvmGroth16Bn254ProofBackend, binding.VerifierBackend);
        Assert.Equal(EthereumMainnetSccp.StarkFriProofFamily, binding.ProofFamily);
        Assert.Equal(
            "evm:0:1:0000000000000000000000000000000000000000000000000000000000000001:"
                + "0x1111111111111111111111111111111111111111:"
                + "0x2222222222222222222222222222222222222222:"
                + "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb:"
                + "0xcccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            binding.Key);
        Assert.Equal(
            "0xc86f9d904df50c4522d01da3773916ebecce816f3fdfa664e2dff7cfbe697c45",
            binding.BindingHash);
        Assert.Equal(
            binding.BindingHash,
            EthereumMainnetSccp.DestinationBindingHash(
                "0x" + new string('1', 40),
                "0x" + new string('2', 40),
                "0x" + new string('b', 64),
                "0x" + new string('c', 64)));
        Assert.Equal(
            binding.BindingHash,
            EthereumMainnetSccp.DestinationBinding(
                "0X" + new string('1', 40).ToUpperInvariant(),
                "0X" + new string('2', 40).ToUpperInvariant(),
                "0X" + new string('b', 64).ToUpperInvariant(),
                "0X" + new string('c', 64).ToUpperInvariant(),
                expectedBindingHash: binding.BindingHash,
                expectedKey: binding.Key).BindingHash);

        Assert.Throws<ArgumentOutOfRangeException>(
            () => EthereumMainnetSccp.RequireMainnetChainId(56));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.RequireMainnetNetworkId("0x38"));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.RequireMainnetNetworkId(
                "0X0000000000000000000000000000000000000000000000000000000000000001"));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.RequireMainnetNetworkId("0x01"));
        Assert.Throws<ArgumentException>(() => EthereumMainnetSccp.RequireInboundRoute(2, 0));
        Assert.Throws<ArgumentException>(() => EthereumMainnetSccp.RequireOutboundRoute(0, 2));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.DestinationBinding(
                "0x" + new string('1', 40),
                "0x" + new string('2', 40),
                "0x" + new string('b', 64),
                "0x" + new string('c', 64),
                networkId: "0x0000000000000000000000000000000000000000000000000000000000000038"));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.DestinationBinding(
                "0x" + new string('1', 40),
                "0x" + new string('1', 40),
                "0x" + new string('b', 64),
                "0x" + new string('c', 64)));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.DestinationBinding(
                "0x" + new string('0', 40),
                "0x" + new string('2', 40),
                "0x" + new string('b', 64),
                "0x" + new string('c', 64)));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.DestinationBinding(
                "0x" + new string('1', 40),
                "0x" + new string('2', 40),
                "0x" + new string('b', 64),
                "0x" + new string('c', 64),
                targetDomain: 2));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.DestinationBinding(
                "0x" + new string('1', 40),
                "0x" + new string('2', 40),
                "0x" + new string('b', 64),
                "0x" + new string('c', 64),
                expectedBindingHash: "0x" + new string('9', 64)));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.DestinationBinding(
                "0x" + new string('1', 40),
                "0x" + new string('2', 40),
                "0x" + new string('b', 64),
                "0x" + new string('c', 64),
                expectedKey: binding.Key + "-wrong"));
    }

    [Fact]
    public void SourceMaterialHashesMatchSharedEthereumVectors()
    {
        var material = SampleSourceMaterial();
        var deployment = SampleSourceAdapterDeployment();

        Assert.Equal(
            ExpectedSourceBridgeConfigHash,
            EthereumMainnetSccp.SourceBridgeConfigHash(
                material.BridgeAddress,
                material.SourceBridgeEmitterCodeHash));
        Assert.Equal(
            ExpectedSourceAdapterVerifierVkHash,
            EthereumMainnetSccp.SourceAdapterVerifierVkHash());
        Assert.NotEmpty(EthereumMainnetSccp.CanonicalSourceVerifierMaterialBytes(material));
        Assert.Equal(
            ExpectedSourceVerifierMaterialHash,
            EthereumMainnetSccp.SourceVerifierMaterialHash(material));
        Assert.Equal(
            ExpectedSourceVerifierMaterialHash,
            EthereumMainnetSccp.SourceVerifierMaterialHash(material with
            {
                SourceBridgeConfigHash = ExpectedSourceBridgeConfigHash,
            }));
        Assert.NotEmpty(EthereumMainnetSccp.CanonicalSourceAdapterEngineDeploymentBytes(deployment));
        Assert.Equal(
            ExpectedSourceAdapterEngineDeploymentHash,
            EthereumMainnetSccp.SourceAdapterEngineDeploymentHash(deployment));

        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.SourceVerifierMaterialHash(material with
            {
                NetworkId =
                    "0x0000000000000000000000000000000000000000000000000000000000000038",
            }));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.SourceVerifierMaterialHash(material with
            {
                SourceBridgeConfigHash = "0x" + new string('9', 64),
            }));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.SourceVerifierMaterialHash(material with
            {
                ConsensusVerifierHash = material.SourceTrustAnchorHash,
            }));
        var replayedNetworkIdRole = Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.SourceVerifierMaterialHash(material with
            {
                SourceTrustAnchorHash = EthereumMainnetSccp.MainnetNetworkId,
                NetworkId = EthereumMainnetSccp.MainnetNetworkId,
            }));
        Assert.Contains("role-separated", replayedNetworkIdRole.Message);
        Assert.Contains(
            nameof(EthereumMainnetSourceVerifierMaterialInput.NetworkId),
            replayedNetworkIdRole.Message);
        var nonCanonicalAdapterVerifier = Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.SourceAdapterEngineDeploymentHash(deployment with
            {
                AdapterVerifierVkHash = "0x" + new string('9', 64),
            }));
        Assert.Contains(
            "canonical Ethereum mainnet source-adapter verifier profile",
            nonCanonicalAdapterVerifier.Message);

        var replayedDeploymentReceipt = Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.SourceAdapterEngineDeploymentHash(deployment with
            {
                DeploymentReceiptHash = ExpectedSourceAdapterVerifierVkHash,
            }));
        Assert.Contains("role-separated", replayedDeploymentReceipt.Message);
        Assert.Contains(
            nameof(EthereumMainnetSourceAdapterDeploymentInput.DeploymentReceiptHash),
            replayedDeploymentReceipt.Message);
    }

    [Fact]
    public void BeaconExecutionPayloadSszRootsMatchSharedVector()
    {
        var headerRlp = SampleEthExecutionHeaderRlp();
        var executionPayloadRoot = EthereumMainnetSccp.EthExecutionPayloadHeaderRootFromRlp(headerRlp);
        var executionPayloadBranch = new[]
        {
            RepeatByte(0xee, 32),
            RepeatByte(0xff, 32),
            RepeatByte(0x11, 32),
            RepeatByte(0x22, 32),
        };
        var beaconBodyRoot = EthereumMainnetSccp.EthBeaconBodyRootFromExecutionPayloadBranch(
            executionPayloadRoot,
            executionPayloadBranch);
        var beaconHeaderRoot = EthereumMainnetSccp.EthBeaconBlockHeaderRoot(
            beaconSlot: 320,
            beaconProposerIndex: 17,
            beaconParentRoot: string.Concat(Enumerable.Repeat("aa", 32)),
            beaconStateRoot: string.Concat(Enumerable.Repeat("bb", 32)),
            beaconBodyRoot: beaconBodyRoot);

        Assert.Equal(
            "0xc029dda492d2e41ad72bd83f1727a67e5331f413ec29d5c31de955d0bea24624",
            executionPayloadRoot);
        Assert.Equal(
            "0x431e6bef5e759e8fdf32d8e8ed1ff761933ddb4de24ec9ae8e2aa0d25fe861ba",
            beaconBodyRoot);
        Assert.Equal(
            "0xd54b406debae26e6ebaef512cc4f9e6bc12cf02af0d4476895383b37f682a179",
            beaconHeaderRoot);
        Assert.NotEqual(
            beaconBodyRoot,
            EthereumMainnetSccp.EthBeaconBodyRootFromExecutionPayloadBranch(
                executionPayloadRoot,
                [
                    RepeatByte(0xff, 32),
                    RepeatByte(0xff, 32),
                    RepeatByte(0x11, 32),
                    RepeatByte(0x22, 32),
                ]));

        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.EthBeaconBodyRootFromExecutionPayloadBranch(
                executionPayloadRoot,
                [RepeatByte(0xee, 32)]));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.EthBeaconBodyRootFromExecutionPayloadBranch(
                executionPayloadRoot,
                [
                    RepeatByte(0xee, 31),
                    RepeatByte(0xff, 32),
                    RepeatByte(0x11, 32),
                    RepeatByte(0x22, 32),
                ]));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.EthExecutionPayloadHeaderRootFromRlp([0x80]));
    }

    [Fact]
    public void ReceiptProofTranscriptMatchesSharedEthereumVector()
    {
        var sourceEventDigest = "0x" + new string('e', 64);
        var proofNodes = new[] { new byte[] { 0x01 }, new byte[] { 0x02, 0x03 } };
        var branch = new[] { RepeatByte(0x11, 32) };

        byte[] BuildBytes(
            string? digest = null,
            string? executionBlockHash = null,
            string? executionReceiptsRoot = null,
            string? beaconFinalizedRoot = null,
            string? syncCommitteeRoot = null,
            IReadOnlyList<byte[]>? nodes = null,
            IReadOnlyList<byte[]>? inclusionBranch = null,
            int sourceDomain = EthereumMainnetSccp.DomainEthereum)
            => EthereumMainnetSccp.CanonicalEvmSccpReceiptProofBytes(
                digest ?? sourceEventDigest,
                beaconSlot: 32,
                executionBlockNumber: 0x1234,
                executionBlockHash: executionBlockHash ?? "0x" + new string('b', 64),
                executionReceiptsRoot: executionReceiptsRoot ?? "0x" + new string('c', 64),
                beaconFinalizedRoot: beaconFinalizedRoot ?? "0x" + new string('d', 64),
                syncCommitteeRoot: syncCommitteeRoot ?? "0x" + new string('a', 64),
                receiptRootIndex: 3,
                receiptTrieProofNodes: nodes ?? proofNodes,
                inclusionBranch: inclusionBranch ?? branch,
                sourceDomain: sourceDomain);

        var bytes = BuildBytes();
        Assert.Equal(240, bytes.Length);
        Assert.Equal(ExpectedReceiptProofBytes, Convert.ToHexString(bytes).ToLowerInvariant());
        Assert.Equal(
            ExpectedReceiptProofHash,
            EthereumMainnetSccp.EvmSccpReceiptProofHash(
                sourceEventDigest,
                beaconSlot: 32,
                executionBlockNumber: 0x1234,
                executionBlockHash: "0x" + new string('b', 64),
                executionReceiptsRoot: "0x" + new string('c', 64),
                beaconFinalizedRoot: "0x" + new string('d', 64),
                syncCommitteeRoot: "0x" + new string('a', 64),
                receiptRootIndex: 3,
                receiptTrieProofNodes: proofNodes,
                inclusionBranch: branch));

        Assert.Throws<ArgumentException>(() => BuildBytes(sourceDomain: 2));
        Assert.Throws<ArgumentException>(() => BuildBytes(digest: "0x" + new string('0', 64)));
        var zeroRoot = "0x" + new string('0', 64);
        Assert.Throws<ArgumentException>(() => BuildBytes(executionBlockHash: zeroRoot));
        Assert.Throws<ArgumentException>(() => BuildBytes(executionReceiptsRoot: zeroRoot));
        Assert.Throws<ArgumentException>(() => BuildBytes(beaconFinalizedRoot: zeroRoot));
        Assert.Throws<ArgumentException>(() => BuildBytes(syncCommitteeRoot: zeroRoot));
        Assert.Throws<ArgumentException>(() => BuildBytes(nodes: Array.Empty<byte[]>()));
        Assert.Throws<ArgumentException>(() => BuildBytes(nodes: [new byte[0]]));
        Assert.Throws<ArgumentException>(() => BuildBytes(inclusionBranch: Array.Empty<byte[]>()));
        Assert.Throws<ArgumentException>(() => BuildBytes(inclusionBranch: [new byte[31]]));
    }

    [Fact]
    public void ReceiptTrieProofBuilderUsesRlpTransactionIndexKeys()
    {
        var blockHash = "0x" + new string('b', 64);
        var logsBloom = "0x" + new string('0', 512);
        var typedReceipt = new Dictionary<string, object?>
        {
            ["type"] = "0x2",
            ["transactionHash"] = "0x" + new string('a', 64),
            ["blockHash"] = blockHash,
            ["blockNumber"] = "0x1234",
            ["transactionIndex"] = "0x0",
            ["status"] = "0x1",
            ["cumulativeGasUsed"] = "0x5208",
            ["logsBloom"] = logsBloom,
            ["logs"] = Array.Empty<object?>(),
        };
        var legacyReceipt = new Dictionary<string, object?>
        {
            ["transactionHash"] = "0x" + string.Concat(Enumerable.Repeat("12", 32)),
            ["blockHash"] = blockHash,
            ["blockNumber"] = "0x1234",
            ["transactionIndex"] = "0x1",
            ["status"] = "0x1",
            ["cumulativeGasUsed"] = "0x5300",
            ["logsBloom"] = logsBloom,
            ["logs"] = Array.Empty<object?>(),
        };

        Assert.Equal("0x80", EthereumMainnetSccp.EvmReceiptTrieKey(0));
        Assert.Equal("0x01", EthereumMainnetSccp.EvmReceiptTrieKey("0x1"));
        Assert.Equal("0x8180", EthereumMainnetSccp.EvmReceiptTrieKey("0x80"));
        Assert.Throws<ArgumentException>(() => EthereumMainnetSccp.EvmReceiptTrieKey("0x01"));
        var typedReceiptRlp = EthereumMainnetSccp.CanonicalEvmReceiptRlp(typedReceipt);
        Assert.Equal(0x02, typedReceiptRlp[0]);

        var proof = EthereumMainnetSccp.BuildEvmReceiptTrieProofFromReceipts(
            [typedReceipt, legacyReceipt],
            "0x0");
        Assert.Equal("0x80", proof.ReceiptTrieKey);
        Assert.Equal("0x" + Convert.ToHexString(typedReceiptRlp).ToLowerInvariant(), proof.ReceiptRlp);
        Assert.Equal(66, proof.ReceiptsRoot.Length);
        Assert.NotEmpty(proof.ReceiptTrieProofNodes);

        var secondProof = EthereumMainnetSccp.BuildEvmReceiptTrieProofFromReceipts(
            [typedReceipt, legacyReceipt],
            "0x1");
        Assert.Equal("0x01", secondProof.ReceiptTrieKey);
        var zeroTopicReceipt = new Dictionary<string, object?>(legacyReceipt)
        {
            ["logs"] = new object?[]
            {
                new Dictionary<string, object?>
                {
                    ["address"] = "0x" + string.Concat(Enumerable.Repeat("12", 20)),
                    ["topics"] = new object?[] { "0x" + new string('0', 64) },
                    ["data"] = "0x",
                },
            },
        };
        var zeroTopicProof = EthereumMainnetSccp.BuildEvmReceiptTrieProofFromReceipts(
            [typedReceipt, zeroTopicReceipt],
            "0x0");
        Assert.Equal(proof.ReceiptRlp, zeroTopicProof.ReceiptRlp);
        var zeroAddressReceipt = new Dictionary<string, object?>(legacyReceipt)
        {
            ["transactionHash"] = "0x" + string.Concat(Enumerable.Repeat("ac", 32)),
            ["logs"] = new object?[]
            {
                new Dictionary<string, object?>
                {
                    ["address"] = "0x" + new string('0', 40),
                    ["topics"] = new object?[] { "0x" + string.Concat(Enumerable.Repeat("44", 32)) },
                    ["data"] = "0x",
                },
            },
        };
        var zeroAddressProof = EthereumMainnetSccp.BuildEvmReceiptTrieProofFromReceipts(
            [typedReceipt, zeroAddressReceipt],
            "0x0");
        Assert.Equal(proof.ReceiptRlp, zeroAddressProof.ReceiptRlp);
        var wrongReceiptIndex = new Dictionary<string, object?>(typedReceipt)
        {
            ["transactionIndex"] = "0x1",
        };
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildEvmReceiptTrieProofFromReceipts(
                [wrongReceiptIndex],
                "0x0"));
        var conflictingReceiptIndex = new Dictionary<string, object?>(typedReceipt)
        {
            ["transaction_index"] = "0x0",
        };
        var conflictingReceiptIndexError = Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildEvmReceiptTrieProofFromReceipts(
                [conflictingReceiptIndex],
                "0x0"));
        Assert.Contains("blockReceipts[0].transactionIndex", conflictingReceiptIndexError.Message);
        var conflictingReceiptHash = new Dictionary<string, object?>(typedReceipt)
        {
            ["transaction_hash"] = typedReceipt["transactionHash"],
        };
        var conflictingReceiptHashError = Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildEvmReceiptTrieProofFromReceipts(
                [conflictingReceiptHash],
                "0x0"));
        Assert.Contains("blockReceipts[0].transactionHash", conflictingReceiptHashError.Message);
        var conflictingCumulativeGas = new Dictionary<string, object?>(typedReceipt)
        {
            ["cumulative_gas_used"] = "0x5208",
        };
        var conflictingCumulativeGasError = Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildEvmReceiptTrieProofFromReceipts(
                [conflictingCumulativeGas],
                "0x0"));
        Assert.Contains("receipt.cumulativeGasUsed", conflictingCumulativeGasError.Message);
        var conflictingLogsBloom = new Dictionary<string, object?>(typedReceipt)
        {
            ["logs_bloom"] = logsBloom,
        };
        var conflictingLogsBloomError = Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildEvmReceiptTrieProofFromReceipts(
                [conflictingLogsBloom],
                "0x0"));
        Assert.Contains("receipt.logsBloom", conflictingLogsBloomError.Message);
        Assert.Throws<ArgumentOutOfRangeException>(
            () => EthereumMainnetSccp.BuildEvmReceiptTrieProofFromReceipts(
                [typedReceipt],
                "0x1"));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildEvmReceiptTrieProofFromReceipts(
                Array.Empty<IReadOnlyDictionary<string, object?>>(),
                "0x0"));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildEvmReceiptTrieProofFromReceipts(
                Enumerable.Repeat<IReadOnlyDictionary<string, object?>>(typedReceipt, 4_097).ToArray(),
                "0x0"));
        var uppercaseBloomReceipt = new Dictionary<string, object?>(typedReceipt)
        {
            ["logsBloom"] = "0x" + new string('A', 512),
        };
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.CanonicalEvmReceiptRlp(uppercaseBloomReceipt));
        var badTypedReceipt = new Dictionary<string, object?>(typedReceipt)
        {
            ["type"] = "0x80",
        };
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.CanonicalEvmReceiptRlp(badTypedReceipt));
        var unsupportedTypedReceipt = new Dictionary<string, object?>(typedReceipt)
        {
            ["type"] = "0x7f",
        };
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.CanonicalEvmReceiptRlp(unsupportedTypedReceipt));
        var validReceiptLog = new Dictionary<string, object?>
        {
            ["address"] = "0x" + new string('1', 40),
            ["topics"] = new object?[] { "0x" + new string('2', 64) },
            ["data"] = "0x",
        };
        var removedLog = new Dictionary<string, object?>(validReceiptLog)
        {
            ["removed"] = true,
        };
        var removedLogReceipt = new Dictionary<string, object?>(typedReceipt)
        {
            ["logs"] = new object?[] { removedLog },
        };
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.CanonicalEvmReceiptRlp(removedLogReceipt));
        var tooManyTopicsLog = new Dictionary<string, object?>(validReceiptLog)
        {
            ["topics"] = Enumerable.Repeat<object?>("0x" + new string('2', 64), 5).ToArray(),
        };
        var tooManyTopicsReceipt = new Dictionary<string, object?>(typedReceipt)
        {
            ["logs"] = new object?[] { tooManyTopicsLog },
        };
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.CanonicalEvmReceiptRlp(tooManyTopicsReceipt));
    }

    [Fact]
    public async Task InboundEvidenceUsesMainnetRpcAndRejectsDrift()
    {
        var txHash = "0x" + new string('a', 64);
        var blockHash = "0x" + new string('b', 64);
        var sourceEventDigest = "0x" + new string('e', 64);
        var sourceBridgeEmitterAddress = "0x" + string.Concat(Enumerable.Repeat("12", 20));
        var unrelatedLog = new Dictionary<string, object?>
        {
            ["address"] = "0x" + new string('0', 40),
            ["topics"] = new object?[] { "0x" + new string('0', 64) },
            ["data"] = "0x1234",
        };
        var sourceEventLog = new Dictionary<string, object?>
        {
            ["address"] = sourceBridgeEmitterAddress,
            ["transactionHash"] = txHash,
            ["blockHash"] = blockHash,
            ["blockNumber"] = "0x1234",
            ["topics"] = new object?[] { EthereumMainnetSccp.SourceEventTopic, sourceEventDigest },
            ["data"] = "0x",
        };
        var receipt = new Dictionary<string, object?>
        {
            ["transactionHash"] = txHash,
            ["blockHash"] = blockHash,
            ["blockNumber"] = "0x1234",
            ["status"] = "0x1",
        };
        var receiptWithSourceEvent = new Dictionary<string, object?>(receipt)
        {
            ["logs"] = new object?[] { unrelatedLog, sourceEventLog },
        };
        var block = new Dictionary<string, object?>
        {
            ["hash"] = blockHash,
            ["number"] = "0x1234",
            ["receiptsRoot"] = "0x" + new string('c', 64),
        };
        var beaconFinalityEvidence = new EthereumMainnetBeaconFinalityEvidence(
            "0x1234",
            blockHash,
            "0x" + new string('c', 64),
            BeaconSlot: "0x20",
            SyncCommitteeBits: EthereumSyncCommitteeSupermajorityBits,
            SyncCommitteeSignature: "0x" + string.Concat(Enumerable.Repeat("34", 96)),
            SyncCommitteeParticipation: EthereumSyncCommitteeSupermajorityParticipation,
            SyncSignatureSlot: "65");
        var beaconFinality = beaconFinalityEvidence.ToDictionary(
            [
                new KeyValuePair<string, object?>("finalityBranch", EthereumFinalityBranch),
                new KeyValuePair<string, object?>("finalizedHeaderRoot", "0x" + new string('d', 64)),
                new KeyValuePair<string, object?>("syncCommitteeRoot", "0x" + new string('a', 64)),
            ]);
        var logsBloom = "0x" + new string('0', 512);
        var rlpUnrelatedLog = new Dictionary<string, object?>
        {
            ["address"] = "0x" + string.Concat(Enumerable.Repeat("11", 20)),
            ["topics"] = new object?[] { "0x" + string.Concat(Enumerable.Repeat("22", 32)) },
            ["data"] = "0x1234",
        };
        var rlpSourceReceipt = new Dictionary<string, object?>(receiptWithSourceEvent)
        {
            ["transactionIndex"] = "0x0",
            ["cumulativeGasUsed"] = "0x5208",
            ["logsBloom"] = logsBloom,
            ["logs"] = new object?[] { rlpUnrelatedLog, sourceEventLog },
        };
        var otherReceipt = new Dictionary<string, object?>
        {
            ["transactionHash"] = "0x" + string.Concat(Enumerable.Repeat("34", 32)),
            ["blockHash"] = blockHash,
            ["blockNumber"] = "0x1234",
            ["transactionIndex"] = "0x1",
            ["status"] = "0x1",
            ["cumulativeGasUsed"] = "0x5300",
            ["logsBloom"] = logsBloom,
            ["logs"] = Array.Empty<object?>(),
        };
        IReadOnlyList<IReadOnlyDictionary<string, object?>> blockReceipts = [rlpSourceReceipt, otherReceipt];
        var receiptTrieProof = EthereumMainnetSccp.BuildEvmReceiptTrieProofFromReceipts(
            blockReceipts,
            "0x0");
        var autoReceiptBlock = new Dictionary<string, object?>
        {
            ["hash"] = blockHash,
            ["number"] = "0x1234",
            ["receiptsRoot"] = receiptTrieProof.ReceiptsRoot,
        };
        var autoReceiptFinality = new Dictionary<string, object?>
        {
            ["executionBlockNumber"] = "0x1234",
            ["executionBlockHash"] = blockHash,
            ["executionReceiptsRoot"] = receiptTrieProof.ReceiptsRoot,
            ["finalizedHeaderRoot"] = "0x" + new string('d', 64),
            ["syncCommitteeRoot"] = "0x" + new string('a', 64),
            ["beaconSlot"] = "0x20",
            ["syncCommitteeBits"] = EthereumSyncCommitteeSupermajorityBits,
            ["syncCommitteeSignature"] = "0x" + string.Concat(Enumerable.Repeat("34", 96)),
            ["syncCommitteeParticipation"] = EthereumSyncCommitteeSupermajorityParticipation,
            ["syncSignatureSlot"] = "65",
        };
        var autoReceiptInclusionBranch = new[] { RepeatByte(0x44, 32) };
        var autoReceiptProvider = new ExecutionProviderStub(
            "0x1",
            rlpSourceReceipt,
            autoReceiptBlock,
            blockReceipts);
        var autoReceiptEvidence = await EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
            new EthereumMainnetInboundEvidence
            {
                Receipt = rlpSourceReceipt,
                Block = autoReceiptBlock,
                BeaconFinality = autoReceiptFinality,
                InclusionBranch = autoReceiptInclusionBranch,
                SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
            },
            autoReceiptProvider);
        Assert.Equal(["eth_chainId", "eth_getBlockReceipts"], autoReceiptProvider.Calls);
        Assert.Equal(sourceEventDigest, autoReceiptEvidence.SourceEventDigest);
        Assert.Equal(2, autoReceiptEvidence.BlockReceipts?.Count);
        Assert.NotNull(autoReceiptEvidence.ReceiptProof);
        Assert.Equal(EthereumMainnetSccp.DomainEthereum, autoReceiptEvidence.ReceiptProof!.SourceDomain);
        Assert.Equal(0UL, autoReceiptEvidence.ReceiptProof.ReceiptRootIndex);
        Assert.Equal(32UL, autoReceiptEvidence.ReceiptProof.BeaconSlot);
        Assert.Equal(0x1234UL, autoReceiptEvidence.ReceiptProof.ExecutionBlockNumber);
        Assert.Equal(receiptTrieProof.ReceiptsRoot, autoReceiptEvidence.ReceiptProof.ExecutionReceiptsRoot);
        Assert.Equal(
            receiptTrieProof.ReceiptTrieProofNodes.Count,
            autoReceiptEvidence.ReceiptProof.ReceiptTrieProofNodes.Count);
        for (var index = 0; index < receiptTrieProof.ReceiptTrieProofNodes.Count; index++)
        {
            Assert.Equal(
                receiptTrieProof.ReceiptTrieProofNodes[index],
                autoReceiptEvidence.ReceiptProof.ReceiptTrieProofNodes[index]);
        }
        Assert.Equal(autoReceiptInclusionBranch[0], autoReceiptEvidence.ReceiptProof.InclusionBranch[0]);
        Assert.Equal(autoReceiptInclusionBranch[0], autoReceiptEvidence.InclusionBranch![0]);
        Assert.Equal(
            EthereumMainnetSccp.EvmSccpReceiptProofHash(
                autoReceiptEvidence.ReceiptProof.SourceEventDigest,
                autoReceiptEvidence.ReceiptProof.BeaconSlot,
                autoReceiptEvidence.ReceiptProof.ExecutionBlockNumber,
                autoReceiptEvidence.ReceiptProof.ExecutionBlockHash,
                autoReceiptEvidence.ReceiptProof.ExecutionReceiptsRoot,
                autoReceiptEvidence.ReceiptProof.BeaconFinalizedRoot,
                autoReceiptEvidence.ReceiptProof.SyncCommitteeRoot,
                autoReceiptEvidence.ReceiptProof.ReceiptRootIndex,
                autoReceiptEvidence.ReceiptProof.ReceiptTrieProofNodes,
                autoReceiptEvidence.ReceiptProof.InclusionBranch),
            autoReceiptEvidence.ReceiptProofHash);
        foreach (var (missingField, label) in new[]
        {
            ("finalizedHeaderRoot", "beaconFinality.finalizedHeaderRoot"),
            ("syncCommitteeRoot", "beaconFinality.syncCommitteeRoot"),
            ("beaconSlot", "beaconFinality.beaconSlot"),
        })
        {
            var incompleteFinality = new Dictionary<string, object?>(autoReceiptFinality);
            incompleteFinality.Remove(missingField);
            var missingFinality = await Assert.ThrowsAsync<ArgumentException>(
                () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                    new EthereumMainnetInboundEvidence
                    {
                        Receipt = rlpSourceReceipt,
                        Block = autoReceiptBlock,
                        BeaconFinality = incompleteFinality,
                        BlockReceipts = blockReceipts,
                        InclusionBranch = [RepeatByte(0x44, 32)],
                        SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                    }).AsTask());
            Assert.Contains(label, missingFinality.Message);
        }
        foreach (var (alias, value, label) in new[]
        {
            ("transaction_hash", "0x" + new string('a', 64), "receipt.transactionHash"),
            ("block_hash", "0x" + new string('a', 64), "receipt.blockHash"),
            ("block_number", "0x1235", "receipt.blockNumber"),
            ("transaction_index", "0x0", "receipt.transactionIndex"),
        })
        {
            var conflictingReceipt = new Dictionary<string, object?>(rlpSourceReceipt)
            {
                [alias] = value,
            };
            var aliasConflict = await Assert.ThrowsAsync<ArgumentException>(
                () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                    new EthereumMainnetInboundEvidence
                    {
                        Receipt = conflictingReceipt,
                        Block = autoReceiptBlock,
                        BeaconFinality = autoReceiptFinality,
                        BlockReceipts = blockReceipts,
                        InclusionBranch = [RepeatByte(0x44, 32)],
                        SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                    }).AsTask());
            Assert.Contains(label, aliasConflict.Message);
        }
        foreach (var (alias, value) in new[] { ("blockNumber", "0x1235"), ("block_number", "0x1235") })
        {
            var conflictingBlock = new Dictionary<string, object?>(autoReceiptBlock)
            {
                [alias] = value,
            };
            var aliasConflict = await Assert.ThrowsAsync<ArgumentException>(
                () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                    new EthereumMainnetInboundEvidence
                    {
                        Receipt = rlpSourceReceipt,
                        Block = conflictingBlock,
                        BeaconFinality = autoReceiptFinality,
                        BlockReceipts = blockReceipts,
                        InclusionBranch = [RepeatByte(0x44, 32)],
                        SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                    }).AsTask());
            Assert.Contains("block.number", aliasConflict.Message);
        }
        var conflictingReceiptsRootBlock = new Dictionary<string, object?>(autoReceiptBlock)
        {
            ["receipts_root"] = "0x" + new string('a', 64),
        };
        var receiptsRootAliasConflict = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = rlpSourceReceipt,
                    Block = conflictingReceiptsRootBlock,
                    BeaconFinality = autoReceiptFinality,
                    BlockReceipts = blockReceipts,
                    InclusionBranch = [RepeatByte(0x44, 32)],
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                }).AsTask());
        Assert.Contains("block.receiptsRoot", receiptsRootAliasConflict.Message);
        foreach (var (alias, value, label) in new[]
        {
            ("block_hash", "0x" + new string('a', 64), "blockReceipts.blockHash"),
            ("block_number", "0x1235", "blockReceipts.blockNumber"),
        })
        {
            var conflictingIndexedReceipt = new Dictionary<string, object?>(rlpSourceReceipt)
            {
                [alias] = value,
            };
            var aliasConflict = await Assert.ThrowsAsync<ArgumentException>(
                () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                    new EthereumMainnetInboundEvidence
                    {
                        Receipt = rlpSourceReceipt,
                        Block = autoReceiptBlock,
                        BeaconFinality = autoReceiptFinality,
                        BlockReceipts = [conflictingIndexedReceipt, otherReceipt],
                        InclusionBranch = [RepeatByte(0x44, 32)],
                        SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                    }).AsTask());
            Assert.Contains(label, aliasConflict.Message);
        }
        var conflictingIndexedHashReceipt = new Dictionary<string, object?>(rlpSourceReceipt)
        {
            ["transaction_hash"] = rlpSourceReceipt["transactionHash"],
        };
        var indexedHashAliasConflict = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = rlpSourceReceipt,
                    Block = autoReceiptBlock,
                    BeaconFinality = autoReceiptFinality,
                    BlockReceipts = [conflictingIndexedHashReceipt, otherReceipt],
                    InclusionBranch = [RepeatByte(0x44, 32)],
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                }).AsTask());
        Assert.Contains("blockReceipts[0].transactionHash", indexedHashAliasConflict.Message);
        var computedRootMismatch = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = rlpSourceReceipt,
                    Block = new Dictionary<string, object?>
                    {
                        ["hash"] = blockHash,
                        ["number"] = "0x1234",
                        ["receiptsRoot"] = "0x" + new string('9', 64),
                    },
                    BeaconFinality = new Dictionary<string, object?>(autoReceiptFinality)
                    {
                        ["executionReceiptsRoot"] = "0x" + new string('9', 64),
                    },
                    BlockReceipts = blockReceipts,
                    InclusionBranch = autoReceiptInclusionBranch,
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                }).AsTask());
        Assert.Contains("receiptProof.executionReceiptsRoot", computedRootMismatch.Message);
        autoReceiptInclusionBranch[0][0] = 0x55;
        Assert.Equal(0x44, autoReceiptEvidence.InclusionBranch[0][0]);
        Assert.Equal(0x44, autoReceiptEvidence.ReceiptProof.InclusionBranch[0][0]);
        var wrongIndexedReceipt = new Dictionary<string, object?>(rlpSourceReceipt)
        {
            ["transactionHash"] = "0x" + string.Concat(Enumerable.Repeat("ac", 32)),
        };
        IReadOnlyList<IReadOnlyDictionary<string, object?>> wrongBlockReceipts =
            [wrongIndexedReceipt, otherReceipt];
        var wrongTransactionHash = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = rlpSourceReceipt,
                    Block = autoReceiptBlock,
                    BeaconFinality = autoReceiptFinality,
                    BlockReceipts = wrongBlockReceipts,
                    InclusionBranch = [RepeatByte(0x44, 32)],
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                }).AsTask());
        Assert.Contains("blockReceipts.transactionHash", wrongTransactionHash.Message);
        var duplicateTransactionHashReceipt = new Dictionary<string, object?>(otherReceipt)
        {
            ["transactionHash"] = rlpSourceReceipt["transactionHash"],
        };
        var duplicateTransactionHash = Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildEvmReceiptTrieProofFromReceipts(
                [rlpSourceReceipt, duplicateTransactionHashReceipt],
                "0x0"));
        Assert.Contains("transactionHash values must be unique", duplicateTransactionHash.Message);
        var mismatchedIndexedReceipt = new Dictionary<string, object?>(rlpSourceReceipt)
        {
            ["logs"] = Array.Empty<object?>(),
        };
        IReadOnlyList<IReadOnlyDictionary<string, object?>> mismatchedBlockReceipts =
            [mismatchedIndexedReceipt, otherReceipt];
        var mismatchedReceiptProof = EthereumMainnetSccp.BuildEvmReceiptTrieProofFromReceipts(
            mismatchedBlockReceipts,
            "0x0");
        var mismatchedReceiptRlp = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = rlpSourceReceipt,
                    Block = new Dictionary<string, object?>
                    {
                        ["hash"] = blockHash,
                        ["number"] = "0x1234",
                        ["receiptsRoot"] = mismatchedReceiptProof.ReceiptsRoot,
                    },
                    BeaconFinality = new Dictionary<string, object?>(autoReceiptFinality)
                    {
                        ["executionReceiptsRoot"] = mismatchedReceiptProof.ReceiptsRoot,
                    },
                    BlockReceipts = mismatchedBlockReceipts,
                    InclusionBranch = [RepeatByte(0x44, 32)],
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                }).AsTask());
        Assert.Contains("blockReceipts.receiptRlp", mismatchedReceiptRlp.Message);
        var blockHashDriftReceipt = new Dictionary<string, object?>(rlpSourceReceipt)
        {
            ["blockHash"] = "0x" + new string('9', 64),
        };
        var blockHashDrift = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = rlpSourceReceipt,
                    Block = autoReceiptBlock,
                    BeaconFinality = autoReceiptFinality,
                    BlockReceipts = [blockHashDriftReceipt, otherReceipt],
                    InclusionBranch = [RepeatByte(0x44, 32)],
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                }).AsTask());
        Assert.Contains("blockReceipts.blockHash", blockHashDrift.Message);
        var blockNumberDriftReceipt = new Dictionary<string, object?>(rlpSourceReceipt)
        {
            ["blockNumber"] = "0x1235",
        };
        var blockNumberDrift = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = rlpSourceReceipt,
                    Block = autoReceiptBlock,
                    BeaconFinality = autoReceiptFinality,
                    BlockReceipts = [blockNumberDriftReceipt, otherReceipt],
                    InclusionBranch = [RepeatByte(0x44, 32)],
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                }).AsTask());
        Assert.Contains("blockReceipts.blockNumber", blockNumberDrift.Message);
        var mutableReceiptProofNode = new byte[] { 0x01 };
        var mutableReceiptProofBranch = RepeatByte(0x11, 32);
        var receiptProof = new EthereumMainnetReceiptProof
        {
            SourceEventDigest = sourceEventDigest,
            BeaconSlot = 32,
            ExecutionBlockNumber = 0x1234,
            ExecutionBlockHash = blockHash,
            ExecutionReceiptsRoot = "0x" + new string('c', 64),
            BeaconFinalizedRoot = "0x" + new string('d', 64),
            SyncCommitteeRoot = "0x" + new string('a', 64),
            ReceiptRootIndex = 3,
            ReceiptTrieProofNodes = [mutableReceiptProofNode, new byte[] { 0x02, 0x03 }],
            InclusionBranch = [mutableReceiptProofBranch],
        };
        var provider = new ExecutionProviderStub("0x1", receipt, block);
        var consensusProvider = new ConsensusProviderStub(
            receipt,
            block,
            txHash,
            beaconFinality);

        var evidence = await EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
            new EthereumMainnetInboundEvidence { TransactionHash = txHash },
            provider,
            consensusProvider);
        Assert.Equal(txHash, evidence.TransactionHash);
        Assert.Equal("0x1", evidence.Receipt?["status"]);
        Assert.Equal("0x" + new string('c', 64), evidence.Block?["receiptsRoot"]);
        Assert.Equal("4660", evidence.BeaconFinality?["executionBlockNumber"]);
        Assert.Equal(blockHash, evidence.BeaconFinality?["executionBlockHash"]);
        Assert.Equal("0x" + new string('d', 64), evidence.BeaconFinality?["finalizedHeaderRoot"]);
        Assert.Equal("0x" + new string('a', 64), evidence.BeaconFinality?["syncCommitteeRoot"]);
        Assert.Equal("32", evidence.BeaconFinality?["beaconSlot"]);
        Assert.Equal(1, consensusProvider.Calls);
        Assert.Equal(
            ["eth_chainId", "eth_getTransactionReceipt", "eth_getBlockByHash"],
            provider.Calls);

        var missingSourceEvent = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.ProveInboundToSoraAsync(
            evidence with
            {
                ReceiptProof = receiptProof,
                ReceiptProofHash = ExpectedReceiptProofHash,
            },
            new InboundProverStub(txHash, ExpectedReceiptProofHash, sourceEventDigest)).AsTask());
        Assert.Contains("receipt source event validation", missingSourceEvent.Message);

        var sourceEventEvidence = await EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
            new EthereumMainnetInboundEvidence
            {
                Receipt = receiptWithSourceEvent,
                Block = block,
                BeaconFinality = beaconFinality,
                SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
            });
        Assert.Equal(sourceEventDigest, sourceEventEvidence.SourceEventDigest);
        Assert.Equal(sourceBridgeEmitterAddress, sourceEventEvidence.SourceBridgeEmitterAddress);
        var explicitSourceEventEvidence = await EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
            new EthereumMainnetInboundEvidence
            {
                Receipt = receiptWithSourceEvent,
                Block = block,
                BeaconFinality = beaconFinality,
                SourceEventDigest = sourceEventDigest,
                SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
            });
        Assert.Equal(sourceEventDigest, explicitSourceEventEvidence.SourceEventDigest);
        var configuredSourceEventEvidence = await EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
            new EthereumMainnetInboundEvidence
            {
                Receipt = receiptWithSourceEvent,
                Block = block,
                BeaconFinality = beaconFinality,
            },
            sourceBridgeEmitterAddress: sourceBridgeEmitterAddress);
        Assert.Equal(sourceEventDigest, configuredSourceEventEvidence.SourceEventDigest);
        Assert.Equal(sourceBridgeEmitterAddress, configuredSourceEventEvidence.SourceBridgeEmitterAddress);
        var configuredBridgeDrift = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = receiptWithSourceEvent,
                    Block = block,
                    BeaconFinality = beaconFinality,
                    SourceBridgeEmitterAddress = "0x" + string.Concat(Enumerable.Repeat("13", 20)),
                },
                sourceBridgeEmitterAddress: sourceBridgeEmitterAddress).AsTask());
        Assert.Contains("sourceBridgeEmitterAddress", configuredBridgeDrift.Message);

        var missingFinalityBranchFinality = new Dictionary<string, object?>(sourceEventEvidence.BeaconFinality!);
        missingFinalityBranchFinality.Remove("finalityBranch");
        var missingFinalityBranch = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.ProveInboundToSoraAsync(
                sourceEventEvidence with
                {
                    BeaconFinality = missingFinalityBranchFinality,
                    ReceiptProof = receiptProof,
                    ReceiptProofHash = ExpectedReceiptProofHash,
                },
                new InboundProverStub(txHash, ExpectedReceiptProofHash, sourceEventDigest)).AsTask());
        Assert.Contains("beaconFinality.finalityBranch", missingFinalityBranch.Message);

        var missingSyncBitsFinality = new Dictionary<string, object?>(sourceEventEvidence.BeaconFinality!);
        missingSyncBitsFinality.Remove("syncCommitteeBits");
        var missingSyncBits = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.ProveInboundToSoraAsync(
                sourceEventEvidence with
                {
                    BeaconFinality = missingSyncBitsFinality,
                    ReceiptProof = receiptProof,
                    ReceiptProofHash = ExpectedReceiptProofHash,
                },
                new InboundProverStub(txHash, ExpectedReceiptProofHash, sourceEventDigest)).AsTask());
        Assert.Contains("beaconFinality.syncCommitteeBits", missingSyncBits.Message);

        var conflictingSyncBitsFinality = new Dictionary<string, object?>(sourceEventEvidence.BeaconFinality!)
        {
            ["sync_committee_bits"] = "0x02" + string.Concat(Enumerable.Repeat("00", 63)),
        };
        var conflictingSyncBits = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.ProveInboundToSoraAsync(
                sourceEventEvidence with
                {
                    BeaconFinality = conflictingSyncBitsFinality,
                    ReceiptProof = receiptProof,
                    ReceiptProofHash = ExpectedReceiptProofHash,
                },
                    new InboundProverStub(txHash, ExpectedReceiptProofHash, sourceEventDigest)).AsTask());
        Assert.Contains("beaconFinality.syncCommitteeBits", conflictingSyncBits.Message);

        var mismatchedSyncParticipationFinality = new Dictionary<string, object?>(sourceEventEvidence.BeaconFinality!)
        {
            ["syncCommitteeParticipation"] = "341",
        };
        var mismatchedSyncParticipation = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.ProveInboundToSoraAsync(
                sourceEventEvidence with
                {
                    BeaconFinality = mismatchedSyncParticipationFinality,
                    ReceiptProof = receiptProof,
                    ReceiptProofHash = ExpectedReceiptProofHash,
                },
                new InboundProverStub(txHash, ExpectedReceiptProofHash, sourceEventDigest)).AsTask());
        Assert.Contains("beaconFinality.syncCommitteeParticipation", mismatchedSyncParticipation.Message);

        var underQuorumSyncBitsFinality = new Dictionary<string, object?>(sourceEventEvidence.BeaconFinality!)
        {
            ["syncCommitteeBits"] = "0x01" + string.Concat(Enumerable.Repeat("00", 63)),
            ["syncCommitteeParticipation"] = "1",
        };
        var underQuorumSyncBits = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.ProveInboundToSoraAsync(
                sourceEventEvidence with
                {
                    BeaconFinality = underQuorumSyncBitsFinality,
                    ReceiptProof = receiptProof,
                    ReceiptProofHash = ExpectedReceiptProofHash,
                },
                new InboundProverStub(txHash, ExpectedReceiptProofHash, sourceEventDigest)).AsTask());
        Assert.Contains("beaconFinality.syncCommitteeBits", underQuorumSyncBits.Message);

        var staleSyncSignatureSlotFinality = new Dictionary<string, object?>(sourceEventEvidence.BeaconFinality!)
        {
            ["syncSignatureSlot"] = "31",
        };
        var staleSyncSignatureSlot = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.ProveInboundToSoraAsync(
                sourceEventEvidence with
                {
                    BeaconFinality = staleSyncSignatureSlotFinality,
                    ReceiptProof = receiptProof,
                    ReceiptProofHash = ExpectedReceiptProofHash,
                },
                new InboundProverStub(txHash, ExpectedReceiptProofHash, sourceEventDigest)).AsTask());
        Assert.Contains("beaconFinality.syncSignatureSlot", staleSyncSignatureSlot.Message);

        var zeroSyncCommitteeSignatureFinality = new Dictionary<string, object?>(sourceEventEvidence.BeaconFinality!)
        {
            ["syncCommitteeSignature"] = "0x" + string.Concat(Enumerable.Repeat("00", 96)),
        };
        var zeroSyncCommitteeSignature = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.ProveInboundToSoraAsync(
                sourceEventEvidence with
                {
                    BeaconFinality = zeroSyncCommitteeSignatureFinality,
                    ReceiptProof = receiptProof,
                    ReceiptProofHash = ExpectedReceiptProofHash,
                },
                new InboundProverStub(txHash, ExpectedReceiptProofHash, sourceEventDigest)).AsTask());
        Assert.Contains("beaconFinality.syncCommitteeSignature", zeroSyncCommitteeSignature.Message);

        var aliasOnlyFinality = new Dictionary<string, object?>
        {
            ["execution_block_number"] = "0x1234",
            ["finality_block_hash"] = blockHash,
            ["receipts_root"] = "0x" + string.Concat(Enumerable.Repeat("cc", 32)),
            ["finalized_header_root"] = "0x" + string.Concat(Enumerable.Repeat("dd", 32)),
            ["sync_committee_root"] = "0x" + string.Concat(Enumerable.Repeat("aa", 32)),
            ["beacon_slot"] = "0x20",
            ["finality_branch"] = EthereumFinalityBranch,
            ["sync_committee_bits"] = EthereumSyncCommitteeSupermajorityBits,
            ["sync_committee_signature"] = "0x" + string.Concat(Enumerable.Repeat("34", 96)),
            ["sync_committee_participation"] = EthereumSyncCommitteeSupermajorityParticipation,
            ["signature_slot"] = "65",
            ["extensionWitness"] = "kept",
        };
        var aliasOnlyProof = await EthereumMainnetSccp.ProveInboundToSoraAsync(
            sourceEventEvidence with
            {
                BeaconFinality = aliasOnlyFinality,
                ReceiptProof = receiptProof,
                ReceiptProofHash = ExpectedReceiptProofHash,
            },
            new DelegateInboundProver(aliasEvidence =>
            {
                var finality = Assert.IsAssignableFrom<IReadOnlyDictionary<string, object?>>(
                    aliasEvidence.BeaconFinality);
                Assert.Equal("4660", finality["executionBlockNumber"]);
                Assert.Equal(blockHash, finality["executionBlockHash"]);
                Assert.Equal("0x" + string.Concat(Enumerable.Repeat("cc", 32)), finality["executionReceiptsRoot"]);
                Assert.Equal("0x" + string.Concat(Enumerable.Repeat("dd", 32)), finality["finalizedHeaderRoot"]);
                Assert.Equal("0x" + string.Concat(Enumerable.Repeat("aa", 32)), finality["syncCommitteeRoot"]);
                Assert.Equal("32", finality["beaconSlot"]);
                Assert.Equal(EthereumSyncCommitteeSupermajorityBits, finality["syncCommitteeBits"]);
                Assert.Equal("0x" + string.Concat(Enumerable.Repeat("34", 96)), finality["syncCommitteeSignature"]);
                Assert.Equal(EthereumSyncCommitteeSupermajorityParticipation, finality["syncCommitteeParticipation"]);
                Assert.Equal("65", finality["syncSignatureSlot"]);
                Assert.Equal("kept", finality["extensionWitness"]);
                foreach (var alias in new[]
                {
                    "execution_block_number",
                    "finalityHeight",
                    "finality_block_hash",
                    "receipts_root",
                    "finalized_header_root",
                    "sync_committee_root",
                    "beacon_slot",
                    "finality_branch",
                    "sync_committee_bits",
                    "sync_committee_signature",
                    "sync_committee_participation",
                    "signature_slot",
                })
                {
                    Assert.False(finality.ContainsKey(alias));
                }

                return new byte[] { 4, 5, 6 };
            }));
        Assert.Equal(new byte[] { 4, 5, 6 }, aliasOnlyProof);

        foreach (var (alias, value, label) in new (string, object?, string)[]
        {
            ("finalized_header_root", "0x" + string.Concat(Enumerable.Repeat("13", 32)), "beaconFinality.finalizedHeaderRoot"),
            ("sync_committee_root", "0x" + string.Concat(Enumerable.Repeat("14", 32)), "beaconFinality.syncCommitteeRoot"),
            ("beacon_slot", "33", "beaconFinality.beaconSlot"),
        })
        {
            var conflictingFinality = new Dictionary<string, object?>(sourceEventEvidence.BeaconFinality!)
            {
                [alias] = value,
            };
            var conflictingAlias = await Assert.ThrowsAsync<ArgumentException>(
                () => EthereumMainnetSccp.ProveInboundToSoraAsync(
                    sourceEventEvidence with
                    {
                        BeaconFinality = conflictingFinality,
                        ReceiptProof = receiptProof,
                        ReceiptProofHash = ExpectedReceiptProofHash,
                    },
                    new InboundProverStub(txHash, ExpectedReceiptProofHash, sourceEventDigest)).AsTask());
            Assert.Contains(label, conflictingAlias.Message);
        }

        var proofBytes = await EthereumMainnetSccp.ProveInboundToSoraAsync(
            sourceEventEvidence with
            {
                ReceiptProof = receiptProof,
                ReceiptProofHash = ExpectedReceiptProofHash,
            },
            new InboundProverStub(txHash, ExpectedReceiptProofHash, sourceEventDigest));
        Assert.Equal(new byte[] { 1, 2, 3 }, proofBytes);
        var oversizedInboundProof = Enumerable
            .Repeat((byte)1, EthereumMainnetSccp.NativeRecursiveMaxProofBytes + 1)
            .ToArray();
        var oversizedProof = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.ProveInboundToSoraAsync(
                sourceEventEvidence with
                {
                    ReceiptProof = receiptProof,
                    ReceiptProofHash = ExpectedReceiptProofHash,
                },
                new DelegateInboundProver(_ => oversizedInboundProof)).AsTask());
        Assert.Contains("proofBytes must be at most", oversizedProof.Message);
        var oversizedSubmit = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.SubmitInboundToIrohaAsync(
                oversizedInboundProof,
                new InboundSubmitterStub()).AsTask());
        Assert.Contains("proofBytes must be at most", oversizedSubmit.Message);
        Assert.Equal(
            "submitted",
            await EthereumMainnetSccp.SubmitInboundToIrohaAsync(
                proofBytes,
                new InboundSubmitterStub()));

        var receiptProofEvidence = await EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
            new EthereumMainnetInboundEvidence
            {
                ReceiptProof = receiptProof,
                ReceiptProofHash = ExpectedReceiptProofHash,
            });
        Assert.Equal(ExpectedReceiptProofHash, receiptProofEvidence.ReceiptProofHash);
        Assert.NotSame(receiptProof, receiptProofEvidence.ReceiptProof);
        mutableReceiptProofNode[0] = 0x7f;
        mutableReceiptProofBranch[0] = 0x7f;
        Assert.Equal(new byte[] { 0x01 }, receiptProofEvidence.ReceiptProof!.ReceiptTrieProofNodes[0]);
        Assert.Equal(RepeatByte(0x11, 32), receiptProofEvidence.ReceiptProof.InclusionBranch[0]);
        mutableReceiptProofNode[0] = 0x01;
        mutableReceiptProofBranch[0] = 0x11;
        var receiptProofHashOnlyEvidence = await EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
            new EthereumMainnetInboundEvidence
            {
                ReceiptProofHash = ExpectedReceiptProofHash,
            });
        Assert.Equal(ExpectedReceiptProofHash, receiptProofHashOnlyEvidence.ReceiptProofHash);
        Assert.Null(receiptProofHashOnlyEvidence.ReceiptProof);
        var zeroReceiptProofHash = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    ReceiptProofHash = "0x" + new string('0', 64),
                }).AsTask());
        Assert.Contains("ReceiptProofHash must not be zero", zeroReceiptProofHash.Message);
        var noncanonicalReceiptProofHash = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    ReceiptProofHash = ExpectedReceiptProofHash + " ",
                }).AsTask());
        Assert.Contains(
            "ReceiptProofHash must be canonical lowercase 0x hex",
            noncanonicalReceiptProofHash.Message);
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    ReceiptProof = receiptProof,
                    ReceiptProofHash = "0x" + new string('9', 64),
                }).AsTask());
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    ReceiptProof = receiptProof with { SourceDomain = 2 },
                }).AsTask());

        var typedFinalityProof = await EthereumMainnetSccp.ProveInboundToSoraAsync(
            new EthereumMainnetInboundEvidence
            {
                Receipt = receiptWithSourceEvent,
                Block = block,
                ReceiptProof = receiptProof,
                ReceiptProofHash = ExpectedReceiptProofHash,
                SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
            }.WithBeaconFinalityEvidence(
                beaconFinalityEvidence,
                [
                    new KeyValuePair<string, object?>("finalizedHeaderRoot", "0x" + new string('d', 64)),
                    new KeyValuePair<string, object?>("syncCommitteeRoot", "0x" + new string('a', 64)),
                    new KeyValuePair<string, object?>("finalityBranch", EthereumFinalityBranch),
                ]),
            new InboundProverStub(txHash, ExpectedReceiptProofHash, sourceEventDigest));
        Assert.Equal(new byte[] { 1, 2, 3 }, typedFinalityProof);

        var missingFinalityProver = new CountingInboundProver();
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.ProveInboundToSoraAsync(
                new EthereumMainnetInboundEvidence { Receipt = receipt, Block = block },
                missingFinalityProver).AsTask());
        Assert.Equal(0, missingFinalityProver.Calls);

        var missingReceiptProofProver = new CountingInboundProver();
        var missingReceiptProof = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.ProveInboundToSoraAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = receipt,
                    Block = block,
                    BeaconFinality = beaconFinality,
                    ReceiptProofHash = ExpectedReceiptProofHash,
                },
                missingReceiptProofProver).AsTask());
        Assert.Contains("receiptProof", missingReceiptProof.Message);
        Assert.Equal(0, missingReceiptProofProver.Calls);

        var unanchoredReceiptProofProver = new CountingInboundProver();
        var unanchoredReceiptProof = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.ProveInboundToSoraAsync(
                new EthereumMainnetInboundEvidence
                {
                    BeaconFinality = beaconFinality,
                    ReceiptProof = receiptProof,
                    ReceiptProofHash = ExpectedReceiptProofHash,
                },
                unanchoredReceiptProofProver).AsTask());
        Assert.Contains("receipt source event validation", unanchoredReceiptProof.Message);
        Assert.Equal(0, unanchoredReceiptProofProver.Calls);

        var driftedReceiptProofProver = new CountingInboundProver();
        var driftedReceiptProof = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.ProveInboundToSoraAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = receiptWithSourceEvent,
                    Block = block,
                    BeaconFinality = beaconFinality,
                    ReceiptProof = receiptProof with
                    {
                        ExecutionReceiptsRoot = "0x" + new string('9', 64),
                    },
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                },
                driftedReceiptProofProver).AsTask());
        Assert.Contains("receiptProof.executionReceiptsRoot", driftedReceiptProof.Message);
        Assert.Equal(0, driftedReceiptProofProver.Calls);

        var missingFinalizedRootProver = new CountingInboundProver();
        var missingFinalizedRootFinality = new Dictionary<string, object?>
        {
            ["syncCommitteeRoot"] = "0x" + new string('a', 64),
            ["beaconSlot"] = "0x20",
            ["executionBlockNumber"] = "0x1234",
            ["executionBlockHash"] = blockHash,
            ["executionReceiptsRoot"] = "0x" + new string('c', 64),
        };
        var missingFinalizedRoot = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.ProveInboundToSoraAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = receiptWithSourceEvent,
                    Block = block,
                    BeaconFinality = missingFinalizedRootFinality,
                    ReceiptProof = receiptProof,
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                },
                missingFinalizedRootProver).AsTask());
        Assert.Contains("beaconFinality.finalizedHeaderRoot", missingFinalizedRoot.Message);
        Assert.Equal(0, missingFinalizedRootProver.Calls);

        var missingSyncCommitteeRootProver = new CountingInboundProver();
        var missingSyncCommitteeRootFinality = new Dictionary<string, object?>
        {
            ["finalizedHeaderRoot"] = "0x" + new string('d', 64),
            ["beaconSlot"] = "0x20",
            ["executionBlockNumber"] = "0x1234",
            ["executionBlockHash"] = blockHash,
            ["executionReceiptsRoot"] = "0x" + new string('c', 64),
        };
        var missingSyncCommitteeRoot = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.ProveInboundToSoraAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = receiptWithSourceEvent,
                    Block = block,
                    BeaconFinality = missingSyncCommitteeRootFinality,
                    ReceiptProof = receiptProof,
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                },
                missingSyncCommitteeRootProver).AsTask());
        Assert.Contains("beaconFinality.syncCommitteeRoot", missingSyncCommitteeRoot.Message);
        Assert.Equal(0, missingSyncCommitteeRootProver.Calls);

        var missingBeaconSlotProver = new CountingInboundProver();
        var missingBeaconSlotFinality = new Dictionary<string, object?>
        {
            ["finalizedHeaderRoot"] = "0x" + new string('d', 64),
            ["syncCommitteeRoot"] = "0x" + new string('a', 64),
            ["executionBlockNumber"] = "0x1234",
            ["executionBlockHash"] = blockHash,
            ["executionReceiptsRoot"] = "0x" + new string('c', 64),
        };
        var missingBeaconSlot = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.ProveInboundToSoraAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = receiptWithSourceEvent,
                    Block = block,
                    BeaconFinality = missingBeaconSlotFinality,
                    ReceiptProof = receiptProof,
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                },
                missingBeaconSlotProver).AsTask());
        Assert.Contains("beaconFinality.beaconSlot", missingBeaconSlot.Message);
        Assert.Equal(0, missingBeaconSlotProver.Calls);

        var driftedFinalizedRootProofProver = new CountingInboundProver();
        var driftedFinalizedRootProof = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.ProveInboundToSoraAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = receiptWithSourceEvent,
                    Block = block,
                    BeaconFinality = beaconFinality,
                    ReceiptProof = receiptProof with
                    {
                        BeaconFinalizedRoot = "0x" + new string('9', 64),
                    },
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                },
                driftedFinalizedRootProofProver).AsTask());
        Assert.Contains("receiptProof.beaconFinalizedRoot", driftedFinalizedRootProof.Message);
        Assert.Equal(0, driftedFinalizedRootProofProver.Calls);

        var driftedSyncCommitteeRootProofProver = new CountingInboundProver();
        var driftedSyncCommitteeRootProof = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.ProveInboundToSoraAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = receiptWithSourceEvent,
                    Block = block,
                    BeaconFinality = beaconFinality,
                    ReceiptProof = receiptProof with
                    {
                        SyncCommitteeRoot = "0x" + new string('9', 64),
                    },
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                },
                driftedSyncCommitteeRootProofProver).AsTask());
        Assert.Contains("receiptProof.syncCommitteeRoot", driftedSyncCommitteeRootProof.Message);
        Assert.Equal(0, driftedSyncCommitteeRootProofProver.Calls);

        var driftedBeaconSlotProofProver = new CountingInboundProver();
        var driftedBeaconSlotProof = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.ProveInboundToSoraAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = receiptWithSourceEvent,
                    Block = block,
                    BeaconFinality = beaconFinality,
                    ReceiptProof = receiptProof with
                    {
                        BeaconSlot = receiptProof.BeaconSlot + 1,
                    },
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                },
                driftedBeaconSlotProofProver).AsTask());
        Assert.Contains("receiptProof.beaconSlot", driftedBeaconSlotProof.Message);
        Assert.Equal(0, driftedBeaconSlotProofProver.Calls);

        await Assert.ThrowsAsync<ArgumentOutOfRangeException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence { Receipt = receipt },
                new ExecutionProviderStub("0x38", receipt, block)).AsTask());
        foreach (var chainId in new object?[]
        {
            "0x01", "1", "0X1", " 0x1", "0x1 ", 1,
        })
        {
            await Assert.ThrowsAsync<ArgumentException>(
                () => EthereumMainnetSccp.ValidateExecutionProviderMainnetAsync(
                    new ExecutionProviderStub(chainId!, receipt, block)).AsTask());
        }

        var failedReceipt = new Dictionary<string, object?>(receipt)
        {
            ["status"] = "0x0",
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence { Receipt = failedReceipt, Block = block }).AsTask());

        var missingReceiptBlockNumber = new Dictionary<string, object?>(receipt);
        missingReceiptBlockNumber.Remove("blockNumber");
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = missingReceiptBlockNumber,
                    Block = block,
                }).AsTask());

        var zeroReceiptBlockNumber = new Dictionary<string, object?>(receipt)
        {
            ["blockNumber"] = "0x0",
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = zeroReceiptBlockNumber,
                    Block = block,
                }).AsTask());

        var missingProvider = await Assert.ThrowsAsync<InvalidOperationException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence { TransactionHash = txHash }).AsTask());
        Assert.Contains("execution provider", missingProvider.Message);

        var driftedReceipt = new Dictionary<string, object?>(receipt)
        {
            ["transactionHash"] = "0x" + new string('d', 64),
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    TransactionHash = txHash,
                    Receipt = driftedReceipt,
                    Block = block,
                }).AsTask());

        var driftedBlock = new Dictionary<string, object?>(block)
        {
            ["hash"] = "0x" + new string('e', 64),
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence { Receipt = receipt, Block = driftedBlock }).AsTask());

        var missingBlockNumber = new Dictionary<string, object?>(block);
        missingBlockNumber.Remove("number");
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence { Receipt = receipt, Block = missingBlockNumber }).AsTask());

        var zeroBlockNumber = new Dictionary<string, object?>(block)
        {
            ["number"] = "0x0",
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence { Receipt = receipt, Block = zeroBlockNumber }).AsTask());

        var uppercaseReceipt = new Dictionary<string, object?>(receipt)
        {
            ["transactionHash"] = txHash.ToUpperInvariant(),
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence { Receipt = uppercaseReceipt, Block = block }).AsTask());

        var driftedFinalityHash = new Dictionary<string, object?>(beaconFinality)
        {
            ["executionBlockHash"] = "0x" + new string('e', 64),
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = receipt,
                    Block = block,
                    BeaconFinality = driftedFinalityHash,
                }).AsTask());

        var driftedFinalityNumber = new Dictionary<string, object?>(beaconFinality)
        {
            ["executionBlockNumber"] = "0x1235",
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = receipt,
                    Block = block,
                    BeaconFinality = driftedFinalityNumber,
                }).AsTask());

        var driftedFinalityReceiptsRoot = new Dictionary<string, object?>(beaconFinality)
        {
            ["executionReceiptsRoot"] = "0x" + new string('e', 64),
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = receipt,
                    Block = block,
                    BeaconFinality = driftedFinalityReceiptsRoot,
                }).AsTask());

        var missingReceiptRootBlock = new Dictionary<string, object?>(block);
        missingReceiptRootBlock.Remove("receiptsRoot");
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = receipt,
                    Block = missingReceiptRootBlock,
                }).AsTask());

        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = receiptWithSourceEvent,
                    Block = block,
                    BeaconFinality = beaconFinality,
                    SourceEventDigest = sourceEventDigest,
                }).AsTask());

        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = receipt,
                    Block = block,
                    BeaconFinality = beaconFinality,
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                }).AsTask());

        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = receiptWithSourceEvent,
                    Block = block,
                    BeaconFinality = beaconFinality,
                    SourceBridgeEmitterAddress = "0x" + string.Concat(Enumerable.Repeat("13", 20)),
                }).AsTask());

        var wrongTopicLog = new Dictionary<string, object?>(sourceEventLog)
        {
            ["topics"] = new object?[] { "0x" + new string('a', 64), sourceEventDigest },
        };
        var wrongTopicReceipt = new Dictionary<string, object?>(receipt)
        {
            ["logs"] = new object?[] { wrongTopicLog },
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = wrongTopicReceipt,
                    Block = block,
                    BeaconFinality = beaconFinality,
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                }).AsTask());

        var extraTopicLog = new Dictionary<string, object?>(sourceEventLog)
        {
            ["topics"] = new object?[]
            {
                EthereumMainnetSccp.SourceEventTopic,
                sourceEventDigest,
                "0x" + new string('6', 64),
            },
        };
        var extraTopicReceipt = new Dictionary<string, object?>(receipt)
        {
            ["logs"] = new object?[] { extraTopicLog },
        };
        var extraTopic = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = extraTopicReceipt,
                    Block = block,
                    BeaconFinality = beaconFinality,
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                }).AsTask());
        Assert.Contains("exactly 2 topics", extraTopic.Message);

        var nonEmptyDataLog = new Dictionary<string, object?>(sourceEventLog)
        {
            ["data"] = "0x01",
        };
        var nonEmptyDataReceipt = new Dictionary<string, object?>(receipt)
        {
            ["logs"] = new object?[] { nonEmptyDataLog },
        };
        var nonEmptyData = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = nonEmptyDataReceipt,
                    Block = block,
                    BeaconFinality = beaconFinality,
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                }).AsTask());
        Assert.Contains("data must be 0x", nonEmptyData.Message);

        var zeroDigestLog = new Dictionary<string, object?>(sourceEventLog)
        {
            ["topics"] = new object?[]
            {
                EthereumMainnetSccp.SourceEventTopic,
                "0x" + new string('0', 64),
            },
        };
        var zeroDigestReceipt = new Dictionary<string, object?>(receipt)
        {
            ["logs"] = new object?[] { zeroDigestLog },
        };
        var zeroDigest = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = zeroDigestReceipt,
                    Block = block,
                    BeaconFinality = beaconFinality,
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                }).AsTask());
        Assert.Contains("digest must not be zero", zeroDigest.Message);

        var duplicateReceipt = new Dictionary<string, object?>(receipt)
        {
            ["logs"] = new object?[] { sourceEventLog, sourceEventLog },
        };
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = duplicateReceipt,
                    Block = block,
                    BeaconFinality = beaconFinality,
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                }).AsTask());

        var removedLog = new Dictionary<string, object?>(sourceEventLog)
        {
            ["removed"] = true,
        };
        var removedReceipt = new Dictionary<string, object?>(receipt)
        {
            ["logs"] = new object?[] { removedLog },
        };
        var removedSourceEventLog = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = removedReceipt,
                    Block = block,
                    BeaconFinality = beaconFinality,
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                }).AsTask());
        Assert.Contains("removed logs", removedSourceEventLog.Message);

        var nonObjectLogReceipt = new Dictionary<string, object?>(receipt)
        {
            ["logs"] = new object?[] { "not-a-log" },
        };
        var nonObjectLog = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = nonObjectLogReceipt,
                    Block = block,
                    BeaconFinality = beaconFinality,
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                }).AsTask());
        Assert.Contains("receipt.logs[0] must be an object", nonObjectLog.Message);

        var missingDataLog = new Dictionary<string, object?>(sourceEventLog);
        missingDataLog.Remove("data");
        var missingDataReceipt = new Dictionary<string, object?>(receipt)
        {
            ["logs"] = new object?[] { missingDataLog },
        };
        var missingData = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = missingDataReceipt,
                    Block = block,
                    BeaconFinality = beaconFinality,
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                }).AsTask());
        Assert.Contains("receipt.logs[0].data", missingData.Message);

        foreach (var missingField in new[] { "transactionHash", "blockHash", "blockNumber" })
        {
            var missingContextLog = new Dictionary<string, object?>(sourceEventLog);
            missingContextLog.Remove(missingField);
            var missingContextReceipt = new Dictionary<string, object?>(receipt)
            {
                ["logs"] = new object?[] { missingContextLog },
            };
            var missingContext = await Assert.ThrowsAsync<ArgumentException>(
                () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                    new EthereumMainnetInboundEvidence
                    {
                        Receipt = missingContextReceipt,
                        Block = block,
                        BeaconFinality = beaconFinality,
                        SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                    }).AsTask());
            Assert.Contains($"receipt.logs[0].{missingField}", missingContext.Message);
        }

        foreach (var (alias, value, label) in new (string, object?, string)[]
        {
            ("transaction_hash", "0x" + new string('d', 64), "receipt.logs[0].transactionHash"),
            ("block_hash", "0x" + new string('a', 64), "receipt.logs[0].blockHash"),
            ("block_number", "0x1235", "receipt.logs[0].blockNumber"),
        })
        {
            var conflictingContextLog = new Dictionary<string, object?>(sourceEventLog)
            {
                [alias] = value,
            };
            var conflictingContextReceipt = new Dictionary<string, object?>(receipt)
            {
                ["logs"] = new object?[] { conflictingContextLog },
            };
            var conflictingContext = await Assert.ThrowsAsync<ArgumentException>(
                () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                    new EthereumMainnetInboundEvidence
                    {
                        Receipt = conflictingContextReceipt,
                        Block = block,
                        BeaconFinality = beaconFinality,
                        SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                    }).AsTask());
            Assert.Contains(label, conflictingContext.Message);
        }

        var driftedLogTransaction = new Dictionary<string, object?>(sourceEventLog)
        {
            ["transactionHash"] = "0x" + new string('d', 64),
        };
        var driftedLogTransactionReceipt = new Dictionary<string, object?>(receipt)
        {
            ["logs"] = new object?[] { driftedLogTransaction },
        };
        var driftedLogTransactionError = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = driftedLogTransactionReceipt,
                    Block = block,
                    BeaconFinality = beaconFinality,
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                }).AsTask());
        Assert.Contains("transactionHash must match", driftedLogTransactionError.Message);

        var driftedLogBlockHash = new Dictionary<string, object?>(sourceEventLog)
        {
            ["blockHash"] = "0x" + new string('a', 64),
        };
        var driftedLogBlockHashReceipt = new Dictionary<string, object?>(receipt)
        {
            ["logs"] = new object?[] { driftedLogBlockHash },
        };
        var driftedLogBlockHashError = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = driftedLogBlockHashReceipt,
                    Block = block,
                    BeaconFinality = beaconFinality,
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                }).AsTask());
        Assert.Contains("blockHash must match", driftedLogBlockHashError.Message);

        var driftedLogBlockNumber = new Dictionary<string, object?>(sourceEventLog)
        {
            ["blockNumber"] = "0x1235",
        };
        var driftedLogBlockNumberReceipt = new Dictionary<string, object?>(receipt)
        {
            ["logs"] = new object?[] { driftedLogBlockNumber },
        };
        var driftedLogBlockNumberError = await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = driftedLogBlockNumberReceipt,
                    Block = block,
                    BeaconFinality = beaconFinality,
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                }).AsTask());
        Assert.Contains("blockNumber must match", driftedLogBlockNumberError.Message);

        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.SubmitInboundToIrohaAsync(
                [0, 0],
                new InboundSubmitterStub()).AsTask());
    }

    [Fact]
    public async Task InboundCallbacksReceiveSnapshotEvidence()
    {
        var txHash = "0x" + new string('a', 64);
        var blockHash = "0x" + new string('b', 64);
        var receiptsRoot = "0x" + new string('c', 64);
        var sourceEventDigest = "0x" + new string('1', 64);
        var logAddress = "0x" + new string('f', 40);
        var finalizedRoot = "0x" + new string('d', 64);
        var syncCommitteeRoot = "0x" + new string('e', 64);
        var logTopics = new object?[] { EthereumMainnetSccp.SourceEventTopic, sourceEventDigest };
        var logMetadata = new Dictionary<string, object?>
        {
            ["address"] = logAddress,
            ["transactionHash"] = txHash,
            ["blockHash"] = blockHash,
            ["blockNumber"] = "0x1234",
            ["topics"] = logTopics,
            ["data"] = "0x",
        };
        var receipt = new Dictionary<string, object?>
        {
            ["status"] = "0x1",
            ["transactionHash"] = txHash,
            ["blockHash"] = blockHash,
            ["blockNumber"] = "0x1234",
            ["logs"] = new object?[] { logMetadata },
        };
        var block = new Dictionary<string, object?>
        {
            ["hash"] = blockHash,
            ["number"] = "0x1234",
            ["receiptsRoot"] = receiptsRoot,
        };
        var beaconFinality = new EthereumMainnetBeaconFinalityEvidence(
            "0x1234",
            blockHash,
            receiptsRoot,
            BeaconSlot: "32",
            SyncCommitteeBits: EthereumSyncCommitteeSupermajorityBits,
            SyncCommitteeSignature: "0x" + string.Concat(Enumerable.Repeat("34", 96)),
            SyncCommitteeParticipation: EthereumSyncCommitteeSupermajorityParticipation,
            SyncSignatureSlot: "65").ToDictionary(
                [
                    new KeyValuePair<string, object?>("finalizedHeaderRoot", finalizedRoot),
                    new KeyValuePair<string, object?>("syncCommitteeRoot", syncCommitteeRoot),
                    new KeyValuePair<string, object?>("finalityBranch", EthereumFinalityBranch),
                ]);
        var receiptProof = new EthereumMainnetReceiptProof
        {
            SourceEventDigest = sourceEventDigest,
            BeaconSlot = 32,
            ExecutionBlockNumber = 0x1234,
            ExecutionBlockHash = blockHash,
            ExecutionReceiptsRoot = receiptsRoot,
            BeaconFinalizedRoot = finalizedRoot,
            SyncCommitteeRoot = syncCommitteeRoot,
            ReceiptRootIndex = 0,
            ReceiptTrieProofNodes = [new byte[] { 0x01 }],
            InclusionBranch = [RepeatByte(0x11, 32)],
        };

        var collected = await EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
            new EthereumMainnetInboundEvidence { TransactionHash = txHash },
            new ExecutionProviderStub("0x1", receipt, block),
            new MutatingConsensusProviderStub(receipt, block, beaconFinality));
        Assert.Equal("0x1", collected.Receipt?["status"]);
        Assert.Equal(receiptsRoot, collected.Block?["receiptsRoot"]);
        Assert.Equal("0x1", receipt["status"]);
        Assert.Equal(receiptsRoot, block["receiptsRoot"]);
        Assert.Equal(logAddress, logMetadata["address"]);
        Assert.Equal(EthereumMainnetSccp.SourceEventTopic, Assert.IsType<string>(logTopics[0]));
        Assert.Equal(sourceEventDigest, Assert.IsType<string>(logTopics[1]));

        var proofBytes = await EthereumMainnetSccp.ProveInboundToSoraAsync(
            new EthereumMainnetInboundEvidence
            {
                Receipt = receipt,
                Block = block,
                BeaconFinality = beaconFinality,
                ReceiptProof = receiptProof,
                SourceBridgeEmitterAddress = logAddress,
            },
            new MutatingInboundProverStub(receipt, block, beaconFinality, txHash));
        Assert.Equal(new byte[] { 1, 2, 3 }, proofBytes);
        Assert.Equal("0x1", receipt["status"]);
        Assert.Equal(receiptsRoot, block["receiptsRoot"]);
        Assert.Equal(blockHash, beaconFinality["executionBlockHash"]);
        Assert.Equal(logAddress, logMetadata["address"]);
        Assert.Equal(EthereumMainnetSccp.SourceEventTopic, Assert.IsType<string>(logTopics[0]));
        Assert.Equal(sourceEventDigest, Assert.IsType<string>(logTopics[1]));
    }

    [Fact]
    public async Task InboundProverReceivesCallbackEvidenceSnapshot()
    {
        var txHash = "0x" + new string('a', 64);
        var blockHash = "0x" + new string('b', 64);
        var sourceEventDigest = "0x" + new string('e', 64);
        var sourceBridgeEmitterAddress = "0x" + string.Concat(Enumerable.Repeat("44", 20));
        var receiptsRoot = "0x" + new string('c', 64);
        var finalizedRoot = "0x" + new string('d', 64);
        var syncCommitteeRoot = "0x" + new string('a', 64);
        var receiptNested = new Dictionary<string, object?>(StringComparer.Ordinal)
        {
            ["value"] = "keep",
            ["bytes"] = new byte[] { 0xbb },
        };
        var receiptWitness = new List<object?> { receiptNested };
        var blockWitness = new Dictionary<string, object?>(StringComparer.Ordinal)
        {
            ["value"] = "block",
            ["bytes"] = new byte[] { 0xcc },
        };
        var finalityBranchWitness = EthereumFinalityBranch.ToList();
        var finalityBytes = new byte[] { 0xaa };
        var finalityWitness = new Dictionary<string, object?>(StringComparer.Ordinal)
        {
            ["branch"] = finalityBranchWitness,
            ["bytes"] = finalityBytes,
        };
        var blockReceiptsWitness = new List<object?> { "receipt-list" };
        var sourceEventLog = new Dictionary<string, object?>(StringComparer.Ordinal)
        {
            ["address"] = sourceBridgeEmitterAddress,
            ["transactionHash"] = txHash,
            ["blockHash"] = blockHash,
            ["blockNumber"] = "0x1234",
            ["topics"] = new object?[] { EthereumMainnetSccp.SourceEventTopic, sourceEventDigest },
            ["data"] = "0x",
        };
        var receipt = new Dictionary<string, object?>(StringComparer.Ordinal)
        {
            ["transactionHash"] = txHash,
            ["blockHash"] = blockHash,
            ["blockNumber"] = "0x1234",
            ["status"] = "0x1",
            ["logs"] = new object?[] { sourceEventLog },
            ["mutableWitness"] = receiptWitness,
        };
        var block = new Dictionary<string, object?>(StringComparer.Ordinal)
        {
            ["hash"] = blockHash,
            ["number"] = "0x1234",
            ["receiptsRoot"] = receiptsRoot,
            ["mutableWitness"] = blockWitness,
        };
        var beaconFinality = new Dictionary<string, object?>(StringComparer.Ordinal)
        {
            ["executionBlockNumber"] = "0x1234",
            ["executionBlockHash"] = blockHash,
            ["executionReceiptsRoot"] = receiptsRoot,
            ["finalizedHeaderRoot"] = finalizedRoot,
            ["syncCommitteeRoot"] = syncCommitteeRoot,
            ["beaconSlot"] = "0x20",
            ["finalityBranch"] = EthereumFinalityBranch,
            ["syncCommitteeBits"] = EthereumSyncCommitteeSupermajorityBits,
            ["syncCommitteeSignature"] = "0x" + string.Concat(Enumerable.Repeat("34", 96)),
            ["syncCommitteeParticipation"] = EthereumSyncCommitteeSupermajorityParticipation,
            ["syncSignatureSlot"] = "65",
            ["mutableWitness"] = finalityWitness,
        };
        var blockReceipt = new Dictionary<string, object?>(receipt, StringComparer.Ordinal)
        {
            ["mutableWitness"] = blockReceiptsWitness,
        };
        var mutableReceiptProofNode = new byte[] { 0x01, 0x02 };
        var mutableReceiptProofBranch = RepeatByte(0x11, 32);
        var mutableInputBranch = new byte[] { 0x44 };
        var receiptProof = new EthereumMainnetReceiptProof
        {
            SourceEventDigest = sourceEventDigest,
            BeaconSlot = 32,
            ExecutionBlockNumber = 0x1234,
            ExecutionBlockHash = blockHash,
            ExecutionReceiptsRoot = receiptsRoot,
            BeaconFinalizedRoot = finalizedRoot,
            SyncCommitteeRoot = syncCommitteeRoot,
            ReceiptRootIndex = 0,
            ReceiptTrieProofNodes = [mutableReceiptProofNode],
            InclusionBranch = [mutableReceiptProofBranch],
        };
        var receiptProofHash = EthereumMainnetSccp.EvmSccpReceiptProofHash(
            receiptProof.SourceEventDigest,
            receiptProof.BeaconSlot,
            receiptProof.ExecutionBlockNumber,
            receiptProof.ExecutionBlockHash,
            receiptProof.ExecutionReceiptsRoot,
            receiptProof.BeaconFinalizedRoot,
            receiptProof.SyncCommitteeRoot,
            receiptProof.ReceiptRootIndex,
            receiptProof.ReceiptTrieProofNodes,
            receiptProof.InclusionBranch);

        var proofBytes = await EthereumMainnetSccp.ProveInboundToSoraAsync(
            new EthereumMainnetInboundEvidence
            {
                Receipt = receipt,
                Block = block,
                BeaconFinality = beaconFinality,
                ReceiptProof = receiptProof,
                ReceiptProofHash = receiptProofHash,
                SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                BlockReceipts = [blockReceipt],
                InclusionBranch = [mutableInputBranch],
            },
            new DelegateInboundProver(evidence =>
            {
                receiptWitness.Add("changed");
                receiptNested["value"] = "changed";
                ((byte[])receiptNested["bytes"]!)[0] = 0x7f;
                blockWitness["value"] = "changed";
                ((byte[])blockWitness["bytes"]!)[0] = 0x7e;
                finalityBranchWitness.Add("0x" + new string('9', 64));
                finalityBytes[0] = 0x7d;
                finalityWitness["new"] = "changed";
                blockReceiptsWitness.Add("changed");
                mutableReceiptProofNode[0] = 0x7c;
                mutableReceiptProofBranch[0] = 0x7b;
                mutableInputBranch[0] = 0x45;

                var receiptSnapshot = Assert.IsAssignableFrom<IReadOnlyList<object?>>(
                    evidence.Receipt?["mutableWitness"]);
                Assert.Single(receiptSnapshot);
                var receiptNestedSnapshot = Assert.IsAssignableFrom<IReadOnlyDictionary<string, object?>>(
                    receiptSnapshot[0]);
                Assert.Equal("keep", receiptNestedSnapshot["value"]);
                Assert.Equal(new byte[] { 0xbb }, Assert.IsType<byte[]>(receiptNestedSnapshot["bytes"]));

                var blockSnapshot = Assert.IsAssignableFrom<IReadOnlyDictionary<string, object?>>(
                    evidence.Block?["mutableWitness"]);
                Assert.Equal("block", blockSnapshot["value"]);
                Assert.Equal(new byte[] { 0xcc }, Assert.IsType<byte[]>(blockSnapshot["bytes"]));

                var finalitySnapshot = Assert.IsAssignableFrom<IReadOnlyDictionary<string, object?>>(
                    evidence.BeaconFinality?["mutableWitness"]);
                var branchSnapshot = Assert.IsAssignableFrom<IReadOnlyList<object?>>(finalitySnapshot["branch"]);
                Assert.Equal(EthereumFinalityBranch.Length, branchSnapshot.Count);
                Assert.Equal(EthereumFinalityBranch[0], branchSnapshot[0]);
                Assert.Equal(new byte[] { 0xaa }, Assert.IsType<byte[]>(finalitySnapshot["bytes"]));

                Assert.NotNull(evidence.BlockReceipts);
                var blockReceiptWitnessSnapshot = Assert.IsAssignableFrom<IReadOnlyList<object?>>(
                    evidence.BlockReceipts![0]["mutableWitness"]);
                Assert.Equal(new object?[] { "receipt-list" }, blockReceiptWitnessSnapshot);

                Assert.NotNull(evidence.InclusionBranch);
                Assert.Equal(new byte[] { 0x44 }, evidence.InclusionBranch![0]);
                Assert.NotNull(evidence.ReceiptProof);
                Assert.Equal(new byte[] { 0x01, 0x02 }, evidence.ReceiptProof!.ReceiptTrieProofNodes[0]);
                Assert.Equal(RepeatByte(0x11, 32), evidence.ReceiptProof.InclusionBranch[0]);
                Assert.Equal(receiptProofHash, evidence.ReceiptProofHash);
                return new byte[] { 9, 8, 7 };
            }));

        Assert.Equal(new byte[] { 9, 8, 7 }, proofBytes);
    }

    [Fact]
    public async Task CollectInboundEvidenceSnapshotsConsensusBoundary()
    {
        var txHash = "0x" + new string('a', 64);
        var blockHash = "0x" + new string('b', 64);
        var sourceEventDigest = "0x" + new string('e', 64);
        var sourceBridgeEmitterAddress = "0x" + string.Concat(Enumerable.Repeat("44", 20));
        var receiptsRoot = "0x" + new string('c', 64);
        var finalizedRoot = "0x" + new string('d', 64);
        var syncCommitteeRoot = "0x" + new string('a', 64);
        var receiptNested = new Dictionary<string, object?>(StringComparer.Ordinal)
        {
            ["value"] = "keep",
            ["bytes"] = new byte[] { 0xbb },
        };
        var receiptWitness = new List<object?> { receiptNested };
        var blockWitness = new Dictionary<string, object?>(StringComparer.Ordinal)
        {
            ["value"] = "block",
            ["bytes"] = new byte[] { 0xcc },
        };
        var finalityBranchWitness = EthereumFinalityBranch.ToList();
        var finalityBytes = new byte[] { 0xaa };
        var finalityWitness = new Dictionary<string, object?>(StringComparer.Ordinal)
        {
            ["branch"] = finalityBranchWitness,
            ["bytes"] = finalityBytes,
        };
        var sourceEventLog = new Dictionary<string, object?>(StringComparer.Ordinal)
        {
            ["address"] = sourceBridgeEmitterAddress,
            ["transactionHash"] = txHash,
            ["blockHash"] = blockHash,
            ["blockNumber"] = "0x1234",
            ["topics"] = new object?[] { EthereumMainnetSccp.SourceEventTopic, sourceEventDigest },
            ["data"] = "0x",
        };
        var receipt = new Dictionary<string, object?>(StringComparer.Ordinal)
        {
            ["transactionHash"] = txHash,
            ["blockHash"] = blockHash,
            ["blockNumber"] = "0x1234",
            ["status"] = "0x1",
            ["logs"] = new object?[] { sourceEventLog },
            ["mutableWitness"] = receiptWitness,
        };
        var block = new Dictionary<string, object?>(StringComparer.Ordinal)
        {
            ["hash"] = blockHash,
            ["number"] = "0x1234",
            ["receiptsRoot"] = receiptsRoot,
            ["mutableWitness"] = blockWitness,
        };
        var beaconFinality = new Dictionary<string, object?>(StringComparer.Ordinal)
        {
            ["executionBlockNumber"] = "0x1234",
            ["executionBlockHash"] = blockHash,
            ["executionReceiptsRoot"] = receiptsRoot,
            ["finalizedHeaderRoot"] = finalizedRoot,
            ["syncCommitteeRoot"] = syncCommitteeRoot,
            ["beaconSlot"] = "0x20",
            ["finalityBranch"] = EthereumFinalityBranch,
            ["syncCommitteeBits"] = EthereumSyncCommitteeSupermajorityBits,
            ["syncCommitteeSignature"] = "0x" + string.Concat(Enumerable.Repeat("34", 96)),
            ["syncCommitteeParticipation"] = EthereumSyncCommitteeSupermajorityParticipation,
            ["syncSignatureSlot"] = "65",
            ["mutableWitness"] = finalityWitness,
        };
        var consensusCalls = 0;
        var consensusProvider = new DelegateConsensusProvider((collectedReceipt, collectedBlock, transactionHash) =>
        {
            consensusCalls++;
            Assert.Equal(txHash, transactionHash);
            Assert.NotSame(receiptWitness, collectedReceipt?["mutableWitness"]);
            var receiptSnapshot = Assert.IsAssignableFrom<IReadOnlyList<object?>>(
                collectedReceipt?["mutableWitness"]);
            var receiptNestedSnapshot = Assert.IsAssignableFrom<IReadOnlyDictionary<string, object?>>(
                receiptSnapshot[0]);
            Assert.Equal("keep", receiptNestedSnapshot["value"]);
            Assert.Equal(new byte[] { 0xbb }, Assert.IsType<byte[]>(receiptNestedSnapshot["bytes"]));
            Assert.NotSame(blockWitness, collectedBlock?["mutableWitness"]);
            var blockSnapshot = Assert.IsAssignableFrom<IReadOnlyDictionary<string, object?>>(
                collectedBlock?["mutableWitness"]);
            Assert.Equal("block", blockSnapshot["value"]);
            Assert.Equal(new byte[] { 0xcc }, Assert.IsType<byte[]>(blockSnapshot["bytes"]));

            receiptWitness.Add("changed");
            receiptNested["value"] = "changed";
            ((byte[])receiptNested["bytes"]!)[0] = 0x7f;
            blockWitness["value"] = "changed";
            ((byte[])blockWitness["bytes"]!)[0] = 0x7e;
            return beaconFinality;
        });

        var evidence = await EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
            new EthereumMainnetInboundEvidence
            {
                Receipt = receipt,
                Block = block,
                SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
            },
            consensusProvider: consensusProvider);
        finalityBranchWitness.Add("0x" + new string('9', 64));
        finalityBytes[0] = 0x7d;
        finalityWitness["new"] = "changed";

        Assert.Equal(1, consensusCalls);
        var returnedReceiptSnapshot = Assert.IsAssignableFrom<IReadOnlyList<object?>>(
            evidence.Receipt?["mutableWitness"]);
        Assert.Single(returnedReceiptSnapshot);
        var returnedReceiptNested = Assert.IsAssignableFrom<IReadOnlyDictionary<string, object?>>(
            returnedReceiptSnapshot[0]);
        Assert.Equal("keep", returnedReceiptNested["value"]);
        Assert.Equal(new byte[] { 0xbb }, Assert.IsType<byte[]>(returnedReceiptNested["bytes"]));
        var returnedBlockSnapshot = Assert.IsAssignableFrom<IReadOnlyDictionary<string, object?>>(
            evidence.Block?["mutableWitness"]);
        Assert.Equal("block", returnedBlockSnapshot["value"]);
        Assert.Equal(new byte[] { 0xcc }, Assert.IsType<byte[]>(returnedBlockSnapshot["bytes"]));
        var returnedFinalitySnapshot = Assert.IsAssignableFrom<IReadOnlyDictionary<string, object?>>(
            evidence.BeaconFinality?["mutableWitness"]);
        var branchSnapshot = Assert.IsAssignableFrom<IReadOnlyList<object?>>(returnedFinalitySnapshot["branch"]);
        Assert.Equal(EthereumFinalityBranch.Length, branchSnapshot.Count);
        Assert.Equal(EthereumFinalityBranch[0], branchSnapshot[0]);
        Assert.Equal(new byte[] { 0xaa }, Assert.IsType<byte[]>(returnedFinalitySnapshot["bytes"]));
        Assert.False(returnedFinalitySnapshot.ContainsKey("new"));
    }

    [Fact]
    public async Task BscMainnetCollectInboundEvidenceSnapshotsConsensusBoundary()
    {
        var txHash = "0x" + new string('a', 64);
        var blockHash = "0x" + new string('b', 64);
        var sourceEventDigest = "0x" + new string('e', 64);
        var sourceBridgeEmitterAddress = "0x" + string.Concat(Enumerable.Repeat("44", 20));
        var receiptsRoot = "0x" + new string('c', 64);
        var validatorSetHash = "0x" + string.Concat(Enumerable.Repeat("ab", 32));
        var commitSealHash = "0x" + new string('d', 64);
        var receiptNested = new Dictionary<string, object?>(StringComparer.Ordinal)
        {
            ["value"] = "keep",
            ["bytes"] = new byte[] { 0xbb },
        };
        var receiptWitness = new List<object?> { receiptNested };
        var blockWitness = new Dictionary<string, object?>(StringComparer.Ordinal)
        {
            ["value"] = "block",
            ["bytes"] = new byte[] { 0xcc },
        };
        var finalityBranchWitness = new List<object?> { validatorSetHash };
        var finalityBytes = new byte[] { 0xaa };
        var finalityWitness = new Dictionary<string, object?>(StringComparer.Ordinal)
        {
            ["branch"] = finalityBranchWitness,
            ["bytes"] = finalityBytes,
        };
        var sourceEventLog = new Dictionary<string, object?>(StringComparer.Ordinal)
        {
            ["address"] = sourceBridgeEmitterAddress,
            ["transactionHash"] = txHash,
            ["blockHash"] = blockHash,
            ["blockNumber"] = "0x1234",
            ["topics"] = new object?[] { BscMainnetSccp.SourceEventTopic, sourceEventDigest },
            ["data"] = "0x",
        };
        var receipt = new Dictionary<string, object?>(StringComparer.Ordinal)
        {
            ["transactionHash"] = txHash,
            ["blockHash"] = blockHash,
            ["blockNumber"] = "0x1234",
            ["status"] = "0x1",
            ["logs"] = new object?[] { sourceEventLog },
            ["mutableWitness"] = receiptWitness,
        };
        var block = new Dictionary<string, object?>(StringComparer.Ordinal)
        {
            ["hash"] = blockHash,
            ["number"] = "0x1234",
            ["receiptsRoot"] = receiptsRoot,
            ["mutableWitness"] = blockWitness,
        };
        var parliaFinality = new Dictionary<string, object?>(StringComparer.Ordinal)
        {
            ["executionBlockNumber"] = "0x1234",
            ["executionBlockHash"] = blockHash,
            ["executionReceiptsRoot"] = receiptsRoot,
            ["validatorEpoch"] = "0x24",
            ["validatorSetHash"] = validatorSetHash,
            ["commitSealHash"] = commitSealHash,
            ["mutableWitness"] = finalityWitness,
        };
        var consensusCalls = 0;
        var consensusProvider = new DelegateBscConsensusProvider((collectedReceipt, collectedBlock, transactionHash) =>
        {
            consensusCalls++;
            Assert.Equal(txHash, transactionHash);
            Assert.NotSame(receiptWitness, collectedReceipt?["mutableWitness"]);
            var receiptSnapshot = Assert.IsAssignableFrom<IReadOnlyList<object?>>(
                collectedReceipt?["mutableWitness"]);
            var receiptNestedSnapshot = Assert.IsAssignableFrom<IReadOnlyDictionary<string, object?>>(
                receiptSnapshot[0]);
            Assert.Equal("keep", receiptNestedSnapshot["value"]);
            Assert.Equal(new byte[] { 0xbb }, Assert.IsType<byte[]>(receiptNestedSnapshot["bytes"]));
            Assert.NotSame(blockWitness, collectedBlock?["mutableWitness"]);
            var blockSnapshot = Assert.IsAssignableFrom<IReadOnlyDictionary<string, object?>>(
                collectedBlock?["mutableWitness"]);
            Assert.Equal("block", blockSnapshot["value"]);
            Assert.Equal(new byte[] { 0xcc }, Assert.IsType<byte[]>(blockSnapshot["bytes"]));

            receiptWitness.Add("changed");
            receiptNested["value"] = "changed";
            ((byte[])receiptNested["bytes"]!)[0] = 0x7f;
            blockWitness["value"] = "changed";
            ((byte[])blockWitness["bytes"]!)[0] = 0x7e;
            return parliaFinality;
        });

        var evidence = await BscMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
            new BscMainnetInboundEvidence
            {
                Receipt = receipt,
                Block = block,
                SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
            },
            consensusProvider: consensusProvider);
        finalityBranchWitness.Add("0x" + new string('9', 64));
        finalityBytes[0] = 0x7d;
        finalityWitness["new"] = "changed";

        Assert.Equal(1, consensusCalls);
        var returnedReceiptSnapshot = Assert.IsAssignableFrom<IReadOnlyList<object?>>(
            evidence.Receipt?["mutableWitness"]);
        Assert.Single(returnedReceiptSnapshot);
        var returnedReceiptNested = Assert.IsAssignableFrom<IReadOnlyDictionary<string, object?>>(
            returnedReceiptSnapshot[0]);
        Assert.Equal("keep", returnedReceiptNested["value"]);
        Assert.Equal(new byte[] { 0xbb }, Assert.IsType<byte[]>(returnedReceiptNested["bytes"]));
        var returnedBlockSnapshot = Assert.IsAssignableFrom<IReadOnlyDictionary<string, object?>>(
            evidence.Block?["mutableWitness"]);
        Assert.Equal("block", returnedBlockSnapshot["value"]);
        Assert.Equal(new byte[] { 0xcc }, Assert.IsType<byte[]>(returnedBlockSnapshot["bytes"]));
        var returnedFinalitySnapshot = Assert.IsAssignableFrom<IReadOnlyDictionary<string, object?>>(
            evidence.ParliaFinality?["mutableWitness"]);
        var branchSnapshot = Assert.IsAssignableFrom<IReadOnlyList<object?>>(returnedFinalitySnapshot["branch"]);
        Assert.Equal(new object?[] { validatorSetHash }, branchSnapshot);
        Assert.Equal(new byte[] { 0xaa }, Assert.IsType<byte[]>(returnedFinalitySnapshot["bytes"]));
        Assert.False(returnedFinalitySnapshot.ContainsKey("new"));
    }

    [Fact]
    public async Task BeaconRestConsensusProviderCollectsFinalizedTargetEvidence()
    {
        var txHash = "0x" + new string('a', 64);
        var blockHash = "0x" + new string('b', 64);
        var receipt = new Dictionary<string, object?>
        {
            ["transactionHash"] = txHash,
            ["blockHash"] = blockHash,
            ["blockNumber"] = "0x1234",
            ["status"] = "0x1",
        };
        var block = new Dictionary<string, object?>
        {
            ["hash"] = blockHash,
            ["number"] = "0x1234",
            ["receiptsRoot"] = "0x" + new string('c', 64),
            ["beaconSlot"] = "64",
        };
        var transport = new BeaconRestTransportStub((url, _) => url switch
        {
            "https://beacon.example/eth/v1/beacon/headers/finalized" =>
                BeaconResponse(BeaconHeaderJson()),
            "https://beacon.example/eth/v1/beacon/headers/64" =>
                BeaconResponse(BeaconHeaderJson()),
            "https://beacon.example/eth/v1/beacon/blocks/64/root" =>
                BeaconResponse(BeaconBlockRootJson()),
            "https://beacon.example/eth/v2/beacon/blocks/64" =>
                BeaconResponse(BeaconBlockJson(slot: "64", blockHash: blockHash)),
            "https://beacon.example/eth/v1/beacon/states/finalized/finality_checkpoints" =>
                BeaconResponse(BeaconCheckpointJson()),
            "https://beacon.example/eth/v1/beacon/light_client/finality_update" =>
                BeaconResponse(BeaconFinalityUpdateJson()),
            _ => throw new InvalidOperationException($"unexpected Beacon REST URL {url}"),
        });
        var syncCommitteePayload = SampleSyncCommitteePayload();
        Assert.Equal(81925, syncCommitteePayload.Length);
        Assert.Equal(
            ExpectedSyncCommitteeRoot,
            EthereumMainnetSccp.EthSyncCommitteeHashFromPayload(syncCommitteePayload));
        var provider = new EthereumMainnetBeaconRestConsensusProvider(
            "https://beacon.example/eth/v1",
            null,
            syncCommitteePayload,
            new Dictionary<string, string> { ["Authorization"] = "Bearer local" },
            transport: transport);

        var evidence = await EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
            new EthereumMainnetInboundEvidence { Receipt = receipt, Block = block },
            consensusProvider: provider);

        Assert.Equal("4660", evidence.BeaconFinality?["executionBlockNumber"]);
        Assert.Equal(blockHash, evidence.BeaconFinality?["executionBlockHash"]);
        Assert.Equal("0x" + new string('c', 64), evidence.BeaconFinality?["executionReceiptsRoot"]);
        Assert.Equal(BeaconHeaderRootSlot64, evidence.BeaconFinality?["finalizedHeaderRoot"]);
        Assert.Equal(ExpectedSyncCommitteeRoot, evidence.BeaconFinality?["syncCommitteeRoot"]);
        Assert.Equal("64", evidence.BeaconFinality?["beaconSlot"]);
        Assert.Equal(EthereumFinalityBranch, Assert.IsAssignableFrom<IReadOnlyList<string>>(evidence.BeaconFinality?["finalityBranch"]));
        Assert.Equal(EthereumSyncCommitteeSupermajorityBits, evidence.BeaconFinality?["syncCommitteeBits"]);
        Assert.Equal("0x" + string.Concat(Enumerable.Repeat("34", 96)), evidence.BeaconFinality?["syncCommitteeSignature"]);
        Assert.Equal(EthereumSyncCommitteeSupermajorityParticipation, evidence.BeaconFinality?["syncCommitteeParticipation"]);
        Assert.Equal("65", evidence.BeaconFinality?["syncSignatureSlot"]);
        Assert.Equal(
            [
                "https://beacon.example/eth/v1/beacon/headers/finalized",
                "https://beacon.example/eth/v1/beacon/headers/64",
                "https://beacon.example/eth/v1/beacon/blocks/64/root",
                "https://beacon.example/eth/v2/beacon/blocks/64",
                "https://beacon.example/eth/v1/beacon/states/finalized/finality_checkpoints",
                "https://beacon.example/eth/v1/beacon/light_client/finality_update",
            ],
            transport.Calls);
        Assert.Equal("Bearer local", transport.HeaderCalls[0]["Authorization"]);
    }

    [Fact]
    public async Task BeaconRestConsensusProviderDerivesTargetSlotFromTimestamp()
    {
        var txHash = "0x" + new string('a', 64);
        var blockHash = "0x" + new string('b', 64);
        var receipt = new Dictionary<string, object?>
        {
            ["transactionHash"] = txHash,
            ["blockHash"] = blockHash,
            ["blockNumber"] = "0x1234",
            ["status"] = "0x1",
        };
        var block = new Dictionary<string, object?>
        {
            ["hash"] = blockHash,
            ["number"] = "0x1234",
            ["receiptsRoot"] = "0x" + new string('c', 64),
            ["timestamp"] = "0x364",
        };
        var transport = new BeaconRestTransportStub((url, _) => url switch
        {
            "https://beacon.example/eth/v1/beacon/genesis" =>
                BeaconResponse(BeaconGenesisJson("100")),
            "https://beacon.example/eth/v1/beacon/headers/finalized" =>
                BeaconResponse(BeaconHeaderJson()),
            "https://beacon.example/eth/v1/beacon/headers/64" =>
                BeaconResponse(BeaconHeaderJson()),
            "https://beacon.example/eth/v1/beacon/blocks/64/root" =>
                BeaconResponse(BeaconBlockRootJson()),
            "https://beacon.example/eth/v2/beacon/blocks/64" =>
                BeaconResponse(BeaconBlockJson(slot: "64", blockHash: blockHash)),
            "https://beacon.example/eth/v1/beacon/states/finalized/finality_checkpoints" =>
                BeaconResponse(BeaconCheckpointJson()),
            "https://beacon.example/eth/v1/beacon/light_client/finality_update" =>
                BeaconResponse(BeaconFinalityUpdateJson()),
            _ => throw new InvalidOperationException($"unexpected Beacon REST URL {url}"),
        });
        var provider = new EthereumMainnetBeaconRestConsensusProvider(
            "https://beacon.example/eth/v1",
            "0x" + new string('e', 64),
            transport: transport);

        var evidence = await EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
            new EthereumMainnetInboundEvidence { Receipt = receipt, Block = block },
            consensusProvider: provider);

        Assert.Equal(BeaconHeaderRootSlot64, evidence.BeaconFinality?["finalizedHeaderRoot"]);
        Assert.Equal("64", evidence.BeaconFinality?["beaconSlot"]);
        Assert.Equal(
            [
                "https://beacon.example/eth/v1/beacon/genesis",
                "https://beacon.example/eth/v1/beacon/headers/finalized",
                "https://beacon.example/eth/v1/beacon/headers/64",
                "https://beacon.example/eth/v1/beacon/blocks/64/root",
                "https://beacon.example/eth/v2/beacon/blocks/64",
                "https://beacon.example/eth/v1/beacon/states/finalized/finality_checkpoints",
                "https://beacon.example/eth/v1/beacon/light_client/finality_update",
            ],
            transport.Calls);
    }

    [Fact]
    public async Task BeaconRestHttpTransportRejectsOversizedBodies()
    {
        using var client = new HttpClient(new BeaconRestHttpHandlerStub(request =>
        {
            Assert.Equal(HttpMethod.Get, request.Method);
            Assert.True(request.Headers.TryGetValues("Authorization", out var values));
            Assert.Contains("Bearer local", values);
            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new UnknownLengthContent(new byte[1024 * 1024 + 1]),
            };
        }));
        var transport = new EthereumMainnetBeaconRestHttpTransport(client);

        var oversized = await Assert.ThrowsAsync<ArgumentException>(
            () => transport.GetAsync(
                    "https://beacon.example/oversized",
                    new Dictionary<string, string> { ["Authorization"] = "Bearer local" })
                .AsTask());

        Assert.Contains("response body must be at most", oversized.Message);
    }

    [Fact]
    public async Task BeaconRestConsensusProviderRejectsUnsafeFinality()
    {
        var block = new Dictionary<string, object?>
        {
            ["hash"] = "0x" + new string('b', 64),
            ["number"] = "0x1234",
            ["receiptsRoot"] = "0x" + new string('c', 64),
        };

        var missingBlock = await Assert.ThrowsAsync<ArgumentException>(
            () => BeaconRestProvider(BeaconResponse(BeaconHeaderJson()), BeaconResponse(BeaconCheckpointJson()))
                .CollectFinalityEvidenceAsync(null, null, null).AsTask());
        Assert.Contains("requires block", missingBlock.Message);

        var failedHeader = await Assert.ThrowsAsync<ArgumentException>(
            () => BeaconRestProvider(
                    new EthereumMainnetBeaconRestResponse(
                        503,
                        Encoding.UTF8.GetBytes("{}"),
                        "Unavailable"),
                    BeaconResponse(BeaconCheckpointJson()))
                .CollectFinalityEvidenceAsync(null, block, null).AsTask());
        Assert.Contains("request failed 503 Unavailable", failedHeader.Message);

        var oversizedHeader = await Assert.ThrowsAsync<ArgumentException>(
            () => BeaconRestProvider(
                    new EthereumMainnetBeaconRestResponse(
                        200,
                        new byte[1024 * 1024 + 1]),
                    BeaconResponse(BeaconCheckpointJson()))
                .CollectFinalityEvidenceAsync(null, block, null).AsTask());
        Assert.Contains("response body must be at most", oversizedHeader.Message);

        var nonObjectHeader = await Assert.ThrowsAsync<ArgumentException>(
            () => BeaconRestProvider(
                    BeaconResponse("[]"),
                    BeaconResponse(BeaconCheckpointJson()))
                .CollectFinalityEvidenceAsync(null, block, null).AsTask());
        Assert.Contains("response JSON must be an object", nonObjectHeader.Message);

        var historicalBlock = new Dictionary<string, object?>(block)
        {
            ["beaconSlot"] = "32",
        };
        var historicalTransport = new BeaconRestTransportStub((url, _) => url switch
        {
            "https://beacon.example/eth/v1/beacon/headers/finalized" =>
                BeaconResponse(BeaconHeaderJson()),
            "https://beacon.example/eth/v1/beacon/headers/32" =>
                BeaconResponse(BeaconHeaderJson(rootNibble: 'a', slot: "32")),
            _ => throw new InvalidOperationException($"unexpected Beacon REST URL {url}"),
        });
        var historicalProvider = new EthereumMainnetBeaconRestConsensusProvider(
            "https://beacon.example/eth/v1",
            "0x" + new string('e', 64),
            transport: historicalTransport);
        var historicalTarget = await Assert.ThrowsAsync<ArgumentException>(
            () => historicalProvider.CollectFinalityEvidenceAsync(null, historicalBlock, null).AsTask());
        Assert.Contains("historical target blocks require an ancestry proof", historicalTarget.Message);

        var optimisticHeader = await Assert.ThrowsAsync<ArgumentException>(
            () => BeaconRestProvider(
                    BeaconResponse(BeaconHeaderJson(executionOptimistic: true)),
                    BeaconResponse(BeaconCheckpointJson()))
                .CollectFinalityEvidenceAsync(null, block, null).AsTask());
        Assert.Contains("must not be execution optimistic", optimisticHeader.Message);

        var malformedOptimisticHeader = await Assert.ThrowsAsync<ArgumentException>(
            () => BeaconRestProvider(
                    BeaconResponse(
                        BeaconHeaderJson()
                            .Replace(
                                "\"execution_optimistic\": false",
                                "\"execution_optimistic\": \"false\"",
                                StringComparison.Ordinal)),
                    BeaconResponse(BeaconCheckpointJson()))
                .CollectFinalityEvidenceAsync(null, block, null).AsTask());
        Assert.Contains("execution_optimistic must be a boolean", malformedOptimisticHeader.Message);

        var malformedFinalizedHeader = await Assert.ThrowsAsync<ArgumentException>(
            () => BeaconRestProvider(
                    BeaconResponse(
                        BeaconHeaderJson()
                            .Replace(
                                "\"finalized\": true",
                                "\"finalized\": \"true\"",
                                StringComparison.Ordinal)),
                    BeaconResponse(BeaconCheckpointJson()))
                .CollectFinalityEvidenceAsync(null, block, null).AsTask());
        Assert.Contains("finalized must be a boolean", malformedFinalizedHeader.Message);

        var malformedCanonicalHeader = await Assert.ThrowsAsync<ArgumentException>(
            () => BeaconRestProvider(
                    BeaconResponse(
                        BeaconHeaderJson()
                            .Replace(
                                "\"canonical\": true",
                                "\"canonical\": \"true\"",
                                StringComparison.Ordinal)),
                    BeaconResponse(BeaconCheckpointJson()))
                .CollectFinalityEvidenceAsync(null, block, null).AsTask());
        Assert.Contains("canonical must be a boolean", malformedCanonicalHeader.Message);

        foreach (var (field, bytePair) in new[]
        {
            ("parent_root", "01"),
            ("state_root", "02"),
            ("body_root", "03"),
        })
        {
            var malformedRoot = await Assert.ThrowsAsync<ArgumentException>(
                () => BeaconRestProvider(
                        BeaconResponse(
                            BeaconHeaderJson()
                                .Replace(
                                    $"\"{field}\": \"0x{string.Concat(Enumerable.Repeat(bytePair, 32))}\"",
                                    $"\"{field}\": \"0x\"",
                                    StringComparison.Ordinal)),
                        BeaconResponse(BeaconCheckpointJson()))
                    .CollectFinalityEvidenceAsync(null, block, null).AsTask());
            Assert.Contains(field, malformedRoot.Message);
        }

        var malformedSignature = await Assert.ThrowsAsync<ArgumentException>(
            () => BeaconRestProvider(
                    BeaconResponse(
                        BeaconHeaderJson()
                            .Replace(
                                $"\"signature\": \"0x{string.Concat(Enumerable.Repeat("12", 96))}\"",
                                $"\"signature\": \"0x{string.Concat(Enumerable.Repeat("12", 95))}\"",
                                StringComparison.Ordinal)),
                    BeaconResponse(BeaconCheckpointJson()))
                .CollectFinalityEvidenceAsync(null, block, null).AsTask());
        Assert.Contains("signature", malformedSignature.Message);

        var driftedBlockRoot = await Assert.ThrowsAsync<ArgumentException>(
            () => BeaconRestProvider(
                    BeaconResponse(BeaconHeaderJson()),
                    BeaconResponse(BeaconBlockRootJson('9')),
                    BeaconResponse(BeaconBlockJson()),
                    BeaconResponse(BeaconCheckpointJson()))
                .CollectFinalityEvidenceAsync(null, block, null).AsTask());
        Assert.Contains("finalized block root must match finalized header root", driftedBlockRoot.Message);

        var driftedPayloadSlot = await Assert.ThrowsAsync<ArgumentException>(
            () => BeaconRestProvider(
                    BeaconResponse(BeaconHeaderJson()),
                    BeaconResponse(BeaconBlockJson(slot: "65")),
                    BeaconResponse(BeaconCheckpointJson()))
                .CollectFinalityEvidenceAsync(null, block, null).AsTask());
        Assert.Contains("finalized block slot must match finalized header slot", driftedPayloadSlot.Message);

        var driftedPayloadBlockHash = await Assert.ThrowsAsync<ArgumentException>(
            () => BeaconRestProvider(
                    BeaconResponse(BeaconHeaderJson()),
                    BeaconResponse(BeaconBlockJson(blockHash: "0x" + new string('9', 64))),
                    BeaconResponse(BeaconCheckpointJson()))
                .CollectFinalityEvidenceAsync(null, block, null).AsTask());
        Assert.Contains("execution payload block_hash must match block.hash", driftedPayloadBlockHash.Message);

        var driftedPayloadBlockNumber = await Assert.ThrowsAsync<ArgumentException>(
            () => BeaconRestProvider(
                    BeaconResponse(BeaconHeaderJson()),
                    BeaconResponse(BeaconBlockJson(blockNumber: "4661")),
                    BeaconResponse(BeaconCheckpointJson()))
                .CollectFinalityEvidenceAsync(null, block, null).AsTask());
        Assert.Contains("execution payload block_number must match block.number", driftedPayloadBlockNumber.Message);

        var driftedPayloadReceiptsRoot = await Assert.ThrowsAsync<ArgumentException>(
            () => BeaconRestProvider(
                    BeaconResponse(BeaconHeaderJson()),
                    BeaconResponse(BeaconBlockJson(receiptsRoot: "0x" + new string('9', 64))),
                    BeaconResponse(BeaconCheckpointJson()))
                .CollectFinalityEvidenceAsync(null, block, null).AsTask());
        Assert.Contains(
            "execution payload receipts_root must match block.receiptsRoot",
            driftedPayloadReceiptsRoot.Message);

        var unfinalizedHeader = await Assert.ThrowsAsync<ArgumentException>(
            () => BeaconRestProvider(
                    BeaconResponse(BeaconHeaderJson(finalized: false)),
                    BeaconResponse(BeaconCheckpointJson()))
                .CollectFinalityEvidenceAsync(null, block, null).AsTask());
        Assert.Contains("must be finalized", unfinalizedHeader.Message);

        var checkpointMismatch = await Assert.ThrowsAsync<ArgumentException>(
            () => BeaconRestProvider(
                    BeaconResponse(BeaconHeaderJson()),
                    BeaconResponse(BeaconCheckpointJson('9')))
                .CollectFinalityEvidenceAsync(null, block, null).AsTask());
        Assert.Contains("checkpoint root must match", checkpointMismatch.Message);

        var emptySyncAggregate = await Assert.ThrowsAsync<ArgumentException>(
            () => BeaconRestProvider(
                    BeaconResponse(BeaconHeaderJson()),
                    BeaconResponse(BeaconBlockRootJson()),
                    BeaconResponse(BeaconBlockJson()),
                    BeaconResponse(BeaconCheckpointJson()),
                    BeaconResponse(BeaconFinalityUpdateJson(syncCommitteeBits: "0x" + new string('0', 128))))
                .CollectFinalityEvidenceAsync(null, block, null).AsTask());
        Assert.Contains("sync_committee_bits must contain at least one participant", emptySyncAggregate.Message);

        var underQuorumSyncAggregate = await Assert.ThrowsAsync<ArgumentException>(
            () => BeaconRestProvider(
                    BeaconResponse(BeaconHeaderJson()),
                    BeaconResponse(BeaconBlockRootJson()),
                    BeaconResponse(BeaconBlockJson()),
                    BeaconResponse(BeaconCheckpointJson()),
                    BeaconResponse(BeaconFinalityUpdateJson(syncCommitteeBits: "0x01" + string.Concat(Enumerable.Repeat("00", 63)))))
                .CollectFinalityEvidenceAsync(null, block, null).AsTask());
        Assert.Contains("sync_committee_bits must contain Ethereum sync committee supermajority", underQuorumSyncAggregate.Message);

        var missingFinalityBranch = await Assert.ThrowsAsync<ArgumentException>(
            () => BeaconRestProvider(
                    BeaconResponse(BeaconHeaderJson()),
                    BeaconResponse(BeaconBlockRootJson()),
                    BeaconResponse(BeaconBlockJson()),
                    BeaconResponse(BeaconCheckpointJson()),
                    BeaconResponse(BeaconFinalityUpdateJson(includeFinalityBranch: false)))
                .CollectFinalityEvidenceAsync(null, block, null).AsTask());
        Assert.Contains("finality_branch", missingFinalityBranch.Message);

        var malformedFinalityBranch = await Assert.ThrowsAsync<ArgumentException>(
            () => BeaconRestProvider(
                    BeaconResponse(BeaconHeaderJson()),
                    BeaconResponse(BeaconBlockRootJson()),
                    BeaconResponse(BeaconBlockJson()),
                    BeaconResponse(BeaconCheckpointJson()),
                    BeaconResponse(BeaconFinalityUpdateJson(finalityBranch: EthereumFinalityBranch.Take(5).ToArray())))
                .CollectFinalityEvidenceAsync(null, block, null).AsTask());
        Assert.Contains("finality_branch", malformedFinalityBranch.Message);

        var zeroSyncAggregateSignature = await Assert.ThrowsAsync<ArgumentException>(
            () => BeaconRestProvider(
                    BeaconResponse(BeaconHeaderJson()),
                    BeaconResponse(BeaconBlockRootJson()),
                    BeaconResponse(BeaconBlockJson()),
                    BeaconResponse(BeaconCheckpointJson()),
                    BeaconResponse(BeaconFinalityUpdateJson(syncCommitteeSignature: "0x" + new string('0', 192))))
                .CollectFinalityEvidenceAsync(null, block, null).AsTask());
        Assert.Contains("sync_committee_signature must not be zero", zeroSyncAggregateSignature.Message);

        var missingSyncRoot = Assert.Throws<ArgumentException>(
            () => new EthereumMainnetBeaconRestConsensusProvider(
                "https://beacon.example",
                syncCommitteeRoot: null,
                syncCommitteePayload: null));
        Assert.Contains("requires syncCommitteeRoot or syncCommitteePayload", missingSyncRoot.Message);

        var malformedPayload = Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.EthSyncCommitteeHashFromPayload([0]));
        Assert.Contains("syncCommitteePayload must have version 1", malformedPayload.Message);

        var compressedPayload = Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.EthSyncCommitteeHashFromPayload(CompressedSyncCommitteePayload()));
        Assert.Contains("syncCommitteePayload must contain exactly 512 entries", compressedPayload.Message);

        var weightedPayload = Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.EthSyncCommitteeHashFromPayload(WeightedSyncCommitteePayload()));
        Assert.Contains("syncCommitteeWeights[0] must be 1 for Ethereum mainnet", weightedPayload.Message);

        var syncCommitteePayload = SampleSyncCommitteePayload();
        var mismatch = Assert.Throws<ArgumentException>(
            () => new EthereumMainnetBeaconRestConsensusProvider(
                "https://beacon.example",
                "0x" + new string('9', 64),
                syncCommitteePayload));
        Assert.Contains("syncCommitteeRoot must match syncCommitteePayload", mismatch.Message);
    }

    [Fact]
    public async Task OutboundProofRequestCalldataAndSubmitUseEthereumMainnetBinding()
    {
        var binding = SampleDestinationBinding();
        var publicInputs = SamplePublicInputs();
        var input = SampleOutboundInput(binding, publicInputs);

        var request = EthereumMainnetSccp.BuildOutboundProofRequest(input);
        Assert.Equal(1, request.Version);
        Assert.Equal(EthereumMainnetSccp.EvmGroth16Bn254ProofBackend, request.Backend);
        Assert.Equal(EthereumMainnetSccp.DomainSora, request.SourceDomain);
        Assert.Equal(EthereumMainnetSccp.DomainEthereum, request.TargetDomain);
        Assert.Equal(ExpectedBindingHash, request.DestinationBindingHash);
        Assert.Equal(ExpectedRequestHash, request.RequestHash);
        Assert.Equal(ExpectedPublicInputsBytes, Convert.ToHexString(request.PublicInputsBytes).ToLowerInvariant());
        Assert.Equal(ExpectedPublicSignalWords, request.PublicSignalWords);
        Assert.Equal(SampleOutboundBundleBytes(), request.BundleBytes);
        Assert.Empty(request.SourceProofBytes);
        Assert.NotSame(input.BundleBytes, request.BundleBytes);

        var artifactInput = input with
        {
            ProofArtifactHash = "0x" + new string('9', 64),
            ProvingKeyHash = "0x" + new string('a', 64),
        };
        var artifactRequest = EthereumMainnetSccp.BuildOutboundProofRequest(artifactInput);
        Assert.Equal("0x" + new string('9', 64), artifactRequest.ProofArtifactHash);
        Assert.Equal("0x" + new string('a', 64), artifactRequest.ProvingKeyHash);
        Assert.NotEqual(ExpectedRequestHash, artifactRequest.RequestHash);
        var artifactResult = EthereumMainnetSccp.WrapOutboundProofResult(Groth16ProofBytes(), artifactRequest);
        Assert.Equal(artifactRequest.ProofArtifactHash, artifactResult.ProofArtifactHash);
        Assert.Equal(artifactRequest.ProvingKeyHash, artifactResult.ProvingKeyHash);

        var nativeProverBundle = SampleNativeEvmProverBundle(ExpectedBindingHash);
        var parsedNativeProverBundle = EthereumMainnetNativeEvmProverBundle.FromJson(
            SampleNativeEvmProverBundleJson(ExpectedBindingHash),
            ExpectedBindingHash);
        Assert.Equal(nativeProverBundle.ProofArtifactHash, parsedNativeProverBundle.ProofArtifactHash);
        Assert.Equal("artifacts/eth-mainnet/proof-artifact.bin", parsedNativeProverBundle.ProofArtifact);
        Assert.Equal(nativeProverBundle.ProvingKeyHash, parsedNativeProverBundle.ProvingKeyHash);
        Assert.Equal("artifacts/eth-mainnet/proving-key.bin", parsedNativeProverBundle.ProvingKey);
        Assert.Equal("artifacts/eth-mainnet/verifier-key.bin", parsedNativeProverBundle.VerifierKey);
        Assert.Equal(nativeProverBundle.DestinationBindingHash, parsedNativeProverBundle.DestinationBindingHash);
        Assert.Equal(
            nativeProverBundle.NativeSdkArtifacts.Select(artifact => artifact.Sdk),
            parsedNativeProverBundle.NativeSdkArtifacts.Select(artifact => artifact.Sdk));
        Assert.Contains(
            parsedNativeProverBundle.NativeSdkArtifacts,
            artifact => artifact.Sdk == "dotnet"
                && artifact.ImplementationArtifact == "artifacts/eth-mainnet/dotnet-implementation.bin");
        var parityFixture = EthereumMainnetNativeEvmProverParityFixture.FromJson(
            SampleNativeEvmProverParityFixtureJson(nativeProverBundle),
            nativeProverBundle);
        Assert.Equal(EthereumMainnetSccp.EthNativeEvmProverParityFixtureSchemaV1, parityFixture.Schema);
        Assert.Equal(ExpectedBindingHash, parityFixture.DestinationBindingHash);
        Assert.Equal(9, parityFixture.PublicSignalWords.Count);
        Assert.Equal(
            parityFixture.ToriiSubmitPayloadHash,
            parityFixture.SdkResults["dotnet"].ToriiSubmitPayloadHash);
        Assert.Contains(
            "sdkResults.dotnet",
            Assert.Throws<ArgumentException>(
                () => EthereumMainnetNativeEvmProverParityFixture.FromJson(
                    SampleNativeEvmProverParityFixtureJson(
                        nativeProverBundle,
                        dotnetCalldataHash: "0x" + new string('9', 64)),
                    nativeProverBundle)).Message);
        Assert.Contains(
            "duplicate JSON key: schema",
            Assert.Throws<ArgumentException>(
                () => EthereumMainnetNativeEvmProverParityFixture.FromJson(
                    SampleNativeEvmProverParityFixtureJson(nativeProverBundle).Replace(
                        "\"schema\": \""
                            + EthereumMainnetSccp.EthNativeEvmProverParityFixtureSchemaV1
                            + "\"",
                        "\"schema\": \"forged\", \"schema\": \""
                            + EthereumMainnetSccp.EthNativeEvmProverParityFixtureSchemaV1
                            + "\"",
                        StringComparison.Ordinal),
                    nativeProverBundle)).Message);
        var selfTestFixture = EthereumMainnetNativeEvmProverSelfTestFixture.FromJson(
            SampleNativeEvmProverSelfTestFixtureJson(nativeProverBundle),
            nativeProverBundle);
        Assert.Equal(EthereumMainnetSccp.EthNativeEvmProverSelfTestSchemaV1, selfTestFixture.Schema);
        Assert.Equal(ExpectedBindingHash, selfTestFixture.DestinationBindingHash);
        Assert.Equal(9, selfTestFixture.PublicSignalWords.Count);
        Assert.Equal(
            selfTestFixture.ProofHash,
            selfTestFixture.SdkResults["dotnet"].ProofHash);
        Assert.Contains(
            "sdkResults.dotnet",
            Assert.Throws<ArgumentException>(
                () => EthereumMainnetNativeEvmProverSelfTestFixture.FromJson(
                    SampleNativeEvmProverSelfTestFixtureJson(
                        nativeProverBundle,
                        dotnetProofHash: "0x" + new string('9', 64)),
                    nativeProverBundle)).Message);
        Assert.Contains(
            "duplicate JSON key: schema",
            Assert.Throws<ArgumentException>(
                () => EthereumMainnetNativeEvmProverSelfTestFixture.FromJson(
                    SampleNativeEvmProverSelfTestFixtureJson(nativeProverBundle).Replace(
                        "\"schema\": \""
                            + EthereumMainnetSccp.EthNativeEvmProverSelfTestSchemaV1
                            + "\"",
                        "\"schema\": \"forged\", \"schema\": \""
                            + EthereumMainnetSccp.EthNativeEvmProverSelfTestSchemaV1
                            + "\"",
                        StringComparison.Ordinal),
                    nativeProverBundle)).Message);
        Assert.Contains(
            "noWasm",
            Assert.Throws<ArgumentException>(
                () => EthereumMainnetNativeEvmProverBundle.FromJson(
                    SampleNativeEvmProverBundleJson(ExpectedBindingHash, noWasm: false),
                    ExpectedBindingHash)).Message);
        Assert.Contains(
            "destinationBindingHash",
            Assert.Throws<ArgumentException>(
                () => EthereumMainnetNativeEvmProverBundle.FromJson(
                    SampleNativeEvmProverBundleJson("0x" + new string('b', 64)),
                    ExpectedBindingHash)).Message);
        var noncanonicalDomain = Assert.Throws<ArgumentException>(
            () => EthereumMainnetNativeEvmProverBundle.FromJson(
                SampleNativeEvmProverBundleJson(ExpectedBindingHash).Replace(
                    "\"domain\": 1",
                    "\"domain\": \"01\"",
                    StringComparison.Ordinal),
                ExpectedBindingHash));
        Assert.Contains("domain", noncanonicalDomain.Message);
        Assert.Contains("canonical decimal integer", noncanonicalDomain.Message);
        var duplicateJsonKey = Assert.Throws<ArgumentException>(
            () => EthereumMainnetNativeEvmProverBundle.FromJson(
                SampleNativeEvmProverBundleJson(ExpectedBindingHash).Replace(
                    "\"bundle_id\": \"sccp:eth:native-evm-groth16-prover:ethereum-mainnet:v1\"",
                    "\"bundle_id\": \"forged\", \"bundle_id\": \"sccp:eth:native-evm-groth16-prover:ethereum-mainnet:v1\"",
                    StringComparison.Ordinal),
                ExpectedBindingHash));
        Assert.Contains("duplicate JSON key: bundle_id", duplicateJsonKey.Message);
        Assert.Contains(
            "proofArtifact",
            Assert.Throws<ArgumentException>(
                () => EthereumMainnetNativeEvmProverBundle.FromJson(
                    SampleNativeEvmProverBundleJson(
                        ExpectedBindingHash,
                        proofArtifact: "../proof-artifact.bin"),
                    ExpectedBindingHash)).Message);
        var uriArtifactPath = Assert.Throws<ArgumentException>(
            () => EthereumMainnetNativeEvmProverBundle.FromJson(
                SampleNativeEvmProverBundleJson(
                    ExpectedBindingHash,
                    proofArtifact: "ipfs:proof-artifact.bin"),
                ExpectedBindingHash));
        Assert.Contains("proofArtifact", uriArtifactPath.Message);
        Assert.Contains("URI schemes", uriArtifactPath.Message);
        var wasmArtifactPath = Assert.Throws<ArgumentException>(
            () => EthereumMainnetNativeEvmProverBundle.FromJson(
                SampleNativeEvmProverBundleJson(
                    ExpectedBindingHash,
                    proofArtifact: "artifacts/eth-mainnet/proof.wasm"),
                ExpectedBindingHash));
        Assert.Contains("proofArtifact", wasmArtifactPath.Message);
        Assert.Contains("forbidden prover dependency marker: wasm", wasmArtifactPath.Message);
        var unknownManifestField = Assert.Throws<ArgumentException>(
            () => EthereumMainnetNativeEvmProverBundle.FromJson(
                SampleNativeEvmProverBundleJson(ExpectedBindingHash).Replace(
                    "\"audit_hashes\":",
                    "\"experimental_manifest_note\": true, \"audit_hashes\":",
                    StringComparison.Ordinal),
                ExpectedBindingHash));
        Assert.Contains("nativeProverBundle", unknownManifestField.Message);
        Assert.Contains("experimental_manifest_note", unknownManifestField.Message);
        Assert.Contains("unknown field", unknownManifestField.Message);
        var duplicateManifestAlias = Assert.Throws<ArgumentException>(
            () => EthereumMainnetNativeEvmProverBundle.FromJson(
                SampleNativeEvmProverBundleJson(ExpectedBindingHash).Replace(
                    "\"proof_artifact_hash\": \"0x" + new string('9', 64) + "\"",
                    "\"proofArtifactHash\": \"0x" + new string('9', 64)
                        + "\", \"proof_artifact_hash\": \"0x" + new string('9', 64) + "\"",
                    StringComparison.Ordinal),
                ExpectedBindingHash));
        Assert.Contains("proofArtifactHash", duplicateManifestAlias.Message);
        Assert.Contains("multiple aliases", duplicateManifestAlias.Message);
        var unknownArtifactField = Assert.Throws<ArgumentException>(
            () => EthereumMainnetNativeEvmProverBundle.FromJson(
                SampleNativeEvmProverBundleJson(ExpectedBindingHash).Replace(
                    "\"implementation_hash\":",
                    "\"experimental_manifest_note\": true, \"implementation_hash\":",
                    StringComparison.Ordinal),
                ExpectedBindingHash));
        Assert.Contains("nativeSdkArtifacts[0]", unknownArtifactField.Message);
        Assert.Contains("experimental_manifest_note", unknownArtifactField.Message);
        Assert.Contains("unknown field", unknownArtifactField.Message);
        var noncanonicalAuditHash = Assert.Throws<ArgumentException>(
            () => EthereumMainnetNativeEvmProverBundle.FromJson(
                SampleNativeEvmProverBundleJson(ExpectedBindingHash).Replace(
                    "\"0x" + new string('d', 64) + "\"",
                    "\"0x" + new string('D', 64) + "\"",
                    StringComparison.Ordinal),
                ExpectedBindingHash));
        Assert.Contains("auditHashes.circuit_security_audit", noncanonicalAuditHash.Message);
        Assert.Contains("canonical lowercase", noncanonicalAuditHash.Message);
        var replayedAuditHash = Assert.Throws<ArgumentException>(
            () => EthereumMainnetNativeEvmProverBundle.FromJson(
                SampleNativeEvmProverBundleJson(ExpectedBindingHash).Replace(
                    "\"0x" + new string('d', 64) + "\"",
                    "\"0x" + new string('9', 64) + "\"",
                    StringComparison.Ordinal),
                ExpectedBindingHash));
        Assert.Contains("auditHashes.circuit_security_audit", replayedAuditHash.Message);
        Assert.Contains("proofArtifactHash", replayedAuditHash.Message);
        Assert.Contains("role-separated", replayedAuditHash.Message);
        var bundledRequest = EthereumMainnetSccp.BuildOutboundProofRequest(input, nativeProverBundle);
        Assert.Equal("0x" + new string('9', 64), bundledRequest.ProofArtifactHash);
        Assert.Equal("0x" + new string('a', 64), bundledRequest.ProvingKeyHash);
        Assert.NotEqual(ExpectedRequestHash, bundledRequest.RequestHash);
        Assert.Equal(
            bundledRequest.RequestHash,
            EthereumMainnetSccp.BuildOutboundProofRequest(nativeProverBundle.ApplyTo(input)).RequestHash);
        Assert.Contains(
            "nativeProverBundle.verifierKeyHash",
            Assert.Throws<ArgumentException>(
                () => new EthereumMainnetNativeEvmProverBundle(
                    nativeProverBundle.ProofArtifactHash,
                    nativeProverBundle.ProvingKeyHash,
                    "0x" + new string('4', 64),
                    ExpectedBindingHash,
                    nativeProverBundle.NativeSdkArtifacts,
                    nativeProverBundle.AuditHashes).ApplyTo(input)).Message);
        var proofArtifactBytes = NativeEvmProverArtifactBytes("dotnet proof artifact v1");
        var provingKeyBytes = NativeEvmProverArtifactBytes("dotnet proving key v1");
        var verifierKeyBytes = NativeEvmProverArtifactBytes("dotnet verifier key v1");
        var implementationBytes = NativeEvmProverArtifactBytes("dotnet implementation artifact v1");
        var proofArtifactHash = Sha256Hex(proofArtifactBytes);
        var provingKeyHash = Sha256Hex(provingKeyBytes);
        var verifierKeyHash = Sha256Hex(verifierKeyBytes);
        var implementationHash = Sha256Hex(implementationBytes);
        var artifactBinding = EthereumMainnetSccp.DestinationBinding(
            "0x" + new string('1', 40),
            "0x" + new string('2', 40),
            "0x" + new string('b', 64),
            verifierKeyHash);
        var verifiedArtifactInput = SampleOutboundInput(artifactBinding, publicInputs);
        var draftVerifiedBundle = new EthereumMainnetNativeEvmProverBundle(
            proofArtifactHash,
            provingKeyHash,
            verifierKeyHash,
            artifactBinding.BindingHash,
            EthereumMainnetSccp.EthNativeEvmProverRequiredImplementationsV1
                .OrderBy(entry => entry.Key, StringComparer.Ordinal)
                .Select((entry, index) => new EthereumMainnetNativeEvmProverBundleSdkArtifact(
                    entry.Key,
                    entry.Value,
                    proofArtifactHash,
                    provingKeyHash,
                    entry.Key == "dotnet"
                        ? implementationHash
                        : "0x" + string.Concat(Enumerable.Repeat((index + 1).ToString("x2"), 32)),
                    implementationArtifact: $"artifacts/eth-mainnet/{entry.Key}-implementation.bin"))
                .ToArray(),
            SampleNativeAuditHashes(),
            expectedDestinationBindingHash: artifactBinding.BindingHash,
            proofArtifact: "artifacts/eth-mainnet/proof-artifact.bin",
            provingKey: "artifacts/eth-mainnet/proving-key.bin",
            verifierKey: "artifacts/eth-mainnet/verifier-key.bin",
            crossSdkFixtureParityArtifact: "artifacts/eth-mainnet/cross-sdk-fixture-parity.json",
            nativeProverSelfTestArtifact: "artifacts/eth-mainnet/native-prover-self-test.json");
        var parityFixtureBytes = Encoding.UTF8.GetBytes(
            SampleNativeEvmProverParityFixtureJson(draftVerifiedBundle));
        var parityFixtureHash = Sha256Hex(parityFixtureBytes);
        var selfTestFixtureBytes = Encoding.UTF8.GetBytes(
            SampleNativeEvmProverSelfTestFixtureJson(draftVerifiedBundle));
        var selfTestFixtureHash = Sha256Hex(selfTestFixtureBytes);
        var verifiedAuditHashes = SampleNativeAuditHashes()
            .ToDictionary(entry => entry.Key, entry => entry.Value, StringComparer.Ordinal);
        verifiedAuditHashes["cross_sdk_fixture_parity"] = parityFixtureHash;
        verifiedAuditHashes["native_prover_self_test"] = selfTestFixtureHash;
        var verifiedBundle = new EthereumMainnetNativeEvmProverBundle(
            proofArtifactHash,
            provingKeyHash,
            verifierKeyHash,
            artifactBinding.BindingHash,
            draftVerifiedBundle.NativeSdkArtifacts,
            verifiedAuditHashes,
            expectedDestinationBindingHash: artifactBinding.BindingHash,
            proofArtifact: "artifacts/eth-mainnet/proof-artifact.bin",
            provingKey: "artifacts/eth-mainnet/proving-key.bin",
            verifierKey: "artifacts/eth-mainnet/verifier-key.bin",
            crossSdkFixtureParityArtifact: "artifacts/eth-mainnet/cross-sdk-fixture-parity.json",
            nativeProverSelfTestArtifact: "artifacts/eth-mainnet/native-prover-self-test.json");
        (
            EthereumMainnetNativeEvmProverBundle Bundle,
            byte[] ParityFixtureBytes,
            byte[] SelfTestFixtureBytes
        ) HashConsistentNativeEvmProverBundle(
            byte[]? proofArtifactBytesOverride = null,
            byte[]? provingKeyBytesOverride = null,
            byte[]? verifierKeyBytesOverride = null,
            byte[]? implementationBytesOverride = null,
            byte[]? crossSdkFixtureParityBytesOverride = null,
            byte[]? nativeProverSelfTestBytesOverride = null)
        {
            var selectedProofArtifactBytes = proofArtifactBytesOverride ?? proofArtifactBytes;
            var selectedProvingKeyBytes = provingKeyBytesOverride ?? provingKeyBytes;
            var selectedVerifierKeyBytes = verifierKeyBytesOverride ?? verifierKeyBytes;
            var selectedImplementationBytes = implementationBytesOverride ?? implementationBytes;
            var selectedProofArtifactHash = Sha256Hex(selectedProofArtifactBytes);
            var selectedProvingKeyHash = Sha256Hex(selectedProvingKeyBytes);
            var selectedVerifierKeyHash = Sha256Hex(selectedVerifierKeyBytes);
            var selectedImplementationHash = Sha256Hex(selectedImplementationBytes);
            var draftBundle = new EthereumMainnetNativeEvmProverBundle(
                selectedProofArtifactHash,
                selectedProvingKeyHash,
                selectedVerifierKeyHash,
                artifactBinding.BindingHash,
                EthereumMainnetSccp.EthNativeEvmProverRequiredImplementationsV1
                    .OrderBy(entry => entry.Key, StringComparer.Ordinal)
                    .Select((entry, index) => new EthereumMainnetNativeEvmProverBundleSdkArtifact(
                        entry.Key,
                        entry.Value,
                        selectedProofArtifactHash,
                        selectedProvingKeyHash,
                        entry.Key == "dotnet"
                            ? selectedImplementationHash
                            : "0x" + string.Concat(Enumerable.Repeat((index + 1).ToString("x2"), 32)),
                        implementationArtifact: $"artifacts/eth-mainnet/{entry.Key}-implementation.bin"))
                    .ToArray(),
                SampleNativeAuditHashes(),
                expectedDestinationBindingHash: artifactBinding.BindingHash,
                proofArtifact: "artifacts/eth-mainnet/proof-artifact.bin",
                provingKey: "artifacts/eth-mainnet/proving-key.bin",
                verifierKey: "artifacts/eth-mainnet/verifier-key.bin",
                crossSdkFixtureParityArtifact: "artifacts/eth-mainnet/cross-sdk-fixture-parity.json",
                nativeProverSelfTestArtifact: "artifacts/eth-mainnet/native-prover-self-test.json");
            var selectedParityFixtureBytes = crossSdkFixtureParityBytesOverride
                ?? Encoding.UTF8.GetBytes(SampleNativeEvmProverParityFixtureJson(draftBundle));
            var selectedSelfTestFixtureBytes = nativeProverSelfTestBytesOverride
                ?? Encoding.UTF8.GetBytes(SampleNativeEvmProverSelfTestFixtureJson(draftBundle));
            var selectedAuditHashes = SampleNativeAuditHashes()
                .ToDictionary(entry => entry.Key, entry => entry.Value, StringComparer.Ordinal);
            selectedAuditHashes["cross_sdk_fixture_parity"] = Sha256Hex(selectedParityFixtureBytes);
            selectedAuditHashes["native_prover_self_test"] = Sha256Hex(selectedSelfTestFixtureBytes);
            return (
                new EthereumMainnetNativeEvmProverBundle(
                    selectedProofArtifactHash,
                    selectedProvingKeyHash,
                    selectedVerifierKeyHash,
                    artifactBinding.BindingHash,
                    draftBundle.NativeSdkArtifacts,
                    selectedAuditHashes,
                    expectedDestinationBindingHash: artifactBinding.BindingHash,
                    proofArtifact: "artifacts/eth-mainnet/proof-artifact.bin",
                    provingKey: "artifacts/eth-mainnet/proving-key.bin",
                    verifierKey: "artifacts/eth-mainnet/verifier-key.bin",
                    crossSdkFixtureParityArtifact: "artifacts/eth-mainnet/cross-sdk-fixture-parity.json",
                    nativeProverSelfTestArtifact: "artifacts/eth-mainnet/native-prover-self-test.json"),
                selectedParityFixtureBytes,
                selectedSelfTestFixtureBytes);
        }
        var verifiedArtifacts = verifiedBundle.VerifiedArtifacts(
            proofArtifactBytes,
            provingKeyBytes,
            verifierKeyBytes,
            "dotnet",
            implementationBytes,
            parityFixtureBytes,
            selfTestFixtureBytes);
        Assert.Equal(EthereumMainnetSccp.NativeEvmProverArtifactHashAlgorithmV1, verifiedArtifacts.HashAlgorithm);
        Assert.Equal(proofArtifactHash, verifiedArtifacts.ProofArtifactHash);
        Assert.Equal(provingKeyHash, verifiedArtifacts.ProvingKeyHash);
        Assert.Equal(verifierKeyHash, verifiedArtifacts.VerifierKeyHash);
        Assert.Equal(parityFixtureHash, verifiedArtifacts.CrossSdkFixtureParityHash);
        Assert.Equal("0x" + new string('3', 64), verifiedArtifacts.CrossSdkFixtureParity?.CalldataHash);
        Assert.Equal(selfTestFixtureHash, verifiedArtifacts.NativeProverSelfTestHash);
        Assert.Equal("0x" + new string('8', 64), verifiedArtifacts.NativeProverSelfTest?.ProofHash);
        Assert.Equal("native-csharp", verifiedArtifacts.Implementation);
        Assert.Equal(implementationHash, verifiedArtifacts.ImplementationHash);
        var dotnetImplementationArtifact = Assert.Single(
            verifiedBundle.NativeSdkArtifacts,
            row => row.Sdk == "dotnet").ImplementationArtifact!;
        var artifactBytesByPath = new Dictionary<string, byte[]>(StringComparer.Ordinal)
        {
            [verifiedBundle.ProofArtifact!] = proofArtifactBytes,
            [verifiedBundle.ProvingKey!] = provingKeyBytes,
            [verifiedBundle.VerifierKey!] = verifierKeyBytes,
            [verifiedBundle.CrossSdkFixtureParityArtifact!] = parityFixtureBytes,
            [verifiedBundle.NativeProverSelfTestArtifact!] = selfTestFixtureBytes,
            [dotnetImplementationArtifact] = implementationBytes,
        };
        var verifiedFromResolver = verifiedBundle.VerifiedArtifacts(
            "dotnet",
            path => artifactBytesByPath.TryGetValue(path, out var bytes)
                ? bytes
                : throw new ArgumentException(path));
        Assert.Equal(implementationHash, verifiedFromResolver.ImplementationHash);
        Assert.Equal(parityFixtureHash, verifiedFromResolver.CrossSdkFixtureParityHash);
        Assert.Equal(selfTestFixtureHash, verifiedFromResolver.NativeProverSelfTestHash);
        Assert.Contains(
            "crossSdkFixtureParityArtifact",
            Assert.Throws<ArgumentException>(
                () => verifiedBundle.VerifiedArtifacts(
                    "dotnet",
                    path => path == verifiedBundle.CrossSdkFixtureParityArtifact
                        ? throw new ArgumentException("crossSdkFixtureParityArtifact")
                        : artifactBytesByPath[path])).Message);
        Assert.Contains(
            "nativeProverSelfTestArtifact",
            Assert.Throws<ArgumentException>(
                () => verifiedBundle.VerifiedArtifacts(
                    "dotnet",
                    path => path == verifiedBundle.NativeProverSelfTestArtifact
                        ? throw new ArgumentException("nativeProverSelfTestArtifact")
                        : artifactBytesByPath[path])).Message);
        var verifiedArtifactRequest = EthereumMainnetSccp.BuildOutboundProofRequest(
            verifiedBundle.ApplyTo(verifiedArtifactInput));
        var artifactBoundProver = new OutboundProverStub(
            Groth16ProofBytes(),
            expectedRequestHash: verifiedArtifactRequest.RequestHash,
            expectedBindingHash: artifactBinding.BindingHash,
            expectedPublicSignalWords: verifiedArtifactRequest.PublicSignalWords);
        var artifactBoundSelfTest = new NativeProverSelfTestStub(
            (fixture, expected, artifacts) =>
            {
                Assert.Equal(selfTestFixtureHash, artifacts.NativeProverSelfTestHash);
                return expected;
            });
        var preflightSelfTest = new NativeProverSelfTestStub(
            (fixture, expected, artifacts) =>
            {
                Assert.Equal(selfTestFixtureHash, artifacts.NativeProverSelfTestHash);
                return expected;
            });
        var preflightResult = await EthereumMainnetSccp.RunNativeProverSelfTestAsync(
            verifiedArtifacts,
            preflightSelfTest);
        Assert.True(preflightSelfTest.Called);
        Assert.Equal("0x" + new string('8', 64), preflightResult.ProofHash);
        var artifactBoundResult = await EthereumMainnetSccp.ProveOutboundToEthereumAsync(
            verifiedArtifactInput,
            artifactBoundProver,
            verifiedArtifacts,
            artifactBoundSelfTest);
        Assert.True(artifactBoundSelfTest.Called);
        Assert.NotNull(artifactBoundProver.Request);
        Assert.Equal(proofArtifactHash, artifactBoundProver.Request!.ProofArtifactHash);
        Assert.Equal(provingKeyHash, artifactBoundProver.Request.ProvingKeyHash);
        Assert.Equal(proofArtifactHash, artifactBoundResult.ProofArtifactHash);
        Assert.Equal(provingKeyHash, artifactBoundResult.ProvingKeyHash);
        var missingSelfTestHookProver = new OutboundProverStub(Groth16ProofBytes());
        Assert.Contains(
            "nativeProverSelfTest runner",
            (await Assert.ThrowsAsync<ArgumentException>(
                async () => await EthereumMainnetSccp.ProveOutboundToEthereumAsync(
                    verifiedArtifactInput,
                    missingSelfTestHookProver,
                    verifiedArtifacts))).Message);
        Assert.Null(missingSelfTestHookProver.Request);
        var driftingSelfTestHookProver = new OutboundProverStub(Groth16ProofBytes());
        var driftingSelfTestHook = new NativeProverSelfTestStub(
            (_, expected, _) => new EthereumMainnetNativeEvmProverSelfTestSdkResult(
                expected.RequestHash,
                expected.WitnessHash,
                expected.SourceProofHash,
                "0x" + new string('9', 64),
                expected.PublicSignalWords,
                expected.CalldataHash,
                expected.ToriiSubmitPayloadHash));
        Assert.Contains(
            "nativeProverSelfTest result",
            (await Assert.ThrowsAsync<ArgumentException>(
                async () => await EthereumMainnetSccp.ProveOutboundToEthereumAsync(
                    verifiedArtifactInput,
                    driftingSelfTestHookProver,
                    verifiedArtifacts,
                    driftingSelfTestHook))).Message);
        Assert.Null(driftingSelfTestHookProver.Request);
        var factoryBoundProver = new OutboundProverStub(
            Groth16ProofBytes(),
            expectedRequestHash: verifiedArtifactRequest.RequestHash,
            expectedBindingHash: artifactBinding.BindingHash,
            expectedPublicSignalWords: verifiedArtifactRequest.PublicSignalWords);
        var factoryBoundSelfTest = new NativeProverSelfTestStub();
        var factoryBoundResult = await EthereumMainnetSccp.ProveOutboundToEthereumFromNativeProverBundleAsync(
            verifiedArtifactInput,
            factoryBoundProver,
            verifiedBundle,
            "dotnet",
            path => artifactBytesByPath.TryGetValue(path, out var bytes)
                ? bytes
                : throw new ArgumentException(path),
            factoryBoundSelfTest);
        Assert.True(factoryBoundSelfTest.Called);
        Assert.NotNull(factoryBoundProver.Request);
        Assert.Equal(proofArtifactHash, factoryBoundProver.Request!.ProofArtifactHash);
        Assert.Equal(provingKeyHash, factoryBoundProver.Request.ProvingKeyHash);
        Assert.Equal(proofArtifactHash, factoryBoundResult.ProofArtifactHash);
        Assert.Equal(provingKeyHash, factoryBoundResult.ProvingKeyHash);
        var missingFactoryParityProver = new OutboundProverStub(Groth16ProofBytes());
        Assert.Contains(
            "crossSdkFixtureParityArtifact",
            (await Assert.ThrowsAsync<ArgumentException>(
                async () => await EthereumMainnetSccp.ProveOutboundToEthereumFromNativeProverBundleAsync(
                    verifiedArtifactInput,
                    missingFactoryParityProver,
                    verifiedBundle,
                    "dotnet",
                    path => path == verifiedBundle.CrossSdkFixtureParityArtifact
                        ? throw new ArgumentException("crossSdkFixtureParityArtifact")
                        : artifactBytesByPath[path]))).Message);
        Assert.Null(missingFactoryParityProver.Request);
        var missingFactorySelfTestProver = new OutboundProverStub(Groth16ProofBytes());
        Assert.Contains(
            "nativeProverSelfTestArtifact",
            (await Assert.ThrowsAsync<ArgumentException>(
                async () => await EthereumMainnetSccp.ProveOutboundToEthereumFromNativeProverBundleAsync(
                    verifiedArtifactInput,
                    missingFactorySelfTestProver,
                    verifiedBundle,
                    "dotnet",
                    path => path == verifiedBundle.NativeProverSelfTestArtifact
                        ? throw new ArgumentException("nativeProverSelfTestArtifact")
                        : artifactBytesByPath[path]))).Message);
        Assert.Null(missingFactorySelfTestProver.Request);
        var implementationUnboundArtifacts = new EthereumMainnetNativeEvmProverArtifacts(
            EthereumMainnetSccp.NativeEvmProverArtifactHashAlgorithmV1,
            verifiedBundle,
            proofArtifactHash,
            provingKeyHash,
            verifierKeyHash,
            parityFixtureHash,
            verifiedArtifacts.CrossSdkFixtureParity,
            selfTestFixtureHash,
            verifiedArtifacts.NativeProverSelfTest,
            "dotnet",
            "native-csharp",
            null);
        var implementationUnboundProver = new OutboundProverStub(Groth16ProofBytes());
        Assert.Contains(
            "nativeProverArtifacts must bind sdk implementation and implementationHash",
            (await Assert.ThrowsAsync<ArgumentException>(
                async () => await EthereumMainnetSccp.ProveOutboundToEthereumAsync(
                    verifiedArtifactInput,
                    implementationUnboundProver,
                    implementationUnboundArtifacts))).Message);
        Assert.Null(implementationUnboundProver.Request);
        var verifierKeyUnboundArtifacts = new EthereumMainnetNativeEvmProverArtifacts(
            EthereumMainnetSccp.NativeEvmProverArtifactHashAlgorithmV1,
            verifiedBundle,
            proofArtifactHash,
            provingKeyHash,
            "0x" + new string('e', 64),
            parityFixtureHash,
            verifiedArtifacts.CrossSdkFixtureParity,
            selfTestFixtureHash,
            verifiedArtifacts.NativeProverSelfTest,
            "dotnet",
            "native-csharp",
            implementationHash);
        var verifierKeyUnboundProver = new OutboundProverStub(Groth16ProofBytes());
        Assert.Contains(
            "nativeProverArtifacts verifierKeyHash must match nativeProverBundle",
            (await Assert.ThrowsAsync<ArgumentException>(
                async () => await EthereumMainnetSccp.ProveOutboundToEthereumAsync(
                    verifiedArtifactInput,
                    verifierKeyUnboundProver,
                    verifierKeyUnboundArtifacts))).Message);
        Assert.Null(verifierKeyUnboundProver.Request);
        Assert.Contains(
            "proofArtifactBytes sha256",
            Assert.Throws<ArgumentException>(
                () => verifiedBundle.VerifiedArtifacts(
                    [0],
                    provingKeyBytes,
                    verifierKeyBytes)).Message);
        var tinyProofArtifactBytes = new byte[] { 1, 2, 3, 4, 5, 6, 7 };
        var tinyProofArtifactHash = Sha256Hex(tinyProofArtifactBytes);
        var draftTinyBundle = new EthereumMainnetNativeEvmProverBundle(
            tinyProofArtifactHash,
            provingKeyHash,
            verifierKeyHash,
            artifactBinding.BindingHash,
            EthereumMainnetSccp.EthNativeEvmProverRequiredImplementationsV1
                .OrderBy(entry => entry.Key, StringComparer.Ordinal)
                .Select((entry, index) => new EthereumMainnetNativeEvmProverBundleSdkArtifact(
                    entry.Key,
                    entry.Value,
                    tinyProofArtifactHash,
                    provingKeyHash,
                    entry.Key == "dotnet"
                        ? implementationHash
                        : "0x" + string.Concat(Enumerable.Repeat((index + 1).ToString("x2"), 32)),
                    implementationArtifact: $"artifacts/eth-mainnet/{entry.Key}-implementation.bin"))
                .ToArray(),
            SampleNativeAuditHashes(),
            expectedDestinationBindingHash: artifactBinding.BindingHash);
        var tinyParityFixtureBytes = Encoding.UTF8.GetBytes(
            SampleNativeEvmProverParityFixtureJson(draftTinyBundle));
        var tinySelfTestFixtureBytes = Encoding.UTF8.GetBytes(
            SampleNativeEvmProverSelfTestFixtureJson(draftTinyBundle));
        var tinyAuditHashes = SampleNativeAuditHashes()
            .ToDictionary(entry => entry.Key, entry => entry.Value, StringComparer.Ordinal);
        tinyAuditHashes["cross_sdk_fixture_parity"] = Sha256Hex(tinyParityFixtureBytes);
        tinyAuditHashes["native_prover_self_test"] = Sha256Hex(tinySelfTestFixtureBytes);
        var tinyBundle = new EthereumMainnetNativeEvmProverBundle(
            tinyProofArtifactHash,
            provingKeyHash,
            verifierKeyHash,
            artifactBinding.BindingHash,
            draftTinyBundle.NativeSdkArtifacts,
            tinyAuditHashes,
            expectedDestinationBindingHash: artifactBinding.BindingHash);
        Assert.Contains(
            "proofArtifactBytes must be at least 65536 bytes",
            Assert.Throws<ArgumentException>(
                () => tinyBundle.VerifiedArtifacts(
                    tinyProofArtifactBytes,
                    provingKeyBytes,
                    verifierKeyBytes,
                    "dotnet",
                    implementationBytes,
                    tinyParityFixtureBytes,
                    tinySelfTestFixtureBytes)).Message);
        var tinyProvingKeyBytes = new byte[] { 8, 9, 10, 11 };
        var tinyProvingKeyFixture = HashConsistentNativeEvmProverBundle(
            provingKeyBytesOverride: tinyProvingKeyBytes);
        Assert.Contains(
            "provingKeyBytes must be at least 65536 bytes",
            Assert.Throws<ArgumentException>(
                () => tinyProvingKeyFixture.Bundle.VerifiedArtifacts(
                    proofArtifactBytes,
                    tinyProvingKeyBytes,
                    verifierKeyBytes,
                    "dotnet",
                    implementationBytes,
                    tinyProvingKeyFixture.ParityFixtureBytes,
                    tinyProvingKeyFixture.SelfTestFixtureBytes)).Message);
        var tinyVerifierKeyBytes = new byte[] { 12, 13, 14, 15 };
        var tinyVerifierKeyFixture = HashConsistentNativeEvmProverBundle(
            verifierKeyBytesOverride: tinyVerifierKeyBytes);
        Assert.Contains(
            "verifierKeyBytes must be at least 128 bytes",
            Assert.Throws<ArgumentException>(
                () => tinyVerifierKeyFixture.Bundle.VerifiedArtifacts(
                    proofArtifactBytes,
                    provingKeyBytes,
                    tinyVerifierKeyBytes,
                    "dotnet",
                    implementationBytes,
                    tinyVerifierKeyFixture.ParityFixtureBytes,
                    tinyVerifierKeyFixture.SelfTestFixtureBytes)).Message);
        var tinyParityFixtureBytesForFloor = Encoding.UTF8.GetBytes("{}");
        var tinyParityFixture = HashConsistentNativeEvmProverBundle(
            crossSdkFixtureParityBytesOverride: tinyParityFixtureBytesForFloor);
        Assert.Contains(
            "crossSdkFixtureParityBytes must be at least 128 bytes",
            Assert.Throws<ArgumentException>(
                () => tinyParityFixture.Bundle.VerifiedArtifacts(
                    proofArtifactBytes,
                    provingKeyBytes,
                    verifierKeyBytes,
                    "dotnet",
                    implementationBytes,
                    tinyParityFixtureBytesForFloor,
                    tinyParityFixture.SelfTestFixtureBytes)).Message);
        var tinySelfTestFixtureBytesForFloor = Encoding.UTF8.GetBytes("{}");
        var tinySelfTestFixture = HashConsistentNativeEvmProverBundle(
            nativeProverSelfTestBytesOverride: tinySelfTestFixtureBytesForFloor);
        Assert.Contains(
            "nativeProverSelfTestBytes must be at least 128 bytes",
            Assert.Throws<ArgumentException>(
                () => tinySelfTestFixture.Bundle.VerifiedArtifacts(
                    proofArtifactBytes,
                    provingKeyBytes,
                    verifierKeyBytes,
                    "dotnet",
                    implementationBytes,
                    tinySelfTestFixture.ParityFixtureBytes,
                    tinySelfTestFixtureBytesForFloor)).Message);
        var tinyImplementationBytes = new byte[] { 16, 17, 18, 19 };
        var tinyImplementationFixture = HashConsistentNativeEvmProverBundle(
            implementationBytesOverride: tinyImplementationBytes);
        Assert.Contains(
            "implementationBytes must be at least 1024 bytes",
            Assert.Throws<ArgumentException>(
                () => tinyImplementationFixture.Bundle.VerifiedArtifacts(
                    proofArtifactBytes,
                    provingKeyBytes,
                    verifierKeyBytes,
                    "dotnet",
                    tinyImplementationBytes,
                    tinyImplementationFixture.ParityFixtureBytes,
                    tinyImplementationFixture.SelfTestFixtureBytes)).Message);
        Assert.Contains(
            "sdk must be a non-empty string",
            Assert.Throws<ArgumentException>(
                () => verifiedBundle.VerifiedArtifacts(
                    proofArtifactBytes,
                    provingKeyBytes,
                    verifierKeyBytes,
                    implementationBytes: implementationBytes,
                    crossSdkFixtureParityBytes: parityFixtureBytes,
                    nativeProverSelfTestBytes: selfTestFixtureBytes)).Message);
        Assert.Contains(
            "implementationBytes are required",
            Assert.Throws<ArgumentException>(
                () => verifiedBundle.VerifiedArtifacts(
                    proofArtifactBytes,
                    provingKeyBytes,
                    verifierKeyBytes,
                    "dotnet",
                    crossSdkFixtureParityBytes: parityFixtureBytes,
                    nativeProverSelfTestBytes: selfTestFixtureBytes)).Message);
        Assert.Contains(
            "implementationBytes sha256",
            Assert.Throws<ArgumentException>(
                () => verifiedBundle.VerifiedArtifacts(
                    proofArtifactBytes,
                    provingKeyBytes,
                    verifierKeyBytes,
                    "dotnet",
                    Encoding.UTF8.GetBytes("tampered"),
                    parityFixtureBytes,
                    selfTestFixtureBytes)).Message);
        Assert.Contains(
            "crossSdkFixtureParityBytes",
            Assert.Throws<ArgumentException>(
                () => verifiedBundle.VerifiedArtifacts(
                    proofArtifactBytes,
                    provingKeyBytes,
                    verifierKeyBytes,
                    "dotnet",
                    implementationBytes)).Message);
        Assert.Contains(
            "crossSdkFixtureParityBytes sha256",
            Assert.Throws<ArgumentException>(
                () => verifiedBundle.VerifiedArtifacts(
                    proofArtifactBytes,
                    provingKeyBytes,
                    verifierKeyBytes,
                    "dotnet",
                    implementationBytes,
                    Encoding.UTF8.GetBytes("{}"))).Message);
        Assert.Contains(
            "nativeProverSelfTestBytes",
            Assert.Throws<ArgumentException>(
                () => verifiedBundle.VerifiedArtifacts(
                    proofArtifactBytes,
                    provingKeyBytes,
                    verifierKeyBytes,
                    "dotnet",
                    implementationBytes,
                    parityFixtureBytes)).Message);
        Assert.Contains(
            "nativeProverSelfTestBytes sha256",
            Assert.Throws<ArgumentException>(
                () => verifiedBundle.VerifiedArtifacts(
                    proofArtifactBytes,
                    provingKeyBytes,
                    verifierKeyBytes,
                    "dotnet",
                    implementationBytes,
                    parityFixtureBytes,
                    Encoding.UTF8.GetBytes("{}"))).Message);
        var flaggedArtifactBytes = NativeEvmProverArtifactBytes("proof.wasm dotnet artifact marker");
        var flaggedArtifactHash = Sha256Hex(flaggedArtifactBytes);
        var draftFlaggedBundle = new EthereumMainnetNativeEvmProverBundle(
            flaggedArtifactHash,
            provingKeyHash,
            verifierKeyHash,
            artifactBinding.BindingHash,
            EthereumMainnetSccp.EthNativeEvmProverRequiredImplementationsV1
                .OrderBy(entry => entry.Key, StringComparer.Ordinal)
                .Select((entry, index) => new EthereumMainnetNativeEvmProverBundleSdkArtifact(
                    entry.Key,
                    entry.Value,
                    flaggedArtifactHash,
                    provingKeyHash,
                    entry.Key == "dotnet"
                        ? implementationHash
                        : "0x" + string.Concat(Enumerable.Repeat((index + 1).ToString("x2"), 32)),
                    implementationArtifact: $"artifacts/eth-mainnet/{entry.Key}-implementation.bin"))
                .ToArray(),
            SampleNativeAuditHashes(),
            expectedDestinationBindingHash: artifactBinding.BindingHash);
        var flaggedParityFixtureBytes = Encoding.UTF8.GetBytes(
            SampleNativeEvmProverParityFixtureJson(draftFlaggedBundle));
        var flaggedSelfTestFixtureBytes = Encoding.UTF8.GetBytes(
            SampleNativeEvmProverSelfTestFixtureJson(draftFlaggedBundle));
        var flaggedAuditHashes = SampleNativeAuditHashes()
            .ToDictionary(entry => entry.Key, entry => entry.Value, StringComparer.Ordinal);
        flaggedAuditHashes["cross_sdk_fixture_parity"] = Sha256Hex(flaggedParityFixtureBytes);
        flaggedAuditHashes["native_prover_self_test"] = Sha256Hex(flaggedSelfTestFixtureBytes);
        var flaggedBundle = new EthereumMainnetNativeEvmProverBundle(
            flaggedArtifactHash,
            provingKeyHash,
            verifierKeyHash,
            artifactBinding.BindingHash,
            draftFlaggedBundle.NativeSdkArtifacts,
            flaggedAuditHashes,
            expectedDestinationBindingHash: artifactBinding.BindingHash);
        Assert.Contains(
            "proofArtifactBytes contains forbidden",
            Assert.Throws<ArgumentException>(
                () => flaggedBundle.VerifiedArtifacts(
                    flaggedArtifactBytes,
                    provingKeyBytes,
                    verifierKeyBytes,
                    crossSdkFixtureParityBytes: flaggedParityFixtureBytes,
                    nativeProverSelfTestBytes: flaggedSelfTestFixtureBytes)).Message);
        Assert.Contains(
            "noWasm",
            Assert.Throws<ArgumentException>(
                () => SampleNativeEvmProverBundle(ExpectedBindingHash, noWasm: false)).Message);
        Assert.Contains(
            "destinationBindingHash",
            Assert.Throws<ArgumentException>(
                () => SampleNativeEvmProverBundle(
                    "0x" + new string('b', 64),
                    expectedDestinationBindingHash: ExpectedBindingHash)).Message);
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildOutboundProofRequest(input with
            {
                ProofArtifactHash = "0x" + new string('9', 64),
            }));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildOutboundProofRequest(input with
            {
                ProofArtifactHash = "0x" + new string('0', 64),
                ProvingKeyHash = "0x" + new string('a', 64),
            }));

        var mutableProof = Groth16ProofBytes();
        var prover = new OutboundProverStub(mutableProof, callbackRequest =>
        {
            callbackRequest.PublicInputsBytes[0] ^= 0x7f;
            callbackRequest.PublicSignalWords[0] = "0x" + new string('f', 64);
            callbackRequest.BundleBytes[0] ^= 0x7f;
            if (callbackRequest.SourceProofBytes.Length > 0)
            {
                callbackRequest.SourceProofBytes[0] ^= 0x7f;
            }
        });
        var proofResult = await EthereumMainnetSccp.ProveOutboundToEthereumAsync(input, prover);
        mutableProof[31] = 9;
        Assert.NotNull(prover.Request);
        Assert.Equal(1, proofResult.ProofBytes[31]);
        Assert.Equal(ExpectedRequestHash, proofResult.RequestHash);
        Assert.Equal(ExpectedEnvelopeHash, proofResult.EnvelopeHash);
        Assert.Equal(ExpectedPublicSignalWords, proofResult.PublicSignalWords);
        Assert.Equal(ExpectedPublicInputsBytes, Convert.ToHexString(proofResult.Request.PublicInputsBytes).ToLowerInvariant());
        Assert.Equal(ExpectedPublicSignalWords, proofResult.Request.PublicSignalWords);
        Assert.Equal(SampleOutboundBundleBytes(), proofResult.Request.BundleBytes);
        Assert.Empty(proofResult.Request.SourceProofBytes);
        Assert.NotEqual(ExpectedPublicInputsBytes, Convert.ToHexString(prover.Request.PublicInputsBytes).ToLowerInvariant());
        Assert.NotEqual(ExpectedPublicSignalWords[0], prover.Request.PublicSignalWords[0]);
        Assert.Equal(publicInputs, proofResult.PublicInputs);
        Assert.Equal(binding, proofResult.DestinationBinding);

        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildEthereumCalldata(
                new EthereumMainnetSccpSubmissionInput(proofResult with
                {
                    ProofArtifactHash = "0x" + new string('9', 64),
                    ProvingKeyHash = "0x" + new string('a', 64),
                })));
        Assert.Contains(
            "verified native EVM prover artifacts",
            Assert.Throws<ArgumentException>(
                () => EthereumMainnetSccp.BuildEthereumCalldata(
                    new EthereumMainnetSccpSubmissionInput(proofResult))).Message);

        var submission = EthereumMainnetSccp.BuildEthereumCalldata(
            new EthereumMainnetSccpSubmissionInput(artifactBoundResult),
            verifiedArtifacts);
        Assert.Equal(1, submission.Version);
        Assert.Equal(EthereumMainnetSccp.StarkFriProofFamily, submission.ProofFamily);
        Assert.Equal(EthereumMainnetSccp.EvmGroth16Bn254ProofBackend, submission.VerifierBackend);
        Assert.Equal(EthereumMainnetSccp.ContractCallAbiTuple, submission.EnvelopeEncoding);
        Assert.Equal(EthereumMainnetSccp.SubmitMessageProofAbi, submission.ContractMethod);
        Assert.Equal(EthereumMainnetSccp.SubmitMessageProofSelector, submission.FunctionSelector);
        Assert.Equal(EthereumMainnetSccp.DomainSora, submission.SourceDomain);
        Assert.Equal(EthereumMainnetSccp.DomainEthereum, submission.TargetDomain);
        Assert.Equal(ExpectedPublicInputWords, submission.PublicInputWords);
        Assert.Equal(artifactBoundResult.PublicSignalWords, submission.PublicSignalWords);
        Assert.Equal(ExpectedCallDataHex, submission.CallDataHex);
        Assert.Equal(676, submission.CallData.Length);
        Assert.Equal(submission.CallData, submission.EnvelopeBytes);
        Assert.Equal(submission.CallDataHex, submission.EnvelopeHex);
        var resolverSubmission = EthereumMainnetSccp.BuildEthereumCalldataFromNativeProverBundle(
            new EthereumMainnetSccpSubmissionInput(artifactBoundResult),
            verifiedBundle,
            "dotnet",
            path => artifactBytesByPath.TryGetValue(path, out var bytes)
                ? bytes
                : throw new ArgumentException(path));
        Assert.Equal(submission.CallDataHex, resolverSubmission.CallDataHex);
        Assert.Contains(
            "crossSdkFixtureParityArtifact",
            Assert.Throws<ArgumentException>(
                () => EthereumMainnetSccp.BuildEthereumCalldataFromNativeProverBundle(
                    new EthereumMainnetSccpSubmissionInput(artifactBoundResult),
                    verifiedBundle,
                    "dotnet",
                    path => path == verifiedBundle.CrossSdkFixtureParityArtifact
                        ? throw new ArgumentException("crossSdkFixtureParityArtifact")
                        : artifactBytesByPath[path])).Message);
        Assert.Contains(
            "nativeProverSelfTestArtifact",
            Assert.Throws<ArgumentException>(
                () => EthereumMainnetSccp.BuildEthereumCalldataFromNativeProverBundle(
                    new EthereumMainnetSccpSubmissionInput(artifactBoundResult),
                    verifiedBundle,
                    "dotnet",
                    path => path == verifiedBundle.NativeProverSelfTestArtifact
                        ? throw new ArgumentException("nativeProverSelfTestArtifact")
                        : artifactBytesByPath[path])).Message);

        var submitter = new OutboundSubmitterStub();
        Assert.Equal(
            "eth-submitted",
            await EthereumMainnetSccp.SubmitOutboundToEthereumAsync(
                new EthereumMainnetSccpSubmissionInput(artifactBoundResult),
                submitter,
                verifiedArtifacts));
        Assert.NotNull(submitter.Submission);
        Assert.Equal(submission.CallDataHex, submitter.Submission.CallDataHex);
        var resolverSubmitter = new OutboundSubmitterStub();
        Assert.Equal(
            "eth-submitted",
            await EthereumMainnetSccp.SubmitOutboundToEthereumFromNativeProverBundleAsync(
                new EthereumMainnetSccpSubmissionInput(artifactBoundResult),
                resolverSubmitter,
                verifiedBundle,
                "dotnet",
                path => artifactBytesByPath.TryGetValue(path, out var bytes)
                    ? bytes
                    : throw new ArgumentException(path)));
        Assert.NotNull(resolverSubmitter.Submission);
        Assert.Equal(submission.CallDataHex, resolverSubmitter.Submission.CallDataHex);
        var missingParitySubmitter = new OutboundSubmitterStub();
        Assert.Contains(
            "crossSdkFixtureParityArtifact",
            (await Assert.ThrowsAsync<ArgumentException>(
                async () => await EthereumMainnetSccp.SubmitOutboundToEthereumFromNativeProverBundleAsync(
                    new EthereumMainnetSccpSubmissionInput(artifactBoundResult),
                    missingParitySubmitter,
                    verifiedBundle,
                    "dotnet",
                    path => path == verifiedBundle.CrossSdkFixtureParityArtifact
                        ? throw new ArgumentException("crossSdkFixtureParityArtifact")
                        : artifactBytesByPath[path]))).Message);
        Assert.Null(missingParitySubmitter.Submission);
        var missingSelfTestSubmitter = new OutboundSubmitterStub();
        Assert.Contains(
            "nativeProverSelfTestArtifact",
            (await Assert.ThrowsAsync<ArgumentException>(
                async () => await EthereumMainnetSccp.SubmitOutboundToEthereumFromNativeProverBundleAsync(
                    new EthereumMainnetSccpSubmissionInput(artifactBoundResult),
                    missingSelfTestSubmitter,
                    verifiedBundle,
                    "dotnet",
                    path => path == verifiedBundle.NativeProverSelfTestArtifact
                        ? throw new ArgumentException("nativeProverSelfTestArtifact")
                        : artifactBytesByPath[path]))).Message);
        Assert.Null(missingSelfTestSubmitter.Submission);

        var guardedSubmitter = new OutboundSubmitterStub();
        await Assert.ThrowsAsync<ArgumentOutOfRangeException>(
            async () => await EthereumMainnetSccp.SubmitOutboundToEthereumAsync(
                new EthereumMainnetSccpSubmissionInput(artifactBoundResult),
                guardedSubmitter,
                verifiedArtifacts,
                new ExecutionProviderStub(
                    "0x38",
                    new Dictionary<string, object?>(),
                    new Dictionary<string, object?>())));
        Assert.Null(guardedSubmitter.Submission);
    }

    [Fact]
    public async Task OutboundCallbackAndSubmissionSnapshotsRejectMutation()
    {
        var input = SampleOutboundInput();
        var expectedRequest = EthereumMainnetSccp.BuildOutboundProofRequest(input);
        var prover = new MutatingOutboundProverStub(Groth16ProofBytes());

        var proofResult = await EthereumMainnetSccp.ProveOutboundToEthereumAsync(input, prover);

        Assert.NotNull(prover.Request);
        Assert.Equal(expectedRequest.RequestHash, proofResult.RequestHash);
        Assert.Equal(expectedRequest.BundleBytes, proofResult.Request.BundleBytes);
        Assert.Equal(expectedRequest.SourceProofBytes, proofResult.Request.SourceProofBytes);
        Assert.Equal(expectedRequest.PublicInputsBytes, proofResult.Request.PublicInputsBytes);
        Assert.Equal(expectedRequest.PublicSignalWords, proofResult.Request.PublicSignalWords);

        Assert.Contains(
            "verified native EVM prover artifacts",
            Assert.Throws<ArgumentException>(
                () => EthereumMainnetSccp.BuildEthereumCalldata(
                    new EthereumMainnetSccpSubmissionInput(proofResult))).Message);

        var mutatedProofBytes = proofResult.ProofBytes.ToArray();
        mutatedProofBytes[31] = 9;
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildEthereumCalldata(
                new EthereumMainnetSccpSubmissionInput(
                    proofResult with { ProofBytes = mutatedProofBytes })));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildEthereumCalldata(
                new EthereumMainnetSccpSubmissionInput(
                    proofResult with
                    {
                        ProofBase64 = Convert.ToBase64String(mutatedProofBytes),
                    })));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildEthereumCalldata(
                new EthereumMainnetSccpSubmissionInput(
                    proofResult with
                    {
                        Request = proofResult.Request with
                        {
                            BundleBytes = "swapped-eth-bundle"u8.ToArray(),
                        },
                    })));
        var mutatedSignals = proofResult.PublicSignalWords.ToArray();
        mutatedSignals[0] = "0x" + new string('9', 64);
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildEthereumCalldata(
                new EthereumMainnetSccpSubmissionInput(
                    proofResult with { PublicSignalWords = mutatedSignals })));
    }

    [Fact]
    public void LocalAdmissionSubmissionWrapsNativeEthereumOutput()
    {
        var input = new EthereumMainnetLocalAdmissionSubmissionInput(
            ProofBytes: [1, 2, 3],
            PublicInputsBytes: [4, 5, 6],
            BundleBytes: [7, 8, 9],
            EnvelopeBytes: [10, 11, 12],
            StatementHash: "0x" + new string('6', 64),
            SourceVerifierMaterialHash: "0x" + new string('7', 64),
            SourceAdapterEngineDeploymentHash: "0x" + new string('8', 64));
        var submission = EthereumMainnetSccp.BuildLocalAdmissionSubmission(input);

        Assert.Equal(EthereumMainnetSccp.LocalAdmissionSubmissionKind, submission.PlatformPayload);
        Assert.Equal(EthereumMainnetSccp.LocalAdmissionEnvelopeEncoding, submission.EnvelopeEncoding);
        Assert.Equal(EthereumMainnetSccp.LocalAdmissionEntrypoint, submission.VerifierEntrypoint);
        Assert.Equal(EthereumMainnetSccp.DomainEthereum, submission.SourceDomain);
        Assert.Equal(EthereumMainnetSccp.DomainSora, submission.TargetDomain);
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
            () => EthereumMainnetSccp.BuildLocalAdmissionSubmission(input with
            {
                SourceDomain = BscMainnetSccp.DomainBsc,
            }));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildLocalAdmissionSubmission(input with
            {
                ProofBytes = [0, 0],
            }));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildLocalAdmissionSubmission(input with
            {
                PublicInputsBytes = [0, 0],
            }));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildLocalAdmissionSubmission(input with
            {
                BundleBytes = [0, 0],
            }));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildLocalAdmissionSubmission(input with
            {
                EnvelopeBytes = [],
            }));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildLocalAdmissionSubmission(input with
            {
                EnvelopeBytes = [0, 0],
            }));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildLocalAdmissionSubmission(input with
            {
                StatementHash = "0x" + new string('0', 64),
            }));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildLocalAdmissionSubmission(input with
            {
                SourceVerifierMaterialHash = "0x" + new string('0', 64),
            }));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildLocalAdmissionSubmission(input with
            {
                SourceAdapterEngineDeploymentHash = "0x" + new string('0', 64),
            }));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildLocalAdmissionSubmission(input with
            {
                EnvelopeEncoding = "abi_tuple_v1",
            }));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildLocalAdmissionSubmission(input with
            {
                ProofFamily = "debug-proof-family",
            }));
    }

    [Fact]
    public void MessageProofBundleGateRejectsMissingAndMismatchedNonSoraSourceProof()
    {
        var sourceProofBytes = NonSoraProofBundleFinalityBytes();
        var summary = SccpMessageProofBundles.RequireMatchesPublicInputs(
            BscMainnetSccp.DomainBsc,
            NonSoraProofBundleMessageId,
            NonSoraProofBundlePayloadHash,
            NonSoraProofBundleCommitmentRoot,
            42,
            SampleOutboundFinalityBlockHash,
            NonSoraProofBundleBytes(),
            sourceProofBytes);

        Assert.Equal(EthereumMainnetSccp.DomainEthereum, summary.SourceDomain);
        Assert.Equal(BscMainnetSccp.DomainBsc, summary.TargetDomain);
        Assert.Equal(NonSoraProofBundleMessageId, summary.MessageId);
        Assert.Equal(NonSoraProofBundlePayloadHash, summary.PayloadHash);
        Assert.Equal(NonSoraProofBundleCommitmentRoot, summary.CommitmentRoot);
        Assert.Equal(sourceProofBytes, summary.FinalityProofBytes);
        Assert.NotSame(sourceProofBytes, summary.FinalityProofBytes);

        var missingSourceProof = Assert.Throws<ArgumentException>(
            () => SccpMessageProofBundles.RequireMatchesPublicInputs(
                BscMainnetSccp.DomainBsc,
                NonSoraProofBundleMessageId,
                NonSoraProofBundlePayloadHash,
                NonSoraProofBundleCommitmentRoot,
                42,
                SampleOutboundFinalityBlockHash,
                NonSoraProofBundleBytes(),
                []));
        Assert.Contains(
            "sourceProofBytes required for non-SORA source bundle",
            missingSourceProof.Message);

        var mismatchedSourceProof = Assert.Throws<ArgumentException>(
            () => SccpMessageProofBundles.RequireMatchesPublicInputs(
                BscMainnetSccp.DomainBsc,
                NonSoraProofBundleMessageId,
                NonSoraProofBundlePayloadHash,
                NonSoraProofBundleCommitmentRoot,
                42,
                SampleOutboundFinalityBlockHash,
                NonSoraProofBundleBytes(),
                "wrong-source-proof"u8.ToArray()));
        Assert.Contains(
            "sourceProofBytes must match bundleBytes finality proof",
            mismatchedSourceProof.Message);

        var undecodableBundleBytes = NonSoraProofBundleBytes();
        var undecodableSourceProof = sourceProofBytes.ToArray();
        var finalityOffset = undecodableBundleBytes.AsSpan().IndexOf(sourceProofBytes);
        Assert.True(finalityOffset >= 0);
        undecodableSourceProof[0] ^= 0x01;
        undecodableSourceProof.AsSpan().CopyTo(
            undecodableBundleBytes.AsSpan(finalityOffset, undecodableSourceProof.Length));
        var undecodableSourceProofError = Assert.Throws<ArgumentException>(
            () => SccpMessageProofBundles.RequireMatchesPublicInputs(
                BscMainnetSccp.DomainBsc,
                NonSoraProofBundleMessageId,
                NonSoraProofBundlePayloadHash,
                NonSoraProofBundleCommitmentRoot,
                42,
                SampleOutboundFinalityBlockHash,
                undecodableBundleBytes,
                undecodableSourceProof));
        Assert.Contains(
            "sourceProofBytes must decode as SccpSourceChainProofEnvelopeV1",
            undecodableSourceProofError.Message);

        var publicInputDrift = Assert.Throws<ArgumentException>(
            () => SccpMessageProofBundles.RequireMatchesPublicInputs(
                BscMainnetSccp.DomainBsc,
                "0x" + new string('9', 64),
                NonSoraProofBundlePayloadHash,
                NonSoraProofBundleCommitmentRoot,
                42,
                SampleOutboundFinalityBlockHash,
                NonSoraProofBundleBytes(),
                sourceProofBytes));
        Assert.Contains("bundleBytes must match publicInputs", publicInputDrift.Message);
    }

    [Fact]
    public void MessageProofBundleGateRejectsTamperedCanonicalBundle()
    {
        var sourceProofBytes = NonSoraProofBundleFinalityBytes();
        var payloadTamper = Assert.Throws<ArgumentException>(
            () => SccpMessageProofBundles.RequireMatchesPublicInputs(
                BscMainnetSccp.DomainBsc,
                NonSoraProofBundleMessageId,
                NonSoraProofBundlePayloadHash,
                NonSoraProofBundleCommitmentRoot,
                42,
                SampleOutboundFinalityBlockHash,
                MutatedNonSoraProofBundle(146, 0x01),
                sourceProofBytes));
        Assert.Contains("bundleBytes.commitment must match payload", payloadTamper.Message);

        var rootTamper = Assert.Throws<ArgumentException>(
            () => SccpMessageProofBundles.RequireMatchesPublicInputs(
                BscMainnetSccp.DomainBsc,
                NonSoraProofBundleMessageId,
                NonSoraProofBundlePayloadHash,
                NonSoraProofBundleCommitmentRoot,
                42,
                SampleOutboundFinalityBlockHash,
                MutatedNonSoraProofBundle(1, 0x01),
                sourceProofBytes));
        Assert.Contains("bundleBytes.commitment_root must match merkle proof", rootTamper.Message);
    }

    [Fact]
    public void BscOutboundProofRequestRejectsBundleSourceDomainDrift()
    {
        var bscDestinationBinding = BscMainnetSccp.DestinationBinding(
            "0x" + new string('1', 40),
            "0x" + new string('2', 40),
            "0x" + new string('b', 64),
            "0x" + new string('c', 64));

        var driftError = Assert.Throws<ArgumentException>(
            () => BscMainnetSccp.BuildOutboundProofRequest(
                new BscMainnetOutboundProofRequestInput
                {
                    PublicInputs = new BscMainnetTransparentPublicInputs(
                        Version: 1,
                        MessageId: NonSoraProofBundleMessageId,
                        PayloadHash: NonSoraProofBundlePayloadHash,
                        TargetDomain: BscMainnetSccp.DomainBsc,
                        CommitmentRoot: NonSoraProofBundleCommitmentRoot,
                        FinalityHeight: 42,
                        FinalityBlockHash: SampleOutboundFinalityBlockHash),
                    BundleBytes = NonSoraProofBundleBytes(),
                    SourceProofBytes = NonSoraProofBundleFinalityBytes(),
                    StatementHash = "0x" + new string('5', 64),
                    DestinationBinding = bscDestinationBinding,
                    DestinationBindingHash = bscDestinationBinding.BindingHash,
                }));

        Assert.Contains("bundleBytes.sourceDomain must match sourceDomain", driftError.Message);
    }

    [Fact]
    public async Task OutboundProofPathRejectsCrossLaneAndMalformedProofs()
    {
        var binding = SampleDestinationBinding();
        var publicInputs = SamplePublicInputs();
        var input = SampleOutboundInput(binding, publicInputs);
        var request = EthereumMainnetSccp.BuildOutboundProofRequest(input);
        var proofResult = EthereumMainnetSccp.WrapOutboundProofResult(Groth16ProofBytes(), request);

        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildOutboundProofRequest(
                SampleOutboundInput(
                    binding,
                    publicInputs with { TargetDomain = 2 })));
        var guardedProver = new OutboundProverStub(Groth16ProofBytes());
        await Assert.ThrowsAsync<ArgumentException>(
            async () => await EthereumMainnetSccp.ProveOutboundToEthereumAsync(
                SampleOutboundInput(
                    binding,
                    publicInputs with { TargetDomain = BscMainnetSccp.DomainBsc }),
                guardedProver));
        // Ethereum outbound prover callback must not see BSC requests.
        Assert.Null(guardedProver.Request);
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildOutboundProofRequest(
                input with
                {
                    DestinationBindingHash = "0x" + new string('9', 64),
                }));
        var zeroBundleError = Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildOutboundProofRequest(
                input with
                {
                    BundleBytes = [0, 0],
                }));
        Assert.Contains("BundleBytes must not be all zero", zeroBundleError.Message);
        var malformedBundleError = Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildOutboundProofRequest(
                input with
                {
                    BundleBytes = [1, 2, 3],
                }));
        Assert.Contains("bundleBytes.commitment_root is too short", malformedBundleError.Message);
        var publicInputDriftError = Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildOutboundProofRequest(
                input with
                {
                    PublicInputs = publicInputs with
                    {
                        MessageId = "0x" + new string('9', 64),
                    },
                }));
        Assert.Contains("bundleBytes must match publicInputs", publicInputDriftError.Message);
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildEthereumCalldata(
                new EthereumMainnetSccpSubmissionInput(
                    proofResult with
                    {
                        DestinationBinding = proofResult.DestinationBinding with
                        {
                            NetworkId =
                                "0x0000000000000000000000000000000000000000000000000000000000000038",
                        },
                    })));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildEthereumCalldata(
                new EthereumMainnetSccpSubmissionInput(
                    proofResult with
                    {
                        DestinationBindingHash = "0x" + new string('9', 64),
                    })));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.BuildEthereumCalldata(
                new EthereumMainnetSccpSubmissionInput(
                    proofResult with
                    {
                        PublicInputs = publicInputs with
                        {
                            PayloadHash = "0x" + new string('9', 64),
                        },
                    })));

        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.WrapOutboundProofResult([1, 2, 3], request));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.WrapOutboundProofResult(
                Groth16ProofBytes(),
                request with { DestinationBindingHash = "0x" + new string('9', 64) }));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.WrapOutboundProofResult(new byte[384], request));

        var wrongMessageId = Groth16ProofBytes();
        wrongMessageId[63] = 0x12;
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.WrapOutboundProofResult(wrongMessageId, request));

        var wrongSourceDomain = Groth16ProofBytes();
        wrongSourceDomain[(2 * 32) + 31] = 0x02;
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.WrapOutboundProofResult(wrongSourceDomain, request));

        var badG1Point = Groth16ProofBytes();
        badG1Point[(5 * 32) + 31] = 0x03;
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.WrapOutboundProofResult(badG1Point, request));
    }
}
