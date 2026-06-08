using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

namespace Hyperledger.Iroha.Privacy;

internal static class PrivacyArchiveBytes
{
    internal static byte[] Copy(
        byte[] noritoBytes,
        string parameterName,
        params byte[] expectedSchemaBytes)
    {
        if (noritoBytes is null)
        {
            throw new ArgumentNullException(parameterName);
        }

        if (noritoBytes.Length == 0)
        {
            throw new ArgumentException("Norito V1 archive must not be empty.", parameterName);
        }

        if (noritoBytes.Length > PrivacyNative.PrivacyNativeArchiveMaxBytes)
        {
            throw new ArgumentException(
                $"Norito V1 archive must not exceed {PrivacyNative.PrivacyNativeArchiveMaxBytes} bytes.",
                parameterName);
        }

        if (!PrivacyNative.IsNoritoV1Archive(noritoBytes))
        {
            throw new ArgumentException(
                "Norito V1 archive must be a valid Norito V1 archive.",
                parameterName);
        }

        if (!PrivacyNative.HasNonEmptyPrivacyNoritoPayload(noritoBytes))
        {
            throw new ArgumentException(
                "Norito V1 archive must contain a non-empty privacy result payload.",
                parameterName);
        }

        if (!PrivacyNative.HasNoritoSchema(noritoBytes, expectedSchemaBytes))
        {
            throw new ArgumentException(
                "Norito V1 archive must use the expected privacy result schema.",
                parameterName);
        }

        return (byte[])noritoBytes.Clone();
    }
}

public sealed class PrivacyCapabilitiesArchive
{
    private readonly byte[] noritoBytes;

    public PrivacyCapabilitiesArchive(byte[] noritoBytes)
    {
        this.noritoBytes = PrivacyArchiveBytes.Copy(
            noritoBytes,
            nameof(noritoBytes),
            PrivacyNative.PrivacyCapabilitiesResultSchemaByte);
    }

    public byte[] NoritoBytes => PrivacyArchiveBytes.Copy(
        noritoBytes,
        nameof(NoritoBytes),
        PrivacyNative.PrivacyCapabilitiesResultSchemaByte);
}

public sealed class PrivacyProofResultArchive
{
    private readonly byte[] noritoBytes;

    public PrivacyProofResultArchive(byte[] noritoBytes)
    {
        this.noritoBytes = PrivacyArchiveBytes.Copy(
            noritoBytes,
            nameof(noritoBytes),
            PrivacyNative.PrivacyBuildProofResultSchemaByte,
            PrivacyNative.PrivacyVerifyProofResultSchemaByte);
    }

    public byte[] NoritoBytes => PrivacyArchiveBytes.Copy(
        noritoBytes,
        nameof(NoritoBytes),
        PrivacyNative.PrivacyBuildProofResultSchemaByte,
        PrivacyNative.PrivacyVerifyProofResultSchemaByte);
}

public sealed class PrivacyCapabilities
{
    private PrivacyCapabilities(
        bool cSharpSdkAvailable,
        bool bridgeAvailable,
        bool productionReady,
        PrivacyProductionGate productionGate)
    {
        CSharpSdkAvailable = cSharpSdkAvailable;
        BridgeAvailable = bridgeAvailable;
        ProductionReady = productionReady;
        ProductionGate = productionGate;
    }

    public bool CSharpSdkAvailable { get; }

    public bool BridgeAvailable { get; }

    public bool ProductionReady { get; }

    public PrivacyProductionGate ProductionGate { get; }

    internal static PrivacyCapabilities FailClosed(bool bridgeAvailable)
    {
        return new PrivacyCapabilities(
            cSharpSdkAvailable: true,
            bridgeAvailable: bridgeAvailable,
            productionReady: false,
            productionGate: PrivacyProductionGate.FailClosed());
    }
}

