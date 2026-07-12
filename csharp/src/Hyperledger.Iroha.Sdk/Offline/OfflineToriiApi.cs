using System.Buffers.Binary;
using System.Text;
using System.Text.Json.Serialization;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Offline;

/// <summary>Canonical first-release Torii Offline routes.</summary>
public static class OfflineApiRoutes
{
    public const string Readiness = "/v1/offline/readiness";
    public const string TopUp = "/v1/offline/top-up";
    public const string Redeem = "/v1/offline/redeem";
    public const string Operations = "/v1/offline/operations";

    public static string Operation(string operationId) =>
        $"{Operations}/{OfflineApiValidation.RequireOperationId(operationId, nameof(operationId))}";
}

/// <summary>Kind of asynchronous Offline command accepted by Torii.</summary>
public enum OfflineOperationKind
{
    TopUp,
    Redeem,
}

/// <summary>Initial state returned after Torii accepts an Offline command.</summary>
public enum OfflineOperationState
{
    Pending,
}

/// <summary>A schema-bound top-up command submitted directly as Norito.</summary>
public sealed class OfflineTopUpRequest
{
    private readonly byte[] archive;

    public OfflineTopUpRequest(byte[] noritoArchive)
    {
        var canonical = OfflineOperationCodec.RequireCanonicalRequest(
            noritoArchive,
            OfflineOperationCodec.TopUpRequestSchema,
            operationIdFieldIndex: 6,
            fieldCount: 8);
        archive = canonical.Archive;
        OperationId = canonical.OperationId;
    }

    /// <summary>Lowercase hexadecimal operation identifier embedded in the signed request.</summary>
    public string OperationId { get; }

    /// <summary>Return a defensive copy of the canonical request archive.</summary>
    public byte[] NoritoArchive() => (byte[])archive.Clone();
}

/// <summary>A schema-bound redemption command submitted directly as Norito.</summary>
public sealed class OfflineRedeemRequest
{
    private readonly byte[] archive;

    public OfflineRedeemRequest(byte[] noritoArchive)
    {
        var canonical = OfflineOperationCodec.RequireCanonicalRequest(
            noritoArchive,
            OfflineOperationCodec.RedeemRequestSchema,
            operationIdFieldIndex: 9,
            fieldCount: 11);
        archive = canonical.Archive;
        OperationId = canonical.OperationId;
    }

    /// <summary>Lowercase hexadecimal operation identifier embedded in the signed request.</summary>
    public string OperationId { get; }

    /// <summary>Return a defensive copy of the canonical request archive.</summary>
    public byte[] NoritoArchive() => (byte[])archive.Clone();
}

/// <summary>One machine-readable reason an asset is not ready for offline payments.</summary>
[JsonConverter(typeof(OfflineReadinessBlockerJsonConverter))]
public sealed record OfflineReadinessBlocker
{
    public OfflineReadinessBlocker(string code, string message)
    {
        Code = OfflineApiValidation.RequireCode(code, nameof(code));
        Message = OfflineApiValidation.RequireBoundedText(message, 1024, nameof(message));
    }

    public string Code { get; }

    public string Message { get; }
}

/// <summary>Stable registry identity of a verifier selected for an Offline proof role.</summary>
[JsonConverter(typeof(OfflineVerifierIdJsonConverter))]
public sealed record OfflineVerifierId
{
    public OfflineVerifierId(string backend, string name)
    {
        Backend = OfflineApiValidation.RequireBoundedText(backend, 256, nameof(backend));
        Name = OfflineApiValidation.RequireBoundedText(name, 256, nameof(name));
    }

    public string Backend { get; }

    public string Name { get; }
}

/// <summary>
/// Key-material-free verifier registry projection used by the distinct transfer and top-up shield roles.
/// </summary>
[JsonConverter(typeof(OfflineActiveTransferVerifierJsonConverter))]
public sealed record OfflineActiveTransferVerifier
{
    public OfflineActiveTransferVerifier(
        OfflineVerifierId id,
        uint version,
        string circuitId,
        string commitment,
        string publicInputsSchemaHash,
        uint maxProofBytes,
        ulong activationHeight,
        ulong? withdrawalHeight)
    {
        Id = id ?? throw new ArgumentNullException(nameof(id));
        Version = version;
        CircuitId = OfflineApiValidation.RequireExactText(circuitId, nameof(circuitId));
        Commitment = OfflineApiValidation.RequireLowercaseHash(
            commitment,
            nameof(commitment),
            "Verifier commitment");
        PublicInputsSchemaHash = OfflineApiValidation.RequireLowercaseHash(
            publicInputsSchemaHash,
            nameof(publicInputsSchemaHash),
            "Public-input schema hash");
        if (maxProofBytes == 0)
        {
            throw new ArgumentOutOfRangeException(
                nameof(maxProofBytes),
                maxProofBytes,
                "Maximum proof bytes must be at least 1.");
        }
        if (withdrawalHeight is 0 || withdrawalHeight <= activationHeight)
        {
            throw new ArgumentOutOfRangeException(
                nameof(withdrawalHeight),
                withdrawalHeight,
                "Withdrawal height must be null or greater than the activation height.");
        }
        MaxProofBytes = maxProofBytes;
        ActivationHeight = activationHeight;
        WithdrawalHeight = withdrawalHeight;
    }

    public OfflineVerifierId Id { get; }

    public uint Version { get; }

    public string CircuitId { get; }

    public string Commitment { get; }

    public string PublicInputsSchemaHash { get; }

    public uint MaxProofBytes { get; }

    public ulong ActivationHeight { get; }

    public ulong? WithdrawalHeight { get; }
}

/// <summary>Snapshot-bound readiness result for an asset definition.</summary>
[JsonConverter(typeof(OfflineReadinessJsonConverter))]
public sealed record OfflineReadiness
{
    private readonly OfflineReadinessBlocker[] blockers;

