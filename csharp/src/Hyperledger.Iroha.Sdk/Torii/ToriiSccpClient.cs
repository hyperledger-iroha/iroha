using System.Buffers;
using System.Net;
using System.Net.Http.Headers;
using System.Text;
using System.Text.Json;
using Hyperledger.Iroha.Sccp;

namespace Hyperledger.Iroha.Torii;

public sealed partial class ToriiClient
{
    private const int SccpCapabilitiesResponseMaximumBytes = 64 * 1024;
    private const int SccpRecentResponseMaximumBytes = 8 * 1024 * 1024;
    private const int SccpJsonResponseMaximumBytes = 64 * 1024 * 1024;

    /// <summary>Fetch consensus-derived SCCP capabilities.</summary>
    public async Task<SccpCapabilities> GetSccpCapabilitiesAsync(
        CancellationToken cancellationToken = default)
    {
        var bytes = await GetExactSccpJsonAsync(
            "/v1/sccp/capabilities",
            query: null,
            "SCCP capabilities",
            SccpCapabilitiesResponseMaximumBytes,
            cancellationToken);
        return SccpCapabilities.Parse(bytes);
    }

    /// <summary>Fetch the authoritative typed SCCP route registry.</summary>
    public async Task<SccpRegistryV1> GetSccpRegistryAsync(
        CancellationToken cancellationToken = default)
    {
        var bytes = await GetExactSccpJsonAsync(
            "/v1/sccp/registry",
            query: null,
            "SCCP registry",
            SccpJsonResponseMaximumBytes,
            cancellationToken);
        return SccpRegistryV1.Parse(bytes);
    }

    /// <summary>Fetch the authoritative SCCP registry as canonical Norito bytes.</summary>
    public Task<byte[]> GetSccpRegistryNoritoAsync(CancellationToken cancellationToken = default) =>
        GetExactSccpNoritoAsync(
            "/v1/sccp/registry",
            "SCCP registry",
            SccpSubmitValidation.MaximumNativeArtifactBytes,
            cancellationToken);

    /// <summary>Fetch one finalized SORA-origin SCCP message bundle.</summary>
    public async Task<SccpMessageBundleV1> GetSccpMessageBundleAsync(
        string messageIdHex,
        CancellationToken cancellationToken = default)
    {
        var id = ExactSccpMessageId(messageIdHex);
        var bytes = await GetExactSccpJsonAsync(
            $"/v1/sccp/proofs/message/{id}",
            query: null,
            "SCCP message bundle",
            SccpJsonResponseMaximumBytes,
            cancellationToken);
        var bundle = SccpMessageBundleV1.Parse(bytes);
        if (bundle.MessageId != "0x" + id)
        {
            throw new InvalidDataException("SCCP bundle message id does not match the requested id.");
        }

        return bundle;
    }

    /// <summary>Fetch one finalized SCCP message bundle as canonical Norito bytes.</summary>
    public Task<byte[]> GetSccpMessageBundleNoritoAsync(
        string messageIdHex,
        CancellationToken cancellationToken = default)
    {
        var id = ExactSccpMessageId(messageIdHex);
        return GetExactSccpNoritoAsync(
            $"/v1/sccp/proofs/message/{id}",
            "SCCP message bundle",
            SccpSubmitValidation.MaximumNativeArtifactBytes,
            cancellationToken);
    }

    /// <summary>Fetch the query-free state-derived Groth16 request for one message.</summary>
    public async Task<SccpGroth16ProofRequestV1> GetSccpProofRequestAsync(
        string messageIdHex,
        CancellationToken cancellationToken = default)
    {
        var id = ExactSccpMessageId(messageIdHex);
        var bytes = await GetExactSccpJsonAsync(
            $"/v1/sccp/proof-requests/{id}",
            query: null,
            "SCCP proof request",
            SccpJsonResponseMaximumBytes,
            cancellationToken);
        var request = SccpGroth16ProofRequestV1.Parse(bytes);
        if (request.MessageId != "0x" + id)
        {
            throw new InvalidDataException("SCCP proof request message id does not match the requested id.");
        }

        return request;
    }