public sealed class PrivacyProductionGate
{
    private static readonly IReadOnlyList<string> EmptyAuditReferences =
        Array.AsReadOnly(Array.Empty<string>());

    private PrivacyProductionGate(
        string version,
        bool ready,
        bool realProving,
        bool realVerification,
        bool chainAdmission,
        bool sdkParity,
        bool walletState,
        bool witnessPrivacyChecks,
        bool deterministicTests,
        bool negativeAdversarialTests,
        bool fuzzing,
        bool parserFuzzing,
        bool verifierFuzzing,
        bool performanceGates,
        bool externalAudit,
        IReadOnlyList<string> missing,
        IReadOnlyList<string> auditReferences)
    {
        Version = version;
        Ready = ready;
        RealProving = realProving;
        RealVerification = realVerification;
        ChainAdmission = chainAdmission;
        SdkParity = sdkParity;
        WalletState = walletState;
        WitnessPrivacyChecks = witnessPrivacyChecks;
        DeterministicTests = deterministicTests;
        NegativeAdversarialTests = negativeAdversarialTests;
        Fuzzing = fuzzing;
        ParserFuzzing = parserFuzzing;
        VerifierFuzzing = verifierFuzzing;
        PerformanceGates = performanceGates;
        ExternalAudit = externalAudit;
        Missing = missing;
        AuditReferences = auditReferences;
    }

    public static IReadOnlyList<string> MissingReasons { get; } =
        Array.AsReadOnly(new[]
        {
            "real proving engine is not registered",
            "real verifier is not registered",
            "chain admission path is not enabled",
            "cross-SDK parity is incomplete",
            "wallet/state support is incomplete",
            "witness privacy checks are incomplete",
            "deterministic tests are incomplete",
            "negative/adversarial tests are incomplete",
            "fuzzing gate is incomplete",
            "parser fuzzing gate is incomplete",
            "verifier fuzzing gate is incomplete",
            "performance gate is incomplete",
            "external audit signoff is missing",
            "implementation stage is not production-hardened",
            "planned SDK entrypoints remain",
            "dev fixture entrypoints are not production entrypoints",
            "Iroha production allowlist is not enabled for this audited row",
        });

    public string Version { get; }

    public bool Ready { get; }

    public bool RealProving { get; }

    public bool RealVerification { get; }

    public bool ChainAdmission { get; }

    public bool SdkParity { get; }

    public bool WalletState { get; }

    public bool WitnessPrivacyChecks { get; }

    public bool DeterministicTests { get; }

    public bool NegativeAdversarialTests { get; }

    public bool Fuzzing { get; }

    public bool ParserFuzzing { get; }

    public bool VerifierFuzzing { get; }

    public bool PerformanceGates { get; }

    public bool ExternalAudit { get; }

    public IReadOnlyList<string> Missing { get; }

    public IReadOnlyList<string> AuditReferences { get; }

    internal static PrivacyProductionGate FailClosed()
    {
        return new PrivacyProductionGate(
            version: PrivacyNative.ProductionGateVersion,
            ready: false,
            realProving: false,
            realVerification: false,
            chainAdmission: false,
            sdkParity: false,
            walletState: false,
            witnessPrivacyChecks: false,
            deterministicTests: false,
            negativeAdversarialTests: false,
            fuzzing: false,
            parserFuzzing: false,
            verifierFuzzing: false,
            performanceGates: false,
            externalAudit: false,
            missing: MissingReasons,
            auditReferences: EmptyAuditReferences);
    }
}

public static class PrivacyNative
{
    public const uint RequiredBridgeAbiVersion = 6;
    public const uint FfiVersionV1 = 1;
    public const string ProductionGateVersion = "privacy-production-gate-v1";
    public const uint StatusError = 1;
    public const uint ErrorNullPointer = 1;
    public const uint ErrorMalformedNorito = 2;
    public const uint ErrorUnsupportedAlgorithm = 3;
    public const uint ErrorProductionDisabled = 4;
    public const uint ErrorInvalidRequest = 5;
    public const int PrivacyNativeArchiveMaxBytes = 64 * 1024 * 1024;

