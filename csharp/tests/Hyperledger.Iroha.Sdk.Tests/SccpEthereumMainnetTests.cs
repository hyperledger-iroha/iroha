using System.Buffers.Binary;
using System.IO;
using System.Net;
using System.Net.Http;
using System.Text;
using Hyperledger.Iroha.Sccp;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class SccpEthereumMainnetTests
{
    private const string ExpectedBindingHash =
        "0xc86f9d904df50c4522d01da3773916ebecce816f3fdfa664e2dff7cfbe697c45";
    private const string ExpectedRequestHash =
        "0x5f8c834251ab586c5beb632f058758647eb7f83c3aa22108d389db33e546c411";
    private const string ExpectedEnvelopeHash =
        "0x1bcaf6039957b7d66feae15cea46d75b026df8e28bfdd6b36926d40514400159";
    private const string ExpectedSourceBridgeConfigHash =
        "0x871a910500648c68576f7d8fb044de1c494ae24c74f435c87dd451e6ae169c6b";
    private const string ExpectedSourceAdapterVerifierVkHash =
        "0x2140903293411cad0f0eb217d8beb18d3a188edf7bba455098589a2409445e46";
    private const string ExpectedSourceVerifierMaterialHash =
        "0x4d1e9d15bc59c0a2157aa967eb033f5778c805aea4707785a31ef6b60f694d77";
    private const string ExpectedSourceAdapterEngineDeploymentHash =
        "0xfeb62925410b1376a2cd3704c3822e335da96c3dcc283b041a559d7b08ab1cc4";
    private const string ExpectedSyncCommitteeRoot =
        "0xb3343685e8ab63a2d66bccebb6c03a149a53330389473b4a495598065c17b445";
    private const string ExpectedPublicInputsBytes =
        "011111111111111111111111111111111111111111111111111111111111111111"
        + "2222222222222222222222222222222222222222222222222222222222222222"
        + "01000000"
        + "3333333333333333333333333333333333333333333333333333333333333333"
        + "2a00000000000000"
        + "4444444444444444444444444444444444444444444444444444444444444444";
    private const string ExpectedCallDataHex =
        "0xbd57826c0000000000000000000000000000000000000000000000000000000000000100"
        + "1111111111111111111111111111111111111111111111111111111111111111"
        + "2222222222222222222222222222222222222222222222222222222222222222"
        + "0000000000000000000000000000000000000000000000000000000000000001"
        + "3333333333333333333333333333333333333333333333333333333333333333"
        + "000000000000000000000000000000000000000000000000000000000000002a"
        + "4444444444444444444444444444444444444444444444444444444444444444"
        + "5555555555555555555555555555555555555555555555555555555555555555"
        + "0000000000000000000000000000000000000000000000000000000000000180"
        + "0000000000000000000000000000000000000000000000000000000000000001"
        + "1111111111111111111111111111111111111111111111111111111111111111"
        + "0000000000000000000000000000000000000000000000000000000000000000"
        + "3333333333333333333333333333333333333333333333333333333333333333"
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

    private static readonly string[] ExpectedPublicSignalWords =
    [
        "0x0ffdbc782e79d1dc508e08af01e87f16d93b6e58e4861a0b8155455e3ee7a683",
        "0x0c5398ea95021a790e276e3ece1592b32b85751dc77e50293c867a5f2e0131bb",
        "0x2eb6b5dbab56255a979f433862429637ba1e8251106271606f0a279f593d7a39",
        "0x01c73f2f9156a52493a9beabeec73e62deed32fcef2e3e6fac86a79f0764f0bc",
        "0x220a98afe36b6d6828e7e852988c8595f0ad6d128e845e74e0161cb0fa2f642f",
        "0x2b153d0fe1bc6e2a6d44e851523edb1511dac55443ca80c22cbe9cb7423886dc",
        "0x2697e4e42f34b673b4aa254c6a92de09304e84c1a667c7d266777775a231efb4",
        "0x16fbe0c1d659f142b3e7815b24df66da3cfd89cc42d051b04bc31aae6925c396",
        "0x02dcef873274cccbb6bde309daaabeec707adc38755c2d118518ecd716151da3",
    ];

    private static readonly string[] ExpectedPublicInputWords =
    [
        "0x1111111111111111111111111111111111111111111111111111111111111111",
        "0x2222222222222222222222222222222222222222222222222222222222222222",
        "0x0000000000000000000000000000000000000000000000000000000000000001",
        "0x3333333333333333333333333333333333333333333333333333333333333333",
        "0x000000000000000000000000000000000000000000000000000000000000002a",
        "0x4444444444444444444444444444444444444444444444444444444444444444",
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
            Assert.Same(expectedReceipt, receipt);
            Assert.Same(expectedBlock, block);
            Assert.Equal(expectedTransactionHash, transactionHash);
            return ValueTask.FromResult<IReadOnlyDictionary<string, object?>?>(finality);
        }
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

    private sealed class OutboundProverStub(byte[] proofBytes) : IEthereumMainnetOutboundProver
    {
        public EthereumMainnetOutboundProofRequest? Request { get; private set; }

        public ValueTask<byte[]> ProveAsync(
            EthereumMainnetOutboundProofRequest request,
            CancellationToken cancellationToken = default)
        {
            Request = request;
            Assert.Equal(ExpectedRequestHash, request.RequestHash);
            Assert.Equal(ExpectedBindingHash, request.DestinationBindingHash);
            Assert.Equal(ExpectedPublicSignalWords, request.PublicSignalWords);
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
            MessageId: "0x" + new string('1', 64),
            PayloadHash: "0x" + new string('2', 64),
            TargetDomain: EthereumMainnetSccp.DomainEthereum,
            CommitmentRoot: "0x" + new string('3', 64),
            FinalityHeight: 42,
            FinalityBlockHash: "0x" + new string('4', 64));

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
            BundleBytes = "eth-mainnet-bundle"u8.ToArray(),
            SourceProofBytes = "eth-source-proof"u8.ToArray(),
            StatementHash = "0x" + new string('5', 64),
            DestinationBinding = binding ?? SampleDestinationBinding(),
            DestinationBindingHash = (binding ?? SampleDestinationBinding()).BindingHash,
            SourceDomain = EthereumMainnetSccp.DomainSora,
        };

    private static byte[] Groth16ProofBytes()
        => Concat(
            AbiWord(1),
            RepeatByte(0x11, 32),
            AbiWord((ulong)EthereumMainnetSccp.DomainSora),
            RepeatByte(0x33, 32),
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

    private static byte[] RepeatByte(byte value, int count)
    {
        var bytes = new byte[count];
        Array.Fill(bytes, value);
        return bytes;
    }

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

    private static byte[] SampleSyncCommitteePayload()
        => Concat(
            [0x01],
            LeU32(2),
            LeU32(48),
            RepeatByte(0x33, 48),
            LeU64(3),
            LeU32(96),
            RepeatByte(0xcc, 96),
            LeU32(48),
            RepeatByte(0x44, 48),
            LeU64(4),
            LeU32(96),
            RepeatByte(0xdd, 96));

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
        char rootNibble = 'd',
        string slot = "64")
        => $$"""
        {
          "execution_optimistic": {{executionOptimistic.ToString().ToLowerInvariant()}},
          "finalized": {{finalized.ToString().ToLowerInvariant()}},
          "data": {
            "root": "0x{{new string(rootNibble, 64)}}",
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

    private static string BeaconCheckpointJson(char rootNibble = 'd')
        => $$"""
        {
          "execution_optimistic": false,
          "finalized": true,
          "data": {
            "finalized": {
              "root": "0x{{new string(rootNibble, 64)}}",
              "epoch": "2"
            }
          }
        }
        """;

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
        string? syncCommitteeSignature = null)
        => $$"""
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
            "sync_aggregate": {
              "sync_committee_bits": "{{syncCommitteeBits ?? ("0x01" + new string('0', 126))}}",
              "sync_committee_signature": "{{syncCommitteeSignature ?? ("0x" + string.Concat(Enumerable.Repeat("34", 96)))}}"
            },
            "signature_slot": "{{signatureSlot}}"
          }
        }
        """;

    private static string BeaconBlockRootJson(char rootNibble = 'd')
        => $$"""
        {
          "execution_optimistic": false,
          "finalized": true,
          "data": {
            "root": "0x{{new string(rootNibble, 64)}}"
          }
        }
        """;

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
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.SourceAdapterEngineDeploymentHash(deployment with
            {
                AdapterVerifierVkHash = "0x" + new string('9', 64),
            }));
        Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.SourceAdapterEngineDeploymentHash(deployment with
            {
                DeploymentReceiptHash = ExpectedSourceAdapterVerifierVkHash,
            }));
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
            SyncCommitteeBits: "0x01" + new string('0', 126),
            SyncCommitteeSignature: "0x" + string.Concat(Enumerable.Repeat("34", 96)),
            SyncCommitteeParticipation: "1",
            SyncSignatureSlot: "65");
        var beaconFinality = beaconFinalityEvidence.ToDictionary(
            [
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
            ["syncCommitteeBits"] = "0x01" + new string('0', 126),
            ["syncCommitteeSignature"] = "0x" + string.Concat(Enumerable.Repeat("34", 96)),
            ["syncCommitteeParticipation"] = "1",
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
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.ValidateExecutionProviderMainnetAsync(
                new ExecutionProviderStub("0x01", receipt, block)).AsTask());
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.ValidateExecutionProviderMainnetAsync(
                new ExecutionProviderStub("1", receipt, block)).AsTask());
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.ValidateExecutionProviderMainnetAsync(
                new ExecutionProviderStub(1, receipt, block)).AsTask());

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
        await Assert.ThrowsAsync<ArgumentException>(
            () => EthereumMainnetSccp.CollectInboundEvidenceFromReceiptAsync(
                new EthereumMainnetInboundEvidence
                {
                    Receipt = removedReceipt,
                    Block = block,
                    BeaconFinality = beaconFinality,
                    SourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                }).AsTask());

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
            ["beaconSlot"] = "32",
        };
        var transport = new BeaconRestTransportStub((url, _) => url switch
        {
            "https://beacon.example/eth/v1/beacon/headers/finalized" =>
                BeaconResponse(BeaconHeaderJson()),
            "https://beacon.example/eth/v1/beacon/headers/32" =>
                BeaconResponse(BeaconHeaderJson(rootNibble: 'a', slot: "32")),
            "https://beacon.example/eth/v1/beacon/blocks/32/root" =>
                BeaconResponse(BeaconBlockRootJson('a')),
            "https://beacon.example/eth/v2/beacon/blocks/32" =>
                BeaconResponse(BeaconBlockJson(slot: "32", blockHash: blockHash)),
            "https://beacon.example/eth/v1/beacon/states/finalized/finality_checkpoints" =>
                BeaconResponse(BeaconCheckpointJson()),
            "https://beacon.example/eth/v1/beacon/light_client/finality_update" =>
                BeaconResponse(BeaconFinalityUpdateJson()),
            _ => throw new InvalidOperationException($"unexpected Beacon REST URL {url}"),
        });
        var syncCommitteePayload = SampleSyncCommitteePayload();
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
        Assert.Equal("0x" + new string('a', 64), evidence.BeaconFinality?["finalizedHeaderRoot"]);
        Assert.Equal(ExpectedSyncCommitteeRoot, evidence.BeaconFinality?["syncCommitteeRoot"]);
        Assert.Equal("32", evidence.BeaconFinality?["beaconSlot"]);
        Assert.Equal("0x01" + new string('0', 126), evidence.BeaconFinality?["syncCommitteeBits"]);
        Assert.Equal("0x" + string.Concat(Enumerable.Repeat("34", 96)), evidence.BeaconFinality?["syncCommitteeSignature"]);
        Assert.Equal("1", evidence.BeaconFinality?["syncCommitteeParticipation"]);
        Assert.Equal("65", evidence.BeaconFinality?["syncSignatureSlot"]);
        Assert.Equal(
            [
                "https://beacon.example/eth/v1/beacon/headers/finalized",
                "https://beacon.example/eth/v1/beacon/headers/32",
                "https://beacon.example/eth/v1/beacon/blocks/32/root",
                "https://beacon.example/eth/v2/beacon/blocks/32",
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
            ["timestamp"] = "0x1e4",
        };
        var transport = new BeaconRestTransportStub((url, _) => url switch
        {
            "https://beacon.example/eth/v1/beacon/genesis" =>
                BeaconResponse(BeaconGenesisJson("100")),
            "https://beacon.example/eth/v1/beacon/headers/finalized" =>
                BeaconResponse(BeaconHeaderJson()),
            "https://beacon.example/eth/v1/beacon/headers/32" =>
                BeaconResponse(BeaconHeaderJson(rootNibble: 'a', slot: "32")),
            "https://beacon.example/eth/v1/beacon/blocks/32/root" =>
                BeaconResponse(BeaconBlockRootJson('a')),
            "https://beacon.example/eth/v2/beacon/blocks/32" =>
                BeaconResponse(BeaconBlockJson(slot: "32", blockHash: blockHash)),
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

        Assert.Equal("0x" + new string('a', 64), evidence.BeaconFinality?["finalizedHeaderRoot"]);
        Assert.Equal("32", evidence.BeaconFinality?["beaconSlot"]);
        Assert.Equal(
            [
                "https://beacon.example/eth/v1/beacon/genesis",
                "https://beacon.example/eth/v1/beacon/headers/finalized",
                "https://beacon.example/eth/v1/beacon/headers/32",
                "https://beacon.example/eth/v1/beacon/blocks/32/root",
                "https://beacon.example/eth/v2/beacon/blocks/32",
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

        var missingSyncRoot = Assert.Throws<ArgumentException>(
            () => new EthereumMainnetBeaconRestConsensusProvider(
                "https://beacon.example",
                syncCommitteeRoot: null,
                syncCommitteePayload: null));
        Assert.Contains("requires syncCommitteeRoot or syncCommitteePayload", missingSyncRoot.Message);

        var malformedPayload = Assert.Throws<ArgumentException>(
            () => EthereumMainnetSccp.EthSyncCommitteeHashFromPayload([0]));
        Assert.Contains("syncCommitteePayload must have version 1", malformedPayload.Message);

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
        Assert.Equal("eth-mainnet-bundle"u8.ToArray(), request.BundleBytes);
        Assert.Equal("eth-source-proof"u8.ToArray(), request.SourceProofBytes);
        Assert.NotSame(input.BundleBytes, request.BundleBytes);
        Assert.NotSame(input.SourceProofBytes, request.SourceProofBytes);

        var mutableProof = Groth16ProofBytes();
        var prover = new OutboundProverStub(mutableProof);
        var proofResult = await EthereumMainnetSccp.ProveOutboundToEthereumAsync(input, prover);
        mutableProof[31] = 9;
        Assert.NotNull(prover.Request);
        Assert.Equal(1, proofResult.ProofBytes[31]);
        Assert.Equal(ExpectedRequestHash, proofResult.RequestHash);
        Assert.Equal(ExpectedEnvelopeHash, proofResult.EnvelopeHash);
        Assert.Equal(ExpectedPublicSignalWords, proofResult.PublicSignalWords);
        Assert.Equal(publicInputs, proofResult.PublicInputs);
        Assert.Equal(binding, proofResult.DestinationBinding);

        var submission = EthereumMainnetSccp.BuildEthereumCalldata(
            new EthereumMainnetSccpSubmissionInput(proofResult));
        Assert.Equal(1, submission.Version);
        Assert.Equal(EthereumMainnetSccp.StarkFriProofFamily, submission.ProofFamily);
        Assert.Equal(EthereumMainnetSccp.EvmGroth16Bn254ProofBackend, submission.VerifierBackend);
        Assert.Equal(EthereumMainnetSccp.ContractCallAbiTuple, submission.EnvelopeEncoding);
        Assert.Equal(EthereumMainnetSccp.SubmitMessageProofAbi, submission.ContractMethod);
        Assert.Equal(EthereumMainnetSccp.SubmitMessageProofSelector, submission.FunctionSelector);
        Assert.Equal(EthereumMainnetSccp.DomainSora, submission.SourceDomain);
        Assert.Equal(EthereumMainnetSccp.DomainEthereum, submission.TargetDomain);
        Assert.Equal(ExpectedPublicInputWords, submission.PublicInputWords);
        Assert.Equal(ExpectedPublicSignalWords, submission.PublicSignalWords);
        Assert.Equal(ExpectedCallDataHex, submission.CallDataHex);
        Assert.Equal(676, submission.CallData.Length);
        Assert.Equal(submission.CallData, submission.EnvelopeBytes);
        Assert.Equal(submission.CallDataHex, submission.EnvelopeHex);

        var submitter = new OutboundSubmitterStub();
        Assert.Equal(
            "eth-submitted",
            await EthereumMainnetSccp.SubmitOutboundToEthereumAsync(
                new EthereumMainnetSccpSubmissionInput(proofResult),
                submitter));
        Assert.NotNull(submitter.Submission);
        Assert.Equal(ExpectedCallDataHex, submitter.Submission.CallDataHex);

        var guardedSubmitter = new OutboundSubmitterStub();
        await Assert.ThrowsAsync<ArgumentOutOfRangeException>(
            async () => await EthereumMainnetSccp.SubmitOutboundToEthereumAsync(
                new EthereumMainnetSccpSubmissionInput(proofResult),
                guardedSubmitter,
                new ExecutionProviderStub(
                    "0x38",
                    new Dictionary<string, object?>(),
                    new Dictionary<string, object?>())));
        Assert.Null(guardedSubmitter.Submission);
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
