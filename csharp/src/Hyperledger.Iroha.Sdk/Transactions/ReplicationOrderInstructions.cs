using System.Buffers.Binary;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Transactions;

/// <summary>
/// Issues one canonical SoraFS <c>ReplicationOrderV1</c>.
/// </summary>
public sealed record class IssueReplicationOrderInstruction : TransactionInstruction
{
    public const int MaximumOrderPayloadBytesV1 = 1_048_576;

    private readonly byte[] orderPayload;

    public IssueReplicationOrderInstruction(
        string orderId,
        ReadOnlySpan<byte> orderPayload,
        ulong issuedEpoch,
        ulong deadlineEpoch,
        string? musubiArchiveId = null)
    {
        OrderId = ReplicationOrderInstructionValidation.RequireIdentifier(
            orderId,
            nameof(orderId));
        if (deadlineEpoch <= issuedEpoch)
        {
            throw new ArgumentOutOfRangeException(
                nameof(deadlineEpoch),
                "Replication-order deadline_epoch must be greater than issued_epoch.");
        }

        this.orderPayload = orderPayload.ToArray();
        ReplicationOrderInstructionValidation.ValidateOrderPayload(
            this.orderPayload,
            OrderId,
            nameof(orderPayload));
        IssuedEpoch = issuedEpoch;
        DeadlineEpoch = deadlineEpoch;
        MusubiArchiveId = musubiArchiveId is null
            ? null
            : ReplicationOrderInstructionValidation.RequireIdentifier(
                musubiArchiveId,
                nameof(musubiArchiveId));
    }

    public IssueReplicationOrderInstruction(
        string orderId,
        string orderPayloadBase64,
        ulong issuedEpoch,
        ulong deadlineEpoch,
        string? musubiArchiveId = null)
        : this(
            orderId,
            ReplicationOrderInstructionValidation.DecodeCanonicalBase64(
                orderPayloadBase64,
                nameof(orderPayloadBase64)),
            issuedEpoch,
            deadlineEpoch,
            musubiArchiveId)
    {
    }

    public string OrderId { get; }

    /// <summary>Returns a defensive copy of the canonical Norito archive.</summary>
    public byte[] OrderPayload => [.. orderPayload];

    public string OrderPayloadBase64 => Convert.ToBase64String(orderPayload);

    public ulong IssuedEpoch { get; }

    public ulong DeadlineEpoch { get; }

    /// <summary>Optional immutable Musubi archive purpose bound to this order.</summary>
    public string? MusubiArchiveId { get; }

    internal override string WireId => TypeName;

    internal override string TypeName =>
        "iroha_data_model::isi::sorafs::IssueReplicationOrder";

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var payloadVector = new CanonicalNoritoWriter();
        payloadVector.WriteUInt64LittleEndian((ulong)orderPayload.Length);
        payloadVector.WriteBytes(orderPayload);

        var writer = new CanonicalNoritoWriter();
        writer.WriteField(
            ReplicationOrderInstructionValidation.EncodeIdentifierNewtype(OrderId));
        writer.WriteField(payloadVector.ToArray());
        writer.WriteField(context.EncodeUInt64(IssuedEpoch));
        writer.WriteField(context.EncodeUInt64(DeadlineEpoch));
        writer.WriteField(
            ReplicationOrderInstructionValidation.EncodeOptionalIdentifierNewtype(
                MusubiArchiveId));
        return writer.ToArray();
    }
}

/// <summary>
/// Exact governed signer-policy identity expected at completion commit.
/// </summary>
public sealed record class ProviderIngestCompletionSignerPolicyV1
{
    public ProviderIngestCompletionSignerPolicyV1(
        string policyId,
        ulong revision,
        string? predecessorDigest,
        string policyDigest)
    {
        PolicyId = ReplicationOrderInstructionValidation.RequireIdentifier(
            policyId,
            nameof(policyId));
        Revision = ReplicationOrderInstructionValidation.RequirePositive(
            revision,
            nameof(revision));
        PolicyDigest = ReplicationOrderInstructionValidation.RequireIdentifier(
            policyDigest,
            nameof(policyDigest));
        if (revision == 1)
        {
            if (predecessorDigest is not null)
            {
                throw new ArgumentException(
                    "Predecessor digest must be absent at revision one.",
                    nameof(predecessorDigest));
            }
            PredecessorDigest = null;
        }
        else
        {
            if (predecessorDigest is null)
            {
                throw new ArgumentException(
                    "Predecessor digest is required after revision one.",
                    nameof(predecessorDigest));
            }
            PredecessorDigest = ReplicationOrderInstructionValidation.RequireIdentifier(
                predecessorDigest,
                nameof(predecessorDigest));
        }
    }

