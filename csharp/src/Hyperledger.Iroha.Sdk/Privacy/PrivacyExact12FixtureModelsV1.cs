using System.Collections.ObjectModel;
using System.Text;

namespace Hyperledger.Iroha.Privacy;

/// <summary>
/// One byte-complete row of the canonical first-release exact-12 privacy fixture.
/// </summary>
public sealed class PrivacyExact12TypedFixtureRowV1 : IEquatable<PrivacyExact12TypedFixtureRowV1>
{
    private readonly byte[] _statementNorito;
    private readonly byte[] _envelopeNorito;
    private readonly byte[] _submitProofInstructionNorito;
    private readonly byte[] _transactionIntentProjectionNorito;
    private readonly byte[] _transactionIntentDigest;
    private readonly byte[] _unsignedTransactionPayloadNorito;
    private readonly byte[] _signedTransactionVersionedNorito;
    private readonly byte[] _signedTransactionHash;

    public PrivacyExact12TypedFixtureRowV1(
        PrivacyProtocolIdV1 protocolId,
        byte[] statementNorito,
        byte[] envelopeNorito,
        string submitProofWireId,
        byte[] submitProofInstructionNorito,
        byte[] transactionIntentProjectionNorito,
        byte[] transactionIntentDigest,
        byte[] unsignedTransactionPayloadNorito,
        byte[] signedTransactionVersionedNorito,
        byte[] signedTransactionHash)
    {
        if (!Enum.IsDefined(protocolId))
        {
            throw new ArgumentOutOfRangeException(nameof(protocolId));
        }

        ArgumentNullException.ThrowIfNull(submitProofWireId);
        if (!string.Equals(
                submitProofWireId,
                PrivacyExact12FixtureCodecV1.SubmitProofWireId,
                StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "Submit-proof wire ID must be the exact first-release identifier.",
                nameof(submitProofWireId));
        }

        RequireBounded(
            statementNorito,
            PrivacyExact12FixtureCodecV1.MaxStatementBytes,
            nameof(statementNorito));
        RequireBounded(
            envelopeNorito,
            PrivacyExact12FixtureCodecV1.MaxEnvelopeBytes,
            nameof(envelopeNorito));
        RequireBounded(
            submitProofInstructionNorito,
            PrivacyExact12FixtureCodecV1.MaxInstructionBytes,
            nameof(submitProofInstructionNorito));
        RequireBounded(
            transactionIntentProjectionNorito,
            PrivacyExact12FixtureCodecV1.MaxIntentProjectionBytes,
            nameof(transactionIntentProjectionNorito));
        RequireFixed(
            transactionIntentDigest,
            PrivacyExact12FixtureCodecV1.HashBytes,
            nameof(transactionIntentDigest));
        RequireBounded(
            unsignedTransactionPayloadNorito,
            PrivacyExact12FixtureCodecV1.MaxUnsignedTransactionBytes,
            nameof(unsignedTransactionPayloadNorito));
        RequireBounded(
            signedTransactionVersionedNorito,
            PrivacyExact12FixtureCodecV1.MaxSignedTransactionBytes,
            nameof(signedTransactionVersionedNorito));
        RequireFixed(
            signedTransactionHash,
            PrivacyExact12FixtureCodecV1.HashBytes,
            nameof(signedTransactionHash));

        var nestedByteCount = checked(
            (long)Encoding.UTF8.GetByteCount(submitProofWireId)
            + statementNorito.Length
            + envelopeNorito.Length
            + submitProofInstructionNorito.Length
            + transactionIntentProjectionNorito.Length
            + transactionIntentDigest.Length
            + unsignedTransactionPayloadNorito.Length
            + signedTransactionVersionedNorito.Length
            + signedTransactionHash.Length);
        if (nestedByteCount > PrivacyExact12FixtureCodecV1.MaxAggregateNestedBytes)
        {
            throw new ArgumentException(
                "Exact-12 row exceeds the aggregate nested-byte limit.");
        }

        ProtocolId = protocolId;
        SubmitProofWireId = submitProofWireId;
        _statementNorito = (byte[])statementNorito.Clone();
        _envelopeNorito = (byte[])envelopeNorito.Clone();
        _submitProofInstructionNorito = (byte[])submitProofInstructionNorito.Clone();
        _transactionIntentProjectionNorito =
            (byte[])transactionIntentProjectionNorito.Clone();
        _transactionIntentDigest = (byte[])transactionIntentDigest.Clone();
        _unsignedTransactionPayloadNorito = (byte[])unsignedTransactionPayloadNorito.Clone();
        _signedTransactionVersionedNorito =
            (byte[])signedTransactionVersionedNorito.Clone();
        _signedTransactionHash = (byte[])signedTransactionHash.Clone();
    }

