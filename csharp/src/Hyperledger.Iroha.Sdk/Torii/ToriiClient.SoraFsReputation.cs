using System.Globalization;
using System.Net;
using System.Net.Http.Headers;
using System.Runtime.CompilerServices;
using System.Text;
using Hyperledger.Iroha.Http;
using Hyperledger.Iroha.SoraFs;

namespace Hyperledger.Iroha.Torii;

public sealed partial class ToriiClient
{
    public const int SoraFsReputationDefaultPageLimit = 500;
    public const int SoraFsReputationMaximumResponseBytes = 4 * 1024 * 1024;
    public const int SoraFsReputationMaximumSseFrameBytes = 64 * 1024;

    private const string SoraFsReputationLatestPath = "/v1/sorafs/reputation/latest";
    private const string SoraFsReputationProviderPathPrefix = "/v1/sorafs/reputation/providers/";
    private const string SoraFsReputationSnapshotPathPrefix = "/v1/sorafs/reputation/snapshots/";
    private const string SoraFsReputationWeightsPath = "/v1/sorafs/reputation/weights";
    private const string SoraFsReputationEventsPath = "/v1/sorafs/reputation/events";
    private const string SoraFsReputationEventsStreamPath =
        "/v1/sorafs/reputation/events/stream";

    private static readonly string[] SoraFsReputationForbiddenDefaultHeaders =
    [
        "Accept",
        "Accept-Encoding",
        "Cache-Control",
        "If-None-Match",
        "Last-Event-ID",
        "Range",
        "X-Iroha-Account",
        "X-Iroha-Signature",
        "X-Iroha-Timestamp-Ms",
        "X-Iroha-Nonce",
        "X-Iroha-Witness",
    ];

    private readonly bool injectedSoraFsReputationTransportIsOneShot;

    internal ToriiClient(
        Uri baseUri,
        HttpClient httpClient,
        ToriiClientOptions? options,
        SoraFsReputationTransportAssurance transportAssurance)
        : this(baseUri, httpClient, options)
    {
        if (transportAssurance
            != SoraFsReputationTransportAssurance.OneShotWithoutRedirectsRetriesOrDecompression)
        {
            throw new ArgumentOutOfRangeException(nameof(transportAssurance));
        }
        injectedSoraFsReputationTransportIsOneShot = true;
    }

    /// <summary>Fetches a bounded prefix of the latest committed reputation snapshot.</summary>
    public async Task<SoraFsReputationSnapshotSummaryV1?> GetSoraFsReputationLatestAsync(
        int limit = SoraFsReputationDefaultPageLimit,
        CancellationToken cancellationToken = default)
    {
        var exactLimit = RequireSoraFsReputationLimit(limit);
        var query = $"limit={exactLimit.ToString(CultureInfo.InvariantCulture)}";
        using var response = await SendSoraFsReputationRequestAsync(
            SoraFsReputationLatestPath,
            query,
            "application/json",
            allowNotFound: true,
            cancellationToken);
        if (response is null)
        {
            return null;
        }

        var payload = await ReadSoraFsReputationResponseAsync(
            response,
            "latest SoraFS reputation response",
            cancellationToken);
        return SoraFsReputationJson.ParseSnapshot(
            payload,
            exactLimit,
            expectedSnapshotIdHex: null,
            "latest SoraFS reputation response");
    }

    /// <summary>Fetches one provider and its proof from the latest committed snapshot.</summary>
    public async Task<SoraFsReputationProviderResponseV1?> GetSoraFsReputationProviderAsync(
        string providerId,
        CancellationToken cancellationToken = default)
    {
        var exactProviderId = SoraFsReputationJson.RequireProviderId(
            providerId,
            nameof(providerId));
        var path = $"{SoraFsReputationProviderPathPrefix}{exactProviderId}";
        using var response = await SendSoraFsReputationRequestAsync(
            path,
            query: null,
            "application/json",
            allowNotFound: true,
            cancellationToken);
        if (response is null)
        {
            return null;
        }

        var payload = await ReadSoraFsReputationResponseAsync(
            response,
            "provider SoraFS reputation response",
            cancellationToken);
        return SoraFsReputationJson.ParseProviderResponse(
            payload,
            exactProviderId,
            "provider SoraFS reputation response");
    }

