using Chaos.NaCl;

namespace Hyperledger.Iroha.Crypto;

public static class Ed25519Signer
{
    public const int PrivateKeySeedLength = 32;
    public const int PublicKeyLength = 32;
    public const int SignatureLength = 64;

    public static byte[] GetPublicKey(ReadOnlySpan<byte> privateKeySeed)
    {
        var seed = ValidatePrivateKeySeed(privateKeySeed);
        return Ed25519.PublicKeyFromSeed(seed);
    }

    public static byte[] Sign(ReadOnlySpan<byte> message, ReadOnlySpan<byte> privateKeySeed)
    {
        var seed = ValidatePrivateKeySeed(privateKeySeed);
        var expandedPrivateKey = Ed25519.ExpandedPrivateKeyFromSeed(seed);
        return Ed25519.Sign(message.ToArray(), expandedPrivateKey);
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

        return Ed25519.Verify(signature.ToArray(), message.ToArray(), publicKey.ToArray());
    }

    private static byte[] ValidatePrivateKeySeed(ReadOnlySpan<byte> privateKeySeed)
    {
        if (privateKeySeed.Length != PrivateKeySeedLength)
        {
            throw new ArgumentException($"private key seed must be {PrivateKeySeedLength} bytes", nameof(privateKeySeed));
        }

        return privateKeySeed.ToArray();
    }
}