    public OfflineReadiness(
        string assetDefinitionId,
        uint? assetScale,
        ulong evaluatedBlockHeight,
        string evaluatedBlockHash,
        OfflineActiveTransferVerifier? activeTransferVerifier,
        OfflineActiveTransferVerifier? activeTopUpShieldVerifier,
        bool ready,
        IReadOnlyList<OfflineReadinessBlocker> blockers)
    {
        ArgumentNullException.ThrowIfNull(blockers);
        AssetDefinitionId = OfflineNoteCanonicalPayloadCodec.RequireCanonicalAssetDefinitionId(
            OfflineApiValidation.RequireExactToken(assetDefinitionId, nameof(assetDefinitionId)));
        EvaluatedBlockHeight = evaluatedBlockHeight;
        EvaluatedBlockHash = OfflineApiValidation.RequireTransactionHash(
            evaluatedBlockHash,
            nameof(evaluatedBlockHash));
        ActiveTransferVerifier = activeTransferVerifier;
        ActiveTopUpShieldVerifier = activeTopUpShieldVerifier;
        Ready = ready;
        this.blockers = new OfflineReadinessBlocker[blockers.Count];
        var blockerCodes = new HashSet<string>(StringComparer.Ordinal);
        for (var index = 0; index < blockers.Count; index++)
        {
            this.blockers[index] = blockers[index]
                ?? throw new ArgumentException("Readiness blockers must not contain null items.", nameof(blockers));
            if (!blockerCodes.Add(this.blockers[index].Code))
            {
                throw new ArgumentException(
                    "Readiness blockers must not repeat blocker codes.",
                    nameof(blockers));
            }
        }

        if (ready && this.blockers.Length != 0)
        {
            throw new ArgumentException("A ready asset must not report blockers.", nameof(blockers));
        }
        if (!ready && this.blockers.Length == 0)
        {
            throw new ArgumentException("A non-ready asset must report at least one blocker.", nameof(blockers));
        }
        if (activeTransferVerifier is not null
            && (activeTransferVerifier.ActivationHeight > evaluatedBlockHeight
                || (activeTransferVerifier.WithdrawalHeight.HasValue
                    && activeTransferVerifier.WithdrawalHeight.Value <= evaluatedBlockHeight)))
        {
            throw new ArgumentException(
                "The transfer verifier must be active at the evaluated block height.",
                nameof(activeTransferVerifier));
        }
        if (activeTopUpShieldVerifier is not null
            && (activeTopUpShieldVerifier.ActivationHeight > evaluatedBlockHeight
                || (activeTopUpShieldVerifier.WithdrawalHeight.HasValue
                    && activeTopUpShieldVerifier.WithdrawalHeight.Value <= evaluatedBlockHeight)))
        {
            throw new ArgumentException(
                "The top-up shield verifier must be active at the evaluated block height.",
                nameof(activeTopUpShieldVerifier));
        }

        var scaleUnavailable = blockerCodes.Contains("asset_scale_unavailable");
        var scaleUnsupported = blockerCodes.Contains("asset_scale_unsupported");
        var verifierUnavailable = blockerCodes.Contains("transfer_verifier_unavailable");
        var topUpShieldVerifierUnavailable = blockerCodes.Contains("topup_shield_verifier_unavailable");
        if (scaleUnavailable != !assetScale.HasValue)
        {
            throw new ArgumentException(
                "asset_scale_unavailable must be present exactly when assetScale is null.",
                nameof(blockers));
        }
        if (scaleUnsupported != (assetScale is > 28))
        {
            throw new ArgumentException(
                "asset_scale_unsupported must be present exactly when assetScale exceeds 28.",
                nameof(blockers));
        }
        if (verifierUnavailable != (activeTransferVerifier is null))
        {
            throw new ArgumentException(
                "transfer_verifier_unavailable must be present exactly when no active verifier is reported.",
                nameof(blockers));
        }
        if (topUpShieldVerifierUnavailable != (activeTopUpShieldVerifier is null))
        {
            throw new ArgumentException(
                "topup_shield_verifier_unavailable must be present exactly when no active top-up shield verifier is reported.",
                nameof(blockers));
        }
        if (ready
            && (assetScale is null or > 28
                || activeTransferVerifier is null
                || activeTopUpShieldVerifier is null))
        {
            throw new ArgumentException(
                "A ready asset requires a supported scale, active transfer verifier, and active top-up shield verifier.",
                nameof(ready));
        }
        AssetScale = assetScale;
    }

    public string AssetDefinitionId { get; }

    public uint? AssetScale { get; }

    public ulong EvaluatedBlockHeight { get; }

    public string EvaluatedBlockHash { get; }

    public OfflineActiveTransferVerifier? ActiveTransferVerifier { get; }

    /// <summary>
    /// Key-material-free public-to-confidential top-up shield verifier active at the evaluated snapshot.
    /// </summary>
    public OfflineActiveTransferVerifier? ActiveTopUpShieldVerifier { get; }

    public bool Ready { get; }

    public IReadOnlyList<OfflineReadinessBlocker> Blockers => (OfflineReadinessBlocker[])blockers.Clone();
}

/// <summary>Reference returned after Torii accepts an asynchronous Offline command.</summary>
public sealed record OfflineOperationReference
{
    public OfflineOperationReference(
        string operationId,
        OfflineOperationKind kind,
        OfflineOperationState state,
        string transactionHash,
        string statusUri,
        ulong submittedAtMs)
    {
        OperationId = OfflineApiValidation.RequireOperationId(operationId, nameof(operationId));
        OfflineApiValidation.RequireDefinedEnum(kind, nameof(kind));
        OfflineApiValidation.RequireDefinedEnum(state, nameof(state));
        Kind = kind;
        State = state;
        TransactionHash = OfflineApiValidation.RequireTransactionHash(transactionHash, nameof(transactionHash));
        var expectedStatusUri = OfflineApiRoutes.Operation(OperationId);
        if (!string.Equals(statusUri, expectedStatusUri, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                $"statusUri must be the canonical operation resource `{expectedStatusUri}`.",
                nameof(statusUri));
        }
        StatusUri = statusUri;
        SubmittedAtMs = submittedAtMs;
    }

    public string OperationId { get; }

    public OfflineOperationKind Kind { get; }

    public OfflineOperationState State { get; }

    public string TransactionHash { get; }

    public string StatusUri { get; }

    public ulong SubmittedAtMs { get; }
}

/// <summary>Schema-bound finalized top-up anchor retained for the wallet prover.</summary>
public sealed class OfflineTopUpAnchor
{
    private readonly byte[] archive;

