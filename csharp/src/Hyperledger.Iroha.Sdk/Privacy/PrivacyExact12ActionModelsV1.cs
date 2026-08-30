using System.Collections.ObjectModel;
using System.Text;

namespace Hyperledger.Iroha.Privacy;

/// <summary>Closed ledger-effect class committed by a public Exact12 operation.</summary>
public enum PrivacyLedgerEffectKindV1 : uint
{
    VerificationOnly = 0,
    ZkAceTransparentTransfer = 1,
    AnonymousPgcAccountStateTransition = 2,
    ZkAmsBatchAdmission = 3,
    ZkAmsProvisionAccount = 4,
    ZkX509CertificateNullifier = 5,
    OrchardNoteStateTransition = 6,
    FcmpMembershipPayment = 7,
    IvmPrivateNoteStateTransition = 8,
    PqMaspNoteStateTransition = 9,
}

/// <summary>Local lifecycle projection for one Exact12 action submission.</summary>
public enum PrivacyActionLocalStateV1
{
    Submitted,
    Terminal,
}

/// <summary>Authenticated terminal pipeline state for one Exact12 action submission.</summary>
public enum PrivacyActionTerminalChainStateV1
{
    Committed,
    Applied,
    Rejected,
    Expired,
}

/// <summary>Closed labels and operation-to-protocol/effect mappings for Exact12.</summary>
public static class PrivacyExact12ActionContractV1
{
    private static readonly IReadOnlyList<PrivacyOperationSchemaV1> OperationValues =
        new ReadOnlyCollection<PrivacyOperationSchemaV1>(
            Enum.GetValues<PrivacyOperationSchemaV1>());

    private static readonly IReadOnlyList<PrivacyLedgerEffectKindV1> LedgerEffectValues =
        new ReadOnlyCollection<PrivacyLedgerEffectKindV1>(
            Enum.GetValues<PrivacyLedgerEffectKindV1>());

    /// <summary>All thirteen public operations in their canonical native order.</summary>
    public static IReadOnlyList<PrivacyOperationSchemaV1> Operations => OperationValues;

    /// <summary>All ten typed ledger-effect kinds in their canonical native order.</summary>
    public static IReadOnlyList<PrivacyLedgerEffectKindV1> LedgerEffectKinds =>
        LedgerEffectValues;

    public static string CanonicalLabel(this PrivacyOperationSchemaV1 operation) =>
        operation switch
        {
            PrivacyOperationSchemaV1.ZkAceAuthorizationActionV1 =>
                "zk_ace_authorization_action_v1",
            PrivacyOperationSchemaV1.AnonymousPgcPaymentActionV1 =>
                "anonymous_pgc_payment_action_v1",
            PrivacyOperationSchemaV1.VeRangeRangeProofV1 =>
                "verange_range_proof_v1",
            PrivacyOperationSchemaV1.ZkAmsBatchAdmissionActionV1 =>
                "zk_ams_batch_admission_action_v1",
            PrivacyOperationSchemaV1.ZkAmsProvisionAccountActionV1 =>
                "zk_ams_provision_account_action_v1",
            PrivacyOperationSchemaV1.VegaCredentialPresentationV1 =>
                "vega_credential_presentation_v1",
            PrivacyOperationSchemaV1.ZkX509IdentityPresentationV1 =>
                "zk_x509_identity_presentation_v1",
            PrivacyOperationSchemaV1.JindoPolynomialEvaluationV1 =>
                "jindo_polynomial_evaluation_v1",
            PrivacyOperationSchemaV1.BootleLanternCredentialPresentationV1 =>
                "bootle_lantern_credential_presentation_v1",
            PrivacyOperationSchemaV1.OrchardNoteActionV1 =>
                "orchard_note_action_v1",
            PrivacyOperationSchemaV1.FcmpMembershipPaymentV1 =>
                "fcmp_membership_payment_v1",
            PrivacyOperationSchemaV1.IvmPrivateNoteActionV1 =>
                "ivm_private_note_action_v1",
            PrivacyOperationSchemaV1.PqMaspNoteActionV1 =>
                "pq_masp_note_action_v1",
            _ => throw new ArgumentOutOfRangeException(nameof(operation)),
        };

