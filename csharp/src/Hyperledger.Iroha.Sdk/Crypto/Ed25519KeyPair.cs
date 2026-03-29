using System.Security.Cryptography;
using Hyperledger.Iroha.Address;

namespace Hyperledger.Iroha.Crypto;

public sealed class Ed25519KeyPair
{
    private readonly byte[] privateKeySeed;
    private readonly byte[] publicKey;

    private Ed25519KeyPair(byte[] privateKeySeed, byte[] publicKey)
    {
        this.privateKeySeed = privateKeySeed;
        this.publicKey = publicKey;
    }

    public byte[] PrivateKeySeed => [.. privateKeySeed];

    public byte[] PublicKey => [.. publicKey];

    public static Ed25519KeyPair FromSeed(ReadOnlySpan<byte> privateKeySeed)
    {
        var seed = privateKeySeed.ToArray();
        var publicKey = Ed25519Signer.GetPublicKey(seed);
        return new Ed25519KeyPair(seed, publicKey);
    }

    public static Ed25519KeyPair Generate()
    {
        return FromSeed(RandomNumberGenerator.GetBytes(Ed25519Signer.PrivateKeySeedLength));
    }

    public AccountAddress ToAccountAddress()
    {
        return AccountAddress.FromPublicKey(publicKey, "ed25519");
    }
}
