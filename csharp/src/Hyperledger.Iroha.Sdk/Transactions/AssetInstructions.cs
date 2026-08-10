using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Numeric;
using System.Text.Json.Nodes;

namespace Hyperledger.Iroha.Transactions;

public sealed record class TransferAssetInstruction(string AssetDefinitionId, string Quantity, string DestinationAccountId)
    : TransactionInstruction
{
    private string destinationAccountId = TransactionEncodingContext.CanonicalizeAccountId(
        DestinationAccountId,
        nameof(DestinationAccountId));
    private NumericV1.QuantityValue quantity = AssetQuantityValidation.RequireCanonicalQuantity(
        Quantity,
        nameof(Quantity));

    public TransferAssetInstruction(
        string assetDefinitionId,
        NumericV1.QuantityValue quantity,
        string destinationAccountId)
        : this(
            assetDefinitionId,
            AssetQuantityValidation.RequireQuantity(quantity, nameof(quantity)).ToString(),
            destinationAccountId)
    {
    }

    public string DestinationAccountId
    {
        get => destinationAccountId;
        init => destinationAccountId = TransactionEncodingContext.CanonicalizeAccountId(
            value,
            nameof(DestinationAccountId));
    }

    public string Quantity
    {
        get => quantity.ToString();
        init => quantity = AssetQuantityValidation.RequireCanonicalQuantity(value, nameof(Quantity));
    }

    public NumericV1.QuantityValue QuantityValue => quantity;

    internal override string WireId => "iroha.transfer";

    internal override string TypeName => "iroha_data_model::isi::transfer::TransferBox";

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteUInt32LittleEndian(2);
        writer.WriteField(context.EncodeAssetId(AssetDefinitionId, context.AuthorityAccountId));
        writer.WriteField(context.EncodeQuantity(quantity));
        writer.WriteField(context.EncodeAccountId(DestinationAccountId));
        return writer.ToArray();
    }
}

public sealed record class TransferDomainInstruction(string DomainId, string DestinationAccountId)
    : TransactionInstruction
{
    private const uint DomainVariant = 0;

    private string destinationAccountId = TransactionEncodingContext.CanonicalizeAccountId(
        DestinationAccountId,
        nameof(DestinationAccountId));

    public string DestinationAccountId
    {
        get => destinationAccountId;
        init => destinationAccountId = TransactionEncodingContext.CanonicalizeAccountId(
            value,
            nameof(DestinationAccountId));
    }

    internal override string WireId => "iroha.transfer";

    internal override string TypeName => "iroha_data_model::isi::transfer::TransferBox";

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
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

    private string destinationAccountId = TransactionEncodingContext.CanonicalizeAccountId(
        DestinationAccountId,
        nameof(DestinationAccountId));

    public string DestinationAccountId
    {
        get => destinationAccountId;
        init => destinationAccountId = TransactionEncodingContext.CanonicalizeAccountId(
            value,
            nameof(DestinationAccountId));
    }

    internal override string WireId => "iroha.transfer";

    internal override string TypeName => "iroha_data_model::isi::transfer::TransferBox";

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
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

    private string destinationAccountId = TransactionEncodingContext.CanonicalizeAccountId(
        DestinationAccountId,
        nameof(DestinationAccountId));

    public string DestinationAccountId
    {
        get => destinationAccountId;
        init => destinationAccountId = TransactionEncodingContext.CanonicalizeAccountId(
            value,
            nameof(DestinationAccountId));
    }

    internal override string WireId => "iroha.transfer";

    internal override string TypeName => "iroha_data_model::isi::transfer::TransferBox";

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
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
    private string destinationAccountId = TransactionEncodingContext.CanonicalizeAccountId(
        DestinationAccountId,
        nameof(DestinationAccountId));
    private NumericV1.QuantityValue quantity = AssetQuantityValidation.RequireCanonicalQuantity(
        Quantity,
        nameof(Quantity));

    public MintAssetInstruction(
        string assetDefinitionId,
        NumericV1.QuantityValue quantity,
        string destinationAccountId)
        : this(
            assetDefinitionId,
            AssetQuantityValidation.RequireQuantity(quantity, nameof(quantity)).ToString(),
            destinationAccountId)
    {
    }

    public string DestinationAccountId
    {
        get => destinationAccountId;
        init => destinationAccountId = TransactionEncodingContext.CanonicalizeAccountId(
            value,
            nameof(DestinationAccountId));
    }

    public string Quantity
    {
        get => quantity.ToString();
        init => quantity = AssetQuantityValidation.RequireCanonicalQuantity(value, nameof(Quantity));
    }

    public NumericV1.QuantityValue QuantityValue => quantity;

    internal override string WireId => "iroha.mint";

    internal override string TypeName => "iroha_data_model::isi::mint_burn::MintBox";

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteUInt32LittleEndian(0);
        writer.WriteField(context.EncodeQuantity(quantity));
        writer.WriteField(context.EncodeAssetId(AssetDefinitionId, DestinationAccountId));
        return writer.ToArray();
    }
}