    internal OfflineTopUpAnchor(byte[] canonicalArchive)
    {
        archive = (byte[])canonicalArchive.Clone();
    }

    /// <summary>Return a defensive copy of the canonical anchor archive.</summary>
    public byte[] NoritoArchive() => (byte[])archive.Clone();
}

/// <summary>Opaque schema-bound Sumeragi proof for one finalized top-up anchor.</summary>
public sealed class OfflineTopUpFinalityProof
{
    private readonly byte[] archive;

    internal OfflineTopUpFinalityProof(byte[] canonicalArchive)
    {
        archive = (byte[])canonicalArchive.Clone();
    }

    /// <summary>Return a defensive copy of the canonical proof archive.</summary>
    public byte[] NoritoArchive() => (byte[])archive.Clone();
}

/// <summary>Final result of an applied top-up operation.</summary>
public sealed record OfflineTopUpResult
{
    public OfflineTopUpResult(
        string transactionHash,
        ulong finalizedBlockHeight,
        ulong serverTimeMs,
        OfflineTopUpAnchor anchor,
        OfflineTopUpFinalityProof finalityProof)
    {
        TransactionHash = OfflineApiValidation.RequireTransactionHash(transactionHash, nameof(transactionHash));
        FinalizedBlockHeight = OfflineApiValidation.RequirePositive(finalizedBlockHeight, nameof(finalizedBlockHeight));
        ServerTimeMs = OfflineApiValidation.RequirePositive(serverTimeMs, nameof(serverTimeMs));
        Anchor = anchor ?? throw new ArgumentNullException(nameof(anchor));
        FinalityProof = finalityProof ?? throw new ArgumentNullException(nameof(finalityProof));
    }

    public string TransactionHash { get; }

    public ulong FinalizedBlockHeight { get; }

    public ulong ServerTimeMs { get; }

    public OfflineTopUpAnchor Anchor { get; }

    public OfflineTopUpFinalityProof FinalityProof { get; }
}

/// <summary>Final result of an applied redemption.</summary>
public sealed record OfflineRedeemResult
{
    public OfflineRedeemResult(string transactionHash, ulong finalizedBlockHeight, ulong serverTimeMs)
    {
        TransactionHash = OfflineApiValidation.RequireTransactionHash(transactionHash, nameof(transactionHash));
        FinalizedBlockHeight = OfflineApiValidation.RequirePositive(finalizedBlockHeight, nameof(finalizedBlockHeight));
        ServerTimeMs = OfflineApiValidation.RequirePositive(serverTimeMs, nameof(serverTimeMs));
    }

    public string TransactionHash { get; }

    public ulong FinalizedBlockHeight { get; }

    public ulong ServerTimeMs { get; }
}

/// <summary>Operation-specific terminal result.</summary>
public abstract record OfflineOperationResult
{
    private OfflineOperationResult()
    {
    }

    public sealed record TopUp : OfflineOperationResult
    {
        public TopUp(OfflineTopUpResult value)
        {
            Value = value ?? throw new ArgumentNullException(nameof(value));
        }

        public OfflineTopUpResult Value { get; }
    }

    public sealed record Redeem : OfflineOperationResult
    {
        public Redeem(OfflineRedeemResult value)
        {
            Value = value ?? throw new ArgumentNullException(nameof(value));
        }

        public OfflineRedeemResult Value { get; }
    }
}

/// <summary>Queue pressure metadata attached to a typed rejection.</summary>
public sealed record OfflineQueueErrorSnapshot
{
    public OfflineQueueErrorSnapshot(string state, ulong queued, ulong capacity, bool saturated)
    {
        State = OfflineApiValidation.RequireExactToken(state, nameof(state));
        Queued = queued;
        Capacity = capacity;
        Saturated = saturated;
    }

    public string State { get; }

    public ulong Queued { get; }

    public ulong Capacity { get; }

    public bool Saturated { get; }
}

/// <summary>AXT rejection metadata attached to a validation failure.</summary>
public sealed record OfflineAxtErrorDetails
{
    public OfflineAxtErrorDetails(
        string? code,
        string? reason,
        ulong? snapshotVersion,
        ulong? dataspace,
        uint? lane,
        ulong? nextMinHandleEra,
        ulong? nextMinSubNonce)
    {
        Code = OfflineApiValidation.RequireOptionalExactText(code, nameof(code));
        Reason = OfflineApiValidation.RequireOptionalExactText(reason, nameof(reason));
        SnapshotVersion = snapshotVersion;
        Dataspace = dataspace;
        Lane = lane;
        NextMinHandleEra = nextMinHandleEra;
        NextMinSubNonce = nextMinSubNonce;
    }

    public string? Code { get; }

    public string? Reason { get; }

    public ulong? SnapshotVersion { get; }

    public ulong? Dataspace { get; }

    public uint? Lane { get; }

    public ulong? NextMinHandleEra { get; }

    public ulong? NextMinSubNonce { get; }
}

/// <summary>Closed structured metadata attached to an Offline rejection.</summary>
public sealed record OfflineOperationErrorDetails
{
    public OfflineOperationErrorDetails(
        string? layer,
        string? rejectCode,
        OfflineQueueErrorSnapshot? queue,
        ulong? retryAfterSeconds,
        string? endpoint,
        string? field,
        string? expected,
        string? actual,
        string? profile,
        ushort? chainDiscriminant,
        string? transactionHash,
        string? lastStatus,
        string? hint,
        OfflineAxtErrorDetails? axt)
    {
        Layer = OfflineApiValidation.RequireOptionalExactText(layer, nameof(layer));
        RejectCode = OfflineApiValidation.RequireOptionalExactText(rejectCode, nameof(rejectCode));
        Queue = queue;
        RetryAfterSeconds = retryAfterSeconds;
        Endpoint = OfflineApiValidation.RequireOptionalExactText(endpoint, nameof(endpoint));
        Field = OfflineApiValidation.RequireOptionalExactText(field, nameof(field));
        Expected = OfflineApiValidation.RequireOptionalExactText(expected, nameof(expected));
        Actual = OfflineApiValidation.RequireOptionalExactText(actual, nameof(actual));
        Profile = OfflineApiValidation.RequireOptionalExactText(profile, nameof(profile));
        ChainDiscriminant = chainDiscriminant;
        TransactionHash = OfflineApiValidation.RequireOptionalExactToken(transactionHash, nameof(transactionHash));
        LastStatus = OfflineApiValidation.RequireOptionalExactText(lastStatus, nameof(lastStatus));
        Hint = OfflineApiValidation.RequireOptionalExactText(hint, nameof(hint));
        Axt = axt;
    }