    public string PolicyId { get; }

    public ulong Revision { get; }

    public string? PredecessorDigest { get; }

    public string PolicyDigest { get; }

    internal byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteField(Convert.FromHexString(PolicyId));
        writer.WriteField(context.EncodeUInt64(Revision));
        writer.WriteField(
            ReplicationOrderInstructionValidation.EncodeOptionalFixedByteArray(
                PredecessorDigest));
        writer.WriteField(Convert.FromHexString(PolicyDigest));
        return writer.ToArray();
    }
}

/// <summary>
/// Exact provider owner and signer policy expected at completion commit.
/// </summary>
public sealed record class ProviderIngestCompletionAuthorityV1
{
    public ProviderIngestCompletionAuthorityV1(
        string providerOwner,
        ProviderIngestCompletionSignerPolicyV1 signerPolicy)
    {
        ProviderOwner = ReplicationOrderInstructionValidation.RequireAccountId(
            providerOwner,
            nameof(providerOwner));
        SignerPolicy = signerPolicy
            ?? throw new ArgumentNullException(nameof(signerPolicy));
    }

    public string ProviderOwner { get; }

    public ProviderIngestCompletionSignerPolicyV1 SignerPolicy { get; }

    internal byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteField(context.EncodeAccountId(ProviderOwner));
        writer.WriteField(SignerPolicy.EncodePayload(context));
        return writer.ToArray();
    }
}

/// <summary>
/// Exact finalized committed-chain prefix used to prepare a completion.
/// </summary>
public sealed record class ProviderIngestFinalizedAnchorV1
{
    public ProviderIngestFinalizedAnchorV1(ulong height, string blockHash)
    {
        Height = ReplicationOrderInstructionValidation.RequirePositive(
            height,
            nameof(height));
        BlockHash = ReplicationOrderInstructionValidation.RequireIdentifier(
            blockHash,
            nameof(blockHash));
    }

    public ulong Height { get; }

    public string BlockHash { get; }

    internal byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteField(context.EncodeUInt64(Height));
        writer.WriteField(Convert.FromHexString(BlockHash));
        return writer.ToArray();
    }
}

/// <summary>
/// Completes one SoraFS replication order with the exact six-field authority hard cut.
/// </summary>
public sealed record class CompleteReplicationOrderInstruction : TransactionInstruction
{
    public CompleteReplicationOrderInstruction(
        string orderId,
        string providerId,
        ulong completionEpoch,
        ProviderIngestCompletionAuthorityV1 expectedAuthority,
        ulong expectedAssignmentRevision,
        ProviderIngestFinalizedAnchorV1 finalizedAnchor)
    {
        OrderId = ReplicationOrderInstructionValidation.RequireIdentifier(
            orderId,
            nameof(orderId));
        ProviderId = ReplicationOrderInstructionValidation.RequireIdentifier(
            providerId,
            nameof(providerId));
        CompletionEpoch = completionEpoch;
        ExpectedAuthority = expectedAuthority
            ?? throw new ArgumentNullException(nameof(expectedAuthority));
        ExpectedAssignmentRevision =
            ReplicationOrderInstructionValidation.RequirePositive(
                expectedAssignmentRevision,
                nameof(expectedAssignmentRevision));
        FinalizedAnchor = finalizedAnchor
            ?? throw new ArgumentNullException(nameof(finalizedAnchor));
    }

    public string OrderId { get; }

    public string ProviderId { get; }

    public ulong CompletionEpoch { get; }

    public ProviderIngestCompletionAuthorityV1 ExpectedAuthority { get; }

    public ulong ExpectedAssignmentRevision { get; }

    public ProviderIngestFinalizedAnchorV1 FinalizedAnchor { get; }

    internal override string WireId => TypeName;

