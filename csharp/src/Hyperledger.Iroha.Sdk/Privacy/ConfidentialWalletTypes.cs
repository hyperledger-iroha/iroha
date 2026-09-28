using System.Buffers.Binary;
using System.Security.Cryptography;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Privacy;

/// <summary>Canonical asset identity for local confidential note proofs.</summary>
public sealed class ConfidentialAssetId
{
    public string Value { get; }
    public ConfidentialAssetId(string value)
    {
        ArgumentNullException.ThrowIfNull(value);
        if (value.Length > 512) throw new ArgumentException("Asset identity exceeds its bound.", nameof(value));
        Value = TransactionEncodingContext.CanonicalizeAssetDefinitionId(value);
    }
    public override string ToString() => Value;
}

/// <summary>Stable native wallet failure; messages never contain private openings.</summary>
public sealed class ConfidentialProverException : InvalidOperationException
{
    public int Code { get; }
    internal ConfidentialProverException(int code, Exception? inner = null) : base(code switch
    {
        -1 => "Invalid confidential operation or input shape.",
        -2 => "Confidential owner or job is closed or consumed.",
        -3 => "Confidential resource limit exceeded.",
        -10 => "A nonzero 32-byte spend key is required.",
        -11 => "Supply one or two input notes.",
        -12 => "Confidential tree exceeds its capacity.",
        -13 => "Supply one membership path per actual input.",
        -14 => "Invalid membership path shape or root.",
        -15 => "Input index lies outside the supplied tree.",
        -16 => "Membership path and input indices differ.",
        -17 => "A spend cannot consume the same note twice.",
        -18 => "Supply one or two transfer outputs.",
        -19 => "Transfer totals must be positive, equal and non-overflowing.",
        -20 => "Input totals must be positive and non-overflowing.",
        -21 => "Public amount must be positive and no greater than the input total.",
        -22 => "Supply exactly the positive remainder as change, or no change for full redemption.",
        -23 => "Canonical confidential key preparation failed.",
        -24 => "Confidential membership or proof generation failed.",
        -101 => "The loaded native bridge does not support confidential proving.",
        _ => "Confidential prover internal failure.",
    }, inner) => Code = code;
    internal static void Check(int code) { if (code != 0) throw new ConfidentialProverException(code); }
}

internal sealed class ConfidentialSecret : IDisposable
{
    private readonly byte[] bytes;
    private bool disposed;
    internal ConfidentialSecret(int length) => bytes = new byte[length];
    internal T Read<T>(Func<byte[], T> read) { lock (bytes) { if (disposed) throw new ConfidentialProverException(-2); return read(bytes); } }
    internal void WriteSpan(int offset, ReadOnlySpan<byte> value) { lock (bytes) { if (disposed) throw new ConfidentialProverException(-2); value.CopyTo(bytes.AsSpan(offset)); } }
    internal void Write(Action<byte[]> write) => Read(value => { write(value); return true; });
    public void Dispose() { lock (bytes) { CryptographicOperations.ZeroMemory(bytes); disposed = true; } GC.SuppressFinalize(this); }
    ~ConfidentialSecret() => CryptographicOperations.ZeroMemory(bytes);
    internal bool ClearedForTest => bytes.All(value => value == 0);
}

internal static class ConfidentialChecks
{
    internal const int Capacity = 65_536;
    internal static void Word(ReadOnlySpan<byte> value) { if (value.Length != 32) throw new ArgumentException("Expected exactly 32 bytes."); }
    internal static void Amount(UInt128 amount) { if (amount == 0) throw new ArgumentException("Amount must be a positive UInt128."); }
    internal static void Index(int index) { if ((uint)index >= Capacity) throw new ConfidentialProverException(-15); }
    internal static UInt128 ReadAmount(byte[] bytes) => BinaryPrimitives.ReadUInt128LittleEndian(bytes);
}

/// <summary>Owned private input; an accepted proving request consumes and clears it.</summary>
public sealed class ConfidentialInputNote : IDisposable
{
    private readonly ConfidentialSecret secret;
    public int LeafIndex { get; }
    public ConfidentialInputNote(UInt128 amount, ReadOnlySpan<byte> rho, ReadOnlySpan<byte> diversifier, int leafIndex)
    {
        ConfidentialChecks.Amount(amount); ConfidentialChecks.Word(rho); ConfidentialChecks.Word(diversifier); ConfidentialChecks.Index(leafIndex);
        secret = new ConfidentialSecret(80); LeafIndex = leafIndex;
        // All validation precedes private copies; storage is already a clearing owner.
        secret.Write(value => BinaryPrimitives.WriteUInt128LittleEndian(value, amount)); secret.WriteSpan(16, rho); secret.WriteSpan(48, diversifier);
    }
    internal UInt128 Amount => secret.Read(ConfidentialChecks.ReadAmount);
    internal void Append(IConfidentialWalletDriver driver, ulong job) => secret.Write(value =>
    {
        var rho = new byte[32]; var diversifier = new byte[32];
        try { value.AsSpan(16, 32).CopyTo(rho); value.AsSpan(48, 32).CopyTo(diversifier); driver.Input(job, ConfidentialChecks.ReadAmount(value), rho, diversifier, LeafIndex); }
        finally { CryptographicOperations.ZeroMemory(rho); CryptographicOperations.ZeroMemory(diversifier); }
    });
    public void Dispose() => secret.Dispose();
    internal bool ClearedForTest => secret.ClearedForTest;
    public override string ToString() => "ConfidentialInputNote([REDACTED])";
}

