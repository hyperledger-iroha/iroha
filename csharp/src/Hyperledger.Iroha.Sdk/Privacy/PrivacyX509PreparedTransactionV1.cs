using System.Runtime.InteropServices;
using System.Security.Cryptography;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Privacy;

/// <summary>Exact typed reason why the selected committed protocol is unavailable.</summary>
public sealed class PrivacyCapabilityUnavailableException : InvalidOperationException
{
    public PrivacyProtocolIdV1 ProtocolId { get; }
    public PrivacyCapabilityUnavailableReasonV1? Reason { get; }
    public PrivacyCompiledProfileUnavailableReasonV1? CompiledProfileReason { get; }
    public PrivacyCompiledStatementSchemaErrorV1? SchemaError { get; }
    internal PrivacyCapabilityUnavailableException(PrivacyExact12CapabilityRowV1 row)
        : base($"Exact12 protocol {row.ProtocolId.CanonicalLabel()} is unavailable: {row.UnavailableReason}; {row.CompiledProfileUnavailableReason}; {row.CompiledStatementSchemaError}.")
    { ProtocolId = row.ProtocolId; Reason = row.UnavailableReason; CompiledProfileReason = row.CompiledProfileUnavailableReason; SchemaError = row.CompiledStatementSchemaError; }
}

/// <summary>A native preparation/signing refusal. Status -24 is JointSecurityReductionIncomplete.</summary>
public sealed class PrivacyX509NativeException : InvalidOperationException
{
    public int Status { get; }
    internal PrivacyX509NativeException(int status)
        : base(status == -24 ? "X509 native profile unavailable: JointSecurityReductionIncomplete."
            : $"Native X509 transaction refused (status {status}).") => Status = status;
    internal static void Check(int status) { if (status != 0) throw new PrivacyX509NativeException(status); }
}

/// <summary>Immutable native custody of the original X509 payload, context and single proof occurrence.</summary>
/// <remarks>
/// Obtain this owner from TransactionBuilder.PrepareX509Transaction. Public projections are copies,
/// never signing authority. Dispose releases custody; accepted native calls retain safe ownership.
/// The sealed SDK admission authenticates HTTPS origin. Native validation checks canonical data and
/// selected local policy; a raw C manifest archive does not authenticate a Torii service.
/// </remarks>
public sealed class PrivacyX509PreparedTransactionV1 : IDisposable
{
    private const PrivacyProtocolIdV1 Protocol = PrivacyProtocolIdV1.IrohaZkX509StarkP256V1;
    private readonly PrivacyExact12CapabilityTupleAdmissionV1 admission;
    private readonly NetworkId network;
    private readonly PrivacyX509NativeOwner owner;
    private PrivacyX509PreparedTransactionV1(PrivacyExact12CapabilityTupleAdmissionV1 admission,
        NetworkId network, PrivacyX509NativeOwner owner)
    { this.admission = admission; this.network = network; this.owner = owner; }
    internal static PrivacyX509PreparedTransactionV1 Prepare(PrivacyExact12CapabilityTupleAdmissionV1 admission,
        NetworkId network, byte[] manifest, byte[] payload, byte[] request)
    {
        admission.RequireAuthentic(Protocol, network);
        var driver = PrivacyX509Native.Driver;
        var handle = driver.Prepare(network.ToBytes(), manifest, payload, request);
        return new(admission, network, new PrivacyX509NativeOwner(driver, handle));
    }
    public NetworkId NetworkId => network;
    public byte[] OrderedIntentDigest => owner.Projection(0);
    public byte[] ActionStatementDigest => owner.Projection(1);
    public byte[] ActionStatementNorito => owner.Projection(2);
    /// <summary>Canonical tagged PrivacyStatementV1, with the materialized X509 variant.</summary>
    public byte[] MaterializedStatementNorito => owner.Projection(3);
    /// <summary>Complete the original occurrence and sign the original payload after fresh admission checks.</summary>
    public SignedTransactionEnvelope Sign(ReadOnlySpan<byte> privateKeySeed, ReadOnlySpan<byte> credentialProof)
    {
        admission.RequireAuthentic(Protocol, network);
        return SignedTransactionEnvelope.FromNativeSignedBytes(owner.Sign(privateKeySeed, credentialProof));
    }
    public void Dispose() => owner.Dispose();
}

