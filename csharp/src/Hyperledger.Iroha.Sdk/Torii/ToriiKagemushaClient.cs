using System.Buffers;
using System.Net;
using System.Net.Http.Headers;
using System.Text;
using System.Text.Json;

namespace Hyperledger.Iroha.Torii;

public sealed partial class ToriiClient
{
    public const int KagemushaRequiredBridgeAbiVersion = ToriiKagemushaTransport.BridgeAbiVersion;
    public const int KagemushaManifestVersion = ToriiKagemushaTransport.ManifestVersion;
    public const int KagemushaMaximumHops = ToriiKagemushaTransport.MaxHops;
    public const string KagemushaCashHandoffCapability = "cash_handoff_v1";
    private static readonly (string Code, string Message)[] OfflineCapabilityActivationBlockersV1 =
    [
        (
            "offline_cash_authenticated_release_unavailable",
            "No authenticated Offline Cash V1 release is selected by this asset-neutral response."),
        (
            "offline_cash_eligible_asset_unavailable",
            "No eligible Offline Cash V1 asset is selected by this asset-neutral response."),
        (
            "offline_cash_proof_backend_unavailable",
            "No reviewed production Offline Cash V1 proof and secure-device backend is authenticated by this response."),
    ];

    public async Task<ToriiOfflineStatus> GetOfflineCapabilityAsync(
        CancellationToken cancellationToken = default)
    {
        using var response = await SendAsync(
            HttpMethod.Get,
            "/v1/offline/readiness",
            content: null,
            accept: "application/json",
            cancellationToken: cancellationToken);
        using var document = await ReadKagemushaJsonAsync(
            response,
            "Offline capability response",
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
            request.OperationId,
            request.IssuedAtMilliseconds,
            request.NetworkId,
            ToriiKagemushaOperationKind.TopUp,
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
            request.OperationId,
            request.IssuedAtMilliseconds,
            request.NetworkId,
            ToriiKagemushaOperationKind.Redeem,
            cancellationToken);
    }

    public async Task<ToriiKagemushaOperationStatus> GetKagemushaOperationStatusAsync(
        string operationId,
        CancellationToken cancellationToken = default)
    {
        var canonicalId = ToriiKagemushaTransport.RequireOperationId(operationId, nameof(operationId));
        var expectedNetworkId = Options.LocalSigningContext?.NetworkId
            ?? throw new InvalidOperationException(
                "Kagemusha operation status validation requires "
                + "ToriiClientOptions.LocalSigningContext with the exact NetworkId.");
        using var response = await SendAsync(
            HttpMethod.Get,
            $"/v1/offline/operations/{canonicalId}",
            content: null,
            accept: "application/json",
            cancellationToken: cancellationToken);
        using var document = await ReadKagemushaJsonAsync(
            response,
            "Kagemusha operation status response",
            cancellationToken);
        return ParseKagemushaOperationStatus(
            document.RootElement,
            canonicalId,
            expectedNetworkId);
    }