    public string? Layer { get; }

    public string? RejectCode { get; }

    public OfflineQueueErrorSnapshot? Queue { get; }

    public ulong? RetryAfterSeconds { get; }

    public string? Endpoint { get; }

    public string? Field { get; }

    public string? Expected { get; }

    public string? Actual { get; }

    public string? Profile { get; }

    public ushort? ChainDiscriminant { get; }

    public string? TransactionHash { get; }

    public string? LastStatus { get; }

    public string? Hint { get; }

    public OfflineAxtErrorDetails? Axt { get; }
}

/// <summary>Stable typed Torii rejection returned by a terminal operation.</summary>
public sealed record OfflineOperationErrorEnvelope
{
    public OfflineOperationErrorEnvelope(
        string code,
        string message,
        OfflineOperationErrorDetails? details = null)
    {
        Code = OfflineApiValidation.RequireCode(code, nameof(code));
        Message = OfflineApiValidation.RequireExactText(message, nameof(message));
        Details = details;
    }

    public string Code { get; }

    public string Message { get; }

    public OfflineOperationErrorDetails? Details { get; }
}

/// <summary>Pollable state of an accepted Offline operation.</summary>
public abstract record OfflineOperationStatus
{
    private OfflineOperationStatus(string operationId)
    {
        OperationId = OfflineApiValidation.RequireOperationId(operationId, nameof(operationId));
    }

    public string OperationId { get; }

    public sealed record Pending : OfflineOperationStatus
    {
        public Pending(
            string operationId,
            OfflineOperationKind kind,
            string transactionHash,
            ulong submittedAtMs)
            : base(operationId)
        {
            OfflineApiValidation.RequireDefinedEnum(kind, nameof(kind));
            Kind = kind;
            TransactionHash = OfflineApiValidation.RequireTransactionHash(
                transactionHash,
                nameof(transactionHash));
            SubmittedAtMs = submittedAtMs;
        }

        public OfflineOperationKind Kind { get; }

        public string TransactionHash { get; }

        public ulong SubmittedAtMs { get; }
    }

    public sealed record Applied : OfflineOperationStatus
    {
        public Applied(string operationId, OfflineOperationResult result)
            : base(operationId)
        {
            Result = result ?? throw new ArgumentNullException(nameof(result));
        }

        public OfflineOperationResult Result { get; }
    }

    public sealed record Rejected : OfflineOperationStatus
    {
        public Rejected(
            string operationId,
            OfflineOperationKind kind,
            string transactionHash,
            OfflineOperationErrorEnvelope error)
            : base(operationId)
        {
            OfflineApiValidation.RequireDefinedEnum(kind, nameof(kind));
            Kind = kind;
            TransactionHash = OfflineApiValidation.RequireTransactionHash(
                transactionHash,
                nameof(transactionHash));
            Error = error ?? throw new ArgumentNullException(nameof(error));
        }

        public OfflineOperationKind Kind { get; }

        public string TransactionHash { get; }

        public OfflineOperationErrorEnvelope Error { get; }
    }
}

/// <summary>Norito decoder for first-release Offline operation responses.</summary>
public static class OfflineOperationCodec
{
    internal const string TopUpRequestSchema = "iroha.torii.v1.offline.top_up.request";
    internal const string RedeemRequestSchema = "iroha.torii.v1.offline.redeem.request";
    private const string ReferenceSchema = "iroha_torii_shared::offline_api::OfflineOperationReference";
    private const string StatusSchema = "iroha_torii_shared::offline_api::OfflineOperationStatus";
    private const string TopUpAnchorSchema =
        "iroha_data_model::offline::model::KagemushaRecursiveSpendTopUpAnchorV2";
    private const string TopUpFinalityProofSchema =
        "iroha_data_model::offline::model::KagemushaTopUpFinalityProofV2";
    private const int TopUpFinalityProofMaxArchiveBytes = 1024 * 1024;
    private const byte CompactLengthFlag = 0x02;
    private const int StatusHeaderPadding = 8;
    private static readonly UTF8Encoding StrictUtf8 = new(false, true);

    /// <summary>Decode a canonical accepted-operation reference.</summary>
    public static OfflineOperationReference DecodeReference(byte[] archive)
    {
        var payload = DecodeCanonicalArchive(ReferenceSchema, archive, expectedPadding: 0);
        var reader = new Reader(payload);
        var operationId = ReadField(reader, ReadString, "operation_id");
        var kind = ReadField(reader, ReadKind, "kind");
        var state = ReadField(reader, ReadState, "state");
        var transactionHash = ReadField(reader, ReadString, "transaction_hash");
        var statusUri = ReadField(reader, ReadString, "status_uri");
        var submittedAtMs = ReadField(reader, static child => child.ReadUInt64(), "submitted_at_ms");
        reader.RequireEnd("Offline operation reference");
        return new OfflineOperationReference(
            operationId,
            kind,
            state,
            transactionHash,
            statusUri,
            submittedAtMs);
    }

    /// <summary>Decode a canonical tagged pending, applied, or rejected operation status.</summary>
    public static OfflineOperationStatus DecodeStatus(byte[] archive)
    {
        var payload = DecodeCanonicalArchive(StatusSchema, archive, StatusHeaderPadding);
        var reader = new Reader(payload);
        OfflineOperationStatus status = reader.ReadUInt32() switch
        {
            0 => DecodePending(reader),
            1 => DecodeApplied(reader),
            2 => DecodeRejected(reader),
            var tag => throw new ArgumentException($"Invalid Offline operation status tag: {tag}.", nameof(archive)),
        };
        reader.RequireEnd("Offline operation status");
        return status;
    }

