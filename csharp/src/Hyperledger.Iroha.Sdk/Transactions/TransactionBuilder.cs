using System.Collections.ObjectModel;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Numeric;
using Hyperledger.Iroha.Privacy;

namespace Hyperledger.Iroha.Transactions;

public sealed class TransactionBuilder
{
    public const ulong DefaultTimeToLiveMilliseconds = 100_000;

    private static readonly HashSet<string> RetiredFeeMetadataKeys =
        new(["fee_sponsor", "gas_asset_id", "gas_limit"], StringComparer.Ordinal);

    private readonly List<TransactionBatchEntry> executableEntries = [];
    private readonly Dictionary<string, JsonNode?> metadata = new(StringComparer.Ordinal);
    private FeePaymentIntent feePayment;
    private bool forceExecutableBatch;

    public TransactionBuilder(
        NetworkId networkId,
        string authorityAccountId,
        FeePaymentIntent feePayment)
    {
        NetworkId = networkId ?? throw new ArgumentNullException(nameof(networkId));
        AuthorityAccountId = TransactionEncodingContext.CanonicalizeAccountId(
            authorityAccountId,
            nameof(authorityAccountId));
        this.feePayment = feePayment ?? throw new ArgumentNullException(nameof(feePayment));
    }

    /// <summary>Exact genesis-header-derived network identity.</summary>
    public NetworkId NetworkId { get; }

    public string AuthorityAccountId { get; }

    public FeePaymentIntent FeePayment => feePayment;

    /// <summary>Public submissions always require quorum-certified QueuePlan admission.</summary>
    public TransactionAdmissionIntent AdmissionIntent =>
        TransactionAdmissionIntent.QueuePlanSynced;

    public ulong? CreationTimeMilliseconds { get; private set; }

    public ulong TimeToLiveMilliseconds { get; private set; } = DefaultTimeToLiveMilliseconds;

    public uint? Nonce { get; private set; }

    public IReadOnlyList<TransactionInstruction> Instructions => executableEntries
        .OfType<TransactionBatchEntry.InstructionEntry>()
        .Select(static entry => entry.Value)
        .ToArray();

    public IReadOnlyList<TransactionBatchEntry> ExecutableEntries => executableEntries.ToArray();

    public IReadOnlyDictionary<string, JsonNode?> Metadata => SnapshotMetadata(metadata);

    /// <summary>
    /// Shared mandatory guard for every retained Exact12 construction method added to this
    /// builder. A local catalog or legacy capability snapshot can never satisfy this boundary.
    /// </summary>
    internal static void RequireExact12CapabilityAdmission(
        PrivacyExact12CapabilityTupleAdmissionV1 admission,
        PrivacyProtocolIdV1 protocol,
        PrivacyOperationSchemaV1 operationSchema) =>
        PrivacyExact12CapabilityAdmissionV1.RequireForConstruction(
            admission,
            protocol,
            operationSchema);

    public TransactionBuilder AddInstruction(TransactionInstruction instruction)
    {
        ArgumentNullException.ThrowIfNull(instruction);
        executableEntries.Add(TransactionBatchEntry.Instruction(instruction));
        return this;
    }

    public TransactionBuilder AddContractCall(TransactionContractInvocation invocation)
    {
        ArgumentNullException.ThrowIfNull(invocation);
        executableEntries.Add(TransactionBatchEntry.ContractCall(invocation));
        return this;
    }

    public TransactionBuilder WithExecutableBatch(IEnumerable<TransactionBatchEntry> entries)
    {
        ArgumentNullException.ThrowIfNull(entries);
        var replacement = entries.ToArray();
        if (replacement.Length == 0 || replacement.Any(static entry => entry is null))
        {
            throw new ArgumentException("Executable batches must contain at least one non-null item.", nameof(entries));
        }
        executableEntries.Clear();
        executableEntries.AddRange(replacement);
        forceExecutableBatch = true;
        return this;
    }

    public TransactionBuilder TransferAsset(string assetDefinitionId, string quantity, string destinationAccountId)
    {
        return AddInstruction(TransactionInstruction.TransferAsset(assetDefinitionId, quantity, destinationAccountId));
    }