    public static PrivacyOperationSchemaV1 ParseOperationCanonicalLabel(string label)
    {
        ArgumentNullException.ThrowIfNull(label);
        return label switch
        {
            "zk_ace_authorization_action_v1" =>
                PrivacyOperationSchemaV1.ZkAceAuthorizationActionV1,
            "anonymous_pgc_payment_action_v1" =>
                PrivacyOperationSchemaV1.AnonymousPgcPaymentActionV1,
            "verange_range_proof_v1" =>
                PrivacyOperationSchemaV1.VeRangeRangeProofV1,
            "zk_ams_batch_admission_action_v1" =>
                PrivacyOperationSchemaV1.ZkAmsBatchAdmissionActionV1,
            "zk_ams_provision_account_action_v1" =>
                PrivacyOperationSchemaV1.ZkAmsProvisionAccountActionV1,
            "vega_credential_presentation_v1" =>
                PrivacyOperationSchemaV1.VegaCredentialPresentationV1,
            "zk_x509_identity_presentation_v1" =>
                PrivacyOperationSchemaV1.ZkX509IdentityPresentationV1,
            "jindo_polynomial_evaluation_v1" =>
                PrivacyOperationSchemaV1.JindoPolynomialEvaluationV1,
            "bootle_lantern_credential_presentation_v1" =>
                PrivacyOperationSchemaV1.BootleLanternCredentialPresentationV1,
            "orchard_note_action_v1" =>
                PrivacyOperationSchemaV1.OrchardNoteActionV1,
            "fcmp_membership_payment_v1" =>
                PrivacyOperationSchemaV1.FcmpMembershipPaymentV1,
            "ivm_private_note_action_v1" =>
                PrivacyOperationSchemaV1.IvmPrivateNoteActionV1,
            "pq_masp_note_action_v1" =>
                PrivacyOperationSchemaV1.PqMaspNoteActionV1,
            _ => throw new ArgumentException(
                "Unknown canonical Exact12 action operation.",
                nameof(label)),
        };
    }

    /// <summary>Return the sole retained protocol that executes this operation.</summary>
    public static PrivacyProtocolIdV1 ProtocolId(this PrivacyOperationSchemaV1 operation) =>
        operation switch
        {
            PrivacyOperationSchemaV1.ZkAceAuthorizationActionV1 =>
                PrivacyProtocolIdV1.ZkAcePqAuthorizationV0,
            PrivacyOperationSchemaV1.AnonymousPgcPaymentActionV1 =>
                PrivacyProtocolIdV1.AnonymousPgcKOutOfNV1,
            PrivacyOperationSchemaV1.VeRangeRangeProofV1 =>
                PrivacyProtocolIdV1.VeRangeTransparentRangeV1,
            PrivacyOperationSchemaV1.ZkAmsBatchAdmissionActionV1 or
            PrivacyOperationSchemaV1.ZkAmsProvisionAccountActionV1 =>
                PrivacyProtocolIdV1.IrohaZkAmsV1,
            PrivacyOperationSchemaV1.VegaCredentialPresentationV1 =>
                PrivacyProtocolIdV1.VegaExistingCredentialZkV0,
            PrivacyOperationSchemaV1.ZkX509IdentityPresentationV1 =>
                PrivacyProtocolIdV1.IrohaZkX509StarkP256V0,
            PrivacyOperationSchemaV1.JindoPolynomialEvaluationV1 =>
                PrivacyProtocolIdV1.IrohaJindoPolynomialCommitmentV0,
            PrivacyOperationSchemaV1.BootleLanternCredentialPresentationV1 =>
                PrivacyProtocolIdV1.IrohaBootleLanternAnoncredV1,
            PrivacyOperationSchemaV1.OrchardNoteActionV1 =>
                PrivacyProtocolIdV1.OrchardHalo2ActionsV1,
            PrivacyOperationSchemaV1.FcmpMembershipPaymentV1 =>
                PrivacyProtocolIdV1.MoneroFcmpPlusPlusV1,
            PrivacyOperationSchemaV1.IvmPrivateNoteActionV1 =>
                PrivacyProtocolIdV1.IrohaIvmPrivateNoteStarkV1,
            PrivacyOperationSchemaV1.PqMaspNoteActionV1 =>
                PrivacyProtocolIdV1.PqMaspStarkV0,
            _ => throw new ArgumentOutOfRangeException(nameof(operation)),
        };