    internal static CanonicalRequest RequireCanonicalRequest(
        byte[] value,
        string schema,
        int operationIdFieldIndex,
        int fieldCount)
    {
        ArgumentNullException.ThrowIfNull(value);
        if (value.Length == 0)
        {
            throw new ArgumentException("Offline request archive must not be empty.", nameof(value));
        }
        if (value.Length > KagemushaRecursiveSpendNative.NativeArchiveMaxBytes)
        {
            throw new ArgumentException(
                $"Offline request archive must not exceed {KagemushaRecursiveSpendNative.NativeArchiveMaxBytes} bytes.",
                nameof(value));
        }
        if (operationIdFieldIndex < 0 || operationIdFieldIndex >= fieldCount || fieldCount <= 0)
        {
            throw new ArgumentOutOfRangeException(nameof(operationIdFieldIndex));
        }

        var archive = (byte[])value.Clone();
        var payload = DecodeCanonicalArchive(schema, archive, expectedPadding: 0);
        var reader = new Reader(payload);
        byte[]? operationId = null;
        for (var fieldIndex = 0; fieldIndex < fieldCount; fieldIndex++)
        {
            var length = reader.ReadCompactLength($"request field {fieldIndex}");
            if (fieldIndex == operationIdFieldIndex)
            {
                operationId = reader.ReadBytes(length, "operation_id");
            }
            else
            {
                reader.Skip(length, $"request field {fieldIndex}");
            }
        }
        reader.RequireEnd("Offline request");

        if (operationId is null || operationId.Length != 32)
        {
            throw new ArgumentException(
                "Offline request operation_id must contain exactly 32 raw bytes.",
                nameof(value));
        }
        if (operationId.All(static valueByte => valueByte == 0))
        {
            throw new ArgumentException("Offline request operation_id must be non-zero.", nameof(value));
        }

        return new CanonicalRequest(LowercaseHex(operationId), archive);
    }

    private static OfflineOperationStatus DecodePending(Reader reader)
    {
        var operationId = ReadField(reader, ReadString, "operation_id");
        var kind = ReadField(reader, ReadKind, "kind");
        var transactionHash = ReadField(reader, ReadString, "transaction_hash");
        var submittedAtMs = ReadField(reader, static child => child.ReadUInt64(), "submitted_at_ms");
        return new OfflineOperationStatus.Pending(operationId, kind, transactionHash, submittedAtMs);
    }

    private static OfflineOperationStatus DecodeApplied(Reader reader)
    {
        var operationId = ReadField(reader, ReadString, "operation_id");
        var result = ReadField(reader, ReadResult, "result");
        return new OfflineOperationStatus.Applied(operationId, result);
    }

    private static OfflineOperationStatus DecodeRejected(Reader reader)
    {
        var operationId = ReadField(reader, ReadString, "operation_id");
        var kind = ReadField(reader, ReadKind, "kind");
        var transactionHash = ReadField(reader, ReadString, "transaction_hash");
        var error = ReadField(reader, ReadError, "error");
        return new OfflineOperationStatus.Rejected(operationId, kind, transactionHash, error);
    }

    private static OfflineOperationResult ReadResult(Reader reader)
    {
        var tag = reader.ReadUInt32();
        var length = reader.ReadCompactLength("result variant");
        var variant = new Reader(reader.ReadBytes(length, "result variant"));
        OfflineOperationResult result = tag switch
        {
            0 => new OfflineOperationResult.TopUp(ReadTopUpResult(variant)),
            1 => new OfflineOperationResult.Redeem(ReadRedeemResult(variant)),
            _ => throw new ArgumentException($"Invalid Offline result tag: {tag}."),
        };
        variant.RequireEnd("Offline result variant");
        return result;
    }

    private static OfflineTopUpResult ReadTopUpResult(Reader reader)
    {
        var transactionHash = ReadField(reader, ReadString, "transaction_hash");
        var finalizedBlockHeight = ReadField(
            reader,
            static child => child.ReadUInt64(),
            "finalized_block_height");
        var serverTimeMs = ReadField(reader, static child => child.ReadUInt64(), "server_time_ms");
        var anchorPayload = ReadField(
            reader,
            static child => child.ReadBytes(child.Remaining, "anchor"),
            "anchor");
        var anchor = CreateTopUpAnchor(anchorPayload);
        var finalityProofPayload = ReadField(
            reader,
            static child => child.ReadBytes(child.Remaining, "finality_proof"),
            "finality_proof");
        var finalityProof = CreateTopUpFinalityProof(finalityProofPayload);
        return new OfflineTopUpResult(
            transactionHash,
            finalizedBlockHeight,
            serverTimeMs,
            anchor,
            finalityProof);
    }

    private static OfflineRedeemResult ReadRedeemResult(Reader reader)
    {
        var transactionHash = ReadField(reader, ReadString, "transaction_hash");
        var finalizedBlockHeight = ReadField(
            reader,
            static child => child.ReadUInt64(),
            "finalized_block_height");
        var serverTimeMs = ReadField(reader, static child => child.ReadUInt64(), "server_time_ms");
        return new OfflineRedeemResult(transactionHash, finalizedBlockHeight, serverTimeMs);
    }

    private static OfflineOperationErrorEnvelope ReadError(Reader reader)
    {
        var code = ReadField(reader, ReadString, "error.code");
        var message = ReadField(reader, ReadString, "error.message");
        var details = ReadField(
            reader,
            static child => ReadReferenceOption(child, ReadErrorDetails, "error.details"),
            "error.details");
        return new OfflineOperationErrorEnvelope(code, message, details);
    }

