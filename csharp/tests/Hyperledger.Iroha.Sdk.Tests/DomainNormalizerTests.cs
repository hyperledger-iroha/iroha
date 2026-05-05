using Hyperledger.Iroha;
using Hyperledger.Iroha.Address;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class DomainNormalizerTests
{
    [Theory]
    [InlineData("Treasury", "treasury")]
    [InlineData("bücher.example", "xn--bcher-kva.example")]
    public void NormalizeLabelCanonicalizesExpectedInputs(string raw, string expected)
    {
        Assert.Equal(expected, DomainNormalizer.NormalizeLabel(raw));
    }

    [Theory]
    [InlineData("Treasury$default")]
    [InlineData("a..b")]
    [InlineData("例え．テスト")]
    [InlineData("wÍḷd-card")]
    public void NormalizeLabelRejectsInvalidInputs(string raw)
    {
        var exception = Assert.Throws<AccountAddressException>(() => DomainNormalizer.NormalizeLabel(raw));
        Assert.Equal(AccountAddressErrorCode.InvalidDomainLabel, exception.Code);
    }
}