    /// <summary>Return the typed ledger effect committed by this operation.</summary>
    public static PrivacyLedgerEffectKindV1 LedgerEffectKind(
        this PrivacyOperationSchemaV1 operation) =>
        operation switch
        {
            PrivacyOperationSchemaV1.ZkAceAuthorizationActionV1 =>
                PrivacyLedgerEffectKindV1.ZkAceTransparentTransfer,
            PrivacyOperationSchemaV1.AnonymousPgcPaymentActionV1 =>
                PrivacyLedgerEffectKindV1.AnonymousPgcAccountStateTransition,
            PrivacyOperationSchemaV1.VeRangeRangeProofV1 or
            PrivacyOperationSchemaV1.VegaCredentialPresentationV1 or
            PrivacyOperationSchemaV1.JindoPolynomialEvaluationV1 or
            PrivacyOperationSchemaV1.BootleLanternCredentialPresentationV1 =>
                PrivacyLedgerEffectKindV1.VerificationOnly,
            PrivacyOperationSchemaV1.ZkAmsBatchAdmissionActionV1 =>
                PrivacyLedgerEffectKindV1.ZkAmsBatchAdmission,
            PrivacyOperationSchemaV1.ZkAmsProvisionAccountActionV1 =>
                PrivacyLedgerEffectKindV1.ZkAmsProvisionAccount,
            PrivacyOperationSchemaV1.ZkX509IdentityPresentationV1 =>
                PrivacyLedgerEffectKindV1.ZkX509CertificateNullifier,
            PrivacyOperationSchemaV1.OrchardNoteActionV1 =>
                PrivacyLedgerEffectKindV1.OrchardNoteStateTransition,
            PrivacyOperationSchemaV1.FcmpMembershipPaymentV1 =>
                PrivacyLedgerEffectKindV1.FcmpMembershipPayment,
            PrivacyOperationSchemaV1.IvmPrivateNoteActionV1 =>
                PrivacyLedgerEffectKindV1.IvmPrivateNoteStateTransition,
            PrivacyOperationSchemaV1.PqMaspNoteActionV1 =>
                PrivacyLedgerEffectKindV1.PqMaspNoteStateTransition,
            _ => throw new ArgumentOutOfRangeException(nameof(operation)),
        };

    public static string CanonicalLabel(this PrivacyLedgerEffectKindV1 effect) =>
        effect switch
        {
            PrivacyLedgerEffectKindV1.VerificationOnly => "verification_only",
            PrivacyLedgerEffectKindV1.ZkAceTransparentTransfer =>
                "zk_ace_transparent_transfer",
            PrivacyLedgerEffectKindV1.AnonymousPgcAccountStateTransition =>
                "anonymous_pgc_account_state_transition",
            PrivacyLedgerEffectKindV1.ZkAmsBatchAdmission => "zk_ams_batch_admission",
            PrivacyLedgerEffectKindV1.ZkAmsProvisionAccount => "zk_ams_provision_account",
            PrivacyLedgerEffectKindV1.ZkX509CertificateNullifier =>
                "zk_x509_certificate_nullifier",
            PrivacyLedgerEffectKindV1.OrchardNoteStateTransition =>
                "orchard_note_state_transition",
            PrivacyLedgerEffectKindV1.FcmpMembershipPayment => "fcmp_membership_payment",
            PrivacyLedgerEffectKindV1.IvmPrivateNoteStateTransition =>
                "ivm_private_note_state_transition",
            PrivacyLedgerEffectKindV1.PqMaspNoteStateTransition =>
                "pq_masp_note_state_transition",
            _ => throw new ArgumentOutOfRangeException(nameof(effect)),
        };

    public static PrivacyLedgerEffectKindV1 ParseLedgerEffectCanonicalLabel(string label)
    {
        ArgumentNullException.ThrowIfNull(label);
        return label switch
        {
            "verification_only" => PrivacyLedgerEffectKindV1.VerificationOnly,
            "zk_ace_transparent_transfer" =>
                PrivacyLedgerEffectKindV1.ZkAceTransparentTransfer,
            "anonymous_pgc_account_state_transition" =>
                PrivacyLedgerEffectKindV1.AnonymousPgcAccountStateTransition,
            "zk_ams_batch_admission" => PrivacyLedgerEffectKindV1.ZkAmsBatchAdmission,
            "zk_ams_provision_account" => PrivacyLedgerEffectKindV1.ZkAmsProvisionAccount,
            "zk_x509_certificate_nullifier" =>
                PrivacyLedgerEffectKindV1.ZkX509CertificateNullifier,
            "orchard_note_state_transition" =>
                PrivacyLedgerEffectKindV1.OrchardNoteStateTransition,
            "fcmp_membership_payment" => PrivacyLedgerEffectKindV1.FcmpMembershipPayment,
            "ivm_private_note_state_transition" =>
                PrivacyLedgerEffectKindV1.IvmPrivateNoteStateTransition,
            "pq_masp_note_state_transition" =>
                PrivacyLedgerEffectKindV1.PqMaspNoteStateTransition,
            _ => throw new ArgumentException(
                "Unknown canonical Exact12 ledger-effect kind.",
                nameof(label)),
        };
    }

    public static string CanonicalLabel(this PrivacyActionLocalStateV1 state) =>
        state switch
        {
            PrivacyActionLocalStateV1.Submitted => "submitted",
            PrivacyActionLocalStateV1.Terminal => "terminal",
            _ => throw new ArgumentOutOfRangeException(nameof(state)),
        };