/// <summary>Owned private transfer output; consumed by a proving request.</summary>
public sealed class ConfidentialOutputNote : IDisposable
{
    private readonly ConfidentialSecret secret;
    public ConfidentialOutputNote(UInt128 amount, ReadOnlySpan<byte> rho, ReadOnlySpan<byte> ownerTag)
    {
        ConfidentialChecks.Amount(amount); ConfidentialChecks.Word(rho); ConfidentialChecks.Word(ownerTag);
        secret = new ConfidentialSecret(80);
        secret.Write(value => BinaryPrimitives.WriteUInt128LittleEndian(value, amount)); secret.WriteSpan(16, rho); secret.WriteSpan(48, ownerTag);
    }
    internal UInt128 Amount => secret.Read(ConfidentialChecks.ReadAmount);
    internal void Append(IConfidentialWalletDriver driver, ulong job) => secret.Write(value =>
    {
        var rho = new byte[32]; var owner = new byte[32];
        try { value.AsSpan(16, 32).CopyTo(rho); value.AsSpan(48, 32).CopyTo(owner); driver.Output(job, ConfidentialChecks.ReadAmount(value), rho, owner); }
        finally { CryptographicOperations.ZeroMemory(rho); CryptographicOperations.ZeroMemory(owner); }
    });
    public void Dispose() => secret.Dispose();
    internal bool ClearedForTest => secret.ClearedForTest;
    public override string ToString() => "ConfidentialOutputNote([REDACTED])";
}

/// <summary>Owned redemption change. Persist its opening securely before proving consumes it.</summary>
public sealed class ConfidentialChangeNote : IDisposable
{
    private readonly ConfidentialSecret secret;
    public ConfidentialChangeNote(UInt128 amount, ReadOnlySpan<byte> rho)
    {
        ConfidentialChecks.Amount(amount); ConfidentialChecks.Word(rho); secret = new ConfidentialSecret(48);
        secret.Write(value => BinaryPrimitives.WriteUInt128LittleEndian(value, amount)); secret.WriteSpan(16, rho);
    }
    internal UInt128 Amount => secret.Read(ConfidentialChecks.ReadAmount);
    /// <summary>Convert a restored opening using Core's default owner diversifier and an independently authenticated leaf index.</summary>
    public ConfidentialInputNote ToInput(int leafIndex)
    {
        ConfidentialChecks.Index(leafIndex);
        return secret.Read(value =>
        {
            var diversifier = ConfidentialNotes.DefaultDiversifier();
            try { return new ConfidentialInputNote(ConfidentialChecks.ReadAmount(value), value.AsSpan(16, 32), diversifier, leafIndex); }
            finally { CryptographicOperations.ZeroMemory(diversifier); }
        });
    }
    internal void Append(IConfidentialWalletDriver driver, ulong job) => secret.Write(value =>
    {
        var rho = value.AsSpan(16, 32).ToArray();
        try { driver.Output(job, ConfidentialChecks.ReadAmount(value), rho, []); }
        finally { CryptographicOperations.ZeroMemory(rho); }
    });
    public void Dispose() => secret.Dispose();
    internal bool ClearedForTest => secret.ClearedForTest;
    public override string ToString() => "ConfidentialChangeNote([REDACTED])";
}

/// <summary>Owned 16-level membership path; this object does not authenticate its root.</summary>
public sealed class ConfidentialMerklePath : IDisposable
{
    private readonly ConfidentialSecret secret;
    public int LeafIndex { get; }
    public ConfidentialMerklePath(ReadOnlySpan<byte> root, int leafIndex, IReadOnlyList<byte[]> siblings, ReadOnlySpan<byte> directions)
    {
        ConfidentialChecks.Word(root); ConfidentialChecks.Index(leafIndex); ArgumentNullException.ThrowIfNull(siblings);
        if (siblings.Count != 16 || directions.Length != 16) throw new ConfidentialProverException(-14);
        for (var i = 0; i < 16; i++) { ArgumentNullException.ThrowIfNull(siblings[i]); ConfidentialChecks.Word(siblings[i]); if (directions[i] != ((leafIndex >> i) & 1)) throw new ConfidentialProverException(-16); }
        secret = new ConfidentialSecret(560); LeafIndex = leafIndex;
        try { secret.WriteSpan(0, root); secret.WriteSpan(544, directions); secret.Write(value => { for (var i = 0; i < 16; i++) siblings[i].CopyTo(value, 32 + 32 * i); }); }
        catch { secret.Dispose(); throw; }
    }
    internal ConfidentialMerklePath(byte[] encoded, int index) { secret = new ConfidentialSecret(560); LeafIndex = index; secret.Write(value => encoded.CopyTo(value, 0)); }
    public byte[] Root => secret.Read(value => value.AsSpan(0, 32).ToArray());
    internal void CopyTo(byte[] root, byte[] siblings, byte[] directions, int index) => secret.Write(value =>
    {
        if (!value.AsSpan(0, 32).SequenceEqual(root)) throw new ConfidentialProverException(-14);
        value.AsSpan(32, 512).CopyTo(siblings.AsSpan(index * 512)); value.AsSpan(544, 16).CopyTo(directions.AsSpan(index * 16));
    });
    public void Dispose() => secret.Dispose();
    public override string ToString() => "ConfidentialMerklePath([REDACTED])";
}

