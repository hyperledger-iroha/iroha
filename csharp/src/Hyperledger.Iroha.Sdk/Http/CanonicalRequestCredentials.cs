using System.Security.Cryptography;
using Hyperledger.Iroha.Crypto;

namespace Hyperledger.Iroha.Http;

public sealed class CanonicalRequestCredentials : IDisposable
{
    private readonly byte[] privateKeySeed;
    private bool disposed;

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

    /// <summary>
    /// The private seed remains write-only from the public API. Internal signing uses this
    /// read-only span directly so each request does not leave another secret array for the GC.
    /// </summary>
    internal ReadOnlySpan<byte> PrivateKeySeedSpan
    {
        get
        {
            ObjectDisposedException.ThrowIf(disposed, this);
            return privateKeySeed;
        }
    }

    /// <summary>Zeros the owned private seed. The credentials cannot be used afterwards.</summary>
    public void Dispose()
    {
        if (disposed)
        {
            return;
        }

        CryptographicOperations.ZeroMemory(privateKeySeed);
        disposed = true;
    }
}
