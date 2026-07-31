using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Runtime.InteropServices;

namespace Hyperledger.Iroha.Privacy;

/// <summary>Closed first-release privacy protocol identity in canonical Norito order.</summary>
public enum PrivacyProtocolIdV1
{
    ZkAcePqAuthorizationV0,
    AnonymousPgcKOutOfNV1,
    VeRangeTransparentRangeV1,
    IrohaZkAmsV1,
    VegaExistingCredentialZkV0,
    IrohaZkX509StarkP256V0,
    IrohaJindoPolynomialCommitmentV0,
    IrohaBootleLanternAnoncredV1,
    OrchardHalo2ActionsV1,
    MoneroFcmpPlusPlusV1,
    IrohaIvmPrivateNoteStarkV1,
    PqMaspStarkV0,
}

/// <summary>Stable ABI-21 result of validating one typed local compiled-profile catalog.</summary>
public enum PrivacyCompiledProfileCatalogValidationStatusV1
{
    Valid = 0,
    NullPointer = 1,
    Empty = 2,
    ArchiveTooLarge = 3,
    DecodeResourceLimit = 4,
    SchemaMismatch = 5,
    NonCanonical = 6,
    MalformedArchive = 7,
    InvalidCatalog = 8,
}

/// <summary>Stable ABI-21 result of validating the Rust-derived exact-12 fixture bundle.</summary>
public enum PrivacyExact12FixtureValidationStatusV1
{
    Valid = 0,
    NullPointer = 1,
    Empty = 2,
    ArchiveTooLarge = 3,
    DecodeResourceLimit = 4,
    SchemaMismatch = 5,
    NonCanonical = 6,
    MalformedArchive = 7,
    InvalidBundle = 8,
}

public static class PrivacyProtocolsV1
{
    private static readonly IReadOnlyList<PrivacyProtocolIdV1> Protocols =
        new ReadOnlyCollection<PrivacyProtocolIdV1>(
            Enum.GetValues<PrivacyProtocolIdV1>());

    /// <summary>All twelve identities in exact wire order.</summary>
    public static IReadOnlyList<PrivacyProtocolIdV1> All => Protocols;

    public static string CanonicalLabel(this PrivacyProtocolIdV1 protocol) =>
        protocol switch
        {
            PrivacyProtocolIdV1.ZkAcePqAuthorizationV0 => "zk-ace-pq-authorization-v0",
            PrivacyProtocolIdV1.AnonymousPgcKOutOfNV1 => "anonymous-pgc-k-out-of-n-v1",
            PrivacyProtocolIdV1.VeRangeTransparentRangeV1 => "verange-transparent-range-v1",
            PrivacyProtocolIdV1.IrohaZkAmsV1 => "iroha-zk-ams-v1",
            PrivacyProtocolIdV1.VegaExistingCredentialZkV0 => "vega-existing-credential-zk-v0",
            PrivacyProtocolIdV1.IrohaZkX509StarkP256V0 => "iroha-zk-x509-stark-p256-v0",
            PrivacyProtocolIdV1.IrohaJindoPolynomialCommitmentV0 =>
                "iroha-jindo-polynomial-commitment-v0",
            PrivacyProtocolIdV1.IrohaBootleLanternAnoncredV1 =>
                "iroha-bootle-lantern-anoncred-v1",
            PrivacyProtocolIdV1.OrchardHalo2ActionsV1 => "orchard-halo2-actions-v1",
            PrivacyProtocolIdV1.MoneroFcmpPlusPlusV1 => "monero-fcmp-plus-plus-v1",
            PrivacyProtocolIdV1.IrohaIvmPrivateNoteStarkV1 =>
                "iroha-ivm-private-note-stark-v1",
            PrivacyProtocolIdV1.PqMaspStarkV0 => "pq-masp-stark-v0",
            _ => throw new ArgumentOutOfRangeException(nameof(protocol)),
        };

