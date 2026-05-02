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
}
