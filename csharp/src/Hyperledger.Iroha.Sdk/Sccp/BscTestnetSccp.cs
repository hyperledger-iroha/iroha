using System.Buffers.Binary;
using System.Text;

namespace Hyperledger.Iroha.Sccp;

/// <summary>
/// BSC testnet SCCP constants, route validators, and destination bindings for native .NET callers.
/// </summary>
public static class BscTestnetSccp
{
    public const int DomainSora = 0;
    public const int DomainBsc = 2;
    public const ulong TestnetChainId = 97;
    public const string EvmGroth16Bn254ProofBackend = "evm-groth16-bn254-v1";
    public const string StarkFriProofFamily = "stark-fri-v1";
    public const string LocalAdmissionEnvelopeEncoding = "norito:sccp-local-admission:v1";
    public const string LocalAdmissionSubmissionKind = "local_admission";
    public const string LocalAdmissionEntrypoint = "SubmitBridgeProof";
    public const int NativeRecursiveMaxProofBytes = 2 * 1024 * 1024;
    public const string TestnetNetworkId =
        "0x0000000000000000000000000000000000000000000000000000000000000061";

    private const string EvmDestinationBindingLabel = "iroha:sccp:evm-destination-binding:v1";
    private const int Keccak256Rate = 136;

    private static readonly int[] KeccakRhoOffsets =
    [
        0, 1, 62, 28, 27,
        36, 44, 6, 55, 20,
        3, 10, 43, 25, 39,
        41, 45, 15, 21, 8,
        18, 2, 61, 56, 14,
    ];

    private static readonly ulong[] KeccakRoundConstants =
    [
        0x0000000000000001UL,
        0x0000000000008082UL,
        0x800000000000808aUL,
        0x8000000080008000UL,
        0x000000000000808bUL,
        0x0000000080000001UL,
        0x8000000080008081UL,
        0x8000000000008009UL,
        0x000000000000008aUL,
        0x0000000000000088UL,
        0x0000000080008009UL,
        0x000000008000000aUL,
        0x000000008000808bUL,
        0x800000000000008bUL,
        0x8000000000008089UL,
        0x8000000000008003UL,
        0x8000000000008002UL,
        0x8000000000000080UL,
        0x000000000000800aUL,
        0x800000008000000aUL,
        0x8000000080008081UL,
        0x8000000000008080UL,
        0x0000000080000001UL,
        0x8000000080008008UL,
    ];

    public static void RequireTestnetChainId(ulong chainId)
    {
        if (chainId != TestnetChainId)
        {
            throw new ArgumentOutOfRangeException(
                nameof(chainId),
                chainId,
                "BSC testnet SCCP requires eth_chainId == 97.");
        }
    }

    public static void RequireTestnetNetworkId(string networkId)
    {
        if (!string.Equals(networkId, TestnetNetworkId, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "BSC testnet SCCP destination bindings must use the canonical chain id 97 "
                    + "bytes32 network id.",
                nameof(networkId));
        }
    }

    public static void RequireInboundRoute(int sourceDomain, int targetDomain)
    {
        if (sourceDomain != DomainBsc || targetDomain != DomainSora)
        {
            throw new ArgumentException("BSC testnet inbound SCCP proofs must route BSC -> SORA.");
        }
    }

    public static void RequireOutboundRoute(int sourceDomain, int targetDomain)
    {
        if (sourceDomain != DomainSora || targetDomain != DomainBsc)
        {
            throw new ArgumentException("BSC testnet outbound SCCP proofs must route SORA -> BSC.");
        }
    }

