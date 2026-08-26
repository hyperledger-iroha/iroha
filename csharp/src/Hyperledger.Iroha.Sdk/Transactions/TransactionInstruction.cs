using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Numeric;

namespace Hyperledger.Iroha.Transactions;

public abstract record class TransactionInstruction
{
    private static readonly byte[] InstructionBoxSchemaHash = Convert.FromHexString("862a7d77075d4d23ff6c1261db027811");

    internal abstract string WireId { get; }

    internal abstract string TypeName { get; }

    internal abstract byte[] EncodePayload(TransactionEncodingContext context);

    internal virtual byte[] EncodeFramedPayload(TransactionEncodingContext context)
    {
        return NoritoCodec.Encode(
            TypeName,
            EncodePayload(context),
            NoritoCodec.CanonicalLayoutFlags);
    }

    /// <summary>Encodes an instruction for the public Taira testnet.</summary>
    public byte[] EncodeInstructionBox(string authorityAccountId)
    {
        return EncodeInstructionBox(
            authorityAccountId,
            Address.AccountAddress.TairaTestnetChainDiscriminant);
    }

    public byte[] EncodeInstructionBox(string authorityAccountId, ushort chainDiscriminant)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(authorityAccountId);

        var context = new TransactionEncodingContext(authorityAccountId, chainDiscriminant);
        return Hyperledger.Iroha.Norito.NoritoCodec.EncodeWithSchemaHash(
            InstructionBoxSchemaHash,
            context.EncodeInstruction(this),
            NoritoCodec.CanonicalLayoutFlags);
    }

    /// <summary>Encodes an instruction for the public Taira testnet.</summary>
    public string EncodeInstructionBoxBase64(string authorityAccountId)
    {
        return EncodeInstructionBoxBase64(
            authorityAccountId,
            Address.AccountAddress.TairaTestnetChainDiscriminant);
    }

    public string EncodeInstructionBoxBase64(string authorityAccountId, ushort chainDiscriminant)
    {
        return Convert.ToBase64String(EncodeInstructionBox(authorityAccountId, chainDiscriminant));
    }

    public static TransferAssetInstruction TransferAsset(string assetDefinitionId, string quantity, string destinationAccountId)
    {
        return new TransferAssetInstruction(assetDefinitionId, quantity, destinationAccountId);
    }

    public static TransferAssetInstruction TransferAsset(
        string assetDefinitionId,
        NumericV1.QuantityValue quantity,
        string destinationAccountId)
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

    public static MintAssetInstruction MintAsset(
        string assetDefinitionId,
        NumericV1.QuantityValue quantity,
        string destinationAccountId)
    {
        return new MintAssetInstruction(assetDefinitionId, quantity, destinationAccountId);
    }

    public static BurnAssetInstruction BurnAsset(string assetDefinitionId, string quantity, string destinationAccountId)
    {
        return new BurnAssetInstruction(assetDefinitionId, quantity, destinationAccountId);
    }

    public static BurnAssetInstruction BurnAsset(
        string assetDefinitionId,
        NumericV1.QuantityValue quantity,
        string destinationAccountId)
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

    public static IssueReplicationOrderInstruction IssueReplicationOrder(
        string orderId,
        ReadOnlySpan<byte> orderPayload,
        ulong issuedEpoch,
        ulong deadlineEpoch,
        string? musubiArchiveId = null)
    {
        return new IssueReplicationOrderInstruction(
            orderId,
            orderPayload,
            issuedEpoch,
            deadlineEpoch,
            musubiArchiveId);
    }

    public static IssueReplicationOrderInstruction IssueReplicationOrder(
        string orderId,
        string orderPayloadBase64,
        ulong issuedEpoch,
        ulong deadlineEpoch,
        string? musubiArchiveId = null)
    {
        return new IssueReplicationOrderInstruction(
            orderId,
            orderPayloadBase64,
            issuedEpoch,
            deadlineEpoch,
            musubiArchiveId);
    }

    public static CompleteReplicationOrderInstruction CompleteReplicationOrder(
        string orderId,
        string providerId,
        ulong completionEpoch,
        ProviderIngestCompletionAuthorityV1 expectedAuthority,
        ulong expectedAssignmentRevision,
        ProviderIngestFinalizedAnchorV1 finalizedAnchor)
    {
        return new CompleteReplicationOrderInstruction(
            orderId,
            providerId,
            completionEpoch,
            expectedAuthority,
            expectedAssignmentRevision,
            finalizedAnchor);
    }

    public static ExpireReplicationOrderInstruction ExpireReplicationOrder(
        string orderId,
        ulong expirationEpoch)
    {
        return new ExpireReplicationOrderInstruction(orderId, expirationEpoch);
    }

    public static CancelAssetLockInstruction CancelAssetLock(
        string lockId,
        string expectedRemainingAmount)
    {
        return new CancelAssetLockInstruction(lockId, expectedRemainingAmount);
    }

    public static CancelAssetLockInstruction CancelAssetLock(
        string lockId,
        NumericV1.QuantityValue expectedRemainingAmount)
    {
        return new CancelAssetLockInstruction(lockId, expectedRemainingAmount);
    }

}