    public static string CanonicalLabel(this PrivacyActionTerminalChainStateV1 state) =>
        state switch
        {
            PrivacyActionTerminalChainStateV1.Committed => "Committed",
            PrivacyActionTerminalChainStateV1.Applied => "Applied",
            PrivacyActionTerminalChainStateV1.Rejected => "Rejected",
            PrivacyActionTerminalChainStateV1.Expired => "Expired",
            _ => throw new ArgumentOutOfRangeException(nameof(state)),
        };
}

/// <summary>
/// One closed Exact12 operation and its already-signed versioned transaction wire.
/// </summary>
/// <remarks>
/// The model snapshots and bounds public wire bytes. It performs no local proof acceptance and
/// grants no capability or submission authority.
/// </remarks>
public sealed class PrivacyExact12ActionRequestV1 : IEquatable<PrivacyExact12ActionRequestV1>
{
    /// <summary>Taira V1 <c>max_tx_bytes</c>, shared with native action inspection.</summary>
    public const int MaxSignedTransactionBytes = 10 * 1024 * 1024;

    private readonly byte[] signedTransactionVersioned;
    private readonly byte[]? expectedManifestDigest;

    public PrivacyExact12ActionRequestV1(
        PrivacyOperationSchemaV1 operation,
        byte[] signedTransactionVersioned,
        byte[]? expectedManifestDigest = null)
    {
        _ = operation.ProtocolId();
        ArgumentNullException.ThrowIfNull(signedTransactionVersioned);
        if (signedTransactionVersioned.Length is < 1 or > MaxSignedTransactionBytes)
        {
            throw new ArgumentOutOfRangeException(
                nameof(signedTransactionVersioned),
                $"Exact12 signed transaction must contain 1..{MaxSignedTransactionBytes} bytes.");
        }
        if (expectedManifestDigest is not null)
        {
            RequireNonzeroFixed32(expectedManifestDigest, nameof(expectedManifestDigest));
        }

        Operation = operation;
        this.signedTransactionVersioned = (byte[])signedTransactionVersioned.Clone();
        this.expectedManifestDigest = expectedManifestDigest is null
            ? null
            : (byte[])expectedManifestDigest.Clone();
    }

    public PrivacyOperationSchemaV1 Operation { get; }

    public byte[] SignedTransactionVersioned => (byte[])signedTransactionVersioned.Clone();

    public byte[]? ExpectedManifestDigest => expectedManifestDigest is null
        ? null
        : (byte[])expectedManifestDigest.Clone();

    public bool Equals(PrivacyExact12ActionRequestV1? other) =>
        other is not null
        && Operation == other.Operation
        && signedTransactionVersioned.AsSpan().SequenceEqual(other.signedTransactionVersioned)
        && NullableBytesEqual(expectedManifestDigest, other.expectedManifestDigest);

    public override bool Equals(object? obj) =>
        obj is PrivacyExact12ActionRequestV1 other && Equals(other);

    public override int GetHashCode()
    {
        var hash = new HashCode();
        hash.Add(Operation);
        AddBytes(ref hash, signedTransactionVersioned);
        if (expectedManifestDigest is not null)
        {
            AddBytes(ref hash, expectedManifestDigest);
        }
        return hash.ToHashCode();
    }

    internal static void RequireNonzeroFixed32(byte[] value, string parameterName)
    {
        ArgumentNullException.ThrowIfNull(value, parameterName);
        if (value.Length != 32 || !Array.Exists(value, static item => item != 0))
        {
            throw new ArgumentException(
                "Exact12 hash or digest must contain exactly 32 non-zero bytes.",
                parameterName);
        }
    }

    private static bool NullableBytesEqual(byte[]? left, byte[]? right) =>
        left is null
            ? right is null
            : right is not null && left.AsSpan().SequenceEqual(right);

    internal static void AddBytes(ref HashCode hash, byte[] bytes)
    {
        foreach (var value in bytes)
        {
            hash.Add(value);
        }
    }
}

/// <summary>
/// Validated immutable state of one Exact12 action. Public construction creates a detached
/// display view; authenticated status queries accept only views returned by submission.
/// </summary>
public sealed class PrivacyActionOperationViewV1 : IEquatable<PrivacyActionOperationViewV1>
{
    private readonly byte[] transactionHash;
    private readonly byte[] transactionIntentDigest;
    private readonly byte[] statementDigest;
    private readonly byte[] proofEnvelopeHash;
    private readonly byte[] capabilityManifestDigest;
    private readonly byte[]? executionCapabilityManifestDigest;
    private readonly byte[]? executionReceiptFinalizedBlockHash;
    private AuthenticatedProvenanceV1? authenticatedProvenance;