    public TransactionBuilder TransferAsset(
        string assetDefinitionId,
        NumericV1.QuantityValue quantity,
        string destinationAccountId)
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

    public TransactionBuilder MintAsset(
        string assetDefinitionId,
        NumericV1.QuantityValue quantity,
        string destinationAccountId)
    {
        return AddInstruction(TransactionInstruction.MintAsset(assetDefinitionId, quantity, destinationAccountId));
    }

    public TransactionBuilder BurnAsset(string assetDefinitionId, string quantity, string destinationAccountId)
    {
        return AddInstruction(TransactionInstruction.BurnAsset(assetDefinitionId, quantity, destinationAccountId));
    }

    public TransactionBuilder BurnAsset(
        string assetDefinitionId,
        NumericV1.QuantityValue quantity,
        string destinationAccountId)
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

    public TransactionBuilder IssueReplicationOrder(
        string orderId,
        ReadOnlySpan<byte> orderPayload,
        ulong issuedEpoch,
        ulong deadlineEpoch,
        string? musubiArchiveId = null)
    {
        return AddInstruction(TransactionInstruction.IssueReplicationOrder(
            orderId,
            orderPayload,
            issuedEpoch,
            deadlineEpoch,
            musubiArchiveId));
    }

    public TransactionBuilder IssueReplicationOrder(
        string orderId,
        string orderPayloadBase64,
        ulong issuedEpoch,
        ulong deadlineEpoch,
        string? musubiArchiveId = null)
    {
        return AddInstruction(TransactionInstruction.IssueReplicationOrder(
            orderId,
            orderPayloadBase64,
            issuedEpoch,
            deadlineEpoch,
            musubiArchiveId));
    }

    public TransactionBuilder CompleteReplicationOrder(
        string orderId,
        string providerId,
        ulong completionEpoch,
        ProviderIngestCompletionAuthorityV1 expectedAuthority,
        ulong expectedAssignmentRevision,
        ProviderIngestFinalizedAnchorV1 finalizedAnchor)
    {
        return AddInstruction(TransactionInstruction.CompleteReplicationOrder(
            orderId,
            providerId,
            completionEpoch,
            expectedAuthority,
            expectedAssignmentRevision,
            finalizedAnchor));
    }

    public TransactionBuilder ExpireReplicationOrder(
        string orderId,
        ulong expirationEpoch)
    {
        return AddInstruction(TransactionInstruction.ExpireReplicationOrder(
            orderId,
            expirationEpoch));
    }

    public TransactionBuilder CancelAssetLock(
        string lockId,
        string expectedRemainingAmount)
    {
        return AddInstruction(TransactionInstruction.CancelAssetLock(
            lockId,
            expectedRemainingAmount));
    }

    public TransactionBuilder CancelAssetLock(
        string lockId,
        NumericV1.QuantityValue expectedRemainingAmount)
    {
        return AddInstruction(TransactionInstruction.CancelAssetLock(
            lockId,
            expectedRemainingAmount));
    }

    public TransactionBuilder SetCreationTimeMilliseconds(ulong creationTimeMilliseconds)
    {
        if (creationTimeMilliseconds == 0)
        {
            throw new ArgumentOutOfRangeException(
                nameof(creationTimeMilliseconds),
                "Transaction creation time must be positive.");
        }

        CreationTimeMilliseconds = creationTimeMilliseconds;
        return this;
    }

    public TransactionBuilder SetCreationTime(DateTimeOffset creationTime)
    {
        var creationTimeMilliseconds = creationTime.ToUnixTimeMilliseconds();
        if (creationTimeMilliseconds <= 0)
        {
            throw new ArgumentOutOfRangeException(
                nameof(creationTime),
                "Transaction creation time must be after the Unix epoch.");
        }

        return SetCreationTimeMilliseconds((ulong)creationTimeMilliseconds);
    }

