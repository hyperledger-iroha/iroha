using Hyperledger.Iroha.Crypto;

namespace Hyperledger.Iroha.Http;

public sealed class CanonicalRequestHeaders
{
    public CanonicalRequestHeaders(string accountId, string signatureBase64, long timestampMs, string nonce)
    {
        var exactAccountId = CanonicalRequest.RequireExactNonBlank(accountId, nameof(accountId));
        var exactSignatureBase64 = CanonicalRequest.RequireExactNonBlank(signatureBase64, nameof(signatureBase64));
        var exactNonce = CanonicalRequest.RequireExactNonBlank(nonce, nameof(nonce));

        AccountId = CanonicalRequest.RequireCanonicalAccountId(exactAccountId, nameof(accountId));
        SignatureBase64 = RequireCanonicalSignatureBase64(exactSignatureBase64, nameof(signatureBase64));
        TimestampMs = CanonicalRequest.RequirePositiveTimestamp(timestampMs, nameof(timestampMs));
        Nonce = CanonicalRequest.RequireCanonicalNonce(exactNonce, nameof(nonce));
    }

    public string AccountId { get; }

    public string Nonce { get; }

    public string SignatureBase64 { get; }

    public long TimestampMs { get; }

    public IReadOnlyDictionary<string, string> ToDictionary()
    {
        return new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["X-Iroha-Account"] = AccountId,
            ["X-Iroha-Nonce"] = Nonce,
            ["X-Iroha-Signature"] = SignatureBase64,
            ["X-Iroha-Timestamp-Ms"] = TimestampMs.ToString(System.Globalization.CultureInfo.InvariantCulture),
        };
    }

    private static string RequireCanonicalSignatureBase64(string value, string paramName)
    {
        var exact = CanonicalRequest.RequireExactNonBlank(value, paramName);
        byte[] signature;
        try
        {
            signature = Convert.FromBase64String(exact);
        }
        catch (FormatException exception)
        {
            throw new ArgumentException("Signature must be base64 encoded.", paramName, exception);
        }

        if (!string.Equals(Convert.ToBase64String(signature), exact, StringComparison.Ordinal))
        {
            throw new ArgumentException("Signature must be canonical base64 text.", paramName);
        }

        if (signature.Length != Ed25519Signer.SignatureLength)
        {
            throw new ArgumentException(
                $"Signature must decode to {Ed25519Signer.SignatureLength} bytes.",
                paramName);
        }

        return exact;
    }
}