    private static OfflineOperationErrorDetails ReadErrorDetails(Reader reader)
    {
        var layer = ReadOptionalStringField(reader, "error.details.layer");
        var rejectCode = ReadOptionalStringField(reader, "error.details.reject_code");
        var queue = ReadField(
            reader,
            static child => ReadReferenceOption(child, ReadQueue, "error.details.queue"),
            "error.details.queue");
        var retryAfterSeconds = ReadOptionalScalarField(
            reader,
            static child => child.ReadUInt64(),
            "error.details.retry_after_seconds");
        var endpoint = ReadOptionalStringField(reader, "error.details.endpoint");
        var field = ReadOptionalStringField(reader, "error.details.field");
        var expected = ReadOptionalStringField(reader, "error.details.expected");
        var actual = ReadOptionalStringField(reader, "error.details.actual");
        var profile = ReadOptionalStringField(reader, "error.details.profile");
        var chainDiscriminant = ReadOptionalScalarField(
            reader,
            static child => child.ReadUInt16(),
            "error.details.chain_discriminant");
        var transactionHash = ReadOptionalStringField(reader, "error.details.tx_hash");
        var lastStatus = ReadOptionalStringField(reader, "error.details.last_status");
        var hint = ReadOptionalStringField(reader, "error.details.hint");
        var axt = ReadField(
            reader,
            static child => ReadReferenceOption(child, ReadAxt, "error.details.axt"),
            "error.details.axt");
        return new OfflineOperationErrorDetails(
            layer,
            rejectCode,
            queue,
            retryAfterSeconds,
            endpoint,
            field,
            expected,
            actual,
            profile,
            chainDiscriminant,
            transactionHash,
            lastStatus,
            hint,
            axt);
    }

    private static OfflineQueueErrorSnapshot ReadQueue(Reader reader)
    {
        var state = ReadField(reader, ReadString, "queue.state");
        var queued = ReadField(reader, static child => child.ReadUInt64(), "queue.queued");
        var capacity = ReadField(reader, static child => child.ReadUInt64(), "queue.capacity");
        var saturated = ReadField(reader, static child => child.ReadBoolean(), "queue.saturated");
        return new OfflineQueueErrorSnapshot(state, queued, capacity, saturated);
    }

    private static OfflineAxtErrorDetails ReadAxt(Reader reader)
    {
        var code = ReadOptionalStringField(reader, "axt.code");
        var reason = ReadOptionalStringField(reader, "axt.reason");
        var snapshotVersion = ReadOptionalScalarField(
            reader,
            static child => child.ReadUInt64(),
            "axt.snapshot_version");
        var dataspace = ReadOptionalScalarField(
            reader,
            static child => child.ReadUInt64(),
            "axt.dataspace");
        var lane = ReadOptionalScalarField(reader, static child => child.ReadUInt32(), "axt.lane");
        var nextMinHandleEra = ReadOptionalScalarField(
            reader,
            static child => child.ReadUInt64(),
            "axt.next_min_handle_era");
        var nextMinSubNonce = ReadOptionalScalarField(
            reader,
            static child => child.ReadUInt64(),
            "axt.next_min_sub_nonce");
        return new OfflineAxtErrorDetails(
            code,
            reason,
            snapshotVersion,
            dataspace,
            lane,
            nextMinHandleEra,
            nextMinSubNonce);
    }

    private static string? ReadOptionalStringField(Reader reader, string field) =>
        ReadField(reader, child => ReadReferenceOption(child, ReadString, field), field);

    private static T? ReadOptionalScalarField<T>(Reader reader, Func<Reader, T> decode, string field)
        where T : struct =>
        ReadField(reader, child => ReadValueOption(child, decode, field), field);

    private static T? ReadReferenceOption<T>(Reader reader, Func<Reader, T> decode, string field)
        where T : class
    {
        return reader.ReadByte() switch
        {
            0 => RequireEmptyOption<T>(reader, field),
            1 => ReadPresentOption(reader, decode, field),
            var tag => throw new ArgumentException($"{field} contains invalid option tag {tag}."),
        };
    }

    private static T? ReadValueOption<T>(Reader reader, Func<Reader, T> decode, string field)
        where T : struct
    {
        return reader.ReadByte() switch
        {
            0 => RequireEmptyOptionValue<T>(reader, field),
            1 => ReadPresentOptionValue(reader, decode, field),
            var tag => throw new ArgumentException($"{field} contains invalid option tag {tag}."),
        };
    }

    private static T? RequireEmptyOption<T>(Reader reader, string field)
        where T : class
    {
        reader.RequireEnd(field);
        return null;
    }

    private static T? RequireEmptyOptionValue<T>(Reader reader, string field)
        where T : struct
    {
        reader.RequireEnd(field);
        return null;
    }

    private static T ReadPresentOption<T>(Reader reader, Func<Reader, T> decode, string field)
    {
        var length = reader.ReadCompactLength(field);
        var child = new Reader(reader.ReadBytes(length, field));
        var value = decode(child);
        child.RequireEnd(field);
        reader.RequireEnd(field);
        return value;
    }

    private static T ReadPresentOptionValue<T>(Reader reader, Func<Reader, T> decode, string field)
        where T : struct => ReadPresentOption(reader, decode, field);

    private static OfflineOperationKind ReadKind(Reader reader) => reader.ReadUInt32() switch
    {
        0 => OfflineOperationKind.TopUp,
        1 => OfflineOperationKind.Redeem,
        var tag => throw new ArgumentException($"Invalid Offline operation kind tag: {tag}."),
    };

    private static OfflineOperationState ReadState(Reader reader) => reader.ReadUInt32() switch
    {
        0 => OfflineOperationState.Pending,
        var tag => throw new ArgumentException($"Invalid Offline operation state tag: {tag}."),
    };

    private static string ReadString(Reader reader)
    {
        var length = reader.ReadCompactLength("string");
        var bytes = reader.ReadBytes(length, "string");
        try
        {
            return StrictUtf8.GetString(bytes);
        }
        catch (DecoderFallbackException exception)
        {
            throw new ArgumentException("Offline operation string must be valid UTF-8.", exception);
        }
    }

    private static T ReadField<T>(Reader reader, Func<Reader, T> decode, string field)
    {
        var length = reader.ReadCompactLength(field);
        var child = new Reader(reader.ReadBytes(length, field));
        var value = decode(child);
        child.RequireEnd(field);
        return value;
    }

