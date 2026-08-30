using System.Globalization;
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
        RequireKagemushaStatus(response, HttpStatusCode.OK, "Kagemusha operation status");
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
                message.Headers.TryAddWithoutValidation("Idempotency-Key", operationId);
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

        var body = await ReadBoundedKagemushaJsonBodyAsync(
            response.Content,
            context,
            cancellationToken);
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
        CancellationToken cancellationToken)
    {
        var declaredLength = content.Headers.ContentLength;
        if (declaredLength is > ToriiKagemushaTransport.MaxJsonResponseBytes)
        {
            throw new InvalidDataException(
                $"{context} exceeds the {ToriiKagemushaTransport.MaxJsonResponseBytes}-byte limit.");
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
            if (output.Length > ToriiKagemushaTransport.MaxJsonResponseBytes - read)
            {
                throw new InvalidDataException(
                    $"{context} exceeds the {ToriiKagemushaTransport.MaxJsonResponseBytes}-byte limit.");
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
            SubmittedAtMilliseconds = RequireJsonPositiveUInt64(root.GetProperty("submitted_at_ms"), "submitted_at_ms"),
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
            SubmittedAtMilliseconds = RequireJsonPositiveUInt64(value.GetProperty("submitted_at_ms"), "value.submitted_at_ms"),
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
                    FinalizedBlockHeight = RequireJsonPositiveUInt64(result.GetProperty("finalized_block_height"), "result.finalized_block_height"),
                    ServerTimeMilliseconds = RequireJsonPositiveUInt64(result.GetProperty("server_time_ms"), "result.server_time_ms"),
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
        RequireJsonFixedBytes32MatchesOperationId(
            RequireJsonProperty(anchor, "topup_operation_id", "result.anchor"),
            operationId,
            "result.anchor.topup_operation_id");

        var finalityProof = RequireJsonObject(
            result.GetProperty("finality_proof"),
            "result.finality_proof");
        if (RequireJsonUInt64(
                RequireJsonProperty(finalityProof, "version", "result.finality_proof"),
                "result.finality_proof.version") != 1)
        {
            throw new JsonException("Kagemusha top-up finality proof must use V1.");
        }
        var finalityAnchor = RequireJsonObject(
            RequireJsonProperty(finalityProof, "anchor", "result.finality_proof"),
            "result.finality_proof.anchor");
        RequireJsonFixedBytes32MatchesOperationId(
            RequireJsonProperty(
                finalityAnchor,
                "topup_operation_id",
                "result.finality_proof.anchor"),
            operationId,
            "result.finality_proof.anchor.topup_operation_id");

        return new ToriiKagemushaOperationStatus
        {
            OperationId = operationId,
            State = ToriiKagemushaOperationState.Applied,
            Kind = ToriiKagemushaOperationKind.TopUp,
            TopUpResult = new ToriiKagemushaTopUpResultV4
            {
                TransactionHash = RequireJsonHash(result.GetProperty("transaction_hash"), "result.transaction_hash"),
                FinalizedBlockHeight = RequireJsonPositiveUInt64(result.GetProperty("finalized_block_height"), "result.finalized_block_height"),
                ServerTimeMilliseconds = RequireJsonPositiveUInt64(result.GetProperty("server_time_ms"), "result.server_time_ms"),
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
            OperationId = operationId,
            State = ToriiKagemushaOperationState.Rejected,
            Kind = ParseTaggedKind(value.GetProperty("kind"), "value.kind"),
            TransactionHash = RequireJsonHash(value.GetProperty("transaction_hash"), "value.transaction_hash"),
            Error = new ToriiKagemushaOperationError
            {
                Code = RequireJsonErrorCode(error.GetProperty("code"), "value.error.code"),
                Message = RequireJsonExactText(error.GetProperty("message"), "value.error.message"),
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

    private static void RequireJsonFixedBytes32MatchesOperationId(
        JsonElement element,
        string operationId,
        string context)
    {
        if (element.ValueKind != JsonValueKind.Array || element.GetArrayLength() != 32)
        {
            throw new JsonException($"{context} must be an array of 32 bytes.");
        }

        var index = 0;
        foreach (var item in element.EnumerateArray())
        {
            if (!item.TryGetInt32(out var value) || value is < 0 or > byte.MaxValue)
            {
                throw new JsonException($"{context} must be an array of 32 bytes.");
            }
            var expected = byte.Parse(
                operationId.AsSpan(index * 2, 2),
                NumberStyles.AllowHexSpecifier,
                CultureInfo.InvariantCulture);
            if (value != expected)
            {
                throw new JsonException($"{context} does not match the requested operation id.");
            }
            index++;
        }
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
