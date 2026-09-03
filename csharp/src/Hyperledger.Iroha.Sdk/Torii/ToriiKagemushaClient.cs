using System.Net;
using System.Text.Json;
using Hyperledger.Iroha.Kagemusha;
using Hyperledger.Iroha.Transactions;
using KagemushaCodec = Hyperledger.Iroha.Kagemusha.Kagemusha;

namespace Hyperledger.Iroha.Torii;

public sealed partial class ToriiClient
{
    private const int KagemushaReadinessMaximumBytes = 4 * 1024;
    private const int KagemushaOperationStatusMaximumJsonBytes = 16 * 1024 * 1024;

    /// <summary>Reads the generic first-release KAGEMUSHA capability.</summary>
    public async Task<ToriiKagemushaReadinessV1> GetKagemushaReadinessAsync(
        CancellationToken cancellationToken = default)
    {
        using var response = await SendAsync(
            HttpMethod.Get,
            "/v1/kagemusha/readiness",
            query: null,
            content: null,
            accept: "application/json",
            cancellationToken: cancellationToken);
        if (response.StatusCode != HttpStatusCode.OK)
        {
            throw new InvalidDataException(
                $"KAGEMUSHA readiness expected HTTP 200, got {(int)response.StatusCode}.");
        }
        if (!string.Equals(
                response.Content.Headers.ContentType?.MediaType,
                "application/json",
                StringComparison.OrdinalIgnoreCase))
        {
            throw new InvalidDataException(
                "KAGEMUSHA readiness response must use Content-Type application/json.");
        }

        var body = await ReadBoundedKagemushaBodyAsync(
            response.Content,
            cancellationToken);
        await using var stream = new MemoryStream(body, writable: false);
        using var document = await ParseJsonDocumentRejectingDuplicatePropertiesAsync(
            stream,
            "KAGEMUSHA readiness response",
            cancellationToken);
        var root = document.RootElement;
        RequireKagemushaFields(
            root,
            "kagemusha_handoff_capability",
            "wire_version",
            "device_lifecycle_version",
            "ready");
        var capability = root.Deserialize<ToriiKagemushaReadinessV1>(SerializerOptions)
            ?? throw new JsonException("KAGEMUSHA readiness response deserialized to null.");
        if (!string.Equals(
                capability.KagemushaHandoffCapability,
                "kagemusha_handoff_v1",
                StringComparison.Ordinal)
            || capability.WireVersion != 1
            || capability.DeviceLifecycleVersion != 1)
        {
            throw new JsonException(
                "KAGEMUSHA readiness must advertise kagemusha_handoff_v1 wire/device lifecycle version 1.");
        }
        return capability;
    }

    /// <summary>Submits one canonical payer-signed KAGEMUSHA top-up transaction.</summary>
    public Task<UnverifiedKagemushaOperationStatusV1> SubmitKagemushaTopUpAsync(
        SignedTransactionEnvelope transaction,
        ReadOnlyMemory<byte> operationId,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(transaction);
        return SubmitKagemushaOperationAsync(
            "/v1/kagemusha/top-up",
            "top_up",
            operationId,
            transaction.VersionedNoritoBytes,
            cancellationToken);
    }

    /// <summary>Submits one canonical KAGEMUSHA V1 full or partial redemption intent.</summary>
    public Task<UnverifiedKagemushaOperationStatusV1> SubmitKagemushaRedemptionAsync(
        KagemushaRedemptionRequestV1 request,
        CancellationToken cancellationToken = default) =>
        SubmitKagemushaOperationAsync(
            "/v1/kagemusha/redeem",
            "redemption",
            request.OperationId,
            KagemushaCodec.EncodeRedemptionRequest(request),
            cancellationToken);

    /// <summary>Reads one operation without exposing an unverified monetary result.</summary>
    public async Task<UnverifiedKagemushaOperationStatusV1> GetKagemushaOperationAsync(
        ReadOnlyMemory<byte> operationId,
        CancellationToken cancellationToken = default)
    {
        var expectedId = KagemushaOperationIdHex(operationId);
        using var response = await SendAsync(
            HttpMethod.Get,
            $"/v1/kagemusha/operations/{expectedId}",
            query: null,
            content: null,
            accept: "application/json",
            cancellationToken: cancellationToken);
        if (response.StatusCode != HttpStatusCode.OK)
        {
            throw new InvalidDataException(
                $"KAGEMUSHA operation status expected HTTP 200, got {(int)response.StatusCode}.");
        }
        var status = await ReadKagemushaOperationStatusAsync(response, cancellationToken);
        if (!status.OperationId.Span.SequenceEqual(operationId.Span))
            throw new InvalidDataException("KAGEMUSHA V1 response operation ID does not match the requested resource.");
        return status;
    }