    /// <summary>
    /// Parse one exact canonical label. Aliases, retired identifiers, whitespace, and case changes
    /// are rejected.
    /// </summary>
    public static PrivacyProtocolIdV1 ParseCanonicalLabel(string label) =>
        label switch
        {
            "zk-ace-pq-authorization-v0" => PrivacyProtocolIdV1.ZkAcePqAuthorizationV0,
            "anonymous-pgc-k-out-of-n-v1" => PrivacyProtocolIdV1.AnonymousPgcKOutOfNV1,
            "verange-transparent-range-v1" => PrivacyProtocolIdV1.VeRangeTransparentRangeV1,
            "iroha-zk-ams-v1" => PrivacyProtocolIdV1.IrohaZkAmsV1,
            "vega-existing-credential-zk-v0" => PrivacyProtocolIdV1.VegaExistingCredentialZkV0,
            "iroha-zk-x509-stark-p256-v0" => PrivacyProtocolIdV1.IrohaZkX509StarkP256V0,
            "iroha-jindo-polynomial-commitment-v0" =>
                PrivacyProtocolIdV1.IrohaJindoPolynomialCommitmentV0,
            "iroha-bootle-lantern-anoncred-v1" =>
                PrivacyProtocolIdV1.IrohaBootleLanternAnoncredV1,
            "orchard-halo2-actions-v1" => PrivacyProtocolIdV1.OrchardHalo2ActionsV1,
            "monero-fcmp-plus-plus-v1" => PrivacyProtocolIdV1.MoneroFcmpPlusPlusV1,
            "iroha-ivm-private-note-stark-v1" => PrivacyProtocolIdV1.IrohaIvmPrivateNoteStarkV1,
            "pq-masp-stark-v0" => PrivacyProtocolIdV1.PqMaspStarkV0,
            _ => throw new ArgumentException(
                "Unknown canonical privacy protocol id.",
                nameof(label)),
        };
}

/// <summary>Validated canonical local <c>PrivacyCompiledProfileCatalogV1</c> archive.</summary>
public sealed class PrivacyCompiledProfileCatalogArchive
{
    private readonly byte[] _noritoBytes;

    internal PrivacyCompiledProfileCatalogArchive(byte[] noritoBytes)
    {
        ArgumentNullException.ThrowIfNull(noritoBytes);
        if (PrivacyNative.ValidateCompiledProfileCatalogV1(noritoBytes)
            != PrivacyCompiledProfileCatalogValidationStatusV1.Valid)
        {
            throw new ArgumentException(
                "Expected this binary's canonical PrivacyCompiledProfileCatalogV1 archive.",
                nameof(noritoBytes));
        }
        _noritoBytes = (byte[])noritoBytes.Clone();
    }

    /// <summary>Returns a defensive copy of the typed Norito archive.</summary>
    public byte[] NoritoBytes => (byte[])_noritoBytes.Clone();
}

/// <summary>
/// Validated canonical Rust-derived bytes through signed-transaction and hash layers for every exact-12 privacy row.
/// </summary>
public sealed class PrivacyExact12FixtureBundleArchive
{
    private readonly byte[] _noritoBytes;

    internal PrivacyExact12FixtureBundleArchive(byte[] noritoBytes)
    {
        ArgumentNullException.ThrowIfNull(noritoBytes);
        if (PrivacyNative.ValidateExact12FixtureBundleV1(noritoBytes)
            != PrivacyExact12FixtureValidationStatusV1.Valid)
        {
            throw new ArgumentException(
                "Expected the canonical Rust-derived exact-12 fixture bundle.",
                nameof(noritoBytes));
        }
        _noritoBytes = (byte[])noritoBytes.Clone();
    }

    /// <summary>Returns a defensive copy of the canonical Norito bundle.</summary>
    public byte[] NoritoBytes => (byte[])_noritoBytes.Clone();
}

/// <summary>
/// Selector-free local privacy build metadata and exact-12 fixture surface. The catalog never
/// establishes network activation or readiness; fetch a fresh authoritative capability snapshot
/// from live Torii before submitting a privacy proof.
/// </summary>
public static class PrivacyNative
{
    public const int PrivacyCompiledProfileCatalogArchiveMaxBytes = 256 * 1024;
    public const int PrivacyExact12FixtureBundleMaxBytes = 2 * 1024 * 1024;
    public const uint RequiredBridgeAbiVersion = 21;
    private const string LibraryName = "connect_norito_bridge";
    private static readonly bool Available = DetectAvailability();
    private delegate int NativeArchiveQuery(out IntPtr output, out UIntPtr outputLength);
    private delegate int NativeArchiveValidator(byte[] archive, UIntPtr archiveLength);

    public static bool IsAvailable() => Available;

