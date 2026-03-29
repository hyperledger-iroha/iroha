using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Transactions;

public sealed record class TransferAssetInstruction(string AssetDefinitionId, string Quantity, string DestinationAccountId)
    : TransactionInstruction
{
    internal override string WireId => "iroha.transfer";

    internal override string TypeName => "iroha_data_model::isi::transfer::TransferBox";

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteUInt32LittleEndian(2);
        writer.WriteField(context.EncodeAssetId(AssetDefinitionId, context.AuthorityAccountId));
        writer.WriteField(context.EncodeNumeric(Quantity));
        writer.WriteField(context.EncodeAccountId(DestinationAccountId));
        return writer.ToArray();
    }
}

public sealed record class MintAssetInstruction(string AssetDefinitionId, string Quantity, string DestinationAccountId)
    : TransactionInstruction
{
    internal override string WireId => "iroha.mint";

    internal override string TypeName => "iroha_data_model::isi::mint_burn::MintBox";

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteUInt32LittleEndian(0);
        writer.WriteField(context.EncodeNumeric(Quantity));
        writer.WriteField(context.EncodeAssetId(AssetDefinitionId, DestinationAccountId));
        return writer.ToArray();
    }
}

public sealed record class BurnAssetInstruction(string AssetDefinitionId, string Quantity, string DestinationAccountId)
    : TransactionInstruction
{
    internal override string WireId => "iroha.burn";

    internal override string TypeName => "iroha_data_model::isi::mint_burn::BurnBox";

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteUInt32LittleEndian(0);
        writer.WriteField(context.EncodeNumeric(Quantity));
        writer.WriteField(context.EncodeAssetId(AssetDefinitionId, DestinationAccountId));
        return writer.ToArray();
    }
}