    private async Task<ToriiKagemushaOperationReference> SubmitKagemushaV4Async(
        string path,
        byte[] norito,
        string operationId,
        ulong submittedAtMilliseconds,
        NetworkId requestNetworkId,
        ToriiKagemushaOperationKind expectedKind,
        CancellationToken cancellationToken)
    {
        var expectedNetworkId = Options.LocalSigningContext?.NetworkId
            ?? throw new InvalidOperationException(
                "Kagemusha command submission requires ToriiClientOptions.LocalSigningContext "
                + "with the exact NetworkId.");
        if (requestNetworkId != expectedNetworkId)
        {
            throw new ArgumentException(
                "Kagemusha signed request network does not match the local signing context.",
                nameof(requestNetworkId));
        }
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
                message.Headers.TryAddWithoutValidation("Idempotency-Key", operationId);
            },
            cancellationToken: cancellationToken);
        if (response.StatusCode != HttpStatusCode.Accepted)
        {
            throw new InvalidDataException($"Kagemusha command expected HTTP 202, got {(int)response.StatusCode}.");
        }
        RequirePositiveRetryAfter(response);

        using var document = await ReadKagemushaJsonAsync(
            response,
            "Kagemusha operation reference response",
            cancellationToken);
        var location = response.Headers.Location?.OriginalString;
        return ParseKagemushaOperationReference(
            document.RootElement,
            operationId,
            submittedAtMilliseconds,
            expectedKind,
            location);
    }

    private static async Task<JsonDocument> ReadKagemushaJsonAsync(
        HttpResponseMessage response,
        string context,
        CancellationToken cancellationToken)
    {
        if (!string.Equals(
                response.Content.Headers.ContentType?.MediaType,
                "application/json",
                StringComparison.OrdinalIgnoreCase))
        {
            throw new InvalidDataException($"{context} must use Content-Type application/json.");
        }

        await using var stream = await response.Content.ReadAsStreamAsync(cancellationToken);
        return await ParseJsonDocumentRejectingDuplicatePropertiesAsync(
            stream,
            context,
            cancellationToken);
    }

    private ToriiOfflineStatus ParseOfflineCapability(JsonElement root)
    {
        RequireExactFields(
            root,
            "Offline capability response",
            "mandatory",
            "cash_handoff_capability",
            "required_bridge_abi_version",
            "max_hops",
            "ready",
            "assets",
            "blockers");
        var status = root.Deserialize<ToriiOfflineStatus>(serializerOptions)
            ?? throw new JsonException("Offline capability response deserialized to null.");
        if (status.Mandatory)
        {
            throw new JsonException("mandatory must be false.");
        }
        if (!string.Equals(
                status.CashHandoffCapability,
                KagemushaCashHandoffCapability,
                StringComparison.Ordinal))
        {
            throw new JsonException($"cash_handoff_capability must be {KagemushaCashHandoffCapability}.");
        }
        if (status.RequiredBridgeAbiVersion != KagemushaRequiredBridgeAbiVersion)
        {
            throw new JsonException($"required_bridge_abi_version must be {KagemushaRequiredBridgeAbiVersion}.");
        }
        if (status.MaxHops != KagemushaMaximumHops)
        {
            throw new JsonException($"max_hops must be {KagemushaMaximumHops}.");
        }
        if (status.Ready)
        {
            throw new JsonException("ready must be false for the asset-neutral capability response.");
        }
        if (status.Assets.Length != 0)
        {
            throw new JsonException("assets must be an empty array for the asset-neutral capability response.");
        }
        if (status.Blockers.Length != OfflineCapabilityActivationBlockersV1.Length)
        {
            throw new JsonException("blockers must contain the three canonical activation blockers.");
        }
        for (var index = 0; index < status.Blockers.Length; index++)
        {
            var blocker = status.Blockers[index];
            RequireExactFields(
                root.GetProperty("blockers")[index],
                $"Offline capability response blocker {index}",
                "code",
                "message");
            var expected = OfflineCapabilityActivationBlockersV1[index];
            if (!string.Equals(blocker.Code, expected.Code, StringComparison.Ordinal)
                || !string.Equals(blocker.Message, expected.Message, StringComparison.Ordinal))
            {
                throw new JsonException($"blockers[{index}] is not the canonical activation blocker.");
            }
        }

        return status with
        {
            Assets = status.Assets.ToArray(),
            Blockers = status.Blockers.ToArray(),
        };
    }

    private static ToriiKagemushaOperationReference ParseKagemushaOperationReference(
        JsonElement root,
        string expectedOperationId,
        ulong expectedSubmittedAtMilliseconds,
        ToriiKagemushaOperationKind expectedKind,
        string? location)
    {
        RequireExactFields(
            root,
            "Kagemusha operation reference",
            "operation_id",
            "kind",
            "state",
            "transaction_hash",
            "status_uri",
            "submitted_at_ms");
        var operationId = RequireJsonOperationId(root.GetProperty("operation_id"), "operation_id");
        var kind = ParseTaggedKind(root.GetProperty("kind"), "kind");
        var state = ParseTaggedPendingState(root.GetProperty("state"));
        var transactionHash = RequireJsonHash(root.GetProperty("transaction_hash"), "transaction_hash");
        var statusUri = RequireJsonString(root.GetProperty("status_uri"), "status_uri");
        var submittedAtMilliseconds = RequireJsonPositiveUInt64(
            root.GetProperty("submitted_at_ms"),
            "submitted_at_ms");
        var expectedUri = $"/v1/offline/operations/{expectedOperationId}";
        if (!string.Equals(operationId, expectedOperationId, StringComparison.Ordinal)
            || kind != expectedKind
            || state != ToriiKagemushaOperationState.Pending
            || !string.Equals(statusUri, expectedUri, StringComparison.Ordinal)
            || submittedAtMilliseconds != expectedSubmittedAtMilliseconds
            || !string.Equals(location, expectedUri, StringComparison.Ordinal))
        {
            throw new JsonException("Kagemusha operation reference does not match the submitted V4 command.");
        }

        return new ToriiKagemushaOperationReference
        {
            OperationId = operationId,
            Kind = kind,
            State = state,
            TransactionHash = transactionHash,
            StatusUri = statusUri,
            SubmittedAtMilliseconds = submittedAtMilliseconds,
        };
    }

    private static ToriiKagemushaOperationStatus ParseKagemushaOperationStatus(
        JsonElement root,
        string expectedOperationId,
        NetworkId expectedNetworkId)
    {
        RequireExactFields(root, "Kagemusha operation status", "state", "value");
        var stateText = RequireJsonString(root.GetProperty("state"), "state");
        var value = root.GetProperty("value");
        if (value.ValueKind != JsonValueKind.Object)
        {
            throw new JsonException("Kagemusha operation status value must be an object.");
        }
        var operationId = RequireJsonOperationId(value.GetProperty("operation_id"), "value.operation_id");
        if (!string.Equals(operationId, expectedOperationId, StringComparison.Ordinal))
        {
            throw new JsonException("Kagemusha operation status does not match the requested operation id.");
        }

        return stateText switch
        {
            "pending" => ParsePendingStatus(value, operationId),
            "applied" => ParseAppliedStatus(value, operationId, expectedNetworkId),
            "rejected" => ParseRejectedStatus(value, operationId),
            _ => throw new JsonException("Kagemusha operation state must be pending, applied, or rejected."),
        };
    }

    private static ToriiKagemushaOperationStatus ParsePendingStatus(JsonElement value, string operationId)
    {
        RequireExactFields(
            value,
            "Kagemusha pending operation",
            "operation_id",
            "kind",
            "transaction_hash",
            "submitted_at_ms");
        return new ToriiKagemushaOperationStatus
        {
            OperationId = operationId,
            State = ToriiKagemushaOperationState.Pending,
            Kind = ParseTaggedKind(value.GetProperty("kind"), "value.kind"),
            TransactionHash = RequireJsonHash(value.GetProperty("transaction_hash"), "value.transaction_hash"),
            SubmittedAtMilliseconds = RequireJsonPositiveUInt64(
                value.GetProperty("submitted_at_ms"),
                "value.submitted_at_ms"),
        };
    }

    private static ToriiKagemushaOperationStatus ParseAppliedStatus(
        JsonElement value,
        string operationId,
        NetworkId expectedNetworkId)
    {
        RequireExactFields(value, "Kagemusha applied operation", "operation_id", "result");
        var resultTag = value.GetProperty("result");
        RequireExactFields(resultTag, "Kagemusha applied result", "kind", "result");
        var kindText = RequireJsonString(resultTag.GetProperty("kind"), "value.result.kind");
        var result = resultTag.GetProperty("result");
        if (kindText == "redeem")
        {
            RequireExactFields(
                result,
                "Kagemusha redeem result",
                "transaction_hash",
                "finalized_block_height",
                "server_time_ms");
            var redeemFinalizedBlockHeight = RequireJsonPositiveUInt64(
                result.GetProperty("finalized_block_height"),
                "result.finalized_block_height");
            var redeemServerTimeMilliseconds = RequireJsonPositiveUInt64(
                result.GetProperty("server_time_ms"),
                "result.server_time_ms");
            return new ToriiKagemushaOperationStatus
            {
                OperationId = operationId,
                State = ToriiKagemushaOperationState.Applied,
                Kind = ToriiKagemushaOperationKind.Redeem,
                RedeemResult = new ToriiKagemushaRedeemResultV4
                {
                    TransactionHash = RequireJsonHash(result.GetProperty("transaction_hash"), "result.transaction_hash"),
                    FinalizedBlockHeight = redeemFinalizedBlockHeight,
                    ServerTimeMilliseconds = redeemServerTimeMilliseconds,
                },
            };
        }
        if (kindText != "top_up")
        {
            throw new JsonException("Kagemusha applied result kind must be top_up or redeem.");
        }

        RequireExactFields(
            result,
            "Kagemusha top-up result",
            "transaction_hash",
            "finalized_block_height",
            "server_time_ms",
            "anchor",
            "finality_proof");
        var transactionHash = RequireJsonHash(
            result.GetProperty("transaction_hash"),
            "result.transaction_hash");
        var finalizedBlockHeight = RequireJsonPositiveUInt64(
            result.GetProperty("finalized_block_height"),
            "result.finalized_block_height");
        var serverTimeMilliseconds = RequireJsonPositiveUInt64(
            result.GetProperty("server_time_ms"),
            "result.server_time_ms");
        var anchor = result.GetProperty("anchor");
        var finalityProof = result.GetProperty("finality_proof");
        ValidateTopUpResultBindings(
            operationId,
            transactionHash,
            finalizedBlockHeight,
            anchor,
            finalityProof,
            expectedNetworkId);

        return new ToriiKagemushaOperationStatus
        {
            OperationId = operationId,
            State = ToriiKagemushaOperationState.Applied,
            Kind = ToriiKagemushaOperationKind.TopUp,
            TopUpResult = new ToriiKagemushaTopUpResultV4
            {
                TransactionHash = transactionHash,
                FinalizedBlockHeight = finalizedBlockHeight,
                ServerTimeMilliseconds = serverTimeMilliseconds,
                Anchor = anchor.Clone(),
                FinalityProof = finalityProof.Clone(),
            },
        };
    }

    private static ToriiKagemushaOperationStatus ParseRejectedStatus(JsonElement value, string operationId)
    {
        RequireExactFields(
            value,
            "Kagemusha rejected operation",
            "operation_id",
            "kind",
            "transaction_hash",
            "error");
        var error = value.GetProperty("error");
        if (error.ValueKind != JsonValueKind.Object)
        {
            throw new JsonException("Kagemusha rejection error must be an object.");
        }
        RequireExactFields(error, "Kagemusha rejection error", "code", "message");

        var code = RequireJsonString(error.GetProperty("code"), "value.error.code");
        if (!string.Equals(code, "offline_operation_rejected", StringComparison.Ordinal))
        {
            throw new JsonException("value.error.code must be offline_operation_rejected.");
        }

        return new ToriiKagemushaOperationStatus
        {
            OperationId = operationId,
            State = ToriiKagemushaOperationState.Rejected,
            Kind = ParseTaggedKind(value.GetProperty("kind"), "value.kind"),
            TransactionHash = RequireJsonHash(value.GetProperty("transaction_hash"), "value.transaction_hash"),
            Error = new ToriiKagemushaOperationError
            {
                Code = code,
                Message = RequireJsonRejectionMessage(error.GetProperty("message"), "value.error.message"),
            },
        };
    }

    private static void RequirePositiveRetryAfter(HttpResponseMessage response)
    {
        if (!response.Headers.NonValidated.TryGetValues("Retry-After", out var values))
        {
            throw new InvalidDataException(
                "Kagemusha command response must include a positive Retry-After value.");
        }

        var valuesArray = values.ToArray();
        if (valuesArray.Length != 1
            || valuesArray[0].Length == 0
            || valuesArray[0][0] is not (>= '1' and <= '9')
            || valuesArray[0].Any(static character => character is not (>= '0' and <= '9'))
            || !ulong.TryParse(valuesArray[0], out var retryAfterSeconds)
            || retryAfterSeconds == 0)
        {
            throw new InvalidDataException(
                "Kagemusha command response must include a positive Retry-After value.");
        }
    }

    private static void ValidateTopUpResultBindings(
        string operationId,
        string transactionHash,
        ulong finalizedBlockHeight,
        JsonElement anchor,
        JsonElement finalityProof,
        NetworkId expectedNetworkId)
    {
        if (anchor.ValueKind != JsonValueKind.Object)
        {
            throw new JsonException("result.anchor must be an object.");
        }
        if (RequireJsonUInt64(anchor.GetProperty("version"), "result.anchor.version")
            != KagemushaManifestVersion)
        {
            throw new JsonException("Kagemusha top-up anchor must use V4.");
        }

        var artifactBinding = anchor.GetProperty("artifact_binding");
        if (artifactBinding.ValueKind != JsonValueKind.Object
            || RequireJsonUInt64(
                artifactBinding.GetProperty("version"),
                "result.anchor.artifact_binding.version") != KagemushaManifestVersion)
        {
            throw new JsonException("Kagemusha top-up artifact binding must use V4.");
        }

        var expectedOperationId = Convert.FromHexString(operationId);
        var expectedTransactionHash = Convert.FromHexString(transactionHash);
        var anchorOperationId = RequireJsonFixed32Bytes(
            anchor.GetProperty("topup_operation_id"),
            "result.anchor.topup_operation_id");
        var anchorTransactionHash = RequireJsonFixed32Bytes(
            anchor.GetProperty("finalized_tx_hash"),
            "result.anchor.finalized_tx_hash");
        var anchorFinalizedHeight = RequireJsonPositiveUInt64(
            anchor.GetProperty("finalized_height"),
            "result.anchor.finalized_height");
        if (!anchorOperationId.AsSpan().SequenceEqual(expectedOperationId)
            || !anchorTransactionHash.AsSpan().SequenceEqual(expectedTransactionHash)
            || anchorFinalizedHeight != finalizedBlockHeight)
        {
            throw new JsonException(
                "Kagemusha top-up anchor does not match the operation, transaction, or finalized height.");
        }

        if (finalityProof.ValueKind != JsonValueKind.Object
            || RequireJsonUInt64(finalityProof.GetProperty("version"), "result.finality_proof.version") != 1)
        {
            throw new JsonException("Kagemusha top-up finality proof must use version 1.");
        }
        var proofAnchor = finalityProof.GetProperty("anchor");
        var proofCommitQc = finalityProof.GetProperty("commit_qc");
        var proofHeightContext = proofCommitQc.GetProperty("height_context");
        var anchorDigest = RequireJsonNonZeroFixed32Bytes(
            anchor.GetProperty("anchor_digest"),
            "result.anchor.anchor_digest");
        var proofOperationId = RequireJsonFixed32Bytes(
            proofAnchor.GetProperty("topup_operation_id"),
            "result.finality_proof.anchor.topup_operation_id");
        var proofAnchorDigest = RequireJsonNonZeroFixed32Bytes(
            proofAnchor.GetProperty("anchor_digest"),
            "result.finality_proof.anchor.anchor_digest");
        var proofHeight = RequireJsonPositiveUInt64(
            proofHeightContext.GetProperty("height"),
            "result.finality_proof.commit_qc.height_context.height");
        var anchorNetworkId = RequireJsonNetworkId(
            anchor.GetProperty("network_id"),
            "result.anchor.network_id");
        var currentNote = anchor.GetProperty("current_note");
        if (currentNote.ValueKind != JsonValueKind.Object)
        {
            throw new JsonException("result.anchor.current_note must be an object.");
        }
        var currentNoteNetworkId = RequireJsonNetworkId(
            currentNote.GetProperty("network_id"),
            "result.anchor.current_note.network_id");
        var proofNetworkId = RequireJsonNetworkId(
            proofHeightContext.GetProperty("network_id"),
            "result.finality_proof.commit_qc.height_context.network_id");
        if (!proofOperationId.AsSpan().SequenceEqual(anchorOperationId)
            || !proofAnchorDigest.AsSpan().SequenceEqual(anchorDigest)
            || proofHeight != finalizedBlockHeight
            || anchorNetworkId != expectedNetworkId
            || currentNoteNetworkId != expectedNetworkId
            || proofNetworkId != expectedNetworkId)
        {
            throw new JsonException(
                "Kagemusha top-up finality proof does not match the exact anchor, height, or expected network.");
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

    private static NetworkId RequireJsonNetworkId(JsonElement element, string context)
    {
        var value = RequireJsonString(element, context);
        try
        {
            return NetworkId.ParseNoritoJsonLiteral(value);
        }
        catch (FormatException error)
        {
            throw new JsonException(
                $"{context} must be a canonical checksummed Norito NetworkId hash literal.",
                error);
        }
    }

    private static string RequireJsonHash(JsonElement element, string context)
    {
        var value = RequireJsonString(element, context);
        RequireLowerHex32(value, context);
        var bytes = Convert.FromHexString(value);
        if ((bytes[^1] & 1) == 0)
        {
            throw new JsonException($"{context} must set the Iroha hash marker bit.");
        }
        return value;
    }

    private static string RequireJsonRejectionMessage(JsonElement element, string context)
    {
        if (element.ValueKind != JsonValueKind.String)
        {
            throw new JsonException($"{context} must be a non-empty string.");
        }

        string? value;
        try
        {
            value = element.GetString();
        }
        catch (InvalidOperationException error)
        {
            throw new JsonException($"{context} must contain only well-formed Unicode scalar values.", error);
        }
        if (string.IsNullOrEmpty(value))
        {
            throw new JsonException($"{context} must be a non-empty string.");
        }

        var remaining = value.AsSpan();
        while (!remaining.IsEmpty)
        {
            if (Rune.DecodeFromUtf16(remaining, out _, out var charactersConsumed) != OperationStatus.Done)
            {
                throw new JsonException($"{context} must contain only well-formed Unicode scalar values.");
            }
            remaining = remaining[charactersConsumed..];
        }

        if (!string.Equals(value, value.Trim(), StringComparison.Ordinal))
        {
            throw new JsonException($"{context} must not contain surrounding whitespace.");
        }

        var scalarCount = 0;
        foreach (var rune in value.EnumerateRunes())
        {
            if (Rune.IsControl(rune))
            {
                throw new JsonException($"{context} must not contain control characters.");
            }
            if (++scalarCount > 1_024)
            {
                throw new JsonException($"{context} must contain at most 1024 Unicode scalar values.");
            }
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

    private static ulong RequireJsonPositiveUInt64(JsonElement element, string context)
    {
        var value = RequireJsonUInt64(element, context);
        if (value == 0)
        {
            throw new JsonException($"{context} must be at least 1.");
        }
        return value;
    }

    private static byte[] RequireJsonFixed32Bytes(JsonElement element, string context)
    {
        if (element.ValueKind != JsonValueKind.Array || element.GetArrayLength() != 32)
        {
            throw new JsonException($"{context} must contain exactly 32 bytes.");
        }

        var bytes = new byte[32];
        var index = 0;
        foreach (var item in element.EnumerateArray())
        {
            if (item.ValueKind != JsonValueKind.Number || !item.TryGetByte(out bytes[index]))
            {
                throw new JsonException($"{context} must contain exactly 32 bytes.");
            }
            index++;
        }
        return bytes;
    }

    private static byte[] RequireJsonNonZeroFixed32Bytes(JsonElement element, string context)
    {
        var bytes = RequireJsonFixed32Bytes(element, context);
        if (bytes.All(static value => value == 0))
        {
            throw new JsonException($"{context} must not be all zeroes.");
        }
        return bytes;
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