    private const int PrivacyNoritoHeaderBytes = 40;
    private const int PrivacyNoritoMaxHeaderPaddingBytes = 64;
    private const byte PrivacyNoritoSupportedFlagsMask = 0x27;
    private const byte PrivacyNoritoFieldBitsetFlag = 0x20;
    private const byte PrivacyNoritoFieldBitsetRequiredFlags = 0x06;
    private const ulong PrivacyCrc64ReflectedPoly = 0xC96C_5795_D787_0F42UL;
    internal const byte PrivacyRequestSchemaByte = 0x52;
    internal const byte PrivacyCapabilitiesResultSchemaByte = 0x50;
    internal const byte PrivacyBuildProofResultSchemaByte = 0x42;
    internal const byte PrivacyVerifyProofResultSchemaByte = 0x56;
    private const string LibraryName = "connect_norito_bridge";
    private static readonly byte[] PrivacyNoritoMagic = Encoding.ASCII.GetBytes("NRT0");
    private static readonly ulong[] PrivacyCrc64Table = BuildPrivacyCrc64Table();
    private static readonly byte[] ZeroClearChunk = new byte[4096];
    private static readonly byte[] PrivacyNativeAvailabilityProbeArchiveBytes =
        BuildPrivacyNativeAvailabilityProbeArchive();

    public static bool IsAvailable()
    {
        return IsAvailable(
            () => TryGetAbiVersion(out var version) ? version : null,
            TryProbeRequiredSymbols);
    }

    internal static bool IsAvailable(Func<uint?> abiVersionProbe, Func<bool> requiredSymbolsProbe)
    {
        try
        {
            var version = abiVersionProbe();
            return version is not null
                && version.Value >= RequiredBridgeAbiVersion
                && requiredSymbolsProbe();
        }
        catch (Exception)
        {
            return false;
        }
    }

    public static PrivacyCapabilities GetPrivacyCapabilities()
    {
        return GetPrivacyCapabilities(IsAvailable());
    }

    internal static PrivacyCapabilities GetPrivacyCapabilities(bool bridgeAvailable)
    {
        return PrivacyCapabilities.FailClosed(bridgeAvailable);
    }

    public static PrivacyCapabilitiesArchive CapabilitiesV1()
    {
        return new PrivacyCapabilitiesArchive(CallCapabilities(
            "iroha_privacy_capabilities_v1",
            NativeCapabilities));
    }

    public static PrivacyProofResultArchive BuildProofV1(ReadOnlySpan<byte> requestArchive)
    {
        return new PrivacyProofResultArchive(CallProof(
            requestArchive,
            "iroha_privacy_build_proof_v1",
            NativeBuildProof));
    }

    public static PrivacyProofResultArchive VerifyProofV1(ReadOnlySpan<byte> requestArchive)
    {
        return new PrivacyProofResultArchive(CallProof(
            requestArchive,
            "iroha_privacy_verify_proof_v1",
            NativeVerifyProof));
    }

    internal delegate int NativeCapabilitiesCall(out IntPtr outPtr, out UIntPtr outLen);

    internal delegate int NativeProofCall(
        byte[] requestPtr,
        UIntPtr requestLen,
        out IntPtr outPtr,
        out UIntPtr outLen);

    internal static byte[] CallCapabilities(
        string symbol,
        NativeCapabilitiesCall nativeCall,
        bool requireAbi = true)
    {
        if (requireAbi)
        {
            RequireAbi();
        }

        var expectedSchemas = RequireKnownPrivacyResultSymbol(symbol);

        int code;
        IntPtr outPtr;
        UIntPtr outLen;
        try
        {
            code = nativeCall(out outPtr, out outLen);
        }
        catch (Exception)
        {
            throw new InvalidOperationException($"{symbol} failed.");
        }
        return ReadPrivacyOutput(symbol, code, outPtr, outLen, expectedSchemas);
    }