    private static bool DetectAvailability()
    {
        IntPtr handle = IntPtr.Zero;
        try
        {
            var symbolsAvailable = NativeLibrary.TryLoad(LibraryName, out handle)
                && NativeLibrary.TryGetExport(
                    handle,
                    "iroha_privacy_compiled_profile_catalog_v1",
                    out _)
                && NativeLibrary.TryGetExport(
                    handle,
                    "iroha_privacy_validate_compiled_profile_catalog_v1",
                    out _)
                && NativeLibrary.TryGetExport(
                    handle,
                    "iroha_privacy_exact12_fixture_bundle_v1",
                    out _)
                && NativeLibrary.TryGetExport(
                    handle,
                    "iroha_privacy_validate_exact12_fixture_bundle_v1",
                    out _)
                && NativeLibrary.TryGetExport(handle, "iroha_privacy_free_buffer", out _)
                && NativeBridgeAbiVersion() == RequiredBridgeAbiVersion;
            return symbolsAvailable
                && ProbeNativeArchive(
                    NativeCompiledProfileCatalog,
                    NativeValidateCompiledProfileCatalog,
                    PrivacyCompiledProfileCatalogArchiveMaxBytes)
                && ProbeNativeArchive(
                    NativeExact12FixtureBundle,
                    NativeValidateExact12FixtureBundle,
                    PrivacyExact12FixtureBundleMaxBytes);
        }
        catch (Exception error) when (
            error is DllNotFoundException
            or EntryPointNotFoundException
            or BadImageFormatException)
        {
            return false;
        }
        finally
        {
            if (handle != IntPtr.Zero)
            {
                NativeLibrary.Free(handle);
            }
        }
    }

    private static bool ProbeNativeArchive(
        NativeArchiveQuery query,
        NativeArchiveValidator validate,
        int maximumBytes)
    {
        IntPtr pointer = IntPtr.Zero;
        UIntPtr length = UIntPtr.Zero;
        var status = query(out pointer, out length);
        try
        {
            var count64 = length.ToUInt64();
            if (status != 0
                || pointer == IntPtr.Zero
                || count64 == 0
                || count64 > (ulong)maximumBytes
                || count64 > int.MaxValue)
            {
                return false;
            }
            var count = (int)count64;
            var bytes = new byte[count];
            Marshal.Copy(pointer, bytes, 0, count);
            return validate(bytes, new UIntPtr((uint)count)) == 0;
        }
        finally
        {
            if (pointer != IntPtr.Zero)
            {
                NativeFree(pointer);
            }
        }
    }

    /// <summary>
    /// Returns canonical Rust-derived bytes through signed-transaction and hash layers for all twelve rows.
    /// </summary>
    public static PrivacyExact12FixtureBundleArchive Exact12FixtureBundleV1()
    {
        if (!IsAvailable())
        {
            throw new InvalidOperationException("Native privacy bridge is unavailable.");
        }

        IntPtr pointer = IntPtr.Zero;
        UIntPtr length = UIntPtr.Zero;
        var status = NativeExact12FixtureBundle(out pointer, out length);
        try
        {
            if (status != 0 || pointer == IntPtr.Zero)
            {
                throw new InvalidOperationException(
                    "Native exact-12 privacy fixture query failed.");
            }
            var count = checked((int)length.ToUInt64());
            if (count <= 0 || count > PrivacyExact12FixtureBundleMaxBytes)
            {
                throw new InvalidOperationException(
                    "Native exact-12 privacy fixture bundle is invalid.");
            }
            var bytes = new byte[count];
            Marshal.Copy(pointer, bytes, 0, count);
            return new PrivacyExact12FixtureBundleArchive(bytes);
        }
        finally
        {
            if (pointer != IntPtr.Zero)
            {
                NativeFree(pointer);
            }
        }
    }

