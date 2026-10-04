using System.Net;
using System.Text.Json;

namespace Hyperledger.Iroha.Torii;

/// <summary>
/// A non-success Torii response. Torii answers every failure with the error envelope
/// <c>{"code": "...", "message": "...", "details": {...}}</c>; this exception exposes its parts.
/// </summary>
/// <remarks>
/// <see cref="IrohaException.Code"/> is the envelope <c>code</c> (for example
/// <c>invalid_filter</c>). Responses without an envelope, such as an intermediary's HTML page,
/// use <c>http_&lt;status&gt;</c>.
/// </remarks>
public sealed class ToriiApiException : IrohaException
{
    /// <summary>The header Torii uses to publish a stable rejection code.</summary>
    public const string RejectCodeHeaderName = "x-iroha-reject-code";

    /// <summary>Creates an exception from a Torii response.</summary>
    /// <param name="statusCode">HTTP status of the response.</param>
    /// <param name="requestUri">The request target.</param>
    /// <param name="responseBody">The bounded response body; parsed as the Torii error envelope.</param>
    /// <param name="reasonPhrase">The HTTP reason phrase.</param>
    /// <param name="rejectCode">The <c>x-iroha-reject-code</c> response header, when present.</param>
    public ToriiApiException(
        HttpStatusCode statusCode,
        Uri? requestUri,
        string? responseBody,
        string? reasonPhrase,
        string? rejectCode = null)
        : this(statusCode, requestUri, responseBody, reasonPhrase, rejectCode, Envelope.Parse(responseBody))
    {
    }

    private ToriiApiException(
        HttpStatusCode statusCode,
        Uri? requestUri,
        string? responseBody,
        string? reasonPhrase,
        string? rejectCode,
        Envelope envelope)
        : base(
            envelope.Code ?? $"http_{(int)statusCode}",
            CreateMessage(statusCode, requestUri, responseBody, reasonPhrase, envelope),
            statusCode,
            envelope.Details,
            innerException: null)
    {
        RequestUri = requestUri;
        ResponseBody = responseBody;
        ReasonPhrase = reasonPhrase;
        ServerMessage = envelope.Message;
        RejectCode = string.IsNullOrWhiteSpace(rejectCode) ? envelope.Details?.RejectCode : rejectCode;
    }

    /// <summary>The request target.</summary>
    public Uri? RequestUri { get; }

    /// <summary>The bounded response body exactly as received.</summary>
    public string? ResponseBody { get; }

    /// <summary>The HTTP reason phrase.</summary>
    public string? ReasonPhrase { get; }

    /// <summary>The envelope <c>message</c>, or <see langword="null"/> without an envelope.</summary>
    public string? ServerMessage { get; }

    /// <summary>
    /// The stable rejection code from the <c>x-iroha-reject-code</c> header, falling back to
    /// <c>details.reject_code</c>.
    /// </summary>
    public string? RejectCode { get; }

    private static string CreateMessage(
        HttpStatusCode statusCode,
        Uri? requestUri,
        string? responseBody,
        string? reasonPhrase,
        Envelope envelope)
    {
        var target = requestUri?.ToString() ?? "<unknown>";
        var reason = string.IsNullOrWhiteSpace(reasonPhrase) ? statusCode.ToString() : reasonPhrase;
        var prefix = $"Torii request to `{target}` failed with {(int)statusCode} {reason}";
        if (envelope.Code is not null)
        {
            return envelope.Message is null
                ? $"{prefix} ({envelope.Code})."
                : $"{prefix} ({envelope.Code}): {envelope.Message}";
        }

        return string.IsNullOrWhiteSpace(responseBody) ? $"{prefix}." : $"{prefix}: {responseBody}";
    }

    private readonly record struct Envelope(string? Code, string? Message, IrohaErrorDetails? Details)
    {
        internal static Envelope Parse(string? body)
        {
            if (string.IsNullOrWhiteSpace(body) || body.TrimStart()[0] != '{')
            {
                return default;
            }

            try
            {
                using var document = JsonDocument.Parse(body, new JsonDocumentOptions { MaxDepth = 64 });
                var root = document.RootElement;
                if (!root.TryGetProperty("code", out var code)
                    || code.ValueKind != JsonValueKind.String
                    || string.IsNullOrWhiteSpace(code.GetString()))
                {
                    return default;
                }

                var message = root.TryGetProperty("message", out var text) && text.ValueKind == JsonValueKind.String
                    ? text.GetString()
                    : null;
                var details = root.TryGetProperty("details", out var detailsElement)
                    ? IrohaErrorDetails.FromElement(detailsElement)
                    : null;
                return new Envelope(code.GetString(), message, details);
            }
            catch (JsonException)
            {
                return default;
            }
        }
    }
}