    internal static byte[] CallProof(
        ReadOnlySpan<byte> requestArchive,
        string symbol,
        NativeProofCall nativeCall,
        bool requireAbi = true)
    {
        return CallProof(requestArchive, symbol, nativeCall, requireAbi, NativeFree);
    }

    internal static byte[] CallProof(
        ReadOnlySpan<byte> requestArchive,
        string symbol,
        NativeProofCall nativeCall,
        bool requireAbi,
        Action<IntPtr> free)
    {
        if (requestArchive.IsEmpty)
        {
            throw new ArgumentException("Request archive must not be empty.", nameof(requestArchive));
        }

        if (requestArchive.Length > PrivacyNativeArchiveMaxBytes)
        {
            throw new ArgumentException(
                $"Request archive must not exceed {PrivacyNativeArchiveMaxBytes} bytes.",
                nameof(requestArchive));
        }

        var request = requestArchive.ToArray();
        try
        {
            if (!IsNoritoV1Archive(request))
            {
                throw new ArgumentException(
                    "Request archive must be a valid Norito V1 archive.",
                    nameof(requestArchive));
            }
            if (!HasNoritoSchema(request, PrivacyRequestSchemaByte))
            {
                throw new ArgumentException(
                    "Request archive must use the privacy request schema.",
                    nameof(requestArchive));
            }
            if (!HasNonEmptyPrivacyNoritoPayload(request))
            {
                throw new ArgumentException(
                    "Request archive must contain a non-empty privacy request payload.",
                    nameof(requestArchive));
            }

            if (requireAbi)
            {
                RequireAbi();
            }

            var expectedSchemas = RequireKnownPrivacyResultSymbol(symbol);

            int code;
            IntPtr outPtr;
            UIntPtr outLen;
            try
            {
                code = nativeCall(request, (UIntPtr)request.Length, out outPtr, out outLen);
            }
            catch (Exception)
            {
                throw new InvalidOperationException($"{symbol} failed.");
            }
            return ReadPrivacyOutput(symbol, code, outPtr, outLen, free, expectedSchemas);
        }
        finally
        {
            Array.Clear(request, 0, request.Length);
        }
    }

    private static void RequireAbi()
    {
        if (!TryGetAbiVersion(out var version))
        {
            throw new InvalidOperationException(
                $"{LibraryName} is unavailable; install the native bridge before using privacy FFI.");
        }

        if (version < RequiredBridgeAbiVersion)
        {
            throw new InvalidOperationException(
                $"{LibraryName} ABI v{RequiredBridgeAbiVersion} is required for privacy FFI, found v{version}.");
        }

        if (!TryProbeRequiredSymbols())
        {
            throw new InvalidOperationException(
                $"{LibraryName} ABI v{RequiredBridgeAbiVersion} privacy FFI surface is incomplete.");
        }
    }

    internal static byte[] ReadPrivacyOutput(
        string symbol,
        int code,
        IntPtr outPtr,
        UIntPtr outLen,
        params byte[] expectedSchemaBytes)
    {
        return ReadPrivacyOutput(symbol, code, outPtr, outLen, NativeFree, expectedSchemaBytes);
    }

