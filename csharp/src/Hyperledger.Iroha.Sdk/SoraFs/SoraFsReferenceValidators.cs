using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;

namespace Hyperledger.Iroha.SoraFs;

/// <summary>
/// Canonical V1 orderbook payload selectors accepted by the Rust reference validator.
/// </summary>
public enum SoraFsOrderbookPayloadKind : uint
{
    /// <summary>An <c>OrderRequestV1</c>.</summary>
    OrderRequest = 1,
    /// <summary>An <c>OrderCancelV1</c>.</summary>
    OrderCancel = 2,
    /// <summary>A <c>TradeEventV1</c>.</summary>
    TradeEvent = 3,
    /// <summary>A <c>SettlementChannelV1</c>.</summary>
    SettlementChannel = 4,
    /// <summary>A <c>SettlementReceiptV1</c>.</summary>
    SettlementReceipt = 5,
    /// <summary>An <c>OrderbookRuntimeSnapshotV1</c>.</summary>
    RuntimeSnapshot = 6,
}

/// <summary>
/// Canonical V1 PDP payload selectors accepted by the Rust reference validator.
/// </summary>
public enum SoraFsPdpPayloadKind : uint
{
    /// <summary>A <c>PdpCommitmentV1</c>.</summary>
    Commitment = 1,
    /// <summary>A <c>PdpChallengeV1</c>.</summary>
    Challenge = 2,
    /// <summary>A <c>PdpProofV1</c>.</summary>
    Proof = 3,
}

/// <summary>
/// One ordered Governance DAG block supplied to signed-head-chain validation.
/// </summary>
public sealed class SoraFsGovernanceDagBlockInput
{
    private readonly byte[] noritoBytes;
    private readonly string? explicitLabel;

    /// <summary>
    /// Creates an immutable snapshot of one canonical block and its diagnostic label.
    /// </summary>
    public SoraFsGovernanceDagBlockInput(byte[] noritoBytes, string? label = null)
    {
        this.noritoBytes = SoraFsReferenceValidators.CopyInput(
            noritoBytes,
            nameof(noritoBytes));
        explicitLabel = label is null
            ? null
            : SoraFsReferenceValidators.ValidateLabel(label, label, nameof(label));
    }

    /// <summary>
    /// Gets a detached copy of the canonical Norito bytes.
    /// </summary>
    public byte[] NoritoBytes => (byte[])noritoBytes.Clone();

    /// <summary>
    /// Gets the explicit UTF-8 diagnostic label, or a generic display default.
    /// During chain validation an omitted label is sent as
    /// <c>governance-dag-block-{index}.to</c>.
    /// </summary>
    public string Label => explicitLabel ?? "governance-dag-block.to";

    internal NativeGovernanceInput Snapshot(int index)
    {
        var label = SoraFsReferenceValidators.ValidateLabel(
            explicitLabel,
            $"governance-dag-block-{index}.to",
            nameof(Label));
        return new NativeGovernanceInput(
            (byte[])noritoBytes.Clone(),
            SoraFsReferenceValidators.EncodeLabel(label, nameof(Label)));
    }
}

/// <summary>
/// Rust-backed SoraFS reference validators that return strict
/// <c>ValidationOutcomeV1</c> JSON.
/// </summary>
public static class SoraFsReferenceValidators
{
    /// <summary>
    /// First bridge ABI that contains both Governance DAG validator entrypoints.
    /// </summary>
    public const uint RequiredBridgeAbiVersion = 21;

    /// <summary>
    /// Bridge error code reserved for SoraFS reference operations.
    /// </summary>
    public const int BridgeReferenceError = -114;

    /// <summary>
    /// Maximum aggregate bytes accepted by one native reference call.
    /// </summary>
    public const int MaxInputBytesV1 = 67_108_864;

    /// <summary>
    /// Maximum UTF-8 label length accepted by the native reference FFI.
    /// </summary>
    public const int MaxLabelBytesV1 = 1_024;

    /// <summary>
    /// Maximum ordered root history or checkpoint-tail window accepted by
    /// signed-head-chain validation.
    /// </summary>
    public const int GovernanceDagMaxBlocksV1 = 64;

    /// <summary>
    /// Canonical byte length for every Governance DAG CID.
    /// </summary>
    public const int GovernanceDagCidBytesV1 = 32;

    /// <summary>
    /// Current <c>ValidationOutcomeV1</c> schema version.
    /// </summary>
    public const int ValidationOutcomeVersionV1 = 1;

    private const string LibraryName = "connect_norito_bridge";
    private const string ErrorsDocument =
        "docs/portal/docs/sorafs/reference-sdk/errors.md";
    private static readonly UTF8Encoding StrictUtf8 = new(
        encoderShouldEmitUTF8Identifier: false,
        throwOnInvalidBytes: true);
    private static readonly HashSet<string> OutcomeFields = new(StringComparer.Ordinal)
    {
        "status",
        "code",
        "category",
        "message",
        "action",
        "docs_url",
        "telemetry_tags",
        "context",
        "inputs",
        "version",
        "generated_at",
    };
    private static readonly HashSet<string> ContextFields = new(StringComparer.Ordinal)
    {
        "key",
        "value",
    };
    private static readonly HashSet<string> InputFields = new(StringComparer.Ordinal)
    {
        "kind",
        "path",
    };
    private static readonly HashSet<string> OutcomeCategories = new(StringComparer.Ordinal)
    {
        "validation",
        "policy",
        "signature",
        "norito",
        "internal",
    };

    /// <summary>
    /// Reports whether the current native bridge exposes the complete ABI-21
    /// Governance DAG reference surface.
    /// </summary>
    public static bool IsAvailable()
    {
        return IsAvailable(PInvokeSoraFsReferenceNativeBoundary.Instance);
    }

    /// <summary>
    /// Reports whether the current native bridge exposes all orderbook and PDP
    /// reference-validator entrypoints.
    /// </summary>
    public static bool IsOrderbookPdpAvailable()
    {
        return IsOrderbookPdpAvailable(PInvokeSoraFsReferenceNativeBoundary.Instance);
    }

    /// <summary>
    /// Validates one canonical orderbook payload.
    /// </summary>
    public static string ValidateOrderbookPayloadJson(
        SoraFsOrderbookPayloadKind kind,
        byte[] noritoBytes,
        string? label = null)
    {
        return ValidateOrderbookPayloadJson(
            kind,
            noritoBytes,
            label,
            CurrentEpochSeconds());
    }

    /// <summary>
    /// Validates one canonical orderbook payload with a caller-bound outcome timestamp.
    /// </summary>
    public static string ValidateOrderbookPayloadJson(
        SoraFsOrderbookPayloadKind kind,
        byte[] noritoBytes,
        string? label,
        long generatedAtUnix)
    {
        return ValidateOrderbookPayloadJson(
            kind,
            noritoBytes,
            label,
            generatedAtUnix,
            PInvokeSoraFsReferenceNativeBoundary.Instance);
    }

    /// <summary>
    /// Diagnoses one canonical PDP payload. A successful outcome is structural
    /// only and does not authorize production acceptance.
    /// </summary>
    public static string ValidatePdpPayloadJson(
        SoraFsPdpPayloadKind kind,
        byte[] noritoBytes,
        string? label = null)
    {
        return ValidatePdpPayloadJson(
            kind,
            noritoBytes,
            label,
            CurrentEpochSeconds());
    }

    /// <summary>
    /// Diagnoses one canonical PDP payload with a caller-bound outcome timestamp.
    /// </summary>
    public static string ValidatePdpPayloadJson(
        SoraFsPdpPayloadKind kind,
        byte[] noritoBytes,
        string? label,
        long generatedAtUnix)
    {
        return ValidatePdpPayloadJson(
            kind,
            noritoBytes,
            label,
            generatedAtUnix,
            PInvokeSoraFsReferenceNativeBoundary.Instance);
    }

