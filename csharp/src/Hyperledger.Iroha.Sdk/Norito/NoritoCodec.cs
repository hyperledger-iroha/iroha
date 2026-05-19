using System.Security.Cryptography;
using System.Text;

namespace Hyperledger.Iroha.Norito;

public static class NoritoCodec
{
    private static readonly byte[] TypeNameSchemaHashDomain = Encoding.UTF8.GetBytes("norito:v1:type-name\0");

    public static byte[] Encode(string typeName, ReadOnlySpan<byte> payload, byte flags = 0)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(typeName);

        var schemaHash = SchemaHash(typeName);
        return EncodeWithSchemaHash(schemaHash, payload, flags);
    }

    public static byte[] EncodeWithSchemaHash(ReadOnlySpan<byte> schemaHash, ReadOnlySpan<byte> payload, byte flags = 0)
    {
        if (schemaHash.Length != 16)
        {
            throw new ArgumentException("Norito schema hash must be 16 bytes.", nameof(schemaHash));
        }

        var checksum = Crc64Ecma.Compute(payload);
        var header = new NoritoHeader(schemaHash.ToArray(), NoritoCompression.None, (ulong)payload.Length, checksum, flags);

        var encodedHeader = header.Encode();
        var output = new byte[encodedHeader.Length + payload.Length];
        encodedHeader.CopyTo(output, 0);
        payload.CopyTo(output.AsSpan(encodedHeader.Length));
        return output;
    }

    public static byte[] SchemaHash(string typeName)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(typeName);

        var typeNameBytes = Encoding.UTF8.GetBytes(typeName);
        var input = new byte[TypeNameSchemaHashDomain.Length + typeNameBytes.Length];
        TypeNameSchemaHashDomain.CopyTo(input, 0);
        typeNameBytes.CopyTo(input, TypeNameSchemaHashDomain.Length);
        return SHA256.HashData(input)[..16];
    }
}