    internal static byte[] ReadPrivacyOutput(
        string symbol,
        int code,
        IntPtr outPtr,
        UIntPtr outLen,
        Action<IntPtr> free,
        params byte[] expectedSchemaBytes)
    {
        try
        {
            if (code != 0)
            {
                throw new InvalidOperationException($"{symbol} failed with bridge error code {code}.");
            }

            var schemas = RequireExplicitPrivacyResultSchemas(symbol, expectedSchemaBytes);

            if (outPtr == IntPtr.Zero)
            {
                throw new InvalidOperationException($"{symbol} returned a null output pointer.");
            }

            var length = CheckedArchiveLength(symbol, outLen);
            var result = new byte[length];
            Marshal.Copy(outPtr, result, 0, length);
            if (!IsNoritoV1Archive(result))
            {
                throw new InvalidOperationException($"{symbol} returned invalid Norito V1 archive.");
            }
            if (!HasNonEmptyPrivacyNoritoPayload(result))
            {
                throw new InvalidOperationException(
                    $"{symbol} returned empty privacy result payload.");
            }
            if (!HasNoritoSchema(result, schemas))
            {
                throw new InvalidOperationException(
                    $"{symbol} returned unexpected privacy result schema.");
            }
            return result;
        }
        finally
        {
            if (outPtr != IntPtr.Zero)
            {
                ClearNativeBuffer(outPtr, outLen);
                free(outPtr);
            }
        }
    }

    private static int CheckedArchiveLength(string symbol, UIntPtr outLen)
    {
        var length = outLen.ToUInt64();
        if (length == 0)
        {
            throw new InvalidOperationException($"{symbol} returned empty output.");
        }

        if (length > PrivacyNativeArchiveMaxBytes)
        {
            throw new InvalidOperationException($"{symbol} returned oversized output.");
        }

        return (int)length;
    }

    internal static bool IsValidProbeResult(
        int code,
        IntPtr outPtr,
        UIntPtr outLen,
        params byte[] expectedSchemaBytes)
    {
        var length = outLen.ToUInt64();
        if (code != 0
            || outPtr == IntPtr.Zero
            || length == 0
            || length > PrivacyNativeArchiveMaxBytes
            || expectedSchemaBytes.Length == 0)
        {
            return false;
        }

        var output = new byte[(int)length];
        try
        {
            Marshal.Copy(outPtr, output, 0, output.Length);
            return IsNoritoV1Archive(output)
                && HasNonEmptyPrivacyNoritoPayload(output)
                && HasNoritoSchema(output, expectedSchemaBytes);
        }
        finally
        {
            Array.Clear(output, 0, output.Length);
            ClearNativeBuffer(outPtr, outLen);
        }
    }

    internal static bool IsNoritoV1Archive(byte[] archive)
    {
        if (archive.Length < PrivacyNoritoHeaderBytes
            || archive.Length > PrivacyNativeArchiveMaxBytes)
        {
            return false;
        }

        for (var index = 0; index < PrivacyNoritoMagic.Length; index++)
        {
            if (archive[index] != PrivacyNoritoMagic[index])
            {
                return false;
            }
        }

        if (archive[4] != 0 || archive[5] != 0 || archive[22] != 0)
        {
            return false;
        }

        var flags = archive[39];
        if ((flags & ~PrivacyNoritoSupportedFlagsMask) != 0)
        {
            return false;
        }

        if ((flags & PrivacyNoritoFieldBitsetFlag) != 0
            && (flags & PrivacyNoritoFieldBitsetRequiredFlags) != PrivacyNoritoFieldBitsetRequiredFlags)
        {
            return false;
        }

        var payloadLength = ReadUInt64LittleEndian(archive, 23);
        if (payloadLength > int.MaxValue - PrivacyNoritoHeaderBytes)
        {
            return false;
        }

        var minimumLength = PrivacyNoritoHeaderBytes + (int)payloadLength;
        if (archive.Length < minimumLength)
        {
            return false;
        }

        var paddingLength = archive.Length - minimumLength;
        if (paddingLength > PrivacyNoritoMaxHeaderPaddingBytes)
        {
            return false;
        }

        for (var index = PrivacyNoritoHeaderBytes;
             index < PrivacyNoritoHeaderBytes + paddingLength;
             index++)
        {
            if (archive[index] != 0)
            {
                return false;
            }
        }

        var payloadOffset = PrivacyNoritoHeaderBytes + paddingLength;
        var expectedCrc = ReadUInt64LittleEndian(archive, 31);
        return PrivacyCrc64(archive, payloadOffset, archive.Length - payloadOffset) == expectedCrc;
    }