    /// <summary>
    /// Diagnoses canonical PDP commitment/challenge binding.
    /// </summary>
    public static string ValidatePdpCommitmentChallengeJson(
        byte[] commitment,
        byte[] challenge,
        string? commitmentLabel = null,
        string? challengeLabel = null)
    {
        return ValidatePdpCommitmentChallengeJson(
            commitment,
            challenge,
            commitmentLabel,
            challengeLabel,
            CurrentEpochSeconds());
    }

    /// <summary>
    /// Diagnoses canonical PDP commitment/challenge binding with a caller-bound
    /// outcome timestamp.
    /// </summary>
    public static string ValidatePdpCommitmentChallengeJson(
        byte[] commitment,
        byte[] challenge,
        string? commitmentLabel,
        string? challengeLabel,
        long generatedAtUnix)
    {
        return ValidatePdpCommitmentChallengeJson(
            commitment,
            challenge,
            commitmentLabel,
            challengeLabel,
            generatedAtUnix,
            PInvokeSoraFsReferenceNativeBoundary.Instance);
    }

    /// <summary>
    /// Diagnoses canonical PDP challenge/proof binding.
    /// </summary>
    public static string ValidatePdpChallengeProofJson(
        byte[] challenge,
        byte[] proof,
        string? challengeLabel = null,
        string? proofLabel = null)
    {
        return ValidatePdpChallengeProofJson(
            challenge,
            proof,
            challengeLabel,
            proofLabel,
            CurrentEpochSeconds());
    }

    /// <summary>
    /// Diagnoses canonical PDP challenge/proof binding with a caller-bound
    /// outcome timestamp.
    /// </summary>
    public static string ValidatePdpChallengeProofJson(
        byte[] challenge,
        byte[] proof,
        string? challengeLabel,
        string? proofLabel,
        long generatedAtUnix)
    {
        return ValidatePdpChallengeProofJson(
            challenge,
            proof,
            challengeLabel,
            proofLabel,
            generatedAtUnix,
            PInvokeSoraFsReferenceNativeBoundary.Instance);
    }

    /// <summary>
    /// Exhaustively diagnoses canonical PDP commitment, challenge, proof,
    /// signature, coverage, and Merkle witnesses without evaluating admission.
    /// </summary>
    public static string ValidatePdpBundleJson(
        byte[] commitment,
        byte[] challenge,
        byte[] proof,
        string? commitmentLabel = null,
        string? challengeLabel = null,
        string? proofLabel = null)
    {
        return ValidatePdpBundleJson(
            commitment,
            challenge,
            proof,
            commitmentLabel,
            challengeLabel,
            proofLabel,
            CurrentEpochSeconds());
    }

    /// <summary>
    /// Exhaustively diagnoses a canonical PDP bundle with a caller-bound
    /// outcome timestamp.
    /// </summary>
    public static string ValidatePdpBundleJson(
        byte[] commitment,
        byte[] challenge,
        byte[] proof,
        string? commitmentLabel,
        string? challengeLabel,
        string? proofLabel,
        long generatedAtUnix)
    {
        return ValidatePdpBundleJson(
            commitment,
            challenge,
            proof,
            commitmentLabel,
            challengeLabel,
            proofLabel,
            generatedAtUnix,
            PInvokeSoraFsReferenceNativeBoundary.Instance);
    }

    /// <summary>
    /// Validates one canonical <c>GovernanceDagBlockV1</c>.
    /// </summary>
    public static string ValidateGovernanceDagBlockJson(
        byte[] noritoBytes,
        string? label = null,
        byte[]? expectedBlockCid = null)
    {
        return ValidateGovernanceDagBlockJson(
            noritoBytes,
            label,
            expectedBlockCid,
            CurrentEpochSeconds());
    }

    /// <summary>
    /// Validates one canonical <c>GovernanceDagBlockV1</c> with a caller-bound
    /// outcome timestamp.
    /// </summary>
    public static string ValidateGovernanceDagBlockJson(
        byte[] noritoBytes,
        string? label,
        byte[]? expectedBlockCid,
        long generatedAtUnix)
    {
        return ValidateGovernanceDagBlockJson(
            noritoBytes,
            label,
            expectedBlockCid,
            generatedAtUnix,
            PInvokeSoraFsReferenceNativeBoundary.Instance);
    }

    /// <summary>
    /// Validates one signed <c>GovernanceDagHeadV1</c> against an ordered
    /// complete root history or signed checkpoint-anchored tail.
    /// </summary>
    public static string ValidateGovernanceDagHeadChainJson(
        byte[] headNoritoBytes,
        IReadOnlyList<SoraFsGovernanceDagBlockInput> blocks,
        string? headLabel = null)
    {
        return ValidateGovernanceDagHeadChainJson(
            headNoritoBytes,
            blocks,
            headLabel,
            CurrentEpochSeconds());
    }

    /// <summary>
    /// Validates one signed <c>GovernanceDagHeadV1</c> against an ordered
    /// complete root history or signed checkpoint-anchored tail with a
    /// caller-bound outcome timestamp.
    /// </summary>
    public static string ValidateGovernanceDagHeadChainJson(
        byte[] headNoritoBytes,
        IReadOnlyList<SoraFsGovernanceDagBlockInput> blocks,
        string? headLabel,
        long generatedAtUnix)
    {
        return ValidateGovernanceDagHeadChainJson(
            headNoritoBytes,
            blocks,
            headLabel,
            generatedAtUnix,
            PInvokeSoraFsReferenceNativeBoundary.Instance);
    }

    internal static bool IsAvailable(ISoraFsReferenceNativeBoundary native)
    {
        try
        {
            return native.AbiVersion() >= RequiredBridgeAbiVersion
                && native.HasGovernanceDagSymbols();
        }
        catch (Exception)
        {
            return false;
        }
    }

    internal static bool IsOrderbookPdpAvailable(ISoraFsReferenceNativeBoundary native)
    {
        try
        {
            return native.AbiVersion() >= RequiredBridgeAbiVersion
                && native.HasOrderbookPdpSymbols();
        }
        catch (Exception)
        {
            return false;
        }
    }

    internal static string ValidateOrderbookPayloadJson(
        SoraFsOrderbookPayloadKind kind,
        byte[] noritoBytes,
        string? label,
        long generatedAtUnix,
        ISoraFsReferenceNativeBoundary native)
    {
        ArgumentNullException.ThrowIfNull(native);
        ValidateGeneratedAt(generatedAtUnix);
        if (!Enum.IsDefined(kind))
        {
            throw new ArgumentOutOfRangeException(nameof(kind));
        }
        var payload = CopyInput(noritoBytes, nameof(noritoBytes));
        var labelBytes = EncodeLabel(
            ValidateLabel(
                label,
                $"sdk:sorafs.orderbook.{OrderbookKindLabel(kind)}",
                nameof(label)),
            nameof(label));
        RequireAggregateBound("Orderbook validation", payload.Length, labelBytes.Length);
        RequireOrderbookPdpBridge(native);
        NativeValidationResult result;
        try
        {
            result = native.ValidateOrderbookPayload(
                (uint)kind,
                payload,
                labelBytes,
                checked((ulong)generatedAtUnix));
        }
        catch (Exception error)
        {
            throw new InvalidOperationException(
                "SoraFS orderbook native validation failed.",
                error);
        }
        return ReadAndValidateOutcome(
            "SoraFS orderbook validation",
            result,
            checked((ulong)generatedAtUnix),
            native);
    }