    public TransactionBuilder SetTimeToLiveMilliseconds(ulong timeToLiveMilliseconds)
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
        var normalizedKey = RequireExactNonBlank(key, nameof(key));
        RejectRetiredFeeMetadata(normalizedKey, nameof(key));
        metadata[normalizedKey] = value?.DeepClone();
        return this;
    }

    public TransactionBuilder ReplaceMetadata(IReadOnlyDictionary<string, JsonNode?> values)
    {
        ArgumentNullException.ThrowIfNull(values);
        var replacement = new Dictionary<string, JsonNode?>(StringComparer.Ordinal);
        foreach (var (key, value) in values)
        {
            var normalizedKey = RequireExactNonBlank(key, nameof(values));
            RejectRetiredFeeMetadata(normalizedKey, nameof(values));
            replacement[normalizedKey] = value?.DeepClone();
        }

        metadata.Clear();
        foreach (var (key, value) in replacement)
        {
            metadata[key] = value;
        }

        return this;
    }

    private static IReadOnlyDictionary<string, JsonNode?> SnapshotMetadata(
        IReadOnlyDictionary<string, JsonNode?> values)
    {
        var snapshot = new Dictionary<string, JsonNode?>(StringComparer.Ordinal);
        foreach (var (key, value) in values)
        {
            snapshot[key] = value?.DeepClone();
        }

        return new ReadOnlyDictionary<string, JsonNode?>(snapshot);
    }

    public SignedTransactionEnvelope BuildSigned(ReadOnlySpan<byte> privateKeySeed)
    {
        if (executableEntries.Count == 0)
        {
            throw new InvalidOperationException("Transactions must contain at least one executable item.");
        }
        ValidateExecutableFeeIntent();

        var context = new TransactionEncodingContext(AuthorityAccountId);
        context.EnsureAuthorityMatchesPrivateKey(privateKeySeed);

        EnsureCreationTimeMilliseconds();
        var transactionPayload = BuildPayloadBytes(context);
        var payloadHash = IrohaHash.Hash(transactionPayload);
        var signature = Ed25519Signer.Sign(payloadHash, privateKeySeed);

        var transactionSignature = new CanonicalNoritoWriter();
        transactionSignature.WriteField(context.EncodeConstVec(signature));

        var signedTransaction = new CanonicalNoritoWriter();
        signedTransaction.WriteField(transactionSignature.ToArray());
        signedTransaction.WriteField(transactionPayload);
        signedTransaction.WriteField(new byte[] { 0 });
        var signedTransactionBytes = signedTransaction.ToArray();

        var entrypoint = new CanonicalNoritoWriter();
        entrypoint.WriteUInt32LittleEndian(0);
        entrypoint.WriteField(transactionPayload);
        var transactionHash = IrohaHash.Hash(entrypoint.ToArray());

        return new SignedTransactionEnvelope(signedTransactionBytes, signedTransactionBytes, transactionPayload, transactionHash);
    }

    internal byte[] BuildPayloadBytes(TransactionEncodingContext context)
    {
        var payload = new CanonicalNoritoWriter();
        payload.WriteField(context.EncodeNetworkDomain(NetworkId));
        payload.WriteField(context.EncodeAccountId(AuthorityAccountId));
        payload.WriteField(context.EncodeUInt64(CreationTimeMilliseconds ?? (ulong)DateTimeOffset.UtcNow.ToUnixTimeMilliseconds()));
        var contractCallPresent = executableEntries.Any(static entry => entry is TransactionBatchEntry.ContractCallEntry);
        payload.WriteField(forceExecutableBatch || contractCallPresent
            ? context.EncodeExecutableBatch(executableEntries)
            : context.EncodeInstructionsExecutable(Instructions));
        payload.WriteField(context.EncodeOption<ulong>(TimeToLiveMilliseconds, context.EncodeUInt64));
        payload.WriteField(context.EncodeOption(Nonce, context.EncodeUInt32));
        payload.WriteField(context.EncodeFeePaymentIntent(feePayment));
        payload.WriteField(context.EncodeUInt32((uint)AdmissionIntent));
        payload.WriteField(metadata.Count == 0 ? context.EncodeEmptyMetadata() : context.EncodeMetadata(metadata));
        payload.WriteField(new byte[] { 0 });
        return payload.ToArray();
    }

    /// <summary>
    /// Builds the exact unsigned JSON payload used for fee quoting and freezes its creation time.
    /// </summary>
    public UnsignedTransactionPayload BuildUnsignedPayload()
    {
        if (executableEntries.Count == 0)
        {
            throw new InvalidOperationException("Transactions must contain at least one executable item.");
        }
        ValidateExecutableFeeIntent();

        EnsureCreationTimeMilliseconds();
        var contractCallPresent = executableEntries.Any(static entry => entry is TransactionBatchEntry.ContractCallEntry);
        JsonObject executable;
        if (forceExecutableBatch || contractCallPresent)
        {
            executable = new JsonObject
            {
                ["Batch"] = new JsonArray(executableEntries.Select(EncodeBatchEntryJson).ToArray()),
            };
        }
        else
        {
            executable = new JsonObject
            {
                ["Instructions"] = new JsonArray(
                    Instructions
                        .Select(instruction => JsonValue.Create(
                            instruction.EncodeInstructionBoxBase64(AuthorityAccountId)))
                        .Cast<JsonNode?>()
                        .ToArray()),
            };
        }
        return new UnsignedTransactionPayload(
            NetworkId,
            AuthorityAccountId,
            CreationTimeMilliseconds!.Value,
            executable,
            TimeToLiveMilliseconds,
            Nonce,
            feePayment,
            AdmissionIntent,
            Metadata);
    }

    /// <summary>
    /// Replaces only the signed fee maxima with a quote that preserves the selected payer,
    /// exact sponsor revision, and gas bound.
    /// </summary>
    public TransactionBuilder ApplyFeeQuote(FeePaymentIntent quotedFeePayment)
    {
        ArgumentNullException.ThrowIfNull(quotedFeePayment);
        if (!feePayment.HasSamePayerAndGasBound(quotedFeePayment))
        {
            throw new InvalidOperationException(
                "Fee quote changed the selected payer, sponsor revision, or gas bound.");
        }
        feePayment = quotedFeePayment;
        return this;
    }

    private void EnsureCreationTimeMilliseconds()
    {
        CreationTimeMilliseconds ??= checked((ulong)DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());
    }

    private JsonNode? EncodeBatchEntryJson(TransactionBatchEntry entry)
    {
        return entry switch
        {
            TransactionBatchEntry.InstructionEntry instruction => new JsonObject
            {
                ["Instruction"] = instruction.Value.EncodeInstructionBoxBase64(AuthorityAccountId),
            },
            TransactionBatchEntry.ContractCallEntry call => new JsonObject
            {
                ["ContractCall"] = new JsonObject
                {
                    ["contract_address"] = call.Invocation.ContractAddress,
                    ["expected_code_hash"] = call.Invocation.ExpectedCodeHashLiteral,
                    ["entrypoint"] = call.Invocation.Entrypoint,
                    ["arguments"] = call.Invocation.Arguments is { } arguments
                        ? new JsonArray(arguments.Select(static value => (JsonNode?)JsonValue.Create(value)).ToArray())
                        : null,
                },
            },
            _ => throw new InvalidOperationException("Unknown executable batch entry."),
        };
    }

    private void ValidateExecutableFeeIntent()
    {
        if (executableEntries.Any(static entry => entry is TransactionBatchEntry.ContractCallEntry)
            && feePayment.GasLimit is null)
        {
            throw new InvalidOperationException(
                "Executable batches containing contract calls require a signature-bound gas limit.");
        }
    }

    private static void RejectRetiredFeeMetadata(string key, string paramName)
    {
        if (RetiredFeeMetadataKeys.Contains(key))
        {
            throw new ArgumentException(
                $"Metadata key `{key}` is retired; use the required fee payment intent.",
                paramName);
        }
    }

    private static string RequireExactNonBlank(string? value, string paramName)
    {
        if (string.IsNullOrEmpty(value))
        {
            throw new ArgumentException("Value cannot be null or empty.", paramName);
        }
        if (!string.Equals(value.Trim(), value, StringComparison.Ordinal))
        {
            throw new ArgumentException("Value must not contain surrounding whitespace.", paramName);
        }
        if (value.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException("Value must not contain whitespace.", paramName);
        }
        if (value.Any(char.IsControl))
        {
            throw new ArgumentException("Value must not contain control characters.", paramName);
        }

        return value;
    }
}