    public PrivacyActionOperationViewV1(
        PrivacyProtocolIdV1 protocolId,
        PrivacyOperationSchemaV1 operationSchema,
        byte[] transactionHash,
        byte[] transactionIntentDigest,
        byte[] statementDigest,
        byte[] proofEnvelopeHash,
        PrivacyActionLocalStateV1 localState,
        PrivacyActionTerminalChainStateV1? terminalChainState,
        ulong? committedHeight,
        string? rejectionReason,
        PrivacyLedgerEffectKindV1 ledgerEffectKind,
        byte[] capabilityManifestDigest,
        ulong capabilityCommittedHeight,
        byte[]? executionCapabilityManifestDigest = null,
        ulong? executionCapabilityCommittedHeight = null,
        ulong? executionReceiptFinalizedHeight = null,
        byte[]? executionReceiptFinalizedBlockHash = null)
    {
        if (protocolId != operationSchema.ProtocolId())
        {
            throw new ArgumentException(
                "Exact12 operation does not belong to the supplied protocol.",
                nameof(protocolId));
        }
        if (ledgerEffectKind != operationSchema.LedgerEffectKind())
        {
            throw new ArgumentException(
                "Exact12 operation does not produce the supplied ledger-effect kind.",
                nameof(ledgerEffectKind));
        }
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
        PrivacyExact12ActionRequestV1.RequireNonzeroFixed32(
            capabilityManifestDigest,
            nameof(capabilityManifestDigest));
        if (executionCapabilityManifestDigest is not null)
        {
            PrivacyExact12ActionRequestV1.RequireNonzeroFixed32(
                executionCapabilityManifestDigest,
                nameof(executionCapabilityManifestDigest));
        }
        if (executionReceiptFinalizedBlockHash is not null)
        {
            PrivacyExact12ActionRequestV1.RequireNonzeroFixed32(
                executionReceiptFinalizedBlockHash,
                nameof(executionReceiptFinalizedBlockHash));
        }
        if (capabilityCommittedHeight == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(capabilityCommittedHeight));
        }
        if (committedHeight == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(committedHeight));
        }
        if (executionCapabilityCommittedHeight == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(executionCapabilityCommittedHeight));
        }
        if (executionReceiptFinalizedHeight == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(executionReceiptFinalizedHeight));
        }

        switch (localState)
        {
            case PrivacyActionLocalStateV1.Submitted:
                if (terminalChainState is not null
                    || committedHeight is not null
                    || rejectionReason is not null
                    || HasAnyExecutionReceiptEvidence(
                        executionCapabilityManifestDigest,
                        executionCapabilityCommittedHeight,
                        executionReceiptFinalizedHeight,
                        executionReceiptFinalizedBlockHash))
                {
                    throw new ArgumentException(
                        "Submitted Exact12 actions cannot carry terminal or execution-receipt fields.",
                        nameof(localState));
                }
                break;
            case PrivacyActionLocalStateV1.Terminal:
                ValidateTerminalState(
                    terminalChainState,
                    committedHeight,
                    rejectionReason,
                    capabilityCommittedHeight,
                    executionCapabilityManifestDigest,
                    executionCapabilityCommittedHeight,
                    executionReceiptFinalizedHeight,
                    executionReceiptFinalizedBlockHash);
                break;
            default:
                throw new ArgumentOutOfRangeException(nameof(localState));
        }

        ProtocolId = protocolId;
        OperationSchema = operationSchema;
        this.transactionHash = (byte[])transactionHash.Clone();
        this.transactionIntentDigest = (byte[])transactionIntentDigest.Clone();
        this.statementDigest = (byte[])statementDigest.Clone();
        this.proofEnvelopeHash = (byte[])proofEnvelopeHash.Clone();
        LocalState = localState;
        TerminalChainState = terminalChainState;
        CommittedHeight = committedHeight;
        RejectionReason = rejectionReason;
        LedgerEffectKind = ledgerEffectKind;
        this.capabilityManifestDigest = (byte[])capabilityManifestDigest.Clone();
        CapabilityCommittedHeight = capabilityCommittedHeight;
        this.executionCapabilityManifestDigest =
            executionCapabilityManifestDigest is null
                ? null
                : (byte[])executionCapabilityManifestDigest.Clone();
        ExecutionCapabilityCommittedHeight = executionCapabilityCommittedHeight;
        ExecutionReceiptFinalizedHeight = executionReceiptFinalizedHeight;
        this.executionReceiptFinalizedBlockHash =
            executionReceiptFinalizedBlockHash is null
                ? null
                : (byte[])executionReceiptFinalizedBlockHash.Clone();
    }

    public PrivacyProtocolIdV1 ProtocolId { get; }

    public PrivacyOperationSchemaV1 OperationSchema { get; }

    public byte[] TransactionHash => (byte[])transactionHash.Clone();

    public byte[] TransactionIntentDigest => (byte[])transactionIntentDigest.Clone();

    public byte[] StatementDigest => (byte[])statementDigest.Clone();

    public byte[] ProofEnvelopeHash => (byte[])proofEnvelopeHash.Clone();

    public PrivacyActionLocalStateV1 LocalState { get; }

    public PrivacyActionTerminalChainStateV1? TerminalChainState { get; }

    public ulong? CommittedHeight { get; }

    public string? RejectionReason { get; }

    public PrivacyLedgerEffectKindV1 LedgerEffectKind { get; }

    public byte[] CapabilityManifestDigest => (byte[])capabilityManifestDigest.Clone();

    public ulong CapabilityCommittedHeight { get; }

    /// <summary>
    /// Capability-manifest digest recorded by native execution. This is distinct from the
    /// pre-submit admission digest and is present only for Applied actions.
    /// </summary>
    public byte[]? ExecutionCapabilityManifestDigest =>
        executionCapabilityManifestDigest is null
            ? null
            : (byte[])executionCapabilityManifestDigest.Clone();

    /// <summary>Finalized capability height recorded by native execution.</summary>
    public ulong? ExecutionCapabilityCommittedHeight { get; }

    /// <summary>Height of the finalized block binding the native execution receipt.</summary>
    public ulong? ExecutionReceiptFinalizedHeight { get; }

    /// <summary>Hash of the finalized block binding the native execution receipt.</summary>
    public byte[]? ExecutionReceiptFinalizedBlockHash =>
        executionReceiptFinalizedBlockHash is null
            ? null
            : (byte[])executionReceiptFinalizedBlockHash.Clone();

    internal PrivacyActionOperationViewV1 BindAuthenticatedSubmissionV1(
        object owner,
        global::Hyperledger.Iroha.NetworkId networkId)
    {
        ArgumentNullException.ThrowIfNull(owner);
        ArgumentNullException.ThrowIfNull(networkId);
        if (LocalState != PrivacyActionLocalStateV1.Submitted
            || TerminalChainState is not null)
        {
            throw new InvalidOperationException(
                "Only a submitted Exact12 action can receive authenticated provenance.");
        }
        if (authenticatedProvenance is not null)
        {
            throw new InvalidOperationException(
                "Exact12 action already carries authenticated provenance.");
        }
        authenticatedProvenance = new AuthenticatedProvenanceV1(owner, networkId, this);
        return this;
    }

    internal void RequireAuthenticatedProvenanceV1(
        object owner,
        global::Hyperledger.Iroha.NetworkId networkId)
    {
        ArgumentNullException.ThrowIfNull(owner);
        ArgumentNullException.ThrowIfNull(networkId);
        if (authenticatedProvenance?.Matches(owner, networkId, this) != true)
        {
            throw new InvalidOperationException(
                "Exact12 status requires a view returned by this client's authenticated submission.");
        }
    }

    internal PrivacyActionOperationViewV1 WithAuthenticatedTerminalStateV1(
        PrivacyActionTerminalChainStateV1 terminalState,
        ulong? committedHeight,
        string? rejectionReason,
        byte[]? executionCapabilityManifestDigest = null,
        ulong? executionCapabilityCommittedHeight = null,
        ulong? executionReceiptFinalizedHeight = null,
        byte[]? executionReceiptFinalizedBlockHash = null)
    {
        if (authenticatedProvenance is null)
        {
            throw new InvalidOperationException(
                "Detached Exact12 views cannot receive authenticated terminal state.");
        }
        var terminal = new PrivacyActionOperationViewV1(
            ProtocolId,
            OperationSchema,
            transactionHash,
            transactionIntentDigest,
            statementDigest,
            proofEnvelopeHash,
            PrivacyActionLocalStateV1.Terminal,
            terminalState,
            committedHeight,
            rejectionReason,
            LedgerEffectKind,
            capabilityManifestDigest,
            CapabilityCommittedHeight,
            executionCapabilityManifestDigest,
            executionCapabilityCommittedHeight,
            executionReceiptFinalizedHeight,
            executionReceiptFinalizedBlockHash);
        terminal.authenticatedProvenance = authenticatedProvenance;
        return terminal;
    }

    public bool Equals(PrivacyActionOperationViewV1? other) =>
        other is not null
        && ProtocolId == other.ProtocolId
        && OperationSchema == other.OperationSchema
        && transactionHash.AsSpan().SequenceEqual(other.transactionHash)
        && transactionIntentDigest.AsSpan().SequenceEqual(other.transactionIntentDigest)
        && statementDigest.AsSpan().SequenceEqual(other.statementDigest)
        && proofEnvelopeHash.AsSpan().SequenceEqual(other.proofEnvelopeHash)
        && LocalState == other.LocalState
        && TerminalChainState == other.TerminalChainState
        && CommittedHeight == other.CommittedHeight
        && RejectionReason == other.RejectionReason
        && LedgerEffectKind == other.LedgerEffectKind
        && capabilityManifestDigest.AsSpan().SequenceEqual(other.capabilityManifestDigest)
        && CapabilityCommittedHeight == other.CapabilityCommittedHeight
        && NullableBytesEqual(
            executionCapabilityManifestDigest,
            other.executionCapabilityManifestDigest)
        && ExecutionCapabilityCommittedHeight == other.ExecutionCapabilityCommittedHeight
        && ExecutionReceiptFinalizedHeight == other.ExecutionReceiptFinalizedHeight
        && NullableBytesEqual(
            executionReceiptFinalizedBlockHash,
            other.executionReceiptFinalizedBlockHash);

    public override bool Equals(object? obj) =>
        obj is PrivacyActionOperationViewV1 other && Equals(other);

    public override int GetHashCode()
    {
        var hash = new HashCode();
        hash.Add(ProtocolId);
        hash.Add(OperationSchema);
        PrivacyExact12ActionRequestV1.AddBytes(ref hash, transactionHash);
        PrivacyExact12ActionRequestV1.AddBytes(ref hash, transactionIntentDigest);
        PrivacyExact12ActionRequestV1.AddBytes(ref hash, statementDigest);
        PrivacyExact12ActionRequestV1.AddBytes(ref hash, proofEnvelopeHash);
        hash.Add(LocalState);
        hash.Add(TerminalChainState);
        hash.Add(CommittedHeight);
        hash.Add(RejectionReason, StringComparer.Ordinal);
        hash.Add(LedgerEffectKind);
        PrivacyExact12ActionRequestV1.AddBytes(ref hash, capabilityManifestDigest);
        hash.Add(CapabilityCommittedHeight);
        hash.Add(executionCapabilityManifestDigest is not null);
        if (executionCapabilityManifestDigest is not null)
        {
            PrivacyExact12ActionRequestV1.AddBytes(
                ref hash,
                executionCapabilityManifestDigest);
        }
        hash.Add(ExecutionCapabilityCommittedHeight);
        hash.Add(ExecutionReceiptFinalizedHeight);
        hash.Add(executionReceiptFinalizedBlockHash is not null);
        if (executionReceiptFinalizedBlockHash is not null)
        {
            PrivacyExact12ActionRequestV1.AddBytes(
                ref hash,
                executionReceiptFinalizedBlockHash);
        }
        return hash.ToHashCode();
    }

    private sealed class AuthenticatedProvenanceV1
    {
        private readonly object owner;
        private readonly global::Hyperledger.Iroha.NetworkId networkId;
        private readonly PrivacyProtocolIdV1 protocolId;
        private readonly PrivacyOperationSchemaV1 operationSchema;
        private readonly byte[] transactionHash;
        private readonly byte[] transactionIntentDigest;
        private readonly byte[] statementDigest;
        private readonly byte[] proofEnvelopeHash;
        private readonly PrivacyLedgerEffectKindV1 ledgerEffectKind;
        private readonly byte[] capabilityManifestDigest;
        private readonly ulong capabilityCommittedHeight;

        internal AuthenticatedProvenanceV1(
            object owner,
            global::Hyperledger.Iroha.NetworkId networkId,
            PrivacyActionOperationViewV1 view)
        {
            this.owner = owner;
            this.networkId = networkId;
            protocolId = view.ProtocolId;
            operationSchema = view.OperationSchema;
            transactionHash = (byte[])view.transactionHash.Clone();
            transactionIntentDigest = (byte[])view.transactionIntentDigest.Clone();
            statementDigest = (byte[])view.statementDigest.Clone();
            proofEnvelopeHash = (byte[])view.proofEnvelopeHash.Clone();
            ledgerEffectKind = view.LedgerEffectKind;
            capabilityManifestDigest = (byte[])view.capabilityManifestDigest.Clone();
            capabilityCommittedHeight = view.CapabilityCommittedHeight;
        }

        internal bool Matches(
            object expectedOwner,
            global::Hyperledger.Iroha.NetworkId expectedNetworkId,
            PrivacyActionOperationViewV1 view) =>
            ReferenceEquals(owner, expectedOwner)
            && networkId == expectedNetworkId
            && protocolId == view.ProtocolId
            && operationSchema == view.OperationSchema
            && transactionHash.AsSpan().SequenceEqual(view.transactionHash)
            && transactionIntentDigest.AsSpan().SequenceEqual(view.transactionIntentDigest)
            && statementDigest.AsSpan().SequenceEqual(view.statementDigest)
            && proofEnvelopeHash.AsSpan().SequenceEqual(view.proofEnvelopeHash)
            && ledgerEffectKind == view.LedgerEffectKind
            && capabilityManifestDigest.AsSpan().SequenceEqual(view.capabilityManifestDigest)
            && capabilityCommittedHeight == view.CapabilityCommittedHeight;
    }

    private static void ValidateTerminalState(
        PrivacyActionTerminalChainStateV1? terminalChainState,
        ulong? committedHeight,
        string? rejectionReason,
        ulong capabilityCommittedHeight,
        byte[]? executionCapabilityManifestDigest,
        ulong? executionCapabilityCommittedHeight,
        ulong? executionReceiptFinalizedHeight,
        byte[]? executionReceiptFinalizedBlockHash)
    {
        var hasExecutionReceiptEvidence = HasAnyExecutionReceiptEvidence(
            executionCapabilityManifestDigest,
            executionCapabilityCommittedHeight,
            executionReceiptFinalizedHeight,
            executionReceiptFinalizedBlockHash);
        if (committedHeight is { } terminalCommittedHeight
            && terminalCommittedHeight < capabilityCommittedHeight)
        {
            throw new ArgumentException(
                "Authenticated terminal height cannot predate pre-submit capability admission.",
                nameof(committedHeight));
        }
        switch (terminalChainState)
        {
            case PrivacyActionTerminalChainStateV1.Committed:
                if (committedHeight is null
                    || rejectionReason is not null
                    || hasExecutionReceiptEvidence)
                {
                    throw new ArgumentException(
                        "Legacy Committed Exact12 views require only an authenticated committed height.",
                        nameof(terminalChainState));
                }
                return;
            case PrivacyActionTerminalChainStateV1.Applied:
                if (committedHeight is null
                    || rejectionReason is not null
                    || executionCapabilityManifestDigest is null
                    || executionCapabilityCommittedHeight is null
                    || executionReceiptFinalizedHeight is null
                    || executionReceiptFinalizedBlockHash is null)
                {
                    throw new ArgumentException(
                        "Applied Exact12 actions require complete authenticated execution-receipt evidence.",
                        nameof(terminalChainState));
                }
                if (executionCapabilityCommittedHeight.Value > committedHeight.Value
                    || executionReceiptFinalizedHeight.Value < committedHeight.Value)
                {
                    throw new ArgumentException(
                        "Applied Exact12 execution-receipt heights contradict the committed action height.",
                        nameof(terminalChainState));
                }
                return;
            case PrivacyActionTerminalChainStateV1.Rejected:
                if (committedHeight is null || hasExecutionReceiptEvidence)
                {
                    throw new ArgumentException(
                        "Rejected Exact12 actions require a committed height and no execution receipt.",
                        nameof(terminalChainState));
                }
                if (string.IsNullOrEmpty(rejectionReason)
                    || Encoding.UTF8.GetByteCount(rejectionReason) > 1_024
                    || !string.Equals(rejectionReason, rejectionReason.Trim(), StringComparison.Ordinal)
                    || rejectionReason.Any(char.IsControl))
                {
                    throw new ArgumentException(
                        "Rejected Exact12 actions require one canonical non-empty reason.",
                        nameof(rejectionReason));
                }
                return;
            case PrivacyActionTerminalChainStateV1.Expired:
                if (committedHeight is not null
                    || rejectionReason is not null
                    || hasExecutionReceiptEvidence)
                {
                    throw new ArgumentException(
                        "Expired Exact12 actions cannot carry committed or execution-receipt fields.",
                        nameof(terminalChainState));
                }
                return;
            case null:
                throw new ArgumentException(
                    "Terminal Exact12 actions require a terminal chain state.",
                    nameof(terminalChainState));
            default:
                throw new ArgumentOutOfRangeException(nameof(terminalChainState));
        }
    }

    private static bool HasAnyExecutionReceiptEvidence(
        byte[]? executionCapabilityManifestDigest,
        ulong? executionCapabilityCommittedHeight,
        ulong? executionReceiptFinalizedHeight,
        byte[]? executionReceiptFinalizedBlockHash) =>
        executionCapabilityManifestDigest is not null
        || executionCapabilityCommittedHeight is not null
        || executionReceiptFinalizedHeight is not null
        || executionReceiptFinalizedBlockHash is not null;

    private static bool NullableBytesEqual(byte[]? left, byte[]? right) =>
        left is null
            ? right is null
            : right is not null && left.AsSpan().SequenceEqual(right);
}
