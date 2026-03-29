namespace Hyperledger.Iroha.Http;

public sealed class CanonicalRequestHeaders
{
    public CanonicalRequestHeaders(string accountId, string signatureBase64, long timestampMs, string nonce)
    {
        AccountId = accountId;
        SignatureBase64 = signatureBase64;
        TimestampMs = timestampMs;
        Nonce = nonce;
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
}