    /// <summary>Fetches a bounded prefix of one exact retained reputation snapshot.</summary>
    public async Task<SoraFsReputationSnapshotSummaryV1?> GetSoraFsReputationSnapshotAsync(
        string snapshotIdHex,
        int limit = SoraFsReputationDefaultPageLimit,
        CancellationToken cancellationToken = default)
    {
        var exactSnapshotId = SoraFsReputationJson.RequireSnapshotId(
            snapshotIdHex,
            nameof(snapshotIdHex));
        var exactLimit = RequireSoraFsReputationLimit(limit);
        var query = $"limit={exactLimit.ToString(CultureInfo.InvariantCulture)}";
        using var response = await SendSoraFsReputationRequestAsync(
            $"{SoraFsReputationSnapshotPathPrefix}{exactSnapshotId}",
            query,
            "application/json",
            allowNotFound: true,
            cancellationToken);
        if (response is null)
        {
            return null;
        }

        var payload = await ReadSoraFsReputationResponseAsync(
            response,
            "historical SoraFS reputation response",
            cancellationToken);
        return SoraFsReputationJson.ParseSnapshot(
            payload,
            exactLimit,
            exactSnapshotId,
            "historical SoraFS reputation response");
    }

    /// <summary>Fetches the active weights bound to the latest committed snapshot.</summary>
    public async Task<SoraFsReputationWeightsResponseV1> GetSoraFsReputationWeightsAsync(
        CancellationToken cancellationToken = default)
    {
        using var response = await SendSoraFsReputationRequestAsync(
            SoraFsReputationWeightsPath,
            query: null,
            "application/json",
            allowNotFound: false,
            cancellationToken);
        var payload = await ReadSoraFsReputationResponseAsync(
            response!,
            "weights SoraFS reputation response",
            cancellationToken);
        return SoraFsReputationJson.ParseWeightsResponse(
            payload,
            "weights SoraFS reputation response");
    }

    /// <summary>Fetches a bounded page of committed reputation publication events.</summary>
    public async Task<SoraFsReputationEventsResponseV1> GetSoraFsReputationEventsAsync(
        ulong? since = null,
        int limit = SoraFsReputationDefaultPageLimit,
        CancellationToken cancellationToken = default)
    {
        var exactLimit = RequireSoraFsReputationLimit(limit);
        var query = BuildSoraFsReputationEventQuery(since, exactLimit);
        using var response = await SendSoraFsReputationRequestAsync(
            SoraFsReputationEventsPath,
            query,
            "application/json",
            allowNotFound: false,
            cancellationToken);
        var payload = await ReadSoraFsReputationResponseAsync(
            response!,
            "events SoraFS reputation response",
            cancellationToken);
        return SoraFsReputationJson.ParseEventPage(
            payload,
            since,
            exactLimit,
            "events SoraFS reputation response");
    }

    /// <summary>
    /// Opens one authenticated SSE request without redirects, retries, resume headers, or
    /// reconnection.
    /// </summary>
    public async IAsyncEnumerable<SoraFsReputationSseFrameV1>
        StreamSoraFsReputationEventsAsync(
            ulong since = 0,
            int limit = SoraFsReputationDefaultPageLimit,
            [EnumeratorCancellation] CancellationToken cancellationToken = default)
    {
        var exactLimit = RequireSoraFsReputationLimit(limit);
        var query = BuildSoraFsReputationEventQuery(since, exactLimit);
        using var response = await SendSoraFsReputationRequestAsync(
            SoraFsReputationEventsStreamPath,
            query,
            "text/event-stream",
            allowNotFound: false,
            cancellationToken);
        ValidateSoraFsReputationResponseHeaders(
            response!,
            "text/event-stream",
            "SoraFS reputation SSE response");

        await using var stream = await response!.Content.ReadAsStreamAsync(cancellationToken);
        var parser = new SoraFsReputationSseParser(since);
        var buffer = new byte[8 * 1024];
        while (true)
        {
            var read = await stream.ReadAsync(buffer.AsMemory(), cancellationToken);
            if (read == 0)
            {
                break;
            }

            for (var index = 0; index < read; index++)
            {
                var frame = parser.Consume(buffer[index]);
                if (frame is not null)
                {
                    yield return frame;
                }
            }
        }

        parser.Finish();
    }

