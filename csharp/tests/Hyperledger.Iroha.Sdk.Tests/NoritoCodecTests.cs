using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class NoritoCodecTests
{
    [Fact]
    public void SchemaHashUsesCanonicalTypeNameDomain()
    {
        var schemaHash = Convert.ToHexString(NoritoCodec.SchemaHash("iroha.test.Header")).ToLowerInvariant();

        Assert.Equal("ae7a510c0b77d4b4c8edb67c99d83baa", schemaHash);
    }

    [Fact]
    public void EncodeWithSchemaHashUsesProvidedSchemaHash()
    {
        var schemaHash = Convert.FromHexString("862a7d77075d4d23ff6c1261db027811");
        var payload = new byte[] { 1, 2, 3 };

        var encoded = NoritoCodec.EncodeWithSchemaHash(schemaHash, payload);

        Assert.Equal("862a7d77075d4d23ff6c1261db027811", Convert.ToHexString(encoded.AsSpan(6, 16)).ToLowerInvariant());
        Assert.Equal(payload, encoded[NoritoHeader.EncodedLength..]);
        Assert.Throws<ArgumentException>(() => NoritoCodec.EncodeWithSchemaHash(schemaHash[..15], payload));
    }
}
