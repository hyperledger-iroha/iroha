using System.Text.Json.Nodes;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Transactions;

public sealed class TransactionBuilder
{
    private readonly List<TransactionInstruction> instructions = [];
    private readonly Dictionary<string, JsonNode?> metadata = new(StringComparer.Ordinal);

    public TransactionBuilder(string chainId, string authorityAccountId)
    {
        ChainId = NormalizeRequiredValue(chainId, nameof(chainId));
        AuthorityAccountId = NormalizeRequiredValue(authorityAccountId, nameof(authorityAccountId));
    }

    public string ChainId { get; }

    public string AuthorityAccountId { get; }

    public ulong? CreationTimeMilliseconds { get; private set; }

    public ulong? TimeToLiveMilliseconds { get; private set; }

    public uint? Nonce { get; private set; }

    public IReadOnlyList<TransactionInstruction> Instructions => instructions;

    public IReadOnlyDictionary<string, JsonNode?> Metadata => metadata;

    public TransactionBuilder AddInstruction(TransactionInstruction instruction)
    {
        ArgumentNullException.ThrowIfNull(instruction);
        instructions.Add(instruction);
        return this;
    }

    public TransactionBuilder TransferAsset(string assetDefinitionId, string quantity, string destinationAccountId)
    {
        return AddInstruction(TransactionInstruction.TransferAsset(assetDefinitionId, quantity, destinationAccountId));
    }

    public TransactionBuilder TransferDomain(string domainId, string destinationAccountId)
    {
        return AddInstruction(TransactionInstruction.TransferDomain(domainId, destinationAccountId));
    }

    public TransactionBuilder TransferAssetDefinition(string assetDefinitionId, string destinationAccountId)
    {
        return AddInstruction(TransactionInstruction.TransferAssetDefinition(assetDefinitionId, destinationAccountId));
    }

    public TransactionBuilder TransferNft(string nftId, string destinationAccountId)
    {
        return AddInstruction(TransactionInstruction.TransferNft(nftId, destinationAccountId));
    }

    public TransactionBuilder MintAsset(string assetDefinitionId, string quantity, string destinationAccountId)
    {
        return AddInstruction(TransactionInstruction.MintAsset(assetDefinitionId, quantity, destinationAccountId));
    }

    public TransactionBuilder BurnAsset(string assetDefinitionId, string quantity, string destinationAccountId)
    {
        return AddInstruction(TransactionInstruction.BurnAsset(assetDefinitionId, quantity, destinationAccountId));
    }

    public TransactionBuilder SetAssetKeyValue(string assetDefinitionId, string accountId, string key, JsonNode? value)
    {
        return AddInstruction(TransactionInstruction.SetAssetKeyValue(assetDefinitionId, accountId, key, value));
    }

    public TransactionBuilder RemoveAssetKeyValue(string assetDefinitionId, string accountId, string key)
    {
        return AddInstruction(TransactionInstruction.RemoveAssetKeyValue(assetDefinitionId, accountId, key));
    }

    public TransactionBuilder SetAccountKeyValue(string accountId, string key, JsonNode? value)
    {
        return AddInstruction(TransactionInstruction.SetAccountKeyValue(accountId, key, value));
    }

    public TransactionBuilder SetDomainKeyValue(string domainId, string key, JsonNode? value)
    {
        return AddInstruction(TransactionInstruction.SetDomainKeyValue(domainId, key, value));
    }

    public TransactionBuilder RemoveAccountKeyValue(string accountId, string key)
    {
        return AddInstruction(TransactionInstruction.RemoveAccountKeyValue(accountId, key));
    }

    public TransactionBuilder RemoveDomainKeyValue(string domainId, string key)
    {
        return AddInstruction(TransactionInstruction.RemoveDomainKeyValue(domainId, key));
    }

    public TransactionBuilder SetAssetDefinitionKeyValue(string assetDefinitionId, string key, JsonNode? value)
    {
        return AddInstruction(TransactionInstruction.SetAssetDefinitionKeyValue(assetDefinitionId, key, value));
    }

    public TransactionBuilder RemoveAssetDefinitionKeyValue(string assetDefinitionId, string key)
    {
        return AddInstruction(TransactionInstruction.RemoveAssetDefinitionKeyValue(assetDefinitionId, key));
    }

    public TransactionBuilder SetNftKeyValue(string nftId, string key, JsonNode? value)
    {
        return AddInstruction(TransactionInstruction.SetNftKeyValue(nftId, key, value));
    }

    public TransactionBuilder RemoveNftKeyValue(string nftId, string key)
    {
        return AddInstruction(TransactionInstruction.RemoveNftKeyValue(nftId, key));
    }

    public TransactionBuilder SetTriggerKeyValue(string triggerId, string key, JsonNode? value)
    {
        return AddInstruction(TransactionInstruction.SetTriggerKeyValue(triggerId, key, value));
    }

