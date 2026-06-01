using System.Buffers.Binary;
using System.Numerics;
using System.Text;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Sccp;

/// <summary>
/// BSC mainnet SORA -> BSC outbound proof request, proof wrapping, and verifier calldata helpers.
/// </summary>
public static partial class BscMainnetSccp
{
    public const string ContractCallAbiTuple = "abi_tuple_v1";
    public const string SubmitMessageProofAbi = "submitSccpMessageProof(bytes,bytes32[6],bytes32)";
    public const string SubmitMessageProofSelector = "0xbd57826c";

    private const string ProofRequestPrefix = "sccp:evm:groth16-proof-request:v1";
    private const string ProofEnvelopePrefix = "sccp:evm:groth16-proof-envelope:v1";
    private const int Groth16Bn254ProofAbiByteLength = 384;

    private static readonly byte[] SubmitMessageProofSelectorBytes = [0xbd, 0x57, 0x82, 0x6c];
    private static readonly BigInteger Bn254ScalarFieldModulus =
        new(
            Convert.FromHexString("30644e72e131a029b85045b68181585d2833e84879b9709143e1f593f0000001"),
            isUnsigned: true,
            isBigEndian: true);
    private static readonly BigInteger Bn254BaseFieldModulus =
        new(
            Convert.FromHexString("30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47"),
            isUnsigned: true,
            isBigEndian: true);
    private static readonly BigInteger Bn254G2BC0 =
        new(
            Convert.FromHexString("2b149d40ceb8aaae81be18991be06ac3b5b4c5e559dbefa33267e6dc24a138e5"),
            isUnsigned: true,
            isBigEndian: true);
    private static readonly BigInteger Bn254G2BC1 =
        new(
            Convert.FromHexString("009713b03af0fed4cd2cafadeed8fdf4a74fa084e52d1852e4a2bd0685c315d2"),
            isUnsigned: true,
            isBigEndian: true);
    private static readonly int[] Bn254ScalarFieldBits = ScalarBits(Bn254ScalarFieldModulus);
    private static readonly string[] Groth16Bn254SignalLabels =
    [
        "sccp:groth16-bn254:signal:message-id:v1",
        "sccp:groth16-bn254:signal:payload-hash:v1",
        "sccp:groth16-bn254:signal:target-domain:v1",
        "sccp:groth16-bn254:signal:commitment-root:v1",
        "sccp:groth16-bn254:signal:finality-height:v1",
        "sccp:groth16-bn254:signal:finality-block-hash:v1",
        "sccp:groth16-bn254:signal:source-domain:v1",
        "sccp:groth16-bn254:signal:statement-hash:v1",
        "sccp:groth16-bn254:signal:destination-binding-hash:v1",
    ];

    private readonly record struct Bn254Fq2(BigInteger C0, BigInteger C1);

    private readonly record struct Bn254G2Projective(
        Bn254Fq2 X,
        Bn254Fq2 Y,
        Bn254Fq2 Z,
        bool Infinity);

    public static BscMainnetOutboundProofRequest BuildOutboundProofRequest(
        BscMainnetOutboundProofRequestInput input)
    {
        ArgumentNullException.ThrowIfNull(input);
        ArgumentNullException.ThrowIfNull(input.PublicInputs);
        ArgumentNullException.ThrowIfNull(input.DestinationBinding);
        RequireOutboundRoute(input.SourceDomain, input.PublicInputs.TargetDomain);

        var publicInputs = NormalizePublicInputs(input.PublicInputs);
        var destinationBinding = RequireBscDestinationBinding(input.DestinationBinding);
        var destinationBindingHash = NormalizeNonZeroHex(
            input.DestinationBindingHash ?? destinationBinding.BindingHash,
            nameof(input.DestinationBindingHash),
            32);
        if (!string.Equals(destinationBindingHash, destinationBinding.BindingHash, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "destinationBindingHash must match BSC mainnet destinationBinding.",
                nameof(input));
        }

        var statementHash = NormalizeNonZeroHex(input.StatementHash, nameof(input.StatementHash), 32);
        var bundleBytes = RequireNonEmptyBytes(input.BundleBytes, nameof(input.BundleBytes));
        var sourceProofBytes = NormalizeOptionalNonZeroBytes(input.SourceProofBytes, nameof(input.SourceProofBytes));
        var publicInputsBytes = CanonicalPublicInputsBytes(publicInputs);
        var publicSignalWords = PublicSignalWords(
            publicInputs,
            input.SourceDomain,
            statementHash,
            destinationBindingHash);
        var requestHash = ComputeProofRequestHash(
            publicInputsBytes,
            bundleBytes,
            sourceProofBytes,
            statementHash,
            destinationBindingHash,
            publicSignalWords);

        return new BscMainnetOutboundProofRequest(
            Version: 1,
            Backend: EvmGroth16Bn254ProofBackend,
            SourceDomain: input.SourceDomain,
            TargetDomain: DomainBsc,
            PublicInputs: publicInputs,
            PublicInputsBytes: publicInputsBytes,
            PublicSignalWords: publicSignalWords,
            BundleBytes: bundleBytes,
            SourceProofBytes: sourceProofBytes,
            ProofContext: new BscMainnetSccpProofContext(statementHash, destinationBindingHash),
            StatementHash: statementHash,
            DestinationBindingHash: destinationBindingHash,
            RequestHash: requestHash,
            DestinationBinding: destinationBinding);
    }