    private async Task<HttpResponseMessage?> SendSoraFsReputationRequestAsync(
        string path,
        string? query,
        string accept,
        bool allowNotFound,
        CancellationToken cancellationToken)
    {
        EnsureSoraFsReputationRequestConfiguration(path);
        using var request = CreateSoraFsReputationRequest(path, query, accept);
        var expectedUri = request.RequestUri!;
        var response = await HttpClient.SendAsync(
            request,
            HttpCompletionOption.ResponseHeadersRead,
            cancellationToken);

        var actualUri = response.RequestMessage?.RequestUri;
        if (actualUri is null
            || !string.Equals(
                actualUri.AbsoluteUri,
                expectedUri.AbsoluteUri,
                StringComparison.Ordinal))
        {
            response.Dispose();
            throw new InvalidDataException(
                "SoraFS reputation transport changed the exact request target.");
        }

        if (allowNotFound && response.StatusCode == HttpStatusCode.NotFound)
        {
            response.Dispose();
            return null;
        }
        if (response.StatusCode != HttpStatusCode.OK)
        {
            var error = new ToriiApiException(
                response.StatusCode,
                actualUri,
                responseBody: null,
                response.ReasonPhrase);
            response.Dispose();
            throw error;
        }

        return response;
    }

    private HttpRequestMessage CreateSoraFsReputationRequest(
        string path,
        string? query,
        string accept)
    {
        var target = BuildExactSoraFsReputationUri(path, query);
        var request = new HttpRequestMessage(HttpMethod.Get, target);
        request.Headers.Accept.Add(new MediaTypeWithQualityHeaderValue(accept));
        if (!request.Headers.TryAddWithoutValidation("Accept-Encoding", "identity")
            || !request.Headers.TryAddWithoutValidation("Cache-Control", "no-store"))
        {
            request.Dispose();
            throw new InvalidOperationException(
                "Unable to set closed SoraFS reputation request headers.");
        }

        var bearerToken = NormalizeOptionalBearerToken(
            Options.BearerToken,
            nameof(ToriiClientOptions.BearerToken));
        if (bearerToken is not null)
        {
            request.Headers.Authorization = new AuthenticationHeaderValue("Bearer", bearerToken);
        }

        var credentials = Options.CanonicalRequestCredentials!;
        var signingContext = Options.LocalSigningContext
            ?? throw new InvalidOperationException(
                "Authenticated SoraFS reputation requests require ToriiClientOptions.LocalSigningContext.");
        var canonicalHeaders = CanonicalRequest.BuildHeadersForExactPath(
            signingContext.NetworkId,
            signingContext.ChainDiscriminant,
            credentials.AccountId,
            credentials.PrivateKeySeed,
            "GET",
            target.AbsolutePath,
            target.Query,
            ReadOnlySpan<byte>.Empty);
        foreach (var header in canonicalHeaders.ToDictionary())
        {
            if (!request.Headers.TryAddWithoutValidation(header.Key, header.Value))
            {
                request.Dispose();
                throw new InvalidOperationException(
                    $"Unable to set canonical SoraFS reputation header `{header.Key}`.");
            }
        }

        return request;
    }