    internal static string ValidatePdpPayloadJson(
        SoraFsPdpPayloadKind kind,
        byte[] noritoBytes,
        string? label,
        long generatedAtUnix,
        ISoraFsReferenceNativeBoundary native)
    {
        ArgumentNullException.ThrowIfNull(native);
        ValidateGeneratedAt(generatedAtUnix);
        if (!Enum.IsDefined(kind))
        {
            throw new ArgumentOutOfRangeException(nameof(kind));
        }
        var payload = CopyInput(noritoBytes, nameof(noritoBytes));
        var labelBytes = EncodeLabel(
            ValidateLabel(
                label,
                $"sdk:sorafs.pdp.{PdpKindLabel(kind)}",
                nameof(label)),
            nameof(label));
        RequireAggregateBound("PDP validation", payload.Length, labelBytes.Length);
        RequireOrderbookPdpBridge(native);
        NativeValidationResult result;
        try
        {
            result = native.ValidatePdpPayload(
                (uint)kind,
                payload,
                labelBytes,
                checked((ulong)generatedAtUnix));
        }
        catch (Exception error)
        {
            throw new InvalidOperationException("SoraFS PDP native validation failed.", error);
        }
        return ReadAndValidateOutcome(
            "SoraFS PDP validation",
            result,
            checked((ulong)generatedAtUnix),
            native);
    }

    internal static string ValidatePdpCommitmentChallengeJson(
        byte[] commitment,
        byte[] challenge,
        string? commitmentLabel,
        string? challengeLabel,
        long generatedAtUnix,
        ISoraFsReferenceNativeBoundary native)
    {
        ArgumentNullException.ThrowIfNull(native);
        ValidateGeneratedAt(generatedAtUnix);
        var commitmentPayload = CopyInput(commitment, nameof(commitment));
        var challengePayload = CopyInput(challenge, nameof(challenge));
        var commitmentLabelBytes = EncodeLabel(
            ValidateLabel(
                commitmentLabel,
                "sdk:sorafs.pdp.commitment",
                nameof(commitmentLabel)),
            nameof(commitmentLabel));
        var challengeLabelBytes = EncodeLabel(
            ValidateLabel(
                challengeLabel,
                "sdk:sorafs.pdp.challenge",
                nameof(challengeLabel)),
            nameof(challengeLabel));
        RequireAggregateBound(
            "PDP commitment/challenge validation",
            commitmentPayload.Length,
            commitmentLabelBytes.Length,
            challengePayload.Length,
            challengeLabelBytes.Length);
        RequireOrderbookPdpBridge(native);
        NativeValidationResult result;
        try
        {
            result = native.ValidatePdpCommitmentChallenge(
                commitmentPayload,
                commitmentLabelBytes,
                challengePayload,
                challengeLabelBytes,
                checked((ulong)generatedAtUnix));
        }
        catch (Exception error)
        {
            throw new InvalidOperationException(
                "SoraFS PDP commitment/challenge native validation failed.",
                error);
        }
        return ReadAndValidateOutcome(
            "SoraFS PDP commitment/challenge validation",
            result,
            checked((ulong)generatedAtUnix),
            native);
    }

    internal static string ValidatePdpChallengeProofJson(
        byte[] challenge,
        byte[] proof,
        string? challengeLabel,
        string? proofLabel,
        long generatedAtUnix,
        ISoraFsReferenceNativeBoundary native)
    {
        ArgumentNullException.ThrowIfNull(native);
        ValidateGeneratedAt(generatedAtUnix);
        var challengePayload = CopyInput(challenge, nameof(challenge));
        var proofPayload = CopyInput(proof, nameof(proof));
        var challengeLabelBytes = EncodeLabel(
            ValidateLabel(
                challengeLabel,
                "sdk:sorafs.pdp.challenge",
                nameof(challengeLabel)),
            nameof(challengeLabel));
        var proofLabelBytes = EncodeLabel(
            ValidateLabel(proofLabel, "sdk:sorafs.pdp.proof", nameof(proofLabel)),
            nameof(proofLabel));
        RequireAggregateBound(
            "PDP challenge/proof validation",
            challengePayload.Length,
            challengeLabelBytes.Length,
            proofPayload.Length,
            proofLabelBytes.Length);
        RequireOrderbookPdpBridge(native);
        NativeValidationResult result;
        try
        {
            result = native.ValidatePdpChallengeProof(
                challengePayload,
                challengeLabelBytes,
                proofPayload,
                proofLabelBytes,
                checked((ulong)generatedAtUnix));
        }
        catch (Exception error)
        {
            throw new InvalidOperationException(
                "SoraFS PDP challenge/proof native validation failed.",
                error);
        }
        return ReadAndValidateOutcome(
            "SoraFS PDP challenge/proof validation",
            result,
            checked((ulong)generatedAtUnix),
            native);
    }

    internal static string ValidatePdpBundleJson(
        byte[] commitment,
        byte[] challenge,
        byte[] proof,
        string? commitmentLabel,
        string? challengeLabel,
        string? proofLabel,
        long generatedAtUnix,
        ISoraFsReferenceNativeBoundary native)
    {
        ArgumentNullException.ThrowIfNull(native);
        ValidateGeneratedAt(generatedAtUnix);
        var commitmentPayload = CopyInput(commitment, nameof(commitment));
        var challengePayload = CopyInput(challenge, nameof(challenge));
        var proofPayload = CopyInput(proof, nameof(proof));
        var commitmentLabelBytes = EncodeLabel(
            ValidateLabel(
                commitmentLabel,
                "sdk:sorafs.pdp.commitment",
                nameof(commitmentLabel)),
            nameof(commitmentLabel));
        var challengeLabelBytes = EncodeLabel(
            ValidateLabel(
                challengeLabel,
                "sdk:sorafs.pdp.challenge",
                nameof(challengeLabel)),
            nameof(challengeLabel));
        var proofLabelBytes = EncodeLabel(
            ValidateLabel(proofLabel, "sdk:sorafs.pdp.proof", nameof(proofLabel)),
            nameof(proofLabel));
        RequireAggregateBound(
            "PDP bundle validation",
            commitmentPayload.Length,
            commitmentLabelBytes.Length,
            challengePayload.Length,
            challengeLabelBytes.Length,
            proofPayload.Length,
            proofLabelBytes.Length);
        RequireOrderbookPdpBridge(native);
        NativeValidationResult result;
        try
        {
            result = native.ValidatePdpBundle(
                commitmentPayload,
                commitmentLabelBytes,
                challengePayload,
                challengeLabelBytes,
                proofPayload,
                proofLabelBytes,
                checked((ulong)generatedAtUnix));
        }
        catch (Exception error)
        {
            throw new InvalidOperationException(
                "SoraFS PDP bundle native validation failed.",
                error);
        }
        return ReadAndValidateOutcome(
            "SoraFS PDP bundle validation",
            result,
            checked((ulong)generatedAtUnix),
            native);
    }

    internal static string ValidateGovernanceDagBlockJson(
        byte[] noritoBytes,
        string? label,
        byte[]? expectedBlockCid,
        long generatedAtUnix,
        ISoraFsReferenceNativeBoundary native)
    {
        ArgumentNullException.ThrowIfNull(native);
        ValidateGeneratedAt(generatedAtUnix);
        var payload = CopyInput(noritoBytes, nameof(noritoBytes));
        var labelBytes = EncodeLabel(
            ValidateLabel(label, "governance-dag-block.to", nameof(label)),
            nameof(label));
        if (expectedBlockCid is not null
            && expectedBlockCid.Length != GovernanceDagCidBytesV1)
        {
            throw new ArgumentException(
                $"Expected Governance DAG block CID must contain exactly {GovernanceDagCidBytesV1} bytes.",
                nameof(expectedBlockCid));
        }
        var expectedCid = expectedBlockCid is null
            ? Array.Empty<byte>()
            : CopyInput(expectedBlockCid, nameof(expectedBlockCid));
        RequireAggregateBound(
            "Governance DAG block validation",
            payload.Length,
            labelBytes.Length,
            expectedCid.Length);
        RequireGovernanceBridge(native);

        NativeValidationResult result;
        try
        {
            result = native.ValidateGovernanceDagBlock(
                payload,
                labelBytes,
                expectedCid,
                checked((ulong)generatedAtUnix));
        }
        catch (Exception error)
        {
            throw new InvalidOperationException(
                "SoraFS Governance DAG block native validation failed.",
                error);
        }

        return ReadAndValidateOutcome(
            "SoraFS Governance DAG block validation",
            result,
            checked((ulong)generatedAtUnix),
            native);
    }