    public static async ValueTask<BscMainnetOutboundProofResult> ProveOutboundToBscAsync(
        BscMainnetOutboundProofRequestInput input,
        IBscMainnetOutboundProver outboundProver,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(outboundProver);

        var request = BuildOutboundProofRequest(input);
        var proofBytes = await outboundProver.ProveAsync(
            Snapshot(request),
            cancellationToken).ConfigureAwait(false);
        return WrapOutboundProofResult(proofBytes, request);
    }

    public static BscMainnetOutboundProofResult WrapOutboundProofResult(
        byte[] proofBytes,
        BscMainnetOutboundProofRequest request)
    {
        ArgumentNullException.ThrowIfNull(request);
        RequireBscProofRequest(request);

        var proofCopy = RequireGroth16ProofBytesForContext(
            proofBytes,
            request.PublicInputs,
            request.SourceDomain,
            nameof(proofBytes));
        var requestHash = ComputeProofRequestHash(request);
        if (!string.Equals(requestHash, request.RequestHash, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "requestHash must match BSC mainnet proof request fields.",
                nameof(request));
        }

        var envelopeHash = PrefixedBlake2bHex(
            Encoding.UTF8.GetBytes(ProofEnvelopePrefix),
            Concat(HexToBytes(request.RequestHash, 32), proofCopy));
        return new BscMainnetOutboundProofResult(
            Version: 1,
            Backend: EvmGroth16Bn254ProofBackend,
            ProofBytes: proofCopy,
            ProofBase64: Convert.ToBase64String(proofCopy),
            Request: Snapshot(request),
            PublicInputs: request.PublicInputs,
            PublicSignalWords: request.PublicSignalWords.ToArray(),
            StatementHash: request.StatementHash,
            DestinationBindingHash: request.DestinationBindingHash,
            ProofContext: request.ProofContext,
            RequestHash: request.RequestHash,
            EnvelopeHash: envelopeHash,
            DestinationBinding: request.DestinationBinding);
    }

    public static BscMainnetSccpSubmission BuildBscCalldata(
        BscMainnetSccpSubmissionInput input)
    {
        ArgumentNullException.ThrowIfNull(input);
        ArgumentNullException.ThrowIfNull(input.ProofResult);
        var proofResult = input.ProofResult;
        RequireBscProofResult(proofResult);

        var publicInputWords = PublicInputAbiWords(proofResult.PublicInputs);
        var publicInputWordBytes = Concat(publicInputWords.Select(HexToBytes32).ToArray());
        var callData = SccpSubmitMessageProofCallData(
            proofResult.ProofBytes,
            publicInputWords,
            proofResult.StatementHash,
            proofResult.Request.SourceDomain);
        return new BscMainnetSccpSubmission(
            Version: 1,
            ProofFamily: StarkFriProofFamily,
            VerifierBackend: EvmGroth16Bn254ProofBackend,
            PlatformPayload: "evm_groth16_contract_call",
            EnvelopeEncoding: ContractCallAbiTuple,
            SubmissionKind: "contract_call",
            VerifierEntrypoint:
                "submitSccpMessageProof(bytes proof_bytes, bytes32[6] public_inputs, bytes32 statement_hash)",
            ContractMethod: SubmitMessageProofAbi,
            FunctionSelector: SubmitMessageProofSelector,
            SourceDomain: proofResult.Request.SourceDomain,
            TargetDomain: DomainBsc,
            PublicInputs: proofResult.PublicInputs,
            PublicInputWords: publicInputWords,
            PublicSignalWords: proofResult.PublicSignalWords.ToArray(),
            StatementHash: proofResult.StatementHash,
            DestinationBindingHash: proofResult.DestinationBindingHash,
            Arguments:
            [
                new BscMainnetSccpSubmissionArgument(
                    "proof_bytes",
                    "raw_bytes",
                    ToHex(proofResult.ProofBytes)),
                new BscMainnetSccpSubmissionArgument(
                    "public_inputs",
                    "abi_bytes32x6",
                    ToHex(publicInputWordBytes)),
                new BscMainnetSccpSubmissionArgument(
                    "statement_hash",
                    "abi_bytes32",
                    proofResult.StatementHash),
            ],
            CallData: callData,
            CallDataHex: ToHex(callData),
            EnvelopeBytes: callData.ToArray(),
            EnvelopeHex: ToHex(callData),
            ProofBytes: proofResult.ProofBytes.ToArray(),
            PublicInputWordsBytes: publicInputWordBytes);
    }

    public static async ValueTask<object?> SubmitOutboundToBscAsync(
        BscMainnetSccpSubmissionInput input,
        IBscMainnetOutboundSubmitter outboundSubmitter,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(outboundSubmitter);

        var submission = BuildBscCalldata(input);
        return await outboundSubmitter.SubmitAsync(submission, cancellationToken).ConfigureAwait(false);
    }