    private async Task<UnverifiedKagemushaOperationStatusV1> SubmitKagemushaOperationAsync(
        string path,
        string expectedKind,
        ReadOnlyMemory<byte> operationId,
        ReadOnlyMemory<byte> body,
        CancellationToken cancellationToken)
    {
        var expectedId = KagemushaOperationIdHex(operationId);
        using var content = CreateBinaryContent(body, "application/x-norito");
        using var response = await SendAsync(
            HttpMethod.Post,
            path,
            query: null,
            content,
            accept: "application/json",
            configureRequest: request => request.Headers.TryAddWithoutValidation("Idempotency-Key", expectedId),
            cancellationToken: cancellationToken);
        if (response.StatusCode is not HttpStatusCode.OK and not HttpStatusCode.Accepted)
        {
            throw new InvalidDataException(
                $"KAGEMUSHA operation submission expected HTTP 200 or 202, got {(int)response.StatusCode}.");
        }
        var status = await ReadKagemushaOperationStatusAsync(response, cancellationToken);
        if (!status.OperationId.Span.SequenceEqual(operationId.Span)
            || !string.Equals(status.Kind, expectedKind, StringComparison.Ordinal))
            throw new InvalidDataException("KAGEMUSHA V1 response does not match the submitted operation.");
        ValidateKagemushaSubmissionResponse(response, status, expectedId);
        return status;
    }

    private static void ValidateKagemushaSubmissionResponse(
        HttpResponseMessage response,
        UnverifiedKagemushaOperationStatusV1 status,
        string operationId)
    {
        var expectedLocation = $"/v1/kagemusha/operations/{operationId}";
        if (!response.Headers.TryGetValues("Location", out var locationValues)
            || locationValues is null
            || !locationValues.SequenceEqual([expectedLocation], StringComparer.Ordinal))
        {
            throw new InvalidDataException(
                "KAGEMUSHA operation response has an invalid Location header.");
        }

        var hasRetryAfter = response.Headers.TryGetValues("Retry-After", out var retryValues);
        if (response.StatusCode == HttpStatusCode.Accepted)
        {
            if (!string.Equals(status.State, "pending", StringComparison.Ordinal)
                || !hasRetryAfter
                || retryValues is null
                || retryValues.Count() != 1
                || !ulong.TryParse(retryValues.Single(), out var seconds)
                || seconds == 0)
            {
                throw new InvalidDataException(
                    "Pending KAGEMUSHA operation response requires HTTP 202 and a positive Retry-After.");
            }
            return;
        }

        if ((status.State != "applied" && status.State != "rejected") || hasRetryAfter)
        {
            throw new InvalidDataException(
                "Terminal KAGEMUSHA operation response requires HTTP 200 without Retry-After.");
        }
    }

    private static async Task<UnverifiedKagemushaOperationStatusV1> ReadKagemushaOperationStatusAsync(
        HttpResponseMessage response,
        CancellationToken cancellationToken)
    {
        if (!string.Equals(response.Content.Headers.ContentType?.MediaType, "application/json", StringComparison.OrdinalIgnoreCase))
            throw new InvalidDataException("KAGEMUSHA V1 operation response must use Content-Type application/json.");
        var body = await ReadBoundedKagemushaBodyAsync(
            response.Content,
            cancellationToken,
            KagemushaOperationStatusMaximumJsonBytes);
        await using var stream = new MemoryStream(body, writable: false);
        using var document = await ParseJsonDocumentRejectingDuplicatePropertiesAsync(
            stream,
            "KAGEMUSHA V1 operation response",
            cancellationToken);
        return ParseKagemushaOperationStatus(document.RootElement);
    }

