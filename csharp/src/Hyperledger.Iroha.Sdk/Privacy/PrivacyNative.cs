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

/// <summary>Stable ABI-21 result of validating one typed privacy capability archive.</summary>
public enum PrivacyCapabilityValidationStatusV1
{
    Valid = 0,
    NullPointer = 1,
    Empty = 2,
    ArchiveTooLarge = 3,
    DecodeResourceLimit = 4,
    SchemaMismatch = 5,
    NonCanonical = 6,
    MalformedArchive = 7,
    InvalidSnapshot = 8,
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

/// <summary>Validated canonical <c>PrivacyCapabilitySnapshotV1</c> Norito archive.</summary>
public sealed class PrivacyCapabilitiesArchive
{
    private readonly byte[] _noritoBytes;

    internal PrivacyCapabilitiesArchive(byte[] noritoBytes)
    {
        ArgumentNullException.ThrowIfNull(noritoBytes);
        if (PrivacyNative.ValidateCapabilitiesV1(noritoBytes)
            != PrivacyCapabilityValidationStatusV1.Valid)
        {
            throw new ArgumentException(
                "Expected a canonical PrivacyCapabilitySnapshotV1 Norito archive.",
                nameof(noritoBytes));
        }
        _noritoBytes = (byte[])noritoBytes.Clone();
    }

    /// <summary>Returns a defensive copy of the typed Norito archive.</summary>
    public byte[] NoritoBytes => (byte[])_noritoBytes.Clone();
}

/// <summary>
/// Capability-only native privacy surface. Generic proof request/build/verify dispatch is
/// deliberately absent; proof protocols expose typed APIs.
/// </summary>
public static class PrivacyNative
{
    public const int PrivacyNativeArchiveMaxBytes = 256 * 1024;
    public const uint RequiredBridgeAbiVersion = 21;
    private const string LibraryName = "connect_norito_bridge";
    private static readonly bool Available = DetectAvailability();

    public static bool IsAvailable() => Available;

    private static bool DetectAvailability()
    {
        IntPtr handle = IntPtr.Zero;
        try
        {
            return NativeLibrary.TryLoad(LibraryName, out handle)
                && NativeLibrary.TryGetExport(handle, "iroha_privacy_capabilities_v1", out _)
                && NativeLibrary.TryGetExport(
                    handle,
                    "iroha_privacy_validate_capabilities_v1",
                    out _)
                && NativeLibrary.TryGetExport(handle, "iroha_privacy_free_buffer", out _)
                && NativeBridgeAbiVersion() == RequiredBridgeAbiVersion;
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

    public static PrivacyCapabilitiesArchive CapabilitiesV1()
    {
        if (!IsAvailable())
        {
            throw new InvalidOperationException("Native privacy capability bridge is unavailable.");
        }

        IntPtr pointer = IntPtr.Zero;
        UIntPtr length = UIntPtr.Zero;
        var status = NativeCapabilities(out pointer, out length);
        try
        {
            if (status != 0 || pointer == IntPtr.Zero)
            {
                throw new InvalidOperationException("Native privacy capability query failed.");
            }
            var count = checked((int)length.ToUInt64());
            if (count <= 0 || count > PrivacyNativeArchiveMaxBytes)
            {
                throw new InvalidOperationException("Native privacy capability archive is invalid.");
            }
            var bytes = new byte[count];
            Marshal.Copy(pointer, bytes, 0, count);
            return new PrivacyCapabilitiesArchive(bytes);
        }
        finally
        {
            if (pointer != IntPtr.Zero)
            {
                NativeFree(pointer);
            }
        }
    }

    public static PrivacyCapabilityValidationStatusV1 ValidateCapabilitiesV1(byte[] archive)
    {
        ArgumentNullException.ThrowIfNull(archive);
        if (archive.Length == 0)
        {
            return PrivacyCapabilityValidationStatusV1.Empty;
        }
        if (archive.Length > PrivacyNativeArchiveMaxBytes)
        {
            return PrivacyCapabilityValidationStatusV1.ArchiveTooLarge;
        }
        if (!IsAvailable())
        {
            throw new InvalidOperationException("Native privacy capability bridge is unavailable.");
        }
        var code = NativeValidateCapabilities(archive, new UIntPtr((uint)archive.Length));
        if (!Enum.IsDefined(typeof(PrivacyCapabilityValidationStatusV1), code))
        {
            throw new InvalidOperationException(
                "Native privacy capability validation returned an unknown status.");
        }
        return (PrivacyCapabilityValidationStatusV1)code;
    }

    [DllImport(
        LibraryName,
        EntryPoint = "connect_norito_bridge_abi_version",
        CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeBridgeAbiVersion();

    [DllImport(
        LibraryName,
        EntryPoint = "iroha_privacy_capabilities_v1",
        CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeCapabilities(out IntPtr output, out UIntPtr outputLength);

    [DllImport(
        LibraryName,
        EntryPoint = "iroha_privacy_validate_capabilities_v1",
        CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateCapabilities(
        [In] byte[] archive,
        UIntPtr archiveLength);

    [DllImport(
        LibraryName,
        EntryPoint = "iroha_privacy_free_buffer",
        CallingConvention = CallingConvention.Cdecl)]
    private static extern void NativeFree(IntPtr pointer);
}