    private static BscMainnetTransparentPublicInputs NormalizePublicInputs(
        BscMainnetTransparentPublicInputs input)
    {
        if (input.Version != 1)
        {
            throw new ArgumentOutOfRangeException(
                nameof(input),
                input.Version,
                "BSC mainnet SCCP public inputs must use version 1.");
        }

        if (input.TargetDomain != DomainBsc)
        {
            throw new ArgumentException(
                "BSC mainnet SCCP public inputs must target BSC.",
                nameof(input));
        }

        if (input.FinalityHeight == 0)
        {
            throw new ArgumentException(
                "BSC mainnet SCCP publicInputs.finalityHeight must not be zero.",
                nameof(input));
        }

        return input with
        {
            MessageId = NormalizeNonZeroHex(input.MessageId, nameof(input.MessageId), 32),
            PayloadHash = NormalizeNonZeroHex(input.PayloadHash, nameof(input.PayloadHash), 32),
            CommitmentRoot = NormalizeNonZeroHex(input.CommitmentRoot, nameof(input.CommitmentRoot), 32),
            FinalityBlockHash = NormalizeNonZeroHex(input.FinalityBlockHash, nameof(input.FinalityBlockHash), 32),
        };
    }

    private static byte[] CanonicalPublicInputsBytes(BscMainnetTransparentPublicInputs input)
    {
        using var payload = new MemoryStream();
        payload.WriteByte((byte)input.Version);
        payload.Write(HexToBytes(input.MessageId, 32));
        payload.Write(HexToBytes(input.PayloadHash, 32));
        payload.Write(LeU32(input.TargetDomain));
        payload.Write(HexToBytes(input.CommitmentRoot, 32));
        payload.Write(LeU64(input.FinalityHeight));
        payload.Write(HexToBytes(input.FinalityBlockHash, 32));
        return payload.ToArray();
    }

    private static string[] PublicInputAbiWords(BscMainnetTransparentPublicInputs input)
    {
        return
        [
            input.MessageId,
            input.PayloadHash,
            ToHex(AbiWordU32(input.TargetDomain)),
            input.CommitmentRoot,
            ToHex(AbiWordU64(input.FinalityHeight)),
            input.FinalityBlockHash,
        ];
    }

    private static string[] PublicSignalWords(
        BscMainnetTransparentPublicInputs input,
        int sourceDomain,
        string statementHash,
        string destinationBindingHash)
    {
        var values = new[]
        {
            HexToBytes(input.MessageId, 32),
            HexToBytes(input.PayloadHash, 32),
            AbiWordU32(input.TargetDomain),
            HexToBytes(input.CommitmentRoot, 32),
            AbiWordU64(input.FinalityHeight),
            HexToBytes(input.FinalityBlockHash, 32),
            AbiWordU32(sourceDomain),
            HexToBytes(statementHash, 32),
            HexToBytes(destinationBindingHash, 32),
        };
        var words = new string[Groth16Bn254SignalLabels.Length];
        for (var index = 0; index < Groth16Bn254SignalLabels.Length; index++)
        {
            words[index] = Groth16Bn254SignalWord(Groth16Bn254SignalLabels[index], values[index]);
        }

        return words;
    }

    private static string Groth16Bn254SignalWord(string label, byte[] value)
    {
        var labelHash = Keccak256(Encoding.UTF8.GetBytes(label));
        var digest = Keccak256(Concat(labelHash, value));
        var reduced = new BigInteger(digest, isUnsigned: true, isBigEndian: true)
            % Bn254ScalarFieldModulus;
        var bytes = reduced.ToByteArray(isUnsigned: true, isBigEndian: true);
        if (bytes.Length > 32)
        {
            throw new InvalidOperationException("BN254 signal word does not fit bytes32.");
        }

        var word = new byte[32];
        bytes.CopyTo(word.AsSpan(32 - bytes.Length));
        return ToHex(word);
    }

    private static string ComputeProofRequestHash(BscMainnetOutboundProofRequest request)
    {
        RequireBscProofRequestShape(request);
        return ComputeProofRequestHash(
            request.PublicInputsBytes,
            request.BundleBytes,
            request.SourceProofBytes,
            request.StatementHash,
            request.DestinationBindingHash,
            request.PublicSignalWords);
    }

    private static string ComputeProofRequestHash(
        byte[] publicInputsBytes,
        byte[] bundleBytes,
        byte[] sourceProofBytes,
        string statementHash,
        string destinationBindingHash,
        IReadOnlyList<string> publicSignalWords)
    {
        if (publicSignalWords.Count != 9)
        {
            throw new ArgumentException(
                "BSC mainnet SCCP publicSignalWords must contain 9 words.",
                nameof(publicSignalWords));
        }

        using var payload = new MemoryStream();
        payload.Write(publicInputsBytes);
        payload.Write(WriteBytes(bundleBytes));
        payload.Write(WriteBytes(sourceProofBytes));
        payload.Write(HexToBytes(statementHash, 32));
        payload.Write(HexToBytes(destinationBindingHash, 32));
        foreach (var word in publicSignalWords)
        {
            payload.Write(FixedHexToBytes(word, "publicSignalWords", 32));
        }

        return PrefixedBlake2bHex(Encoding.UTF8.GetBytes(ProofRequestPrefix), payload.ToArray());
    }

    private static string PrefixedBlake2bHex(byte[] prefix, byte[] payload)
        => ToHex(Blake2b.Hash256(Concat(prefix, payload)));

