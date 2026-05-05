using Hyperledger.Iroha.Norito;
using System.Text.Json.Nodes;

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

public sealed record class TransferDomainInstruction(string DomainId, string DestinationAccountId)
    : TransactionInstruction
{
    private const uint DomainVariant = 0;

    internal override string WireId => "iroha.transfer";

    internal override string TypeName => "iroha_data_model::isi::transfer::TransferBox";

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteUInt32LittleEndian(DomainVariant);
        writer.WriteField(context.EncodeAccountId(context.AuthorityAccountId));
        writer.WriteField(context.EncodeName(DomainId));
        writer.WriteField(context.EncodeAccountId(DestinationAccountId));
        return writer.ToArray();
    }
}

public sealed record class TransferAssetDefinitionInstruction(string AssetDefinitionId, string DestinationAccountId)
    : TransactionInstruction
{
    private const uint AssetDefinitionVariant = 1;

    internal override string WireId => "iroha.transfer";

    internal override string TypeName => "iroha_data_model::isi::transfer::TransferBox";

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteUInt32LittleEndian(AssetDefinitionVariant);
        writer.WriteField(context.EncodeAccountId(context.AuthorityAccountId));
        writer.WriteField(context.EncodeAssetDefinitionId(AssetDefinitionId));
        writer.WriteField(context.EncodeAccountId(DestinationAccountId));
        return writer.ToArray();
    }
}

public sealed record class TransferNftInstruction(string NftId, string DestinationAccountId)
    : TransactionInstruction
{
    private const uint NftVariant = 3;

    internal override string WireId => "iroha.transfer";

    internal override string TypeName => "iroha_data_model::isi::transfer::TransferBox";

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteUInt32LittleEndian(NftVariant);
        writer.WriteField(context.EncodeAccountId(context.AuthorityAccountId));
        writer.WriteField(context.EncodeNftId(NftId));
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

public sealed record class SetAssetKeyValueInstruction(
    string AssetDefinitionId,
    string AccountId,
    string Key,
    JsonNode? Value)
    : TransactionInstruction
{
    private const string InstructionTypeName = "iroha_data_model::isi::transparent::SetAssetKeyValue";

    internal override string WireId => InstructionTypeName;

    internal override string TypeName => InstructionTypeName;

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(context.EncodeAssetId(AssetDefinitionId, AccountId));
        writer.WriteField(context.EncodeName(Key));
        writer.WriteField(context.EncodeJson(Value));
        return writer.ToArray();
    }
}

public sealed record class RemoveAssetKeyValueInstruction(string AssetDefinitionId, string AccountId, string Key)
    : TransactionInstruction
{
    private const string InstructionTypeName = "iroha_data_model::isi::transparent::RemoveAssetKeyValue";

    internal override string WireId => InstructionTypeName;

    internal override string TypeName => InstructionTypeName;

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(context.EncodeAssetId(AssetDefinitionId, AccountId));
        writer.WriteField(context.EncodeName(Key));
        return writer.ToArray();
    }
}

public sealed record class SetAccountKeyValueInstruction(string AccountId, string Key, JsonNode? Value)
    : TransactionInstruction
{
    private const string InstructionWireId = "iroha.set_key_value";
    private const string InstructionTypeName = "iroha_data_model::isi::SetKeyValueBox";
    private const uint AccountVariant = 1;

    internal override string WireId => InstructionWireId;

    internal override string TypeName => InstructionTypeName;

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteUInt32LittleEndian(AccountVariant);
        writer.WriteField(context.EncodeAccountId(AccountId));
        writer.WriteField(context.EncodeName(Key));
        writer.WriteField(context.EncodeJson(Value));
        return writer.ToArray();
    }
}

public sealed record class SetDomainKeyValueInstruction(string DomainId, string Key, JsonNode? Value)
    : TransactionInstruction
{
    private const string InstructionWireId = "iroha.set_key_value";
    private const string InstructionTypeName = "iroha_data_model::isi::SetKeyValueBox";
    private const uint DomainVariant = 0;

    internal override string WireId => InstructionWireId;

    internal override string TypeName => InstructionTypeName;

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteUInt32LittleEndian(DomainVariant);
        writer.WriteField(context.EncodeName(DomainId));
        writer.WriteField(context.EncodeName(Key));
        writer.WriteField(context.EncodeJson(Value));
        return writer.ToArray();
    }
}

public sealed record class RemoveAccountKeyValueInstruction(string AccountId, string Key)
    : TransactionInstruction
{
    private const string InstructionWireId = "iroha.remove_key_value";
    private const string InstructionTypeName = "iroha_data_model::isi::RemoveKeyValueBox";
    private const uint AccountVariant = 1;

    internal override string WireId => InstructionWireId;

    internal override string TypeName => InstructionTypeName;

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteUInt32LittleEndian(AccountVariant);
        writer.WriteField(context.EncodeAccountId(AccountId));
        writer.WriteField(context.EncodeName(Key));
        return writer.ToArray();
    }
}

public sealed record class RemoveDomainKeyValueInstruction(string DomainId, string Key)
    : TransactionInstruction
{
    private const string InstructionWireId = "iroha.remove_key_value";
    private const string InstructionTypeName = "iroha_data_model::isi::RemoveKeyValueBox";
    private const uint DomainVariant = 0;

    internal override string WireId => InstructionWireId;

    internal override string TypeName => InstructionTypeName;

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteUInt32LittleEndian(DomainVariant);
        writer.WriteField(context.EncodeName(DomainId));
        writer.WriteField(context.EncodeName(Key));
        return writer.ToArray();
    }
}

