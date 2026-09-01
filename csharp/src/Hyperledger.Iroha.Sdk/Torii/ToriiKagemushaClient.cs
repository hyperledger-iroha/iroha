using System.Buffers.Binary;
using System.Globalization;
using System.Net;
using System.Net.Http.Headers;
using System.Text;
using System.Text.Json;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Torii;

public sealed partial class ToriiClient
{
    public const int KagemushaRequiredBridgeAbiVersion = ToriiKagemushaTransport.BridgeAbiVersion;
    public const int KagemushaManifestVersion = ToriiKagemushaTransport.ManifestVersion;
    public const int KagemushaMaximumHops = ToriiKagemushaTransport.MaxHops;

    public async Task<ToriiOfflineStatus> GetOfflineCapabilityAsync(
        CancellationToken cancellationToken = default)
    {
        using var response = await SendAsync(
            HttpMethod.Get,
            "/v1/offline/readiness",
            query: null,
            content: null,
            accept: "application/json",
            cancellationToken: cancellationToken);
        RequireKagemushaStatus(response, HttpStatusCode.OK, "Offline capability");
        using var document = await ReadKagemushaJsonAsync(
            response,
            "Offline capability response",
            ToriiKagemushaTransport.MaxReadinessJsonResponseBytes,
            cancellationToken);
        return ParseOfflineCapability(document.RootElement);
    }