    private static byte[] SccpSubmitMessageProofCallData(
        byte[] proofBytes,
        IReadOnlyList<string> publicInputWords,
        string statementHash,
        int sourceDomain)
    {
        if (sourceDomain != DomainSora)
        {
            throw new ArgumentException("BSC mainnet verifier calldata must prove SORA-origin messages.");
        }

        if (publicInputWords.Count != 6)
        {
            throw new ArgumentException("publicInputWords must contain 6 ABI words.", nameof(publicInputWords));
        }

        var proofCopy = RequireGroth16ProofBytes(proofBytes, nameof(proofBytes));
        using var payload = new MemoryStream();
        payload.Write(SubmitMessageProofSelectorBytes);
        payload.Write(AbiWordU256(32UL * 8UL));
        foreach (var word in publicInputWords)
        {
            payload.Write(FixedHexToBytes(word, "publicInputWords", 32));
        }

        payload.Write(HexToBytes(statementHash, 32));
        payload.Write(AbiWordU256((ulong)proofCopy.Length));
        payload.Write(proofCopy);
        var padding = (32 - proofCopy.Length % 32) % 32;
        if (padding > 0)
        {
            payload.Write(new byte[padding]);
        }

        return payload.ToArray();
    }

    private static void RequireBscProofResult(BscMainnetOutboundProofResult proofResult)
    {
        if (proofResult.Version != 1)
        {
            throw new ArgumentException("proofResult.version must be 1.");
        }

        if (!string.Equals(proofResult.Backend, EvmGroth16Bn254ProofBackend, StringComparison.Ordinal))
        {
            throw new ArgumentException("proofResult.backend must be evm-groth16-bn254-v1.");
        }

        RequireBscProofRequest(proofResult.Request);
        var publicInputs = NormalizePublicInputs(proofResult.PublicInputs);
        if (!publicInputs.Equals(NormalizePublicInputs(proofResult.Request.PublicInputs)))
        {
            throw new ArgumentException("proofResult.publicInputs must match request.publicInputs.");
        }

        RequireGroth16ProofBytesForContext(
            proofResult.ProofBytes,
            publicInputs,
            proofResult.Request.SourceDomain,
            nameof(proofResult.ProofBytes));
        var expectedRequestHash = ComputeProofRequestHash(proofResult.Request);
        if (!string.Equals(expectedRequestHash, proofResult.RequestHash, StringComparison.Ordinal))
        {
            throw new ArgumentException("proofResult.requestHash must match request fields.");
        }

        if (!string.Equals(
                Convert.ToBase64String(proofResult.ProofBytes),
                proofResult.ProofBase64,
                StringComparison.Ordinal))
        {
            throw new ArgumentException("proofResult.proofBase64 must match proof bytes.");
        }

        if (!string.Equals(proofResult.StatementHash, proofResult.Request.StatementHash, StringComparison.Ordinal)
            || !string.Equals(
                proofResult.ProofContext.StatementHash,
                proofResult.Request.StatementHash,
                StringComparison.Ordinal))
        {
            throw new ArgumentException("proofResult statement hash must match request.");
        }

        if (!string.Equals(
                proofResult.DestinationBindingHash,
                proofResult.Request.DestinationBindingHash,
                StringComparison.Ordinal)
            || !string.Equals(
                proofResult.ProofContext.DestinationBindingHash,
                proofResult.Request.DestinationBindingHash,
                StringComparison.Ordinal))
        {
            throw new ArgumentException("proofResult destination binding hash must match request.");
        }

        if (!proofResult.PublicSignalWords.SequenceEqual(proofResult.Request.PublicSignalWords))
        {
            throw new ArgumentException("proofResult publicSignalWords must match request.");
        }

        var expectedEnvelopeHash = PrefixedBlake2bHex(
            Encoding.UTF8.GetBytes(ProofEnvelopePrefix),
            Concat(HexToBytes(proofResult.RequestHash, 32), proofResult.ProofBytes));
        if (!string.Equals(expectedEnvelopeHash, proofResult.EnvelopeHash, StringComparison.Ordinal))
        {
            throw new ArgumentException("proofResult.envelopeHash must match wrapped proof bytes.");
        }

        var destinationBinding = RequireBscDestinationBinding(proofResult.DestinationBinding);
        if (!string.Equals(destinationBinding.BindingHash, proofResult.DestinationBindingHash, StringComparison.Ordinal))
        {
            throw new ArgumentException("proofResult destinationBindingHash must match destinationBinding.");
        }
    }

    private static void RequireBscProofRequest(BscMainnetOutboundProofRequest request)
    {
        RequireBscProofRequestShape(request);
        var expectedRequestHash = ComputeProofRequestHash(
            request.PublicInputsBytes,
            request.BundleBytes,
            request.SourceProofBytes,
            request.StatementHash,
            request.DestinationBindingHash,
            request.PublicSignalWords);
        if (!string.Equals(expectedRequestHash, request.RequestHash, StringComparison.Ordinal))
        {
            throw new ArgumentException("requestHash must match BSC mainnet proof request fields.");
        }
    }

