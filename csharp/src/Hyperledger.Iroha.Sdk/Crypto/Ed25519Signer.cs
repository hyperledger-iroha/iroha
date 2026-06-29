using Chaos.NaCl;
using System.Security.Cryptography;

namespace Hyperledger.Iroha.Crypto;

public static class Ed25519Signer
{
    public const int PrivateKeySeedLength = 32;
    public const int PublicKeyLength = 32;
    public const int SignatureLength = 64;

    public static byte[] GetPublicKey(ReadOnlySpan<byte> privateKeySeed)
    {
        var seed = ValidatePrivateKeySeed(privateKeySeed);
        try
        {
            return Ed25519.PublicKeyFromSeed(seed);
        }
        finally
        {
            Clear(seed);
        }
    }

    public static byte[] Sign(ReadOnlySpan<byte> message, ReadOnlySpan<byte> privateKeySeed)
    {
        var seed = ValidatePrivateKeySeed(privateKeySeed);
        byte[]? expandedPrivateKey = null;
        byte[]? messageBytes = null;
        try
        {
            expandedPrivateKey = Ed25519.ExpandedPrivateKeyFromSeed(seed);
            messageBytes = message.ToArray();
            return Ed25519.Sign(messageBytes, expandedPrivateKey);
        }
        finally
        {
            Clear(messageBytes);
            Clear(expandedPrivateKey);
            Clear(seed);
        }
    }

    public static bool Verify(ReadOnlySpan<byte> message, ReadOnlySpan<byte> signature, ReadOnlySpan<byte> publicKey)
    {
        if (signature.Length != SignatureLength)
        {
            throw new ArgumentException($"signature must be {SignatureLength} bytes", nameof(signature));
        }

        if (publicKey.Length != PublicKeyLength)
        {
            throw new ArgumentException($"public key must be {PublicKeyLength} bytes", nameof(publicKey));
        }

        byte[]? signatureBytes = null;
        byte[]? messageBytes = null;
        byte[]? publicKeyBytes = null;
        try
        {
            signatureBytes = signature.ToArray();
            messageBytes = message.ToArray();
            publicKeyBytes = publicKey.ToArray();
            return Ed25519.Verify(signatureBytes, messageBytes, publicKeyBytes);
        }
        finally
        {
            Clear(signatureBytes);
            Clear(messageBytes);
            Clear(publicKeyBytes);
        }
    }

    private static byte[] ValidatePrivateKeySeed(ReadOnlySpan<byte> privateKeySeed)
    {
        RequirePrivateKeySeedLength(privateKeySeed, nameof(privateKeySeed));

        return privateKeySeed.ToArray();
    }

    internal static void RequirePrivateKeySeedLength(ReadOnlySpan<byte> privateKeySeed, string paramName)
    {
        if (privateKeySeed.Length != PrivateKeySeedLength)
        {
            throw new ArgumentException($"private key seed must be {PrivateKeySeedLength} bytes", paramName);
        }
    }

    private static void Clear(byte[]? buffer)
    {
        if (buffer is not null)
        {
            CryptographicOperations.ZeroMemory(buffer);
        }
    }
}
