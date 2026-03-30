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

    public static TransferDomainInstruction TransferDomain(string domainId, string destinationAccountId)
    {
        return new TransferDomainInstruction(domainId, destinationAccountId);
    }

    public static TransferAssetDefinitionInstruction TransferAssetDefinition(
        string assetDefinitionId,
        string destinationAccountId)
    {
        return new TransferAssetDefinitionInstruction(assetDefinitionId, destinationAccountId);
    }

    public static TransferNftInstruction TransferNft(string nftId, string destinationAccountId)
    {
        return new TransferNftInstruction(nftId, destinationAccountId);
    }

    public static MintAssetInstruction MintAsset(string assetDefinitionId, string quantity, string destinationAccountId)
    {
        return new MintAssetInstruction(assetDefinitionId, quantity, destinationAccountId);
    }

    public static BurnAssetInstruction BurnAsset(string assetDefinitionId, string quantity, string destinationAccountId)
    {
        return new BurnAssetInstruction(assetDefinitionId, quantity, destinationAccountId);
    }

    public static SetAssetKeyValueInstruction SetAssetKeyValue(
        string assetDefinitionId,
        string accountId,
        string key,
        System.Text.Json.Nodes.JsonNode? value)
    {
        return new SetAssetKeyValueInstruction(assetDefinitionId, accountId, key, value);
    }

    public static RemoveAssetKeyValueInstruction RemoveAssetKeyValue(
        string assetDefinitionId,
        string accountId,
        string key)
    {
        return new RemoveAssetKeyValueInstruction(assetDefinitionId, accountId, key);
    }

    public static SetAccountKeyValueInstruction SetAccountKeyValue(
        string accountId,
        string key,
        System.Text.Json.Nodes.JsonNode? value)
    {
        return new SetAccountKeyValueInstruction(accountId, key, value);
    }

    public static SetDomainKeyValueInstruction SetDomainKeyValue(
        string domainId,
        string key,
        System.Text.Json.Nodes.JsonNode? value)
    {
        return new SetDomainKeyValueInstruction(domainId, key, value);
    }

    public static RemoveAccountKeyValueInstruction RemoveAccountKeyValue(
        string accountId,
        string key)
    {
        return new RemoveAccountKeyValueInstruction(accountId, key);
    }

    public static RemoveDomainKeyValueInstruction RemoveDomainKeyValue(
        string domainId,
        string key)
    {
        return new RemoveDomainKeyValueInstruction(domainId, key);
    }

    public static SetAssetDefinitionKeyValueInstruction SetAssetDefinitionKeyValue(
        string assetDefinitionId,
        string key,
        System.Text.Json.Nodes.JsonNode? value)
    {
        return new SetAssetDefinitionKeyValueInstruction(assetDefinitionId, key, value);
    }

    public static RemoveAssetDefinitionKeyValueInstruction RemoveAssetDefinitionKeyValue(
        string assetDefinitionId,
        string key)
    {
        return new RemoveAssetDefinitionKeyValueInstruction(assetDefinitionId, key);
    }

    public static SetNftKeyValueInstruction SetNftKeyValue(
        string nftId,
        string key,
        System.Text.Json.Nodes.JsonNode? value)
    {
        return new SetNftKeyValueInstruction(nftId, key, value);
    }

    public static RemoveNftKeyValueInstruction RemoveNftKeyValue(
        string nftId,
        string key)
    {
        return new RemoveNftKeyValueInstruction(nftId, key);
    }

    public static SetTriggerKeyValueInstruction SetTriggerKeyValue(
        string triggerId,
        string key,
        System.Text.Json.Nodes.JsonNode? value)
    {
        return new SetTriggerKeyValueInstruction(triggerId, key, value);
    }

    public static RemoveTriggerKeyValueInstruction RemoveTriggerKeyValue(
        string triggerId,
        string key)
    {
        return new RemoveTriggerKeyValueInstruction(triggerId, key);
    }

    public static MintTriggerRepetitionsInstruction MintTriggerRepetitions(uint repetitions, string triggerId)
    {
        return new MintTriggerRepetitionsInstruction(repetitions, triggerId);
    }

    public static BurnTriggerRepetitionsInstruction BurnTriggerRepetitions(uint repetitions, string triggerId)
    {
        return new BurnTriggerRepetitionsInstruction(repetitions, triggerId);
    }

    public static ExecuteTriggerInstruction ExecuteTrigger(
        string triggerId,
        System.Text.Json.Nodes.JsonNode? args = null)
    {
        return new ExecuteTriggerInstruction(triggerId, args);
    }
}
