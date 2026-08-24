using System.Net;
using System.Net.Http.Headers;
using System.Text.Json;

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
            ToriiKagemushaOperationKind.Redeem,
            cancellationToken);
    }

    public async Task<ToriiKagemushaOperationStatus> GetKagemushaOperationStatusAsync(
        string operationId,
        CancellationToken cancellationToken = default)
    {
        var canonicalId = ToriiKagemushaTransport.RequireOperationId(operationId, nameof(operationId));
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
        return ParseKagemushaOperationStatus(document.RootElement, canonicalId);
    }

    private async Task<ToriiKagemushaOperationReference> SubmitKagemushaV4Async(
        string path,
        byte[] norito,
        string operationId,
        ToriiKagemushaOperationKind expectedKind,
        CancellationToken cancellationToken)
    {
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

        using var document = await ReadKagemushaJsonAsync(
            response,
            "Kagemusha operation reference response",
            cancellationToken);
        var location = response.Headers.Location?.OriginalString;
        return ParseKagemushaOperationReference(
            document.RootElement,
            operationId,
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
            "cash_handoff_capability",
            "required_bridge_abi_version",
            "max_hops",
            "ready");
        var capability = root.Deserialize<ToriiOfflineStatus>(serializerOptions)
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
        string expectedOperationId,
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
        var expectedUri = $"/v1/offline/operations/{expectedOperationId}";
        if (!string.Equals(operationId, expectedOperationId, StringComparison.Ordinal)
            || kind != expectedKind
            || state != ToriiKagemushaOperationState.Pending
            || !string.Equals(statusUri, expectedUri, StringComparison.Ordinal)
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
            SubmittedAtMilliseconds = RequireJsonUInt64(root.GetProperty("submitted_at_ms"), "submitted_at_ms"),
        };
    }

    private static ToriiKagemushaOperationStatus ParseKagemushaOperationStatus(
        JsonElement root,
        string expectedOperationId)
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
            "applied" => ParseAppliedStatus(value, operationId),
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
            SubmittedAtMilliseconds = RequireJsonUInt64(value.GetProperty("submitted_at_ms"), "value.submitted_at_ms"),
        };
    }

    private static ToriiKagemushaOperationStatus ParseAppliedStatus(JsonElement value, string operationId)
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
            return new ToriiKagemushaOperationStatus
            {
                OperationId = operationId,
                State = ToriiKagemushaOperationState.Applied,
                Kind = ToriiKagemushaOperationKind.Redeem,
                RedeemResult = new ToriiKagemushaRedeemResultV4
                {
                    TransactionHash = RequireJsonHash(result.GetProperty("transaction_hash"), "result.transaction_hash"),
                    FinalizedBlockHeight = RequireJsonUInt64(result.GetProperty("finalized_block_height"), "result.finalized_block_height"),
                    ServerTimeMilliseconds = RequireJsonUInt64(result.GetProperty("server_time_ms"), "result.server_time_ms"),
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
        var anchor = result.GetProperty("anchor");
        if (RequireJsonUInt64(anchor.GetProperty("version"), "result.anchor.version") != KagemushaManifestVersion)
        {
            throw new JsonException("Kagemusha top-up anchor must use V4.");
        }
        var artifactBinding = anchor.GetProperty("artifact_binding");
        if (RequireJsonUInt64(artifactBinding.GetProperty("version"), "result.anchor.artifact_binding.version")
            != KagemushaManifestVersion)
        {
            throw new JsonException("Kagemusha top-up artifact binding must use V4.");
        }

        return new ToriiKagemushaOperationStatus
        {
            OperationId = operationId,
            State = ToriiKagemushaOperationState.Applied,
            Kind = ToriiKagemushaOperationKind.TopUp,
            TopUpResult = new ToriiKagemushaTopUpResultV4
            {
                TransactionHash = RequireJsonHash(result.GetProperty("transaction_hash"), "result.transaction_hash"),
                FinalizedBlockHeight = RequireJsonUInt64(result.GetProperty("finalized_block_height"), "result.finalized_block_height"),
                ServerTimeMilliseconds = RequireJsonUInt64(result.GetProperty("server_time_ms"), "result.server_time_ms"),
                Anchor = anchor.Clone(),
                FinalityProof = result.GetProperty("finality_proof").Clone(),
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
        var details = error.TryGetProperty("details", out var detailValue) && detailValue.ValueKind != JsonValueKind.Null
            ? detailValue.Clone()
            : (JsonElement?)null;
        return new ToriiKagemushaOperationStatus
        {
            OperationId = operationId,
            State = ToriiKagemushaOperationState.Rejected,
            Kind = ParseTaggedKind(value.GetProperty("kind"), "value.kind"),
            TransactionHash = RequireJsonHash(value.GetProperty("transaction_hash"), "value.transaction_hash"),
            Error = new ToriiKagemushaOperationError
            {
                Code = RequireJsonString(error.GetProperty("code"), "value.error.code"),
                Message = RequireJsonString(error.GetProperty("message"), "value.error.message"),
                Details = details,
            },
        };
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

    private static string RequireJsonHash(JsonElement element, string context)
    {
        var value = RequireJsonString(element, context);
        RequireLowerHex32(value, context);
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