public sealed record class BurnAssetInstruction(string AssetDefinitionId, string Quantity, string DestinationAccountId)
    : TransactionInstruction
{
    private string destinationAccountId = TransactionEncodingContext.CanonicalizeAccountId(
        DestinationAccountId,
        nameof(DestinationAccountId));
    private NumericV1.QuantityValue quantity = AssetQuantityValidation.RequireCanonicalQuantity(
        Quantity,
        nameof(Quantity));

    public BurnAssetInstruction(
        string assetDefinitionId,
        NumericV1.QuantityValue quantity,
        string destinationAccountId)
        : this(
            assetDefinitionId,
            AssetQuantityValidation.RequireQuantity(quantity, nameof(quantity)).ToString(),
            destinationAccountId)
    {
    }

    public string DestinationAccountId
    {
        get => destinationAccountId;
        init => destinationAccountId = TransactionEncodingContext.CanonicalizeAccountId(
            value,
            nameof(DestinationAccountId));
    }

    public string Quantity
    {
        get => quantity.ToString();
        init => quantity = AssetQuantityValidation.RequireCanonicalQuantity(value, nameof(Quantity));
    }

    public NumericV1.QuantityValue QuantityValue => quantity;

    internal override string WireId => "iroha.burn";

    internal override string TypeName => "iroha_data_model::isi::mint_burn::BurnBox";

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteUInt32LittleEndian(0);
        writer.WriteField(context.EncodeQuantity(quantity));
        writer.WriteField(context.EncodeAssetId(AssetDefinitionId, DestinationAccountId));
        return writer.ToArray();
    }
}

internal static class AssetQuantityValidation
{
    internal static NumericV1.QuantityValue RequireCanonicalQuantity(string? quantity, string paramName)
    {
        if (quantity is null)
        {
            throw new ArgumentException("Asset quantity must not be null.", paramName);
        }

        try
        {
            return NumericV1.QuantityValue.ParseCanonical(quantity);
        }
        catch (NumericV1.NumericException exception)
        {
            throw new ArgumentException(
                $"Asset quantity must be a canonical non-negative V1 quantity: {exception.Message}",
                paramName,
                exception);
        }
    }

    internal static NumericV1.QuantityValue RequireQuantity(
        NumericV1.QuantityValue? quantity,
        string paramName)
    {
        return quantity ?? throw new ArgumentNullException(paramName);
    }
}

internal static class InstructionJsonPayload
{
    internal static JsonNode? Clone(JsonNode? value) => value?.DeepClone();
}

public sealed record class SetAssetKeyValueInstruction(
    string AssetDefinitionId,
    string AccountId,
    string Key,
    JsonNode? Value)
    : TransactionInstruction
{
    private const string InstructionTypeName = "iroha_data_model::isi::transparent::SetAssetKeyValue";

    private string accountId = TransactionEncodingContext.CanonicalizeAccountId(
        AccountId,
        nameof(AccountId));
    private JsonNode? jsonValue = InstructionJsonPayload.Clone(Value);

    public string AccountId
    {
        get => accountId;
        init => accountId = TransactionEncodingContext.CanonicalizeAccountId(value, nameof(AccountId));
    }

    public JsonNode? Value
    {
        get => InstructionJsonPayload.Clone(jsonValue);
        init => jsonValue = InstructionJsonPayload.Clone(value);
    }

    internal override string WireId => InstructionTypeName;

    internal override string TypeName => InstructionTypeName;

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteField(context.EncodeAssetId(AssetDefinitionId, AccountId));
        writer.WriteField(context.EncodeName(Key));
        writer.WriteField(context.EncodeJson(jsonValue));
        return writer.ToArray();
    }
}