    internal static string ValidateGovernanceDagHeadChainJson(
        byte[] headNoritoBytes,
        IReadOnlyList<SoraFsGovernanceDagBlockInput> blocks,
        string? headLabel,
        long generatedAtUnix,
        ISoraFsReferenceNativeBoundary native)
    {
        ArgumentNullException.ThrowIfNull(blocks);
        ArgumentNullException.ThrowIfNull(native);
        ValidateGeneratedAt(generatedAtUnix);
        if (blocks.Count == 0 || blocks.Count > GovernanceDagMaxBlocksV1)
        {
            throw new ArgumentException(
                $"Governance DAG head chain must contain 1..{GovernanceDagMaxBlocksV1} blocks.",
                nameof(blocks));
        }

        var head = CopyInput(headNoritoBytes, nameof(headNoritoBytes));
        var headLabelBytes = EncodeLabel(
            ValidateLabel(headLabel, "governance-dag-head.to", nameof(headLabel)),
            nameof(headLabel));
        var nativeBlocks = new NativeGovernanceInput[blocks.Count];
        long aggregateBytes = checked((long)head.Length + headLabelBytes.Length);
        for (var index = 0; index < blocks.Count; index++)
        {
            var block = blocks[index] ?? throw new ArgumentException(
                $"Governance DAG block at index {index} must not be null.",
                nameof(blocks));
            var snapshot = block.Snapshot(index);
            aggregateBytes = checked(
                aggregateBytes + snapshot.Bytes.Length + snapshot.LabelBytes.Length);
            if (aggregateBytes > MaxInputBytesV1)
            {
                throw new ArgumentException(
                    $"Governance DAG head chain must not exceed {MaxInputBytesV1} aggregate bytes.",
                    nameof(blocks));
            }
            nativeBlocks[index] = snapshot;
        }
        if (aggregateBytes > MaxInputBytesV1)
        {
            throw new ArgumentException(
                $"Governance DAG head chain must not exceed {MaxInputBytesV1} aggregate bytes.",
                nameof(headNoritoBytes));
        }
        RequireGovernanceBridge(native);

        NativeValidationResult result;
        try
        {
            result = native.ValidateGovernanceDagHeadChain(
                head,
                headLabelBytes,
                nativeBlocks,
                checked((ulong)generatedAtUnix));
        }
        catch (Exception error)
        {
            throw new InvalidOperationException(
                "SoraFS Governance DAG head-chain native validation failed.",
                error);
        }

        return ReadAndValidateOutcome(
            "SoraFS Governance DAG head-chain validation",
            result,
            checked((ulong)generatedAtUnix),
            native);
    }

    internal static byte[] CopyInput(byte[] value, string parameterName)
    {
        ArgumentNullException.ThrowIfNull(value, parameterName);
        if (value.Length > MaxInputBytesV1)
        {
            throw new ArgumentException(
                $"SoraFS reference input must not exceed {MaxInputBytesV1} bytes.",
                parameterName);
        }
        return (byte[])value.Clone();
    }