    /// <summary>Returns this binary's canonical local compiled-profile catalog.</summary>
    public static PrivacyCompiledProfileCatalogArchive CompiledProfileCatalogV1()
    {
        if (!IsAvailable())
        {
            throw new InvalidOperationException(
                "Native privacy compiled-profile catalog is unavailable.");
        }

        IntPtr pointer = IntPtr.Zero;
        UIntPtr length = UIntPtr.Zero;
        var status = NativeCompiledProfileCatalog(out pointer, out length);
        try
        {
            if (status != 0 || pointer == IntPtr.Zero)
            {
                throw new InvalidOperationException(
                    "Native privacy compiled-profile catalog query failed.");
            }
            var count = checked((int)length.ToUInt64());
            if (count <= 0 || count > PrivacyCompiledProfileCatalogArchiveMaxBytes)
            {
                throw new InvalidOperationException(
                    "Native privacy compiled-profile catalog is invalid.");
            }
            var bytes = new byte[count];
            Marshal.Copy(pointer, bytes, 0, count);
            return new PrivacyCompiledProfileCatalogArchive(bytes);
        }
        finally
        {
            if (pointer != IntPtr.Zero)
            {
                NativeFree(pointer);
            }
        }
    }

    /// <summary>Validates bytes as the exact compiled-profile catalog of the loaded binary.</summary>
    public static PrivacyCompiledProfileCatalogValidationStatusV1 ValidateCompiledProfileCatalogV1(
        byte[] archive)
    {
        ArgumentNullException.ThrowIfNull(archive);
        if (archive.Length == 0)
        {
            return PrivacyCompiledProfileCatalogValidationStatusV1.Empty;
        }
        if (archive.Length > PrivacyCompiledProfileCatalogArchiveMaxBytes)
        {
            return PrivacyCompiledProfileCatalogValidationStatusV1.ArchiveTooLarge;
        }
        if (!IsAvailable())
        {
            throw new InvalidOperationException(
                "Native privacy compiled-profile catalog is unavailable.");
        }
        var code = NativeValidateCompiledProfileCatalog(
            archive,
            new UIntPtr((uint)archive.Length));
        if (!Enum.IsDefined(typeof(PrivacyCompiledProfileCatalogValidationStatusV1), code))
        {
            throw new InvalidOperationException(
                "Native privacy compiled-profile catalog validation returned an unknown status.");
        }
        return (PrivacyCompiledProfileCatalogValidationStatusV1)code;
    }

    /// <summary>
    /// Validates an untrusted exact-12 fixture bundle against the Rust-compiled canonical bytes.
    /// </summary>
    public static PrivacyExact12FixtureValidationStatusV1 ValidateExact12FixtureBundleV1(
        byte[] archive)
    {
        ArgumentNullException.ThrowIfNull(archive);
        if (archive.Length == 0)
        {
            return PrivacyExact12FixtureValidationStatusV1.Empty;
        }
        if (archive.Length > PrivacyExact12FixtureBundleMaxBytes)
        {
            return PrivacyExact12FixtureValidationStatusV1.ArchiveTooLarge;
        }
        if (!IsAvailable())
        {
            throw new InvalidOperationException("Native privacy bridge is unavailable.");
        }
        var code = NativeValidateExact12FixtureBundle(
            archive,
            new UIntPtr((uint)archive.Length));
        if (!Enum.IsDefined(typeof(PrivacyExact12FixtureValidationStatusV1), code))
        {
            throw new InvalidOperationException(
                "Native exact-12 privacy fixture validation returned an unknown status.");
        }
        return (PrivacyExact12FixtureValidationStatusV1)code;
    }

    [DllImport(
        LibraryName,
        EntryPoint = "connect_norito_bridge_abi_version",
        CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeBridgeAbiVersion();

    [DllImport(
        LibraryName,
        EntryPoint = "iroha_privacy_compiled_profile_catalog_v1",
        CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeCompiledProfileCatalog(
        out IntPtr output,
        out UIntPtr outputLength);

    [DllImport(
        LibraryName,
        EntryPoint = "iroha_privacy_validate_compiled_profile_catalog_v1",
        CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateCompiledProfileCatalog(
        [In] byte[] archive,
        UIntPtr archiveLength);

    [DllImport(
        LibraryName,
        EntryPoint = "iroha_privacy_exact12_fixture_bundle_v1",
        CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeExact12FixtureBundle(
        out IntPtr output,
        out UIntPtr outputLength);

    [DllImport(
        LibraryName,
        EntryPoint = "iroha_privacy_validate_exact12_fixture_bundle_v1",
        CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateExact12FixtureBundle(
        [In] byte[] archive,
        UIntPtr archiveLength);

    [DllImport(
        LibraryName,
        EntryPoint = "iroha_privacy_free_buffer",
        CallingConvention = CallingConvention.Cdecl)]
    private static extern void NativeFree(IntPtr pointer);
}