    public static BscTestnetSccpDestinationBinding DestinationBinding(
        string verifierAddress,
        string bridgeAddress,
        string verifierCodeHash,
        string verifierKeyHash,
        string? networkId = null,
        int sourceDomain = DomainSora,
        int targetDomain = DomainBsc,
        string? expectedBindingHash = null,
        string? expectedKey = null)
    {
        RequireOutboundRoute(sourceDomain, targetDomain);

        var canonicalNetworkId = NormalizeNonZeroHex(
            networkId ?? TestnetNetworkId,
            nameof(networkId),
            32);
        if (!string.Equals(canonicalNetworkId, TestnetNetworkId, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "BSC testnet destination bindings must use chain id 97.",
                nameof(networkId));
        }

        var canonicalVerifierAddress = NormalizeNonZeroHex(
            verifierAddress,
            nameof(verifierAddress),
            20);
        var canonicalBridgeAddress = NormalizeNonZeroHex(
            bridgeAddress,
            nameof(bridgeAddress),
            20);
        if (string.Equals(canonicalVerifierAddress, canonicalBridgeAddress, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "BSC testnet verifierAddress must differ from bridgeAddress.",
                nameof(verifierAddress));
        }

        var canonicalVerifierCodeHash = NormalizeNonZeroHex(
            verifierCodeHash,
            nameof(verifierCodeHash),
            32);
        var canonicalVerifierKeyHash = NormalizeNonZeroHex(
            verifierKeyHash,
            nameof(verifierKeyHash),
            32);
        var key =
            $"evm:{sourceDomain}:{targetDomain}:{canonicalNetworkId[2..]}:" +
            $"{canonicalVerifierAddress}:{canonicalBridgeAddress}:" +
            $"{canonicalVerifierCodeHash}:{canonicalVerifierKeyHash}";
        var bindingHash = ComputeDestinationBindingHash(
            canonicalNetworkId,
            sourceDomain,
            targetDomain,
            canonicalVerifierAddress,
            canonicalBridgeAddress,
            canonicalVerifierCodeHash,
            canonicalVerifierKeyHash);

        if (expectedBindingHash is not null)
        {
            var canonicalExpectedBindingHash = NormalizeNonZeroHex(
                expectedBindingHash,
                nameof(expectedBindingHash),
                32);
            if (!string.Equals(canonicalExpectedBindingHash, bindingHash, StringComparison.Ordinal))
            {
                throw new ArgumentException(
                    "expectedBindingHash must match the BSC testnet destination binding.",
                    nameof(expectedBindingHash));
            }
        }

        if (expectedKey is not null && !string.Equals(expectedKey.Trim(), key, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "expectedKey must match the BSC testnet destination binding.",
                nameof(expectedKey));
        }

        return new BscTestnetSccpDestinationBinding(
            Version: 1,
            SourceDomain: sourceDomain,
            TargetDomain: targetDomain,
            NetworkId: canonicalNetworkId,
            VerifierAddress: canonicalVerifierAddress,
            BridgeAddress: canonicalBridgeAddress,
            VerifierCodeHash: canonicalVerifierCodeHash,
            VerifierKeyHash: canonicalVerifierKeyHash,
            VerifierBackend: EvmGroth16Bn254ProofBackend,
            ProofFamily: StarkFriProofFamily,
            Key: key,
            BindingHash: bindingHash);
    }

    public static string DestinationBindingHash(
        string verifierAddress,
        string bridgeAddress,
        string verifierCodeHash,
        string verifierKeyHash,
        string? networkId = null)
    {
        return DestinationBinding(
            verifierAddress,
            bridgeAddress,
            verifierCodeHash,
            verifierKeyHash,
            networkId).BindingHash;
    }