    internal static string ValidateLabel(
        string? label,
        string fallback,
        string parameterName)
    {
        var value = label ?? fallback;
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new ArgumentException("SoraFS reference label must not be blank.", parameterName);
        }
        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "SoraFS reference label must not contain surrounding whitespace.",
                parameterName);
        }
        foreach (var character in value)
        {
            if (char.IsControl(character))
            {
                throw new ArgumentException(
                    "SoraFS reference label must not contain control characters.",
                    parameterName);
            }
        }
        _ = EncodeLabel(value, parameterName);
        return value;
    }

    internal static byte[] EncodeLabel(string label, string parameterName)
    {
        byte[] bytes;
        try
        {
            bytes = StrictUtf8.GetBytes(label);
        }
        catch (EncoderFallbackException error)
        {
            throw new ArgumentException(
                "SoraFS reference label must be valid UTF-8 text.",
                parameterName,
                error);
        }
        if (bytes.Length > MaxLabelBytesV1)
        {
            throw new ArgumentException(
                $"SoraFS reference label must not exceed {MaxLabelBytesV1} UTF-8 bytes.",
                parameterName);
        }
        return bytes;
    }

    private static long CurrentEpochSeconds()
    {
        var seconds = DateTimeOffset.UtcNow.ToUnixTimeSeconds();
        return seconds < 0 ? 0 : seconds;
    }

    private static void ValidateGeneratedAt(long generatedAtUnix)
    {
        if (generatedAtUnix < 0)
        {
            throw new ArgumentOutOfRangeException(
                nameof(generatedAtUnix),
                "generatedAtUnix must be non-negative.");
        }
    }

    private static string OrderbookKindLabel(SoraFsOrderbookPayloadKind kind)
    {
        return kind switch
        {
            SoraFsOrderbookPayloadKind.OrderRequest => "order-request",
            SoraFsOrderbookPayloadKind.OrderCancel => "order-cancel",
            SoraFsOrderbookPayloadKind.TradeEvent => "trade-event",
            SoraFsOrderbookPayloadKind.SettlementChannel => "settlement-channel",
            SoraFsOrderbookPayloadKind.SettlementReceipt => "settlement-receipt",
            SoraFsOrderbookPayloadKind.RuntimeSnapshot => "runtime-snapshot",
            _ => throw new ArgumentOutOfRangeException(nameof(kind)),
        };
    }

    private static string PdpKindLabel(SoraFsPdpPayloadKind kind)
    {
        return kind switch
        {
            SoraFsPdpPayloadKind.Commitment => "commitment",
            SoraFsPdpPayloadKind.Challenge => "challenge",
            SoraFsPdpPayloadKind.Proof => "proof",
            _ => throw new ArgumentOutOfRangeException(nameof(kind)),
        };
    }

    private static void RequireAggregateBound(string operation, params int[] lengths)
    {
        long aggregate = 0;
        foreach (var length in lengths)
        {
            aggregate = checked(aggregate + length);
        }
        if (aggregate > MaxInputBytesV1)
        {
            throw new ArgumentException(
                $"{operation} must not exceed {MaxInputBytesV1} aggregate bytes.");
        }
    }

    private static void RequireGovernanceBridge(ISoraFsReferenceNativeBoundary native)
    {
        uint version;
        try
        {
            version = native.AbiVersion();
        }
        catch (Exception error)
        {
            throw new InvalidOperationException(
                $"{LibraryName} is unavailable; install ABI {RequiredBridgeAbiVersion} or later.",
                error);
        }
        if (version < RequiredBridgeAbiVersion)
        {
            throw new InvalidOperationException(
                $"{LibraryName} ABI {RequiredBridgeAbiVersion} or later is required; found {version}.");
        }
        bool hasSymbols;
        try
        {
            hasSymbols = native.HasGovernanceDagSymbols();
        }
        catch (Exception error)
        {
            throw new InvalidOperationException(
                $"{LibraryName} does not expose the Governance DAG reference symbols.",
                error);
        }
        if (!hasSymbols)
        {
            throw new InvalidOperationException(
                $"{LibraryName} does not expose the Governance DAG reference symbols.");
        }
    }

    private static void RequireOrderbookPdpBridge(ISoraFsReferenceNativeBoundary native)
    {
        uint version;
        try
        {
            version = native.AbiVersion();
        }
        catch (Exception error)
        {
            throw new InvalidOperationException(
                $"{LibraryName} is unavailable; install ABI {RequiredBridgeAbiVersion} or later.",
                error);
        }
        if (version < RequiredBridgeAbiVersion)
        {
            throw new InvalidOperationException(
                $"{LibraryName} ABI {RequiredBridgeAbiVersion} or later is required; found {version}.");
        }
        bool hasSymbols;
        try
        {
            hasSymbols = native.HasOrderbookPdpSymbols();
        }
        catch (Exception error)
        {
            throw new InvalidOperationException(
                $"{LibraryName} does not expose the orderbook/PDP reference symbols.",
                error);
        }
        if (!hasSymbols)
        {
            throw new InvalidOperationException(
                $"{LibraryName} does not expose the orderbook/PDP reference symbols.");
        }
    }

    private static string ReadAndValidateOutcome(
        string operation,
        NativeValidationResult result,
        ulong expectedGeneratedAt,
        ISoraFsReferenceNativeBoundary native)
    {
        try
        {
            if (result.Code != 0)
            {
                throw new InvalidOperationException(
                    $"{operation} failed with bridge error code {result.Code}.");
            }
            if (result.Pointer == IntPtr.Zero)
            {
                throw new InvalidOperationException($"{operation} returned a null output pointer.");
            }
            var length = result.Length.ToUInt64();
            if (length == 0)
            {
                throw new InvalidOperationException($"{operation} returned empty outcome JSON.");
            }
            if (length > MaxInputBytesV1)
            {
                throw new InvalidOperationException($"{operation} returned oversized outcome JSON.");
            }

            var output = new byte[(int)length];
            Marshal.Copy(result.Pointer, output, 0, output.Length);
            string json;
            try
            {
                json = StrictUtf8.GetString(output);
            }
            catch (DecoderFallbackException error)
            {
                throw new InvalidOperationException(
                    $"{operation} returned invalid UTF-8 outcome JSON.",
                    error);
            }
            ValidateOutcomeJson(json, expectedGeneratedAt, operation);
            return json;
        }
        finally
        {
            if (result.Pointer != IntPtr.Zero)
            {
                native.Free(result.Pointer);
            }
        }
    }

    private static void ValidateOutcomeJson(
        string json,
        ulong expectedGeneratedAt,
        string operation)
    {
        JsonDocument document;
        try
        {
            document = JsonDocument.Parse(
                json,
                new JsonDocumentOptions
                {
                    AllowTrailingCommas = false,
                    CommentHandling = JsonCommentHandling.Disallow,
                    MaxDepth = 64,
                });
        }
        catch (JsonException error)
        {
            throw new InvalidOperationException(
                $"{operation} returned malformed ValidationOutcomeV1 JSON.",
                error);
        }

        using (document)
        {
            var root = document.RootElement;
            RequireExactObject(root, OutcomeFields, "ValidationOutcomeV1", operation);
            var status = RequireString(root, "status", operation);
            if (status is not ("Ok" or "Error"))
            {
                throw InvalidOutcome(operation, "status must be `Ok` or `Error`.");
            }
            var code = RequireString(root, "code", operation);
            if (!code.StartsWith("SFS-", StringComparison.Ordinal))
            {
                throw InvalidOutcome(operation, "code must use the `SFS-` namespace.");
            }
            var category = RequireString(root, "category", operation);
            if (!OutcomeCategories.Contains(category))
            {
                throw InvalidOutcome(operation, "category is not a V1 outcome category.");
            }
            _ = RequireNonEmptyString(root, "message", operation);
            ValidateOptionalOutcomeString(root, "action", status == "Error", operation);
            var docsUrl = RequireString(root, "docs_url", operation);
            if (!string.Equals(docsUrl, ErrorsDocument, StringComparison.Ordinal))
            {
                throw InvalidOutcome(operation, "docs_url does not match the V1 error catalogue.");
            }
            ValidateStringArray(root, "telemetry_tags", operation);
            ValidateObjectArray(root, "context", ContextFields, operation);
            ValidateObjectArray(root, "inputs", InputFields, operation, requireNonEmpty: true);

            if (!root.GetProperty("version").TryGetInt32(out var version)
                || version != ValidationOutcomeVersionV1)
            {
                throw InvalidOutcome(operation, "version must be 1.");
            }
            if (!root.GetProperty("generated_at").TryGetUInt64(out var generatedAt)
                || generatedAt != expectedGeneratedAt)
            {
                throw InvalidOutcome(
                    operation,
                    "generated_at does not match the caller-bound timestamp.");
            }
        }
    }

    private static void RequireExactObject(
        JsonElement element,
        HashSet<string> expectedFields,
        string context,
        string operation)
    {
        if (element.ValueKind != JsonValueKind.Object)
        {
            throw InvalidOutcome(operation, $"{context} must be an object.");
        }
        var seen = new HashSet<string>(StringComparer.Ordinal);
        foreach (var property in element.EnumerateObject())
        {
            if (!seen.Add(property.Name))
            {
                throw InvalidOutcome(
                    operation,
                    $"{context}.{property.Name} must not appear more than once.");
            }
            if (!expectedFields.Contains(property.Name))
            {
                throw InvalidOutcome(
                    operation,
                    $"{context}.{property.Name} is not part of the V1 schema.");
            }
        }
        if (!seen.SetEquals(expectedFields))
        {
            throw InvalidOutcome(operation, $"{context} is missing a required V1 field.");
        }
    }

    private static string RequireString(
        JsonElement element,
        string propertyName,
        string operation)
    {
        var value = element.GetProperty(propertyName);
        if (value.ValueKind != JsonValueKind.String)
        {
            throw InvalidOutcome(operation, $"{propertyName} must be a string.");
        }
        return value.GetString()!;
    }

    private static string RequireNonEmptyString(
        JsonElement element,
        string propertyName,
        string operation)
    {
        var value = RequireString(element, propertyName, operation);
        if (string.IsNullOrEmpty(value))
        {
            throw InvalidOutcome(operation, $"{propertyName} must not be empty.");
        }
        return value;
    }

    private static void ValidateOptionalOutcomeString(
        JsonElement element,
        string propertyName,
        bool required,
        string operation)
    {
        var value = element.GetProperty(propertyName);
        if (!required && value.ValueKind == JsonValueKind.Null)
        {
            return;
        }
        if (value.ValueKind != JsonValueKind.String
            || string.IsNullOrEmpty(value.GetString()))
        {
            throw InvalidOutcome(
                operation,
                required
                    ? $"{propertyName} must be a non-empty string for an Error outcome."
                    : $"{propertyName} must be null for an Ok outcome.");
        }
        if (!required)
        {
            throw InvalidOutcome(operation, $"{propertyName} must be null for an Ok outcome.");
        }
    }

    private static void ValidateStringArray(
        JsonElement element,
        string propertyName,
        string operation)
    {
        var value = element.GetProperty(propertyName);
        if (value.ValueKind != JsonValueKind.Array)
        {
            throw InvalidOutcome(operation, $"{propertyName} must be an array.");
        }
        foreach (var item in value.EnumerateArray())
        {
            if (item.ValueKind != JsonValueKind.String || string.IsNullOrEmpty(item.GetString()))
            {
                throw InvalidOutcome(
                    operation,
                    $"{propertyName} entries must be non-empty strings.");
            }
        }
    }

    private static void ValidateObjectArray(
        JsonElement element,
        string propertyName,
        HashSet<string> fields,
        string operation,
        bool requireNonEmpty = false)
    {
        var value = element.GetProperty(propertyName);
        if (value.ValueKind != JsonValueKind.Array)
        {
            throw InvalidOutcome(operation, $"{propertyName} must be an array.");
        }
        var count = 0;
        foreach (var item in value.EnumerateArray())
        {
            RequireExactObject(item, fields, propertyName, operation);
            foreach (var field in fields)
            {
                _ = RequireString(item, field, operation);
            }
            count++;
        }
        if (requireNonEmpty && count == 0)
        {
            throw InvalidOutcome(operation, $"{propertyName} must not be empty.");
        }
    }

    private static InvalidOperationException InvalidOutcome(
        string operation,
        string diagnostic)
    {
        return new InvalidOperationException(
            $"{operation} returned invalid ValidationOutcomeV1 JSON: {diagnostic}");
    }

    [StructLayout(LayoutKind.Sequential)]
    private readonly struct NativeInputDescriptor
    {
        internal NativeInputDescriptor(
            IntPtr bytesPointer,
            UIntPtr bytesLength,
            IntPtr labelPointer,
            UIntPtr labelLength)
        {
            BytesPointer = bytesPointer;
            BytesLength = bytesLength;
            LabelPointer = labelPointer;
            LabelLength = labelLength;
        }

        internal readonly IntPtr BytesPointer;
        internal readonly UIntPtr BytesLength;
        internal readonly IntPtr LabelPointer;
        internal readonly UIntPtr LabelLength;
    }

    private sealed class PInvokeSoraFsReferenceNativeBoundary
        : ISoraFsReferenceNativeBoundary
    {
        internal static readonly PInvokeSoraFsReferenceNativeBoundary Instance = new();

        private PInvokeSoraFsReferenceNativeBoundary()
        {
        }

        public uint AbiVersion()
        {
            return NativeAbiVersion();
        }

        public bool HasGovernanceDagSymbols()
        {
            if (!NativeLibrary.TryLoad(
                    LibraryName,
                    typeof(SoraFsReferenceValidators).Assembly,
                    null,
                    out var handle))
            {
                return false;
            }
            try
            {
                return NativeLibrary.TryGetExport(
                        handle,
                        "connect_norito_sorafs_reference_validate_governance_dag_block_json",
                        out _)
                    && NativeLibrary.TryGetExport(
                        handle,
                        "connect_norito_sorafs_reference_validate_governance_dag_head_chain_json",
                        out _)
                    && NativeLibrary.TryGetExport(handle, "connect_norito_free", out _);
            }
            finally
            {
                NativeLibrary.Free(handle);
            }
        }

        public bool HasOrderbookPdpSymbols()
        {
            if (!NativeLibrary.TryLoad(
                    LibraryName,
                    typeof(SoraFsReferenceValidators).Assembly,
                    null,
                    out var handle))
            {
                return false;
            }
            try
            {
                return NativeLibrary.TryGetExport(
                        handle,
                        "connect_norito_sorafs_reference_validate_orderbook_json",
                        out _)
                    && NativeLibrary.TryGetExport(
                        handle,
                        "connect_norito_sorafs_reference_validate_pdp_payload_json",
                        out _)
                    && NativeLibrary.TryGetExport(
                        handle,
                        "connect_norito_sorafs_reference_validate_pdp_commitment_challenge_json",
                        out _)
                    && NativeLibrary.TryGetExport(
                        handle,
                        "connect_norito_sorafs_reference_validate_pdp_challenge_proof_json",
                        out _)
                    && NativeLibrary.TryGetExport(
                        handle,
                        "connect_norito_sorafs_reference_validate_pdp_bundle_json",
                        out _)
                    && NativeLibrary.TryGetExport(handle, "connect_norito_free", out _);
            }
            finally
            {
                NativeLibrary.Free(handle);
            }
        }

        public NativeValidationResult ValidateOrderbookPayload(
            uint kind,
            byte[] bytes,
            byte[] label,
            ulong generatedAt)
        {
            var code = NativeValidateOrderbookPayload(
                kind,
                bytes,
                (UIntPtr)bytes.Length,
                label,
                (UIntPtr)label.Length,
                generatedAt,
                out var output,
                out var outputLength);
            return new NativeValidationResult(code, output, outputLength);
        }

        public NativeValidationResult ValidatePdpPayload(
            uint kind,
            byte[] bytes,
            byte[] label,
            ulong generatedAt)
        {
            var code = NativeValidatePdpPayload(
                kind,
                bytes,
                (UIntPtr)bytes.Length,
                label,
                (UIntPtr)label.Length,
                generatedAt,
                out var output,
                out var outputLength);
            return new NativeValidationResult(code, output, outputLength);
        }

        public NativeValidationResult ValidatePdpCommitmentChallenge(
            byte[] commitment,
            byte[] commitmentLabel,
            byte[] challenge,
            byte[] challengeLabel,
            ulong generatedAt)
        {
            var code = NativeValidatePdpCommitmentChallenge(
                commitment,
                (UIntPtr)commitment.Length,
                commitmentLabel,
                (UIntPtr)commitmentLabel.Length,
                challenge,
                (UIntPtr)challenge.Length,
                challengeLabel,
                (UIntPtr)challengeLabel.Length,
                generatedAt,
                out var output,
                out var outputLength);
            return new NativeValidationResult(code, output, outputLength);
        }

        public NativeValidationResult ValidatePdpChallengeProof(
            byte[] challenge,
            byte[] challengeLabel,
            byte[] proof,
            byte[] proofLabel,
            ulong generatedAt)
        {
            var code = NativeValidatePdpChallengeProof(
                challenge,
                (UIntPtr)challenge.Length,
                challengeLabel,
                (UIntPtr)challengeLabel.Length,
                proof,
                (UIntPtr)proof.Length,
                proofLabel,
                (UIntPtr)proofLabel.Length,
                generatedAt,
                out var output,
                out var outputLength);
            return new NativeValidationResult(code, output, outputLength);
        }

        public NativeValidationResult ValidatePdpBundle(
            byte[] commitment,
            byte[] commitmentLabel,
            byte[] challenge,
            byte[] challengeLabel,
            byte[] proof,
            byte[] proofLabel,
            ulong generatedAt)
        {
            var code = NativeValidatePdpBundle(
                commitment,
                (UIntPtr)commitment.Length,
                commitmentLabel,
                (UIntPtr)commitmentLabel.Length,
                challenge,
                (UIntPtr)challenge.Length,
                challengeLabel,
                (UIntPtr)challengeLabel.Length,
                proof,
                (UIntPtr)proof.Length,
                proofLabel,
                (UIntPtr)proofLabel.Length,
                generatedAt,
                out var output,
                out var outputLength);
            return new NativeValidationResult(code, output, outputLength);
        }

        public NativeValidationResult ValidateGovernanceDagBlock(
            byte[] bytes,
            byte[] label,
            byte[] expectedBlockCid,
            ulong generatedAt)
        {
            var code = NativeValidateGovernanceDagBlock(
                bytes,
                (UIntPtr)bytes.Length,
                label,
                (UIntPtr)label.Length,
                expectedBlockCid,
                (UIntPtr)expectedBlockCid.Length,
                generatedAt,
                out var output,
                out var outputLength);
            return new NativeValidationResult(code, output, outputLength);
        }

        public NativeValidationResult ValidateGovernanceDagHeadChain(
            byte[] head,
            byte[] headLabel,
            NativeGovernanceInput[] blocks,
            ulong generatedAt)
        {
            var handles = new List<GCHandle>(checked(blocks.Length * 2));
            try
            {
                var descriptors = new NativeInputDescriptor[blocks.Length];
                for (var index = 0; index < blocks.Length; index++)
                {
                    descriptors[index] = new NativeInputDescriptor(
                        Pin(blocks[index].Bytes, handles),
                        (UIntPtr)blocks[index].Bytes.Length,
                        Pin(blocks[index].LabelBytes, handles),
                        (UIntPtr)blocks[index].LabelBytes.Length);
                }
                var code = NativeValidateGovernanceDagHeadChain(
                    head,
                    (UIntPtr)head.Length,
                    headLabel,
                    (UIntPtr)headLabel.Length,
                    descriptors.Length == 0 ? null : descriptors,
                    (UIntPtr)descriptors.Length,
                    generatedAt,
                    out var output,
                    out var outputLength);
                return new NativeValidationResult(code, output, outputLength);
            }
            finally
            {
                foreach (var handle in handles)
                {
                    handle.Free();
                }
            }
        }

        public void Free(IntPtr pointer)
        {
            NativeFree(pointer);
        }

        private static IntPtr Pin(byte[] bytes, ICollection<GCHandle> handles)
        {
            if (bytes.Length == 0)
            {
                return IntPtr.Zero;
            }
            var handle = GCHandle.Alloc(bytes, GCHandleType.Pinned);
            handles.Add(handle);
            return handle.AddrOfPinnedObject();
        }

        // The bridge ABI uses C `unsigned long`: 32 bits on Windows and
        // pointer-sized on the mandatory Unix targets.
        private static int NativeValidateOrderbookPayload(
            uint kind,
            byte[] bytes,
            UIntPtr bytesLength,
            byte[] label,
            UIntPtr labelLength,
            ulong generatedAt,
            out IntPtr output,
            out UIntPtr outputLength)
        {
            if (!OperatingSystem.IsWindows())
            {
                return NativeValidateOrderbookPayloadUnix(
                    kind,
                    bytes,
                    bytesLength,
                    label,
                    labelLength,
                    generatedAt,
                    out output,
                    out outputLength);
            }
            var code = NativeValidateOrderbookPayloadWindows(
                kind,
                bytes,
                checked((uint)bytesLength.ToUInt64()),
                label,
                checked((uint)labelLength.ToUInt64()),
                generatedAt,
                out output,
                out var windowsLength);
            outputLength = (UIntPtr)windowsLength;
            return code;
        }

        private static int NativeValidatePdpPayload(
            uint kind,
            byte[] bytes,
            UIntPtr bytesLength,
            byte[] label,
            UIntPtr labelLength,
            ulong generatedAt,
            out IntPtr output,
            out UIntPtr outputLength)
        {
            if (!OperatingSystem.IsWindows())
            {
                return NativeValidatePdpPayloadUnix(
                    kind,
                    bytes,
                    bytesLength,
                    label,
                    labelLength,
                    generatedAt,
                    out output,
                    out outputLength);
            }
            var code = NativeValidatePdpPayloadWindows(
                kind,
                bytes,
                checked((uint)bytesLength.ToUInt64()),
                label,
                checked((uint)labelLength.ToUInt64()),
                generatedAt,
                out output,
                out var windowsLength);
            outputLength = (UIntPtr)windowsLength;
            return code;
        }

        private static int NativeValidatePdpCommitmentChallenge(
            byte[] commitment,
            UIntPtr commitmentLength,
            byte[] commitmentLabel,
            UIntPtr commitmentLabelLength,
            byte[] challenge,
            UIntPtr challengeLength,
            byte[] challengeLabel,
            UIntPtr challengeLabelLength,
            ulong generatedAt,
            out IntPtr output,
            out UIntPtr outputLength)
        {
            if (!OperatingSystem.IsWindows())
            {
                return NativeValidatePdpCommitmentChallengeUnix(
                    commitment,
                    commitmentLength,
                    commitmentLabel,
                    commitmentLabelLength,
                    challenge,
                    challengeLength,
                    challengeLabel,
                    challengeLabelLength,
                    generatedAt,
                    out output,
                    out outputLength);
            }
            var code = NativeValidatePdpCommitmentChallengeWindows(
                commitment,
                checked((uint)commitmentLength.ToUInt64()),
                commitmentLabel,
                checked((uint)commitmentLabelLength.ToUInt64()),
                challenge,
                checked((uint)challengeLength.ToUInt64()),
                challengeLabel,
                checked((uint)challengeLabelLength.ToUInt64()),
                generatedAt,
                out output,
                out var windowsLength);
            outputLength = (UIntPtr)windowsLength;
            return code;
        }

        private static int NativeValidatePdpChallengeProof(
            byte[] challenge,
            UIntPtr challengeLength,
            byte[] challengeLabel,
            UIntPtr challengeLabelLength,
            byte[] proof,
            UIntPtr proofLength,
            byte[] proofLabel,
            UIntPtr proofLabelLength,
            ulong generatedAt,
            out IntPtr output,
            out UIntPtr outputLength)
        {
            if (!OperatingSystem.IsWindows())
            {
                return NativeValidatePdpChallengeProofUnix(
                    challenge,
                    challengeLength,
                    challengeLabel,
                    challengeLabelLength,
                    proof,
                    proofLength,
                    proofLabel,
                    proofLabelLength,
                    generatedAt,
                    out output,
                    out outputLength);
            }
            var code = NativeValidatePdpChallengeProofWindows(
                challenge,
                checked((uint)challengeLength.ToUInt64()),
                challengeLabel,
                checked((uint)challengeLabelLength.ToUInt64()),
                proof,
                checked((uint)proofLength.ToUInt64()),
                proofLabel,
                checked((uint)proofLabelLength.ToUInt64()),
                generatedAt,
                out output,
                out var windowsLength);
            outputLength = (UIntPtr)windowsLength;
            return code;
        }

        private static int NativeValidatePdpBundle(
            byte[] commitment,
            UIntPtr commitmentLength,
            byte[] commitmentLabel,
            UIntPtr commitmentLabelLength,
            byte[] challenge,
            UIntPtr challengeLength,
            byte[] challengeLabel,
            UIntPtr challengeLabelLength,
            byte[] proof,
            UIntPtr proofLength,
            byte[] proofLabel,
            UIntPtr proofLabelLength,
            ulong generatedAt,
            out IntPtr output,
            out UIntPtr outputLength)
        {
            if (!OperatingSystem.IsWindows())
            {
                return NativeValidatePdpBundleUnix(
                    commitment,
                    commitmentLength,
                    commitmentLabel,
                    commitmentLabelLength,
                    challenge,
                    challengeLength,
                    challengeLabel,
                    challengeLabelLength,
                    proof,
                    proofLength,
                    proofLabel,
                    proofLabelLength,
                    generatedAt,
                    out output,
                    out outputLength);
            }
            var code = NativeValidatePdpBundleWindows(
                commitment,
                checked((uint)commitmentLength.ToUInt64()),
                commitmentLabel,
                checked((uint)commitmentLabelLength.ToUInt64()),
                challenge,
                checked((uint)challengeLength.ToUInt64()),
                challengeLabel,
                checked((uint)challengeLabelLength.ToUInt64()),
                proof,
                checked((uint)proofLength.ToUInt64()),
                proofLabel,
                checked((uint)proofLabelLength.ToUInt64()),
                generatedAt,
                out output,
                out var windowsLength);
            outputLength = (UIntPtr)windowsLength;
            return code;
        }

        [DllImport(
            LibraryName,
            EntryPoint = "connect_norito_bridge_abi_version",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern uint NativeAbiVersion();

        [DllImport(
            LibraryName,
            EntryPoint = "connect_norito_sorafs_reference_validate_orderbook_json",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern int NativeValidateOrderbookPayloadUnix(
            uint kind,
            [In] byte[] bytes,
            UIntPtr bytesLength,
            [In] byte[] label,
            UIntPtr labelLength,
            ulong generatedAt,
            out IntPtr output,
            out UIntPtr outputLength);

        [DllImport(
            LibraryName,
            EntryPoint = "connect_norito_sorafs_reference_validate_orderbook_json",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern int NativeValidateOrderbookPayloadWindows(
            uint kind,
            [In] byte[] bytes,
            uint bytesLength,
            [In] byte[] label,
            uint labelLength,
            ulong generatedAt,
            out IntPtr output,
            out uint outputLength);

        [DllImport(
            LibraryName,
            EntryPoint = "connect_norito_sorafs_reference_validate_pdp_payload_json",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern int NativeValidatePdpPayloadUnix(
            uint kind,
            [In] byte[] bytes,
            UIntPtr bytesLength,
            [In] byte[] label,
            UIntPtr labelLength,
            ulong generatedAt,
            out IntPtr output,
            out UIntPtr outputLength);

        [DllImport(
            LibraryName,
            EntryPoint = "connect_norito_sorafs_reference_validate_pdp_payload_json",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern int NativeValidatePdpPayloadWindows(
            uint kind,
            [In] byte[] bytes,
            uint bytesLength,
            [In] byte[] label,
            uint labelLength,
            ulong generatedAt,
            out IntPtr output,
            out uint outputLength);

        [DllImport(
            LibraryName,
            EntryPoint =
                "connect_norito_sorafs_reference_validate_pdp_commitment_challenge_json",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern int NativeValidatePdpCommitmentChallengeUnix(
            [In] byte[] commitment,
            UIntPtr commitmentLength,
            [In] byte[] commitmentLabel,
            UIntPtr commitmentLabelLength,
            [In] byte[] challenge,
            UIntPtr challengeLength,
            [In] byte[] challengeLabel,
            UIntPtr challengeLabelLength,
            ulong generatedAt,
            out IntPtr output,
            out UIntPtr outputLength);

        [DllImport(
            LibraryName,
            EntryPoint =
                "connect_norito_sorafs_reference_validate_pdp_commitment_challenge_json",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern int NativeValidatePdpCommitmentChallengeWindows(
            [In] byte[] commitment,
            uint commitmentLength,
            [In] byte[] commitmentLabel,
            uint commitmentLabelLength,
            [In] byte[] challenge,
            uint challengeLength,
            [In] byte[] challengeLabel,
            uint challengeLabelLength,
            ulong generatedAt,
            out IntPtr output,
            out uint outputLength);

        [DllImport(
            LibraryName,
            EntryPoint =
                "connect_norito_sorafs_reference_validate_pdp_challenge_proof_json",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern int NativeValidatePdpChallengeProofUnix(
            [In] byte[] challenge,
            UIntPtr challengeLength,
            [In] byte[] challengeLabel,
            UIntPtr challengeLabelLength,
            [In] byte[] proof,
            UIntPtr proofLength,
            [In] byte[] proofLabel,
            UIntPtr proofLabelLength,
            ulong generatedAt,
            out IntPtr output,
            out UIntPtr outputLength);

        [DllImport(
            LibraryName,
            EntryPoint =
                "connect_norito_sorafs_reference_validate_pdp_challenge_proof_json",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern int NativeValidatePdpChallengeProofWindows(
            [In] byte[] challenge,
            uint challengeLength,
            [In] byte[] challengeLabel,
            uint challengeLabelLength,
            [In] byte[] proof,
            uint proofLength,
            [In] byte[] proofLabel,
            uint proofLabelLength,
            ulong generatedAt,
            out IntPtr output,
            out uint outputLength);

        [DllImport(
            LibraryName,
            EntryPoint = "connect_norito_sorafs_reference_validate_pdp_bundle_json",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern int NativeValidatePdpBundleUnix(
            [In] byte[] commitment,
            UIntPtr commitmentLength,
            [In] byte[] commitmentLabel,
            UIntPtr commitmentLabelLength,
            [In] byte[] challenge,
            UIntPtr challengeLength,
            [In] byte[] challengeLabel,
            UIntPtr challengeLabelLength,
            [In] byte[] proof,
            UIntPtr proofLength,
            [In] byte[] proofLabel,
            UIntPtr proofLabelLength,
            ulong generatedAt,
            out IntPtr output,
            out UIntPtr outputLength);

        [DllImport(
            LibraryName,
            EntryPoint = "connect_norito_sorafs_reference_validate_pdp_bundle_json",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern int NativeValidatePdpBundleWindows(
            [In] byte[] commitment,
            uint commitmentLength,
            [In] byte[] commitmentLabel,
            uint commitmentLabelLength,
            [In] byte[] challenge,
            uint challengeLength,
            [In] byte[] challengeLabel,
            uint challengeLabelLength,
            [In] byte[] proof,
            uint proofLength,
            [In] byte[] proofLabel,
            uint proofLabelLength,
            ulong generatedAt,
            out IntPtr output,
            out uint outputLength);

        [DllImport(
            LibraryName,
            EntryPoint =
                "connect_norito_sorafs_reference_validate_governance_dag_block_json",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern int NativeValidateGovernanceDagBlock(
            [In] byte[] bytesPointer,
            UIntPtr bytesLength,
            [In] byte[] labelPointer,
            UIntPtr labelLength,
            [In] byte[] expectedBlockCidPointer,
            UIntPtr expectedBlockCidLength,
            ulong generatedAt,
            out IntPtr outputPointer,
            out UIntPtr outputLength);

        [DllImport(
            LibraryName,
            EntryPoint =
                "connect_norito_sorafs_reference_validate_governance_dag_head_chain_json",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern int NativeValidateGovernanceDagHeadChain(
            [In] byte[] headPointer,
            UIntPtr headLength,
            [In] byte[] headLabelPointer,
            UIntPtr headLabelLength,
            [In] NativeInputDescriptor[]? blocksPointer,
            UIntPtr blocksLength,
            ulong generatedAt,
            out IntPtr outputPointer,
            out UIntPtr outputLength);

        [DllImport(
            LibraryName,
            EntryPoint = "connect_norito_free",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern void NativeFree(IntPtr pointer);
    }
}

internal sealed class NativeGovernanceInput
{
    internal NativeGovernanceInput(byte[] bytes, byte[] labelBytes)
    {
        Bytes = bytes;
        LabelBytes = labelBytes;
    }

    internal byte[] Bytes { get; }

    internal byte[] LabelBytes { get; }
}

internal readonly record struct NativeValidationResult(
    int Code,
    IntPtr Pointer,
    UIntPtr Length);

internal interface ISoraFsReferenceNativeBoundary
{
    uint AbiVersion();

    bool HasGovernanceDagSymbols();

    bool HasOrderbookPdpSymbols();

    NativeValidationResult ValidateOrderbookPayload(
        uint kind,
        byte[] bytes,
        byte[] label,
        ulong generatedAt);

    NativeValidationResult ValidatePdpPayload(
        uint kind,
        byte[] bytes,
        byte[] label,
        ulong generatedAt);

    NativeValidationResult ValidatePdpCommitmentChallenge(
        byte[] commitment,
        byte[] commitmentLabel,
        byte[] challenge,
        byte[] challengeLabel,
        ulong generatedAt);

    NativeValidationResult ValidatePdpChallengeProof(
        byte[] challenge,
        byte[] challengeLabel,
        byte[] proof,
        byte[] proofLabel,
        ulong generatedAt);

    NativeValidationResult ValidatePdpBundle(
        byte[] commitment,
        byte[] commitmentLabel,
        byte[] challenge,
        byte[] challengeLabel,
        byte[] proof,
        byte[] proofLabel,
        ulong generatedAt);

    NativeValidationResult ValidateGovernanceDagBlock(
        byte[] bytes,
        byte[] label,
        byte[] expectedBlockCid,
        ulong generatedAt);

    NativeValidationResult ValidateGovernanceDagHeadChain(
        byte[] head,
        byte[] headLabel,
        NativeGovernanceInput[] blocks,
        ulong generatedAt);

    void Free(IntPtr pointer);
}