    internal static bool HasNonEmptyPrivacyNoritoPayload(byte[] archive)
    {
        return IsNoritoV1Archive(archive) && ReadUInt64LittleEndian(archive, 23) > 0;
    }

    internal static bool HasNoritoSchema(byte[] archive, params byte[] expectedSchemaBytes)
    {
        if (expectedSchemaBytes.Length == 0)
        {
            return false;
        }

        if (archive.Length < 22)
        {
            return false;
        }

        foreach (var expectedSchemaByte in expectedSchemaBytes)
        {
            var matches = true;
            for (var index = 6; index < 22; index++)
            {
                if (archive[index] != expectedSchemaByte)
                {
                    matches = false;
                    break;
                }
            }

            if (matches)
            {
                return true;
            }
        }

        return false;
    }

    private static byte[] ExpectedPrivacyResultSchemas(string symbol)
    {
        return symbol switch
        {
            "iroha_privacy_capabilities_v1" => new[] { PrivacyCapabilitiesResultSchemaByte },
            "iroha_privacy_build_proof_v1" => new[] { PrivacyBuildProofResultSchemaByte },
            "iroha_privacy_verify_proof_v1" => new[] { PrivacyVerifyProofResultSchemaByte },
            _ => Array.Empty<byte>(),
        };
    }

    private static byte[] RequireKnownPrivacyResultSymbol(string symbol)
    {
        var schemas = ExpectedPrivacyResultSchemas(symbol);
        if (schemas.Length == 0)
        {
            throw new InvalidOperationException(
                $"{symbol} is not a supported privacy native operation.");
        }
        return schemas;
    }

    private static byte[] RequireExplicitPrivacyResultSchemas(
        string symbol,
        byte[]? expectedSchemaBytes)
    {
        var schemas = RequireKnownPrivacyResultSymbol(symbol);
        if (expectedSchemaBytes is null || expectedSchemaBytes.Length == 0)
        {
            throw new InvalidOperationException(
                $"{symbol} requires explicit privacy result schemas.");
        }
        if (!PrivacyResultSchemasEqual(expectedSchemaBytes, schemas))
        {
            throw new InvalidOperationException(
                $"{symbol} expected privacy result schemas do not match the supported operation.");
        }
        return expectedSchemaBytes;
    }

    private static bool PrivacyResultSchemasEqual(byte[] left, byte[] right)
    {
        if (left.Length != right.Length)
        {
            return false;
        }

        for (var index = 0; index < left.Length; index++)
        {
            if (left[index] != right[index])
            {
                return false;
            }
        }
        return true;
    }

    private static ulong[] BuildPrivacyCrc64Table()
    {
        var table = new ulong[256];
        for (var index = 0; index < table.Length; index++)
        {
            var crc = (ulong)index;
            for (var bit = 0; bit < 8; bit++)
            {
                crc = (crc & 1UL) != 0
                    ? (crc >> 1) ^ PrivacyCrc64ReflectedPoly
                    : crc >> 1;
            }
            table[index] = crc;
        }
        return table;
    }

    private static byte[] BuildPrivacyNativeAvailabilityProbeArchive()
    {
        var archive = new byte[PrivacyNoritoHeaderBytes];
        PrivacyNoritoMagic.CopyTo(archive, 0);
        Array.Fill(archive, PrivacyRequestSchemaByte, 6, 16);
        return archive;
    }

    private static ulong PrivacyCrc64(byte[] archive, int offset, int length)
    {
        var crc = ulong.MaxValue;
        for (var index = offset; index < offset + length; index++)
        {
            crc = PrivacyCrc64Table[(byte)(crc ^ archive[index])] ^ (crc >> 8);
        }
        return crc ^ ulong.MaxValue;
    }