public sealed record class RemoveAssetKeyValueInstruction(string AssetDefinitionId, string AccountId, string Key)
    : TransactionInstruction
{
    private const string InstructionTypeName = "iroha_data_model::isi::transparent::RemoveAssetKeyValue";

    private string accountId = TransactionEncodingContext.CanonicalizeAccountId(
        AccountId,
        nameof(AccountId));

    public string AccountId
    {
        get => accountId;
        init => accountId = TransactionEncodingContext.CanonicalizeAccountId(value, nameof(AccountId));
    }

    internal override string WireId => InstructionTypeName;

    internal override string TypeName => InstructionTypeName;

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
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

    private string accountId = TransactionEncodingContext.CanonicalizeAccountId(
        AccountId,
        nameof(AccountId));
    private JsonNode? jsonValue = InstructionJsonPayload.Clone(Value);

    public string AccountId
    {
        get => accountId;
        init => accountId = TransactionEncodingContext.CanonicalizeAccountId(value, nameof(AccountId));
    }

    public JsonNode? Value
    {
        get => InstructionJsonPayload.Clone(jsonValue);
        init => jsonValue = InstructionJsonPayload.Clone(value);
    }

    internal override string WireId => InstructionWireId;

    internal override string TypeName => InstructionTypeName;

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteUInt32LittleEndian(AccountVariant);
        writer.WriteField(context.EncodeAccountId(AccountId));
        writer.WriteField(context.EncodeName(Key));
        writer.WriteField(context.EncodeJson(jsonValue));
        return writer.ToArray();
    }
}

public sealed record class SetDomainKeyValueInstruction(string DomainId, string Key, JsonNode? Value)
    : TransactionInstruction
{
    private const string InstructionWireId = "iroha.set_key_value";
    private const string InstructionTypeName = "iroha_data_model::isi::SetKeyValueBox";
    private const uint DomainVariant = 0;
    private JsonNode? jsonValue = InstructionJsonPayload.Clone(Value);

    public JsonNode? Value
    {
        get => InstructionJsonPayload.Clone(jsonValue);
        init => jsonValue = InstructionJsonPayload.Clone(value);
    }

    internal override string WireId => InstructionWireId;

    internal override string TypeName => InstructionTypeName;

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteUInt32LittleEndian(DomainVariant);
        writer.WriteField(context.EncodeName(DomainId));
        writer.WriteField(context.EncodeName(Key));
        writer.WriteField(context.EncodeJson(jsonValue));
        return writer.ToArray();
    }
}

