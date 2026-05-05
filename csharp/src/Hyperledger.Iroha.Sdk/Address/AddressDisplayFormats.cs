namespace Hyperledger.Iroha.Address;

public sealed class AddressDisplayFormats
{
    public AddressDisplayFormats(string i105, ushort chainDiscriminant, string i105Warning)
    {
        I105 = i105;
        ChainDiscriminant = chainDiscriminant;
        I105Warning = i105Warning;
    }

    public ushort ChainDiscriminant { get; }

    public string I105 { get; }

    public string I105Warning { get; }
}