    private static ulong ReadUInt64LittleEndian(byte[] archive, int offset)
    {
        ulong value = 0;
        for (var index = 0; index < 8; index++)
        {
            value |= (ulong)archive[offset + index] << (8 * index);
        }
        return value;
    }

    internal static byte[] PrivacyNativeAvailabilityProbeArchive()
    {
        return (byte[])PrivacyNativeAvailabilityProbeArchiveBytes.Clone();
    }

    private static bool TryProbeRequiredSymbols()
    {
        try
        {
            if (!Probe(NativeCapabilities, PrivacyCapabilitiesResultSchemaByte)
                || !Probe(NativeBuildProof, PrivacyBuildProofResultSchemaByte)
                || !Probe(NativeVerifyProof, PrivacyVerifyProofResultSchemaByte))
            {
                return false;
            }
            NativeFree(IntPtr.Zero);
            return true;
        }
        catch (Exception)
        {
            return false;
        }
    }

    private static bool Probe(NativeCapabilitiesCall nativeCall, byte expectedSchemaByte)
    {
        var code = nativeCall(out var outPtr, out var outLen);
        return ConsumeProbeResult(code, outPtr, outLen, expectedSchemaByte);
    }

    private static bool Probe(NativeProofCall nativeCall, byte expectedSchemaByte)
    {
        var request = PrivacyNativeAvailabilityProbeArchive();
        try
        {
            var code = nativeCall(request, (UIntPtr)request.Length, out var outPtr, out var outLen);
            return ConsumeProbeResult(code, outPtr, outLen, expectedSchemaByte);
        }
        finally
        {
            Array.Clear(request, 0, request.Length);
        }
    }

    private static bool ConsumeProbeResult(
        int code,
        IntPtr outPtr,
        UIntPtr outLen,
        byte expectedSchemaByte)
    {
        try
        {
            return IsValidProbeResult(code, outPtr, outLen, expectedSchemaByte);
        }
        finally
        {
            if (outPtr != IntPtr.Zero)
            {
                ClearNativeBuffer(outPtr, outLen);
                NativeFree(outPtr);
            }
        }
    }

    private static void ClearNativeBuffer(IntPtr ptr, UIntPtr outLen)
    {
        if (ptr == IntPtr.Zero)
        {
            return;
        }

        var length = outLen.ToUInt64();
        if (length == 0 || length > PrivacyNativeArchiveMaxBytes)
        {
            return;
        }

        var remaining = (int)length;
        var offset = 0;
        while (remaining > 0)
        {
            var chunk = Math.Min(remaining, ZeroClearChunk.Length);
            Marshal.Copy(ZeroClearChunk, 0, IntPtr.Add(ptr, offset), chunk);
            remaining -= chunk;
            offset += chunk;
        }
    }

    private static bool TryGetAbiVersion(out uint version)
    {
        try
        {
            version = NativeAbiVersion();
            return true;
        }
        catch (Exception)
        {
            version = 0;
            return false;
        }
    }

    [DllImport(LibraryName, EntryPoint = "connect_norito_bridge_abi_version", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeAbiVersion();

    [DllImport(LibraryName, EntryPoint = "iroha_privacy_capabilities_v1", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeCapabilities(out IntPtr outPtr, out UIntPtr outLen);

    [DllImport(LibraryName, EntryPoint = "iroha_privacy_build_proof_v1", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeBuildProof(byte[] requestPtr, UIntPtr requestLen, out IntPtr outPtr, out UIntPtr outLen);

    [DllImport(LibraryName, EntryPoint = "iroha_privacy_verify_proof_v1", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeVerifyProof(byte[] requestPtr, UIntPtr requestLen, out IntPtr outPtr, out UIntPtr outLen);

    [DllImport(LibraryName, EntryPoint = "iroha_privacy_free_buffer", CallingConvention = CallingConvention.Cdecl)]
    private static extern void NativeFree(IntPtr ptr);
}
