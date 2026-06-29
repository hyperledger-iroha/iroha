using Hyperledger.Iroha.Crypto;

namespace Hyperledger.Iroha.Http;

public sealed class CanonicalRequestCredentials
{
    private readonly byte[] privateKeySeed;

    public CanonicalRequestCredentials(string accountId, ReadOnlySpan<byte> privateKeySeed)
    {
        var exactAccountId = CanonicalRequest.RequireExactNonBlank(accountId, nameof(accountId));
        if (privateKeySeed.IsEmpty)
        {
            throw new ArgumentException("private key seed must not be empty.", nameof(privateKeySeed));
        }

        var canonicalAccountId = CanonicalRequest.RequireCanonicalAccountId(exactAccountId, nameof(accountId));
        Ed25519Signer.RequirePrivateKeySeedLength(privateKeySeed, nameof(privateKeySeed));
        CanonicalRequest.EnsureAccountMatchesPrivateKey(canonicalAccountId, privateKeySeed, nameof(accountId));
        AccountId = canonicalAccountId;
        this.privateKeySeed = privateKeySeed.ToArray();
    }

    public string AccountId { get; }

    public byte[] PrivateKeySeed => [.. privateKeySeed];
}