    internal override string TypeName =>
        "iroha_data_model::isi::sorafs::CompleteReplicationOrder";

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteField(
            ReplicationOrderInstructionValidation.EncodeIdentifierNewtype(OrderId));
        writer.WriteField(
            ReplicationOrderInstructionValidation.EncodeIdentifierNewtype(ProviderId));
        writer.WriteField(context.EncodeUInt64(CompletionEpoch));
        writer.WriteField(ExpectedAuthority.EncodePayload(context));
        writer.WriteField(context.EncodeUInt64(ExpectedAssignmentRevision));
        writer.WriteField(FinalizedAnchor.EncodePayload(context));
        return writer.ToArray();
    }
}

/// <summary>
/// Expires one SoraFS replication order at a non-negative ledger epoch.
/// </summary>
public sealed record class ExpireReplicationOrderInstruction : TransactionInstruction
{
    public ExpireReplicationOrderInstruction(string orderId, ulong expirationEpoch)
    {
        OrderId = ReplicationOrderInstructionValidation.RequireIdentifier(
            orderId,
            nameof(orderId));
        ExpirationEpoch = expirationEpoch;
    }

    public string OrderId { get; }

    public ulong ExpirationEpoch { get; }

    internal override string WireId => TypeName;

    internal override string TypeName =>
        "iroha_data_model::isi::sorafs::ExpireReplicationOrder";

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteField(
            ReplicationOrderInstructionValidation.EncodeIdentifierNewtype(OrderId));
        writer.WriteField(context.EncodeUInt64(ExpirationEpoch));
        return writer.ToArray();
    }
}

internal static class ReplicationOrderInstructionValidation
{
    private const int MaximumOrderPayloadBytesV1 =
        IssueReplicationOrderInstruction.MaximumOrderPayloadBytesV1;
    private const int MaximumOrderPayloadBase64CharactersV1 =
        4 * ((MaximumOrderPayloadBytesV1 + 2) / 3);
    private const int MaximumAssignmentsV1 = 1_024;
    private const byte CompactLengthFlag = 0x02;
    private const string ReplicationOrderTypeName =
        "sorafs_manifest::capacity::ReplicationOrderV1";

    internal static string RequireIdentifier(string? value, string parameterName)
    {
        if (value is null
            || value.Length != 64
            || value.Any(static character =>
                !(character >= '0' && character <= '9')
                && !(character >= 'a' && character <= 'f'))
            || value.All(static character => character == '0'))
        {
            throw new ArgumentException(
                "Identifier must be exactly 64 lowercase hexadecimal characters and non-zero.",
                parameterName);
        }
        return value;
    }

    internal static ulong RequirePositive(ulong value, string parameterName)
    {
        if (value == 0)
        {
            throw new ArgumentOutOfRangeException(
                parameterName,
                "Value must be greater than zero.");
        }
        return value;
    }

    internal static string RequireAccountId(string? value, string parameterName)
    {
        if (value is null)
        {
            throw new ArgumentNullException(parameterName);
        }
        try
        {
            _ = AccountAddress.Parse(value);
        }
        catch (AccountAddressException error)
        {
            throw new ArgumentException(
                "Account id must be an exact canonical I105 literal.",
                parameterName,
                error);
        }
        return value;
    }

    internal static byte[] EncodeIdentifierNewtype(string value)
    {
        var writer = new CanonicalNoritoWriter();
        writer.WriteField(Convert.FromHexString(value));
        return writer.ToArray();
    }

    internal static byte[] EncodeOptionalFixedByteArray(string? value)
    {
        var writer = new CanonicalNoritoWriter();
        if (value is null)
        {
            writer.WriteByte(0);
            return writer.ToArray();
        }

        writer.WriteByte(1);
        var array = new CanonicalNoritoWriter();
        foreach (var item in Convert.FromHexString(value))
        {
            array.WriteField(new[] { item });
        }
        writer.WriteField(array.ToArray());
        return writer.ToArray();
    }

    internal static byte[] EncodeOptionalIdentifierNewtype(string? value)
    {
        var writer = new CanonicalNoritoWriter();
        if (value is null)
        {
            writer.WriteByte(0);
            return writer.ToArray();
        }

        writer.WriteByte(1);
        writer.WriteField(EncodeIdentifierNewtype(value));
        return writer.ToArray();
    }