    public static BscTestnetLocalAdmissionSubmission BuildLocalAdmissionSubmission(
        BscTestnetLocalAdmissionSubmissionInput input)
    {
        ArgumentNullException.ThrowIfNull(input);
        RequireInboundRoute(input.SourceDomain, input.TargetDomain);

        if (!string.Equals(input.EnvelopeEncoding, LocalAdmissionEnvelopeEncoding, StringComparison.Ordinal)
            || !string.Equals(input.SubmissionKind, LocalAdmissionSubmissionKind, StringComparison.Ordinal)
            || !string.Equals(input.VerifierEntrypoint, LocalAdmissionEntrypoint, StringComparison.Ordinal)
            || !string.Equals(input.ProofFamily, StarkFriProofFamily, StringComparison.Ordinal)
            || !string.Equals(input.VerifierBackend, EvmGroth16Bn254ProofBackend, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "BSC testnet local-admission submission metadata is not canonical.",
                nameof(input));
        }

        var proofBytes = RequireNativeRecursiveBytes(input.ProofBytes, nameof(input.ProofBytes));
        var publicInputsBytes = RequireNativeRecursiveBytes(
            input.PublicInputsBytes,
            nameof(input.PublicInputsBytes));
        var bundleBytes = RequireNativeRecursiveBytes(input.BundleBytes, nameof(input.BundleBytes));
        var envelopeBytes = RequireNativeRecursiveBytes(input.EnvelopeBytes, nameof(input.EnvelopeBytes));
        var statementHash = NormalizeNonZeroHex(input.StatementHash, nameof(input.StatementHash), 32);
        var sourceVerifierMaterialHash = NormalizeNonZeroHex(
            input.SourceVerifierMaterialHash,
            nameof(input.SourceVerifierMaterialHash),
            32);
        var sourceAdapterEngineDeploymentHash = NormalizeNonZeroHex(
            input.SourceAdapterEngineDeploymentHash,
            nameof(input.SourceAdapterEngineDeploymentHash),
            32);
        var payload = new BscTestnetLocalAdmissionPayload(
            ProofBytes: proofBytes,
            PublicInputsBytes: publicInputsBytes,
            BundleBytes: bundleBytes,
            StatementHash: statementHash,
            SourceVerifierMaterialHash: sourceVerifierMaterialHash,
            SourceAdapterEngineDeploymentHash: sourceAdapterEngineDeploymentHash);

        return new BscTestnetLocalAdmissionSubmission(
            ProofFamily: input.ProofFamily,
            VerifierBackend: input.VerifierBackend,
            SourceDomain: DomainBsc,
            TargetDomain: DomainSora,
            StatementHash: statementHash,
            SourceVerifierMaterialHash: sourceVerifierMaterialHash,
            SourceAdapterEngineDeploymentHash: sourceAdapterEngineDeploymentHash,
            LocalAdmission: payload,
            ProofBytes: proofBytes,
            PublicInputsBytes: publicInputsBytes,
            BundleBytes: bundleBytes,
            EnvelopeBytes: envelopeBytes);
    }

    private static string ComputeDestinationBindingHash(
        string networkId,
        int sourceDomain,
        int targetDomain,
        string verifierAddress,
        string bridgeAddress,
        string verifierCodeHash,
        string verifierKeyHash)
    {
        using var payload = new MemoryStream();
        payload.Write(Keccak256(Encoding.UTF8.GetBytes(EvmDestinationBindingLabel)));
        payload.Write(Keccak256(Encoding.UTF8.GetBytes(EvmGroth16Bn254ProofBackend)));
        payload.Write(Keccak256(Encoding.UTF8.GetBytes(StarkFriProofFamily)));
        payload.Write(HexToBytes(networkId, 32));
        payload.Write(AbiWordU32(sourceDomain));
        payload.Write(AbiWordU32(targetDomain));
        payload.Write(AbiWordAddress20(verifierAddress));
        payload.Write(AbiWordAddress20(bridgeAddress));
        payload.Write(HexToBytes(verifierCodeHash, 32));
        payload.Write(HexToBytes(verifierKeyHash, 32));
        return ToHex(Keccak256(payload.ToArray()));
    }

    private static bool IsLowerHex(char character)
    {
        return character is >= '0' and <= '9' or >= 'a' and <= 'f';
    }

    private static byte[] AbiWordU32(int value)
    {
        if (value < 0)
        {
            throw new ArgumentOutOfRangeException(nameof(value), value, "Domain id must be u32.");
        }

        var outBytes = new byte[32];
        BinaryPrimitives.WriteUInt32BigEndian(outBytes.AsSpan(28, 4), (uint)value);
        return outBytes;
    }

    private static byte[] AbiWordAddress20(string value)
    {
        var address = HexToBytes(value, 20);
        var outBytes = new byte[32];
        address.CopyTo(outBytes.AsSpan(12));
        return outBytes;
    }

    private static string NormalizeNonZeroHex(string value, string parameterName, int byteLength)
    {
        if (value is null)
        {
            throw new ArgumentNullException(parameterName);
        }

        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new ArgumentException($"{parameterName} must be canonical hex.", parameterName);
        }

        if (byteLength != 20
            && (!value.StartsWith("0x", StringComparison.Ordinal)
                || value.Length != byteLength * 2 + 2
                || !value[2..].All(IsLowerHex)))
        {
            throw new ArgumentException(
                $"{parameterName} must be canonical lowercase 0x-prefixed {byteLength}-byte hex.",
                parameterName);
        }

