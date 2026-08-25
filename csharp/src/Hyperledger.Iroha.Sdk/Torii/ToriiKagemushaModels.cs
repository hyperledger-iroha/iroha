using System.Text.Json;
using System.Text.Json.Serialization;

namespace Hyperledger.Iroha.Torii;

/// <summary>
/// One machine-readable reason why an asset is not ready for Kagemusha issuance.
/// </summary>
public sealed record class ToriiKagemushaReadinessBlocker
{
    [JsonPropertyName("code")]
    public string Code { get; init; } = string.Empty;

    [JsonPropertyName("message")]
    public string Message { get; init; } = string.Empty;
}

/// <summary>
/// Registry identity of a verifier selected at a readiness snapshot.
/// </summary>
public sealed record class ToriiKagemushaVerifierId
{
    [JsonPropertyName("backend")]
    public string Backend { get; init; } = string.Empty;

    [JsonPropertyName("name")]
    public string Name { get; init; } = string.Empty;
}

/// <summary>
/// Key-material-free verifier record returned by Torii.
/// </summary>
public sealed record class ToriiKagemushaActiveVerifier
{
    [JsonPropertyName("id")]
    public ToriiKagemushaVerifierId Id { get; init; } = new();

    [JsonPropertyName("version")]
    public uint Version { get; init; }

    [JsonPropertyName("circuit_id")]
    public string CircuitId { get; init; } = string.Empty;

    [JsonPropertyName("commitment")]
    public string Commitment { get; init; } = string.Empty;

    [JsonPropertyName("public_inputs_schema_hash")]
    public string PublicInputsSchemaHash { get; init; } = string.Empty;

    [JsonPropertyName("max_proof_bytes")]
    public uint MaxProofBytes { get; init; }

    [JsonPropertyName("activation_height")]
    public ulong ActivationHeight { get; init; }

    [JsonPropertyName("withdrawal_height")]
    public ulong? WithdrawalHeight { get; init; }
}

/// <summary>
/// Authenticated Kagemusha V4/manifest-V4 release selected by Torii.
/// </summary>
public sealed record class ToriiKagemushaAuthenticatedArtifactSetV4
{
    [JsonPropertyName("generation")]
    public string Generation { get; init; } = string.Empty;

    [JsonPropertyName("manifest_sha256")]
    public string ManifestSha256 { get; init; } = string.Empty;

    [JsonPropertyName("release_policy_sha256")]
    public string ReleasePolicySha256 { get; init; } = string.Empty;

    [JsonPropertyName("release_attestation_sha256")]
    public string ReleaseAttestationSha256 { get; init; } = string.Empty;

    [JsonPropertyName("activation_height")]
    public ulong ActivationHeight { get; init; }

    [JsonPropertyName("withdrawal_height")]
    public ulong WithdrawalHeight { get; init; }

    [JsonPropertyName("max_proof_bytes")]
    public uint MaxProofBytes { get; init; }

    [JsonPropertyName("asset_scale")]
    public uint AssetScale { get; init; }
}

/// <summary>
/// Snapshot-bound bridge ABI-22 / Kagemusha V4 readiness projection.
/// </summary>
public sealed record class ToriiKagemushaReadinessV4
{
    [JsonPropertyName("required_bridge_abi_version")]
    public uint RequiredBridgeAbiVersion { get; init; }

    [JsonPropertyName("max_hops")]
    public uint MaxHops { get; init; }

    [JsonPropertyName("asset_definition_id")]
    public string AssetDefinitionId { get; init; } = string.Empty;

    [JsonPropertyName("asset_scale")]
    public uint? AssetScale { get; init; }

    [JsonPropertyName("evaluated_block_height")]
    public ulong EvaluatedBlockHeight { get; init; }

    [JsonPropertyName("evaluated_block_hash")]
    public string EvaluatedBlockHash { get; init; } = string.Empty;

    [JsonPropertyName("active_transfer_verifier")]
    public ToriiKagemushaActiveVerifier? ActiveTransferVerifier { get; init; }

    [JsonPropertyName("active_topup_shield_verifier")]
    public ToriiKagemushaActiveVerifier? ActiveTopUpShieldVerifier { get; init; }

    [JsonPropertyName("active_unshield_verifier")]
    public ToriiKagemushaActiveVerifier? ActiveUnshieldVerifier { get; init; }

    [JsonPropertyName("active_recursive_step_eq_verifier")]
    public ToriiKagemushaActiveVerifier? ActiveRecursiveStepEqVerifier { get; init; }

    [JsonPropertyName("active_recursive_step_ep_verifier")]
    public ToriiKagemushaActiveVerifier? ActiveRecursiveStepEpVerifier { get; init; }

    [JsonPropertyName("artifact_set")]
    public ToriiKagemushaAuthenticatedArtifactSetV4? ArtifactSet { get; init; }

    [JsonPropertyName("proof_backend_available")]
    public bool ProofBackendAvailable { get; init; }

    [JsonPropertyName("recursive_lineage_supported")]
    public bool RecursiveLineageSupported { get; init; }

    [JsonPropertyName("ready")]
    public bool Ready { get; init; }

    [JsonPropertyName("blockers")]
    public ToriiKagemushaReadinessBlocker[] Blockers { get; init; } = [];
}