    internal static byte[] DecodeCanonicalBase64(string? value, string parameterName)
    {
        if (string.IsNullOrEmpty(value)
            || value.Length > MaximumOrderPayloadBase64CharactersV1
            || !string.Equals(value.Trim(), value, StringComparison.Ordinal)
            || value.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException(
                "Payload must be non-empty canonical standard base64.",
                parameterName);
        }

        byte[] decoded;
        try
        {
            decoded = Convert.FromBase64String(value);
        }
        catch (FormatException error)
        {
            throw new ArgumentException(
                "Payload must be canonical standard base64.",
                parameterName,
                error);
        }
        if (decoded.Length == 0
            || !string.Equals(
                Convert.ToBase64String(decoded),
                value,
                StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "Payload must be non-empty canonical standard base64.",
                parameterName);
        }
        return decoded;
    }

    internal static void ValidateOrderPayload(
        ReadOnlySpan<byte> archive,
        string expectedOrderId,
        string parameterName)
    {
        if (archive.IsEmpty || archive.Length > MaximumOrderPayloadBytesV1)
        {
            throw Invalid(
                $"Decoded payload must contain 1..{MaximumOrderPayloadBytesV1} bytes.",
                parameterName);
        }

        byte[] payload;
        byte flags;
        try
        {
            (payload, flags) = NoritoCodec.Decode(ReplicationOrderTypeName, archive);
        }
        catch (ArgumentException error)
        {
            throw new ArgumentException(
                "Payload must be a canonical ReplicationOrderV1 Norito frame.",
                parameterName,
                error);
        }
        if (flags != CompactLengthFlag
            || !NoritoCodec.Encode(
                ReplicationOrderTypeName,
                payload,
                CompactLengthFlag).AsSpan().SequenceEqual(archive))
        {
            throw Invalid(
                "Payload must use canonical unpadded compact-length Norito framing.",
                parameterName);
        }

        var reader = new CompactReader(payload, "ReplicationOrderV1", parameterName);
        var version = reader.ReadField("version");
        var orderId = reader.ReadField("order_id");
        _ = reader.ReadField("manifest_cid");
        _ = reader.ReadField("manifest_digest");
        _ = reader.ReadField("chunking_profile");
        var targetReplicasBytes = reader.ReadField("target_replicas");
        var assignmentsBytes = reader.ReadField("assignments").ToArray();
        var issuedAtBytes = reader.ReadField("issued_at");
        var deadlineAtBytes = reader.ReadField("deadline_at");
        _ = reader.ReadField("sla");
        _ = reader.ReadField("metadata");
        reader.RequireEnd();

        if (version.Length != 1 || version[0] != 1)
        {
            throw Invalid("ReplicationOrderV1.version must be 1.", parameterName);
        }
        if (orderId.Length != 32 || IsZero(orderId))
        {
            throw Invalid(
                "ReplicationOrderV1.order_id must be a non-zero 32-byte value.",
                parameterName);
        }
        if (!Convert.ToHexString(orderId).Equals(
            expectedOrderId,
            StringComparison.OrdinalIgnoreCase))
        {
            throw Invalid(
                "Instruction order_id must match ReplicationOrderV1.order_id.",
                parameterName);
        }
        if (targetReplicasBytes.Length != sizeof(ushort))
        {
            throw Invalid("ReplicationOrderV1.target_replicas must be a u16.", parameterName);
        }
        var targetReplicas = BinaryPrimitives.ReadUInt16LittleEndian(targetReplicasBytes);
        if (targetReplicas == 0)
        {
            throw Invalid(
                "ReplicationOrderV1.target_replicas must be greater than zero.",
                parameterName);
        }

        var assignments = new CompactReader(
            assignmentsBytes,
            "ReplicationOrderV1.assignments",
            parameterName);
        var assignmentCount = assignments.ReadFixedUInt64("count");
        if (assignmentCount == 0 || assignmentCount > MaximumAssignmentsV1)
        {
            throw Invalid(
                $"ReplicationOrderV1.assignments must contain 1..{MaximumAssignmentsV1} entries.",
                parameterName);
        }
        byte[]? previousProvider = null;
        for (var index = 0; index < (int)assignmentCount; index++)
        {
            var assignmentBytes = assignments.ReadField($"item[{index}]").ToArray();
            var assignment = new CompactReader(
                assignmentBytes,
                $"ReplicationOrderV1.assignments[{index}]",
                parameterName);
            var provider = assignment.ReadField("provider_id").ToArray();
            var sliceGiB = assignment.ReadField("slice_gib");
            _ = assignment.ReadField("lane");
            assignment.RequireEnd();
            if (provider.Length != 32 || IsZero(provider))
            {
                throw Invalid(
                    $"ReplicationOrderV1.assignments[{index}].provider_id must be non-zero.",
                    parameterName);
            }
            if (ReadExactUInt64(sliceGiB, $"assignments[{index}].slice_gib", parameterName) == 0)
            {
                throw Invalid(
                    $"ReplicationOrderV1.assignments[{index}].slice_gib must be positive.",
                    parameterName);
            }
            if (previousProvider is not null
                && previousProvider.AsSpan().SequenceCompareTo(provider) >= 0)
            {
                throw Invalid(
                    "ReplicationOrderV1 assignments must use unique, strictly increasing providers.",
                    parameterName);
            }
            previousProvider = provider;
        }
        assignments.RequireEnd();
        if (targetReplicas > assignmentCount)
        {
            throw Invalid(
                "ReplicationOrderV1.target_replicas must not exceed assignment count.",
                parameterName);
        }

        var issuedAt = ReadExactUInt64(issuedAtBytes, "issued_at", parameterName);
        var deadlineAt = ReadExactUInt64(deadlineAtBytes, "deadline_at", parameterName);
        if (deadlineAt <= issuedAt)
        {
            throw Invalid(
                "ReplicationOrderV1.deadline_at must be greater than issued_at.",
                parameterName);
        }
    }