    private void EnsureSoraFsReputationRequestConfiguration(string path)
    {
        RequireCanonicalRequestCredentials(path);
        if (!ownsHttpClient && !injectedSoraFsReputationTransportIsOneShot)
        {
            throw new InvalidOperationException(
                "Authenticated SoraFS reputation requests require ToriiClient's internally managed one-shot, no-redirect, identity transport.");
        }
        if (!string.Equals(BaseUri.Scheme, Uri.UriSchemeHttps, StringComparison.OrdinalIgnoreCase)
            || string.IsNullOrEmpty(BaseUri.Host))
        {
            throw new InvalidOperationException(
                "Authenticated SoraFS reputation requests require an absolute HTTPS Torii authority.");
        }

        foreach (var name in SoraFsReputationForbiddenDefaultHeaders)
        {
            if (HttpClient.DefaultRequestHeaders.Contains(name))
            {
                throw new InvalidOperationException(
                    $"HttpClient.DefaultRequestHeaders must not inject `{name}` into SoraFS reputation requests.");
            }
        }
    }

    private Uri BuildExactSoraFsReputationUri(string path, string? query)
    {
        var baseText = BaseUri.AbsoluteUri.EndsWith("/", StringComparison.Ordinal)
            ? BaseUri.AbsoluteUri[..^1]
            : BaseUri.AbsoluteUri;
        var targetText = string.IsNullOrEmpty(query)
            ? $"{baseText}{path}"
            : $"{baseText}{path}?{query}";
        var creationOptions = new UriCreationOptions
        {
            DangerousDisablePathAndQueryCanonicalization = true,
        };
        var target = new Uri(targetText, in creationOptions);
        var basePath = BaseUri.AbsolutePath == "/"
            ? string.Empty
            : BaseUri.AbsolutePath.TrimEnd('/');
        var expectedPath = $"{basePath}{path}";
        var expectedQuery = string.IsNullOrEmpty(query) ? string.Empty : $"?{query}";
        if (!string.Equals(target.AbsolutePath, expectedPath, StringComparison.Ordinal)
            || !string.Equals(target.Query, expectedQuery, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "SoraFS reputation target could not preserve its exact path and query.",
                nameof(path));
        }

        return target;
    }

    private static async Task<byte[]> ReadSoraFsReputationResponseAsync(
        HttpResponseMessage response,
        string context,
        CancellationToken cancellationToken)
    {
        ValidateSoraFsReputationResponseHeaders(response, "application/json", context);
        var declaredLength = response.Content.Headers.ContentLength;
        if (declaredLength is > SoraFsReputationMaximumResponseBytes)
        {
            throw new InvalidDataException(
                $"{context} exceeds the {SoraFsReputationMaximumResponseBytes}-byte limit.");
        }

        await using var stream = await response.Content.ReadAsStreamAsync(cancellationToken);
        using var output = declaredLength is > 0
            ? new MemoryStream(checked((int)declaredLength.Value))
            : new MemoryStream();
        var buffer = new byte[8 * 1024];
        var total = 0;
        while (true)
        {
            var read = await stream.ReadAsync(buffer.AsMemory(), cancellationToken);
            if (read == 0)
            {
                break;
            }
            if (read > SoraFsReputationMaximumResponseBytes - total)
            {
                throw new InvalidDataException(
                    $"{context} exceeds the {SoraFsReputationMaximumResponseBytes}-byte limit.");
            }
            await output.WriteAsync(buffer.AsMemory(0, read), cancellationToken);
            total += read;
        }

        if (declaredLength.HasValue && declaredLength.Value != total)
        {
            throw new InvalidDataException(
                $"{context} length does not match its Content-Length header.");
        }
        return output.ToArray();
    }