public sealed record class RemoveAccountKeyValueInstruction(string AccountId, string Key)
    : TransactionInstruction
{
    private const string InstructionWireId = "iroha.remove_key_value";
    private const string InstructionTypeName = "iroha_data_model::isi::RemoveKeyValueBox";
    private const uint AccountVariant = 1;

    private string accountId = TransactionEncodingContext.CanonicalizeAccountId(
        AccountId,
        nameof(AccountId));

    public string AccountId
    {
        get => accountId;
        init => accountId = TransactionEncodingContext.CanonicalizeAccountId(value, nameof(AccountId));
    }

    internal override string WireId => InstructionWireId;

    internal override string TypeName => InstructionTypeName;

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
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
        var writer = new CanonicalNoritoWriter();
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
    private JsonNode? jsonValue = InstructionJsonPayload.Clone(Value);

    public JsonNode? Value
    {
        get => InstructionJsonPayload.Clone(jsonValue);
        init => jsonValue = InstructionJsonPayload.Clone(value);
    }

    internal override string WireId => InstructionWireId;

    internal override string TypeName => InstructionTypeName;

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteUInt32LittleEndian(AssetDefinitionVariant);
        writer.WriteField(context.EncodeAssetDefinitionId(AssetDefinitionId));
        writer.WriteField(context.EncodeName(Key));
        writer.WriteField(context.EncodeJson(jsonValue));
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
        var writer = new CanonicalNoritoWriter();
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
    private JsonNode? jsonValue = InstructionJsonPayload.Clone(Value);

    public JsonNode? Value
    {
        get => InstructionJsonPayload.Clone(jsonValue);
        init => jsonValue = InstructionJsonPayload.Clone(value);
    }

    internal override string WireId => InstructionWireId;

    internal override string TypeName => InstructionTypeName;

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteUInt32LittleEndian(NftVariant);
        writer.WriteField(context.EncodeNftId(NftId));
        writer.WriteField(context.EncodeName(Key));
        writer.WriteField(context.EncodeJson(jsonValue));
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
        var writer = new CanonicalNoritoWriter();
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
    private JsonNode? jsonValue = InstructionJsonPayload.Clone(Value);

    public JsonNode? Value
    {
        get => InstructionJsonPayload.Clone(jsonValue);
        init => jsonValue = InstructionJsonPayload.Clone(value);
    }

    internal override string WireId => InstructionWireId;

    internal override string TypeName => InstructionTypeName;

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteUInt32LittleEndian(TriggerVariant);
        writer.WriteField(context.EncodeTriggerId(TriggerId));
        writer.WriteField(context.EncodeName(Key));
        writer.WriteField(context.EncodeJson(jsonValue));
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
        var writer = new CanonicalNoritoWriter();
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
    private uint repetitions = RequirePositiveRepetitions(Repetitions, nameof(Repetitions));

    public uint Repetitions
    {
        get => repetitions;
        init => repetitions = RequirePositiveRepetitions(value, nameof(Repetitions));
    }

    internal override string WireId => "iroha.mint";

    internal override string TypeName => "iroha_data_model::isi::mint_burn::MintBox";

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteUInt32LittleEndian(TriggerRepetitionsVariant);
        writer.WriteField(context.EncodeUInt32(Repetitions));
        writer.WriteField(context.EncodeTriggerId(TriggerId));
        return writer.ToArray();
    }

    private static uint RequirePositiveRepetitions(uint repetitions, string paramName)
    {
        if (repetitions == 0)
        {
            throw new ArgumentOutOfRangeException(paramName, "Trigger repetitions must be positive.");
        }

        return repetitions;
    }
}

public sealed record class BurnTriggerRepetitionsInstruction(uint Repetitions, string TriggerId)
    : TransactionInstruction
{
    private const uint TriggerRepetitionsVariant = 1;
    private uint repetitions = RequirePositiveRepetitions(Repetitions, nameof(Repetitions));

    public uint Repetitions
    {
        get => repetitions;
        init => repetitions = RequirePositiveRepetitions(value, nameof(Repetitions));
    }

    internal override string WireId => "iroha.burn";

    internal override string TypeName => "iroha_data_model::isi::mint_burn::BurnBox";

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteUInt32LittleEndian(TriggerRepetitionsVariant);
        writer.WriteField(context.EncodeUInt32(Repetitions));
        writer.WriteField(context.EncodeTriggerId(TriggerId));
        return writer.ToArray();
    }

    private static uint RequirePositiveRepetitions(uint repetitions, string paramName)
    {
        if (repetitions == 0)
        {
            throw new ArgumentOutOfRangeException(paramName, "Trigger repetitions must be positive.");
        }

        return repetitions;
    }
}

public sealed record class ExecuteTriggerInstruction(string TriggerId, JsonNode? Args)
    : TransactionInstruction
{
    private JsonNode? args = InstructionJsonPayload.Clone(Args);

    public JsonNode? Args
    {
        get => InstructionJsonPayload.Clone(args);
        init => args = InstructionJsonPayload.Clone(value);
    }

    internal override string WireId => "iroha.execute_trigger";

    internal override string TypeName => "iroha_data_model::isi::transparent::ExecuteTrigger";

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteField(context.EncodeTriggerId(TriggerId));
        writer.WriteField(context.EncodeJson(args));
        return writer.ToArray();
    }
}
