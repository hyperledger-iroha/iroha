using Hyperledger.Iroha.Privacy;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class PrivacyExact12ActionModelsV1Tests
{
    private static readonly PrivacyProtocolIdV1[] Protocols =
    [
        PrivacyProtocolIdV1.ZkAcePqAuthorizationV0,
        PrivacyProtocolIdV1.AnonymousPgcKOutOfNV1,
        PrivacyProtocolIdV1.VeRangeTransparentRangeV1,
        PrivacyProtocolIdV1.IrohaZkAmsV1,
        PrivacyProtocolIdV1.IrohaZkAmsV1,
        PrivacyProtocolIdV1.VegaExistingCredentialZkV0,
        PrivacyProtocolIdV1.IrohaZkX509StarkP256V0,
        PrivacyProtocolIdV1.IrohaJindoPolynomialCommitmentV0,
        PrivacyProtocolIdV1.IrohaBootleLanternAnoncredV1,
        PrivacyProtocolIdV1.OrchardHalo2ActionsV1,
        PrivacyProtocolIdV1.MoneroFcmpPlusPlusV1,
        PrivacyProtocolIdV1.IrohaIvmPrivateNoteStarkV1,
        PrivacyProtocolIdV1.PqMaspStarkV0,
    ];

    private static readonly PrivacyLedgerEffectKindV1[] Effects =
    [
        PrivacyLedgerEffectKindV1.ZkAceTransparentTransfer,
        PrivacyLedgerEffectKindV1.AnonymousPgcAccountStateTransition,
        PrivacyLedgerEffectKindV1.VerificationOnly,
        PrivacyLedgerEffectKindV1.ZkAmsBatchAdmission,
        PrivacyLedgerEffectKindV1.ZkAmsProvisionAccount,
        PrivacyLedgerEffectKindV1.VerificationOnly,
        PrivacyLedgerEffectKindV1.ZkX509CertificateNullifier,
        PrivacyLedgerEffectKindV1.VerificationOnly,
        PrivacyLedgerEffectKindV1.VerificationOnly,
        PrivacyLedgerEffectKindV1.OrchardNoteStateTransition,
        PrivacyLedgerEffectKindV1.FcmpMembershipPayment,
        PrivacyLedgerEffectKindV1.IvmPrivateNoteStateTransition,
        PrivacyLedgerEffectKindV1.PqMaspNoteStateTransition,
    ];

    [Fact]
    public void OperationVocabularyAndMappingsAreClosed()
    {
        Assert.Equal(13, PrivacyExact12ActionContractV1.Operations.Count);
        Assert.Equal(10, PrivacyExact12ActionContractV1.LedgerEffectKinds.Count);
        Assert.Equal(
            Protocols,
            PrivacyExact12ActionContractV1.Operations.Select(operation => operation.ProtocolId()));
        Assert.Equal(
            Effects,
            PrivacyExact12ActionContractV1.Operations.Select(operation => operation.LedgerEffectKind()));
        Assert.True(
            PrivacyExact12ActionContractV1.LedgerEffectKinds
                .ToHashSet()
                .SetEquals(Effects));
        Assert.Equal(
            "zk_ams_batch_admission_action_v1",
            PrivacyOperationSchemaV1.ZkAmsBatchAdmissionActionV1.CanonicalLabel());
        Assert.Equal(
            PrivacyOperationSchemaV1.ZkAmsProvisionAccountActionV1,
            PrivacyExact12ActionContractV1.ParseOperationCanonicalLabel(
                "zk_ams_provision_account_action_v1"));
        Assert.Throws<ArgumentException>(() =>
            PrivacyExact12ActionContractV1.ParseOperationCanonicalLabel(
                "zk_ams_admission_and_provisioning_v1"));
    }

    [Fact]
    public void RequestsBoundAndSnapshotWireAndOptionalManifestDigest()
    {
        var wire = new byte[] { 1, 2 };
        var digest = Fixed32(0x21);
        var request = new PrivacyExact12ActionRequestV1(
            PrivacyOperationSchemaV1.ZkAmsProvisionAccountActionV1,
            wire,
            digest);
        wire[0] = 0xff;
        digest[0] = 0xff;
        Assert.Equal(new byte[] { 1, 2 }, request.SignedTransactionVersioned);
        Assert.Equal(Fixed32(0x21), request.ExpectedManifestDigest);
        var leaked = request.SignedTransactionVersioned;
        leaked[0] = 0xee;
        Assert.Equal(1, request.SignedTransactionVersioned[0]);

        _ = new PrivacyExact12ActionRequestV1(
            PrivacyOperationSchemaV1.VeRangeRangeProofV1,
            new byte[PrivacyExact12ActionRequestV1.MaxSignedTransactionBytes]);
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            new PrivacyExact12ActionRequestV1(
                PrivacyOperationSchemaV1.VeRangeRangeProofV1,
                []));
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            new PrivacyExact12ActionRequestV1(
                PrivacyOperationSchemaV1.VeRangeRangeProofV1,
                new byte[PrivacyExact12ActionRequestV1.MaxSignedTransactionBytes + 1]));
        Assert.Throws<ArgumentException>(() =>
            new PrivacyExact12ActionRequestV1(
                PrivacyOperationSchemaV1.VeRangeRangeProofV1,
                [1],
                new byte[32]));
        Assert.Throws<ArgumentException>(() =>
            new PrivacyExact12ActionRequestV1(
                PrivacyOperationSchemaV1.VeRangeRangeProofV1,
                [1],
                Enumerable.Repeat((byte)1, 31).ToArray()));
    }

    [Fact]
    public void ViewsAcceptOnlyAuthenticatedLifecycleShapes()
    {
        Assert.Equal(PrivacyActionLocalStateV1.Submitted, View().LocalState);
        var committed = View(
            localState: PrivacyActionLocalStateV1.Terminal,
            terminalChainState: PrivacyActionTerminalChainStateV1.Committed,
            committedHeight: 42);
        Assert.Equal(42UL, committed.CommittedHeight);
        Assert.Null(committed.ExecutionCapabilityManifestDigest);

        var applied = View(
            localState: PrivacyActionLocalStateV1.Terminal,
            terminalChainState: PrivacyActionTerminalChainStateV1.Applied,
            committedHeight: 42,
            executionCapabilityManifestDigest: Fixed32(6),
            executionCapabilityCommittedHeight: 40,
            executionReceiptFinalizedHeight: 44,
            executionReceiptFinalizedBlockHash: Fixed32(7));
        Assert.Equal(42UL, applied.CommittedHeight);
        Assert.Equal(Fixed32(6), applied.ExecutionCapabilityManifestDigest);
        Assert.Equal(40UL, applied.ExecutionCapabilityCommittedHeight);
        Assert.Equal(44UL, applied.ExecutionReceiptFinalizedHeight);
        Assert.Equal(Fixed32(7), applied.ExecutionReceiptFinalizedBlockHash);

        var rejected = View(
            localState: PrivacyActionLocalStateV1.Terminal,
            terminalChainState: PrivacyActionTerminalChainStateV1.Rejected,
            committedHeight: 43,
            rejectionReason: "proof envelope expired");
        Assert.Equal("proof envelope expired", rejected.RejectionReason);
        var expired = View(
            localState: PrivacyActionLocalStateV1.Terminal,
            terminalChainState: PrivacyActionTerminalChainStateV1.Expired);
        Assert.Null(expired.CommittedHeight);

        var hostile = new Action[]
        {
            () => View(terminalChainState: PrivacyActionTerminalChainStateV1.Committed),
            () => View(committedHeight: 1),
            () => View(localState: PrivacyActionLocalStateV1.Terminal),
            () => View(
                localState: PrivacyActionLocalStateV1.Terminal,
                terminalChainState: PrivacyActionTerminalChainStateV1.Committed),
            () => View(
                localState: PrivacyActionLocalStateV1.Terminal,
                terminalChainState: PrivacyActionTerminalChainStateV1.Applied,
                committedHeight: 42,
                rejectionReason: "unexpected"),
            () => View(
                localState: PrivacyActionLocalStateV1.Terminal,
                terminalChainState: PrivacyActionTerminalChainStateV1.Applied,
                committedHeight: 42),
            () => View(
                localState: PrivacyActionLocalStateV1.Terminal,
                terminalChainState: PrivacyActionTerminalChainStateV1.Applied,
                committedHeight: 42,
                executionCapabilityManifestDigest: Fixed32(6)),
            () => View(
                executionCapabilityManifestDigest: Fixed32(6),
                executionCapabilityCommittedHeight: 10,
                executionReceiptFinalizedHeight: 42,
                executionReceiptFinalizedBlockHash: Fixed32(7)),
            () => View(
                localState: PrivacyActionLocalStateV1.Terminal,
                terminalChainState: PrivacyActionTerminalChainStateV1.Committed,
                committedHeight: 42,
                executionCapabilityManifestDigest: Fixed32(6),
                executionCapabilityCommittedHeight: 10,
                executionReceiptFinalizedHeight: 42,
                executionReceiptFinalizedBlockHash: Fixed32(7)),
            () => View(
                localState: PrivacyActionLocalStateV1.Terminal,
                terminalChainState: PrivacyActionTerminalChainStateV1.Rejected,
                rejectionReason: "rejected"),
            () => View(
                localState: PrivacyActionLocalStateV1.Terminal,
                terminalChainState: PrivacyActionTerminalChainStateV1.Rejected,
                committedHeight: 1,
                rejectionReason: " rejected "),
            () => View(
                localState: PrivacyActionLocalStateV1.Terminal,
                terminalChainState: PrivacyActionTerminalChainStateV1.Rejected,
                committedHeight: 1,
                rejectionReason: "policy\u0001rejected"),
            () => View(
                localState: PrivacyActionLocalStateV1.Terminal,
                terminalChainState: PrivacyActionTerminalChainStateV1.Rejected,
                committedHeight: 1,
                rejectionReason: new string('é', 513)),
            () => View(
                localState: PrivacyActionLocalStateV1.Terminal,
                terminalChainState: PrivacyActionTerminalChainStateV1.Expired,
                committedHeight: 1),
            () => View(
                localState: PrivacyActionLocalStateV1.Terminal,
                terminalChainState: PrivacyActionTerminalChainStateV1.Applied,
                committedHeight: 42,
                executionCapabilityManifestDigest: Fixed32(6),
                executionCapabilityCommittedHeight: 43,
                executionReceiptFinalizedHeight: 44,
                executionReceiptFinalizedBlockHash: Fixed32(7)),
            () => View(
                localState: PrivacyActionLocalStateV1.Terminal,
                terminalChainState: PrivacyActionTerminalChainStateV1.Applied,
                committedHeight: 42,
                executionCapabilityManifestDigest: Fixed32(6),
                executionCapabilityCommittedHeight: 40,
                executionReceiptFinalizedHeight: 41,
                executionReceiptFinalizedBlockHash: Fixed32(7)),
        };
        Assert.All(hostile, construct => Assert.ThrowsAny<ArgumentException>(construct));
    }

    [Fact]
    public void ViewsRejectForgedMappingsBytesAndZeroHeightsAndSnapshotInputs()
    {
        Assert.Throws<ArgumentException>(() => View(protocolId: PrivacyProtocolIdV1.IrohaZkAmsV1));
        Assert.Throws<ArgumentException>(() =>
            View(ledgerEffectKind: PrivacyLedgerEffectKindV1.VerificationOnly));
        Assert.Throws<ArgumentException>(() => View(transactionHash: new byte[32]));
        Assert.Throws<ArgumentException>(() =>
            View(capabilityManifestDigest: Enumerable.Repeat((byte)1, 31).ToArray()));
        Assert.Throws<ArgumentOutOfRangeException>(() => View(capabilityCommittedHeight: 0));
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            View(
                localState: PrivacyActionLocalStateV1.Terminal,
                terminalChainState: PrivacyActionTerminalChainStateV1.Committed,
                committedHeight: 0));
        Assert.Throws<ArgumentException>(() =>
            View(
                localState: PrivacyActionLocalStateV1.Terminal,
                terminalChainState: PrivacyActionTerminalChainStateV1.Rejected,
                committedHeight: 9,
                rejectionReason: "proof rejected"));
        Assert.Throws<ArgumentException>(() =>
            View(
                localState: PrivacyActionLocalStateV1.Terminal,
                terminalChainState: PrivacyActionTerminalChainStateV1.Applied,
                committedHeight: 42,
                executionCapabilityManifestDigest: new byte[32],
                executionCapabilityCommittedHeight: 40,
                executionReceiptFinalizedHeight: 44,
                executionReceiptFinalizedBlockHash: Fixed32(7)));

        var transactionHash = Fixed32(0x11);
        var capabilityDigest = Fixed32(0x12);
        var snapshot = View(
            transactionHash: transactionHash,
            capabilityManifestDigest: capabilityDigest,
            capabilityCommittedHeight: ulong.MaxValue);
        transactionHash[0] = 0;
        capabilityDigest[0] = 0;
        Assert.Equal(Fixed32(0x11), snapshot.TransactionHash);
        Assert.Equal(Fixed32(0x12), snapshot.CapabilityManifestDigest);
        var leaked = snapshot.TransactionHash;
        leaked[0] = 0;
        Assert.Equal(0x11, snapshot.TransactionHash[0]);
        Assert.Equal(ulong.MaxValue, snapshot.CapabilityCommittedHeight);

        var executionDigest = Fixed32(0x31);
        var finalizedBlockHash = Fixed32(0x32);
        var applied = View(
            localState: PrivacyActionLocalStateV1.Terminal,
            terminalChainState: PrivacyActionTerminalChainStateV1.Applied,
            committedHeight: 42,
            executionCapabilityManifestDigest: executionDigest,
            executionCapabilityCommittedHeight: 40,
            executionReceiptFinalizedHeight: 44,
            executionReceiptFinalizedBlockHash: finalizedBlockHash);
        executionDigest[0] = 0;
        finalizedBlockHash[0] = 0;
        Assert.Equal(Fixed32(0x31), applied.ExecutionCapabilityManifestDigest);
        Assert.Equal(Fixed32(0x32), applied.ExecutionReceiptFinalizedBlockHash);
        var leakedExecutionDigest = applied.ExecutionCapabilityManifestDigest!;
        leakedExecutionDigest[0] = 0;
        Assert.Equal(0x31, applied.ExecutionCapabilityManifestDigest![0]);
    }

    [Fact]
    public void AuthenticatedProvenanceBindsClientNetworkAndSurvivesTerminalCopy()
    {
        var detached = View();
        var owner = new object();
        var otherOwner = new object();
        var network = global::Hyperledger.Iroha.NetworkId.Parse(
            "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0");
        var otherNetwork = global::Hyperledger.Iroha.NetworkId.Parse(
            "hash:82531CE8EAE8BFF6BEECA4698BFD13A3BC8BEC5F0EE0D23D428C97FC17AB0F3B#3E94");
        Assert.Throws<InvalidOperationException>(() =>
            detached.RequireAuthenticatedProvenanceV1(owner, network));

        detached.BindAuthenticatedSubmissionV1(owner, network);
        detached.RequireAuthenticatedProvenanceV1(owner, network);
        Assert.Throws<InvalidOperationException>(() =>
            detached.RequireAuthenticatedProvenanceV1(otherOwner, network));
        Assert.Throws<InvalidOperationException>(() =>
            detached.RequireAuthenticatedProvenanceV1(owner, otherNetwork));

        var terminal = detached.WithAuthenticatedTerminalStateV1(
            PrivacyActionTerminalChainStateV1.Applied,
            committedHeight: 17,
            rejectionReason: null,
            executionCapabilityManifestDigest: Fixed32(6),
            executionCapabilityCommittedHeight: 16,
            executionReceiptFinalizedHeight: 18,
            executionReceiptFinalizedBlockHash: Fixed32(7));
        terminal.RequireAuthenticatedProvenanceV1(owner, network);
        Assert.Equal(PrivacyActionLocalStateV1.Terminal, terminal.LocalState);
        Assert.NotSame(detached, terminal);
    }

    private static PrivacyActionOperationViewV1 View(
        PrivacyActionLocalStateV1 localState = PrivacyActionLocalStateV1.Submitted,
        PrivacyActionTerminalChainStateV1? terminalChainState = null,
        ulong? committedHeight = null,
        string? rejectionReason = null,
        PrivacyProtocolIdV1? protocolId = null,
        PrivacyLedgerEffectKindV1? ledgerEffectKind = null,
        byte[]? transactionHash = null,
        byte[]? capabilityManifestDigest = null,
        ulong capabilityCommittedHeight = 10,
        byte[]? executionCapabilityManifestDigest = null,
        ulong? executionCapabilityCommittedHeight = null,
        ulong? executionReceiptFinalizedHeight = null,
        byte[]? executionReceiptFinalizedBlockHash = null)
    {
        const PrivacyOperationSchemaV1 operation = PrivacyOperationSchemaV1.OrchardNoteActionV1;
        return new PrivacyActionOperationViewV1(
            protocolId ?? operation.ProtocolId(),
            operation,
            transactionHash ?? Fixed32(1),
            Fixed32(2),
            Fixed32(3),
            Fixed32(4),
            localState,
            terminalChainState,
            committedHeight,
            rejectionReason,
            ledgerEffectKind ?? operation.LedgerEffectKind(),
            capabilityManifestDigest ?? Fixed32(5),
            capabilityCommittedHeight,
            executionCapabilityManifestDigest,
            executionCapabilityCommittedHeight,
            executionReceiptFinalizedHeight,
            executionReceiptFinalizedBlockHash);
    }

    private static byte[] Fixed32(byte value) => Enumerable.Repeat(value, 32).ToArray();
}
