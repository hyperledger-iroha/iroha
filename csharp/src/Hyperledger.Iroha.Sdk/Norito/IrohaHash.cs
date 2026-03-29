namespace Hyperledger.Iroha.Norito;

public static class IrohaHash
{
    public const int Length = 32;

    public static byte[] Hash(ReadOnlySpan<byte> data)
    {
        var digest = Blake2b.Hash256(data);
        digest[^1] |= 0x01;
        return digest;
    }
}