        var text = value.StartsWith("0x", StringComparison.OrdinalIgnoreCase)
            ? value[2..]
            : value;
        if (text.Length != byteLength * 2 || text.Length == 0 || text.Length % 2 != 0)
        {
            throw new ArgumentException($"{parameterName} must be {byteLength} bytes.", parameterName);
        }

        byte[] bytes;
        try
        {
            bytes = Convert.FromHexString(text);
        }
        catch (FormatException ex)
        {
            throw new ArgumentException($"{parameterName} must be hex.", parameterName, ex);
        }

        if (!bytes.Any(static value => value != 0))
        {
            throw new ArgumentException($"{parameterName} must not be zero.", parameterName);
        }

        return ToHex(bytes);
    }

    private static byte[] HexToBytes(string value, int byteLength)
    {
        var normalized = NormalizeNonZeroHex(value, nameof(value), byteLength);
        return Convert.FromHexString(normalized[2..]);
    }

    private static byte[] RequireNativeRecursiveBytes(byte[] bytes, string parameterName)
    {
        ArgumentNullException.ThrowIfNull(bytes, parameterName);
        if (bytes.Length == 0)
        {
            throw new ArgumentException($"{parameterName} must not be empty.", parameterName);
        }

        if (!bytes.Any(static value => value != 0))
        {
            throw new ArgumentException($"{parameterName} must not be all zero.", parameterName);
        }

        if (bytes.Length > NativeRecursiveMaxProofBytes)
        {
            throw new ArgumentException(
                $"{parameterName} must be at most {NativeRecursiveMaxProofBytes} bytes.",
                parameterName);
        }

        return bytes.ToArray();
    }

    private static string ToHex(ReadOnlySpan<byte> value)
    {
        return "0x" + Convert.ToHexString(value).ToLowerInvariant();
    }

    private static byte[] Keccak256(ReadOnlySpan<byte> data)
    {
        var state = new ulong[25];
        var offset = 0;
        while (offset + Keccak256Rate <= data.Length)
        {
            AbsorbKeccakBlock(state, data.Slice(offset, Keccak256Rate));
            KeccakF1600(state);
            offset += Keccak256Rate;
        }

        Span<byte> block = stackalloc byte[Keccak256Rate];
        data[offset..].CopyTo(block);
        block[data.Length - offset] ^= 0x01;
        block[Keccak256Rate - 1] ^= 0x80;
        AbsorbKeccakBlock(state, block);
        KeccakF1600(state);

        var output = new byte[32];
        var written = 0;
        Span<byte> laneBytes = stackalloc byte[8];
        for (var lane = 0; written < output.Length; lane++)
        {
            BinaryPrimitives.WriteUInt64LittleEndian(laneBytes, state[lane]);
            var count = Math.Min(8, output.Length - written);
            laneBytes[..count].CopyTo(output.AsSpan(written));
            written += count;
        }

        return output;
    }

    private static void AbsorbKeccakBlock(ulong[] state, ReadOnlySpan<byte> block)
    {
        for (var lane = 0; lane < Keccak256Rate / 8; lane++)
        {
            state[lane] ^= BinaryPrimitives.ReadUInt64LittleEndian(block.Slice(lane * 8, 8));
        }
    }

    private static void KeccakF1600(ulong[] state)
    {
        Span<ulong> c = stackalloc ulong[5];
        Span<ulong> d = stackalloc ulong[5];
        Span<ulong> b = stackalloc ulong[25];

        foreach (var roundConstant in KeccakRoundConstants)
        {
            for (var x = 0; x < 5; x++)
            {
                c[x] = state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20];
            }

            for (var x = 0; x < 5; x++)
            {
                d[x] = c[(x + 4) % 5] ^ RotateLeft(c[(x + 1) % 5], 1);
            }

            for (var x = 0; x < 5; x++)
            {
                for (var y = 0; y < 5; y++)
                {
                    state[x + 5 * y] ^= d[x];
                }
            }

            for (var x = 0; x < 5; x++)
            {
                for (var y = 0; y < 5; y++)
                {
                    var sourceIndex = x + 5 * y;
                    var targetIndex = y + 5 * ((2 * x + 3 * y) % 5);
                    b[targetIndex] = RotateLeft(state[sourceIndex], KeccakRhoOffsets[sourceIndex]);
                }
            }

            for (var x = 0; x < 5; x++)
            {
                for (var y = 0; y < 5; y++)
                {
                    state[x + 5 * y] =
                        b[x + 5 * y] ^ ((~b[((x + 1) % 5) + 5 * y]) & b[((x + 2) % 5) + 5 * y]);
                }
            }

            state[0] ^= roundConstant;
        }
    }

    private static ulong RotateLeft(ulong value, int amount)
    {
        return amount == 0 ? value : (value << amount) | (value >> (64 - amount));
    }
}