    private static void ValidateSoraFsReputationResponseHeaders(
        HttpResponseMessage response,
        string expectedMediaType,
        string context)
    {
        if (response.Content is null)
        {
            throw new InvalidDataException($"{context} omitted its response content.");
        }

        var contentType = response.Content.Headers.ContentType;
        if (contentType?.MediaType is null
            || !string.Equals(
                contentType.MediaType,
                expectedMediaType,
                StringComparison.OrdinalIgnoreCase))
        {
            throw new InvalidDataException(
                $"{context} must use Content-Type {expectedMediaType}.");
        }

        var parameters = contentType.Parameters.ToArray();
        if (parameters.Length > 1)
        {
            throw new InvalidDataException(
                $"{context} uses unsupported duplicate Content-Type parameters.");
        }
        foreach (var parameter in parameters)
        {
            if (!string.Equals(parameter.Name, "charset", StringComparison.OrdinalIgnoreCase)
                || !string.Equals(
                    parameter.Value?.Trim('"'),
                    "utf-8",
                    StringComparison.OrdinalIgnoreCase))
            {
                throw new InvalidDataException(
                    $"{context} uses an unsupported Content-Type parameter.");
            }
        }

        var encodings = response.Content.Headers.ContentEncoding.ToArray();
        if (encodings.Length > 1
            || (encodings.Length == 1
                && !string.Equals(encodings[0], "identity", StringComparison.OrdinalIgnoreCase)))
        {
            throw new InvalidDataException(
                $"{context} must use identity content encoding.");
        }
    }

    private static int RequireSoraFsReputationLimit(int limit)
    {
        if (limit is < 1 or > 500)
        {
            throw new ArgumentOutOfRangeException(
                nameof(limit),
                "SoraFS reputation limit must be within 1..500.");
        }
        return limit;
    }

    private static string BuildSoraFsReputationEventQuery(ulong? since, int limit)
    {
        var limitText = limit.ToString(CultureInfo.InvariantCulture);
        return since.HasValue
            ? $"since={since.Value.ToString(CultureInfo.InvariantCulture)}&limit={limitText}"
            : $"limit={limitText}";
    }
}

internal sealed class SoraFsReputationSseParser
{
    private static readonly Encoding StrictUtf8 = new UTF8Encoding(false, true);

    private readonly ulong requestedSince;
    private readonly List<byte> line = [];
    private ulong cursor;
    private ulong? expectedAfterLag;
    private int frameBytes;
    private string? eventName;
    private string? eventId;
    private string? eventData;
    private SoraFsReputationSnapshotEventV1? previousSnapshot;

    internal SoraFsReputationSseParser(ulong requestedSince)
    {
        this.requestedSince = requestedSince;
        cursor = requestedSince;
    }

    internal SoraFsReputationSseFrameV1? Consume(byte value)
    {
        if (frameBytes >= ToriiClient.SoraFsReputationMaximumSseFrameBytes)
        {
            throw Invalid(
                $"SSE frame exceeds {ToriiClient.SoraFsReputationMaximumSseFrameBytes} bytes.");
        }
        frameBytes++;
        if (value == 0)
        {
            throw Invalid("SSE stream contains a NUL byte.");
        }

        if (value != (byte)'\n')
        {
            line.Add(value);
            return null;
        }

        if (line.Count > 0 && line[^1] == (byte)'\r')
        {
            line.RemoveAt(line.Count - 1);
        }
        string rendered;
        try
        {
            rendered = StrictUtf8.GetString(line.ToArray());
        }
        catch (DecoderFallbackException exception)
        {
            throw new InvalidDataException(
                "SoraFS reputation SSE line is not strict UTF-8.",
                exception);
        }
        line.Clear();

        if (rendered.Length == 0)
        {
            var frame = FinishFrame();
            ResetFrame();
            return frame;
        }
        ConsumeLine(rendered);
        return null;
    }

    internal void Finish()
    {
        if (line.Count != 0
            || eventName is not null
            || eventId is not null
            || eventData is not null)
        {
            throw Invalid("SSE stream terminated inside a frame.");
        }
        if (expectedAfterLag.HasValue)
        {
            throw Invalid("SSE stream terminated before a declared lag was closed.");
        }
    }

