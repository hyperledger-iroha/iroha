using System.Net.Http.Headers;
using System.Text;
using System.Text.Json;
using Hyperledger.Iroha.Sccp;

namespace Hyperledger.Iroha.Torii;

public sealed partial class ToriiClient
{
    /// <summary>Fetch consensus-derived SCCP capabilities.</summary>
    public async Task<SccpCapabilities> GetSccpCapabilitiesAsync(
        CancellationToken cancellationToken = default)
    {
        var bytes = await GetExactSccpJsonAsync(
            "/v1/sccp/capabilities",
            query: null,
            "SCCP capabilities",
            cancellationToken);
        return SccpCapabilities.Parse(bytes);
    }

    /// <summary>Fetch exact inbound native-verifier and outbound destination route manifests.</summary>
    public async Task<SccpProofManifestSet> GetSccpProofManifestsAsync(
        CancellationToken cancellationToken = default)
    {
        var bytes = await GetExactSccpJsonAsync(
            "/v1/sccp/manifests",
            query: null,
            "SCCP proof manifests",
            cancellationToken);
        return SccpProofManifestSet.Parse(bytes);
    }

    /// <summary>Fetch newest-first committed outbound SCCP messages.</summary>
    public async Task<SccpRecentMessages> GetSccpRecentMessagesAsync(
        ulong? from = null,
        uint? limit = null,
        CancellationToken cancellationToken = default)
    {
        if (from == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(from), "from must be positive.");
        }

        if (limit is 0 or > 50)
        {
            throw new ArgumentOutOfRangeException(nameof(limit), "limit must be in 1..50.");
        }

        var fields = new List<string>();
        if (from is { } fromValue)
        {
            fields.Add($"from={fromValue.ToString(System.Globalization.CultureInfo.InvariantCulture)}");
        }

        if (limit is { } limitValue)
        {
            fields.Add($"limit={limitValue.ToString(System.Globalization.CultureInfo.InvariantCulture)}");
        }

        var bytes = await GetExactSccpJsonAsync(
            "/v1/sccp/messages/recent",
            fields.Count == 0 ? null : string.Join('&', fields),
            "SCCP recent messages",
            cancellationToken);
        return SccpRecentMessages.Parse(bytes);
    }

    /// <summary>Submit one canonical outbound SCCP message bundle.</summary>
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
        using var response = await SendAsync(
            HttpMethod.Post,
            path,
            content: content,
            accept: "application/json",
            cancellationToken: cancellationToken);
        var bytes = await ReadExactSccpJsonAsync(response, context, cancellationToken);
        return SccpBridgeSubmitResponse.Parse(bytes, expectation);
    }

    private async Task<byte[]> GetExactSccpJsonAsync(
        string path,
        string? query,
        string context,
        CancellationToken cancellationToken)
    {
        using var response = await SendAsync(
            HttpMethod.Get,
            path,
            query,
            accept: "application/json",
            cancellationToken: cancellationToken);
        return await ReadExactSccpJsonAsync(response, context, cancellationToken);
    }

    private static async Task<byte[]> ReadExactSccpJsonAsync(
        HttpResponseMessage response,
        string context,
        CancellationToken cancellationToken)
    {
        var mediaType = response.Content.Headers.ContentType?.MediaType;
        if (mediaType is null
            || !(string.Equals(mediaType, "application/json", StringComparison.OrdinalIgnoreCase)
                || mediaType.EndsWith("+json", StringComparison.OrdinalIgnoreCase)))
        {
            throw new InvalidDataException($"{context} response must use a JSON content type.");
        }

        var bytes = await response.Content.ReadAsByteArrayAsync(cancellationToken);
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
}
