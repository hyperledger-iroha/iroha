using System.Net;

namespace Hyperledger.Iroha.Torii;

public sealed class ToriiApiException : HttpRequestException
{
    public ToriiApiException(HttpStatusCode statusCode, Uri? requestUri, string? responseBody, string? reasonPhrase)
        : base(CreateMessage(statusCode, requestUri, responseBody, reasonPhrase), inner: null, statusCode)
    {
        RequestUri = requestUri;
        ResponseBody = responseBody;
        ReasonPhrase = reasonPhrase;
    }

    public Uri? RequestUri { get; }

    public string? ResponseBody { get; }

    public string? ReasonPhrase { get; }

    private static string CreateMessage(
        HttpStatusCode statusCode,
        Uri? requestUri,
        string? responseBody,
        string? reasonPhrase)
    {
        var target = requestUri?.ToString() ?? "<unknown>";
        var reason = string.IsNullOrWhiteSpace(reasonPhrase) ? statusCode.ToString() : reasonPhrase;
        if (string.IsNullOrWhiteSpace(responseBody))
        {
            return $"Torii request to `{target}` failed with {(int)statusCode} {reason}.";
        }

        return $"Torii request to `{target}` failed with {(int)statusCode} {reason}: {responseBody}";
    }
}