    private void ConsumeLine(string value)
    {
        if (value[0] == ':')
        {
            if (eventName is not null || eventId is not null || eventData is not null)
            {
                throw Invalid("SSE comment interrupts a data frame.");
            }
            frameBytes = 0;
            return;
        }

        if (value.StartsWith("event: ", StringComparison.Ordinal))
        {
            if (eventName is not null)
            {
                throw Invalid("SSE frame repeats event.");
            }
            eventName = value["event: ".Length..];
            return;
        }
        if (value.StartsWith("id: ", StringComparison.Ordinal))
        {
            if (eventId is not null)
            {
                throw Invalid("SSE frame repeats id.");
            }
            eventId = value["id: ".Length..];
            return;
        }
        if (value.StartsWith("data: ", StringComparison.Ordinal))
        {
            if (eventData is not null)
            {
                throw Invalid("SSE frame repeats data.");
            }
            eventData = value["data: ".Length..];
            return;
        }

        throw Invalid("SSE frame contains an unsupported field.");
    }

    private SoraFsReputationSseFrameV1? FinishFrame()
    {
        if (eventName is null && eventId is null && eventData is null)
        {
            return null;
        }
        if (eventName is null || eventData is null)
        {
            throw Invalid("SSE frame omits event or data.");
        }

        if (string.Equals(eventName, "lagged", StringComparison.Ordinal))
        {
            if (eventId is not null
                || expectedAfterLag.HasValue
                || !TryParsePositiveCanonicalU64(eventData, out var skipped)
                || cursor == ulong.MaxValue
                || skipped > ulong.MaxValue - cursor - 1)
            {
                throw Invalid("lagged SSE frame is not canonical.");
            }

            expectedAfterLag = cursor + skipped + 1;
            previousSnapshot = null;
            return new SoraFsReputationLaggedSseFrameV1(skipped);
        }

        if (!string.Equals(eventName, "reputation_snapshot", StringComparison.Ordinal)
            || eventId is null
            || !TryParsePositiveCanonicalU64(eventId, out var parsedId))
        {
            throw Invalid("snapshot SSE frame is not canonical.");
        }

        var eventPayload = Encoding.UTF8.GetBytes(eventData);
        var snapshot = SoraFsReputationJson.ParseCompactEvent(
            eventPayload,
            "SoraFS reputation SSE snapshot data");
        ulong? expectedSequence = expectedAfterLag;
        if (!expectedSequence.HasValue && cursor != ulong.MaxValue)
        {
            expectedSequence = cursor + 1;
        }
        if (!expectedSequence.HasValue
            || parsedId != snapshot.Sequence
            || snapshot.Sequence != expectedSequence.Value
            || snapshot.Sequence <= requestedSince)
        {
            throw Invalid("snapshot SSE id, request cursor, and event sequence do not bind.");
        }

        if (previousSnapshot is not null
            && (!string.Equals(
                    snapshot.PreviousSnapshotIdHex,
                    previousSnapshot.SnapshotIdHex,
                    StringComparison.Ordinal)
                || snapshot.GeneratedAtUnix <= previousSnapshot.GeneratedAtUnix))
        {
            throw Invalid("snapshot SSE events do not form a continuous snapshot chain.");
        }

        cursor = snapshot.Sequence;
        expectedAfterLag = null;
        previousSnapshot = snapshot;
        return new SoraFsReputationSnapshotSseFrameV1(parsedId, snapshot);
    }

    private void ResetFrame()
    {
        frameBytes = 0;
        eventName = null;
        eventId = null;
        eventData = null;
    }

    private static bool TryParsePositiveCanonicalU64(string value, out ulong parsed)
    {
        parsed = 0;
        return value.Length is >= 1 and <= 20
            && value[0] is >= '1' and <= '9'
            && value.All(static character => character is >= '0' and <= '9')
            && ulong.TryParse(
                value,
                NumberStyles.None,
                CultureInfo.InvariantCulture,
                out parsed);
    }

    private static InvalidDataException Invalid(string message) =>
        new($"SoraFS reputation {message}");
}

internal enum SoraFsReputationTransportAssurance
{
    OneShotWithoutRedirectsRetriesOrDecompression,
}