    private static UnverifiedKagemushaOperationStatusV1 ParseKagemushaOperationStatus(JsonElement root)
    {
        RequireKagemushaFields(root, "version", "operation_id", "kind", "state", "result", "rejection");
        if (!root.GetProperty("version").TryGetUInt16(out var version) || version != 1)
            throw new JsonException("KAGEMUSHA V1 operation status version must be 1.");
        var operationId = ReadKagemushaFixedBytes(root.GetProperty("operation_id"), "operation_id");
        var kind = ReadKagemushaTaggedUnit(root.GetProperty("kind"), "kind", "top_up", "redemption");
        var state = ReadKagemushaTaggedUnit(root.GetProperty("state"), "state", "pending", "applied", "rejected");
        var result = root.GetProperty("result");
        var rejectionValue = root.GetProperty("rejection");
        KagemushaOperationRejectionV1? rejection = null;
        if (state == "pending")
        {
            if (result.ValueKind != JsonValueKind.Null || rejectionValue.ValueKind != JsonValueKind.Null)
                throw new JsonException("Pending KAGEMUSHA V1 status cannot contain a result or rejection.");
        }
        else if (state == "applied")
        {
            if (result.ValueKind != JsonValueKind.Object || rejectionValue.ValueKind != JsonValueKind.Null)
                throw new JsonException("Applied KAGEMUSHA V1 status has an invalid terminal envelope.");
        }
        else
        {
            if (result.ValueKind != JsonValueKind.Null)
                throw new JsonException("Rejected KAGEMUSHA V1 status cannot contain a result.");
            RequireKagemushaFields(rejectionValue, "code", "detail_digest");
            var code = ReadKagemushaTaggedUnit(
                rejectionValue.GetProperty("code"),
                "code",
                "invalid_request", "unauthorized", "insufficient_online_balance", "invalid_proof",
                "hardware_policy_rejected", "identity_conflict", "reserve_underflow", "arithmetic_overflow",
                "internal_failure");
            rejection = new KagemushaOperationRejectionV1(
                code,
                ReadKagemushaFixedBytes(rejectionValue.GetProperty("detail_digest"), "detail_digest"));
        }
        return new UnverifiedKagemushaOperationStatusV1(operationId, kind, state, rejection, root);
    }

    private static string ReadKagemushaTaggedUnit(JsonElement element, string tag, params string[] allowed)
    {
        RequireKagemushaFields(element, tag, "value");
        var value = element.GetProperty(tag);
        if (value.ValueKind != JsonValueKind.String
            || element.GetProperty("value").ValueKind != JsonValueKind.Null)
            throw new JsonException($"KAGEMUSHA V1 {tag} enum is invalid.");
        var text = value.GetString()!;
        if (!allowed.Contains(text, StringComparer.Ordinal))
            throw new JsonException($"KAGEMUSHA V1 {tag} enum is unsupported.");
        return text;
    }

    private static byte[] ReadKagemushaFixedBytes(JsonElement element, string field)
    {
        if (element.ValueKind != JsonValueKind.Array || element.GetArrayLength() != 32)
            throw new JsonException($"KAGEMUSHA V1 {field} must be one 32-byte array.");
        var bytes = new byte[32];
        var index = 0;
        foreach (var value in element.EnumerateArray())
        {
            if (!value.TryGetByte(out bytes[index++]))
                throw new JsonException($"KAGEMUSHA V1 {field} contains a non-byte value.");
        }
        if (bytes.All(static value => value == 0))
            throw new JsonException($"KAGEMUSHA V1 {field} cannot be zero.");
        return bytes;
    }

    private static string KagemushaOperationIdHex(ReadOnlyMemory<byte> operationId)
    {
        if (operationId.Length != 32 || operationId.Span.IndexOfAnyExcept((byte)0) < 0)
            throw new ArgumentException("KAGEMUSHA V1 operation ID must be one nonzero 32-byte value.", nameof(operationId));
        return Convert.ToHexString(operationId.Span).ToLowerInvariant();
    }

    private static async Task<byte[]> ReadBoundedKagemushaBodyAsync(
        HttpContent content,
        CancellationToken cancellationToken,
        int maximumBytes = KagemushaReadinessMaximumBytes)
    {
        if (content.Headers.ContentLength is long contentLength && contentLength > maximumBytes)
        {
            throw new InvalidDataException(
                $"KAGEMUSHA V1 response exceeds {maximumBytes} bytes.");
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
                    $"KAGEMUSHA V1 response exceeds {maximumBytes} bytes.");
            }
            await output.WriteAsync(buffer.AsMemory(0, read), cancellationToken);
        }
    }

    private static void RequireKagemushaFields(
        JsonElement element,
        params string[] expected)
    {
        if (element.ValueKind != JsonValueKind.Object)
        {
            throw new JsonException("KAGEMUSHA readiness response must be an object.");
        }
        var fields = element.EnumerateObject().Select(property => property.Name).ToArray();
        if (fields.Length != expected.Length
            || fields.Any(name => !expected.Contains(name, StringComparer.Ordinal)))
        {
            throw new JsonException(
                "KAGEMUSHA readiness response contains missing or unknown fields.");
        }
    }
}