internal interface IPrivacyX509Driver
{
    ulong Prepare(byte[] network, byte[] manifest, byte[] payload, byte[] request);
    byte[] Projection(ulong handle, uint kind);
    byte[] Sign(ulong handle, byte[] seed, byte[] proof);
    void Close(ulong handle);
}
// This resource owner has no manifest/admission authority. Its test driver exercises only custody mechanics.
internal sealed class PrivacyX509NativeOwner : IDisposable
{
    private readonly object gate = new();
    private readonly IPrivacyX509Driver driver;
    private ulong handle;
    internal PrivacyX509NativeOwner(IPrivacyX509Driver driver, ulong handle)
    { if (handle == 0) throw new PrivacyX509NativeException(-100); this.driver = driver; this.handle = handle; }
    private ulong RequireHandle() => handle != 0 ? handle : throw new ObjectDisposedException(nameof(PrivacyX509PreparedTransactionV1));
    internal byte[] Projection(uint kind)
    { lock (gate) { return (byte[])driver.Projection(RequireHandle(), kind).Clone(); } }
    internal byte[] Sign(ReadOnlySpan<byte> seed, ReadOnlySpan<byte> proof)
    {
        if (seed.Length != 32 || proof.Length is < 1 or > 9 * 1024 * 1024)
            throw new ArgumentException("X509 signing requires a 32-byte seed and bounded raw credential proof.");
        var secret = seed.ToArray();
        try { lock (gate) { return (byte[])driver.Sign(RequireHandle(), secret, proof.ToArray()).Clone(); } }
        finally { CryptographicOperations.ZeroMemory(secret); }
    }
    public void Dispose()
    {
        lock (gate) { var previous = handle; handle = 0; if (previous != 0) driver.Close(previous); }
        GC.SuppressFinalize(this);
    }
    ~PrivacyX509NativeOwner() { try { if (handle != 0) driver.Close(handle); } catch { /* Cleanup cannot throw from finalization. */ } }
}

internal sealed class PrivacyX509Native : IPrivacyX509Driver
{
    internal static readonly IPrivacyX509Driver Driver = new PrivacyX509Native();
    private const string Library = "connect_norito_bridge";
    private static readonly string[] Exports = ["connect_norito_bridge_abi_version", "iroha_privacy_free_buffer",
        "connect_norito_zk_x509_transaction_prepare_v1", "connect_norito_zk_x509_transaction_projection_v1",
        "connect_norito_zk_x509_transaction_sign_v1", "connect_norito_zk_x509_transaction_close_v1"];
    private static readonly Lazy<bool> Available = new(() =>
    {
        IntPtr library = IntPtr.Zero;
        try { return NativeLibrary.TryLoad(Library, typeof(PrivacyX509Native).Assembly, null, out library)
            && Exports.All(symbol => NativeLibrary.TryGetExport(library, symbol, out _)) && Abi() == PrivacyNative.RequiredBridgeAbiVersion; }
        catch (Exception error) when (error is DllNotFoundException or EntryPointNotFoundException or BadImageFormatException) { return false; }
        finally { if (library != IntPtr.Zero) NativeLibrary.Free(library); }
    });
    public ulong Prepare(byte[] network, byte[] manifest, byte[] payload, byte[] request)
    {
        if (!Available.Value) throw new PrivacyX509NativeException(-101);
        PrivacyX509NativeException.Check(PrepareNative(network, Len(network), manifest, Len(manifest), payload, Len(payload), request, Len(request), out var handle));
        return handle;
    }
    public byte[] Projection(ulong handle, uint kind)
    {
        IntPtr pointer = IntPtr.Zero;
        try { PrivacyX509NativeException.Check(ProjectionNative(handle, kind, out pointer, out var length)); return Copy(pointer, length); }
        finally { if (pointer != IntPtr.Zero) Free(pointer); }
    }
    public byte[] Sign(ulong handle, byte[] seed, byte[] proof)
    {
        IntPtr pointer = IntPtr.Zero;
        try { PrivacyX509NativeException.Check(SignNative(handle, seed, Len(seed), proof, Len(proof), out pointer, out var length)); return Copy(pointer, length); }
        finally { if (pointer != IntPtr.Zero) Free(pointer); }
    }
    public void Close(ulong handle) => PrivacyX509NativeException.Check(CloseNative(handle));
    private static CULong Len(byte[] bytes) => new(checked((uint)bytes.Length));
    private static byte[] Copy(IntPtr pointer, CULong length)
    {
        var count = length.Value.ToUInt64();
        if (pointer == IntPtr.Zero || count is < 1 or > 10 * 1024 * 1024) throw new PrivacyX509NativeException(-100);
        var result = new byte[(int)count]; Marshal.Copy(pointer, result, 0, result.Length); return result;
    }
    [DllImport(Library, EntryPoint = "connect_norito_bridge_abi_version", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint Abi();
    [DllImport(Library, EntryPoint = "connect_norito_zk_x509_transaction_prepare_v1", CallingConvention = CallingConvention.Cdecl)]
    private static extern int PrepareNative(byte[] network, CULong networkLen, byte[] manifest, CULong manifestLen,
        byte[] payload, CULong payloadLen, byte[] request, CULong requestLen, out ulong handle);
    [DllImport(Library, EntryPoint = "connect_norito_zk_x509_transaction_projection_v1", CallingConvention = CallingConvention.Cdecl)]
    private static extern int ProjectionNative(ulong handle, uint kind, out IntPtr output, out CULong length);
    [DllImport(Library, EntryPoint = "connect_norito_zk_x509_transaction_sign_v1", CallingConvention = CallingConvention.Cdecl)]
    private static extern int SignNative(ulong handle, byte[] seed, CULong seedLen, byte[] proof, CULong proofLen, out IntPtr output, out CULong length);
    [DllImport(Library, EntryPoint = "connect_norito_zk_x509_transaction_close_v1", CallingConvention = CallingConvention.Cdecl)]
    private static extern int CloseNative(ulong handle);
    [DllImport(Library, EntryPoint = "iroha_privacy_free_buffer", CallingConvention = CallingConvention.Cdecl)]
    private static extern void Free(IntPtr pointer);
}
