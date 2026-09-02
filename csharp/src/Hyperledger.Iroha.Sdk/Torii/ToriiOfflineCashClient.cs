using System.Net;
using System.Text.Json;
using Hyperledger.Iroha.OfflineCash;

namespace Hyperledger.Iroha.Torii;

public sealed partial class ToriiClient
{
    private const int OfflineCashReadinessMaximumBytes = 4 * 1024;
    private const int OfflineCashOperationStatusMaximumJsonBytes = 16 * 1024 * 1024;

    /// <summary>Reads the generic first-release Offline Cash capability.</summary>
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
        if (response.StatusCode != HttpStatusCode.OK)
        {
            throw new InvalidDataException(
                $"Offline capability expected HTTP 200, got {(int)response.StatusCode}.");
        }
        if (!string.Equals(
                response.Content.Headers.ContentType?.MediaType,
                "application/json",
                StringComparison.OrdinalIgnoreCase))
        {
            throw new InvalidDataException(
                "Offline capability response must use Content-Type application/json.");
        }

        var body = await ReadBoundedOfflineCashBodyAsync(
            response.Content,
            cancellationToken);
        await using var stream = new MemoryStream(body, writable: false);
        using var document = await ParseJsonDocumentRejectingDuplicatePropertiesAsync(
            stream,
            "Offline capability response",
            cancellationToken);
        var root = document.RootElement;
        RequireOfflineCashFields(
            root,
            "cash_handoff_capability",
            "wire_version",
            "device_lifecycle_version",
            "ready");
        var capability = root.Deserialize<ToriiOfflineStatus>(SerializerOptions)
            ?? throw new JsonException("Offline capability response deserialized to null.");
        if (!string.Equals(
                capability.CashHandoffCapability,
                "cash_handoff_v1",
                StringComparison.Ordinal)
            || capability.WireVersion != 1
            || capability.DeviceLifecycleVersion != 1
            || !capability.Ready)
        {
            throw new JsonException(
                "Offline capability must advertise ready cash_handoff_v1 wire/device lifecycle version 1.");
        }
        return capability;
    }

    /// <summary>Submits one canonical Offline Cash V1 top-up intent.</summary>
    public Task<UnverifiedOfflineCashOperationStatusV1> SubmitOfflineCashTopUpAsync(
        OfflineCashTopUpRequestV1 request,
        CancellationToken cancellationToken = default) =>
        SubmitOfflineCashOperationAsync(
            "/v1/offline/top-up",
            "top_up",
            request.OperationId,
            OfflineCashV1.EncodeTopUpRequest(request),
            cancellationToken);

    /// <summary>Submits one canonical Offline Cash V1 full or partial redemption intent.</summary>
    public Task<UnverifiedOfflineCashOperationStatusV1> SubmitOfflineCashRedemptionAsync(
        OfflineCashRedemptionRequestV1 request,
        CancellationToken cancellationToken = default) =>
        SubmitOfflineCashOperationAsync(
            "/v1/offline/redeem",
            "redemption",
            request.OperationId,
            OfflineCashV1.EncodeRedemptionRequest(request),
            cancellationToken);

    /// <summary>Reads one operation without exposing an unverified monetary result.</summary>
    public async Task<UnverifiedOfflineCashOperationStatusV1> GetOfflineCashOperationAsync(
        ReadOnlyMemory<byte> operationId,
        CancellationToken cancellationToken = default)
    {
        var expectedId = OfflineCashOperationIdHex(operationId);
        using var response = await SendAsync(
            HttpMethod.Get,
            $"/v1/offline/operations/{expectedId}",
            query: null,
            content: null,
            accept: "application/json",
            cancellationToken: cancellationToken);
        var status = await ReadOfflineCashOperationStatusAsync(response, cancellationToken);
        if (!status.OperationId.Span.SequenceEqual(operationId.Span))
            throw new InvalidDataException("Offline Cash V1 response operation ID does not match the requested resource.");
        return status;
    }

    private async Task<UnverifiedOfflineCashOperationStatusV1> SubmitOfflineCashOperationAsync(
        string path,
        string expectedKind,
        ReadOnlyMemory<byte> operationId,
        ReadOnlyMemory<byte> body,
        CancellationToken cancellationToken)
    {
        var expectedId = OfflineCashOperationIdHex(operationId);
        using var content = CreateBinaryContent(body, "application/x-norito");
        using var response = await SendAsync(
            HttpMethod.Post,
            path,
            query: null,
            content,
            accept: "application/json",
            configureRequest: request => request.Headers.TryAddWithoutValidation("Idempotency-Key", expectedId),
            cancellationToken: cancellationToken);
        var status = await ReadOfflineCashOperationStatusAsync(response, cancellationToken);
        if (!status.OperationId.Span.SequenceEqual(operationId.Span)
            || !string.Equals(status.Kind, expectedKind, StringComparison.Ordinal))
            throw new InvalidDataException("Offline Cash V1 response does not match the submitted operation.");
        return status;
    }

    private static async Task<UnverifiedOfflineCashOperationStatusV1> ReadOfflineCashOperationStatusAsync(
        HttpResponseMessage response,
        CancellationToken cancellationToken)
    {
        if (!string.Equals(response.Content.Headers.ContentType?.MediaType, "application/json", StringComparison.OrdinalIgnoreCase))
            throw new InvalidDataException("Offline Cash V1 operation response must use Content-Type application/json.");
        var body = await ReadBoundedOfflineCashBodyAsync(
            response.Content,
            cancellationToken,
            OfflineCashOperationStatusMaximumJsonBytes);
        await using var stream = new MemoryStream(body, writable: false);
        using var document = await ParseJsonDocumentRejectingDuplicatePropertiesAsync(
            stream,
            "Offline Cash V1 operation response",
            cancellationToken);
        return ParseOfflineCashOperationStatus(document.RootElement);
    }

    private static UnverifiedOfflineCashOperationStatusV1 ParseOfflineCashOperationStatus(JsonElement root)
    {
        RequireOfflineCashFields(root, "version", "operation_id", "kind", "state", "result", "rejection");
        if (!root.GetProperty("version").TryGetUInt16(out var version) || version != 1)
            throw new JsonException("Offline Cash V1 operation status version must be 1.");
        var operationId = ReadOfflineCashFixedBytes(root.GetProperty("operation_id"), "operation_id");
        var kind = ReadOfflineCashTaggedUnit(root.GetProperty("kind"), "kind", "top_up", "redemption");
        var state = ReadOfflineCashTaggedUnit(root.GetProperty("state"), "state", "pending", "applied", "rejected");
        var result = root.GetProperty("result");
        var rejectionValue = root.GetProperty("rejection");
        OfflineCashOperationRejectionV1? rejection = null;
        if (state == "pending")
        {
            if (result.ValueKind != JsonValueKind.Null || rejectionValue.ValueKind != JsonValueKind.Null)
                throw new JsonException("Pending Offline Cash V1 status cannot contain a result or rejection.");
        }
        else if (state == "applied")
        {
            if (result.ValueKind != JsonValueKind.Object || rejectionValue.ValueKind != JsonValueKind.Null)
                throw new JsonException("Applied Offline Cash V1 status has an invalid terminal envelope.");
        }
        else
        {
            if (result.ValueKind != JsonValueKind.Null)
                throw new JsonException("Rejected Offline Cash V1 status cannot contain a result.");
            RequireOfflineCashFields(rejectionValue, "code", "detail_digest");
            var code = ReadOfflineCashTaggedUnit(
                rejectionValue.GetProperty("code"),
                "code",
                "invalid_request", "unauthorized", "insufficient_online_balance", "invalid_proof",
                "hardware_policy_rejected", "identity_conflict", "reserve_underflow", "arithmetic_overflow",
                "internal_failure");
            rejection = new OfflineCashOperationRejectionV1(
                code,
                ReadOfflineCashFixedBytes(rejectionValue.GetProperty("detail_digest"), "detail_digest"));
        }
        return new UnverifiedOfflineCashOperationStatusV1(operationId, kind, state, rejection, root);
    }

    private static string ReadOfflineCashTaggedUnit(JsonElement element, string tag, params string[] allowed)
    {
        RequireOfflineCashFields(element, tag, "value");
        var value = element.GetProperty(tag);
        if (value.ValueKind != JsonValueKind.String
            || element.GetProperty("value").ValueKind != JsonValueKind.Null)
            throw new JsonException($"Offline Cash V1 {tag} enum is invalid.");
        var text = value.GetString()!;
        if (!allowed.Contains(text, StringComparer.Ordinal))
            throw new JsonException($"Offline Cash V1 {tag} enum is unsupported.");
        return text;
    }

    private static byte[] ReadOfflineCashFixedBytes(JsonElement element, string field)
    {
        if (element.ValueKind != JsonValueKind.Array || element.GetArrayLength() != 32)
            throw new JsonException($"Offline Cash V1 {field} must be one 32-byte array.");
        var bytes = new byte[32];
        var index = 0;
        foreach (var value in element.EnumerateArray())
        {
            if (!value.TryGetByte(out bytes[index++]))
                throw new JsonException($"Offline Cash V1 {field} contains a non-byte value.");
        }
        if (bytes.All(static value => value == 0))
            throw new JsonException($"Offline Cash V1 {field} cannot be zero.");
        return bytes;
    }

    private static string OfflineCashOperationIdHex(ReadOnlyMemory<byte> operationId)
    {
        if (operationId.Length != 32 || operationId.Span.IndexOfAnyExcept((byte)0) < 0)
            throw new ArgumentException("Offline Cash V1 operation ID must be one nonzero 32-byte value.", nameof(operationId));
        return Convert.ToHexString(operationId.Span).ToLowerInvariant();
    }

    private static async Task<byte[]> ReadBoundedOfflineCashBodyAsync(
        HttpContent content,
        CancellationToken cancellationToken,
        int maximumBytes = OfflineCashReadinessMaximumBytes)
    {
        if (content.Headers.ContentLength is long contentLength && contentLength > maximumBytes)
        {
            throw new InvalidDataException(
                $"Offline Cash V1 response exceeds {maximumBytes} bytes.");
        }
        await using var input = await content.ReadAsStreamAsync(cancellationToken);
        using var output = new MemoryStream();
        var buffer = new byte[1024];
        while (true)
        {
            var read = await input.ReadAsync(buffer.AsMemory(), cancellationToken);
            if (read == 0)
            {
                return output.ToArray();
            }
            if (output.Length > maximumBytes - read)
            {
                throw new InvalidDataException(
                    $"Offline Cash V1 response exceeds {maximumBytes} bytes.");
            }
            await output.WriteAsync(buffer.AsMemory(0, read), cancellationToken);
        }
    }

    private static void RequireOfflineCashFields(
        JsonElement element,
        params string[] expected)
    {
        if (element.ValueKind != JsonValueKind.Object)
        {
            throw new JsonException("Offline capability response must be an object.");
        }
        var fields = element.EnumerateObject().Select(property => property.Name).ToArray();
        if (fields.Length != expected.Length
            || fields.Any(name => !expected.Contains(name, StringComparer.Ordinal)))
        {
            throw new JsonException(
                "Offline capability response contains missing or unknown fields.");
        }
    }
}