    /// <summary>Closed protocol identity in canonical wire order.</summary>
    public PrivacyProtocolIdV1 ProtocolId { get; }

    /// <summary>Complete canonical <c>PrivacyStatementV1</c> archive.</summary>
    public byte[] StatementNorito => (byte[])_statementNorito.Clone();

    /// <summary>Complete canonical <c>PrivacyProofEnvelopeV1</c> archive.</summary>
    public byte[] EnvelopeNorito => (byte[])_envelopeNorito.Clone();

    /// <summary>Exact first-release <c>SubmitPrivacyProofV1</c> wire identifier.</summary>
    public string SubmitProofWireId { get; }

    /// <summary>Complete canonical submit-proof instruction archive.</summary>
    public byte[] SubmitProofInstructionNorito =>
        (byte[])_submitProofInstructionNorito.Clone();

    /// <summary>Canonical transaction-intent projection archive.</summary>
    public byte[] TransactionIntentProjectionNorito =>
        (byte[])_transactionIntentProjectionNorito.Clone();

    /// <summary>Exact 32-byte transaction-intent digest.</summary>
    public byte[] TransactionIntentDigest => (byte[])_transactionIntentDigest.Clone();

    /// <summary>Complete canonical unsigned transaction payload archive.</summary>
    public byte[] UnsignedTransactionPayloadNorito =>
        (byte[])_unsignedTransactionPayloadNorito.Clone();

    /// <summary>Complete canonical versioned signed transaction archive.</summary>
    public byte[] SignedTransactionVersionedNorito =>
        (byte[])_signedTransactionVersionedNorito.Clone();

    /// <summary>Exact 32-byte pipeline transaction hash.</summary>
    public byte[] SignedTransactionHash => (byte[])_signedTransactionHash.Clone();

    internal ReadOnlySpan<byte> StatementNoritoSpan => _statementNorito;

    internal ReadOnlySpan<byte> EnvelopeNoritoSpan => _envelopeNorito;

    internal ReadOnlySpan<byte> SubmitProofInstructionNoritoSpan =>
        _submitProofInstructionNorito;

    internal ReadOnlySpan<byte> TransactionIntentProjectionNoritoSpan =>
        _transactionIntentProjectionNorito;

    internal ReadOnlySpan<byte> TransactionIntentDigestSpan => _transactionIntentDigest;

    internal ReadOnlySpan<byte> UnsignedTransactionPayloadNoritoSpan =>
        _unsignedTransactionPayloadNorito;

    internal ReadOnlySpan<byte> SignedTransactionVersionedNoritoSpan =>
        _signedTransactionVersionedNorito;

    internal ReadOnlySpan<byte> SignedTransactionHashSpan => _signedTransactionHash;

    internal long NestedByteCount
    {
        get
        {
            long total = Encoding.UTF8.GetByteCount(SubmitProofWireId);
            foreach (var length in new[]
                     {
                         _statementNorito.Length,
                         _envelopeNorito.Length,
                         _submitProofInstructionNorito.Length,
                         _transactionIntentProjectionNorito.Length,
                         _transactionIntentDigest.Length,
                         _unsignedTransactionPayloadNorito.Length,
                         _signedTransactionVersionedNorito.Length,
                         _signedTransactionHash.Length,
                     })
            {
                total = checked(total + length);
            }

            return total;
        }
    }

    public bool Equals(PrivacyExact12TypedFixtureRowV1? other) =>
        other is not null
        && ProtocolId == other.ProtocolId
        && string.Equals(SubmitProofWireId, other.SubmitProofWireId, StringComparison.Ordinal)
        && _statementNorito.AsSpan().SequenceEqual(other._statementNorito)
        && _envelopeNorito.AsSpan().SequenceEqual(other._envelopeNorito)
        && _submitProofInstructionNorito.AsSpan().SequenceEqual(
            other._submitProofInstructionNorito)
        && _transactionIntentProjectionNorito.AsSpan().SequenceEqual(
            other._transactionIntentProjectionNorito)
        && _transactionIntentDigest.AsSpan().SequenceEqual(other._transactionIntentDigest)
        && _unsignedTransactionPayloadNorito.AsSpan().SequenceEqual(
            other._unsignedTransactionPayloadNorito)
        && _signedTransactionVersionedNorito.AsSpan().SequenceEqual(
            other._signedTransactionVersionedNorito)
        && _signedTransactionHash.AsSpan().SequenceEqual(other._signedTransactionHash);

    public override bool Equals(object? obj) => Equals(obj as PrivacyExact12TypedFixtureRowV1);

