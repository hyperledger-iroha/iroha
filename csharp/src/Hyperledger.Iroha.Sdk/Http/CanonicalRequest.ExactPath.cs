namespace Hyperledger.Iroha.Http;

public static partial class CanonicalRequest
{
    /// <summary>
    /// Builds canonical request headers for a caller-validated Taira-testnet URI path.
    /// </summary>
    internal static CanonicalRequestHeaders BuildHeadersForExactPath(
        NetworkId networkId,
        string accountId,
        ReadOnlySpan<byte> privateKeySeed,
        string method,
        string path,
        string? query = null,
        ReadOnlySpan<byte> body = default,
        long? timestampMs = null,
        string? nonce = null)
    {
        return BuildHeadersForExactPath(
            networkId,
            Address.AccountAddress.TairaTestnetChainDiscriminant,
            accountId,
            privateKeySeed,
            method,
            path,
            query,
            body,
            timestampMs,
            nonce);
    }

    /// <summary>
    /// Builds canonical request headers for a caller-validated exact URI path.
    /// </summary>
    /// <remarks>
    /// This internal entry point exists for closed Torii routes that validate
    /// their path segments before canonical signing. It applies the same exact
    /// ASCII wire-path checks as the public canonical-request builder.
    /// </remarks>
    internal static CanonicalRequestHeaders BuildHeadersForExactPath(
        NetworkId networkId,
        ushort chainDiscriminant,
        string accountId,
        ReadOnlySpan<byte> privateKeySeed,
        string method,
        string path,
        string? query = null,
        ReadOnlySpan<byte> body = default,
        long? timestampMs = null,
        string? nonce = null)
    {
        ArgumentNullException.ThrowIfNull(networkId);
        var exactAccountId = RequireExactNonBlank(accountId, nameof(accountId));
        var canonicalAccountId = RequireCanonicalAccountId(
            exactAccountId,
            nameof(accountId),
            chainDiscriminant);
        var exactMethod = RequireHttpMethodToken(method, nameof(method));
        var exactPath = RequireCallerValidatedExactPath(path, nameof(path));
        var effectiveTimestamp = RequirePositiveTimestamp(
            timestampMs ?? DateTimeOffset.UtcNow.ToUnixTimeMilliseconds(),
            nameof(timestampMs));
        var effectiveNonce = nonce is null
            ? GenerateNonce()
            : RequireCanonicalNonce(nonce, nameof(nonce));

        EnsureAccountMatchesPrivateKey(canonicalAccountId, privateKeySeed, nameof(accountId));
        var signatureMessage = BuildSignatureMessageForExactPath(
            networkId,
            exactMethod,
            exactPath,
            query,
            body,
            effectiveTimestamp,
            effectiveNonce);
        var signature = Hyperledger.Iroha.Crypto.Ed25519Signer.Sign(
            signatureMessage,
            privateKeySeed);
        return new CanonicalRequestHeaders(
            canonicalAccountId,
            Convert.ToBase64String(signature),
            effectiveTimestamp,
            effectiveNonce);
    }

    private static string RequireCallerValidatedExactPath(string? value, string paramName)
    {
        var exact = RequireExactNonBlank(value, paramName);
        RequireCanonicalPathByteLength(exact, paramName);
        RequireCanonicalPathAsciiWireSpelling(exact, paramName);
        if (exact[0] != '/' || (exact.Length > 1 && exact[1] == '/'))
        {
            throw new ArgumentException(
                $"{paramName} must be a root-relative, non-scheme-relative path.",
                paramName);
        }
        if (exact.Contains('?', StringComparison.Ordinal)
            || exact.Contains('#', StringComparison.Ordinal)
            || exact.Contains('\\', StringComparison.Ordinal))
        {
            throw new ArgumentException(
                $"{paramName} must be an exact path without query, fragment, or backslash characters.",
                paramName);
        }

        RequireUriStablePath(exact, paramName);
        return exact;
    }
}