    /// <summary>Fetch one state-derived Groth16 request as canonical Norito bytes.</summary>
    public Task<byte[]> GetSccpProofRequestNoritoAsync(
        string messageIdHex,
        CancellationToken cancellationToken = default)
    {
        var id = ExactSccpMessageId(messageIdHex);
        return GetExactSccpNoritoAsync(
            $"/v1/sccp/proof-requests/{id}",
            "SCCP proof request",
            SccpSubmitValidation.MaximumDestinationArtifactBytes,
            cancellationToken);
    }

    /// <summary>Fetch newest-first committed outbound SCCP messages.</summary>
    public async Task<SccpRecentMessages> GetSccpRecentMessagesAsync(
        ulong? from = null,
        uint? afterIndex = null,
        uint? limit = null,
        CancellationToken cancellationToken = default)
    {
        if (from == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(from), "from must be positive when provided.");
        }

        if (limit is 0 or > 50)
        {
            throw new ArgumentOutOfRangeException(nameof(limit), "limit must be in 1..50.");
        }

        if (afterIndex is not null && from is null)
        {
            throw new ArgumentException("afterIndex requires from.", nameof(afterIndex));
        }

        if (afterIndex is > 511)
        {
            throw new ArgumentOutOfRangeException(nameof(afterIndex), "afterIndex must be in 0..511.");
        }

        var fields = new List<string>();
        if (from is { } fromValue)
        {
            fields.Add($"from={fromValue.ToString(System.Globalization.CultureInfo.InvariantCulture)}");
        }

        if (afterIndex is { } afterIndexValue)
        {
            fields.Add(
                $"after_index={afterIndexValue.ToString(System.Globalization.CultureInfo.InvariantCulture)}");
        }

        if (limit is { } limitValue)
        {
            fields.Add($"limit={limitValue.ToString(System.Globalization.CultureInfo.InvariantCulture)}");
        }

