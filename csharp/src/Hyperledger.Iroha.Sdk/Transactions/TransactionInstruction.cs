namespace Hyperledger.Iroha.Transactions;

public abstract record class TransactionInstruction
{
    internal abstract string WireId { get; }

    internal abstract string TypeName { get; }

    internal abstract byte[] EncodePayload(TransactionEncodingContext context);

    public static TransferAssetInstruction TransferAsset(string assetDefinitionId, string quantity, string destinationAccountId)
    {
        return new TransferAssetInstruction(assetDefinitionId, quantity, destinationAccountId);
    }

    public static MintAssetInstruction MintAsset(string assetDefinitionId, string quantity, string destinationAccountId)
    {
        return new MintAssetInstruction(assetDefinitionId, quantity, destinationAccountId);
    }

    public static BurnAssetInstruction BurnAsset(string assetDefinitionId, string quantity, string destinationAccountId)
    {
        return new BurnAssetInstruction(assetDefinitionId, quantity, destinationAccountId);
    }
}