    public Task<ToriiKagemushaOperationReference> SubmitKagemushaTopUpV4Async(
        ToriiKagemushaTopUpRequestV4 request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);
        return SubmitKagemushaV4Async(
            "/v1/offline/top-up",
            request.Norito,
            request.Identity,
            cancellationToken);
    }

    public Task<ToriiKagemushaOperationReference> SubmitKagemushaRedeemV4Async(
        ToriiKagemushaRedeemRequestV4 request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);
        return SubmitKagemushaV4Async(
            "/v1/offline/redeem",
            request.Norito,
            request.Identity,
            cancellationToken);
    }

    /// <summary>
    /// Polls one accepted operation identity and validates the returned
    /// result's continuity. Exact retries and a foreign-authority global Applied
    /// winner may advance the transaction hash. Pending must preserve the
    /// signed request's authorization issuance timestamp even when a retry
    /// replaces its transaction hash. Applied top-ups additionally authenticate their balanced-
    /// Merkle path and combined post-state root against the embedded
    /// execution commitment. The embedded Commit-QC signature still requires
    /// verification against a separately trusted validator roster before the
    /// proof can establish offline consensus finality.
    /// </summary>
    public Task<ToriiKagemushaOperationStatus> GetKagemushaOperationStatusAsync(
        ToriiKagemushaOperationReference reference,
        CancellationToken cancellationToken = default) =>
        GetKagemushaOperationStatusAsync(
            reference,
            NativeKagemushaOperationStatusValidator.Instance,
            cancellationToken);

    internal async Task<ToriiKagemushaOperationStatus> GetKagemushaOperationStatusAsync(
        ToriiKagemushaOperationReference reference,
        IKagemushaOperationStatusValidator statusValidator,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(statusValidator);
        var canonicalId = RequirePollableKagemushaOperationReference(reference);
        using var response = await SendAsync(
            HttpMethod.Get,
            $"/v1/offline/operations/{canonicalId}",
            content: null,
            accept: "application/json",
            cancellationToken: cancellationToken);
        RequireKagemushaStatus(response, HttpStatusCode.OK, "Kagemusha operation status");
        using var document = await ReadKagemushaJsonAsync(
            response,
            "Kagemusha operation status response",
            ToriiKagemushaTransport.MaxOperationStatusJsonResponseBytes,
            cancellationToken,
            statusValidator);
        return ParseKagemushaOperationStatus(document.RootElement, reference);
    }

    private async Task<ToriiKagemushaOperationReference> SubmitKagemushaV4Async(
        string path,
        byte[] norito,
        ToriiKagemushaOperationIdentity identity,
        CancellationToken cancellationToken)
    {
        EnsureOneShotTransportIsVerified();
        using var content = new ByteArrayContent(norito);
        content.Headers.ContentType = new MediaTypeHeaderValue("application/x-norito");
        using var response = await SendAsync(
            HttpMethod.Post,
            path,
            content: content,
            accept: "application/json",
            configureRequest: message =>
            {
                message.Headers.Remove("Idempotency-Key");
                message.Headers.TryAddWithoutValidation("Idempotency-Key", identity.OperationId);
            },
            cancellationToken: cancellationToken);
        if (response.StatusCode != HttpStatusCode.Accepted)
        {
            throw new InvalidDataException($"Kagemusha command expected HTTP 202, got {(int)response.StatusCode}.");
        }
        RequireKagemushaRetryAfter(response);

        using var document = await ReadKagemushaJsonAsync(
            response,
            "Kagemusha operation reference response",
            ToriiKagemushaTransport.MaxOperationReferenceJsonResponseBytes,
            cancellationToken);
        var location = response.Headers.Location?.OriginalString;
        return ParseKagemushaOperationReference(
            document.RootElement,
            identity,
            location);
    }

    private static async Task<JsonDocument> ReadKagemushaJsonAsync(
        HttpResponseMessage response,
        string context,
        int maximumBytes,
        CancellationToken cancellationToken,
        IKagemushaOperationStatusValidator? statusValidator = null)
    {
        if (!string.Equals(
                response.Content.Headers.ContentType?.MediaType,
                "application/json",
                StringComparison.OrdinalIgnoreCase))
        {
            throw new InvalidDataException($"{context} must use Content-Type application/json.");
        }

        var body = await ReadBoundedKagemushaJsonBodyAsync(
            response.Content,
            context,
            maximumBytes,
            cancellationToken);
        statusValidator?.Validate(body);
        await using var stream = new MemoryStream(body, writable: false);
        return await ParseJsonDocumentRejectingDuplicatePropertiesAsync(
            stream,
            context,
            cancellationToken);
    }

    private static void RequireKagemushaStatus(
        HttpResponseMessage response,
        HttpStatusCode expected,
        string context)
    {
        if (response.StatusCode != expected)
        {
            throw new InvalidDataException(
                $"{context} expected HTTP {(int)expected}, got {(int)response.StatusCode}.");
        }
    }

    private static async Task<byte[]> ReadBoundedKagemushaJsonBodyAsync(
        HttpContent content,
        string context,
        int maximumBytes,
        CancellationToken cancellationToken)
    {
        var declaredLength = content.Headers.ContentLength;
        if (declaredLength.HasValue && declaredLength.Value > maximumBytes)
        {
            throw new InvalidDataException(
                $"{context} exceeds the {maximumBytes}-byte limit.");
        }

        await using var input = await content.ReadAsStreamAsync(cancellationToken);
        using var output = declaredLength is > 0
            ? new MemoryStream(checked((int)declaredLength.Value))
            : new MemoryStream();
        var buffer = new byte[8 * 1024];
        while (true)
        {
            var read = await input.ReadAsync(buffer.AsMemory(), cancellationToken);
            if (read == 0)
            {
                break;
            }
            if (output.Length > maximumBytes - read)
            {
                throw new InvalidDataException(
                    $"{context} exceeds the {maximumBytes}-byte limit.");
            }
            await output.WriteAsync(buffer.AsMemory(0, read), cancellationToken);
        }
        return output.ToArray();
    }

    private ToriiOfflineStatus ParseOfflineCapability(JsonElement root)
    {
        RequireExactFields(
            root,
            "Offline capability response",
            "cash_handoff_capability",
            "required_bridge_abi_version",
            "max_hops",
            "ready");
        var capability = root.Deserialize<ToriiOfflineStatus>(SerializerOptions)
            ?? throw new JsonException("Offline capability response deserialized to null.");
        if (!string.Equals(
                capability.CashHandoffCapability,
                "cash_handoff_v1",
                StringComparison.Ordinal))
        {
            throw new JsonException("cash_handoff_capability must be cash_handoff_v1.");
        }
        if (capability.RequiredBridgeAbiVersion != KagemushaRequiredBridgeAbiVersion)
        {
            throw new JsonException(
                $"required_bridge_abi_version must be {KagemushaRequiredBridgeAbiVersion}.");
        }
        if (capability.MaxHops != KagemushaMaximumHops)
        {
            throw new JsonException($"max_hops must be {KagemushaMaximumHops}.");
        }
        if (!capability.Ready)
        {
            throw new JsonException("ready must be true for universal offline capability.");
        }

        return capability;
    }

    private static ToriiKagemushaOperationReference ParseKagemushaOperationReference(
        JsonElement root,
        ToriiKagemushaOperationIdentity expectedIdentity,
        string? location)
    {
        RequireExactFields(
            root,
            "Kagemusha operation reference",
            "identity",
            "state",
            "transaction_hash",
            "status_uri");
        var identity = ParseKagemushaOperationIdentity(
            root.GetProperty("identity"),
            "identity");
        var state = ParseTaggedPendingState(root.GetProperty("state"));
        var transactionHash = RequireJsonHash(root.GetProperty("transaction_hash"), "transaction_hash");
        var statusUri = RequireJsonString(root.GetProperty("status_uri"), "status_uri");
        var expectedUri = $"/v1/offline/operations/{expectedIdentity.OperationId}";
        if (!identity.Equals(expectedIdentity)
            || state != ToriiKagemushaOperationState.Pending
            || !string.Equals(statusUri, expectedUri, StringComparison.Ordinal)
            || !string.Equals(location, expectedUri, StringComparison.Ordinal))
        {
            throw new JsonException("Kagemusha operation reference does not match the submitted V4 command.");
        }

        return new ToriiKagemushaOperationReference
        {
            Identity = identity,
            State = state,
            TransactionHash = transactionHash,
            StatusUri = statusUri,
        };
    }

    private static ToriiKagemushaOperationStatus ParseKagemushaOperationStatus(
        JsonElement root,
        ToriiKagemushaOperationReference expectedReference)
    {
        RequireExactFields(root, "Kagemusha operation status", "state", "value");
        var stateText = RequireJsonString(root.GetProperty("state"), "state");
        var value = root.GetProperty("value");
        if (value.ValueKind != JsonValueKind.Object)
        {
            throw new JsonException("Kagemusha operation status value must be an object.");
        }
        // Bind the whole immutable identity before accepting a Pending cursor
        // or parsing terminal result/error data.
        var identity = ParseKagemushaOperationIdentity(
            value.GetProperty("identity"),
            "value.identity");
        if (!identity.Equals(expectedReference.Identity))
        {
            throw new JsonException("Kagemusha operation status identity does not match the accepted reference.");
        }

        var status = stateText switch
        {
            "pending" => ParsePendingStatus(value, identity),
            "applied" => ParseAppliedStatus(value, identity),
            "rejected" => ParseRejectedStatus(value, identity),
            _ => throw new JsonException("Kagemusha operation state must be pending, applied, or rejected."),
        };

        return status;
    }

    private static ToriiKagemushaOperationStatus ParsePendingStatus(
        JsonElement value,
        ToriiKagemushaOperationIdentity identity)
    {
        RequireExactFields(
            value,
            "Kagemusha pending operation",
            "identity",
            "transaction_hash");
        return new ToriiKagemushaOperationStatus
        {
            Identity = identity,
            State = ToriiKagemushaOperationState.Pending,
            TransactionHash = RequireJsonHash(value.GetProperty("transaction_hash"), "value.transaction_hash"),
        };
    }

    private static ToriiKagemushaOperationStatus ParseAppliedStatus(
        JsonElement value,
        ToriiKagemushaOperationIdentity identity)
    {
        RequireExactFields(value, "Kagemusha applied operation", "identity", "result");
        var resultTag = value.GetProperty("result");
        RequireExactFields(resultTag, "Kagemusha applied result", "kind", "result");
        var kindText = RequireJsonString(resultTag.GetProperty("kind"), "value.result.kind");
        var result = resultTag.GetProperty("result");
        if (kindText == "redeem")
        {
            if (identity.Kind != ToriiKagemushaOperationKind.Redeem)
            {
                throw new JsonException("Kagemusha applied result kind does not match its identity.");
            }
            RequireExactFields(
                result,
                "Kagemusha redeem result",
                "transaction_hash",
                "finalized_block_height");
            return new ToriiKagemushaOperationStatus
            {
                Identity = identity,
                State = ToriiKagemushaOperationState.Applied,
                TransactionHash = RequireJsonHash(
                    result.GetProperty("transaction_hash"),
                    "result.transaction_hash"),
                RedeemResult = new ToriiKagemushaRedeemResultV4
                {
                    TransactionHash = RequireJsonHash(result.GetProperty("transaction_hash"), "result.transaction_hash"),
                    FinalizedBlockHeight = RequireJsonPositiveUInt64(result.GetProperty("finalized_block_height"), "result.finalized_block_height"),
                },
            };
        }
        if (kindText != "top_up")
        {
            throw new JsonException("Kagemusha applied result kind must be top_up or redeem.");
        }
        if (identity.Kind != ToriiKagemushaOperationKind.TopUp)
        {
            throw new JsonException("Kagemusha applied result kind does not match its identity.");
        }

        RequireExactFields(
            result,
            "Kagemusha top-up result",
            "transaction_hash",
            "finalized_block_height",
            "anchor",
            "finality_proof");
        var transactionHash = RequireJsonHash(
            result.GetProperty("transaction_hash"),
            "result.transaction_hash");
        var finalizedBlockHeight = RequireJsonPositiveUInt64(
            result.GetProperty("finalized_block_height"),
            "result.finalized_block_height");
        var anchor = RequireJsonObject(result.GetProperty("anchor"), "result.anchor");
        if (RequireJsonUInt64(
                RequireJsonProperty(anchor, "version", "result.anchor"),
                "result.anchor.version") != KagemushaManifestVersion)
        {
            throw new JsonException("Kagemusha top-up anchor must use V4.");
        }
        var artifactBinding = RequireJsonObject(
            RequireJsonProperty(anchor, "artifact_binding", "result.anchor"),
            "result.anchor.artifact_binding");
        if (RequireJsonUInt64(
                RequireJsonProperty(artifactBinding, "version", "result.anchor.artifact_binding"),
                "result.anchor.artifact_binding.version")
            != KagemushaManifestVersion)
        {
            throw new JsonException("Kagemusha top-up artifact binding must use V4.");
        }
        var anchorOperationId = RequireJsonFixedBytes32MatchesOperationId(
            RequireJsonProperty(anchor, "topup_operation_id", "result.anchor"),
            identity.OperationId,
            "result.anchor.topup_operation_id");

        var finalityProof = RequireJsonObject(
            result.GetProperty("finality_proof"),
            "result.finality_proof");
        if (RequireJsonUInt64(
                RequireJsonProperty(finalityProof, "version", "result.finality_proof"),
                "result.finality_proof.version") != 1)
        {
            throw new JsonException(
                "KagemushaTopUpFinalityProofV2 must use numeric wire version 1.");
        }
        var finalityAnchor = RequireJsonObject(
            RequireJsonProperty(finalityProof, "anchor", "result.finality_proof"),
            "result.finality_proof.anchor");
        var finalityOperationId = RequireJsonFixedBytes32MatchesOperationId(
            RequireJsonProperty(
                finalityAnchor,
                "topup_operation_id",
                "result.finality_proof.anchor"),
            identity.OperationId,
            "result.finality_proof.anchor.topup_operation_id");

        // This authenticates the anchor path and combined state root against
        // the execution commitment embedded in the response. It deliberately
        // does not claim that the Commit-QC signature is authentic: that
        // requires a separately trusted validator roster.
        ValidateKagemushaTopUpExecutionCommitment(
            anchor,
            finalityProof,
            anchorOperationId,
            finalityOperationId,
            transactionHash,
            finalizedBlockHeight);

        return new ToriiKagemushaOperationStatus
        {
            Identity = identity,
            State = ToriiKagemushaOperationState.Applied,
            TransactionHash = transactionHash,
            TopUpResult = new ToriiKagemushaTopUpResultV4
            {
                TransactionHash = transactionHash,
                FinalizedBlockHeight = finalizedBlockHeight,
                Anchor = anchor.Clone(),
                FinalityProof = finalityProof.Clone(),
            },
        };
    }

    private static ToriiKagemushaOperationStatus ParseRejectedStatus(
        JsonElement value,
        ToriiKagemushaOperationIdentity identity)
    {
        RequireExactFields(
            value,
            "Kagemusha rejected operation",
            "identity",
            "transaction_hash",
            "error");
        var error = value.GetProperty("error");
        if (error.ValueKind != JsonValueKind.Object)
        {
            throw new JsonException("Kagemusha rejection error must be an object.");
        }
        var hasDetails = error.TryGetProperty("details", out var detailValue);
        if (hasDetails)
        {
            RequireExactFields(error, "Kagemusha rejection error", "code", "message", "details");
            if (detailValue.ValueKind != JsonValueKind.Object)
            {
                throw new JsonException("value.error.details must be an object when present.");
            }
        }
        else
        {
            RequireExactFields(error, "Kagemusha rejection error", "code", "message");
        }

        var details = hasDetails ? detailValue.Clone() : (JsonElement?)null;
        return new ToriiKagemushaOperationStatus
        {
            Identity = identity,
            State = ToriiKagemushaOperationState.Rejected,
            TransactionHash = RequireJsonHash(value.GetProperty("transaction_hash"), "value.transaction_hash"),
            Error = new ToriiKagemushaOperationError
            {
                Code = RequireJsonErrorCode(error.GetProperty("code"), "value.error.code"),
                Message = RequireJsonExactText(error.GetProperty("message"), "value.error.message"),
                Details = details,
            },
        };
    }

    private static ToriiKagemushaOperationIdentity ParseKagemushaOperationIdentity(
        JsonElement element,
        string context)
    {
        RequireExactFields(
            element,
            context,
            "operation_id",
            "request_authority_digest",
            "canonical_request_digest",
            "kind",
            "issued_at_ms",
            "expires_at_ms");
        try
        {
            return new ToriiKagemushaOperationIdentity(
                RequireJsonOperationId(
                    element.GetProperty("operation_id"),
                    $"{context}.operation_id"),
                RequireJsonHash(
                    element.GetProperty("request_authority_digest"),
                    $"{context}.request_authority_digest"),
                RequireJsonHash(
                    element.GetProperty("canonical_request_digest"),
                    $"{context}.canonical_request_digest"),
                ParseTaggedKind(element.GetProperty("kind"), $"{context}.kind"),
                RequireJsonPositiveUInt64(
                    element.GetProperty("issued_at_ms"),
                    $"{context}.issued_at_ms"),
                RequireJsonPositiveUInt64(
                    element.GetProperty("expires_at_ms"),
                    $"{context}.expires_at_ms"));
        }
        catch (ArgumentException error)
        {
            throw new JsonException($"{context} is invalid.", error);
        }
    }

    private static ToriiKagemushaOperationKind ParseTaggedKind(JsonElement element, string context)
    {
        RequireExactFields(element, context, "kind", "value");
        if (element.GetProperty("value").ValueKind != JsonValueKind.Null)
        {
            throw new JsonException($"{context}.value must be null.");
        }
        return RequireJsonString(element.GetProperty("kind"), $"{context}.kind") switch
        {
            "top_up" => ToriiKagemushaOperationKind.TopUp,
            "redeem" => ToriiKagemushaOperationKind.Redeem,
            _ => throw new JsonException($"{context}.kind must be top_up or redeem."),
        };
    }

    private static ToriiKagemushaOperationState ParseTaggedPendingState(JsonElement element)
    {
        RequireExactFields(element, "Kagemusha pending state", "state", "value");
        if (!string.Equals(
                RequireJsonString(element.GetProperty("state"), "state.state"),
                "pending",
                StringComparison.Ordinal)
            || element.GetProperty("value").ValueKind != JsonValueKind.Null)
        {
            throw new JsonException("Kagemusha command reference state must be pending with a null value.");
        }
        return ToriiKagemushaOperationState.Pending;
    }

    private static string RequirePollableKagemushaOperationReference(
        ToriiKagemushaOperationReference reference)
    {
        ArgumentNullException.ThrowIfNull(reference);
        ArgumentNullException.ThrowIfNull(reference.Identity);
        var operationId = ToriiKagemushaTransport.RequireOperationId(
            reference.Identity.OperationId,
            nameof(reference));
        var expectedStatusUri = $"/v1/offline/operations/{operationId}";
        if (reference.State != ToriiKagemushaOperationState.Pending
            || !string.Equals(reference.StatusUri, expectedStatusUri, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "Kagemusha operation reference must be the exact pending reference returned by Torii.",
                nameof(reference));
        }

        var transactionHash = reference.TransactionHash;
        if (transactionHash is null
            || transactionHash.Length != 64
            || transactionHash.Any(static character =>
                character is not (>= '0' and <= '9') and not (>= 'a' and <= 'f'))
            || "13579bdf".IndexOf(transactionHash[^1]) < 0)
        {
            throw new ArgumentException(
                "Kagemusha operation reference must contain an exact canonical marker-bearing Iroha transaction hash.",
                nameof(reference));
        }

        return operationId;
    }

    private static void RequireExactFields(JsonElement element, string context, params string[] expected)
    {
        if (element.ValueKind != JsonValueKind.Object)
        {
            throw new JsonException($"{context} must be an object.");
        }
        var expectedSet = expected.ToHashSet(StringComparer.Ordinal);
        var actual = element.EnumerateObject().Select(static property => property.Name).ToArray();
        if (actual.Length != expected.Length || actual.Any(name => !expectedSet.Contains(name)))
        {
            throw new JsonException($"{context} contains missing or unknown fields.");
        }
    }

    private static string RequireJsonOperationId(JsonElement element, string context) =>
        ToriiKagemushaTransport.RequireOperationId(RequireJsonString(element, context), context);

    private static string RequireJsonHash(JsonElement element, string context)
    {
        var value = RequireJsonString(element, context);
        RequireLowerHex32(value, context);
        if ("13579bdf".IndexOf(value[^1]) < 0)
        {
            throw new JsonException(
                $"{context} must be an exact canonical marker-bearing lowercase 32-byte Iroha hash.");
        }
        return value;
    }

    private static string RequireJsonString(JsonElement element, string context)
    {
        if (element.ValueKind != JsonValueKind.String || string.IsNullOrEmpty(element.GetString()))
        {
            throw new JsonException($"{context} must be a non-empty string.");
        }
        return element.GetString()!;
    }

    private static ulong RequireJsonUInt64(JsonElement element, string context)
    {
        if (element.ValueKind != JsonValueKind.Number || !element.TryGetUInt64(out var value))
        {
            throw new JsonException($"{context} must be an unsigned 64-bit integer.");
        }
        return value;
    }

    private static uint RequireJsonUInt32(JsonElement element, string context)
    {
        if (element.ValueKind != JsonValueKind.Number || !element.TryGetUInt32(out var value))
        {
            throw new JsonException($"{context} must be an unsigned 32-bit integer.");
        }
        return value;
    }

    private static ulong RequireJsonPositiveUInt64(JsonElement element, string context)
    {
        var value = RequireJsonUInt64(element, context);
        if (value == 0)
        {
            throw new JsonException($"{context} must be a positive unsigned 64-bit integer.");
        }
        return value;
    }

    private static JsonElement RequireJsonProperty(
        JsonElement element,
        string propertyName,
        string context)
    {
        RequireJsonObject(element, context);
        if (!element.TryGetProperty(propertyName, out var property))
        {
            throw new JsonException($"{context} is missing {propertyName}.");
        }
        return property;
    }

    private static byte[] RequireJsonFixedBytes32MatchesOperationId(
        JsonElement element,
        string operationId,
        string context)
    {
        var bytes = RequireJsonFixedBytes32(element, context);
        var expected = Convert.FromHexString(operationId);
        if (!bytes.AsSpan().SequenceEqual(expected))
        {
            throw new JsonException($"{context} does not match the requested operation id.");
        }

        return bytes;
    }

    private static byte[] RequireJsonFixedBytes32(JsonElement element, string context)
    {
        if (element.ValueKind != JsonValueKind.Array || element.GetArrayLength() != 32)
        {
            throw new JsonException($"{context} must be an array of 32 bytes.");
        }

        var bytes = new byte[32];
        var index = 0;
        foreach (var item in element.EnumerateArray())
        {
            if (!item.TryGetInt32(out var value) || value is < 0 or > byte.MaxValue)
            {
                throw new JsonException($"{context} must be an array of 32 bytes.");
            }
            bytes[index] = (byte)value;
            index++;
        }
        return bytes;
    }

    private static void ValidateKagemushaTopUpExecutionCommitment(
        JsonElement anchor,
        JsonElement finalityProof,
        byte[] anchorOperationId,
        byte[] finalityOperationId,
        string transactionHash,
        ulong finalizedBlockHeight)
    {
        const string proofContext = "result.finality_proof";
        var anchorDigest = RequireJsonFixedBytes32(
            RequireJsonProperty(anchor, "anchor_digest", "result.anchor"),
            "result.anchor.anchor_digest");
        var finalityAnchor = RequireJsonObject(
            RequireJsonProperty(finalityProof, "anchor", proofContext),
            $"{proofContext}.anchor");
        var finalityAnchorDigest = RequireJsonFixedBytes32(
            RequireJsonProperty(finalityAnchor, "anchor_digest", $"{proofContext}.anchor"),
            $"{proofContext}.anchor.anchor_digest");
        if (!anchorOperationId.AsSpan().SequenceEqual(finalityOperationId)
            || !anchorDigest.AsSpan().SequenceEqual(finalityAnchorDigest)
            || anchorDigest.AsSpan().IndexOfAnyExcept((byte)0) < 0)
        {
            throw new JsonException(
                "Kagemusha top-up finality proof anchor does not match the finalized anchor.");
        }

        var anchorTransactionHash = RequireJsonFixedBytes32(
            RequireJsonProperty(anchor, "finalized_tx_hash", "result.anchor"),
            "result.anchor.finalized_tx_hash");
        var anchorFinalizedHeight = RequireJsonPositiveUInt64(
            RequireJsonProperty(anchor, "finalized_height", "result.anchor"),
            "result.anchor.finalized_height");
        var anchorNetworkId = RequireJsonIrohaHash(
            RequireJsonProperty(anchor, "network_id", "result.anchor"),
            "result.anchor.network_id");
        if (!anchorTransactionHash.AsSpan().SequenceEqual(Convert.FromHexString(transactionHash))
            || anchorFinalizedHeight != finalizedBlockHeight)
        {
            throw new JsonException(
                "Kagemusha top-up terminal transaction or height does not match the finalized anchor.");
        }

        var anchorPath = RequireJsonObject(
            RequireJsonProperty(finalityProof, "anchor_path", proofContext),
            $"{proofContext}.anchor_path");
        RequireExactFields(
            anchorPath,
            "Kagemusha top-up finality anchor path",
            "leaf_index",
            "leaf_count",
            "siblings");
        var leafIndex = RequireJsonUInt32(
            anchorPath.GetProperty("leaf_index"),
            $"{proofContext}.anchor_path.leaf_index");
        var leafCount = RequireJsonUInt32(
            anchorPath.GetProperty("leaf_count"),
            $"{proofContext}.anchor_path.leaf_count");
        if (leafCount is 0 or > 16 || leafIndex >= leafCount)
        {
            throw new JsonException(
                "Kagemusha top-up finality anchor path has an invalid leaf index or count.");
        }

        var siblingsElement = anchorPath.GetProperty("siblings");
        if (siblingsElement.ValueKind != JsonValueKind.Array)
        {
            throw new JsonException(
                $"{proofContext}.anchor_path.siblings must be an array.");
        }
        var expectedDepth = 0;
        for (var width = 1U; width < leafCount; width <<= 1)
        {
            expectedDepth++;
        }
        if (siblingsElement.GetArrayLength() != expectedDepth || expectedDepth > 4)
        {
            throw new JsonException(
                $"{proofContext}.anchor_path.siblings must contain the canonical {expectedDepth}-level path.");
        }

        var siblings = new byte[expectedDepth][];
        var siblingIndex = 0;
        foreach (var siblingElement in siblingsElement.EnumerateArray())
        {
            var sibling = RequireJsonFixedBytes32(
                siblingElement,
                $"{proofContext}.anchor_path.siblings[{siblingIndex}]");
            if ((sibling[^1] & 1) == 0)
            {
                throw new JsonException(
                    $"{proofContext}.anchor_path.siblings[{siblingIndex}] must set the Iroha hash marker bit.");
            }
            siblings[siblingIndex++] = sibling;
        }

        var commitQc = RequireJsonObject(
            RequireJsonProperty(finalityProof, "commit_qc", proofContext),
            $"{proofContext}.commit_qc");
        var heightContext = RequireJsonObject(
            RequireJsonProperty(commitQc, "height_context", $"{proofContext}.commit_qc"),
            $"{proofContext}.commit_qc.height_context");
        var contextHeight = RequireJsonPositiveUInt64(
            RequireJsonProperty(
                heightContext,
                "height",
                $"{proofContext}.commit_qc.height_context"),
            $"{proofContext}.commit_qc.height_context.height");
        var contextNetworkId = RequireJsonIrohaHash(
            RequireJsonProperty(
                heightContext,
                "network_id",
                $"{proofContext}.commit_qc.height_context"),
            $"{proofContext}.commit_qc.height_context.network_id");
        if (contextHeight != finalizedBlockHeight
            || !contextNetworkId.AsSpan().SequenceEqual(anchorNetworkId))
        {
            throw new JsonException(
                "Kagemusha top-up Commit-QC height context does not match the finalized anchor.");
        }
        var certificate = RequireJsonObject(
            RequireJsonProperty(commitQc, "certificate", $"{proofContext}.commit_qc"),
            $"{proofContext}.commit_qc.certificate");
        var executionCommitment = RequireJsonObject(
            RequireJsonProperty(
                certificate,
                "execution_commitment",
                $"{proofContext}.commit_qc.certificate"),
            $"{proofContext}.commit_qc.certificate.execution_commitment");
        var commitmentContext =
            $"{proofContext}.commit_qc.certificate.execution_commitment";
        var committedLeafCount = RequireJsonUInt32(
            RequireJsonProperty(executionCommitment, "topup_anchor_count", commitmentContext),
            $"{commitmentContext}.topup_anchor_count");
        if (committedLeafCount != leafCount)
        {
            throw new JsonException(
                "Kagemusha top-up finality anchor path leaf count does not match the execution commitment.");
        }

        var committedTopUpRoot = RequireJsonIrohaHash(
            RequireJsonProperty(executionCommitment, "topup_anchor_root", commitmentContext),
            $"{commitmentContext}.topup_anchor_root");
        var current = ComputeKagemushaTopUpLeafHash(finalityOperationId, finalityAnchorDigest);
        var index = leafIndex;
        for (var level = 0; level < siblings.Length; level++)
        {
            var sibling = siblings[level];
            current = (index & 1) == 0
                ? ComputeKagemushaTopUpNodeHash((ushort)level, current, sibling)
                : ComputeKagemushaTopUpNodeHash((ushort)level, sibling, current);
            index >>= 1;
        }
        if (!current.AsSpan().SequenceEqual(committedTopUpRoot))
        {
            throw new JsonException(
                "Kagemusha top-up finality anchor path does not authenticate the anchor against the embedded execution commitment.");
        }

        var ordinaryWritesRoot = RequireJsonIrohaHash(
            RequireJsonProperty(executionCommitment, "ordinary_writes_root", commitmentContext),
            $"{commitmentContext}.ordinary_writes_root");
        var committedPostStateRoot = RequireJsonIrohaHash(
            RequireJsonProperty(executionCommitment, "post_state_root", commitmentContext),
            $"{commitmentContext}.post_state_root");
        var expectedPostStateRoot = ComputeKagemushaTopUpPostStateRoot(
            leafCount,
            ordinaryWritesRoot,
            committedTopUpRoot);
        if (!expectedPostStateRoot.AsSpan().SequenceEqual(committedPostStateRoot))
        {
            throw new JsonException(
                "Kagemusha top-up execution commitment post-state root does not authenticate the top-up projection.");
        }
    }

    private static byte[] ComputeKagemushaTopUpLeafHash(
        ReadOnlySpan<byte> operationId,
        ReadOnlySpan<byte> anchorDigest)
    {
        var key = new byte[1 + IrohaHash.Length];
        key[0] = 0xd2;
        operationId.CopyTo(key.AsSpan(1));
        var keyHash = IrohaHash.Hash(key);
        var valueHash = IrohaHash.Hash(anchorDigest);
        var preimage = new byte[1 + 2 * IrohaHash.Length];
        preimage[0] = 0;
        keyHash.CopyTo(preimage.AsSpan(1));
        valueHash.CopyTo(preimage.AsSpan(1 + IrohaHash.Length));
        return IrohaHash.Hash(preimage);
    }

    private static byte[] ComputeKagemushaTopUpNodeHash(
        ushort level,
        ReadOnlySpan<byte> left,
        ReadOnlySpan<byte> right)
    {
        ReadOnlySpan<byte> domain = "iroha:kagemusha:v2:topup-node"u8;
        var preimage = new byte[domain.Length + 1 + sizeof(ushort) + 2 * IrohaHash.Length];
        domain.CopyTo(preimage);
        BinaryPrimitives.WriteUInt16LittleEndian(
            preimage.AsSpan(domain.Length + 1, sizeof(ushort)),
            level);
        left.CopyTo(preimage.AsSpan(domain.Length + 1 + sizeof(ushort)));
        right.CopyTo(
            preimage.AsSpan(domain.Length + 1 + sizeof(ushort) + IrohaHash.Length));
        return IrohaHash.Hash(preimage);
    }

    private static byte[] ComputeKagemushaTopUpPostStateRoot(
        uint topUpCount,
        ReadOnlySpan<byte> ordinaryWritesRoot,
        ReadOnlySpan<byte> topUpRoot)
    {
        ReadOnlySpan<byte> domain = "iroha:kagemusha:v2:post-state-root"u8;
        var preimage = new byte[domain.Length + 1 + sizeof(uint) + 2 * IrohaHash.Length];
        domain.CopyTo(preimage);
        BinaryPrimitives.WriteUInt32LittleEndian(
            preimage.AsSpan(domain.Length + 1, sizeof(uint)),
            topUpCount);
        ordinaryWritesRoot.CopyTo(preimage.AsSpan(domain.Length + 1 + sizeof(uint)));
        topUpRoot.CopyTo(
            preimage.AsSpan(domain.Length + 1 + sizeof(uint) + IrohaHash.Length));
        return IrohaHash.Hash(preimage);
    }

    private static byte[] RequireJsonIrohaHash(JsonElement element, string context)
    {
        var value = RequireJsonString(element, context);
        if (value.Length != 74
            || !value.StartsWith("hash:", StringComparison.Ordinal)
            || value[69] != '#')
        {
            throw new JsonException(
                $"{context} must be a canonical checksummed Norito hash literal.");
        }

        var body = value.AsSpan(5, 64);
        var checksum = value.AsSpan(70, 4);
        if (body.IndexOfAnyExcept("0123456789ABCDEF".AsSpan()) >= 0
            || checksum.IndexOfAnyExcept("0123456789ABCDEF".AsSpan()) >= 0
            || !ushort.TryParse(
                checksum,
                NumberStyles.HexNumber,
                CultureInfo.InvariantCulture,
                out var supplied)
            || supplied != ComputeKagemushaCrc16(
                Encoding.ASCII.GetBytes(value.AsSpan(0, 69).ToString())))
        {
            throw new JsonException(
                $"{context} has a malformed or invalid Norito hash checksum.");
        }

        var bytes = Convert.FromHexString(body);
        if ((bytes[^1] & 1) == 0)
        {
            throw new JsonException($"{context} must set the Iroha hash marker bit.");
        }
        return bytes;
    }

    private static ushort ComputeKagemushaCrc16(ReadOnlySpan<byte> bytes)
    {
        var crc = 0xffff;
        foreach (var item in bytes)
        {
            crc ^= item << 8;
            for (var bit = 0; bit < 8; bit++)
            {
                crc = (crc & 0x8000) != 0
                    ? ((crc << 1) ^ 0x1021) & 0xffff
                    : (crc << 1) & 0xffff;
            }
        }
        return (ushort)crc;
    }

    private static string RequireJsonErrorCode(JsonElement element, string context)
    {
        var value = RequireJsonExactText(element, context, 64, 64);
        if (value.Any(static character =>
                character is not (>= 'a' and <= 'z')
                    and not (>= '0' and <= '9')
                    and not '_')
            || value[0] == '_')
        {
            throw new JsonException($"{context} must be a stable lowercase error code.");
        }
        return value;
    }

    private static string RequireJsonExactText(
        JsonElement element,
        string context,
        int maximumScalars = 1024,
        int maximumUtf8Bytes = 4096)
    {
        var value = RequireJsonString(element, context);
        var scalarCount = 0;
        for (var index = 0; index < value.Length; index++)
        {
            var character = value[index];
            if (character is >= '\u0000' and <= '\u001f'
                or >= '\u007f' and <= '\u009f')
            {
                throw new JsonException($"{context} must be exact non-empty text.");
            }
            if (char.IsHighSurrogate(character))
            {
                if (index + 1 >= value.Length || !char.IsLowSurrogate(value[index + 1]))
                {
                    throw new JsonException($"{context} must be exact non-empty text.");
                }
                index++;
            }
            else if (char.IsLowSurrogate(character))
            {
                throw new JsonException($"{context} must be exact non-empty text.");
            }
            scalarCount++;
        }

        if (value[0] == '\ufeff'
            || value[^1] == '\ufeff'
            || !string.Equals(value.Trim(), value, StringComparison.Ordinal)
            || scalarCount > maximumScalars
            || Encoding.UTF8.GetByteCount(value) > maximumUtf8Bytes)
        {
            throw new JsonException($"{context} must be exact non-empty text.");
        }
        return value;
    }

    private static void RequireKagemushaRetryAfter(HttpResponseMessage response)
    {
        if (!response.Headers.NonValidated.TryGetValues("Retry-After", out var values))
        {
            throw new InvalidDataException(
                "Kagemusha operation reference must include a positive Retry-After value.");
        }

        var rawValues = values.ToArray();
        if (rawValues.Length != 1)
        {
            throw new InvalidDataException(
                "Kagemusha operation reference has an ambiguous Retry-After header.");
        }

        var raw = rawValues[0];
        if (raw.Length == 0
            || raw.Length > 20
            || raw.Any(static character => character is < '0' or > '9')
            || !ulong.TryParse(raw, NumberStyles.None, CultureInfo.InvariantCulture, out var seconds)
            || seconds == 0)
        {
            throw new InvalidDataException(
                "Kagemusha operation reference Retry-After must be a positive u64 number of seconds.");
        }
    }

    private static void RequireLowerHex32(string? value, string context)
    {
        if (value is null
            || value.Length != 64
            || value.Any(static character => character is not (>= '0' and <= '9') and not (>= 'a' and <= 'f')))
        {
            throw new JsonException($"{context} must be lowercase 32-byte hexadecimal.");
        }
    }
}
