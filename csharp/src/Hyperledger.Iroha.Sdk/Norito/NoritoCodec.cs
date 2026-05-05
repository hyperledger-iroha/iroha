using System.Text;

namespace Hyperledger.Iroha.Norito;

public static class NoritoCodec
{
    public static byte[] Encode(string typeName, ReadOnlySpan<byte> payload, byte flags = 0)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(typeName);

        var schemaHash = SchemaHash(typeName);
        var checksum = Crc64Ecma.Compute(payload);
        var header = new NoritoHeader(schemaHash, NoritoCompression.None, (ulong)payload.Length, checksum, flags);

        var encodedHeader = header.Encode();
        var output = new byte[encodedHeader.Length + payload.Length];
        encodedHeader.CopyTo(output, 0);
        payload.CopyTo(output.AsSpan(encodedHeader.Length));
        return output;
    }

    public static byte[] SchemaHash(string typeName)
    {
        const ulong fnvOffset = 0xcbf29ce484222325UL;
        const ulong fnvPrime = 0x100000001b3UL;

        ulong hash = fnvOffset;
        foreach (var value in Encoding.UTF8.GetBytes(typeName))
        {
            hash ^= value;
            hash *= fnvPrime;
        }

        var bytes = new byte[16];
        var low = BitConverter.GetBytes(hash);
        low.CopyTo(bytes, 0);
        low.CopyTo(bytes, 8);
        return bytes;
    }
}