    private static void RequireBscProofRequestShape(BscMainnetOutboundProofRequest request)
    {
        ArgumentNullException.ThrowIfNull(request);
        if (request.Version != 1
            || !string.Equals(request.Backend, EvmGroth16Bn254ProofBackend, StringComparison.Ordinal))
        {
            throw new ArgumentException("BSC mainnet proof requests must use EVM Groth16 v1.");
        }

        RequireOutboundRoute(request.SourceDomain, request.TargetDomain);
        if (request.PublicInputs.TargetDomain != DomainBsc)
        {
            throw new ArgumentException("BSC mainnet proof request public inputs must target BSC.");
        }

        var publicInputs = NormalizePublicInputs(request.PublicInputs);
        if (!publicInputs.Equals(request.PublicInputs))
        {
            throw new ArgumentException("publicInputs must be canonical.");
        }

        if (!CanonicalPublicInputsBytes(publicInputs).SequenceEqual(request.PublicInputsBytes))
        {
            throw new ArgumentException("publicInputsBytes must match publicInputs.");
        }

        RequireNonEmptyBytes(request.BundleBytes, nameof(request.BundleBytes));
        NormalizeOptionalNonZeroBytes(request.SourceProofBytes, nameof(request.SourceProofBytes));
        var statementHash = NormalizeNonZeroHex(request.StatementHash, nameof(request.StatementHash), 32);
        var destinationBinding = RequireBscDestinationBinding(request.DestinationBinding);
        if (!string.Equals(destinationBinding.BindingHash, request.DestinationBindingHash, StringComparison.Ordinal))
        {
            throw new ArgumentException("destinationBindingHash must match BSC mainnet destinationBinding.");
        }

        var publicSignalWords = PublicSignalWords(
            publicInputs,
            request.SourceDomain,
            statementHash,
            request.DestinationBindingHash);
        if (!publicSignalWords.SequenceEqual(request.PublicSignalWords))
        {
            throw new ArgumentException("publicSignalWords must match public inputs and proof context.");
        }
    }

    private static BscMainnetSccpDestinationBinding RequireBscDestinationBinding(
        BscMainnetSccpDestinationBinding binding)
    {
        ArgumentNullException.ThrowIfNull(binding);
        var normalized = DestinationBinding(
            binding.VerifierAddress,
            binding.BridgeAddress,
            binding.VerifierCodeHash,
            binding.VerifierKeyHash,
            binding.NetworkId,
            binding.SourceDomain,
            binding.TargetDomain,
            binding.BindingHash,
            binding.Key);
        if (!string.Equals(binding.VerifierBackend, EvmGroth16Bn254ProofBackend, StringComparison.Ordinal)
            || !string.Equals(binding.ProofFamily, StarkFriProofFamily, StringComparison.Ordinal))
        {
            throw new ArgumentException("BSC mainnet destinationBinding verifier profile is invalid.");
        }

        return normalized;
    }

    private static byte[] RequireGroth16ProofBytes(byte[] proofBytes, string parameterName)
    {
        var proofCopy = RequireNonZeroProofBytes(proofBytes, parameterName);
        if (proofCopy.Length != Groth16Bn254ProofAbiByteLength)
        {
            throw new ArgumentException(
                $"{parameterName} must be {Groth16Bn254ProofAbiByteLength} bytes.",
                parameterName);
        }

        RequireGroth16Bn254ProofTuple(proofCopy, parameterName);
        return proofCopy;
    }

    private static byte[] RequireGroth16ProofBytesForContext(
        byte[] proofBytes,
        BscMainnetTransparentPublicInputs publicInputs,
        int sourceDomain,
        string parameterName)
    {
        var proofCopy = RequireGroth16ProofBytes(proofBytes, parameterName);
        var normalizedInputs = NormalizePublicInputs(publicInputs);
        if (!ProofWord(proofCopy, 1).SequenceEqual(HexToBytes(normalizedInputs.MessageId, 32)))
        {
            throw new ArgumentException(
                $"{parameterName}.messageId must match publicInputs.messageId.",
                parameterName);
        }

        if (!ProofWord(proofCopy, 3).SequenceEqual(HexToBytes(normalizedInputs.CommitmentRoot, 32)))
        {
            throw new ArgumentException(
                $"{parameterName}.commitmentRoot must match publicInputs.commitmentRoot.",
                parameterName);
        }

        if (ProofWordValue(proofCopy, 2) != sourceDomain)
        {
            throw new ArgumentException(
                $"{parameterName}.sourceDomain must match sourceDomain.",
                parameterName);
        }

        return proofCopy;
    }

    private static void RequireGroth16Bn254ProofTuple(byte[] proofBytes, string parameterName)
    {
        if (ProofWordValue(proofBytes, 0) != BigInteger.One)
        {
            throw new ArgumentException($"{parameterName}.version must be 1.", parameterName);
        }

        if (ProofWordIsZero(proofBytes, 1))
        {
            throw new ArgumentException($"{parameterName}.messageId must not be zero.", parameterName);
        }

        if (ProofWordValue(proofBytes, 2) > uint.MaxValue)
        {
            throw new ArgumentException($"{parameterName}.sourceDomain must fit u32.", parameterName);
        }

        if (ProofWordIsZero(proofBytes, 3))
        {
            throw new ArgumentException($"{parameterName}.commitmentRoot must not be zero.", parameterName);
        }

        var fields = new[] { "a.x", "a.y", "b.x0", "b.x1", "b.y0", "b.y1", "c.x", "c.y" };
        for (var index = 0; index < fields.Length; index++)
        {
            RequireGroth16BaseFieldWord(proofBytes, 4 + index, $"{parameterName}.{fields[index]}");
        }

        RequireGroth16G1Point(proofBytes, [4, 5], $"{parameterName}.a");
        RequireGroth16G2Point(proofBytes, [6, 7, 8, 9], $"{parameterName}.b");
        RequireGroth16G1Point(proofBytes, [10, 11], $"{parameterName}.c");
    }