    private static OfflineTopUpAnchor CreateTopUpAnchor(byte[] payload)
    {
        var reader = new Reader(payload);
        var fields = new byte[17][];
        for (var index = 0; index < fields.Length; index++)
        {
            var length = reader.ReadCompactLength($"anchor field {index}");
            fields[index] = reader.ReadBytes(length, $"anchor field {index}");
        }
        reader.RequireEnd("top-up anchor");

        if (fields[0].Length != sizeof(ushort)
            || BinaryPrimitives.ReadUInt16LittleEndian(fields[0]) != 2)
        {
            throw new ArgumentException("Top-up anchor version must be 2.");
        }
        RequireNonZeroFixed32(fields[6], "anchor.initial_root");
        RequireNonZeroFixed32(fields[7], "anchor.finalized_root");
        if (fields[6].AsSpan().SequenceEqual(fields[7]))
        {
            throw new ArgumentException("Top-up anchor roots must differ.");
        }
        RequireNonZeroFixed32(fields[10], "anchor.topup_operation_id");
        RequireNonZeroFixed32(fields[12], "anchor.transfer_verifier_commitment");
        RequireNonZeroFixed32(fields[15], "anchor.finalized_tx_hash");
        RequireNonZeroFixed32(fields[16], "anchor.anchor_digest");

        var artifactReader = new Reader(fields[13]);
        OfflineApiValidation.RequireExactText(ReadString(artifactReader), "anchor.artifact_generation");
        artifactReader.RequireEnd("anchor.artifact_generation");
        if (fields[14].Length != sizeof(ulong)
            || BinaryPrimitives.ReadUInt64LittleEndian(fields[14]) == 0)
        {
            throw new ArgumentException("Top-up anchor finalized_height must be positive.");
        }

        return new OfflineTopUpAnchor(NoritoCodec.Encode(TopUpAnchorSchema, payload, CompactLengthFlag));
    }

    private static OfflineTopUpFinalityProof CreateTopUpFinalityProof(byte[] payload)
    {
        var archive = NoritoCodec.Encode(TopUpFinalityProofSchema, payload, CompactLengthFlag);
        if (archive.Length > TopUpFinalityProofMaxArchiveBytes)
        {
            throw new ArgumentException(
                $"Top-up finality proof archive must not exceed {TopUpFinalityProofMaxArchiveBytes} bytes.");
        }
        return new OfflineTopUpFinalityProof(archive);
    }

    private static void RequireNonZeroFixed32(byte[] value, string field)
    {
        if (value.Length != 32 || value.All(static valueByte => valueByte == 0))
        {
            throw new ArgumentException($"{field} must contain exactly 32 non-zero bytes.");
        }
    }

    private static byte[] DecodeCanonicalArchive(string schema, byte[] value, int expectedPadding)
    {
        ArgumentNullException.ThrowIfNull(value);
        if (value.Length > KagemushaRecursiveSpendNative.NativeArchiveMaxBytes)
        {
            throw new ArgumentException(
                $"Norito archive must not exceed {KagemushaRecursiveSpendNative.NativeArchiveMaxBytes} bytes.",
                nameof(value));
        }
        if (value.Length < NoritoHeader.EncodedLength)
        {
            throw new ArgumentException("Norito archive is shorter than the header.", nameof(value));
        }

        var payloadLength = BinaryPrimitives.ReadUInt64LittleEndian(value.AsSpan(23, sizeof(ulong)));
        if (payloadLength > int.MaxValue)
        {
            throw new ArgumentException("Norito archive payload is too large.", nameof(value));
        }
        var expectedLength = (ulong)NoritoHeader.EncodedLength + (uint)expectedPadding + payloadLength;
        if ((ulong)value.Length != expectedLength)
        {
            throw new ArgumentException("Norito archive does not use canonical header alignment.", nameof(value));
        }
        if (value[22] != (byte)NoritoCompression.None || value[39] != CompactLengthFlag)
        {
            throw new ArgumentException(
                "Norito archive must use uncompressed compact sequential field framing.",
                nameof(value));
        }
        if (expectedPadding != 0
            && value.AsSpan(NoritoHeader.EncodedLength, expectedPadding).IndexOfAnyExcept((byte)0) >= 0)
        {
            throw new ArgumentException("Norito archive alignment padding must be zero.", nameof(value));
        }

        var (payload, flags) = NoritoCodec.Decode(schema, value);
        if (flags != CompactLengthFlag || payload.Length != (int)payloadLength)
        {
            throw new ArgumentException("Norito archive framing is not canonical.", nameof(value));
        }
        return payload;
    }

    private static string LowercaseHex(ReadOnlySpan<byte> value)
    {
        const string alphabet = "0123456789abcdef";
        var output = new char[value.Length * 2];
        for (var index = 0; index < value.Length; index++)
        {
            output[index * 2] = alphabet[value[index] >> 4];
            output[index * 2 + 1] = alphabet[value[index] & 0x0f];
        }
        return new string(output);
    }

    internal sealed record CanonicalRequest(string OperationId, byte[] Archive);

    private sealed class Reader(byte[] data)
    {
        private int offset;

        internal int Remaining => data.Length - offset;

        internal byte ReadByte()
        {
            RequireAvailable(1, "byte");
            return data[offset++];
        }

        internal bool ReadBoolean() => ReadByte() switch
        {
            0 => false,
            1 => true,
            var value => throw new ArgumentException($"Invalid Norito boolean value: {value}."),
        };

        internal ushort ReadUInt16()
        {
            RequireAvailable(sizeof(ushort), "u16");
            var value = BinaryPrimitives.ReadUInt16LittleEndian(data.AsSpan(offset, sizeof(ushort)));
            offset += sizeof(ushort);
            return value;
        }

        internal uint ReadUInt32()
        {
            RequireAvailable(sizeof(uint), "u32");
            var value = BinaryPrimitives.ReadUInt32LittleEndian(data.AsSpan(offset, sizeof(uint)));
            offset += sizeof(uint);
            return value;
        }

        internal ulong ReadUInt64()
        {
            RequireAvailable(sizeof(ulong), "u64");
            var value = BinaryPrimitives.ReadUInt64LittleEndian(data.AsSpan(offset, sizeof(ulong)));
            offset += sizeof(ulong);
            return value;
        }