    public override int GetHashCode()
    {
        var hash = new HashCode();
        hash.Add(ProtocolId);
        hash.Add(SubmitProofWireId, StringComparer.Ordinal);
        AddBytes(ref hash, _statementNorito);
        AddBytes(ref hash, _envelopeNorito);
        AddBytes(ref hash, _submitProofInstructionNorito);
        AddBytes(ref hash, _transactionIntentProjectionNorito);
        AddBytes(ref hash, _transactionIntentDigest);
        AddBytes(ref hash, _unsignedTransactionPayloadNorito);
        AddBytes(ref hash, _signedTransactionVersionedNorito);
        AddBytes(ref hash, _signedTransactionHash);
        return hash.ToHashCode();
    }

    private static void RequireBounded(byte[] value, int maximum, string parameterName)
    {
        ArgumentNullException.ThrowIfNull(value, parameterName);
        if (value.Length == 0 || value.Length > maximum)
        {
            throw new ArgumentException(
                $"{parameterName} must contain between 1 and {maximum} bytes.",
                parameterName);
        }
    }

    private static void RequireFixed(byte[] value, int expected, string parameterName)
    {
        ArgumentNullException.ThrowIfNull(value, parameterName);
        if (value.Length != expected)
        {
            throw new ArgumentException(
                $"{parameterName} must contain exactly {expected} bytes.",
                parameterName);
        }
    }

    private static void AddBytes(ref HashCode hash, ReadOnlySpan<byte> bytes)
    {
        foreach (var value in bytes)
        {
            hash.Add(value);
        }
    }
}

/// <summary>
/// Immutable typed outer bundle containing all twelve first-release privacy rows.
/// </summary>
public sealed class PrivacyExact12FixtureBundleV1 : IEquatable<PrivacyExact12FixtureBundleV1>
{
    private readonly ReadOnlyCollection<PrivacyExact12TypedFixtureRowV1> _rows;

    public PrivacyExact12FixtureBundleV1(
        uint version,
        IReadOnlyList<PrivacyExact12TypedFixtureRowV1> rows)
    {
        ArgumentNullException.ThrowIfNull(rows);
        if (version != PrivacyExact12FixtureCodecV1.Version)
        {
            throw new ArgumentOutOfRangeException(
                nameof(version),
                $"Exact-12 fixture version must be {PrivacyExact12FixtureCodecV1.Version}.");
        }

        if (rows.Count != PrivacyExact12FixtureCodecV1.RowCount)
        {
            throw new ArgumentException(
                $"Exact-12 fixture must contain exactly {PrivacyExact12FixtureCodecV1.RowCount} rows.",
                nameof(rows));
        }

        if (Enum.GetValues<PrivacyProtocolIdV1>().Length
            != PrivacyExact12FixtureCodecV1.RowCount)
        {
            throw new InvalidOperationException(
                "The first-release privacy protocol registry is not closed at exactly twelve entries.");
        }

        var snapshot = new PrivacyExact12TypedFixtureRowV1[rows.Count];
        long aggregate = 0;
        for (var index = 0; index < rows.Count; index++)
        {
            var row = rows[index]
                ?? throw new ArgumentException("Exact-12 rows must not contain null.", nameof(rows));
            if ((uint)row.ProtocolId != (uint)index)
            {
                throw new ArgumentException(
                    $"Exact-12 row {index} is out of canonical protocol order.",
                    nameof(rows));
            }

            aggregate = checked(aggregate + row.NestedByteCount);
            if (aggregate > PrivacyExact12FixtureCodecV1.MaxAggregateNestedBytes)
            {
                throw new ArgumentException(
                    "Exact-12 bundle exceeds the aggregate nested-byte limit.",
                    nameof(rows));
            }

            snapshot[index] = row;
        }

        Version = version;
        _rows = new ReadOnlyCollection<PrivacyExact12TypedFixtureRowV1>(snapshot);
    }

    /// <summary>Exact first-release bundle version.</summary>
    public uint Version { get; }

    /// <summary>Twelve rows in closed <see cref="PrivacyProtocolIdV1"/> order.</summary>
    public IReadOnlyList<PrivacyExact12TypedFixtureRowV1> Rows => _rows;

    public bool Equals(PrivacyExact12FixtureBundleV1? other) =>
        other is not null
        && Version == other.Version
        && _rows.SequenceEqual(other._rows);

    public override bool Equals(object? obj) => Equals(obj as PrivacyExact12FixtureBundleV1);

    public override int GetHashCode()
    {
        var hash = new HashCode();
        hash.Add(Version);
        foreach (var row in _rows)
        {
            hash.Add(row);
        }

        return hash.ToHashCode();
    }
}
