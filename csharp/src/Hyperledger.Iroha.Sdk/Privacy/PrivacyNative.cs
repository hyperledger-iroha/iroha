using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Globalization;
using System.Runtime.ExceptionServices;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Threading;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Http;

namespace Hyperledger.Iroha.Privacy;

/// <summary>Closed first-release privacy protocol identity in canonical Norito order.</summary>
public enum PrivacyProtocolIdV1 : uint
{
    ZkAcePqAuthorizationV0 = 0,
    AnonymousPgcKOutOfNV1 = 1,
    VeRangeTransparentRangeV1 = 2,
    IrohaZkAmsV1 = 3,
    VegaExistingCredentialZkV0 = 4,
    IrohaZkX509StarkP256V0 = 5,
    IrohaJindoPolynomialCommitmentV0 = 6,
    IrohaBootleLanternAnoncredV1 = 7,
    OrchardHalo2ActionsV1 = 8,
    MoneroFcmpPlusPlusV1 = 9,
    IrohaIvmPrivateNoteStarkV1 = 10,
    PqMaspStarkV0 = 11,
}

/// <summary>Stable ABI-22 result of validating one typed local compiled-profile catalog.</summary>
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

/// <summary>Stable ABI-22 result of validating the Rust-derived exact-12 fixture bundle.</summary>
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
    public static PrivacyProtocolIdV1 ParseCanonicalLabel(string label)
    {
        ArgumentNullException.ThrowIfNull(label);
        return label switch
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

    /// <summary>
    /// Return the exact first-release Norito statement/proof variant label for one protocol.
    /// </summary>
    public static string CanonicalTypedVariantLabel(this PrivacyProtocolIdV1 protocol) =>
        protocol switch
        {
            PrivacyProtocolIdV1.ZkAcePqAuthorizationV0 => "ZkAcePqAuthorizationV0",
            PrivacyProtocolIdV1.AnonymousPgcKOutOfNV1 => "AnonymousPgcKOutOfNV1",
            PrivacyProtocolIdV1.VeRangeTransparentRangeV1 => "VeRangeTransparentRangeV1",
            PrivacyProtocolIdV1.IrohaZkAmsV1 => "IrohaZkAmsV1",
            PrivacyProtocolIdV1.VegaExistingCredentialZkV0 => "VegaExistingCredentialZkV0",
            PrivacyProtocolIdV1.IrohaZkX509StarkP256V0 => "IrohaZkX509StarkP256V0",
            PrivacyProtocolIdV1.IrohaJindoPolynomialCommitmentV0 =>
                "IrohaJindoPolynomialCommitmentV0",
            PrivacyProtocolIdV1.IrohaBootleLanternAnoncredV1 =>
                "IrohaBootleLanternAnoncredV1",
            PrivacyProtocolIdV1.OrchardHalo2ActionsV1 => "OrchardHalo2ActionsV1",
            PrivacyProtocolIdV1.MoneroFcmpPlusPlusV1 => "MoneroFcmpPlusPlusV1",
            PrivacyProtocolIdV1.IrohaIvmPrivateNoteStarkV1 =>
                "IrohaIvmPrivateNoteStarkV1",
            PrivacyProtocolIdV1.PqMaspStarkV0 => "PqMaspStarkV0",
            _ => throw new ArgumentOutOfRangeException(nameof(protocol)),
        };

    /// <summary>
    /// Parse one exact first-release Norito statement/proof variant label. Legacy row names,
    /// aliases, whitespace, and case changes are rejected.
    /// </summary>
    public static PrivacyProtocolIdV1 ParseCanonicalTypedVariantLabel(string label)
    {
        ArgumentNullException.ThrowIfNull(label);
        return label switch
        {
            "ZkAcePqAuthorizationV0" => PrivacyProtocolIdV1.ZkAcePqAuthorizationV0,
            "AnonymousPgcKOutOfNV1" => PrivacyProtocolIdV1.AnonymousPgcKOutOfNV1,
            "VeRangeTransparentRangeV1" => PrivacyProtocolIdV1.VeRangeTransparentRangeV1,
            "IrohaZkAmsV1" => PrivacyProtocolIdV1.IrohaZkAmsV1,
            "VegaExistingCredentialZkV0" => PrivacyProtocolIdV1.VegaExistingCredentialZkV0,
            "IrohaZkX509StarkP256V0" => PrivacyProtocolIdV1.IrohaZkX509StarkP256V0,
            "IrohaJindoPolynomialCommitmentV0" =>
                PrivacyProtocolIdV1.IrohaJindoPolynomialCommitmentV0,
            "IrohaBootleLanternAnoncredV1" =>
                PrivacyProtocolIdV1.IrohaBootleLanternAnoncredV1,
            "OrchardHalo2ActionsV1" => PrivacyProtocolIdV1.OrchardHalo2ActionsV1,
            "MoneroFcmpPlusPlusV1" => PrivacyProtocolIdV1.MoneroFcmpPlusPlusV1,
            "IrohaIvmPrivateNoteStarkV1" => PrivacyProtocolIdV1.IrohaIvmPrivateNoteStarkV1,
            "PqMaspStarkV0" => PrivacyProtocolIdV1.PqMaspStarkV0,
            _ => throw new ArgumentException(
                "Unknown canonical privacy statement/proof variant.",
                nameof(label)),
        };
    }
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

internal sealed class PrivacySignedExact12ActionInspectionV1
{
    private readonly byte[] transactionHash;
    private readonly byte[] transactionIntentDigest;
    private readonly byte[] statementDigest;
    private readonly byte[] proofEnvelopeHash;

    internal PrivacySignedExact12ActionInspectionV1(byte[] projection)
    {
        ArgumentNullException.ThrowIfNull(projection);
        if (projection.Length != PrivacyNative.PrivacySignedExact12ActionProjectionBytes)
        {
            throw new InvalidDataException(
                "Native Exact12 signed-action projection must contain exactly 128 bytes.");
        }

        transactionHash = projection[0..32];
        transactionIntentDigest = projection[32..64];
        statementDigest = projection[64..96];
        proofEnvelopeHash = projection[96..128];
        PrivacyExact12ActionRequestV1.RequireNonzeroFixed32(
            transactionHash,
            nameof(transactionHash));
        PrivacyExact12ActionRequestV1.RequireNonzeroFixed32(
            transactionIntentDigest,
            nameof(transactionIntentDigest));
        PrivacyExact12ActionRequestV1.RequireNonzeroFixed32(
            statementDigest,
            nameof(statementDigest));
        PrivacyExact12ActionRequestV1.RequireNonzeroFixed32(
            proofEnvelopeHash,
            nameof(proofEnvelopeHash));
    }

    internal byte[] TransactionHash => (byte[])transactionHash.Clone();

    internal byte[] TransactionIntentDigest => (byte[])transactionIntentDigest.Clone();

    internal byte[] StatementDigest => (byte[])statementDigest.Clone();

    internal byte[] ProofEnvelopeHash => (byte[])proofEnvelopeHash.Clone();
}

internal sealed class PrivacyAuthenticatedTransactionDetailsQueryV1
{
    private readonly byte[] preparation;
    private readonly byte[] signedQuery;

    internal PrivacyAuthenticatedTransactionDetailsQueryV1(
        byte[] preparation,
        byte[] signedQuery,
        string transactionHashHex,
        string transactionAuthority)
    {
        ArgumentNullException.ThrowIfNull(preparation);
        ArgumentNullException.ThrowIfNull(signedQuery);
        this.preparation = (byte[])preparation.Clone();
        this.signedQuery = (byte[])signedQuery.Clone();
        TransactionHashHex = transactionHashHex;
        TransactionAuthority = transactionAuthority;
    }

    internal byte[] Preparation => (byte[])preparation.Clone();

    internal byte[] SignedQuery => (byte[])signedQuery.Clone();

    internal string TransactionHashHex { get; }

    internal string TransactionAuthority { get; }
}

/// <summary>
/// Authenticated Torii committed-result projection. It carries no signed block/header or QC;
/// independent finality requires separate verification of the projected exact block.
/// </summary>
internal sealed record class PrivacyAuthenticatedCommittedResultV1(
    string TransactionHashHex,
    string TransactionAuthority,
    string BlockHashHex,
    string ResultHashHex,
    bool ResultOk,
    string? RejectionMessage,
    ulong CommittedBlockHeight);

internal sealed class PrivacyAuthenticatedActionReceiptQueryV1
{
    private readonly byte[] preparation;
    private readonly byte[] signedQuery;
    private readonly byte[] transactionIntentDigest;
    private readonly byte[] statementDigest;
    private readonly byte[] proofEnvelopeHash;

    internal PrivacyAuthenticatedActionReceiptQueryV1(
        byte[] preparation,
        byte[] signedQuery,
        string networkIdHex,
        PrivacyProtocolIdV1 protocolId,
        PrivacyOperationSchemaV1 operationSchema,
        PrivacyLedgerEffectKindV1 ledgerEffectKind,
        string transactionHashHex,
        uint actionIndex,
        byte[] transactionIntentDigest,
        byte[] statementDigest,
        byte[] proofEnvelopeHash)
    {
        ArgumentNullException.ThrowIfNull(preparation);
        ArgumentNullException.ThrowIfNull(signedQuery);
        ArgumentNullException.ThrowIfNull(networkIdHex);
        ArgumentNullException.ThrowIfNull(transactionHashHex);
        if (actionIndex != 0)
        {
            throw new ArgumentOutOfRangeException(
                nameof(actionIndex),
                "Exact12 V1 action receipts require action index zero.");
        }
        if (protocolId != operationSchema.ProtocolId()
            || ledgerEffectKind != operationSchema.LedgerEffectKind())
        {
            throw new ArgumentException(
                "Authenticated action-receipt query has contradictory typed bindings.",
                nameof(operationSchema));
        }
        PrivacyExact12ActionRequestV1.RequireNonzeroFixed32(
            transactionIntentDigest,
            nameof(transactionIntentDigest));
        PrivacyExact12ActionRequestV1.RequireNonzeroFixed32(
            statementDigest,
            nameof(statementDigest));
        PrivacyExact12ActionRequestV1.RequireNonzeroFixed32(
            proofEnvelopeHash,
            nameof(proofEnvelopeHash));
        this.preparation = (byte[])preparation.Clone();
        this.signedQuery = (byte[])signedQuery.Clone();
        NetworkIdHex = networkIdHex;
        ProtocolId = protocolId;
        OperationSchema = operationSchema;
        LedgerEffectKind = ledgerEffectKind;
        TransactionHashHex = transactionHashHex;
        ActionIndex = actionIndex;
        this.transactionIntentDigest = (byte[])transactionIntentDigest.Clone();
        this.statementDigest = (byte[])statementDigest.Clone();
        this.proofEnvelopeHash = (byte[])proofEnvelopeHash.Clone();
    }

    internal byte[] Preparation => (byte[])preparation.Clone();

    internal byte[] SignedQuery => (byte[])signedQuery.Clone();

    internal string NetworkIdHex { get; }

    internal PrivacyProtocolIdV1 ProtocolId { get; }

    internal PrivacyOperationSchemaV1 OperationSchema { get; }

    internal PrivacyLedgerEffectKindV1 LedgerEffectKind { get; }

    internal string TransactionHashHex { get; }

    internal uint ActionIndex { get; }

    internal byte[] TransactionIntentDigest => (byte[])transactionIntentDigest.Clone();

    internal byte[] StatementDigest => (byte[])statementDigest.Clone();

    internal byte[] ProofEnvelopeHash => (byte[])proofEnvelopeHash.Clone();
}

/// <summary>
/// Native-inspected ID105 receipt for one exact action. The receipt is authenticated by the
/// signed query and bound to a finalized ledger block, but the managed wrapper never treats its
/// capability snapshot as the earlier pre-submit admission snapshot.
/// </summary>
internal sealed class PrivacyAuthenticatedActionExecutionReceiptV1
{
    private readonly byte[] capabilityManifestDigest;
    private readonly byte[] finalizedBlockHash;

    internal PrivacyAuthenticatedActionExecutionReceiptV1(
        string networkIdHex,
        PrivacyProtocolIdV1 protocolId,
        PrivacyOperationSchemaV1 operationSchema,
        PrivacyLedgerEffectKindV1 ledgerEffectKind,
        string transactionHashHex,
        uint actionIndex,
        byte[] capabilityManifestDigest,
        ulong capabilityCommittedHeight,
        ulong admittedAtHeight,
        ulong finalizedHeight,
        byte[] finalizedBlockHash)
    {
        NetworkIdHex = networkIdHex;
        ProtocolId = protocolId;
        OperationSchema = operationSchema;
        LedgerEffectKind = ledgerEffectKind;
        TransactionHashHex = transactionHashHex;
        ActionIndex = actionIndex;
        this.capabilityManifestDigest = (byte[])capabilityManifestDigest.Clone();
        CapabilityCommittedHeight = capabilityCommittedHeight;
        AdmittedAtHeight = admittedAtHeight;
        FinalizedHeight = finalizedHeight;
        this.finalizedBlockHash = (byte[])finalizedBlockHash.Clone();
    }

    internal string NetworkIdHex { get; }

    internal PrivacyProtocolIdV1 ProtocolId { get; }

    internal PrivacyOperationSchemaV1 OperationSchema { get; }

    internal PrivacyLedgerEffectKindV1 LedgerEffectKind { get; }

    internal string TransactionHashHex { get; }

    internal uint ActionIndex { get; }

    internal byte[] CapabilityManifestDigest => (byte[])capabilityManifestDigest.Clone();

    internal ulong CapabilityCommittedHeight { get; }

    internal ulong AdmittedAtHeight { get; }

    internal ulong FinalizedHeight { get; }

    internal byte[] FinalizedBlockHash => (byte[])finalizedBlockHash.Clone();
}

internal sealed class PrivacyAuthenticatedStateQueryV1
{
    private readonly byte[] preparation;
    private readonly byte[] signedQuery;
    private readonly byte[] requestBinding;

    internal PrivacyAuthenticatedStateQueryV1(
        byte[] preparation,
        byte[] signedQuery,
        NetworkId networkId,
        uint queryId,
        uint protocolIndex,
        byte[] requestBinding)
    {
        ArgumentNullException.ThrowIfNull(preparation);
        ArgumentNullException.ThrowIfNull(signedQuery);
        ArgumentNullException.ThrowIfNull(networkId);
        ArgumentNullException.ThrowIfNull(requestBinding);
        PrivacyNative.RequirePrivacyStateQueryBindingV1(
            queryId,
            protocolIndex,
            requestBinding);
        this.preparation = (byte[])preparation.Clone();
        this.signedQuery = (byte[])signedQuery.Clone();
        NetworkId = networkId;
        QueryId = queryId;
        ProtocolIndex = protocolIndex;
        this.requestBinding = (byte[])requestBinding.Clone();
    }

    internal byte[] Preparation => (byte[])preparation.Clone();

    internal byte[] SignedQuery => (byte[])signedQuery.Clone();

    internal NetworkId NetworkId { get; }

    internal uint QueryId { get; }

    internal uint ProtocolIndex { get; }

    internal byte[] RequestBinding => (byte[])requestBinding.Clone();
}

/// <summary>
/// Selector-free local privacy build metadata and exact-12 fixture surface. The catalog never
/// establishes network activation or readiness; fetch a fresh authoritative capability snapshot
/// from live Torii before submitting a privacy proof.
/// </summary>
public static class PrivacyNative
{
    public const int PrivacyCompiledProfileCatalogArchiveMaxBytes = 256 * 1024;
    public const int PrivacyExact12FixtureBundleMaxBytes =
        PrivacyExact12FixtureCodecV1.MaxArchiveBytes;
    public const int PrivacySignedExact12ActionProjectionBytes = 128;
    public const uint RequiredBridgeAbiVersion = 22;
    internal const int PrivacyAuthenticatedTransactionDetailsPreparationMaxBytes = 64 * 1024;
    internal const int PrivacyAuthenticatedTransactionDetailsSignedQueryMaxBytes = 64 * 1024;
    internal const int PrivacyAuthenticatedTransactionDetailsResponseMaxBytes = 64 * 1024 * 1024;
    private const int PrivacyAuthenticatedTransactionDetailsProjectionMaxBytes = 8 * 1024;
    internal const int PrivacyAuthenticatedActionReceiptPreparationMaxBytes = 64 * 1024;
    internal const int PrivacyAuthenticatedActionReceiptSignedQueryMaxBytes = 64 * 1024;
    internal const int PrivacyAuthenticatedActionReceiptResponseMaxBytes = 256 * 1024;
    private const int PrivacyAuthenticatedActionReceiptProjectionMaxBytes = 32 * 1024;
    private const int PrivacyAuthenticatedActionReceiptBindingBytes = 96;
    private const uint PrivacyAuthenticatedActionReceiptIndexV1 = 0;
    internal const int PrivacyAuthenticatedStateQueryPreparationMaxBytes = 64 * 1024;
    internal const int PrivacyAuthenticatedStateQuerySignedQueryMaxBytes = 64 * 1024;
    internal const int PrivacyAuthenticatedStateQueryResponseMaxBytes = 256 * 1024;
    internal const int PrivacyAuthenticatedStateQueryProjectionMaxBytes = 256 * 1024;
    // Do not inherit the comparatively small worker stacks used by foreign managed runtimes.
    private const int NativeWorkerStackBytes = 16 * 1024 * 1024;
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
            var symbolsAvailable = NativeLibrary.TryLoad(
                    LibraryName,
                    typeof(PrivacyNative).Assembly,
                    null,
                    out handle)
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
                && NativeLibrary.TryGetExport(
                    handle,
                    "iroha_privacy_inspect_signed_exact12_action_v1",
                    out _)
                && NativeLibrary.TryGetExport(
                    handle,
                    "iroha_privacy_authenticated_transaction_details_prepare_v1",
                    out _)
                && NativeLibrary.TryGetExport(
                    handle,
                    "iroha_privacy_authenticated_transaction_details_finalize_v1",
                    out _)
                && NativeLibrary.TryGetExport(
                    handle,
                    "iroha_privacy_authenticated_transaction_details_project_result_v1",
                    out _)
                && NativeLibrary.TryGetExport(
                    handle,
                    "iroha_privacy_authenticated_action_receipt_prepare_v1",
                    out _)
                && NativeLibrary.TryGetExport(
                    handle,
                    "iroha_privacy_authenticated_action_receipt_finalize_v1",
                    out _)
                && NativeLibrary.TryGetExport(
                    handle,
                    "iroha_privacy_authenticated_action_receipt_project_result_v1",
                    out _)
                && NativeLibrary.TryGetExport(
                    handle,
                    "iroha_privacy_authenticated_state_query_prepare_v1",
                    out _)
                && NativeLibrary.TryGetExport(
                    handle,
                    "iroha_privacy_authenticated_state_query_finalize_v1",
                    out _)
                && NativeLibrary.TryGetExport(
                    handle,
                    "iroha_privacy_authenticated_state_query_project_result_v1",
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
        return RunWithNativeStack(() =>
            ProbeNativeArchiveOnWorker(query, validate, maximumBytes));
    }

    private static bool ProbeNativeArchiveOnWorker(
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

        return RunWithNativeStack(QueryExact12FixtureBundleOnWorker);
    }

    /// <summary>Returns this binary's canonical local compiled-profile catalog.</summary>
    public static PrivacyCompiledProfileCatalogArchive CompiledProfileCatalogV1()
    {
        if (!IsAvailable())
        {
            throw new InvalidOperationException(
                "Native privacy compiled-profile catalog is unavailable.");
        }

        return RunWithNativeStack(QueryCompiledProfileCatalogOnWorker);
    }

    private static PrivacyExact12FixtureBundleArchive QueryExact12FixtureBundleOnWorker()
    {
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

    private static PrivacyCompiledProfileCatalogArchive QueryCompiledProfileCatalogOnWorker()
    {
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
        var snapshot = (byte[])archive.Clone();
        var code = RunWithNativeStack(() => NativeValidateCompiledProfileCatalog(
            snapshot,
            new UIntPtr((uint)snapshot.Length)));
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
        var snapshot = (byte[])archive.Clone();
        var code = RunWithNativeStack(() => NativeValidateExact12FixtureBundle(
            snapshot,
            new UIntPtr((uint)snapshot.Length)));
        if (!Enum.IsDefined(typeof(PrivacyExact12FixtureValidationStatusV1), code))
        {
            throw new InvalidOperationException(
                "Native exact-12 privacy fixture validation returned an unknown status.");
        }
        return (PrivacyExact12FixtureValidationStatusV1)code;
    }

    /// <summary>
    /// Strictly validate canonical committed Exact12 manifest bytes and compare every complete
    /// compiled-profile result with this ABI-22 binary's native-validated local catalog.
    /// </summary>
    /// <remarks>
    /// A valid result is a structural and local-tuple prerequisite only. This method cannot mint
    /// network authority; only the authenticated Torii fetch can issue a manifest model usable by
    /// <see cref="PrivacyExact12CapabilityAdmissionV1.RequireExact12CapabilityTupleV1"/>.
    /// </remarks>
    public static PrivacyExact12CapabilityManifestValidationStatusV1
        ValidateExact12CapabilityManifestV1(byte[] archive)
    {
        ArgumentNullException.ThrowIfNull(archive);
        if (archive.Length == 0)
        {
            return PrivacyExact12CapabilityManifestValidationStatusV1.Empty;
        }
        if (archive.Length > PrivacyExact12CapabilityManifestV1.MaxArchiveBytes)
        {
            return PrivacyExact12CapabilityManifestValidationStatusV1.ArchiveTooLarge;
        }
        if (!IsAvailable())
        {
            return PrivacyExact12CapabilityManifestValidationStatusV1.NativeUnavailable;
        }

        try
        {
            var catalog = CompiledProfileCatalogV1().NoritoBytes;
            PrivacyExact12CapabilityManifestCodecV1.Validate(archive, catalog);
            return PrivacyExact12CapabilityManifestValidationStatusV1.Valid;
        }
        catch (PrivacyExact12CapabilityManifestCodecV1.LocalTupleMismatchException)
        {
            return PrivacyExact12CapabilityManifestValidationStatusV1.LocalCompiledTupleMismatch;
        }
        catch (PrivacyExact12CapabilityManifestException)
        {
            return PrivacyExact12CapabilityManifestValidationStatusV1.InvalidManifest;
        }
    }

    internal static PrivacySignedExact12ActionInspectionV1 InspectSignedExact12ActionV1(
        byte[] signedTransactionVersioned,
        NetworkId networkId,
        string authorityAccountId,
        PrivacyOperationSchemaV1 operation)
    {
        ArgumentNullException.ThrowIfNull(signedTransactionVersioned);
        ArgumentNullException.ThrowIfNull(networkId);
        ArgumentNullException.ThrowIfNull(authorityAccountId);
        _ = operation.ProtocolId();
        if (signedTransactionVersioned.Length is < 1
            or > PrivacyExact12ActionRequestV1.MaxSignedTransactionBytes)
        {
            throw new ArgumentOutOfRangeException(nameof(signedTransactionVersioned));
        }
        if (!IsAvailable())
        {
            throw new InvalidOperationException(
                "ABI-22 native Exact12 signed-action inspection is unavailable.");
        }

        var wire = (byte[])signedTransactionVersioned.Clone();
        var network = networkId.ToBytes();
        var authority = StrictUtf8Bytes(authorityAccountId, nameof(authorityAccountId));
        return RunWithNativeStack(() =>
            InspectSignedExact12ActionOnWorker(wire, network, authority, operation));
    }

    internal static PrivacyAuthenticatedTransactionDetailsQueryV1
        BuildAuthenticatedTransactionDetailsQueryV1(
            NetworkId networkId,
            CanonicalRequestCredentials credentials,
            string transactionHashHex)
    {
        ArgumentNullException.ThrowIfNull(networkId);
        ArgumentNullException.ThrowIfNull(credentials);
        RequireCanonicalLowerHash(transactionHashHex, nameof(transactionHashHex));
        if (!IsAvailable())
        {
            throw new InvalidOperationException(
                "ABI-22 native authenticated transaction-details bridge is unavailable.");
        }

        var network = networkId.ToBytes();
        var authority = StrictUtf8Bytes(credentials.AccountId, nameof(credentials));
        var transactionHash = Encoding.ASCII.GetBytes(transactionHashHex);
        var nonce = RandomNumberGenerator.GetBytes(32);
        if (!Array.Exists(nonce, static value => value != 0))
        {
            CryptographicOperations.ZeroMemory(nonce);
            throw new CryptographicException(
                "OS randomness returned an invalid all-zero transaction-details nonce.");
        }
        var privateKey = credentials.PrivateKeySeed;
        try
        {
            return RunWithNativeStack(() =>
                BuildAuthenticatedTransactionDetailsQueryOnWorker(
                    network,
                    authority,
                    transactionHash,
                    credentials.AccountId,
                    transactionHashHex,
                    nonce,
                    privateKey));
        }
        finally
        {
            CryptographicOperations.ZeroMemory(privateKey);
            CryptographicOperations.ZeroMemory(nonce);
        }
    }

    internal static PrivacyAuthenticatedCommittedResultV1
        ProjectAuthenticatedTransactionDetailsResultV1(
            PrivacyAuthenticatedTransactionDetailsQueryV1 query,
            byte[] responseNorito)
    {
        ArgumentNullException.ThrowIfNull(query);
        ArgumentNullException.ThrowIfNull(responseNorito);
        if (responseNorito.Length is < 1
            or > PrivacyAuthenticatedTransactionDetailsResponseMaxBytes)
        {
            throw new ArgumentOutOfRangeException(nameof(responseNorito));
        }
        if (!IsAvailable())
        {
            throw new InvalidOperationException(
                "ABI-22 native authenticated transaction-details bridge is unavailable.");
        }

        var response = (byte[])responseNorito.Clone();
        return RunWithNativeStack(() =>
            ProjectAuthenticatedTransactionDetailsResultOnWorker(query, response));
    }

    internal static PrivacyAuthenticatedActionReceiptQueryV1
        BuildAuthenticatedPrivacyActionReceiptQueryV1(
            NetworkId networkId,
            CanonicalRequestCredentials credentials,
            PrivacyActionOperationViewV1 operation)
    {
        ArgumentNullException.ThrowIfNull(networkId);
        ArgumentNullException.ThrowIfNull(credentials);
        ArgumentNullException.ThrowIfNull(operation);
        if (!IsAvailable())
        {
            throw new InvalidOperationException(
                "ABI-22 native authenticated Exact12 action-receipt bridge is unavailable.");
        }

        var network = networkId.ToBytes();
        var networkIdHex = Convert.ToHexString(network).ToLowerInvariant();
        RequireCanonicalNonzeroLowerHash(networkIdHex, nameof(networkId));
        var authority = StrictUtf8Bytes(credentials.AccountId, nameof(credentials));
        var transactionHashHex = Convert.ToHexString(operation.TransactionHash).ToLowerInvariant();
        RequireCanonicalNonzeroLowerHash(transactionHashHex, nameof(operation));
        var transactionHash = Encoding.ASCII.GetBytes(transactionHashHex);
        var requestedBinding = new byte[PrivacyAuthenticatedActionReceiptBindingBytes];
        operation.TransactionIntentDigest.CopyTo(requestedBinding, 0);
        operation.StatementDigest.CopyTo(requestedBinding, 32);
        operation.ProofEnvelopeHash.CopyTo(requestedBinding, 64);
        var nonce = RandomNumberGenerator.GetBytes(32);
        if (!Array.Exists(nonce, static value => value != 0))
        {
            CryptographicOperations.ZeroMemory(nonce);
            throw new CryptographicException(
                "OS randomness returned an invalid all-zero action-receipt nonce.");
        }
        var privateKey = credentials.PrivateKeySeed;
        try
        {
            return RunWithNativeStack(() =>
                BuildAuthenticatedPrivacyActionReceiptQueryOnWorker(
                    network,
                    authority,
                    checked((int)(uint)operation.OperationSchema),
                    transactionHash,
                    transactionHashHex,
                    PrivacyAuthenticatedActionReceiptIndexV1,
                    requestedBinding,
                    networkIdHex,
                    operation.ProtocolId,
                    operation.OperationSchema,
                    operation.LedgerEffectKind,
                    operation.TransactionIntentDigest,
                    operation.StatementDigest,
                    operation.ProofEnvelopeHash,
                    nonce,
                    privateKey));
        }
        finally
        {
            CryptographicOperations.ZeroMemory(privateKey);
            CryptographicOperations.ZeroMemory(nonce);
            CryptographicOperations.ZeroMemory(requestedBinding);
        }
    }

    internal static PrivacyAuthenticatedActionExecutionReceiptV1
        ProjectAuthenticatedPrivacyActionReceiptResultV1(
            PrivacyAuthenticatedActionReceiptQueryV1 query,
            byte[] responseNorito)
    {
        ArgumentNullException.ThrowIfNull(query);
        ArgumentNullException.ThrowIfNull(responseNorito);
        if (responseNorito.Length is < 1
            or > PrivacyAuthenticatedActionReceiptResponseMaxBytes)
        {
            throw new ArgumentOutOfRangeException(nameof(responseNorito));
        }
        if (!IsAvailable())
        {
            throw new InvalidOperationException(
                "ABI-22 native authenticated Exact12 action-receipt bridge is unavailable.");
        }

        var response = (byte[])responseNorito.Clone();
        return RunWithNativeStack(() =>
            ProjectAuthenticatedPrivacyActionReceiptResultOnWorker(query, response));
    }

    internal static PrivacyAuthenticatedStateQueryV1
        BuildAuthenticatedPrivacyStateQueryV1(
            NetworkId networkId,
            CanonicalRequestCredentials credentials,
            uint queryId,
            uint protocolIndex,
            byte[] requestBinding)
    {
        ArgumentNullException.ThrowIfNull(networkId);
        ArgumentNullException.ThrowIfNull(credentials);
        ArgumentNullException.ThrowIfNull(requestBinding);
        RequirePrivacyStateQueryBindingV1(queryId, protocolIndex, requestBinding);
        if (!IsAvailable())
        {
            throw new InvalidOperationException(
                "ABI-22 native authenticated privacy state-query bridge is unavailable.");
        }

        var network = networkId.ToBytes();
        if (network.Length != NetworkId.ByteLength
            || !Array.Exists(network, static value => value != 0))
        {
            throw new ArgumentException("Privacy state query requires a nonzero NetworkId.", nameof(networkId));
        }
        var authority = StrictUtf8Bytes(credentials.AccountId, nameof(credentials));
        var binding = (byte[])requestBinding.Clone();
        var nonce = RandomNumberGenerator.GetBytes(32);
        if (!Array.Exists(nonce, static value => value != 0))
        {
            CryptographicOperations.ZeroMemory(nonce);
            throw new CryptographicException(
                "OS randomness returned an invalid all-zero privacy state-query nonce.");
        }
        var privateKey = credentials.PrivateKeySeed;
        try
        {
            return RunWithNativeStack(() =>
                BuildAuthenticatedPrivacyStateQueryOnWorker(
                    network,
                    authority,
                    queryId,
                    protocolIndex,
                    binding,
                    networkId,
                    nonce,
                    privateKey));
        }
        finally
        {
            CryptographicOperations.ZeroMemory(privateKey);
            CryptographicOperations.ZeroMemory(nonce);
            CryptographicOperations.ZeroMemory(binding);
        }
    }

    internal static byte[] ProjectAuthenticatedPrivacyStateQueryResultV1(
        PrivacyAuthenticatedStateQueryV1 query,
        byte[] responseNorito)
    {
        ArgumentNullException.ThrowIfNull(query);
        ArgumentNullException.ThrowIfNull(responseNorito);
        if (responseNorito.Length is < 1
            or > PrivacyAuthenticatedStateQueryResponseMaxBytes)
        {
            throw new ArgumentOutOfRangeException(nameof(responseNorito));
        }
        if (!IsAvailable())
        {
            throw new InvalidOperationException(
                "ABI-22 native authenticated privacy state-query bridge is unavailable.");
        }

        var response = (byte[])responseNorito.Clone();
        return RunWithNativeStack(() =>
            ProjectAuthenticatedPrivacyStateQueryResultOnWorker(query, response));
    }

    internal static void RequirePrivacyStateQueryBindingV1(
        uint queryId,
        uint protocolIndex,
        byte[] requestBinding)
    {
        ArgumentNullException.ThrowIfNull(requestBinding);
        var expectedLength = queryId switch
        {
            97 => 64,
            98 => 32,
            99 => 32,
            100 => 64,
            101 => 32,
            102 => 128,
            103 => 128,
            104 => 96,
            _ => throw new ArgumentOutOfRangeException(
                nameof(queryId),
                "Privacy state-query ID must be in the closed range 97 through 104."),
        };
        if (requestBinding.Length != expectedLength)
        {
            throw new ArgumentException(
                "Privacy state-query binding has the wrong fixed width.",
                nameof(requestBinding));
        }
        if (queryId == 98)
        {
            if (protocolIndex > 2)
            {
                throw new ArgumentOutOfRangeException(
                    nameof(protocolIndex),
                    "Proof-managed pool protocol index must select FCMP++, private-IVM, or PQ-MASP.");
            }
        }
        else if (protocolIndex != 0)
        {
            throw new ArgumentOutOfRangeException(
                nameof(protocolIndex),
                "Only query ID 98 accepts a nonzero protocol index.");
        }
        for (var offset = 0; offset < requestBinding.Length; offset += 32)
        {
            var nonzero = false;
            for (var index = offset; index < offset + 32; index++)
            {
                nonzero |= requestBinding[index] != 0;
            }
            if (!nonzero)
            {
                throw new ArgumentException(
                    "Every privacy state-query selector chunk must be nonzero.",
                    nameof(requestBinding));
            }
        }
    }

    private static PrivacySignedExact12ActionInspectionV1 InspectSignedExact12ActionOnWorker(
        byte[] signedTransactionVersioned,
        byte[] networkId,
        byte[] authorityAccountId,
        PrivacyOperationSchemaV1 operation)
    {
        IntPtr pointer = IntPtr.Zero;
        UIntPtr length = UIntPtr.Zero;
        var status = NativeInspectSignedExact12Action(
            signedTransactionVersioned,
            new UIntPtr((uint)signedTransactionVersioned.Length),
            networkId,
            new UIntPtr((uint)networkId.Length),
            authorityAccountId,
            new UIntPtr((uint)authorityAccountId.Length),
            checked((int)(uint)operation),
            out pointer,
            out length);
        try
        {
            var projection = CopyNativeOutput(
                status,
                pointer,
                length,
                PrivacySignedExact12ActionProjectionBytes,
                PrivacySignedExact12ActionProjectionBytes,
                "Native Exact12 signed-action inspection failed.");
            return new PrivacySignedExact12ActionInspectionV1(projection);
        }
        finally
        {
            if (pointer != IntPtr.Zero)
            {
                NativeFree(pointer);
            }
        }
    }

    private static PrivacyAuthenticatedTransactionDetailsQueryV1
        BuildAuthenticatedTransactionDetailsQueryOnWorker(
            byte[] networkId,
            byte[] authority,
            byte[] transactionHash,
            string authorityLiteral,
            string transactionHashHex,
            byte[] nonce,
            byte[] privateKey)
    {
        IntPtr preparationPointer = IntPtr.Zero;
        UIntPtr preparationLength = UIntPtr.Zero;
        IntPtr digestPointer = IntPtr.Zero;
        UIntPtr digestLength = UIntPtr.Zero;
        var creationTime = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
        if (creationTime <= 0)
        {
            throw new InvalidOperationException(
                "System clock cannot create a fresh authenticated transaction-details query.");
        }
        var status = NativeAuthenticatedTransactionDetailsPrepare(
            networkId,
            new UIntPtr((uint)networkId.Length),
            authority,
            new UIntPtr((uint)authority.Length),
            transactionHash,
            new UIntPtr((uint)transactionHash.Length),
            checked((ulong)creationTime),
            nonce,
            new UIntPtr((uint)nonce.Length),
            out preparationPointer,
            out preparationLength,
            out digestPointer,
            out digestLength);
        byte[]? signingDigest = null;
        byte[]? signature = null;
        try
        {
            var preparation = CopyNativeOutput(
                status,
                preparationPointer,
                preparationLength,
                1,
                PrivacyAuthenticatedTransactionDetailsPreparationMaxBytes,
                "Native authenticated transaction-details preparation failed.");
            signingDigest = CopyNativeOutput(
                status,
                digestPointer,
                digestLength,
                32,
                32,
                "Native authenticated transaction-details signing digest is invalid.");
            if (!Array.Exists(signingDigest, static value => value != 0))
            {
                throw new InvalidDataException(
                    "Native authenticated transaction-details signing digest is all zero.");
            }
            signature = Ed25519Signer.Sign(signingDigest, privateKey);
            return FinalizeAuthenticatedTransactionDetailsQueryOnWorker(
                preparation,
                signature,
                transactionHashHex,
                authorityLiteral);
        }
        finally
        {
            if (preparationPointer != IntPtr.Zero)
            {
                NativeFree(preparationPointer);
            }
            if (digestPointer != IntPtr.Zero)
            {
                NativeFree(digestPointer);
            }
            if (signingDigest is not null)
            {
                CryptographicOperations.ZeroMemory(signingDigest);
            }
            if (signature is not null)
            {
                CryptographicOperations.ZeroMemory(signature);
            }
        }
    }

    private static PrivacyAuthenticatedTransactionDetailsQueryV1
        FinalizeAuthenticatedTransactionDetailsQueryOnWorker(
            byte[] preparation,
            byte[] signature,
            string transactionHashHex,
            string authorityLiteral)
    {
        IntPtr pointer = IntPtr.Zero;
        UIntPtr length = UIntPtr.Zero;
        var status = NativeAuthenticatedTransactionDetailsFinalize(
            preparation,
            new UIntPtr((uint)preparation.Length),
            signature,
            new UIntPtr((uint)signature.Length),
            out pointer,
            out length);
        try
        {
            var signedQuery = CopyNativeOutput(
                status,
                pointer,
                length,
                1,
                PrivacyAuthenticatedTransactionDetailsSignedQueryMaxBytes,
                "Native authenticated transaction-details finalization failed.");
            return new PrivacyAuthenticatedTransactionDetailsQueryV1(
                preparation,
                signedQuery,
                transactionHashHex,
                authorityLiteral);
        }
        finally
        {
            if (pointer != IntPtr.Zero)
            {
                NativeFree(pointer);
            }
        }
    }

    private static PrivacyAuthenticatedCommittedResultV1
        ProjectAuthenticatedTransactionDetailsResultOnWorker(
            PrivacyAuthenticatedTransactionDetailsQueryV1 query,
            byte[] responseNorito)
    {
        var preparation = query.Preparation;
        IntPtr pointer = IntPtr.Zero;
        UIntPtr length = UIntPtr.Zero;
        var status = NativeAuthenticatedTransactionDetailsProjectResult(
            preparation,
            new UIntPtr((uint)preparation.Length),
            responseNorito,
            new UIntPtr((uint)responseNorito.Length),
            out pointer,
            out length);
        try
        {
            var jsonBytes = CopyNativeOutput(
                status,
                pointer,
                length,
                1,
                PrivacyAuthenticatedTransactionDetailsProjectionMaxBytes,
                "Native authenticated transaction-details result projection failed.");
            return ParseAuthenticatedTransactionDetailsResult(
                jsonBytes,
                query.TransactionHashHex,
                query.TransactionAuthority);
        }
        finally
        {
            if (pointer != IntPtr.Zero)
            {
                NativeFree(pointer);
            }
        }
    }

    private static PrivacyAuthenticatedActionReceiptQueryV1
        BuildAuthenticatedPrivacyActionReceiptQueryOnWorker(
            byte[] networkId,
            byte[] authority,
            int operationIndex,
            byte[] transactionHash,
            string transactionHashHex,
            uint actionIndex,
            byte[] requestedBinding,
            string networkIdHex,
            PrivacyProtocolIdV1 protocolId,
            PrivacyOperationSchemaV1 operationSchema,
            PrivacyLedgerEffectKindV1 ledgerEffectKind,
            byte[] transactionIntentDigest,
            byte[] statementDigest,
            byte[] proofEnvelopeHash,
            byte[] nonce,
            byte[] privateKey)
    {
        IntPtr preparationPointer = IntPtr.Zero;
        UIntPtr preparationLength = UIntPtr.Zero;
        IntPtr digestPointer = IntPtr.Zero;
        UIntPtr digestLength = UIntPtr.Zero;
        var creationTime = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
        if (creationTime <= 0)
        {
            throw new InvalidOperationException(
                "System clock cannot create a fresh authenticated action-receipt query.");
        }
        var status = NativeAuthenticatedActionReceiptPrepare(
            networkId,
            new UIntPtr((uint)networkId.Length),
            authority,
            new UIntPtr((uint)authority.Length),
            operationIndex,
            transactionHash,
            new UIntPtr((uint)transactionHash.Length),
            actionIndex,
            requestedBinding,
            new UIntPtr((uint)requestedBinding.Length),
            checked((ulong)creationTime),
            nonce,
            new UIntPtr((uint)nonce.Length),
            out preparationPointer,
            out preparationLength,
            out digestPointer,
            out digestLength);
        byte[]? signingDigest = null;
        byte[]? signature = null;
        try
        {
            var preparation = CopyNativeOutput(
                status,
                preparationPointer,
                preparationLength,
                1,
                PrivacyAuthenticatedActionReceiptPreparationMaxBytes,
                "Native authenticated action-receipt preparation failed.");
            signingDigest = CopyNativeOutput(
                status,
                digestPointer,
                digestLength,
                32,
                32,
                "Native authenticated action-receipt signing digest is invalid.");
            if (!Array.Exists(signingDigest, static value => value != 0))
            {
                throw new InvalidDataException(
                    "Native authenticated action-receipt signing digest is all zero.");
            }
            signature = Ed25519Signer.Sign(signingDigest, privateKey);
            return FinalizeAuthenticatedPrivacyActionReceiptQueryOnWorker(
                preparation,
                signature,
                networkIdHex,
                protocolId,
                operationSchema,
                ledgerEffectKind,
                transactionHashHex,
                actionIndex,
                transactionIntentDigest,
                statementDigest,
                proofEnvelopeHash);
        }
        finally
        {
            if (preparationPointer != IntPtr.Zero)
            {
                NativeFree(preparationPointer);
            }
            if (digestPointer != IntPtr.Zero)
            {
                NativeFree(digestPointer);
            }
            if (signingDigest is not null)
            {
                CryptographicOperations.ZeroMemory(signingDigest);
            }
            if (signature is not null)
            {
                CryptographicOperations.ZeroMemory(signature);
            }
        }
    }

    private static PrivacyAuthenticatedActionReceiptQueryV1
        FinalizeAuthenticatedPrivacyActionReceiptQueryOnWorker(
            byte[] preparation,
            byte[] signature,
            string networkIdHex,
            PrivacyProtocolIdV1 protocolId,
            PrivacyOperationSchemaV1 operationSchema,
            PrivacyLedgerEffectKindV1 ledgerEffectKind,
            string transactionHashHex,
            uint actionIndex,
            byte[] transactionIntentDigest,
            byte[] statementDigest,
            byte[] proofEnvelopeHash)
    {
        IntPtr pointer = IntPtr.Zero;
        UIntPtr length = UIntPtr.Zero;
        var status = NativeAuthenticatedActionReceiptFinalize(
            preparation,
            new UIntPtr((uint)preparation.Length),
            signature,
            new UIntPtr((uint)signature.Length),
            out pointer,
            out length);
        try
        {
            var signedQuery = CopyNativeOutput(
                status,
                pointer,
                length,
                1,
                PrivacyAuthenticatedActionReceiptSignedQueryMaxBytes,
                "Native authenticated action-receipt finalization failed.");
            return new PrivacyAuthenticatedActionReceiptQueryV1(
                preparation,
                signedQuery,
                networkIdHex,
                protocolId,
                operationSchema,
                ledgerEffectKind,
                transactionHashHex,
                actionIndex,
                transactionIntentDigest,
                statementDigest,
                proofEnvelopeHash);
        }
        finally
        {
            if (pointer != IntPtr.Zero)
            {
                NativeFree(pointer);
            }
        }
    }

    private static PrivacyAuthenticatedActionExecutionReceiptV1
        ProjectAuthenticatedPrivacyActionReceiptResultOnWorker(
            PrivacyAuthenticatedActionReceiptQueryV1 query,
            byte[] responseNorito)
    {
        var preparation = query.Preparation;
        IntPtr pointer = IntPtr.Zero;
        UIntPtr length = UIntPtr.Zero;
        var status = NativeAuthenticatedActionReceiptProjectResult(
            preparation,
            new UIntPtr((uint)preparation.Length),
            responseNorito,
            new UIntPtr((uint)responseNorito.Length),
            out pointer,
            out length);
        try
        {
            var jsonBytes = CopyNativeOutput(
                status,
                pointer,
                length,
                1,
                PrivacyAuthenticatedActionReceiptProjectionMaxBytes,
                "Native authenticated action-receipt result projection failed.");
            return ParseAuthenticatedPrivacyActionReceiptResult(jsonBytes, query);
        }
        finally
        {
            if (pointer != IntPtr.Zero)
            {
                NativeFree(pointer);
            }
        }
    }

    private static PrivacyAuthenticatedStateQueryV1
        BuildAuthenticatedPrivacyStateQueryOnWorker(
            byte[] networkId,
            byte[] authority,
            uint queryId,
            uint protocolIndex,
            byte[] requestBinding,
            NetworkId expectedNetworkId,
            byte[] nonce,
            byte[] privateKey)
    {
        IntPtr preparationPointer = IntPtr.Zero;
        UIntPtr preparationLength = UIntPtr.Zero;
        IntPtr digestPointer = IntPtr.Zero;
        UIntPtr digestLength = UIntPtr.Zero;
        var creationTime = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
        if (creationTime <= 0)
        {
            throw new InvalidOperationException(
                "System clock cannot create a fresh authenticated privacy state query.");
        }
        var status = NativeAuthenticatedPrivacyStateQueryPrepare(
            networkId,
            new UIntPtr((uint)networkId.Length),
            authority,
            new UIntPtr((uint)authority.Length),
            queryId,
            protocolIndex,
            requestBinding,
            new UIntPtr((uint)requestBinding.Length),
            checked((ulong)creationTime),
            nonce,
            new UIntPtr((uint)nonce.Length),
            out preparationPointer,
            out preparationLength,
            out digestPointer,
            out digestLength);
        byte[]? signingDigest = null;
        byte[]? signature = null;
        try
        {
            var preparation = CopyNativeOutput(
                status,
                preparationPointer,
                preparationLength,
                1,
                PrivacyAuthenticatedStateQueryPreparationMaxBytes,
                "Native authenticated privacy state-query preparation failed.");
            signingDigest = CopyNativeOutput(
                status,
                digestPointer,
                digestLength,
                32,
                32,
                "Native authenticated privacy state-query signing digest is invalid.");
            if (!Array.Exists(signingDigest, static value => value != 0))
            {
                throw new InvalidDataException(
                    "Native authenticated privacy state-query signing digest is all zero.");
            }
            signature = Ed25519Signer.Sign(signingDigest, privateKey);
            return FinalizeAuthenticatedPrivacyStateQueryOnWorker(
                preparation,
                signature,
                expectedNetworkId,
                queryId,
                protocolIndex,
                requestBinding);
        }
        finally
        {
            if (preparationPointer != IntPtr.Zero)
            {
                NativeFree(preparationPointer);
            }
            if (digestPointer != IntPtr.Zero)
            {
                NativeFree(digestPointer);
            }
            if (signingDigest is not null)
            {
                CryptographicOperations.ZeroMemory(signingDigest);
            }
            if (signature is not null)
            {
                CryptographicOperations.ZeroMemory(signature);
            }
        }
    }

    private static PrivacyAuthenticatedStateQueryV1
        FinalizeAuthenticatedPrivacyStateQueryOnWorker(
            byte[] preparation,
            byte[] signature,
            NetworkId networkId,
            uint queryId,
            uint protocolIndex,
            byte[] requestBinding)
    {
        IntPtr pointer = IntPtr.Zero;
        UIntPtr length = UIntPtr.Zero;
        var status = NativeAuthenticatedPrivacyStateQueryFinalize(
            preparation,
            new UIntPtr((uint)preparation.Length),
            signature,
            new UIntPtr((uint)signature.Length),
            out pointer,
            out length);
        try
        {
            var signedQuery = CopyNativeOutput(
                status,
                pointer,
                length,
                1,
                PrivacyAuthenticatedStateQuerySignedQueryMaxBytes,
                "Native authenticated privacy state-query finalization failed.");
            return new PrivacyAuthenticatedStateQueryV1(
                preparation,
                signedQuery,
                networkId,
                queryId,
                protocolIndex,
                requestBinding);
        }
        finally
        {
            if (pointer != IntPtr.Zero)
            {
                NativeFree(pointer);
            }
        }
    }

    private static byte[] ProjectAuthenticatedPrivacyStateQueryResultOnWorker(
        PrivacyAuthenticatedStateQueryV1 query,
        byte[] responseNorito)
    {
        var preparation = query.Preparation;
        IntPtr pointer = IntPtr.Zero;
        UIntPtr length = UIntPtr.Zero;
        var status = NativeAuthenticatedPrivacyStateQueryProjectResult(
            preparation,
            new UIntPtr((uint)preparation.Length),
            responseNorito,
            new UIntPtr((uint)responseNorito.Length),
            out pointer,
            out length);
        try
        {
            return CopyNativeOutput(
                status,
                pointer,
                length,
                1,
                PrivacyAuthenticatedStateQueryProjectionMaxBytes,
                "Native authenticated privacy state-query result projection failed.");
        }
        finally
        {
            if (pointer != IntPtr.Zero)
            {
                NativeFree(pointer);
            }
        }
    }

    internal static PrivacyAuthenticatedCommittedResultV1
        ParseAuthenticatedTransactionDetailsResult(
            byte[] jsonBytes,
            string expectedTransactionHash,
            string expectedAuthority)
    {
        string json;
        try
        {
            json = new UTF8Encoding(false, true).GetString(jsonBytes);
        }
        catch (DecoderFallbackException error)
        {
            throw new InvalidDataException(
                "Native authenticated transaction-details projection is not UTF-8.",
                error);
        }
        using var document = JsonDocument.Parse(
            json,
            new JsonDocumentOptions
            {
                AllowTrailingCommas = false,
                CommentHandling = JsonCommentHandling.Disallow,
                MaxDepth = 8,
            });
        var root = document.RootElement;
        if (root.ValueKind != JsonValueKind.Object)
        {
            throw new InvalidDataException(
                "Native authenticated transaction-details projection must be an object.");
        }
        var expectedFields = new HashSet<string>(StringComparer.Ordinal)
        {
            "version",
            "transaction_hash_hex",
            "transaction_authority",
            "block_hash_hex",
            "result_hash_hex",
            "result_ok",
            "rejection_message",
            "committed_block_height",
        };
        var observedFields = new HashSet<string>(StringComparer.Ordinal);
        foreach (var property in root.EnumerateObject())
        {
            if (!observedFields.Add(property.Name))
            {
                throw new InvalidDataException(
                    "Native authenticated transaction-details projection contains duplicate fields.");
            }
        }
        if (!observedFields.SetEquals(expectedFields))
        {
            throw new InvalidDataException(
                "Native authenticated transaction-details projection fields are invalid.");
        }

        var version = root.GetProperty("version");
        if (version.ValueKind != JsonValueKind.Number
            || !version.TryGetUInt32(out var versionValue)
            || versionValue != 1
            || version.GetRawText() != "1")
        {
            throw new InvalidDataException(
                "Native authenticated transaction-details projection version is invalid.");
        }
        var transactionHash = RequireCanonicalLowerHashJson(
            root.GetProperty("transaction_hash_hex"),
            "transaction_hash_hex");
        if (!string.Equals(transactionHash, expectedTransactionHash, StringComparison.Ordinal))
        {
            throw new InvalidDataException(
                "Native authenticated transaction-details projection changed the transaction hash.");
        }
        var authority = RequireExactJsonString(
            root.GetProperty("transaction_authority"),
            "transaction_authority");
        if (!string.Equals(authority, expectedAuthority, StringComparison.Ordinal))
        {
            throw new InvalidDataException(
                "Native authenticated transaction-details projection changed the authority.");
        }
        var blockHash = RequireCanonicalLowerHashJson(
            root.GetProperty("block_hash_hex"),
            "block_hash_hex");
        var resultHash = RequireCanonicalLowerHashJson(
            root.GetProperty("result_hash_hex"),
            "result_hash_hex");
        var resultElement = root.GetProperty("result_ok");
        if (resultElement.ValueKind is not (JsonValueKind.True or JsonValueKind.False))
        {
            throw new InvalidDataException(
                "Native authenticated transaction-details result_ok must be a boolean.");
        }
        var resultOk = resultElement.GetBoolean();
        var rejectionElement = root.GetProperty("rejection_message");
        string? rejectionMessage;
        if (resultOk)
        {
            if (rejectionElement.ValueKind != JsonValueKind.Null)
            {
                throw new InvalidDataException(
                    "Successful authenticated transaction details carry a rejection message.");
            }
            rejectionMessage = null;
        }
        else
        {
            rejectionMessage = RequireExactJsonString(
                rejectionElement,
                "rejection_message");
            if (rejectionMessage.Length < 1
                || Encoding.UTF8.GetByteCount(rejectionMessage) > 1_024
                || !string.Equals(
                    rejectionMessage,
                    rejectionMessage.Trim(),
                    StringComparison.Ordinal)
                || rejectionMessage.Any(char.IsControl))
            {
                throw new InvalidDataException(
                    "Rejected authenticated transaction details have an invalid reason.");
            }
        }
        var committedHeightText = RequireExactJsonString(
            root.GetProperty("committed_block_height"),
            "committed_block_height");
        if (committedHeightText.Length == 0
            || committedHeightText[0] == '0'
            || !ulong.TryParse(
                committedHeightText,
                NumberStyles.None,
                CultureInfo.InvariantCulture,
                out var committedHeight)
            || committedHeight == 0)
        {
            throw new InvalidDataException(
                "Native authenticated transaction-details committed height is invalid.");
        }
        return new PrivacyAuthenticatedCommittedResultV1(
            transactionHash,
            authority,
            blockHash,
            resultHash,
            resultOk,
            rejectionMessage,
            committedHeight);
    }

    internal static PrivacyAuthenticatedActionExecutionReceiptV1
        ParseAuthenticatedPrivacyActionReceiptResult(
            byte[] jsonBytes,
            PrivacyAuthenticatedActionReceiptQueryV1 query)
    {
        ArgumentNullException.ThrowIfNull(jsonBytes);
        ArgumentNullException.ThrowIfNull(query);
        string json;
        try
        {
            json = new UTF8Encoding(false, true).GetString(jsonBytes);
        }
        catch (DecoderFallbackException error)
        {
            throw new InvalidDataException(
                "Native authenticated action-receipt projection is not UTF-8.",
                error);
        }
        using var document = JsonDocument.Parse(
            json,
            new JsonDocumentOptions
            {
                AllowTrailingCommas = false,
                CommentHandling = JsonCommentHandling.Disallow,
                MaxDepth = 8,
            });
        var root = document.RootElement;
        if (root.ValueKind != JsonValueKind.Object)
        {
            throw new InvalidDataException(
                "Native authenticated action-receipt projection must be an object.");
        }
        var expectedFields = new HashSet<string>(StringComparer.Ordinal)
        {
            "version",
            "network_id",
            "protocol_id",
            "operation_schema",
            "ledger_effect_kind",
            "transaction_hash",
            "action_index",
            "transaction_intent_digest",
            "statement_digest",
            "proof_envelope_hash",
            "capability_manifest_digest",
            "capability_committed_height",
            "admitted_at_height",
            "finalized_height",
            "finalized_block_hash",
        };
        var observedFields = new HashSet<string>(StringComparer.Ordinal);
        foreach (var property in root.EnumerateObject())
        {
            if (!observedFields.Add(property.Name))
            {
                throw new InvalidDataException(
                    "Native authenticated action-receipt projection contains duplicate fields.");
            }
        }
        if (!observedFields.SetEquals(expectedFields))
        {
            throw new InvalidDataException(
                "Native authenticated action-receipt projection fields are invalid.");
        }

        var version = root.GetProperty("version");
        if (version.ValueKind != JsonValueKind.Number
            || !version.TryGetUInt32(out var versionValue)
            || versionValue != 1
            || version.GetRawText() != "1")
        {
            throw new InvalidDataException(
                "Native authenticated action-receipt projection version is invalid.");
        }
        var actionIndexElement = root.GetProperty("action_index");
        if (actionIndexElement.ValueKind != JsonValueKind.Number
            || !actionIndexElement.TryGetUInt32(out var actionIndex)
            || actionIndex != query.ActionIndex
            || actionIndexElement.GetRawText()
                != query.ActionIndex.ToString(CultureInfo.InvariantCulture))
        {
            throw new InvalidDataException(
                "Native authenticated action-receipt action index is invalid.");
        }

        var networkId = RequireCanonicalNonzeroLowerHashJson(
            root.GetProperty("network_id"),
            "network_id");
        var transactionHash = RequireCanonicalNonzeroLowerHashJson(
            root.GetProperty("transaction_hash"),
            "transaction_hash");
        if (!string.Equals(networkId, query.NetworkIdHex, StringComparison.Ordinal)
            || !string.Equals(
                transactionHash,
                query.TransactionHashHex,
                StringComparison.Ordinal))
        {
            throw new InvalidDataException(
                "Native authenticated action receipt changed its network or transaction binding.");
        }

        var protocolLabel = RequireReceiptJsonString(
            root.GetProperty("protocol_id"),
            "protocol_id");
        var operationLabel = RequireReceiptJsonString(
            root.GetProperty("operation_schema"),
            "operation_schema");
        var effectLabel = RequireReceiptJsonString(
            root.GetProperty("ledger_effect_kind"),
            "ledger_effect_kind");
        PrivacyProtocolIdV1 protocolId;
        PrivacyOperationSchemaV1 operationSchema;
        PrivacyLedgerEffectKindV1 ledgerEffectKind;
        try
        {
            protocolId = PrivacyProtocolsV1.ParseCanonicalLabel(protocolLabel);
            operationSchema = PrivacyExact12ActionContractV1.ParseOperationCanonicalLabel(
                operationLabel);
            ledgerEffectKind = PrivacyExact12ActionContractV1
                .ParseLedgerEffectCanonicalLabel(effectLabel);
        }
        catch (ArgumentException error)
        {
            throw new InvalidDataException(
                "Native authenticated action receipt contains a non-canonical typed label.",
                error);
        }
        if (protocolId != query.ProtocolId
            || operationSchema != query.OperationSchema
            || ledgerEffectKind != query.LedgerEffectKind
            || protocolId != operationSchema.ProtocolId()
            || ledgerEffectKind != operationSchema.LedgerEffectKind())
        {
            throw new InvalidDataException(
                "Native authenticated action receipt changed its protocol, operation, or effect binding.");
        }

        var transactionIntentDigest = RequireCanonicalNonzeroLowerHashBytesJson(
            root.GetProperty("transaction_intent_digest"),
            "transaction_intent_digest");
        var statementDigest = RequireCanonicalNonzeroLowerHashBytesJson(
            root.GetProperty("statement_digest"),
            "statement_digest");
        var proofEnvelopeHash = RequireCanonicalNonzeroLowerHashBytesJson(
            root.GetProperty("proof_envelope_hash"),
            "proof_envelope_hash");
        if (!CryptographicOperations.FixedTimeEquals(
                transactionIntentDigest,
                query.TransactionIntentDigest)
            || !CryptographicOperations.FixedTimeEquals(
                statementDigest,
                query.StatementDigest)
            || !CryptographicOperations.FixedTimeEquals(
                proofEnvelopeHash,
                query.ProofEnvelopeHash))
        {
            throw new InvalidDataException(
                "Native authenticated action receipt changed the inspected action binding.");
        }

        var capabilityManifestDigest = RequireCanonicalNonzeroLowerHashBytesJson(
            root.GetProperty("capability_manifest_digest"),
            "capability_manifest_digest");
        var finalizedBlockHash = RequireCanonicalNonzeroLowerHashBytesJson(
            root.GetProperty("finalized_block_hash"),
            "finalized_block_hash");
        var capabilityCommittedHeight = RequirePositiveDecimalHeightJson(
            root.GetProperty("capability_committed_height"),
            "capability_committed_height");
        var admittedAtHeight = RequirePositiveDecimalHeightJson(
            root.GetProperty("admitted_at_height"),
            "admitted_at_height");
        var finalizedHeight = RequirePositiveDecimalHeightJson(
            root.GetProperty("finalized_height"),
            "finalized_height");
        if (capabilityCommittedHeight > admittedAtHeight
            || admittedAtHeight > finalizedHeight)
        {
            throw new InvalidDataException(
                "Native authenticated action receipt has contradictory capability, admission, or finality heights.");
        }

        return new PrivacyAuthenticatedActionExecutionReceiptV1(
            networkId,
            protocolId,
            operationSchema,
            ledgerEffectKind,
            transactionHash,
            actionIndex,
            capabilityManifestDigest,
            capabilityCommittedHeight,
            admittedAtHeight,
            finalizedHeight,
            finalizedBlockHash);
    }

    private static byte[] CopyNativeOutput(
        int status,
        IntPtr pointer,
        UIntPtr length,
        int minimumBytes,
        int maximumBytes,
        string failureMessage)
    {
        var count64 = length.ToUInt64();
        if (status != 0
            || pointer == IntPtr.Zero
            || count64 < (ulong)minimumBytes
            || count64 > (ulong)maximumBytes
            || count64 > int.MaxValue)
        {
            throw new InvalidOperationException(failureMessage);
        }
        var output = new byte[(int)count64];
        Marshal.Copy(pointer, output, 0, output.Length);
        return output;
    }

    private static byte[] StrictUtf8Bytes(string value, string parameterName)
    {
        ArgumentNullException.ThrowIfNull(value, parameterName);
        try
        {
            return new UTF8Encoding(false, true).GetBytes(value);
        }
        catch (EncoderFallbackException error)
        {
            throw new ArgumentException("Value must be valid UTF-8.", parameterName, error);
        }
    }

    private static string RequireExactJsonString(JsonElement element, string field)
    {
        if (element.ValueKind != JsonValueKind.String)
        {
            throw new InvalidDataException(
                $"Native authenticated transaction-details {field} must be a string.");
        }
        return element.GetString()
            ?? throw new InvalidDataException(
                $"Native authenticated transaction-details {field} is null.");
    }

    private static string RequireCanonicalLowerHashJson(JsonElement element, string field)
    {
        var value = RequireExactJsonString(element, field);
        RequireCanonicalLowerHash(value, field);
        return value;
    }

    private static string RequireReceiptJsonString(JsonElement element, string field)
    {
        if (element.ValueKind != JsonValueKind.String)
        {
            throw new InvalidDataException(
                $"Native authenticated action-receipt {field} must be a string.");
        }
        return element.GetString()
            ?? throw new InvalidDataException(
                $"Native authenticated action-receipt {field} is null.");
    }

    private static string RequireCanonicalNonzeroLowerHashJson(
        JsonElement element,
        string field)
    {
        var value = RequireReceiptJsonString(element, field);
        try
        {
            RequireCanonicalNonzeroLowerHash(value, field);
        }
        catch (ArgumentException error)
        {
            throw new InvalidDataException(
                $"Native authenticated action-receipt {field} is not a nonzero lowercase hash.",
                error);
        }
        return value;
    }

    private static byte[] RequireCanonicalNonzeroLowerHashBytesJson(
        JsonElement element,
        string field) =>
        Convert.FromHexString(RequireCanonicalNonzeroLowerHashJson(element, field));

    private static ulong RequirePositiveDecimalHeightJson(
        JsonElement element,
        string field)
    {
        var value = RequireReceiptJsonString(element, field);
        if (value.Length == 0
            || value[0] == '0'
            || !ulong.TryParse(
                value,
                NumberStyles.None,
                CultureInfo.InvariantCulture,
                out var height)
            || height == 0)
        {
            throw new InvalidDataException(
                $"Native authenticated action-receipt {field} is not a positive canonical decimal height.");
        }
        return height;
    }

    private static void RequireCanonicalLowerHash(string value, string parameterName)
    {
        ArgumentNullException.ThrowIfNull(value, parameterName);
        if (value.Length != 64
            || !value.All(static character =>
                character is >= '0' and <= '9' or >= 'a' and <= 'f'))
        {
            throw new ArgumentException(
                "Hash must contain exactly 64 lowercase hexadecimal characters.",
                parameterName);
        }
    }

    private static void RequireCanonicalNonzeroLowerHash(
        string value,
        string parameterName)
    {
        RequireCanonicalLowerHash(value, parameterName);
        if (!value.Any(static character => character != '0'))
        {
            throw new ArgumentException(
                "Hash must not be all zero.",
                parameterName);
        }
    }

    private static T RunWithNativeStack<T>(Func<T> action)
    {
        ArgumentNullException.ThrowIfNull(action);
        T result = default!;
        ExceptionDispatchInfo? failure = null;
        var worker = new Thread(
            () =>
            {
                try
                {
                    result = action();
                }
                catch (Exception error)
                {
                    failure = ExceptionDispatchInfo.Capture(error);
                }
            },
            NativeWorkerStackBytes)
        {
            IsBackground = true,
            Name = "Iroha privacy native bridge",
        };
        worker.Start();
        worker.Join();
        failure?.Throw();
        return result;
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
        EntryPoint = "iroha_privacy_inspect_signed_exact12_action_v1",
        CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeInspectSignedExact12Action(
        [In] byte[] signedTransactionVersioned,
        UIntPtr signedTransactionVersionedLength,
        [In] byte[] networkId,
        UIntPtr networkIdLength,
        [In] byte[] authorityAccountId,
        UIntPtr authorityAccountIdLength,
        int operationIndex,
        out IntPtr output,
        out UIntPtr outputLength);

    [DllImport(
        LibraryName,
        EntryPoint = "iroha_privacy_authenticated_transaction_details_prepare_v1",
        CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeAuthenticatedTransactionDetailsPrepare(
        [In] byte[] networkId,
        UIntPtr networkIdLength,
        [In] byte[] authority,
        UIntPtr authorityLength,
        [In] byte[] transactionHashHex,
        UIntPtr transactionHashHexLength,
        ulong creationTimeMs,
        [In] byte[] nonce,
        UIntPtr nonceLength,
        out IntPtr preparation,
        out UIntPtr preparationLength,
        out IntPtr signingDigest,
        out UIntPtr signingDigestLength);

    [DllImport(
        LibraryName,
        EntryPoint = "iroha_privacy_authenticated_transaction_details_finalize_v1",
        CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeAuthenticatedTransactionDetailsFinalize(
        [In] byte[] preparation,
        UIntPtr preparationLength,
        [In] byte[] signature,
        UIntPtr signatureLength,
        out IntPtr output,
        out UIntPtr outputLength);

    [DllImport(
        LibraryName,
        EntryPoint = "iroha_privacy_authenticated_transaction_details_project_result_v1",
        CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeAuthenticatedTransactionDetailsProjectResult(
        [In] byte[] preparation,
        UIntPtr preparationLength,
        [In] byte[] response,
        UIntPtr responseLength,
        out IntPtr output,
        out UIntPtr outputLength);

    [DllImport(
        LibraryName,
        EntryPoint = "iroha_privacy_authenticated_action_receipt_prepare_v1",
        CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeAuthenticatedActionReceiptPrepare(
        [In] byte[] networkId,
        UIntPtr networkIdLength,
        [In] byte[] authority,
        UIntPtr authorityLength,
        int operationIndex,
        [In] byte[] transactionHashHex,
        UIntPtr transactionHashHexLength,
        uint actionIndex,
        [In] byte[] requestedActionBinding,
        UIntPtr requestedActionBindingLength,
        ulong creationTimeMs,
        [In] byte[] nonce,
        UIntPtr nonceLength,
        out IntPtr preparation,
        out UIntPtr preparationLength,
        out IntPtr signingDigest,
        out UIntPtr signingDigestLength);

    [DllImport(
        LibraryName,
        EntryPoint = "iroha_privacy_authenticated_action_receipt_finalize_v1",
        CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeAuthenticatedActionReceiptFinalize(
        [In] byte[] preparation,
        UIntPtr preparationLength,
        [In] byte[] signature,
        UIntPtr signatureLength,
        out IntPtr output,
        out UIntPtr outputLength);

    [DllImport(
        LibraryName,
        EntryPoint = "iroha_privacy_authenticated_action_receipt_project_result_v1",
        CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeAuthenticatedActionReceiptProjectResult(
        [In] byte[] preparation,
        UIntPtr preparationLength,
        [In] byte[] response,
        UIntPtr responseLength,
        out IntPtr output,
        out UIntPtr outputLength);

    [DllImport(
        LibraryName,
        EntryPoint = "iroha_privacy_authenticated_state_query_prepare_v1",
        CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeAuthenticatedPrivacyStateQueryPrepare(
        [In] byte[] networkId,
        UIntPtr networkIdLength,
        [In] byte[] authority,
        UIntPtr authorityLength,
        uint queryId,
        uint protocolIndex,
        [In] byte[] requestBinding,
        UIntPtr requestBindingLength,
        ulong creationTimeMs,
        [In] byte[] nonce,
        UIntPtr nonceLength,
        out IntPtr preparation,
        out UIntPtr preparationLength,
        out IntPtr signingDigest,
        out UIntPtr signingDigestLength);

    [DllImport(
        LibraryName,
        EntryPoint = "iroha_privacy_authenticated_state_query_finalize_v1",
        CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeAuthenticatedPrivacyStateQueryFinalize(
        [In] byte[] preparation,
        UIntPtr preparationLength,
        [In] byte[] signature,
        UIntPtr signatureLength,
        out IntPtr output,
        out UIntPtr outputLength);

    [DllImport(
        LibraryName,
        EntryPoint = "iroha_privacy_authenticated_state_query_project_result_v1",
        CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeAuthenticatedPrivacyStateQueryProjectResult(
        [In] byte[] preparation,
        UIntPtr preparationLength,
        [In] byte[] response,
        UIntPtr responseLength,
        out IntPtr output,
        out UIntPtr outputLength);

    [DllImport(
        LibraryName,
        EntryPoint = "iroha_privacy_free_buffer",
        CallingConvention = CallingConvention.Cdecl)]
    private static extern void NativeFree(IntPtr pointer);
}