        var bytes = await GetExactSccpJsonAsync(
            "/v1/sccp/messages/recent",
            fields.Count == 0 ? null : string.Join('&', fields),
            "SCCP recent messages",
            SccpRecentResponseMaximumBytes,
            cancellationToken);
        return SccpRecentMessages.Parse(bytes);
    }

    /// <summary>Submit one closed destination-proof artifact derived from authoritative route state.</summary>
    public Task<SccpBridgeSubmitResponse> SubmitSccpBridgeProofAsync(
        SccpBridgeProofSubmitRequest request,
        SccpBridgeResponseExpectation? expectation = null,
        CancellationToken cancellationToken = default) =>
        SubmitExactSccpAsync(
            "/v1/bridge/proofs/submit",
            request,
            expectation,
            "SCCP bridge proof submit",
            cancellationToken);

    /// <summary>Submit one protocol-native inbound SCCP proof.</summary>
    public Task<SccpBridgeSubmitResponse> SubmitSccpBridgeMessageAsync(
        SccpBridgeMessageSubmitRequest request,
        SccpBridgeResponseExpectation? expectation = null,
        CancellationToken cancellationToken = default) =>
        SubmitExactSccpAsync(
            "/v1/bridge/messages",
            request,
            expectation,
            "SCCP bridge message submit",
            cancellationToken);

    private async Task<SccpBridgeSubmitResponse> SubmitExactSccpAsync<TRequest>(
        string path,
        TRequest request,
        SccpBridgeResponseExpectation? expectation,
        string context,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(request);
        var body = JsonSerializer.SerializeToUtf8Bytes(request, serializerOptions);
        using var content = new ByteArrayContent(body);
        content.Headers.ContentType = new MediaTypeHeaderValue("application/json");
        using var response = await SendExactSccpAsync(
            HttpMethod.Post,
            path,
            content: content,
            accept: "application/json",
            maximumResponseBytes: SccpJsonResponseMaximumBytes,
            context: context,
            cancellationToken: cancellationToken);
        var bytes = await ReadExactSccpJsonAsync(
            response,
            context,
            SccpJsonResponseMaximumBytes,
            cancellationToken);
        var (authority, proof, transactionPayload, signature) = request switch
        {
            SccpBridgeProofSubmitRequest proofRequest =>
                (
                    proofRequest.Authority,
                    Convert.FromBase64String(proofRequest.DestinationProofBase64),
                    proofRequest.TransactionPayloadBase64,
                    proofRequest.SignatureBase64),
            SccpBridgeMessageSubmitRequest messageRequest =>
                (
                    messageRequest.Authority,
                    Convert.FromBase64String(messageRequest.NativeProofBase64),
                    messageRequest.TransactionPayloadBase64,
                    messageRequest.SignatureBase64),
            _ => throw new InvalidOperationException("Unknown SCCP submit request type."),
        };
        return SccpBridgeSubmitResponse.ParseForRequest(
            bytes,
            expectation,
            authority,
            proof,
            transactionPayload,
            signature);
    }

    private async Task<byte[]> GetExactSccpJsonAsync(
        string path,
        string? query,
        string context,
        int maximumBytes,
        CancellationToken cancellationToken)
    {
        using var response = await SendExactSccpAsync(
            HttpMethod.Get,
            path,
            query: query,
            accept: "application/json",
            maximumResponseBytes: maximumBytes,
            context: context,
            cancellationToken: cancellationToken);
        return await ReadExactSccpJsonAsync(response, context, maximumBytes, cancellationToken);
    }

    private async Task<HttpResponseMessage> SendExactSccpAsync(
        HttpMethod method,
        string path,
        int maximumResponseBytes,
        string context,
        string? query = null,
        HttpContent? content = null,
        string? accept = null,
        CancellationToken cancellationToken = default)
    {
        var request = await CreateRequestAsync(
            method,
            path,
            query,
            content,
            accept,
            configureRequest: null,
            cancellationToken: cancellationToken);
        var response = await HttpClient.SendAsync(
            request,
            HttpCompletionOption.ResponseHeadersRead,
            cancellationToken);
        if (response.StatusCode == HttpStatusCode.OK)
        {
            return response;
        }

        try
        {
            string? responseBody = null;
            if (response.Content is not null)
            {
                try
                {
                    var bytes = await ReadBoundedSccpBodyAsync(
                        response.Content,
                        maximumResponseBytes,
                        $"{context} error",
                        cancellationToken);
                    try
                    {
                        responseBody = StrictUtf8.GetString(bytes);
                    }
                    catch (DecoderFallbackException)
                    {
                        responseBody = InvalidUtf8ResponseBody;
                    }
                }
                catch (InvalidDataException)
                {
                    responseBody = "<response body exceeds or violates the SCCP response limit>";
                }
            }

            throw new ToriiApiException(
                response.StatusCode,
                response.RequestMessage?.RequestUri,
                responseBody,
                response.ReasonPhrase);
        }
        finally
        {
            response.Dispose();
        }
    }

    private async Task<byte[]> GetExactSccpNoritoAsync(
        string path,
        string context,
        int maximumBytes,
        CancellationToken cancellationToken)
    {
        using var response = await SendExactSccpAsync(
            HttpMethod.Get,
            path,
            query: null,
            accept: "application/x-norito",
            maximumResponseBytes: maximumBytes,
            context: context,
            cancellationToken: cancellationToken);
        var mediaType = response.Content.Headers.ContentType?.MediaType;
        if (!string.Equals(mediaType, "application/x-norito", StringComparison.OrdinalIgnoreCase))
        {
            throw new InvalidDataException($"{context} response must use application/x-norito.");
        }

        var bytes = await ReadBoundedSccpBodyAsync(
            response.Content,
            maximumBytes,
            context,
            cancellationToken);
        if (bytes.Length == 0 || bytes.Length > maximumBytes)
        {
            throw new InvalidDataException($"{context} response body has an invalid SCCP size.");
        }

        try
        {
            _ = SccpSubmitValidation.CanonicalNoritoBase64(
                Convert.ToBase64String(bytes),
                context,
                maximumBytes);
        }
        catch (ArgumentException error)
        {
            throw new InvalidDataException($"{context} response is not one canonical uncompressed Norito envelope.", error);
        }

        return bytes;
    }

    private static string ExactSccpMessageId(string value)
    {
        try
        {
            return SccpSubmitValidation.ResponseHash(value, "message_id");
        }
        catch (ArgumentException error)
        {
            throw new ArgumentException(
                "message_id must be canonical lowercase nonzero prefixless 32-byte hex.",
                nameof(value),
                error);
        }
    }

    private static async Task<byte[]> ReadExactSccpJsonAsync(
        HttpResponseMessage response,
        string context,
        int maximumBytes,
        CancellationToken cancellationToken)
    {
        var mediaType = response.Content.Headers.ContentType?.MediaType;
        if (!string.Equals(mediaType, "application/json", StringComparison.OrdinalIgnoreCase))
        {
            throw new InvalidDataException($"{context} response must use a JSON content type.");
        }

        var bytes = await ReadBoundedSccpBodyAsync(
            response.Content,
            maximumBytes,
            context,
            cancellationToken);
        if (bytes.Length == 0)
        {
            throw new InvalidDataException($"{context} response body must not be empty.");
        }

        try
        {
            _ = new UTF8Encoding(false, true).GetString(bytes);
        }
        catch (DecoderFallbackException error)
        {
            throw new InvalidDataException($"{context} response must be UTF-8 JSON.", error);
        }

        return bytes;
    }

    internal static async Task<byte[]> ReadBoundedSccpBodyAsync(
        HttpContent content,
        int maximumBytes,
        string context,
        CancellationToken cancellationToken)
    {
        if (maximumBytes <= 0)
        {
            throw new ArgumentOutOfRangeException(
                nameof(maximumBytes),
                "SCCP response size bound must be positive.");
        }

        var declared = CanonicalContentLength(content, context);
        if (declared is { } declaredLength && declaredLength > maximumBytes)
        {
            throw new InvalidDataException($"{context} response body exceeds its SCCP size bound.");
        }

        await using var stream = await content.ReadAsStreamAsync(cancellationToken);
        using var output = declared is { } length
            ? new MemoryStream(checked((int)length))
            : new MemoryStream();
        var buffer = ArrayPool<byte>.Shared.Rent(81_920);
        try
        {
            while (true)
            {
                var count = await stream.ReadAsync(buffer.AsMemory(0, buffer.Length), cancellationToken);
                if (count == 0)
                {
                    break;
                }

                if (output.Length > maximumBytes - count)
                {
                    throw new InvalidDataException($"{context} response body exceeds its SCCP size bound.");
                }

                output.Write(buffer, 0, count);
            }

            return output.ToArray();
        }
        finally
        {
            ArrayPool<byte>.Shared.Return(buffer, clearArray: true);
        }
    }

    private static long? CanonicalContentLength(HttpContent content, string context)
    {
        if (!content.Headers.TryGetValues("Content-Length", out var values))
        {
            return null;
        }

        var rawValues = values.ToArray();
        if (rawValues.Length != 1)
        {
            throw new InvalidDataException(
                $"{context} response has an ambiguous Content-Length header.");
        }

        var raw = rawValues[0];
        if (raw.Length == 0
            || (raw.Length > 1 && raw[0] == '0')
            || raw.Any(static value => value is < '0' or > '9')
            || !long.TryParse(
                raw,
                System.Globalization.NumberStyles.None,
                System.Globalization.CultureInfo.InvariantCulture,
                out var parsed)
            || parsed.ToString(System.Globalization.CultureInfo.InvariantCulture) != raw)
        {
            throw new InvalidDataException(
                $"{context} response has a noncanonical Content-Length header.");
        }

        if (content.Headers.ContentLength is { } typed && typed != parsed)
        {
            throw new InvalidDataException(
                $"{context} response has an inconsistent Content-Length header.");
        }

        return parsed;
    }
}