    private static byte[] ProofWord(byte[] proofBytes, int index)
    {
        var word = new byte[32];
        proofBytes.AsSpan(index * 32, 32).CopyTo(word);
        return word;
    }

    private static BigInteger ProofWordValue(byte[] proofBytes, int index)
        => new(proofBytes.AsSpan(index * 32, 32), isUnsigned: true, isBigEndian: true);

    private static bool ProofWordIsZero(byte[] proofBytes, int index)
        => proofBytes.AsSpan(index * 32, 32).IndexOfAnyExcept((byte)0) < 0;

    private static void RequireGroth16BaseFieldWord(byte[] proofBytes, int index, string label)
    {
        if (ProofWordValue(proofBytes, index) >= Bn254BaseFieldModulus)
        {
            throw new ArgumentException($"{label} must be a BN254 base-field element.");
        }
    }

    private static void RequireGroth16NonZeroPoint(byte[] proofBytes, IReadOnlyList<int> indexes, string label)
    {
        if (indexes.All(index => ProofWordIsZero(proofBytes, index)))
        {
            throw new ArgumentException($"{label} must not be zero.");
        }
    }

    private static void RequireGroth16G1Point(byte[] proofBytes, IReadOnlyList<int> indexes, string label)
    {
        RequireGroth16NonZeroPoint(proofBytes, indexes, label);
        var x = ProofWordValue(proofBytes, indexes[0]);
        var y = ProofWordValue(proofBytes, indexes[1]);
        if (Bn254Fq(y * y) != Bn254Fq(x * x * x + 3))
        {
            throw new ArgumentException($"{label} must be a BN254 G1 point.");
        }
    }

    private static void RequireGroth16G2Point(byte[] proofBytes, IReadOnlyList<int> indexes, string label)
    {
        RequireGroth16NonZeroPoint(proofBytes, indexes, label);
        var x = new Bn254Fq2(
            ProofWordValue(proofBytes, indexes[0]),
            ProofWordValue(proofBytes, indexes[1]));
        var y = new Bn254Fq2(
            ProofWordValue(proofBytes, indexes[2]),
            ProofWordValue(proofBytes, indexes[3]));
        var left = Bn254Fq2Mul(y, y);
        var x2 = Bn254Fq2Mul(x, x);
        var right = Bn254Fq2Add(Bn254Fq2Mul(x2, x), new Bn254Fq2(Bn254G2BC0, Bn254G2BC1));
        if (!left.Equals(right) || !Bn254G2PointIsInPrimeSubgroup(x, y))
        {
            throw new ArgumentException($"{label} must be a BN254 G2 point.");
        }
    }

    private static BigInteger Bn254Fq(BigInteger value)
    {
        var reduced = value % Bn254BaseFieldModulus;
        return reduced.Sign < 0 ? reduced + Bn254BaseFieldModulus : reduced;
    }

    private static Bn254Fq2 Bn254Fq2Add(Bn254Fq2 left, Bn254Fq2 right)
        => new(Bn254Fq(left.C0 + right.C0), Bn254Fq(left.C1 + right.C1));

    private static Bn254Fq2 Bn254Fq2Sub(Bn254Fq2 left, Bn254Fq2 right)
        => new(Bn254Fq(left.C0 - right.C0), Bn254Fq(left.C1 - right.C1));

    private static Bn254Fq2 Bn254Fq2Scale(Bn254Fq2 left, BigInteger scalar)
        => new(Bn254Fq(left.C0 * scalar), Bn254Fq(left.C1 * scalar));

    private static Bn254Fq2 Bn254Fq2Mul(Bn254Fq2 left, Bn254Fq2 right)
        => new(
            Bn254Fq(left.C0 * right.C0 - left.C1 * right.C1),
            Bn254Fq(left.C0 * right.C1 + left.C1 * right.C0));

    private static bool Bn254Fq2IsZero(Bn254Fq2 value)
        => value.C0.IsZero && value.C1.IsZero;

    private static Bn254G2Projective Bn254G2Infinity()
        => new(
            new Bn254Fq2(BigInteger.Zero, BigInteger.Zero),
            new Bn254Fq2(BigInteger.One, BigInteger.Zero),
            new Bn254Fq2(BigInteger.Zero, BigInteger.Zero),
            true);

    private static Bn254G2Projective Bn254G2AffineProjective(Bn254Fq2 x, Bn254Fq2 y)
        => new(x, y, new Bn254Fq2(BigInteger.One, BigInteger.Zero), false);

    private static bool Bn254G2ProjectiveIsInfinity(Bn254G2Projective point)
        => point.Infinity || Bn254Fq2IsZero(point.Z);

