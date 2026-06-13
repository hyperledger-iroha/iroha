namespace Hyperledger.Iroha.Http;

public sealed class CanonicalRequestCredentials
{
    private readonly byte[] privateKeySeed;

    public CanonicalRequestCredentials(string accountId, ReadOnlySpan<byte> privateKeySeed)
    {
        if (privateKeySeed.Length == 0)
        {
            throw new ArgumentException("private key seed must not be empty", nameof(privateKeySeed));
        }

        AccountId = CanonicalRequest.RequireExactNonBlank(accountId, nameof(accountId));
        this.privateKeySeed = privateKeySeed.ToArray();
    }

    public string AccountId { get; }

    public byte[] PrivateKeySeed => [.. privateKeySeed];
}
