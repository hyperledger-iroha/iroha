using System.Security.Cryptography;
using Hyperledger.Iroha.Address;

namespace Hyperledger.Iroha.Crypto;

public sealed class Ed25519KeyPair : IDisposable
{
    private readonly byte[] privateKeySeed;
    private readonly byte[] publicKey;
    private bool disposed;

    private Ed25519KeyPair(byte[] privateKeySeed, byte[] publicKey)
    {
        this.privateKeySeed = privateKeySeed;
        this.publicKey = publicKey;
    }

    internal ReadOnlySpan<byte> PrivateKeySeedSpan
    {
        get
        {
            ThrowIfDisposed();
            return privateKeySeed;
        }
    }

    public byte[] PublicKey
    {
        get
        {
            ThrowIfDisposed();
            return [.. publicKey];
        }
    }

    /// <summary>
    /// Explicitly exports a copy of the private seed for APIs that cannot accept this key pair.
    /// The caller owns the returned secret and should zero it after use.
    /// </summary>
    public byte[] ExportPrivateKeySeed()
    {
        ThrowIfDisposed();
        return [.. privateKeySeed];
    }

    public static Ed25519KeyPair FromSeed(ReadOnlySpan<byte> privateKeySeed)
    {
        Ed25519Signer.RequirePrivateKeySeedLength(privateKeySeed, nameof(privateKeySeed));

        var seed = privateKeySeed.ToArray();
        try
        {
            var publicKey = Ed25519Signer.GetPublicKey(seed);
            return new Ed25519KeyPair(seed, publicKey);
        }
        catch
        {
            CryptographicOperations.ZeroMemory(seed);
            throw;
        }
    }

    public static Ed25519KeyPair Generate()
    {
        var seed = RandomNumberGenerator.GetBytes(Ed25519Signer.PrivateKeySeedLength);
        try
        {
            return FromSeed(seed);
        }
        finally
        {
            CryptographicOperations.ZeroMemory(seed);
        }
    }

    public AccountAddress ToAccountAddress()
    {
        ThrowIfDisposed();
        return AccountAddress.FromPublicKey(publicKey);
    }

    /// <summary>Zeros the owned private seed. The key pair cannot be used afterwards.</summary>
    public void Dispose()
    {
        if (disposed)
        {
            return;
        }

        CryptographicOperations.ZeroMemory(privateKeySeed);
        disposed = true;
    }

    private void ThrowIfDisposed()
    {
        ObjectDisposedException.ThrowIf(disposed, this);
    }
}