    private static Bn254G2Projective Bn254G2ProjectiveDouble(Bn254G2Projective point)
    {
        if (Bn254G2ProjectiveIsInfinity(point) || Bn254Fq2IsZero(point.Y))
        {
            return Bn254G2Infinity();
        }

        var xx = Bn254Fq2Mul(point.X, point.X);
        var yy = Bn254Fq2Mul(point.Y, point.Y);
        var yyyy = Bn254Fq2Mul(yy, yy);
        var s = Bn254Fq2Scale(
            Bn254Fq2Sub(
                Bn254Fq2Sub(
                    Bn254Fq2Mul(Bn254Fq2Add(point.X, yy), Bn254Fq2Add(point.X, yy)),
                    xx),
                yyyy),
            2);
        var m = Bn254Fq2Scale(xx, 3);
        var x3 = Bn254Fq2Sub(Bn254Fq2Mul(m, m), Bn254Fq2Scale(s, 2));
        var y3 = Bn254Fq2Sub(
            Bn254Fq2Mul(m, Bn254Fq2Sub(s, x3)),
            Bn254Fq2Scale(yyyy, 8));
        var z3 = Bn254Fq2Scale(Bn254Fq2Mul(point.Y, point.Z), 2);
        return new Bn254G2Projective(x3, y3, z3, false);
    }

    private static Bn254G2Projective Bn254G2ProjectiveAddAffine(
        Bn254G2Projective point,
        Bn254Fq2 affineX,
        Bn254Fq2 affineY)
    {
        if (Bn254G2ProjectiveIsInfinity(point))
        {
            return Bn254G2AffineProjective(affineX, affineY);
        }

        var z1z1 = Bn254Fq2Mul(point.Z, point.Z);
        var u2 = Bn254Fq2Mul(affineX, z1z1);
        var s2 = Bn254Fq2Mul(affineY, Bn254Fq2Mul(point.Z, z1z1));
        var h = Bn254Fq2Sub(u2, point.X);
        if (Bn254Fq2IsZero(h))
        {
            return s2.Equals(point.Y) ? Bn254G2ProjectiveDouble(point) : Bn254G2Infinity();
        }

        var hh = Bn254Fq2Mul(h, h);
        var i = Bn254Fq2Scale(hh, 4);
        var j = Bn254Fq2Mul(h, i);
        var r = Bn254Fq2Scale(Bn254Fq2Sub(s2, point.Y), 2);
        var v = Bn254Fq2Mul(point.X, i);
        var x3 = Bn254Fq2Sub(Bn254Fq2Sub(Bn254Fq2Mul(r, r), j), Bn254Fq2Scale(v, 2));
        var y3 = Bn254Fq2Sub(
            Bn254Fq2Mul(r, Bn254Fq2Sub(v, x3)),
            Bn254Fq2Scale(Bn254Fq2Mul(point.Y, j), 2));
        var z3 = Bn254Fq2Sub(
            Bn254Fq2Sub(
                Bn254Fq2Mul(Bn254Fq2Add(point.Z, h), Bn254Fq2Add(point.Z, h)),
                z1z1),
            hh);
        return new Bn254G2Projective(x3, y3, z3, false);
    }

    private static bool Bn254G2PointIsInPrimeSubgroup(Bn254Fq2 x, Bn254Fq2 y)
    {
        var acc = Bn254G2Infinity();
        foreach (var bit in Bn254ScalarFieldBits)
        {
            acc = Bn254G2ProjectiveDouble(acc);
            if (bit == 1)
            {
                acc = Bn254G2ProjectiveAddAffine(acc, x, y);
            }
        }

        return Bn254G2ProjectiveIsInfinity(acc);
    }

    private static int[] ScalarBits(BigInteger value)
    {
        var bytes = value.ToByteArray(isUnsigned: true, isBigEndian: true);
        var bits = new List<int>(bytes.Length * 8);
        var started = false;
        foreach (var item in bytes)
        {
            for (var bit = 7; bit >= 0; bit--)
            {
                var selected = (item >> bit) & 1;
                if (selected == 1)
                {
                    started = true;
                }

                if (started)
                {
                    bits.Add(selected);
                }
            }
        }

        return bits.ToArray();
    }

    private static byte[] RequireNonEmptyBytes(byte[] bytes, string parameterName)
    {
        ArgumentNullException.ThrowIfNull(bytes);
        if (bytes.Length == 0)
        {
            throw new ArgumentException($"{parameterName} must not be empty.", parameterName);
        }

        return bytes.ToArray();
    }

    private static byte[] NormalizeOptionalNonZeroBytes(byte[]? bytes, string parameterName)
    {
        if (bytes is null || bytes.Length == 0)
        {
            return [];
        }

        if (!bytes.Any(static value => value != 0))
        {
            throw new ArgumentException($"{parameterName} must not be all zero.", parameterName);
        }

        return bytes.ToArray();
    }

    private static BscMainnetOutboundProofRequest Snapshot(
        BscMainnetOutboundProofRequest request)
    {
        return request with
        {
            PublicInputsBytes = request.PublicInputsBytes.ToArray(),
            PublicSignalWords = request.PublicSignalWords.ToArray(),
            BundleBytes = request.BundleBytes.ToArray(),
            SourceProofBytes = request.SourceProofBytes.ToArray(),
        };
    }

    private static byte[] AbiWordU64(ulong value)
    {
        var outBytes = new byte[32];
        BinaryPrimitives.WriteUInt64BigEndian(outBytes.AsSpan(24, 8), value);
        return outBytes;
    }

    private static byte[] AbiWordU256(ulong value)
    {
        var outBytes = new byte[32];
        BinaryPrimitives.WriteUInt64BigEndian(outBytes.AsSpan(24, 8), value);
        return outBytes;
    }