/// <summary>
/// A canonical externally-produced V4 top-up request archive for Torii transport.
/// </summary>
public sealed class ToriiKagemushaTopUpRequestV4
{
    private readonly byte[] norito;

    public ToriiKagemushaTopUpRequestV4(string operationId, ReadOnlySpan<byte> norito)
    {
        OperationId = ToriiKagemushaTransport.RequireOperationId(operationId, nameof(operationId));
        this.norito = ToriiKagemushaTransport.RequireNoritoArchive(
            norito,
            ToriiKagemushaTransport.MaxTopUpNoritoRequestBytes,
            nameof(norito));
    }

    public int Version => ToriiKagemushaTransport.ManifestVersion;

    public string OperationId { get; }

    public byte[] Norito => norito.ToArray();
}

/// <summary>
/// A canonical externally-produced V4 redemption request archive for Torii transport.
/// </summary>
public sealed class ToriiKagemushaRedeemRequestV4
{
    private readonly byte[] norito;

    public ToriiKagemushaRedeemRequestV4(string operationId, ReadOnlySpan<byte> norito)
    {
        OperationId = ToriiKagemushaTransport.RequireOperationId(operationId, nameof(operationId));
        this.norito = ToriiKagemushaTransport.RequireNoritoArchive(
            norito,
            ToriiKagemushaTransport.MaxRedeemNoritoRequestBytes,
            nameof(norito));
    }

    public int Version => ToriiKagemushaTransport.ManifestVersion;

    public string OperationId { get; }

    public byte[] Norito => norito.ToArray();
}

public enum ToriiKagemushaOperationKind
{
    TopUp,
    Redeem,
}

public enum ToriiKagemushaOperationState
{
    Pending,
    Applied,
    Rejected,
}

/// <summary>
/// Initial reference returned after Torii accepts an offline command.
/// </summary>
public sealed record class ToriiKagemushaOperationReference
{
    public required string OperationId { get; init; }

    public required ToriiKagemushaOperationKind Kind { get; init; }

    public ToriiKagemushaOperationState State { get; init; } = ToriiKagemushaOperationState.Pending;

    public required string TransactionHash { get; init; }

    public required string StatusUri { get; init; }

    public ulong SubmittedAtMilliseconds { get; init; }
}

/// <summary>
/// Terminal top-up projection. Anchor and finality proof remain typed JSON
/// documents because this SDK intentionally does not ship a native prover.
/// </summary>
public sealed record class ToriiKagemushaTopUpResultV4
{
    public required string TransactionHash { get; init; }

    public ulong FinalizedBlockHeight { get; init; }

    public ulong ServerTimeMilliseconds { get; init; }

    public required JsonElement Anchor { get; init; }

    public required JsonElement FinalityProof { get; init; }
}

/// <summary>
/// Terminal redemption projection.
/// </summary>
public sealed record class ToriiKagemushaRedeemResultV4
{
    public required string TransactionHash { get; init; }

    public ulong FinalizedBlockHeight { get; init; }

    public ulong ServerTimeMilliseconds { get; init; }
}

/// <summary>
/// Stable Torii error attached to a rejected operation.
/// </summary>
public sealed record class ToriiKagemushaOperationError
{
    public required string Code { get; init; }

    public required string Message { get; init; }

    public JsonElement? Details { get; init; }
}

/// <summary>
/// Pollable status returned by <c>/v1/offline/operations/{operation_id}</c>.
/// </summary>
public sealed record class ToriiKagemushaOperationStatus
{
    public required string OperationId { get; init; }

    public required ToriiKagemushaOperationState State { get; init; }

    public ToriiKagemushaOperationKind? Kind { get; init; }

    public string? TransactionHash { get; init; }

    public ulong? SubmittedAtMilliseconds { get; init; }

    public ToriiKagemushaTopUpResultV4? TopUpResult { get; init; }

    public ToriiKagemushaRedeemResultV4? RedeemResult { get; init; }

    public ToriiKagemushaOperationError? Error { get; init; }
}

internal static class ToriiKagemushaTransport
{
    internal const int BridgeAbiVersion = 22;
    internal const int ManifestVersion = 4;
    internal const int MaxHops = 8;
    internal const int MaxTopUpNoritoRequestBytes = 512 * 1024;
    internal const int MaxRedeemNoritoRequestBytes = 48 * 1024 * 1024;

    internal static string RequireOperationId(string? value, string parameterName)
    {
        ArgumentNullException.ThrowIfNull(value, parameterName);
        if (value.Length != 64
            || value.All(static character => character == '0')
            || value.Any(static character => character is not (>= '0' and <= '9') and not (>= 'a' and <= 'f')))
        {
            throw new ArgumentException(
                "Kagemusha operation id must be non-zero lowercase 32-byte hexadecimal.",
                parameterName);
        }

        return value;
    }

    internal static byte[] RequireNoritoArchive(
        ReadOnlySpan<byte> value,
        int maximumBytes,
        string parameterName)
    {
        if (value.Length < 40 || value.Length > maximumBytes
            || !value[..4].SequenceEqual("NRT0"u8))
        {
            throw new ArgumentException(
                $"Kagemusha V4 request must be a Norito archive between 40 and {maximumBytes} bytes.",
                parameterName);
        }

        return value.ToArray();
    }
}