public sealed record BscTestnetSccpDestinationBinding(
    int Version,
    int SourceDomain,
    int TargetDomain,
    string NetworkId,
    string VerifierAddress,
    string BridgeAddress,
    string VerifierCodeHash,
    string VerifierKeyHash,
    string VerifierBackend,
    string ProofFamily,
    string Key,
    string BindingHash);

public sealed record BscTestnetLocalAdmissionSubmissionInput(
    byte[] ProofBytes,
    byte[] PublicInputsBytes,
    byte[] BundleBytes,
    byte[] EnvelopeBytes,
    string StatementHash,
    string SourceVerifierMaterialHash,
    string SourceAdapterEngineDeploymentHash,
    int SourceDomain = BscTestnetSccp.DomainBsc,
    int TargetDomain = BscTestnetSccp.DomainSora,
    string ProofFamily = BscTestnetSccp.StarkFriProofFamily,
    string VerifierBackend = BscTestnetSccp.EvmGroth16Bn254ProofBackend,
    string EnvelopeEncoding = BscTestnetSccp.LocalAdmissionEnvelopeEncoding,
    string SubmissionKind = BscTestnetSccp.LocalAdmissionSubmissionKind,
    string VerifierEntrypoint = BscTestnetSccp.LocalAdmissionEntrypoint);

public sealed record BscTestnetLocalAdmissionPayload(
    byte[] ProofBytes,
    byte[] PublicInputsBytes,
    byte[] BundleBytes,
    string StatementHash,
    string SourceVerifierMaterialHash,
    string SourceAdapterEngineDeploymentHash)
{
    public int Version { get; } = 1;
    public string ProofBytesHex { get; } = "0x" + Convert.ToHexString(ProofBytes).ToLowerInvariant();
    public string PublicInputsBytesHex { get; } = "0x" + Convert.ToHexString(PublicInputsBytes).ToLowerInvariant();
    public string BundleBytesHex { get; } = "0x" + Convert.ToHexString(BundleBytes).ToLowerInvariant();
}

public sealed record BscTestnetLocalAdmissionSubmission(
    string ProofFamily,
    string VerifierBackend,
    int SourceDomain,
    int TargetDomain,
    string StatementHash,
    string SourceVerifierMaterialHash,
    string SourceAdapterEngineDeploymentHash,
    BscTestnetLocalAdmissionPayload LocalAdmission,
    byte[] ProofBytes,
    byte[] PublicInputsBytes,
    byte[] BundleBytes,
    byte[] EnvelopeBytes)
{
    public int Version { get; } = 1;
    public string PlatformPayload { get; } = BscTestnetSccp.LocalAdmissionSubmissionKind;
    public string EnvelopeEncoding { get; } = BscTestnetSccp.LocalAdmissionEnvelopeEncoding;
    public string SubmissionKind { get; } = BscTestnetSccp.LocalAdmissionSubmissionKind;
    public string VerifierEntrypoint { get; } = BscTestnetSccp.LocalAdmissionEntrypoint;
    public IReadOnlyList<object> Arguments { get; } = Array.Empty<object>();
    public string ProofBytesHex { get; } = "0x" + Convert.ToHexString(ProofBytes).ToLowerInvariant();
    public string PublicInputsBytesHex { get; } = "0x" + Convert.ToHexString(PublicInputsBytes).ToLowerInvariant();
    public string BundleBytesHex { get; } = "0x" + Convert.ToHexString(BundleBytes).ToLowerInvariant();
    public string EnvelopeHex { get; } = "0x" + Convert.ToHexString(EnvelopeBytes).ToLowerInvariant();
}