    private static byte[] LeU32(int value)
    {
        if (value < 0)
        {
            throw new ArgumentOutOfRangeException(nameof(value), value, "Domain id must be u32.");
        }

        var outBytes = new byte[4];
        BinaryPrimitives.WriteUInt32LittleEndian(outBytes, (uint)value);
        return outBytes;
    }

    private static byte[] LeU64(ulong value)
    {
        var outBytes = new byte[8];
        BinaryPrimitives.WriteUInt64LittleEndian(outBytes, value);
        return outBytes;
    }

    private static byte[] WriteBytes(byte[] value)
    {
        var length = new byte[4];
        BinaryPrimitives.WriteUInt32LittleEndian(length, checked((uint)value.Length));
        return Concat(length, value);
    }

    private static byte[] HexToBytes32(string value)
        => FixedHexToBytes(value, nameof(value), 32);

    private static byte[] FixedHexToBytes(string value, string parameterName, int byteLength)
    {
        if (value is null)
        {
            throw new ArgumentNullException(parameterName);
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new ArgumentException($"{parameterName} must be canonical hex.", parameterName);
        }

        var text = value.StartsWith("0x", StringComparison.OrdinalIgnoreCase)
            ? value[2..]
            : value;
        if (text.Length != byteLength * 2 || text.Length == 0 || text.Length % 2 != 0)
        {
            throw new ArgumentException($"{parameterName} must be {byteLength} bytes.", parameterName);
        }

        try
        {
            return Convert.FromHexString(text);
        }
        catch (FormatException ex)
        {
            throw new ArgumentException($"{parameterName} must be hex.", parameterName, ex);
        }
    }

    private static byte[] Concat(params byte[][] chunks)
    {
        var total = checked(chunks.Sum(static chunk => chunk.Length));
        var output = new byte[total];
        var offset = 0;
        foreach (var chunk in chunks)
        {
            chunk.CopyTo(output.AsSpan(offset));
            offset += chunk.Length;
        }

        return output;
    }
}

public sealed record BscMainnetTransparentPublicInputs(
    int Version,
    string MessageId,
    string PayloadHash,
    int TargetDomain,
    string CommitmentRoot,
    ulong FinalityHeight,
    string FinalityBlockHash);

public sealed record BscMainnetSccpProofContext(
    string StatementHash,
    string DestinationBindingHash);

public sealed record BscMainnetOutboundProofRequestInput
{
    public BscMainnetTransparentPublicInputs? PublicInputs { get; init; }

    public byte[] BundleBytes { get; init; } = [];

    public byte[]? SourceProofBytes { get; init; }

    public string StatementHash { get; init; } = string.Empty;

    public string? DestinationBindingHash { get; init; }

    public int SourceDomain { get; init; } = BscMainnetSccp.DomainSora;

    public BscMainnetSccpDestinationBinding? DestinationBinding { get; init; }
}

public sealed record BscMainnetOutboundProofRequest(
    int Version,
    string Backend,
    int SourceDomain,
    int TargetDomain,
    BscMainnetTransparentPublicInputs PublicInputs,
    byte[] PublicInputsBytes,
    string[] PublicSignalWords,
    byte[] BundleBytes,
    byte[] SourceProofBytes,
    BscMainnetSccpProofContext ProofContext,
    string StatementHash,
    string DestinationBindingHash,
    string RequestHash,
    BscMainnetSccpDestinationBinding DestinationBinding);

public sealed record BscMainnetOutboundProofResult(
    int Version,
    string Backend,
    byte[] ProofBytes,
    string ProofBase64,
    BscMainnetOutboundProofRequest Request,
    BscMainnetTransparentPublicInputs PublicInputs,
    string[] PublicSignalWords,
    string StatementHash,
    string DestinationBindingHash,
    BscMainnetSccpProofContext ProofContext,
    string RequestHash,
    string EnvelopeHash,
    BscMainnetSccpDestinationBinding DestinationBinding);

public sealed record BscMainnetSccpSubmissionInput(
    BscMainnetOutboundProofResult ProofResult);

public sealed record BscMainnetSccpSubmissionArgument(
    string Key,
    string Encoding,
    string Bytes);

public sealed record BscMainnetSccpSubmission(
    int Version,
    string ProofFamily,
    string VerifierBackend,
    string PlatformPayload,
    string EnvelopeEncoding,
    string SubmissionKind,
    string VerifierEntrypoint,
    string ContractMethod,
    string FunctionSelector,
    int SourceDomain,
    int TargetDomain,
    BscMainnetTransparentPublicInputs PublicInputs,
    string[] PublicInputWords,
    string[] PublicSignalWords,
    string StatementHash,
    string DestinationBindingHash,
    BscMainnetSccpSubmissionArgument[] Arguments,
    byte[] CallData,
    string CallDataHex,
    byte[] EnvelopeBytes,
    string EnvelopeHex,
    byte[] ProofBytes,
    byte[] PublicInputWordsBytes);

public interface IBscMainnetOutboundProver
{
    ValueTask<byte[]> ProveAsync(
        BscMainnetOutboundProofRequest request,
        CancellationToken cancellationToken = default);
}

public interface IBscMainnetOutboundSubmitter
{
    ValueTask<object?> SubmitAsync(
        BscMainnetSccpSubmission submission,
        CancellationToken cancellationToken = default);
}