    public TransactionBuilder RemoveTriggerKeyValue(string triggerId, string key)
    {
        return AddInstruction(TransactionInstruction.RemoveTriggerKeyValue(triggerId, key));
    }

    public TransactionBuilder MintTriggerRepetitions(uint repetitions, string triggerId)
    {
        return AddInstruction(TransactionInstruction.MintTriggerRepetitions(repetitions, triggerId));
    }

    public TransactionBuilder BurnTriggerRepetitions(uint repetitions, string triggerId)
    {
        return AddInstruction(TransactionInstruction.BurnTriggerRepetitions(repetitions, triggerId));
    }

    public TransactionBuilder ExecuteTrigger(string triggerId, JsonNode? args = null)
    {
        return AddInstruction(TransactionInstruction.ExecuteTrigger(triggerId, args));
    }

    public TransactionBuilder SetCreationTimeMilliseconds(ulong creationTimeMilliseconds)
    {
        CreationTimeMilliseconds = creationTimeMilliseconds;
        return this;
    }

    public TransactionBuilder SetCreationTime(DateTimeOffset creationTime)
    {
        return SetCreationTimeMilliseconds((ulong)creationTime.ToUnixTimeMilliseconds());
    }

    public TransactionBuilder SetTimeToLiveMilliseconds(ulong? timeToLiveMilliseconds)
    {
        if (timeToLiveMilliseconds == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(timeToLiveMilliseconds), "Transaction TTL must be positive when provided.");
        }

        TimeToLiveMilliseconds = timeToLiveMilliseconds;
        return this;
    }

    public TransactionBuilder SetNonce(uint? nonce)
    {
        if (nonce == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(nonce), "Transaction nonce must be non-zero when provided.");
        }

        Nonce = nonce;
        return this;
    }

    public TransactionBuilder SetMetadata(string key, JsonNode? value)
    {
        metadata[NormalizeRequiredValue(key, nameof(key))] = value?.DeepClone();
        return this;
    }

    public TransactionBuilder ReplaceMetadata(IReadOnlyDictionary<string, JsonNode?> values)
    {
        ArgumentNullException.ThrowIfNull(values);
        metadata.Clear();
        foreach (var (key, value) in values)
        {
            metadata[NormalizeRequiredValue(key, nameof(values))] = value?.DeepClone();
        }

        return this;
    }

    public SignedTransactionEnvelope BuildSigned(ReadOnlySpan<byte> privateKeySeed)
    {
        if (instructions.Count == 0)
        {
            throw new InvalidOperationException("Transactions must contain at least one instruction.");
        }

        var context = new TransactionEncodingContext(AuthorityAccountId);
        context.EnsureAuthorityMatchesPrivateKey(privateKeySeed);

        var transactionPayload = BuildPayloadBytes(context);
        var payloadHash = IrohaHash.Hash(transactionPayload);
        var signature = Ed25519Signer.Sign(payloadHash, privateKeySeed);

        var signedTransaction = new OfflineNoritoWriter();
        signedTransaction.WriteField(context.EncodeConstVec(signature));
        signedTransaction.WriteField(transactionPayload);
        signedTransaction.WriteField(new byte[] { 0 });
        signedTransaction.WriteField(new byte[] { 0 });
        var signedTransactionBytes = signedTransaction.ToArray();

        var entrypoint = new OfflineNoritoWriter();
        entrypoint.WriteUInt32LittleEndian(0);
        entrypoint.WriteField(signedTransactionBytes);
        var transactionHash = IrohaHash.Hash(entrypoint.ToArray());

        return new SignedTransactionEnvelope(signedTransactionBytes, signedTransactionBytes, transactionPayload, transactionHash);
    }

    internal byte[] BuildPayloadBytes(TransactionEncodingContext context)
    {
        var payload = new OfflineNoritoWriter();
        payload.WriteField(context.EncodeChainId(ChainId));
        payload.WriteField(context.EncodeAccountId(AuthorityAccountId));
        payload.WriteField(context.EncodeUInt64(CreationTimeMilliseconds ?? (ulong)DateTimeOffset.UtcNow.ToUnixTimeMilliseconds()));
        payload.WriteField(context.EncodeInstructionsExecutable(instructions));
        payload.WriteField(context.EncodeOption(TimeToLiveMilliseconds, context.EncodeUInt64));
        payload.WriteField(context.EncodeOption(Nonce, context.EncodeUInt32));
        payload.WriteField(metadata.Count == 0 ? context.EncodeEmptyMetadata() : context.EncodeMetadata(metadata));
        return payload.ToArray();
    }

    private static string NormalizeRequiredValue(string value, string paramName)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new ArgumentException("Value cannot be null or whitespace.", paramName);
        }

        return value.Trim();
    }
}