public sealed record class SetAssetDefinitionKeyValueInstruction(string AssetDefinitionId, string Key, JsonNode? Value)
    : TransactionInstruction
{
    private const string InstructionWireId = "iroha.set_key_value";
    private const string InstructionTypeName = "iroha_data_model::isi::SetKeyValueBox";
    private const uint AssetDefinitionVariant = 2;

    internal override string WireId => InstructionWireId;

    internal override string TypeName => InstructionTypeName;

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteUInt32LittleEndian(AssetDefinitionVariant);
        writer.WriteField(context.EncodeAssetDefinitionId(AssetDefinitionId));
        writer.WriteField(context.EncodeName(Key));
        writer.WriteField(context.EncodeJson(Value));
        return writer.ToArray();
    }
}

public sealed record class RemoveAssetDefinitionKeyValueInstruction(string AssetDefinitionId, string Key)
    : TransactionInstruction
{
    private const string InstructionWireId = "iroha.remove_key_value";
    private const string InstructionTypeName = "iroha_data_model::isi::RemoveKeyValueBox";
    private const uint AssetDefinitionVariant = 2;

    internal override string WireId => InstructionWireId;

    internal override string TypeName => InstructionTypeName;

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteUInt32LittleEndian(AssetDefinitionVariant);
        writer.WriteField(context.EncodeAssetDefinitionId(AssetDefinitionId));
        writer.WriteField(context.EncodeName(Key));
        return writer.ToArray();
    }
}

public sealed record class SetNftKeyValueInstruction(string NftId, string Key, JsonNode? Value)
    : TransactionInstruction
{
    private const string InstructionWireId = "iroha.set_key_value";
    private const string InstructionTypeName = "iroha_data_model::isi::SetKeyValueBox";
    private const uint NftVariant = 3;

    internal override string WireId => InstructionWireId;

    internal override string TypeName => InstructionTypeName;

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteUInt32LittleEndian(NftVariant);
        writer.WriteField(context.EncodeNftId(NftId));
        writer.WriteField(context.EncodeName(Key));
        writer.WriteField(context.EncodeJson(Value));
        return writer.ToArray();
    }
}

public sealed record class RemoveNftKeyValueInstruction(string NftId, string Key)
    : TransactionInstruction
{
    private const string InstructionWireId = "iroha.remove_key_value";
    private const string InstructionTypeName = "iroha_data_model::isi::RemoveKeyValueBox";
    private const uint NftVariant = 3;

    internal override string WireId => InstructionWireId;

    internal override string TypeName => InstructionTypeName;

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteUInt32LittleEndian(NftVariant);
        writer.WriteField(context.EncodeNftId(NftId));
        writer.WriteField(context.EncodeName(Key));
        return writer.ToArray();
    }
}

public sealed record class SetTriggerKeyValueInstruction(string TriggerId, string Key, JsonNode? Value)
    : TransactionInstruction
{
    private const string InstructionWireId = "iroha.set_key_value";
    private const string InstructionTypeName = "iroha_data_model::isi::SetKeyValueBox";
    private const uint TriggerVariant = 4;

    internal override string WireId => InstructionWireId;

    internal override string TypeName => InstructionTypeName;

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteUInt32LittleEndian(TriggerVariant);
        writer.WriteField(context.EncodeTriggerId(TriggerId));
        writer.WriteField(context.EncodeName(Key));
        writer.WriteField(context.EncodeJson(Value));
        return writer.ToArray();
    }
}

public sealed record class RemoveTriggerKeyValueInstruction(string TriggerId, string Key)
    : TransactionInstruction
{
    private const string InstructionWireId = "iroha.remove_key_value";
    private const string InstructionTypeName = "iroha_data_model::isi::RemoveKeyValueBox";
    private const uint TriggerVariant = 4;

    internal override string WireId => InstructionWireId;

    internal override string TypeName => InstructionTypeName;

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteUInt32LittleEndian(TriggerVariant);
        writer.WriteField(context.EncodeTriggerId(TriggerId));
        writer.WriteField(context.EncodeName(Key));
        return writer.ToArray();
    }
}

public sealed record class MintTriggerRepetitionsInstruction(uint Repetitions, string TriggerId)
    : TransactionInstruction
{
    private const uint TriggerRepetitionsVariant = 1;

    internal override string WireId => "iroha.mint";

    internal override string TypeName => "iroha_data_model::isi::mint_burn::MintBox";

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteUInt32LittleEndian(TriggerRepetitionsVariant);
        writer.WriteField(context.EncodeUInt32(Repetitions));
        writer.WriteField(context.EncodeTriggerId(TriggerId));
        return writer.ToArray();
    }
}

public sealed record class BurnTriggerRepetitionsInstruction(uint Repetitions, string TriggerId)
    : TransactionInstruction
{
    private const uint TriggerRepetitionsVariant = 1;

    internal override string WireId => "iroha.burn";

    internal override string TypeName => "iroha_data_model::isi::mint_burn::BurnBox";

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteUInt32LittleEndian(TriggerRepetitionsVariant);
        writer.WriteField(context.EncodeUInt32(Repetitions));
        writer.WriteField(context.EncodeTriggerId(TriggerId));
        return writer.ToArray();
    }
}

public sealed record class ExecuteTriggerInstruction(string TriggerId, JsonNode? Args)
    : TransactionInstruction
{
    internal override string WireId => "iroha.execute_trigger";

    internal override string TypeName => "iroha_data_model::isi::transparent::ExecuteTrigger";

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new OfflineNoritoWriter();
        writer.WriteField(context.EncodeTriggerId(TriggerId));
        writer.WriteField(context.EncodeJson(Args));
        return writer.ToArray();
    }
}