        internal int ReadCompactLength(string field)
        {
            ulong value = 0;
            var shift = 0;
            var start = offset;
            for (var index = 0; index < 10; index++)
            {
                var current = ReadByte();
                var low = current & 0x7f;
                if (shift >= 63 && low > 1)
                {
                    throw new ArgumentException($"{field} length overflows u64.");
                }
                value |= (ulong)low << shift;
                if ((current & 0x80) == 0)
                {
                    var encodedLength = offset - start;
                    if (encodedLength > 1 && value < (1UL << (7 * (encodedLength - 1))))
                    {
                        throw new ArgumentException($"{field} length is not minimally encoded.");
                    }
                    if (value > int.MaxValue)
                    {
                        throw new ArgumentException($"{field} length is too large.");
                    }
                    return (int)value;
                }
                shift += 7;
            }
            throw new ArgumentException($"{field} length is unterminated.");
        }

        internal byte[] ReadBytes(int length, string field)
        {
            if (length < 0)
            {
                throw new ArgumentOutOfRangeException(nameof(length));
            }
            RequireAvailable(length, field);
            var output = data.AsSpan(offset, length).ToArray();
            offset += length;
            return output;
        }

        internal void Skip(int length, string field)
        {
            if (length < 0)
            {
                throw new ArgumentOutOfRangeException(nameof(length));
            }
            RequireAvailable(length, field);
            offset += length;
        }

        internal void RequireEnd(string context)
        {
            if (Remaining != 0)
            {
                throw new ArgumentException($"Trailing fields or bytes after {context}.");
            }
        }

        private void RequireAvailable(int length, string field)
        {
            if (length > Remaining)
            {
                throw new ArgumentException($"Unexpected end of {field}.");
            }
        }
    }
}

internal static class OfflineApiValidation
{
    private static readonly System.Text.RegularExpressions.Regex AssetAliasPattern = new(
        "^[a-z0-9]+(?:[._-][a-z0-9]+)*#[a-z0-9]+(?:-[a-z0-9]+)*(?:\\.[a-z0-9]+(?:-[a-z0-9]+)*)?$",
        System.Text.RegularExpressions.RegexOptions.CultureInvariant);

    internal static ulong RequirePositive(ulong value, string parameterName)
    {
        if (value == 0)
        {
            throw new ArgumentOutOfRangeException(
                parameterName,
                value,
                "Applied result fields must be at least 1.");
        }
        return value;
    }

    internal static string RequireOperationId(string? value, string parameterName)
    {
        if (value is null
            || value.Length != 64
            || value.All(static character => character == '0')
            || value.Any(static character => character is not (>= '0' and <= '9')
                and not (>= 'a' and <= 'f')))
        {
            throw new ArgumentException(
                "Operation ID must be exactly 32 non-zero bytes encoded as lowercase hexadecimal.",
                parameterName);
        }
        return value;
    }

    internal static string RequireAssetSelector(string? value, string parameterName)
    {
        var exact = RequireExactText(value, parameterName);
        if (exact.Contains('#'))
        {
            if (!AssetAliasPattern.IsMatch(exact))
            {
                throw new ArgumentException(
                    "Asset selector must be a lowercase scoped asset alias.",
                    parameterName);
            }
            return exact;
        }
        return OfflineNoteCanonicalPayloadCodec.RequireCanonicalAssetDefinitionId(exact);
    }

    internal static string RequireTransactionHash(string? value, string parameterName)
        => RequireLowercaseHash(value, parameterName, "Transaction hash");

    internal static string RequireLowercaseHash(
        string? value,
        string parameterName,
        string label)
    {
        if (value is null
            || value.Length != 64
            || value.Any(static character => character is not (>= '0' and <= '9')
                and not (>= 'a' and <= 'f')))
        {
            throw new ArgumentException(
                $"{label} must be exactly 32 bytes encoded as lowercase hexadecimal.",
                parameterName);
        }
        return value;
    }

    internal static string RequireBoundedText(string? value, int maxLength, string parameterName)
    {
        var exact = RequireExactText(value, parameterName);
        if (CountUnicodeScalars(exact, parameterName) > maxLength)
        {
            throw new ArgumentException(
                $"Value must contain at most {maxLength} Unicode characters.",
                parameterName);
        }
        return exact;
    }

    internal static string RequireCode(string? value, string parameterName)
    {
        var exact = RequireExactToken(value, parameterName);
        if (exact.Length > 64
            || exact[0] is not (>= 'a' and <= 'z') and not (>= '0' and <= '9')
            || exact.Any(static character => character is not (>= 'a' and <= 'z')
                and not (>= '0' and <= '9')
                and not '_'))
        {
            throw new ArgumentException(
                "Code must be a 1-64 character lowercase stable identifier.",
                parameterName);
        }
        return exact;
    }

    internal static string RequireExactToken(string? value, string parameterName)
    {
        var exact = RequireExactText(value, parameterName);
        if (exact.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException("Value must not contain whitespace.", parameterName);
        }
        return exact;
    }

    internal static string? RequireOptionalExactToken(string? value, string parameterName) =>
        value is null ? null : RequireExactToken(value, parameterName);

    internal static string? RequireOptionalExactText(string? value, string parameterName) =>
        value is null ? null : RequireExactText(value, parameterName);

    internal static void RequireDefinedEnum<T>(T value, string parameterName)
        where T : struct, Enum
    {
        if (!Enum.IsDefined(value))
        {
            throw new ArgumentOutOfRangeException(parameterName, value, "Value is not a defined enum member.");
        }
    }

    internal static string RequireExactText(string? value, string parameterName)
    {
        if (string.IsNullOrWhiteSpace(value)
            || !string.Equals(value.Trim(), value, StringComparison.Ordinal)
            || value.Any(char.IsControl))
        {
            throw new ArgumentException("Value must be exact non-empty text.", parameterName);
        }
        _ = CountUnicodeScalars(value, parameterName);
        return value;
    }

    private static int CountUnicodeScalars(string value, string parameterName)
    {
        var count = 0;
        for (var index = 0; index < value.Length; index++)
        {
            var character = value[index];
            if (char.IsHighSurrogate(character))
            {
                if (++index >= value.Length || !char.IsLowSurrogate(value[index]))
                {
                    throw new ArgumentException(
                        "Value must contain well-formed Unicode.",
                        parameterName);
                }
            }
            else if (char.IsLowSurrogate(character))
            {
                throw new ArgumentException(
                    "Value must contain well-formed Unicode.",
                    parameterName);
            }
            count++;
        }
        return count;
    }
}