    private static ulong ReadExactUInt64(
        ReadOnlySpan<byte> value,
        string field,
        string parameterName)
    {
        if (value.Length != sizeof(ulong))
        {
            throw Invalid($"{field} must contain exactly eight bytes.", parameterName);
        }
        return BinaryPrimitives.ReadUInt64LittleEndian(value);
    }

    private static bool IsZero(ReadOnlySpan<byte> value)
    {
        foreach (var item in value)
        {
            if (item != 0)
            {
                return false;
            }
        }
        return true;
    }

    private static ArgumentException Invalid(string message, string parameterName) =>
        new(message, parameterName);

    private ref struct CompactReader
    {
        private readonly ReadOnlySpan<byte> payload;
        private readonly string context;
        private readonly string parameterName;
        private int offset;

        internal CompactReader(
            ReadOnlySpan<byte> payload,
            string context,
            string parameterName)
        {
            this.payload = payload;
            this.context = context;
            this.parameterName = parameterName;
            offset = 0;
        }

        internal ReadOnlySpan<byte> ReadField(string name)
        {
            return ReadBytes(ReadCompactLength(name), name);
        }

        internal ulong ReadFixedUInt64(string name)
        {
            return BinaryPrimitives.ReadUInt64LittleEndian(
                ReadBytes(sizeof(ulong), name));
        }

        internal void RequireEnd()
        {
            if (offset != payload.Length)
            {
                throw Invalid($"{context} contains trailing bytes.", parameterName);
            }
        }

        private int ReadCompactLength(string name)
        {
            ulong value = 0;
            var shift = 0;
            var consumed = 0;
            while (consumed < 10)
            {
                var item = ReadBytes(1, $"{name}.length")[0];
                consumed++;
                var part = (ulong)(item & 0x7f);
                if (shift == 63 && part > 1)
                {
                    throw InvalidLength(name);
                }
                value |= part << shift;
                if ((item & 0x80) == 0)
                {
                    if (consumed > 1 && part == 0)
                    {
                        throw InvalidLength(name);
                    }
                    if (value > int.MaxValue)
                    {
                        throw InvalidLength(name);
                    }
                    return (int)value;
                }
                shift += 7;
            }
            throw InvalidLength(name);
        }

        private ReadOnlySpan<byte> ReadBytes(int count, string name)
        {
            if (count < 0 || offset > payload.Length || count > payload.Length - offset)
            {
                throw Invalid(
                    $"{context}.{name} overruns the Norito payload.",
                    parameterName);
            }
            var result = payload.Slice(offset, count);
            offset += count;
            return result;
        }

        private ArgumentException InvalidLength(string name) =>
            Invalid(
                $"{context}.{name} uses a noncanonical compact length.",
                parameterName);
    }
}
