using System.Collections.Generic;
using System.Runtime.InteropServices;
using Hyperledger.Iroha.Privacy;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class PrivacyNativeTests
{
    [Fact]
    public void PrivacyNativeAvailabilityProbeDoesNotThrow()
    {
        _ = PrivacyNative.IsAvailable();
    }

    [Fact]
    public void PrivacyNativeAvailabilityRequiresCompleteAbiSurface()
    {
        Assert.True(PrivacyNative.IsAvailable(() => 6u, () => true));
        Assert.False(PrivacyNative.IsAvailable(() => null, () => true));
        Assert.False(PrivacyNative.IsAvailable(() => 5u, () => true));
        Assert.False(PrivacyNative.IsAvailable(() => 6u, () => false));
        Assert.False(PrivacyNative.IsAvailable(
            () => throw new DllNotFoundException("missing bridge"),
            () => true));
        Assert.False(PrivacyNative.IsAvailable(
            () => throw new EntryPointNotFoundException("missing ABI probe"),
            () => true));
        Assert.False(PrivacyNative.IsAvailable(
            () => throw new BadImageFormatException("wrong architecture"),
            () => true));
        Assert.False(PrivacyNative.IsAvailable(
            () => throw new InvalidCastException("unexpected ABI probe failure"),
            () => true));
        Assert.False(PrivacyNative.IsAvailable(
            () => throw new ApplicationException("managed ABI shim failure"),
            () => true));
        Assert.False(PrivacyNative.IsAvailable(
            () => 6u,
            () => throw new EntryPointNotFoundException("missing privacy symbol")));
        Assert.False(PrivacyNative.IsAvailable(
            () => 6u,
            () => throw new InvalidOperationException("privacy probe failed")));
        Assert.False(PrivacyNative.IsAvailable(
            () => 6u,
            () => throw new ArithmeticException("unexpected privacy probe failure")));
        Assert.False(PrivacyNative.IsAvailable(
            () => 6u,
            () => throw new ApplicationException("managed privacy probe failure")));
    }

    [Fact]
    public void PrivacyNativeConstantsMatchRustFfiContract()
    {
        Assert.Equal(6u, PrivacyNative.RequiredBridgeAbiVersion);
        Assert.Equal(1u, PrivacyNative.FfiVersionV1);
        Assert.Equal("privacy-production-gate-v1", PrivacyNative.ProductionGateVersion);
        Assert.Equal(1u, PrivacyNative.StatusError);
        Assert.Equal(1u, PrivacyNative.ErrorNullPointer);
        Assert.Equal(2u, PrivacyNative.ErrorMalformedNorito);
        Assert.Equal(3u, PrivacyNative.ErrorUnsupportedAlgorithm);
        Assert.Equal(4u, PrivacyNative.ErrorProductionDisabled);
        Assert.Equal(5u, PrivacyNative.ErrorInvalidRequest);
        Assert.Equal(64 * 1024 * 1024, PrivacyNative.PrivacyNativeArchiveMaxBytes);
    }

    [Fact]
    public void PrivacyNativeReportsFailClosedPrivacyCapabilities()
    {
        var current = PrivacyNative.GetPrivacyCapabilities();
        Assert.True(current.CSharpSdkAvailable);
        Assert.Equal(PrivacyNative.IsAvailable(), current.BridgeAvailable);
        AssertFailClosedProductionGate(current);

        var bridgeAvailable = PrivacyNative.GetPrivacyCapabilities(bridgeAvailable: true);
        Assert.True(bridgeAvailable.CSharpSdkAvailable);
        Assert.True(bridgeAvailable.BridgeAvailable);
        AssertFailClosedProductionGate(bridgeAvailable);

        var bridgeUnavailable = PrivacyNative.GetPrivacyCapabilities(bridgeAvailable: false);
        Assert.True(bridgeUnavailable.CSharpSdkAvailable);
        Assert.False(bridgeUnavailable.BridgeAvailable);
        AssertFailClosedProductionGate(bridgeUnavailable);

        var missing = Assert.IsAssignableFrom<IList<string>>(bridgeAvailable.ProductionGate.Missing);
        Assert.Throws<NotSupportedException>(() => missing.Add("tampered"));
        var auditReferences = Assert.IsAssignableFrom<IList<string>>(
            bridgeAvailable.ProductionGate.AuditReferences);
        Assert.Throws<NotSupportedException>(() =>
            auditReferences.Add("https://audit.example/forged-signoff"));

        var fresh = PrivacyNative.GetPrivacyCapabilities(bridgeAvailable: true);
        Assert.DoesNotContain("tampered", fresh.ProductionGate.Missing);
        Assert.DoesNotContain(
            "https://audit.example/forged-signoff",
            fresh.ProductionGate.AuditReferences);
        Assert.Equal(PrivacyProductionGate.MissingReasons, fresh.ProductionGate.Missing);
    }

    [Fact]
    public void PrivacyNativeArchiveWrappersDefensivelyCopyNoritoBytes()
    {
        var capabilitiesBytes = PrivacyNoritoFrameWithPayload(0x50);
        var capabilities = new PrivacyCapabilitiesArchive(capabilitiesBytes);
        capabilitiesBytes[0] = 0x7f;

        var firstCapabilitiesRead = capabilities.NoritoBytes;
        Assert.Equal(PrivacyNoritoFrameWithPayload(0x50), firstCapabilitiesRead);
        firstCapabilitiesRead[1] = 0x7f;
        Assert.Equal(PrivacyNoritoFrameWithPayload(0x50), capabilities.NoritoBytes);
        var maxPaddedCapabilities =
            new PrivacyCapabilitiesArchive(PrivacyNoritoFrameWithPadding(0x50, 64));
        Assert.Equal(PrivacyNoritoFrameWithPadding(0x50, 64), maxPaddedCapabilities.NoritoBytes);
        var capabilitiesSchemaError = Assert.Throws<ArgumentException>(() =>
            new PrivacyCapabilitiesArchive(PrivacyNoritoFrameWithPayload(0x42)));
        Assert.Contains("expected privacy result schema", capabilitiesSchemaError.Message);
        Assert.Equal("noritoBytes", capabilitiesSchemaError.ParamName);

        var proofBytes = PrivacyNoritoFrameWithPayload(0x42);
        var proof = new PrivacyProofResultArchive(proofBytes);
        proofBytes[0] = 0x7f;

        var firstProofRead = proof.NoritoBytes;
        Assert.Equal(PrivacyNoritoFrameWithPayload(0x42), firstProofRead);
        firstProofRead[2] = 0x7f;
        Assert.Equal(PrivacyNoritoFrameWithPayload(0x42), proof.NoritoBytes);
        var verifyProof = new PrivacyProofResultArchive(PrivacyNoritoFrameWithPayload(0x56));
        Assert.Equal(PrivacyNoritoFrameWithPayload(0x56), verifyProof.NoritoBytes);
        var proofSchemaError = Assert.Throws<ArgumentException>(() =>
            new PrivacyProofResultArchive(PrivacyNoritoFrameWithPayload(0x50)));
        Assert.Contains("expected privacy result schema", proofSchemaError.Message);
        Assert.Equal("noritoBytes", proofSchemaError.ParamName);
    }

    [Fact]
    public void PrivacyNativeSchemaMatcherRequiresExplicitExpectedSchemas()
    {
        var capabilitiesBytes = PrivacyNoritoFrameWithPayload(0x50);

        Assert.False(PrivacyNative.HasNoritoSchema(capabilitiesBytes));
        Assert.False(PrivacyNative.HasNoritoSchema(capabilitiesBytes, 0x42));
        Assert.True(PrivacyNative.HasNoritoSchema(capabilitiesBytes, 0x50));
        Assert.True(PrivacyNative.HasNoritoSchema(capabilitiesBytes, 0x42, 0x50));
    }

    [Fact]
    public void PrivacyNativeArchiveWrappersRejectUnsafeNoritoBytes()
    {
        var capabilitiesError = Assert.Throws<ArgumentException>(() =>
            new PrivacyCapabilitiesArchive(Array.Empty<byte>()));
        Assert.Contains("must not be empty", capabilitiesError.Message);
        Assert.Equal("noritoBytes", capabilitiesError.ParamName);

        var proofError = Assert.Throws<ArgumentException>(() =>
            new PrivacyProofResultArchive(Array.Empty<byte>()));
        Assert.Contains("must not be empty", proofError.Message);
        Assert.Equal("noritoBytes", proofError.ParamName);

        var emptyPayloadCapabilities = Assert.Throws<ArgumentException>(() =>
            new PrivacyCapabilitiesArchive(PrivacyNoritoFrame(0x50)));
        Assert.Contains("non-empty privacy result payload", emptyPayloadCapabilities.Message);
        Assert.Equal("noritoBytes", emptyPayloadCapabilities.ParamName);

        var emptyPayloadBuildProof = Assert.Throws<ArgumentException>(() =>
            new PrivacyProofResultArchive(PrivacyNoritoFrame(0x42)));
        Assert.Contains("non-empty privacy result payload", emptyPayloadBuildProof.Message);
        Assert.Equal("noritoBytes", emptyPayloadBuildProof.ParamName);

        var emptyPayloadVerifyProof = Assert.Throws<ArgumentException>(() =>
            new PrivacyProofResultArchive(PrivacyNoritoFrame(0x56)));
        Assert.Contains("non-empty privacy result payload", emptyPayloadVerifyProof.Message);
        Assert.Equal("noritoBytes", emptyPayloadVerifyProof.ParamName);

        foreach (var malformed in InvalidPrivacyRequestArchives())
        {
            var malformedCapabilities = Assert.Throws<ArgumentException>(() =>
                new PrivacyCapabilitiesArchive(malformed));
            Assert.Contains("valid Norito V1 archive", malformedCapabilities.Message);
            Assert.Equal("noritoBytes", malformedCapabilities.ParamName);

            var malformedProof = Assert.Throws<ArgumentException>(() =>
                new PrivacyProofResultArchive(malformed));
            Assert.Contains("valid Norito V1 archive", malformedProof.Message);
            Assert.Equal("noritoBytes", malformedProof.ParamName);
        }

        var oversized = new byte[PrivacyNative.PrivacyNativeArchiveMaxBytes + 1];
        var oversizedCapabilities = Assert.Throws<ArgumentException>(() =>
            new PrivacyCapabilitiesArchive(oversized));
        Assert.Contains("must not exceed", oversizedCapabilities.Message);
        Assert.Equal("noritoBytes", oversizedCapabilities.ParamName);
    }

    [Fact]
    public void PrivacyNativeRejectsEmptyProofRequestArchivesBeforeLoadingNativeBridge()
    {
        Assert.Throws<ArgumentException>(() => PrivacyNative.BuildProofV1(Array.Empty<byte>()));
        Assert.Throws<ArgumentException>(() => PrivacyNative.VerifyProofV1(Array.Empty<byte>()));

        var buildEmptyPayload = Assert.Throws<ArgumentException>(() =>
            PrivacyNative.BuildProofV1(PrivacyNoritoFrame(0x52)));
        var verifyEmptyPayload = Assert.Throws<ArgumentException>(() =>
            PrivacyNative.VerifyProofV1(PrivacyNoritoFrame(0x52)));
        Assert.Contains("non-empty privacy request payload", buildEmptyPayload.Message);
        Assert.Contains("non-empty privacy request payload", verifyEmptyPayload.Message);
        Assert.Equal("requestArchive", buildEmptyPayload.ParamName);
        Assert.Equal("requestArchive", verifyEmptyPayload.ParamName);
    }

    [Fact]
    public void PrivacyNativeRejectsOversizedProofRequestArchivesBeforeLoadingNativeBridge()
    {
        var oversized = new byte[PrivacyNative.PrivacyNativeArchiveMaxBytes + 1];

        var buildError = Assert.Throws<ArgumentException>(() =>
            PrivacyNative.BuildProofV1(oversized));
        var verifyError = Assert.Throws<ArgumentException>(() =>
            PrivacyNative.VerifyProofV1(oversized));

        Assert.Contains("must not exceed", buildError.Message);
        Assert.Contains("must not exceed", verifyError.Message);
        Assert.Equal("requestArchive", buildError.ParamName);
        Assert.Equal("requestArchive", verifyError.ParamName);
    }

    [Fact]
    public void PrivacyNativeAcceptsMaxPaddingProofRequestArchivesBeforeNativeDispatch()
    {
        var requestArchive = PrivacyNoritoFrameWithPadding(0x52, 64);

        var buildArchive = PrivacyNative.CallProof(
            requestArchive,
            "iroha_privacy_build_proof_v1",
            (byte[] requestPtr, UIntPtr requestLen, out IntPtr outPtr, out UIntPtr outLen) =>
            {
                Assert.Equal(requestArchive, requestPtr);
                var output = PrivacyNoritoFrameWithPayload(0x42);
                outPtr = Marshal.AllocHGlobal(output.Length);
                Marshal.Copy(output, 0, outPtr, output.Length);
                outLen = (UIntPtr)output.Length;
                return 0;
            },
            requireAbi: false,
            free: Marshal.FreeHGlobal);

        Assert.Equal(PrivacyNoritoFrameWithPayload(0x42), buildArchive);
    }

    [Fact]
    public void PrivacyNativeAcceptsCompleteFieldBitsetProofRequestArchives()
    {
        var requestArchive = PrivacyNoritoFrameWithFlags(0x52, 0x26);
        var expectedOutput = PrivacyNoritoFrameWithFlags(0x42, 0x26);

        var buildArchive = PrivacyNative.CallProof(
            requestArchive,
            "iroha_privacy_build_proof_v1",
            (byte[] requestPtr, UIntPtr requestLen, out IntPtr outPtr, out UIntPtr outLen) =>
            {
                Assert.Equal(requestArchive, requestPtr);
                outPtr = Marshal.AllocHGlobal(expectedOutput.Length);
                Marshal.Copy(expectedOutput, 0, outPtr, expectedOutput.Length);
                outLen = (UIntPtr)expectedOutput.Length;
                return 0;
            },
            requireAbi: false,
            free: Marshal.FreeHGlobal);

        Assert.Equal(expectedOutput, buildArchive);
    }

    [Fact]
    public void PrivacyNativeRejectsInvalidProofRequestArchivesBeforeNativeDispatch()
    {
        var emptyPayloadRequest = PrivacyNoritoFrame(0x52);
        var emptyBuildPayloadError = Assert.Throws<ArgumentException>(() =>
            PrivacyNative.CallProof(
                emptyPayloadRequest,
                "iroha_privacy_build_proof_v1",
                (byte[] requestPtr, UIntPtr requestLen, out IntPtr outPtr, out UIntPtr outLen) =>
                {
                    throw new InvalidOperationException(
                        "empty-payload build request reached native dispatch");
                },
                requireAbi: false));
        var emptyVerifyPayloadError = Assert.Throws<ArgumentException>(() =>
            PrivacyNative.CallProof(
                emptyPayloadRequest,
                "iroha_privacy_verify_proof_v1",
                (byte[] requestPtr, UIntPtr requestLen, out IntPtr outPtr, out UIntPtr outLen) =>
                {
                    throw new InvalidOperationException(
                        "empty-payload verify request reached native dispatch");
                },
                requireAbi: false));

        Assert.Contains(
            "non-empty privacy request payload",
            emptyBuildPayloadError.Message);
        Assert.Contains(
            "non-empty privacy request payload",
            emptyVerifyPayloadError.Message);
        Assert.Equal("requestArchive", emptyBuildPayloadError.ParamName);
        Assert.Equal("requestArchive", emptyVerifyPayloadError.ParamName);

        foreach (var malformed in InvalidPrivacyRequestArchives())
        {
            var buildError = Assert.Throws<ArgumentException>(() =>
                PrivacyNative.CallProof(
                    malformed,
                    "iroha_privacy_build_proof_v1",
                    (byte[] requestPtr, UIntPtr requestLen, out IntPtr outPtr, out UIntPtr outLen) =>
                    {
                        throw new InvalidOperationException(
                            "invalid build request reached native dispatch");
                    },
                    requireAbi: false));
            var verifyError = Assert.Throws<ArgumentException>(() =>
                PrivacyNative.CallProof(
                    malformed,
                    "iroha_privacy_verify_proof_v1",
                    (byte[] requestPtr, UIntPtr requestLen, out IntPtr outPtr, out UIntPtr outLen) =>
                    {
                        throw new InvalidOperationException(
                            "invalid verify request reached native dispatch");
                    },
                    requireAbi: false));

            Assert.Contains("valid Norito V1 archive", buildError.Message);
            Assert.Contains("valid Norito V1 archive", verifyError.Message);
            Assert.Equal("requestArchive", buildError.ParamName);
            Assert.Equal("requestArchive", verifyError.ParamName);
        }
    }

    [Fact]
    public void PrivacyNativeRejectsWrongSchemaProofRequestArchivesBeforeNativeDispatch()
    {
        foreach (var forgedRequest in WrongSchemaPrivacyRequestArchives())
        {
            var buildError = Assert.Throws<ArgumentException>(() =>
                PrivacyNative.CallProof(
                    forgedRequest,
                    "iroha_privacy_build_proof_v1",
                    (byte[] requestPtr, UIntPtr requestLen, out IntPtr outPtr, out UIntPtr outLen) =>
                    {
                        throw new InvalidOperationException(
                            "wrong-schema build request reached native dispatch");
                    },
                    requireAbi: false));
            var verifyError = Assert.Throws<ArgumentException>(() =>
                PrivacyNative.CallProof(
                    forgedRequest,
                    "iroha_privacy_verify_proof_v1",
                    (byte[] requestPtr, UIntPtr requestLen, out IntPtr outPtr, out UIntPtr outLen) =>
                    {
                        throw new InvalidOperationException(
                            "wrong-schema verify request reached native dispatch");
                    },
                    requireAbi: false));

            Assert.Contains("privacy request schema", buildError.Message);
            Assert.Contains("privacy request schema", verifyError.Message);
            Assert.Equal("requestArchive", buildError.ParamName);
            Assert.Equal("requestArchive", verifyError.ParamName);
        }
    }

    [Fact]
    public void PrivacyNativeProbeRequiresSuccessfulNonemptyOutput()
    {
        Assert.False(IsValidProbeOutput(0, PrivacyNoritoFrame(0x50)));
        Assert.False(IsValidProbeOutput(0, PrivacyNoritoFrameWithPayload(0x51)));
        Assert.True(IsValidProbeOutput(0, PrivacyNoritoFrameWithPadding(0x50, 64), 0x50));
        Assert.True(IsValidProbeOutput(0, PrivacyNoritoFrameWithPayload(0x50), 0x50));
        Assert.True(IsValidProbeOutput(0, PrivacyNoritoFrameWithPayload(0x42), 0x42));
        Assert.True(IsValidProbeOutput(0, PrivacyNoritoFrameWithPayload(0x56), 0x56));
        Assert.True(IsValidProbeOutput(0, PrivacyNoritoFrameWithFlags(0x42, 0x26), 0x42));
        Assert.False(IsValidProbeOutput(0, PrivacyNoritoFrameWithPayload(0x42), 0x50));
        Assert.False(IsValidProbeOutput(0, PrivacyNoritoFrameWithPayload(0x56), 0x42));
        Assert.False(IsValidProbeOutput(0, PrivacyNoritoFrameWithPayload(0x50), 0x56));
        Assert.False(IsValidProbeOutput(0, new byte[] { 1 }));
        Assert.False(IsValidProbeOutput(0, InvalidPrivacyNoritoFrame(0, (byte)'X')));
        Assert.False(IsValidProbeOutput(0, InvalidPrivacyNoritoFrame(4, 1)));
        Assert.False(IsValidProbeOutput(0, InvalidPrivacyNoritoFrame(5, 1)));
        Assert.False(IsValidProbeOutput(0, InvalidPrivacyNoritoFrame(22, 1)));
        Assert.False(IsValidProbeOutput(0, InvalidPrivacyNoritoDeclaredPayloadLength(0x50)));
        Assert.False(IsValidProbeOutput(0, InvalidPrivacyNoritoOversizedPayloadLength(0x50)));
        Assert.False(IsValidProbeOutput(0, InvalidPrivacyNoritoFrame(39, 0x40)));
        Assert.False(IsValidProbeOutput(0, InvalidPrivacyNoritoFrame(39, 0x20)));
        Assert.False(IsValidProbeOutput(0, InvalidPrivacyNoritoWithNonzeroPadding()));
        Assert.False(IsValidProbeOutput(0, InvalidPrivacyNoritoWithExcessivePadding()));
        Assert.False(IsValidProbeOutput(0, InvalidPrivacyNoritoFrame(31, 1)));
        Assert.False(IsValidProbeOutput(0, InvalidPrivacyNoritoPayloadTamper()));
        Assert.False(PrivacyNative.IsValidProbeResult(-1, new IntPtr(1), (UIntPtr)1));
        Assert.False(PrivacyNative.IsValidProbeResult(0, IntPtr.Zero, (UIntPtr)1));
        Assert.False(PrivacyNative.IsValidProbeResult(0, IntPtr.Zero, UIntPtr.Zero));
        Assert.False(PrivacyNative.IsValidProbeResult(
            0,
            IntPtr.Zero,
            (UIntPtr)((ulong)PrivacyNative.PrivacyNativeArchiveMaxBytes + 1UL)));
    }

    [Fact]
    public void PrivacyNativeAvailabilityProbeArchiveIsStableAndDefensive()
    {
        var first = PrivacyNative.PrivacyNativeAvailabilityProbeArchive();
        var second = PrivacyNative.PrivacyNativeAvailabilityProbeArchive();

        Assert.NotSame(first, second);
        Assert.Equal(PrivacyNoritoFrame(0x52), first);
        Assert.True(PrivacyNative.IsNoritoV1Archive(first));
        Assert.NotEqual(
            System.Text.Encoding.UTF8.GetBytes("iroha-privacy-native-availability-probe-v1"),
            first);
        first[0] = 0x7f;
        Assert.Equal(PrivacyNoritoFrame(0x52), second);
    }

    [Fact]
    public void PrivacyNativeReadOutputPropagatesBridgeErrors()
    {
        var error = Assert.Throws<InvalidOperationException>(() =>
            PrivacyNative.ReadPrivacyOutput(
                "iroha_privacy_verify_proof_v1",
                -311,
                IntPtr.Zero,
                UIntPtr.Zero,
                _ => { },
                PrivacyNative.PrivacyVerifyProofResultSchemaByte));

        Assert.Contains("iroha_privacy_verify_proof_v1", error.Message);
        Assert.Contains("-311", error.Message);
    }

    [Fact]
    public void PrivacyNativeReadOutputFreesNonNullPointerOnBridgeErrors()
    {
        var freed = false;
        var pointer = new IntPtr(1);

        var error = Assert.Throws<InvalidOperationException>(() =>
            PrivacyNative.ReadPrivacyOutput(
                "iroha_privacy_verify_proof_v1",
                -311,
                pointer,
                (UIntPtr)1,
                ptr =>
                {
                    Assert.Equal(pointer, ptr);
                    freed = true;
                },
                PrivacyNative.PrivacyVerifyProofResultSchemaByte));

        Assert.True(freed);
        Assert.Contains("-311", error.Message);
    }

    [Fact]
    public void PrivacyNativeReadOutputRejectsNullSuccessPointer()
    {
        var error = Assert.Throws<InvalidOperationException>(() =>
            PrivacyNative.ReadPrivacyOutput(
                "iroha_privacy_build_proof_v1",
                0,
                IntPtr.Zero,
                UIntPtr.Zero,
                _ => { },
                PrivacyNative.PrivacyBuildProofResultSchemaByte));

        Assert.Contains("null output pointer", error.Message);
    }

    [Fact]
    public void PrivacyNativeReadOutputRejectsEmptySuccessArchiveAndFreesPointer()
    {
        var freed = false;
        var pointer = new IntPtr(1);

        var error = Assert.Throws<InvalidOperationException>(() =>
            PrivacyNative.ReadPrivacyOutput(
                "iroha_privacy_capabilities_v1",
                0,
                pointer,
                UIntPtr.Zero,
                ptr =>
                {
                    Assert.Equal(pointer, ptr);
                    freed = true;
                },
                PrivacyNative.PrivacyCapabilitiesResultSchemaByte));

        Assert.True(freed);
        Assert.Contains("empty output", error.Message);
    }

    [Fact]
    public void PrivacyNativeReadOutputRejectsEmptyPayloadSuccessArchiveAndFreesPointer()
    {
        AssertReadOutputRejectsEmptyPayload("iroha_privacy_capabilities_v1", 0x50);
        AssertReadOutputRejectsEmptyPayload("iroha_privacy_build_proof_v1", 0x42);
        AssertReadOutputRejectsEmptyPayload("iroha_privacy_verify_proof_v1", 0x56);
    }

    private static void AssertReadOutputRejectsEmptyPayload(string symbol, byte schemaByte)
    {
        var bytes = PrivacyNoritoFrame(schemaByte);
        var pointer = Marshal.AllocHGlobal(bytes.Length);
        var freed = false;
        try
        {
            Marshal.Copy(bytes, 0, pointer, bytes.Length);

            var error = Assert.Throws<InvalidOperationException>(() =>
                PrivacyNative.ReadPrivacyOutput(
                    symbol,
                    0,
                    pointer,
                    (UIntPtr)bytes.Length,
                    ptr =>
                    {
                        Assert.Equal(pointer, ptr);
                        Marshal.FreeHGlobal(ptr);
                        freed = true;
                    },
                    schemaByte));

            Assert.True(freed);
            Assert.Contains("empty privacy result payload", error.Message);
            pointer = IntPtr.Zero;
        }
        finally
        {
            if (pointer != IntPtr.Zero)
            {
                Marshal.FreeHGlobal(pointer);
            }
        }
    }

    [Fact]
    public void PrivacyNativeReadOutputRejectsOversizedSuccessArchiveAndFreesPointer()
    {
        var freed = false;
        var pointer = new IntPtr(1);
        var oversizedLength = (UIntPtr)((ulong)PrivacyNative.PrivacyNativeArchiveMaxBytes + 1UL);

        var error = Assert.Throws<InvalidOperationException>(() =>
            PrivacyNative.ReadPrivacyOutput(
                "iroha_privacy_capabilities_v1",
                0,
                pointer,
                oversizedLength,
                ptr =>
                {
                    Assert.Equal(pointer, ptr);
                    freed = true;
                },
                PrivacyNative.PrivacyCapabilitiesResultSchemaByte));

        Assert.True(freed);
        Assert.Contains("oversized output", error.Message);
    }

    [Fact]
    public void PrivacyNativeReadOutputCopiesArchiveAndFreesPointer()
    {
        var bytes = PrivacyNoritoFrameWithPayload(0x50);
        var pointer = Marshal.AllocHGlobal(bytes.Length);
        var freed = false;
        try
        {
            Marshal.Copy(bytes, 0, pointer, bytes.Length);

            var archive = PrivacyNative.ReadPrivacyOutput(
                "iroha_privacy_capabilities_v1",
                0,
                pointer,
                (UIntPtr)bytes.Length,
                ptr =>
                {
                    Assert.Equal(pointer, ptr);
                    AssertPointerZeroed(ptr, bytes.Length);
                    Marshal.FreeHGlobal(ptr);
                    freed = true;
                },
                PrivacyNative.PrivacyCapabilitiesResultSchemaByte);

            Assert.Equal(bytes, archive);
            Assert.True(freed);
            pointer = IntPtr.Zero;
        }
        finally
        {
            if (pointer != IntPtr.Zero)
            {
                Marshal.FreeHGlobal(pointer);
            }
        }
    }

    [Fact]
    public void PrivacyNativeReadOutputCopiesArchiveBeforeFreeCallbackCanMutateBuffer()
    {
        var bytes = PrivacyNoritoFrameWithPayload(0x50);
        var pointer = Marshal.AllocHGlobal(bytes.Length);
        var freed = false;
        try
        {
            Marshal.Copy(bytes, 0, pointer, bytes.Length);

            var archive = PrivacyNative.ReadPrivacyOutput(
                "iroha_privacy_capabilities_v1",
                0,
                pointer,
                (UIntPtr)bytes.Length,
                ptr =>
                {
                    Assert.Equal(pointer, ptr);
                    AssertPointerZeroed(ptr, bytes.Length);
                    Marshal.Copy(FilledBytes(0x7f, bytes.Length), 0, ptr, bytes.Length);
                    Marshal.FreeHGlobal(ptr);
                    freed = true;
                },
                PrivacyNative.PrivacyCapabilitiesResultSchemaByte);

            Assert.Equal(bytes, archive);
            Assert.True(freed);
            pointer = IntPtr.Zero;
        }
        finally
        {
            if (pointer != IntPtr.Zero)
            {
                Marshal.FreeHGlobal(pointer);
            }
        }
    }

    [Fact]
    public void PrivacyNativeReadOutputRejectsInvalidNoritoArchiveAndFreesPointer()
    {
        foreach (var bytes in InvalidPrivacyNativeOutputArchives())
        {
            var pointer = Marshal.AllocHGlobal(bytes.Length);
            var freed = false;
            try
            {
                Marshal.Copy(bytes, 0, pointer, bytes.Length);

                var error = Assert.Throws<InvalidOperationException>(() =>
                    PrivacyNative.ReadPrivacyOutput(
                        "iroha_privacy_capabilities_v1",
                        0,
                        pointer,
                        (UIntPtr)bytes.Length,
                        ptr =>
                        {
                            Assert.Equal(pointer, ptr);
                            AssertPointerZeroed(ptr, bytes.Length);
                            Marshal.FreeHGlobal(ptr);
                            pointer = IntPtr.Zero;
                            freed = true;
                        },
                        PrivacyNative.PrivacyCapabilitiesResultSchemaByte));

                Assert.True(freed);
                Assert.Contains("invalid Norito V1 archive", error.Message);
            }
            finally
            {
                if (pointer != IntPtr.Zero)
                {
                    Marshal.FreeHGlobal(pointer);
                }
            }
        }
    }

    [Fact]
    public void PrivacyNativeReadOutputRejectsWrongOperationSchemaAndFreesPointer()
    {
        AssertReadOutputRejectsWrongSchema(
            "iroha_privacy_capabilities_v1",
            PrivacyNative.PrivacyCapabilitiesResultSchemaByte,
            PrivacyNoritoFrameWithSchemaOverride(0x50, 21, 0x42));
        AssertReadOutputRejectsWrongSchema(
            "iroha_privacy_build_proof_v1",
            PrivacyNative.PrivacyBuildProofResultSchemaByte,
            PrivacyNoritoFrameWithSchemaOverride(0x42, 6, 0x56));
        AssertReadOutputRejectsWrongSchema(
            "iroha_privacy_verify_proof_v1",
            PrivacyNative.PrivacyVerifyProofResultSchemaByte,
            PrivacyNoritoFrameWithSchemaOverride(0x56, 21, 0x42));
    }

    [Fact]
    public void PrivacyNativeReadOutputRequiresExplicitExpectedSchemasAndFreesPointer()
    {
        var bytes = PrivacyNoritoFrameWithPayload(0x50);
        var pointer = Marshal.AllocHGlobal(bytes.Length);
        var freed = false;
        try
        {
            Marshal.Copy(bytes, 0, pointer, bytes.Length);

            var error = Assert.Throws<InvalidOperationException>(() =>
                PrivacyNative.ReadPrivacyOutput(
                    "iroha_privacy_capabilities_v1",
                    0,
                    pointer,
                    (UIntPtr)bytes.Length,
                    ptr =>
                    {
                        Assert.Equal(pointer, ptr);
                        Marshal.FreeHGlobal(ptr);
                        pointer = IntPtr.Zero;
                        freed = true;
                    }));

            Assert.True(freed);
            Assert.Contains("requires explicit privacy result schemas", error.Message);
        }
        finally
        {
            if (pointer != IntPtr.Zero)
            {
                Marshal.FreeHGlobal(pointer);
            }
        }
    }

    [Fact]
    public void PrivacyNativeReadOutputRejectsMismatchedExpectedSchemaSetAndFreesPointer()
    {
        var bytes = PrivacyNoritoFrameWithPayload(0x56);
        var pointer = Marshal.AllocHGlobal(bytes.Length);
        var freed = false;
        try
        {
            Marshal.Copy(bytes, 0, pointer, bytes.Length);

            var error = Assert.Throws<InvalidOperationException>(() =>
                PrivacyNative.ReadPrivacyOutput(
                    "iroha_privacy_verify_proof_v1",
                    0,
                    pointer,
                    (UIntPtr)bytes.Length,
                    ptr =>
                    {
                        Assert.Equal(pointer, ptr);
                        Marshal.FreeHGlobal(ptr);
                        pointer = IntPtr.Zero;
                        freed = true;
                    },
                    PrivacyNative.PrivacyBuildProofResultSchemaByte));

            Assert.True(freed);
            Assert.Contains(
                "expected privacy result schemas do not match",
                error.Message);
        }
        finally
        {
            if (pointer != IntPtr.Zero)
            {
                Marshal.FreeHGlobal(pointer);
            }
        }
    }

    [Fact]
    public void PrivacyNativeRejectsUnknownOperationSchemaBeforeNativeDispatch()
    {
        AssertReadOutputRejectsUnknownSymbol(
            "iroha_privacy_forged_operation_v1",
            PrivacyNoritoFrameWithPayload(0x50),
            PrivacyNative.PrivacyCapabilitiesResultSchemaByte);

        var invoked = false;
        var error = Assert.Throws<InvalidOperationException>(() =>
            PrivacyNative.CallProof(
                PrivacyNoritoFrameWithPayload(0x52),
                "iroha_privacy_forged_operation_v1",
                (byte[] request, UIntPtr requestLen, out IntPtr outPtr, out UIntPtr outLen) =>
                {
                    invoked = true;
                    outPtr = IntPtr.Zero;
                    outLen = UIntPtr.Zero;
                    return 0;
                },
                requireAbi: false,
                free: _ => { }));

        Assert.False(invoked);
        Assert.Contains("not a supported privacy native operation", error.Message);
    }

    [Fact]
    public void PrivacyNativeSanitizesNativeExceptionsBeforeExposingRequestBytes()
    {
        var witness = "csharp-sdk-private-witness-never-echo-64dc";
        var requestArchive = PrivacyNoritoFrameWithPayload(0x52);
        byte[]? buildRequest = null;
        byte[]? verifyRequest = null;

        var capabilitiesError = Assert.Throws<InvalidOperationException>(() =>
            PrivacyNative.CallCapabilities(
                "iroha_privacy_capabilities_v1",
                (out IntPtr outPtr, out UIntPtr outLen) =>
                {
                    outPtr = IntPtr.Zero;
                    outLen = UIntPtr.Zero;
                    throw new ApplicationException($"native panic included {witness}");
                },
                requireAbi: false));
        AssertSanitizedNativeFailure(
            capabilitiesError,
            "iroha_privacy_capabilities_v1 failed.",
            witness);

        var buildError = Assert.Throws<InvalidOperationException>(() =>
            PrivacyNative.CallProof(
                requestArchive,
                "iroha_privacy_build_proof_v1",
                (byte[] requestPtr, UIntPtr requestLen, out IntPtr outPtr, out UIntPtr outLen) =>
                {
                    buildRequest = requestPtr;
                    Assert.NotSame(requestArchive, requestPtr);
                    Assert.Equal(requestArchive, requestPtr);
                    outPtr = IntPtr.Zero;
                    outLen = UIntPtr.Zero;
                    throw new ApplicationException($"native panic included {witness}");
                },
                requireAbi: false));
        AssertSanitizedNativeFailure(
            buildError,
            "iroha_privacy_build_proof_v1 failed.",
            witness);

        var verifyError = Assert.Throws<InvalidOperationException>(() =>
            PrivacyNative.CallProof(
                requestArchive,
                "iroha_privacy_verify_proof_v1",
                (byte[] requestPtr, UIntPtr requestLen, out IntPtr outPtr, out UIntPtr outLen) =>
                {
                    verifyRequest = requestPtr;
                    Assert.NotSame(requestArchive, requestPtr);
                    Assert.Equal(requestArchive, requestPtr);
                    outPtr = IntPtr.Zero;
                    outLen = UIntPtr.Zero;
                    throw new BadImageFormatException($"native panic included {witness}");
                },
                requireAbi: false));
        AssertSanitizedNativeFailure(
            verifyError,
            "iroha_privacy_verify_proof_v1 failed.",
            witness);
        Assert.NotNull(buildRequest);
        Assert.NotNull(verifyRequest);
        Assert.True(Array.TrueForAll(buildRequest!, value => value == 0));
        Assert.True(Array.TrueForAll(verifyRequest!, value => value == 0));
        Assert.Equal(PrivacyNoritoFrameWithPayload(0x52), requestArchive);
    }

    [Fact]
    public void PrivacyNativeClearsTemporaryProofRequestCopyAfterNativeDispatch()
    {
        var requestArchive = PrivacyNoritoFrameWithPayload(0x52);
        byte[]? capturedRequest = null;

        var error = Assert.Throws<InvalidOperationException>(() =>
            PrivacyNative.CallProof(
                requestArchive,
                "iroha_privacy_build_proof_v1",
                (byte[] requestPtr, UIntPtr requestLen, out IntPtr outPtr, out UIntPtr outLen) =>
                {
                    capturedRequest = requestPtr;
                    Assert.Equal(requestArchive, requestPtr);
                    outPtr = IntPtr.Zero;
                    outLen = UIntPtr.Zero;
                    return -311;
                },
                requireAbi: false));

        Assert.Contains("-311", error.Message);
        Assert.NotNull(capturedRequest);
        Assert.True(Array.TrueForAll(capturedRequest!, value => value == 0));
        Assert.Equal(PrivacyNoritoFrameWithPayload(0x52), requestArchive);
    }

    [Fact]
    public void PrivacyNativeHostileRequestMutationCannotMutateCallerArchive()
    {
        var requestArchive = PrivacyNoritoFrameWithPayload(0x52);
        var originalArchive = requestArchive.ToArray();
        byte[]? buildRequest = null;
        byte[]? verifyRequest = null;

        var buildArchive = PrivacyNative.CallProof(
            requestArchive,
            "iroha_privacy_build_proof_v1",
            (byte[] requestPtr, UIntPtr requestLen, out IntPtr outPtr, out UIntPtr outLen) =>
            {
                buildRequest = requestPtr;
                requestPtr[0] = 0x00;
                requestPtr[6] = 0x7f;
                var output = PrivacyNoritoFrameWithPayload(0x42);
                outPtr = Marshal.AllocHGlobal(output.Length);
                Marshal.Copy(output, 0, outPtr, output.Length);
                outLen = (UIntPtr)output.Length;
                return 0;
            },
            requireAbi: false,
            free: Marshal.FreeHGlobal);
        Assert.Equal(PrivacyNoritoFrameWithPayload(0x42), buildArchive);

        var verifyArchive = PrivacyNative.CallProof(
            requestArchive,
            "iroha_privacy_verify_proof_v1",
            (byte[] requestPtr, UIntPtr requestLen, out IntPtr outPtr, out UIntPtr outLen) =>
            {
                verifyRequest = requestPtr;
                requestPtr[0] = 0x00;
                requestPtr[6] = 0x7f;
                var output = PrivacyNoritoFrameWithPayload(0x56);
                outPtr = Marshal.AllocHGlobal(output.Length);
                Marshal.Copy(output, 0, outPtr, output.Length);
                outLen = (UIntPtr)output.Length;
                return 0;
            },
            requireAbi: false,
            free: Marshal.FreeHGlobal);
        Assert.Equal(PrivacyNoritoFrameWithPayload(0x56), verifyArchive);

        Assert.Equal(originalArchive, requestArchive);
        Assert.NotNull(buildRequest);
        Assert.NotNull(verifyRequest);
        Assert.True(Array.TrueForAll(buildRequest!, value => value == 0));
        Assert.True(Array.TrueForAll(verifyRequest!, value => value == 0));
    }

    [Fact]
    public void PrivacyNativeRejectsMalformedProofRequestsBeforeLoadingNativeBridge()
    {
        var emptyPayloadRequest = PrivacyNoritoFrame(0x52);
        var emptyBuildPayloadError = Assert.Throws<ArgumentException>(() =>
            PrivacyNative.BuildProofV1(emptyPayloadRequest));
        var emptyVerifyPayloadError = Assert.Throws<ArgumentException>(() =>
            PrivacyNative.VerifyProofV1(emptyPayloadRequest));

        Assert.Contains(
            "non-empty privacy request payload",
            emptyBuildPayloadError.Message);
        Assert.Contains(
            "non-empty privacy request payload",
            emptyVerifyPayloadError.Message);

        foreach (var malformed in InvalidPrivacyRequestArchives())
        {
            var buildError = Assert.Throws<ArgumentException>(() =>
                PrivacyNative.BuildProofV1(malformed));
            var verifyError = Assert.Throws<ArgumentException>(() =>
                PrivacyNative.VerifyProofV1(malformed));

            Assert.Contains("valid Norito V1 archive", buildError.Message);
            Assert.Contains("valid Norito V1 archive", verifyError.Message);
        }
    }

    private static void AssertFailClosedProductionGate(PrivacyCapabilities capabilities)
    {
        Assert.False(capabilities.ProductionReady);
        Assert.Equal(PrivacyNative.ProductionGateVersion, capabilities.ProductionGate.Version);
        Assert.False(capabilities.ProductionGate.Ready);
        Assert.False(capabilities.ProductionGate.RealProving);
        Assert.False(capabilities.ProductionGate.RealVerification);
        Assert.False(capabilities.ProductionGate.ChainAdmission);
        Assert.False(capabilities.ProductionGate.SdkParity);
        Assert.False(capabilities.ProductionGate.WalletState);
        Assert.False(capabilities.ProductionGate.DeterministicTests);
        Assert.False(capabilities.ProductionGate.Fuzzing);
        Assert.False(capabilities.ProductionGate.PerformanceGates);
        Assert.False(capabilities.ProductionGate.ExternalAudit);
        Assert.Empty(capabilities.ProductionGate.AuditReferences);
        Assert.Equal(PrivacyProductionGate.MissingReasons, capabilities.ProductionGate.Missing);
        Assert.Contains(
            "real proving engine is not registered",
            capabilities.ProductionGate.Missing);
        Assert.Contains(
            "chain admission path is not enabled",
            capabilities.ProductionGate.Missing);
        Assert.Contains(
            "external audit signoff is missing",
            capabilities.ProductionGate.Missing);
        Assert.Contains(
            "implementation stage is not production-hardened",
            capabilities.ProductionGate.Missing);
        Assert.Contains(
            "planned SDK entrypoints remain",
            capabilities.ProductionGate.Missing);
        Assert.Contains(
            "dev fixture entrypoints are not production entrypoints",
            capabilities.ProductionGate.Missing);
        Assert.Contains(
            "Iroha production allowlist is not enabled for this audited row",
            capabilities.ProductionGate.Missing);
    }

    private static void AssertSanitizedNativeFailure(
        InvalidOperationException error,
        string message,
        string witness)
    {
        Assert.Equal(message, error.Message);
        Assert.Null(error.InnerException);
        Assert.DoesNotContain(witness, error.Message);
        Assert.DoesNotContain(witness, error.ToString());
    }

    private static bool IsValidProbeOutput(int code, byte[] output, params byte[] expectedSchemaBytes)
    {
        var pointer = Marshal.AllocHGlobal(output.Length);
        try
        {
            Marshal.Copy(output, 0, pointer, output.Length);
            var valid = PrivacyNative.IsValidProbeResult(
                code,
                pointer,
                (UIntPtr)output.Length,
                expectedSchemaBytes);
            AssertPointerZeroed(pointer, output.Length);
            return valid;
        }
        finally
        {
            Marshal.FreeHGlobal(pointer);
        }
    }

    private static void AssertReadOutputRejectsWrongSchema(
        string symbol,
        byte expectedSchemaByte,
        byte[] output)
    {
        var pointer = Marshal.AllocHGlobal(output.Length);
        var freed = false;
        try
        {
            Marshal.Copy(output, 0, pointer, output.Length);

            var error = Assert.Throws<InvalidOperationException>(() =>
                PrivacyNative.ReadPrivacyOutput(
                    symbol,
                    0,
                    pointer,
                    (UIntPtr)output.Length,
                    ptr =>
                    {
                        Assert.Equal(pointer, ptr);
                        Marshal.FreeHGlobal(ptr);
                        pointer = IntPtr.Zero;
                        freed = true;
                    },
                    expectedSchemaByte));

            Assert.True(freed);
            Assert.Contains("unexpected privacy result schema", error.Message);
        }
        finally
        {
            if (pointer != IntPtr.Zero)
            {
                Marshal.FreeHGlobal(pointer);
            }
        }
    }

    private static void AssertPointerZeroed(IntPtr pointer, int length)
    {
        var observed = new byte[length];
        Marshal.Copy(pointer, observed, 0, observed.Length);
        Assert.All(observed, value => Assert.Equal((byte)0, value));
    }

    private static void AssertReadOutputRejectsUnknownSymbol(
        string symbol,
        byte[] output,
        byte expectedSchemaByte)
    {
        var pointer = Marshal.AllocHGlobal(output.Length);
        var freed = false;
        try
        {
            Marshal.Copy(output, 0, pointer, output.Length);

            var error = Assert.Throws<InvalidOperationException>(() =>
                PrivacyNative.ReadPrivacyOutput(
                    symbol,
                    0,
                    pointer,
                    (UIntPtr)output.Length,
                    ptr =>
                    {
                        Assert.Equal(pointer, ptr);
                        Marshal.FreeHGlobal(ptr);
                        pointer = IntPtr.Zero;
                        freed = true;
                    },
                    expectedSchemaByte));

            Assert.True(freed);
            Assert.Contains("not a supported privacy native operation", error.Message);
        }
        finally
        {
            if (pointer != IntPtr.Zero)
            {
                Marshal.FreeHGlobal(pointer);
            }
        }
    }

    private static byte[] PrivacyNoritoFrame(byte schemaByte)
    {
        var frame = new byte[40];
        frame[0] = (byte)'N';
        frame[1] = (byte)'R';
        frame[2] = (byte)'T';
        frame[3] = (byte)'0';
        Array.Fill(frame, schemaByte, 6, 16);
        return frame;
    }

    private static byte[] PrivacyNoritoFrameWithPayload(byte schemaByte)
    {
        var frame = new byte[45];
        PrivacyNoritoFrame(schemaByte).CopyTo(frame, 0);
        frame[23] = 3;
        new byte[]
        {
            0xb9,
            0xd3,
            0xa8,
            0x0c,
            0xcd,
            0x5d,
            0x13,
            0x24,
        }.CopyTo(frame, 31);
        frame[42] = 0xa5;
        frame[43] = 0x5a;
        frame[44] = 0x11;
        return frame;
    }

    private static byte[] PrivacyNoritoFrameWithPadding(byte schemaByte, int paddingLength)
    {
        var frame = new byte[43 + paddingLength];
        PrivacyNoritoFrame(schemaByte).CopyTo(frame, 0);
        frame[23] = 3;
        new byte[]
        {
            0xb9,
            0xd3,
            0xa8,
            0x0c,
            0xcd,
            0x5d,
            0x13,
            0x24,
        }.CopyTo(frame, 31);
        frame[40 + paddingLength] = 0xa5;
        frame[41 + paddingLength] = 0x5a;
        frame[42 + paddingLength] = 0x11;
        return frame;
    }

    private static byte[] PrivacyNoritoFrameWithSchemaOverride(
        byte schemaByte,
        int offset,
        byte value)
    {
        var frame = PrivacyNoritoFrameWithPayload(schemaByte);
        frame[offset] = value;
        return frame;
    }

    private static byte[] PrivacyNoritoFrameWithDeclaredPayloadLength(
        byte schemaByte,
        ulong payloadLength)
    {
        var frame = PrivacyNoritoFrameWithPayload(schemaByte);
        for (var index = 0; index < 8; index++)
        {
            frame[23 + index] = (byte)((payloadLength >> (8 * index)) & 0xffUL);
        }
        return frame;
    }

    private static byte[] PrivacyNoritoFrameWithFlags(byte schemaByte, byte flags)
    {
        var frame = PrivacyNoritoFrameWithPayload(schemaByte);
        frame[39] = flags;
        return frame;
    }

    private static byte[] InvalidPrivacyNoritoFrame(int offset, byte value)
    {
        var frame = PrivacyNoritoFrame(0x50);
        frame[offset] = value;
        return frame;
    }

    private static byte[] InvalidPrivacyNoritoDeclaredPayloadLength(byte schemaByte)
    {
        return PrivacyNoritoFrameWithDeclaredPayloadLength(schemaByte, 6);
    }

    private static byte[] InvalidPrivacyNoritoOversizedPayloadLength(byte schemaByte)
    {
        return PrivacyNoritoFrameWithDeclaredPayloadLength(schemaByte, 0x8000_0000_0000_0000UL);
    }

    private static byte[] InvalidPrivacyNoritoWithNonzeroPadding()
    {
        var frame = new byte[41];
        PrivacyNoritoFrame(0x50).CopyTo(frame, 0);
        frame[40] = 1;
        return frame;
    }

    private static byte[] InvalidPrivacyNoritoWithExcessivePadding()
    {
        return PrivacyNoritoFrameWithPadding(0x50, 65);
    }

    private static byte[] InvalidPrivacyNoritoPayloadTamper()
    {
        var frame = PrivacyNoritoFrameWithPayload(0x50);
        frame[44] ^= 0x7f;
        return frame;
    }

    private static IEnumerable<byte[]> InvalidPrivacyNativeOutputArchives()
    {
        yield return new byte[] { 0x50, 0x01, 0x02 };
        yield return InvalidPrivacyNoritoFrame(0, (byte)'X');
        yield return InvalidPrivacyNoritoFrame(4, 1);
        yield return InvalidPrivacyNoritoFrame(5, 1);
        yield return InvalidPrivacyNoritoFrame(22, 1);
        yield return InvalidPrivacyNoritoDeclaredPayloadLength(0x50);
        yield return InvalidPrivacyNoritoOversizedPayloadLength(0x50);
        yield return InvalidPrivacyNoritoFrame(39, 0x40);
        yield return InvalidPrivacyNoritoFrame(39, 0x20);
        yield return InvalidPrivacyNoritoWithNonzeroPadding();
        yield return InvalidPrivacyNoritoWithExcessivePadding();
        yield return InvalidPrivacyNoritoFrame(31, 1);
        yield return InvalidPrivacyNoritoPayloadTamper();
    }

    private static IEnumerable<byte[]> InvalidPrivacyRequestArchives()
    {
        yield return new byte[] { 0x01 };
        yield return InvalidPrivacyNoritoFrame(0, (byte)'X');
        yield return InvalidPrivacyNoritoFrame(4, 1);
        yield return InvalidPrivacyNoritoFrame(5, 1);
        yield return InvalidPrivacyNoritoFrame(22, 1);
        yield return InvalidPrivacyNoritoDeclaredPayloadLength(0x52);
        yield return InvalidPrivacyNoritoOversizedPayloadLength(0x52);
        yield return InvalidPrivacyNoritoFrame(39, 0x40);
        yield return InvalidPrivacyNoritoFrame(39, 0x20);
        yield return InvalidPrivacyNoritoWithNonzeroPadding();
        yield return InvalidPrivacyNoritoWithExcessivePadding();
        yield return InvalidPrivacyNoritoFrame(31, 1);
        yield return InvalidPrivacyNoritoPayloadTamper();
    }

    private static IEnumerable<byte[]> WrongSchemaPrivacyRequestArchives()
    {
        yield return PrivacyNoritoFrameWithPayload(0x50);
        yield return PrivacyNoritoFrameWithPayload(0x42);
        yield return PrivacyNoritoFrameWithPayload(0x56);
        yield return PrivacyNoritoFrameWithSchemaOverride(0x52, 6, 0x42);
        yield return PrivacyNoritoFrameWithSchemaOverride(0x52, 21, 0x56);
    }

    private static byte[] FilledBytes(byte value, int length)
    {
        var bytes = new byte[length];
        Array.Fill(bytes, value);
        return bytes;
    }
}