/// <summary>Owned bounded tree snapshot, consumed by a proving request. Obtain roots from authenticated protocol state.</summary>
public sealed class ConfidentialTreeEvidence : IDisposable
{
    private readonly ConfidentialSecret secret;
    private readonly int[]? indices;
    private readonly int count;
    private readonly bool paths;
    private ConfidentialTreeEvidence(int bytes, int count, bool paths, int[]? indices) { secret = new ConfidentialSecret(bytes); this.count = count; this.paths = paths; this.indices = indices; }
    public static ConfidentialTreeEvidence Commitments(ReadOnlySpan<byte> root, IReadOnlyList<byte[]> leaves)
    {
        ConfidentialChecks.Word(root); ArgumentNullException.ThrowIfNull(leaves);
        if (leaves.Count > ConfidentialChecks.Capacity) throw new ConfidentialProverException(-12);
        for (var i = 0; i < leaves.Count; i++) { ArgumentNullException.ThrowIfNull(leaves[i]); ConfidentialChecks.Word(leaves[i]); }
        var count = leaves.Count; var evidence = new ConfidentialTreeEvidence(32 + count * 32, count, false, null); var r = root.ToArray();
        try { evidence.secret.Write(value => { r.CopyTo(value, 0); for (var i = 0; i < count; i++) leaves[i].CopyTo(value, 32 + i * 32); }); return evidence; }
        catch { evidence.Dispose(); throw; }
        finally { CryptographicOperations.ZeroMemory(r); }
    }
    /// <summary>Copies one path per real input; the caller retains and disposes the original paths.</summary>
    public static ConfidentialTreeEvidence Paths(ReadOnlySpan<byte> root, IReadOnlyList<ConfidentialMerklePath> membership)
    {
        ConfidentialChecks.Word(root); ArgumentNullException.ThrowIfNull(membership);
        if (membership.Count is < 1 or > 2) throw new ConfidentialProverException(-13);
        var evidence = new ConfidentialTreeEvidence(32 + membership.Count * 528, membership.Count, true, new int[membership.Count]);
        var r = new byte[32]; var siblings = new byte[membership.Count * 512]; var directions = new byte[membership.Count * 16]; root.CopyTo(r);
        try { for (var i = 0; i < membership.Count; i++) { ArgumentNullException.ThrowIfNull(membership[i]); membership[i].CopyTo(r, siblings, directions, i); evidence.indices![i] = membership[i].LeafIndex; }
            evidence.secret.Write(value => { r.CopyTo(value, 0); siblings.CopyTo(value, 32); directions.CopyTo(value, 32 + siblings.Length); }); return evidence; }
        catch { evidence.Dispose(); throw; }
        finally { CryptographicOperations.ZeroMemory(r); CryptographicOperations.ZeroMemory(siblings); CryptographicOperations.ZeroMemory(directions); }
    }
    internal byte[] Root => secret.Read(value => value.AsSpan(0, 32).ToArray());
    internal void Validate(IReadOnlyList<ConfidentialInputNote> inputs)
    {
        if (paths && count != inputs.Count) throw new ConfidentialProverException(-13);
        for (var i = 0; i < inputs.Count; i++) if (paths ? indices![i] != inputs[i].LeafIndex : inputs[i].LeafIndex >= count) throw new ConfidentialProverException(paths ? -16 : -15);
        secret.Read(_ => true);
    }
    internal void Append(IConfidentialWalletDriver driver, ulong job) => secret.Write(value =>
    {
        var first = value.AsSpan(32, count * (paths ? 512 : 32)).ToArray();
        byte[] second = [];
        try { if (paths) { second = value.AsSpan(32 + count * 512, count * 16).ToArray(); driver.Paths(job, first, second); } else driver.Commitments(job, first); }
        finally { CryptographicOperations.ZeroMemory(first); CryptographicOperations.ZeroMemory(second); }
    });
    public void Dispose() { secret.Dispose(); if (indices is not null) Array.Clear(indices); }
    internal bool ClearedForTest => secret.ClearedForTest;
    public override string ToString() => "ConfidentialTreeEvidence([REDACTED])";
}
