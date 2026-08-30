using System.Globalization;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace Hyperledger.Iroha.Privacy;

/// <summary>Authentication context for one finalized typed privacy-state query.</summary>
public sealed class PrivacyFinalizedStateQueryOptionsV1
{
    public required NetworkId NetworkId { get; init; }

    public string Scope { get; init; } = "global";
}

internal interface IPrivacyFinalizedStateRequestV1
{
    uint QueryId { get; }

    uint ProtocolIndex { get; }

    byte[] RequestBinding { get; }
}

/// <summary>Selector for finalized provenance of one consumed ZK-ACE replay nullifier.</summary>
public sealed class PrivacyZkAceReplayNullifierRequestV1 : IPrivacyFinalizedStateRequestV1
{
    private readonly byte[] policyId;
    private readonly byte[] replayNullifier;

    public PrivacyZkAceReplayNullifierRequestV1(byte[] policyId, byte[] replayNullifier)
    {
        this.policyId = PrivacyFinalizedStateContractV1.RequireFixed32(policyId, nameof(policyId));
        this.replayNullifier = PrivacyFinalizedStateContractV1.RequireFixed32(
            replayNullifier,
            nameof(replayNullifier));
    }

    public byte[] PolicyId => (byte[])policyId.Clone();

    public byte[] ReplayNullifier => (byte[])replayNullifier.Clone();

    uint IPrivacyFinalizedStateRequestV1.QueryId => 97;

    uint IPrivacyFinalizedStateRequestV1.ProtocolIndex => 0;

    byte[] IPrivacyFinalizedStateRequestV1.RequestBinding =>
        PrivacyFinalizedStateContractV1.ConcatFixed32(policyId, replayNullifier);
}

/// <summary>Selector for finalized FCMP++, private-IVM, or PQ-MASP pool state.</summary>
public sealed class PrivacyProofManagedPoolStateRequestV1 : IPrivacyFinalizedStateRequestV1
{
    private readonly byte[] poolId;

    public PrivacyProofManagedPoolStateRequestV1(
        PrivacyProtocolIdV1 protocolId,
        byte[] poolId)
    {
        ProtocolId = protocolId;
        _ = PrivacyFinalizedStateContractV1.ProofManagedProtocolIndex(protocolId);
        this.poolId = PrivacyFinalizedStateContractV1.RequireFixed32(poolId, nameof(poolId));
    }

    public PrivacyProtocolIdV1 ProtocolId { get; }

    public byte[] PoolId => (byte[])poolId.Clone();

    uint IPrivacyFinalizedStateRequestV1.QueryId => 98;

    uint IPrivacyFinalizedStateRequestV1.ProtocolIndex =>
        PrivacyFinalizedStateContractV1.ProofManagedProtocolIndex(ProtocolId);

    byte[] IPrivacyFinalizedStateRequestV1.RequestBinding => (byte[])poolId.Clone();
}

/// <summary>Selector for finalized state of one governed Orchard pool.</summary>
public sealed class PrivacyOrchardPoolStateRequestV1 : IPrivacyFinalizedStateRequestV1
{
    private readonly byte[] poolId;

    public PrivacyOrchardPoolStateRequestV1(byte[] poolId)
    {
        this.poolId = PrivacyFinalizedStateContractV1.RequireFixed32(poolId, nameof(poolId));
    }

    public byte[] PoolId => (byte[])poolId.Clone();

    uint IPrivacyFinalizedStateRequestV1.QueryId => 99;

    uint IPrivacyFinalizedStateRequestV1.ProtocolIndex => 0;

    byte[] IPrivacyFinalizedStateRequestV1.RequestBinding => (byte[])poolId.Clone();
}

/// <summary>Selector for finalized provenance of one consumed Orchard nullifier.</summary>
public sealed class PrivacyOrchardNullifierRequestV1 : IPrivacyFinalizedStateRequestV1
{
    private readonly byte[] poolId;
    private readonly byte[] nullifier;

    public PrivacyOrchardNullifierRequestV1(byte[] poolId, byte[] nullifier)
    {
        this.poolId = PrivacyFinalizedStateContractV1.RequireFixed32(poolId, nameof(poolId));
        this.nullifier = PrivacyFinalizedStateContractV1.RequireFixed32(
            nullifier,
            nameof(nullifier));
    }

    public byte[] PoolId => (byte[])poolId.Clone();

    public byte[] Nullifier => (byte[])nullifier.Clone();

    uint IPrivacyFinalizedStateRequestV1.QueryId => 100;

    uint IPrivacyFinalizedStateRequestV1.ProtocolIndex => 0;

    byte[] IPrivacyFinalizedStateRequestV1.RequestBinding =>
        PrivacyFinalizedStateContractV1.ConcatFixed32(poolId, nullifier);
}

/// <summary>Selector for finalized public state of one Anonymous PGC pool.</summary>
public sealed class PrivacyAnonymousPgcPoolStateRequestV1 : IPrivacyFinalizedStateRequestV1
{
    private readonly byte[] poolId;

    public PrivacyAnonymousPgcPoolStateRequestV1(byte[] poolId)
    {
        this.poolId = PrivacyFinalizedStateContractV1.RequireFixed32(poolId, nameof(poolId));
    }

    public byte[] PoolId => (byte[])poolId.Clone();

    uint IPrivacyFinalizedStateRequestV1.QueryId => 101;

    uint IPrivacyFinalizedStateRequestV1.ProtocolIndex => 0;

    byte[] IPrivacyFinalizedStateRequestV1.RequestBinding => (byte[])poolId.Clone();
}

/// <summary>Selector for finalized provenance of one admitted ZK-AMS PHC anchor.</summary>
public sealed class PrivacyZkAmsAdmissionRequestV1 : IPrivacyFinalizedStateRequestV1
{
    private readonly byte[] issuerId;
    private readonly byte[] registryId;
    private readonly byte[] policyId;
    private readonly byte[] phcHash;

    public PrivacyZkAmsAdmissionRequestV1(
        byte[] issuerId,
        byte[] registryId,
        byte[] policyId,
        byte[] phcHash)
    {
        this.issuerId = PrivacyFinalizedStateContractV1.RequireFixed32(issuerId, nameof(issuerId));
        this.registryId = PrivacyFinalizedStateContractV1.RequireFixed32(
            registryId,
            nameof(registryId));
        this.policyId = PrivacyFinalizedStateContractV1.RequireFixed32(policyId, nameof(policyId));
        this.phcHash = PrivacyFinalizedStateContractV1.RequireFixed32(phcHash, nameof(phcHash));
    }

    public byte[] IssuerId => (byte[])issuerId.Clone();

    public byte[] RegistryId => (byte[])registryId.Clone();

    public byte[] PolicyId => (byte[])policyId.Clone();

    public byte[] PhcHash => (byte[])phcHash.Clone();

    uint IPrivacyFinalizedStateRequestV1.QueryId => 102;

    uint IPrivacyFinalizedStateRequestV1.ProtocolIndex => 0;

    byte[] IPrivacyFinalizedStateRequestV1.RequestBinding =>
        PrivacyFinalizedStateContractV1.ConcatFixed32(
            issuerId,
            registryId,
            policyId,
            phcHash);
}

/// <summary>Selector for finalized provenance of one anonymous ZK-AMS provision.</summary>
public sealed class PrivacyZkAmsProvisionRequestV1 : IPrivacyFinalizedStateRequestV1
{
    private readonly byte[] issuerId;
    private readonly byte[] registryId;
    private readonly byte[] policyId;
    private readonly byte[] keyImage;

    public PrivacyZkAmsProvisionRequestV1(
        byte[] issuerId,
        byte[] registryId,
        byte[] policyId,
        byte[] keyImage)
    {
        this.issuerId = PrivacyFinalizedStateContractV1.RequireFixed32(issuerId, nameof(issuerId));
        this.registryId = PrivacyFinalizedStateContractV1.RequireFixed32(
            registryId,
            nameof(registryId));
        this.policyId = PrivacyFinalizedStateContractV1.RequireFixed32(policyId, nameof(policyId));
        this.keyImage = PrivacyFinalizedStateContractV1.RequireFixed32(keyImage, nameof(keyImage));
    }

    public byte[] IssuerId => (byte[])issuerId.Clone();

    public byte[] RegistryId => (byte[])registryId.Clone();

    public byte[] PolicyId => (byte[])policyId.Clone();

    public byte[] KeyImage => (byte[])keyImage.Clone();

    uint IPrivacyFinalizedStateRequestV1.QueryId => 103;

    uint IPrivacyFinalizedStateRequestV1.ProtocolIndex => 0;

    byte[] IPrivacyFinalizedStateRequestV1.RequestBinding =>
        PrivacyFinalizedStateContractV1.ConcatFixed32(
            issuerId,
            registryId,
            policyId,
            keyImage);
}

/// <summary>Selector for finalized provenance of one consumed ZK-X509 nullifier.</summary>
public sealed class PrivacyZkX509CertificateNullifierRequestV1
    : IPrivacyFinalizedStateRequestV1
{
    private readonly byte[] trustAnchorId;
    private readonly byte[] policyId;
    private readonly byte[] nullifier;

    public PrivacyZkX509CertificateNullifierRequestV1(
        byte[] trustAnchorId,
        byte[] policyId,
        byte[] nullifier)
    {
        this.trustAnchorId = PrivacyFinalizedStateContractV1.RequireFixed32(
            trustAnchorId,
            nameof(trustAnchorId));
        this.policyId = PrivacyFinalizedStateContractV1.RequireFixed32(policyId, nameof(policyId));
        this.nullifier = PrivacyFinalizedStateContractV1.RequireFixed32(
            nullifier,
            nameof(nullifier));
    }

    public byte[] TrustAnchorId => (byte[])trustAnchorId.Clone();

    public byte[] PolicyId => (byte[])policyId.Clone();

    public byte[] Nullifier => (byte[])nullifier.Clone();

    uint IPrivacyFinalizedStateRequestV1.QueryId => 104;

    uint IPrivacyFinalizedStateRequestV1.ProtocolIndex => 0;

    byte[] IPrivacyFinalizedStateRequestV1.RequestBinding =>
        PrivacyFinalizedStateContractV1.ConcatFixed32(
            trustAnchorId,
            policyId,
            nullifier);
}

/// <summary>Closed root roles carried by the finalized proof-managed pool view.</summary>
public enum PrivacyFinalizedRootRoleV1
{
    PgcAccountState,
    AccountRegistry,
    Revocation,
    CertificateAuthorityMembership,
    NoteCommitmentAnchor,
    OutputSet,
    ProgramState,
}

/// <summary>Closed public-balance scope carried by one finalized Orchard pool view.</summary>
public sealed record class PrivacyFinalizedAssetBalanceScopeV1
{
    private PrivacyFinalizedAssetBalanceScopeV1(ulong? dataspaceId)
    {
        DataspaceId = dataspaceId;
    }

    public bool IsGlobal => DataspaceId is null;

    public ulong? DataspaceId { get; }

    internal static PrivacyFinalizedAssetBalanceScopeV1 Global { get; } = new(null);

    internal static PrivacyFinalizedAssetBalanceScopeV1 Dataspace(ulong id) => new(id);
}

/// <summary>Shared finalized state binding returned by every query in the closed ID97-104 union.</summary>
public abstract class PrivacyFinalizedStateViewV1
{
    private readonly byte[] finalizedBlockHash;

    protected PrivacyFinalizedStateViewV1(
        NetworkId networkId,
        ulong finalizedHeight,
        byte[] finalizedBlockHash)
    {
        NetworkId = networkId;
        FinalizedHeight = finalizedHeight;
        this.finalizedBlockHash = (byte[])finalizedBlockHash.Clone();
    }

    public NetworkId NetworkId { get; }

    public ulong FinalizedHeight { get; }

    public byte[] FinalizedBlockHash => (byte[])finalizedBlockHash.Clone();
}

/// <summary>Latest verified transition in one proof-managed privacy pool.</summary>
public sealed class PrivacyProofManagedPoolTransitionViewV1
{
    private readonly byte[] statementDigest;

    internal PrivacyProofManagedPoolTransitionViewV1(
        byte[] statementDigest,
        ulong successorEpoch,
        ulong admittedAtHeight,
        uint actionIndex,
        uint nullifierCount,
        uint outputCount)
    {
        this.statementDigest = (byte[])statementDigest.Clone();
        SuccessorEpoch = successorEpoch;
        AdmittedAtHeight = admittedAtHeight;
        ActionIndex = actionIndex;
        NullifierCount = nullifierCount;
        OutputCount = outputCount;
    }

    public byte[] StatementDigest => (byte[])statementDigest.Clone();

    public ulong SuccessorEpoch { get; }

    public ulong AdmittedAtHeight { get; }

    public uint ActionIndex { get; }

    public uint NullifierCount { get; }

    public uint OutputCount { get; }
}

/// <summary>Latest adjacent transition in one governed Orchard pool.</summary>
public sealed class PrivacyOrchardPoolTransitionViewV1
{
    private readonly byte[] statementDigest;
    private readonly byte[] parentRoot;

    internal PrivacyOrchardPoolTransitionViewV1(
        byte[] statementDigest,
        ulong successorEpoch,
        ulong parentEpoch,
        byte[] parentRoot,
        ulong admittedAtHeight,
        uint actionIndex)
    {
        this.statementDigest = (byte[])statementDigest.Clone();
        SuccessorEpoch = successorEpoch;
        ParentEpoch = parentEpoch;
        this.parentRoot = (byte[])parentRoot.Clone();
        AdmittedAtHeight = admittedAtHeight;
        ActionIndex = actionIndex;
    }

    public byte[] StatementDigest => (byte[])statementDigest.Clone();

    public ulong SuccessorEpoch { get; }

    public ulong ParentEpoch { get; }

    public byte[] ParentRoot => (byte[])parentRoot.Clone();

    public ulong AdmittedAtHeight { get; }

    public uint ActionIndex { get; }
}

/// <summary>Latest adjacent transition in one Anonymous PGC pool.</summary>
public sealed class PrivacyAnonymousPgcPoolTransitionViewV1
{
    private readonly byte[] statementDigest;
    private readonly byte[] parentRoot;

    internal PrivacyAnonymousPgcPoolTransitionViewV1(
        byte[] statementDigest,
        ulong successorEpoch,
        ulong parentEpoch,
        byte[] parentRoot,
        ulong admittedAtHeight,
        uint actionIndex)
    {
        this.statementDigest = (byte[])statementDigest.Clone();
        SuccessorEpoch = successorEpoch;
        ParentEpoch = parentEpoch;
        this.parentRoot = (byte[])parentRoot.Clone();
        AdmittedAtHeight = admittedAtHeight;
        ActionIndex = actionIndex;
    }

    public byte[] StatementDigest => (byte[])statementDigest.Clone();

    public ulong SuccessorEpoch { get; }

    public ulong ParentEpoch { get; }

    public byte[] ParentRoot => (byte[])parentRoot.Clone();

    public ulong AdmittedAtHeight { get; }

    public uint ActionIndex { get; }
}

/// <summary>Finalized provenance for one consumed ZK-ACE replay nullifier.</summary>
public sealed class PrivacyZkAceReplayNullifierProvenanceV1 : PrivacyFinalizedStateViewV1
{
    private readonly byte[] policyId;
    private readonly byte[] replayNullifier;
    private readonly byte[] policyRecordDigest;
    private readonly byte[] statementDigest;

    internal PrivacyZkAceReplayNullifierProvenanceV1(
        NetworkId networkId,
        byte[] policyId,
        byte[] replayNullifier,
        byte[] policyRecordDigest,
        byte[] statementDigest,
        ulong admittedAtHeight,
        uint actionIndex,
        ulong finalizedHeight,
        byte[] finalizedBlockHash)
        : base(networkId, finalizedHeight, finalizedBlockHash)
    {
        this.policyId = (byte[])policyId.Clone();
        this.replayNullifier = (byte[])replayNullifier.Clone();
        this.policyRecordDigest = (byte[])policyRecordDigest.Clone();
        this.statementDigest = (byte[])statementDigest.Clone();
        AdmittedAtHeight = admittedAtHeight;
        ActionIndex = actionIndex;
    }

    public byte[] PolicyId => (byte[])policyId.Clone();

    public byte[] ReplayNullifier => (byte[])replayNullifier.Clone();

    public byte[] PolicyRecordDigest => (byte[])policyRecordDigest.Clone();

    public byte[] StatementDigest => (byte[])statementDigest.Clone();

    public ulong AdmittedAtHeight { get; }

    public uint ActionIndex { get; }
}

/// <summary>Finalized typed state of one FCMP++, private-IVM, or PQ-MASP pool.</summary>
public sealed class PrivacyProofManagedPoolStateViewV1 : PrivacyFinalizedStateViewV1
{
    private readonly byte[] poolId;
    private readonly byte[] bootstrapDigest;
    private readonly byte[] initialRoot;
    private readonly byte[] currentRoot;

    internal PrivacyProofManagedPoolStateViewV1(
        NetworkId networkId,
        PrivacyProtocolIdV1 protocolId,
        byte[] poolId,
        string assetDefinitionId,
        PrivacyFinalizedRootRoleV1 rootRole,
        byte[] bootstrapDigest,
        byte[] initialRoot,
        ulong currentEpoch,
        byte[] currentRoot,
        ulong outputCount,
        ulong bootstrapAdmittedAtHeight,
        PrivacyProofManagedPoolTransitionViewV1? latestTransition,
        ulong finalizedHeight,
        byte[] finalizedBlockHash)
        : base(networkId, finalizedHeight, finalizedBlockHash)
    {
        ProtocolId = protocolId;
        this.poolId = (byte[])poolId.Clone();
        AssetDefinitionId = assetDefinitionId;
        RootRole = rootRole;
        this.bootstrapDigest = (byte[])bootstrapDigest.Clone();
        this.initialRoot = (byte[])initialRoot.Clone();
        CurrentEpoch = currentEpoch;
        this.currentRoot = (byte[])currentRoot.Clone();
        OutputCount = outputCount;
        BootstrapAdmittedAtHeight = bootstrapAdmittedAtHeight;
        LatestTransition = latestTransition;
    }

    public PrivacyProtocolIdV1 ProtocolId { get; }

    public byte[] PoolId => (byte[])poolId.Clone();

    public string AssetDefinitionId { get; }

    public PrivacyFinalizedRootRoleV1 RootRole { get; }

    public byte[] BootstrapDigest => (byte[])bootstrapDigest.Clone();

    public byte[] InitialRoot => (byte[])initialRoot.Clone();

    public ulong CurrentEpoch { get; }

    public byte[] CurrentRoot => (byte[])currentRoot.Clone();

    public ulong OutputCount { get; }

    public ulong BootstrapAdmittedAtHeight { get; }

    public PrivacyProofManagedPoolTransitionViewV1? LatestTransition { get; }
}

/// <summary>Finalized typed public state of one governed Orchard pool.</summary>
public sealed class PrivacyOrchardPoolStateViewV1 : PrivacyFinalizedStateViewV1
{
    private readonly byte[] poolId;
    private readonly byte[] bootstrapDigest;
    private readonly byte[] currentRoot;

    internal PrivacyOrchardPoolStateViewV1(
        NetworkId networkId,
        byte[] poolId,
        string assetDefinitionId,
        PrivacyFinalizedAssetBalanceScopeV1 publicBalanceScope,
        string reserveAccount,
        byte[] bootstrapDigest,
        ulong currentEpoch,
        byte[] currentRoot,
        ulong treeSize,
        PrivacyOrchardPoolTransitionViewV1? latestTransition,
        ulong finalizedHeight,
        byte[] finalizedBlockHash)
        : base(networkId, finalizedHeight, finalizedBlockHash)
    {
        this.poolId = (byte[])poolId.Clone();
        AssetDefinitionId = assetDefinitionId;
        PublicBalanceScope = publicBalanceScope;
        ReserveAccount = reserveAccount;
        this.bootstrapDigest = (byte[])bootstrapDigest.Clone();
        CurrentEpoch = currentEpoch;
        this.currentRoot = (byte[])currentRoot.Clone();
        TreeSize = treeSize;
        LatestTransition = latestTransition;
    }

    public byte[] PoolId => (byte[])poolId.Clone();

    public string AssetDefinitionId { get; }

    public PrivacyFinalizedAssetBalanceScopeV1 PublicBalanceScope { get; }

    public string ReserveAccount { get; }

    public byte[] BootstrapDigest => (byte[])bootstrapDigest.Clone();

    public ulong CurrentEpoch { get; }

    public byte[] CurrentRoot => (byte[])currentRoot.Clone();

    public ulong TreeSize { get; }

    public PrivacyOrchardPoolTransitionViewV1? LatestTransition { get; }
}

/// <summary>Finalized provenance for one consumed Orchard nullifier.</summary>
public sealed class PrivacyOrchardNullifierProvenanceV1 : PrivacyFinalizedStateViewV1
{
    private readonly byte[] poolId;
    private readonly byte[] nullifier;
    private readonly byte[] bootstrapDigest;
    private readonly byte[] statementDigest;

    internal PrivacyOrchardNullifierProvenanceV1(
        NetworkId networkId,
        byte[] poolId,
        byte[] nullifier,
        byte[] bootstrapDigest,
        byte[] statementDigest,
        ulong admittedAtHeight,
        uint actionIndex,
        ulong finalizedHeight,
        byte[] finalizedBlockHash)
        : base(networkId, finalizedHeight, finalizedBlockHash)
    {
        this.poolId = (byte[])poolId.Clone();
        this.nullifier = (byte[])nullifier.Clone();
        this.bootstrapDigest = (byte[])bootstrapDigest.Clone();
        this.statementDigest = (byte[])statementDigest.Clone();
        AdmittedAtHeight = admittedAtHeight;
        ActionIndex = actionIndex;
    }

    public byte[] PoolId => (byte[])poolId.Clone();

    public byte[] Nullifier => (byte[])nullifier.Clone();

    public byte[] BootstrapDigest => (byte[])bootstrapDigest.Clone();

    public byte[] StatementDigest => (byte[])statementDigest.Clone();

    public ulong AdmittedAtHeight { get; }

    public uint ActionIndex { get; }
}

/// <summary>Finalized bounded public state of one Anonymous PGC pool.</summary>
public sealed class PrivacyAnonymousPgcPoolStateViewV1 : PrivacyFinalizedStateViewV1
{
    private readonly byte[] poolId;
    private readonly byte[] bootstrapRoot;
    private readonly byte[] bootstrapDigest;
    private readonly byte[] bootstrapProofDigest;
    private readonly byte[] currentRoot;

    internal PrivacyAnonymousPgcPoolStateViewV1(
        NetworkId networkId,
        byte[] poolId,
        uint totalSupply,
        byte[] bootstrapRoot,
        byte[] bootstrapDigest,
        byte[] bootstrapProofDigest,
        ulong currentEpoch,
        byte[] currentRoot,
        uint accountCount,
        ulong currentStateAdmittedAtHeight,
        PrivacyAnonymousPgcPoolTransitionViewV1? latestTransition,
        ulong finalizedHeight,
        byte[] finalizedBlockHash)
        : base(networkId, finalizedHeight, finalizedBlockHash)
    {
        this.poolId = (byte[])poolId.Clone();
        TotalSupply = totalSupply;
        this.bootstrapRoot = (byte[])bootstrapRoot.Clone();
        this.bootstrapDigest = (byte[])bootstrapDigest.Clone();
        this.bootstrapProofDigest = (byte[])bootstrapProofDigest.Clone();
        CurrentEpoch = currentEpoch;
        this.currentRoot = (byte[])currentRoot.Clone();
        AccountCount = accountCount;
        CurrentStateAdmittedAtHeight = currentStateAdmittedAtHeight;
        LatestTransition = latestTransition;
    }

    public byte[] PoolId => (byte[])poolId.Clone();

    public uint TotalSupply { get; }

    public byte[] BootstrapRoot => (byte[])bootstrapRoot.Clone();

    public byte[] BootstrapDigest => (byte[])bootstrapDigest.Clone();

    public byte[] BootstrapProofDigest => (byte[])bootstrapProofDigest.Clone();

    public ulong CurrentEpoch { get; }

    public byte[] CurrentRoot => (byte[])currentRoot.Clone();

    public uint AccountCount { get; }

    public ulong CurrentStateAdmittedAtHeight { get; }

    public PrivacyAnonymousPgcPoolTransitionViewV1? LatestTransition { get; }
}

/// <summary>Finalized provenance for one admitted ZK-AMS personhood anchor.</summary>
public sealed class PrivacyZkAmsAdmissionViewV1 : PrivacyFinalizedStateViewV1
{
    private readonly byte[] issuerId;
    private readonly byte[] registryId;
    private readonly byte[] policyId;
    private readonly byte[] phcHash;
    private readonly byte[] seedPublicKey;
    private readonly byte[] bootstrapDigest;
    private readonly byte[] issuerPolicyRecordDigest;
    private readonly byte[] policyDigest;
    private readonly byte[] registryRecordDigest;
    private readonly byte[] parentRoot;
    private readonly byte[] successorRoot;
    private readonly byte[] statementDigest;

    internal PrivacyZkAmsAdmissionViewV1(
        NetworkId networkId,
        byte[] issuerId,
        byte[] registryId,
        byte[] policyId,
        byte[] phcHash,
        byte[] seedPublicKey,
        byte[] bootstrapDigest,
        byte[] issuerPolicyRecordDigest,
        byte[] policyDigest,
        byte[] registryRecordDigest,
        ulong parentEpoch,
        byte[] parentRoot,
        uint anchorIndex,
        uint batchSize,
        ulong successorEpoch,
        byte[] successorRoot,
        byte[] statementDigest,
        ulong admittedAtHeight,
        uint actionIndex,
        ulong finalizedHeight,
        byte[] finalizedBlockHash)
        : base(networkId, finalizedHeight, finalizedBlockHash)
    {
        this.issuerId = (byte[])issuerId.Clone();
        this.registryId = (byte[])registryId.Clone();
        this.policyId = (byte[])policyId.Clone();
        this.phcHash = (byte[])phcHash.Clone();
        this.seedPublicKey = (byte[])seedPublicKey.Clone();
        this.bootstrapDigest = (byte[])bootstrapDigest.Clone();
        this.issuerPolicyRecordDigest = (byte[])issuerPolicyRecordDigest.Clone();
        this.policyDigest = (byte[])policyDigest.Clone();
        this.registryRecordDigest = (byte[])registryRecordDigest.Clone();
        ParentEpoch = parentEpoch;
        this.parentRoot = (byte[])parentRoot.Clone();
        AnchorIndex = anchorIndex;
        BatchSize = batchSize;
        SuccessorEpoch = successorEpoch;
        this.successorRoot = (byte[])successorRoot.Clone();
        this.statementDigest = (byte[])statementDigest.Clone();
        AdmittedAtHeight = admittedAtHeight;
        ActionIndex = actionIndex;
    }

    public byte[] IssuerId => (byte[])issuerId.Clone();

    public byte[] RegistryId => (byte[])registryId.Clone();

    public byte[] PolicyId => (byte[])policyId.Clone();

    public byte[] PhcHash => (byte[])phcHash.Clone();

    public byte[] SeedPublicKey => (byte[])seedPublicKey.Clone();

    public byte[] BootstrapDigest => (byte[])bootstrapDigest.Clone();

    public byte[] IssuerPolicyRecordDigest => (byte[])issuerPolicyRecordDigest.Clone();

    public byte[] PolicyDigest => (byte[])policyDigest.Clone();

    public byte[] RegistryRecordDigest => (byte[])registryRecordDigest.Clone();

    public ulong ParentEpoch { get; }

    public byte[] ParentRoot => (byte[])parentRoot.Clone();

    public uint AnchorIndex { get; }

    public uint BatchSize { get; }

    public ulong SuccessorEpoch { get; }

    public byte[] SuccessorRoot => (byte[])successorRoot.Clone();

    public byte[] StatementDigest => (byte[])statementDigest.Clone();

    public ulong AdmittedAtHeight { get; }

    public uint ActionIndex { get; }
}

/// <summary>Finalized provenance for one anonymous ZK-AMS account provision.</summary>
public sealed class PrivacyZkAmsProvisionViewV1 : PrivacyFinalizedStateViewV1
{
    private readonly byte[] issuerId;
    private readonly byte[] registryId;
    private readonly byte[] policyId;
    private readonly byte[] keyImage;
    private readonly byte[] bootstrapDigest;
    private readonly byte[] issuerPolicyRecordDigest;
    private readonly byte[] policyDigest;
    private readonly byte[] registryRecordDigest;
    private readonly byte[] registryRoot;
    private readonly byte[] statementDigest;

    internal PrivacyZkAmsProvisionViewV1(
        NetworkId networkId,
        byte[] issuerId,
        byte[] registryId,
        byte[] policyId,
        byte[] keyImage,
        string accountId,
        byte[] bootstrapDigest,
        byte[] issuerPolicyRecordDigest,
        byte[] policyDigest,
        byte[] registryRecordDigest,
        ulong registryEpoch,
        byte[] registryRoot,
        byte[] statementDigest,
        ulong admittedAtHeight,
        uint actionIndex,
        ulong finalizedHeight,
        byte[] finalizedBlockHash)
        : base(networkId, finalizedHeight, finalizedBlockHash)
    {
        this.issuerId = (byte[])issuerId.Clone();
        this.registryId = (byte[])registryId.Clone();
        this.policyId = (byte[])policyId.Clone();
        this.keyImage = (byte[])keyImage.Clone();
        AccountId = accountId;
        this.bootstrapDigest = (byte[])bootstrapDigest.Clone();
        this.issuerPolicyRecordDigest = (byte[])issuerPolicyRecordDigest.Clone();
        this.policyDigest = (byte[])policyDigest.Clone();
        this.registryRecordDigest = (byte[])registryRecordDigest.Clone();
        RegistryEpoch = registryEpoch;
        this.registryRoot = (byte[])registryRoot.Clone();
        this.statementDigest = (byte[])statementDigest.Clone();
        AdmittedAtHeight = admittedAtHeight;
        ActionIndex = actionIndex;
    }

    public byte[] IssuerId => (byte[])issuerId.Clone();

    public byte[] RegistryId => (byte[])registryId.Clone();

    public byte[] PolicyId => (byte[])policyId.Clone();

    public byte[] KeyImage => (byte[])keyImage.Clone();

    public string AccountId { get; }

    public byte[] BootstrapDigest => (byte[])bootstrapDigest.Clone();

    public byte[] IssuerPolicyRecordDigest => (byte[])issuerPolicyRecordDigest.Clone();

    public byte[] PolicyDigest => (byte[])policyDigest.Clone();

    public byte[] RegistryRecordDigest => (byte[])registryRecordDigest.Clone();

    public ulong RegistryEpoch { get; }

    public byte[] RegistryRoot => (byte[])registryRoot.Clone();

    public byte[] StatementDigest => (byte[])statementDigest.Clone();

    public ulong AdmittedAtHeight { get; }

    public uint ActionIndex { get; }
}

/// <summary>Finalized provenance for one consumed ZK-X509 certificate nullifier.</summary>
public sealed class PrivacyZkX509CertificateNullifierProvenanceV1
    : PrivacyFinalizedStateViewV1
{
    private readonly byte[] trustAnchorId;
    private readonly byte[] policyId;
    private readonly byte[] nullifier;
    private readonly byte[] trustAnchorRecordDigest;
    private readonly byte[] certificatePolicyRecordDigest;
    private readonly byte[] crlRecordDigest;
    private readonly byte[] statementDigest;

    internal PrivacyZkX509CertificateNullifierProvenanceV1(
        NetworkId networkId,
        byte[] trustAnchorId,
        byte[] policyId,
        byte[] nullifier,
        byte[] trustAnchorRecordDigest,
        ulong trustAnchorRecordEpoch,
        byte[] certificatePolicyRecordDigest,
        ulong certificatePolicyRecordEpoch,
        byte[] crlRecordDigest,
        ulong crlRecordEpoch,
        byte[] statementDigest,
        ulong admittedAtHeight,
        uint actionIndex,
        ulong finalizedHeight,
        byte[] finalizedBlockHash)
        : base(networkId, finalizedHeight, finalizedBlockHash)
    {
        this.trustAnchorId = (byte[])trustAnchorId.Clone();
        this.policyId = (byte[])policyId.Clone();
        this.nullifier = (byte[])nullifier.Clone();
        this.trustAnchorRecordDigest = (byte[])trustAnchorRecordDigest.Clone();
        TrustAnchorRecordEpoch = trustAnchorRecordEpoch;
        this.certificatePolicyRecordDigest = (byte[])certificatePolicyRecordDigest.Clone();
        CertificatePolicyRecordEpoch = certificatePolicyRecordEpoch;
        this.crlRecordDigest = (byte[])crlRecordDigest.Clone();
        CrlRecordEpoch = crlRecordEpoch;
        this.statementDigest = (byte[])statementDigest.Clone();
        AdmittedAtHeight = admittedAtHeight;
        ActionIndex = actionIndex;
    }

    public byte[] TrustAnchorId => (byte[])trustAnchorId.Clone();

    public byte[] PolicyId => (byte[])policyId.Clone();

    public byte[] Nullifier => (byte[])nullifier.Clone();

    public byte[] TrustAnchorRecordDigest => (byte[])trustAnchorRecordDigest.Clone();

    public ulong TrustAnchorRecordEpoch { get; }

    public byte[] CertificatePolicyRecordDigest =>
        (byte[])certificatePolicyRecordDigest.Clone();

    public ulong CertificatePolicyRecordEpoch { get; }

    public byte[] CrlRecordDigest => (byte[])crlRecordDigest.Clone();

    public ulong CrlRecordEpoch { get; }

    public byte[] StatementDigest => (byte[])statementDigest.Clone();

    public ulong AdmittedAtHeight { get; }

    public uint ActionIndex { get; }
}

internal static class PrivacyFinalizedStateContractV1
{
    private static readonly UTF8Encoding StrictUtf8 = new(false, true);

    internal static byte[] RequireFixed32(byte[] value, string parameterName)
    {
        ArgumentNullException.ThrowIfNull(value, parameterName);
        if (value.Length != 32 || !Array.Exists(value, static item => item != 0))
        {
            throw new ArgumentException(
                "Finalized privacy-state selectors must contain exactly 32 nonzero bytes.",
                parameterName);
        }
        return (byte[])value.Clone();
    }

    internal static byte[] ConcatFixed32(params byte[][] chunks)
    {
        var output = new byte[checked(chunks.Length * 32)];
        for (var index = 0; index < chunks.Length; index++)
        {
            var chunk = RequireFixed32(chunks[index], nameof(chunks));
            chunk.CopyTo(output, index * 32);
        }
        return output;
    }

    internal static uint ProofManagedProtocolIndex(PrivacyProtocolIdV1 protocolId) =>
        protocolId switch
        {
            PrivacyProtocolIdV1.MoneroFcmpPlusPlusV1 => 0,
            PrivacyProtocolIdV1.IrohaIvmPrivateNoteStarkV1 => 1,
            PrivacyProtocolIdV1.PqMaspStarkV0 => 2,
            _ => throw new ArgumentOutOfRangeException(
                nameof(protocolId),
                "Proof-managed state supports only FCMP++, private-IVM, or PQ-MASP."),
        };

    internal static PrivacyFinalizedStateViewV1 ParseProjectionV1(
        byte[] jsonBytes,
        PrivacyAuthenticatedStateQueryV1 query)
    {
        ArgumentNullException.ThrowIfNull(jsonBytes);
        ArgumentNullException.ThrowIfNull(query);
        if (jsonBytes.Length is < 1 or > PrivacyNative.PrivacyAuthenticatedStateQueryProjectionMaxBytes)
        {
            throw new InvalidDataException(
                "Authenticated privacy state-query projection violates its byte bound.");
        }
        string json;
        try
        {
            json = StrictUtf8.GetString(jsonBytes);
        }
        catch (DecoderFallbackException error)
        {
            throw new InvalidDataException(
                "Authenticated privacy state-query projection is not UTF-8.",
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
                "Authenticated privacy state-query projection must be an object.");
        }
        return query.QueryId switch
        {
            97 => ParseZkAceReplay(root, query),
            98 => ParseProofManagedPool(root, query),
            99 => ParseOrchardPool(root, query),
            100 => ParseOrchardNullifier(root, query),
            101 => ParseAnonymousPgcPool(root, query),
            102 => ParseZkAmsAdmission(root, query),
            103 => ParseZkAmsProvision(root, query),
            104 => ParseZkX509Nullifier(root, query),
            _ => throw new InvalidDataException(
                "Authenticated privacy state-query projection has an unsupported query ID."),
        };
    }

    private static PrivacyZkAceReplayNullifierProvenanceV1 ParseZkAceReplay(
        JsonElement root,
        PrivacyAuthenticatedStateQueryV1 query)
    {
        RequireExactFields(
            root,
            "network_id",
            "policy_id",
            "replay_nullifier",
            "policy_record_digest",
            "statement_digest",
            "admitted_at_height",
            "action_index",
            "finalized_height",
            "finalized_block_hash");
        var networkId = RequireExpectedNetwork(root, query);
        var policyId = RequireFixed32Array(root, "policy_id");
        var replayNullifier = RequireFixed32Array(root, "replay_nullifier");
        RequireBinding(query, policyId, replayNullifier);
        var admittedAtHeight = RequireU64(root, "admitted_at_height", allowZero: false);
        var finalizedHeight = RequireU64(root, "finalized_height", allowZero: false);
        if (finalizedHeight < admittedAtHeight)
        {
            throw Invalid("ZK-ACE finality predates replay admission");
        }
        return new PrivacyZkAceReplayNullifierProvenanceV1(
            networkId,
            policyId,
            replayNullifier,
            RequireFixed32Array(root, "policy_record_digest"),
            RequireFixed32Array(root, "statement_digest"),
            admittedAtHeight,
            RequireU32(root, "action_index", allowZero: true),
            finalizedHeight,
            RequireHashLiteral(root, "finalized_block_hash"));
    }

    private static PrivacyProofManagedPoolStateViewV1 ParseProofManagedPool(
        JsonElement root,
        PrivacyAuthenticatedStateQueryV1 query)
    {
        RequireExactFields(
            root,
            "network_id",
            "protocol_id",
            "pool_id",
            "asset_definition_id",
            "root_role",
            "bootstrap_digest",
            "initial_root",
            "current_epoch",
            "current_root",
            "output_count",
            "bootstrap_admitted_at_height",
            "latest_transition",
            "finalized_height",
            "finalized_block_hash");
        var networkId = RequireExpectedNetwork(root, query);
        var protocolId = RequireProtocol(root, "protocol_id");
        if (ProofManagedProtocolIndex(protocolId) != query.ProtocolIndex)
        {
            throw Invalid("proof-managed protocol binding");
        }
        var poolId = RequireFixed32Array(root, "pool_id");
        RequireBinding(query, poolId);
        var rootRole = RequireRootRole(root, "root_role");
        var expectedRole = protocolId switch
        {
            PrivacyProtocolIdV1.MoneroFcmpPlusPlusV1 => PrivacyFinalizedRootRoleV1.OutputSet,
            PrivacyProtocolIdV1.IrohaIvmPrivateNoteStarkV1 => PrivacyFinalizedRootRoleV1.ProgramState,
            PrivacyProtocolIdV1.PqMaspStarkV0 => PrivacyFinalizedRootRoleV1.NoteCommitmentAnchor,
            _ => throw Invalid("proof-managed protocol"),
        };
        if (rootRole != expectedRole)
        {
            throw Invalid("proof-managed root role");
        }
        var currentEpoch = RequireU64(root, "current_epoch", allowZero: false);
        var outputCount = RequireU64(root, "output_count", allowZero: false);
        var bootstrapHeight = RequireU64(
            root,
            "bootstrap_admitted_at_height",
            allowZero: false);
        var finalizedHeight = RequireU64(root, "finalized_height", allowZero: false);
        if (finalizedHeight < bootstrapHeight)
        {
            throw Invalid("proof-managed finality before bootstrap");
        }
        var transition = RequireOptionalProofManagedTransition(root, "latest_transition");
        if ((currentEpoch == 1) != (transition is null))
        {
            throw Invalid("proof-managed epoch/transition relationship");
        }
        if (transition is not null
            && (transition.SuccessorEpoch != currentEpoch
                || transition.OutputCount > outputCount
                || transition.AdmittedAtHeight > finalizedHeight))
        {
            throw Invalid("proof-managed transition relationship");
        }
        return new PrivacyProofManagedPoolStateViewV1(
            networkId,
            protocolId,
            poolId,
            RequireIdentifierString(root, "asset_definition_id"),
            rootRole,
            RequireFixed32Array(root, "bootstrap_digest"),
            RequireFixed32Array(root, "initial_root"),
            currentEpoch,
            RequireFixed32Array(root, "current_root"),
            outputCount,
            bootstrapHeight,
            transition,
            finalizedHeight,
            RequireHashLiteral(root, "finalized_block_hash"));
    }

    private static PrivacyOrchardPoolStateViewV1 ParseOrchardPool(
        JsonElement root,
        PrivacyAuthenticatedStateQueryV1 query)
    {
        RequireExactFields(
            root,
            "network_id",
            "pool_id",
            "asset_definition_id",
            "public_balance_scope",
            "reserve_account",
            "bootstrap_digest",
            "current_epoch",
            "current_root",
            "tree_size",
            "latest_transition",
            "finalized_height",
            "finalized_block_hash");
        var networkId = RequireExpectedNetwork(root, query);
        var poolId = RequireFixed32Array(root, "pool_id");
        RequireBinding(query, poolId);
        var currentEpoch = RequireU64(root, "current_epoch", allowZero: false);
        var transitions = checked(currentEpoch - 1);
        var treeSize = RequireU64(root, "tree_size", allowZero: true);
        if (treeSize < transitions || treeSize > checked(transitions * 2))
        {
            throw Invalid("Orchard tree size/epoch relationship");
        }
        var finalizedHeight = RequireU64(root, "finalized_height", allowZero: false);
        var transition = RequireOptionalOrchardTransition(root, "latest_transition");
        if ((transitions == 0) != (transition is null))
        {
            throw Invalid("Orchard epoch/transition relationship");
        }
        if (transition is not null
            && (transition.SuccessorEpoch != currentEpoch
                || transition.AdmittedAtHeight > finalizedHeight))
        {
            throw Invalid("Orchard transition relationship");
        }
        return new PrivacyOrchardPoolStateViewV1(
            networkId,
            poolId,
            RequireIdentifierString(root, "asset_definition_id"),
            RequireBalanceScope(root, "public_balance_scope"),
            RequireIdentifierString(root, "reserve_account"),
            RequireFixed32Array(root, "bootstrap_digest"),
            currentEpoch,
            RequireFixed32Array(root, "current_root"),
            treeSize,
            transition,
            finalizedHeight,
            RequireHashLiteral(root, "finalized_block_hash"));
    }

    private static PrivacyOrchardNullifierProvenanceV1 ParseOrchardNullifier(
        JsonElement root,
        PrivacyAuthenticatedStateQueryV1 query)
    {
        RequireExactFields(
            root,
            "network_id",
            "pool_id",
            "nullifier",
            "bootstrap_digest",
            "statement_digest",
            "admitted_at_height",
            "action_index",
            "finalized_height",
            "finalized_block_hash");
        var networkId = RequireExpectedNetwork(root, query);
        var poolId = RequireFixed32Array(root, "pool_id");
        var nullifier = RequireFixed32Array(root, "nullifier");
        RequireBinding(query, poolId, nullifier);
        var admittedAtHeight = RequireU64(root, "admitted_at_height", allowZero: false);
        var finalizedHeight = RequireU64(root, "finalized_height", allowZero: false);
        if (finalizedHeight < admittedAtHeight)
        {
            throw Invalid("Orchard nullifier finality before admission");
        }
        return new PrivacyOrchardNullifierProvenanceV1(
            networkId,
            poolId,
            nullifier,
            RequireFixed32Array(root, "bootstrap_digest"),
            RequireFixed32Array(root, "statement_digest"),
            admittedAtHeight,
            RequireU32(root, "action_index", allowZero: true),
            finalizedHeight,
            RequireHashLiteral(root, "finalized_block_hash"));
    }

    private static PrivacyAnonymousPgcPoolStateViewV1 ParseAnonymousPgcPool(
        JsonElement root,
        PrivacyAuthenticatedStateQueryV1 query)
    {
        RequireExactFields(
            root,
            "network_id",
            "pool_id",
            "total_supply",
            "bootstrap_root",
            "bootstrap_digest",
            "bootstrap_proof_digest",
            "current_epoch",
            "current_root",
            "account_count",
            "current_state_admitted_at_height",
            "latest_transition",
            "finalized_height",
            "finalized_block_hash");
        var networkId = RequireExpectedNetwork(root, query);
        var poolId = RequireFixed32Array(root, "pool_id");
        RequireBinding(query, poolId);
        var totalSupply = RequireU32(root, "total_supply", allowZero: false);
        var bootstrapRoot = RequireFixed32Array(root, "bootstrap_root");
        var currentEpoch = RequireU64(root, "current_epoch", allowZero: false);
        var currentRoot = RequireFixed32Array(root, "current_root");
        var accountCount = RequireU32(root, "account_count", allowZero: false);
        if (accountCount is not (16 or 32 or 64))
        {
            throw Invalid("Anonymous PGC account count");
        }
        var admittedAtHeight = RequireU64(
            root,
            "current_state_admitted_at_height",
            allowZero: false);
        var finalizedHeight = RequireU64(root, "finalized_height", allowZero: false);
        if (finalizedHeight < admittedAtHeight)
        {
            throw Invalid("Anonymous PGC finality before current-state admission");
        }
        var transition = RequireOptionalAnonymousPgcTransition(root, "latest_transition");
        if (currentEpoch == 1)
        {
            if (transition is not null
                || !CryptographicOperations.FixedTimeEquals(currentRoot, bootstrapRoot))
            {
                throw Invalid("Anonymous PGC bootstrap relationship");
            }
        }
        else if (transition is null
            || transition.SuccessorEpoch != currentEpoch
            || transition.AdmittedAtHeight != admittedAtHeight)
        {
            throw Invalid("Anonymous PGC transition relationship");
        }
        return new PrivacyAnonymousPgcPoolStateViewV1(
            networkId,
            poolId,
            totalSupply,
            bootstrapRoot,
            RequireFixed32Array(root, "bootstrap_digest"),
            RequireFixed32Array(root, "bootstrap_proof_digest"),
            currentEpoch,
            currentRoot,
            accountCount,
            admittedAtHeight,
            transition,
            finalizedHeight,
            RequireHashLiteral(root, "finalized_block_hash"));
    }

    private static PrivacyZkAmsAdmissionViewV1 ParseZkAmsAdmission(
        JsonElement root,
        PrivacyAuthenticatedStateQueryV1 query)
    {
        RequireExactFields(
            root,
            "network_id",
            "issuer_id",
            "registry_id",
            "policy_id",
            "phc_hash",
            "seed_public_key",
            "bootstrap_digest",
            "issuer_policy_record_digest",
            "policy_digest",
            "registry_record_digest",
            "parent_epoch",
            "parent_root",
            "anchor_index",
            "batch_size",
            "successor_epoch",
            "successor_root",
            "statement_digest",
            "admitted_at_height",
            "action_index",
            "finalized_height",
            "finalized_block_hash");
        var networkId = RequireExpectedNetwork(root, query);
        var issuerId = RequireFixed32Array(root, "issuer_id");
        var registryId = RequireFixed32Array(root, "registry_id");
        var policyId = RequireFixed32Array(root, "policy_id");
        var phcHash = RequireFixed32Array(root, "phc_hash");
        RequireBinding(query, issuerId, registryId, policyId, phcHash);
        var parentEpoch = RequireU64(root, "parent_epoch", allowZero: false);
        var successorEpoch = RequireU64(root, "successor_epoch", allowZero: false);
        if (parentEpoch == ulong.MaxValue || successorEpoch != parentEpoch + 1)
        {
            throw Invalid("ZK-AMS admission epoch relationship");
        }
        var anchorIndex = RequireU32(root, "anchor_index", allowZero: true);
        var batchSize = RequireU32(root, "batch_size", allowZero: false);
        if (batchSize > 8 || anchorIndex >= batchSize)
        {
            throw Invalid("ZK-AMS admission batch position");
        }
        var admittedAtHeight = RequireU64(root, "admitted_at_height", allowZero: false);
        var finalizedHeight = RequireU64(root, "finalized_height", allowZero: false);
        if (finalizedHeight < admittedAtHeight)
        {
            throw Invalid("ZK-AMS admission finality");
        }
        return new PrivacyZkAmsAdmissionViewV1(
            networkId,
            issuerId,
            registryId,
            policyId,
            phcHash,
            RequireFixed32Array(root, "seed_public_key"),
            RequireFixed32Array(root, "bootstrap_digest"),
            RequireFixed32Array(root, "issuer_policy_record_digest"),
            RequireFixed32Array(root, "policy_digest"),
            RequireFixed32Array(root, "registry_record_digest"),
            parentEpoch,
            RequireFixed32Array(root, "parent_root"),
            anchorIndex,
            batchSize,
            successorEpoch,
            RequireFixed32Array(root, "successor_root"),
            RequireFixed32Array(root, "statement_digest"),
            admittedAtHeight,
            RequireU32(root, "action_index", allowZero: true),
            finalizedHeight,
            RequireHashLiteral(root, "finalized_block_hash"));
    }

    private static PrivacyZkAmsProvisionViewV1 ParseZkAmsProvision(
        JsonElement root,
        PrivacyAuthenticatedStateQueryV1 query)
    {
        RequireExactFields(
            root,
            "network_id",
            "issuer_id",
            "registry_id",
            "policy_id",
            "key_image",
            "account_id",
            "bootstrap_digest",
            "issuer_policy_record_digest",
            "policy_digest",
            "registry_record_digest",
            "registry_epoch",
            "registry_root",
            "statement_digest",
            "admitted_at_height",
            "action_index",
            "finalized_height",
            "finalized_block_hash");
        var networkId = RequireExpectedNetwork(root, query);
        var issuerId = RequireFixed32Array(root, "issuer_id");
        var registryId = RequireFixed32Array(root, "registry_id");
        var policyId = RequireFixed32Array(root, "policy_id");
        var keyImage = RequireFixed32Array(root, "key_image");
        RequireBinding(query, issuerId, registryId, policyId, keyImage);
        var admittedAtHeight = RequireU64(root, "admitted_at_height", allowZero: false);
        var finalizedHeight = RequireU64(root, "finalized_height", allowZero: false);
        if (finalizedHeight < admittedAtHeight)
        {
            throw Invalid("ZK-AMS provision finality");
        }
        return new PrivacyZkAmsProvisionViewV1(
            networkId,
            issuerId,
            registryId,
            policyId,
            keyImage,
            RequireIdentifierString(root, "account_id"),
            RequireFixed32Array(root, "bootstrap_digest"),
            RequireFixed32Array(root, "issuer_policy_record_digest"),
            RequireFixed32Array(root, "policy_digest"),
            RequireFixed32Array(root, "registry_record_digest"),
            RequireU64(root, "registry_epoch", allowZero: false),
            RequireFixed32Array(root, "registry_root"),
            RequireFixed32Array(root, "statement_digest"),
            admittedAtHeight,
            RequireU32(root, "action_index", allowZero: true),
            finalizedHeight,
            RequireHashLiteral(root, "finalized_block_hash"));
    }

    private static PrivacyZkX509CertificateNullifierProvenanceV1 ParseZkX509Nullifier(
        JsonElement root,
        PrivacyAuthenticatedStateQueryV1 query)
    {
        RequireExactFields(
            root,
            "network_id",
            "trust_anchor_id",
            "policy_id",
            "nullifier",
            "trust_anchor_record_digest",
            "trust_anchor_record_epoch",
            "certificate_policy_record_digest",
            "certificate_policy_record_epoch",
            "crl_record_digest",
            "crl_record_epoch",
            "statement_digest",
            "admitted_at_height",
            "action_index",
            "finalized_height",
            "finalized_block_hash");
        var networkId = RequireExpectedNetwork(root, query);
        var trustAnchorId = RequireFixed32Array(root, "trust_anchor_id");
        var policyId = RequireFixed32Array(root, "policy_id");
        var nullifier = RequireFixed32Array(root, "nullifier");
        RequireBinding(query, trustAnchorId, policyId, nullifier);
        var admittedAtHeight = RequireU64(root, "admitted_at_height", allowZero: false);
        var finalizedHeight = RequireU64(root, "finalized_height", allowZero: false);
        if (finalizedHeight < admittedAtHeight)
        {
            throw Invalid("ZK-X509 nullifier finality");
        }
        return new PrivacyZkX509CertificateNullifierProvenanceV1(
            networkId,
            trustAnchorId,
            policyId,
            nullifier,
            RequireFixed32Array(root, "trust_anchor_record_digest"),
            RequireU64(root, "trust_anchor_record_epoch", allowZero: false),
            RequireFixed32Array(root, "certificate_policy_record_digest"),
            RequireU64(root, "certificate_policy_record_epoch", allowZero: false),
            RequireFixed32Array(root, "crl_record_digest"),
            RequireU64(root, "crl_record_epoch", allowZero: false),
            RequireFixed32Array(root, "statement_digest"),
            admittedAtHeight,
            RequireU32(root, "action_index", allowZero: true),
            finalizedHeight,
            RequireHashLiteral(root, "finalized_block_hash"));
    }

    private static PrivacyProofManagedPoolTransitionViewV1?
        RequireOptionalProofManagedTransition(JsonElement root, string field)
    {
        var element = root.GetProperty(field);
        if (element.ValueKind == JsonValueKind.Null)
        {
            return null;
        }
        RequireExactFields(
            element,
            "statement_digest",
            "successor_epoch",
            "admitted_at_height",
            "action_index",
            "nullifier_count",
            "output_count");
        var successorEpoch = RequireU64(element, "successor_epoch", allowZero: false);
        var admittedAtHeight = RequireU64(element, "admitted_at_height", allowZero: false);
        var nullifierCount = RequireU32(element, "nullifier_count", allowZero: false);
        var outputCount = RequireU32(element, "output_count", allowZero: false);
        if (successorEpoch <= 1)
        {
            throw Invalid("proof-managed transition successor epoch");
        }
        return new PrivacyProofManagedPoolTransitionViewV1(
            RequireFixed32Array(element, "statement_digest"),
            successorEpoch,
            admittedAtHeight,
            RequireU32(element, "action_index", allowZero: true),
            nullifierCount,
            outputCount);
    }

    private static PrivacyOrchardPoolTransitionViewV1? RequireOptionalOrchardTransition(
        JsonElement root,
        string field)
    {
        var element = root.GetProperty(field);
        if (element.ValueKind == JsonValueKind.Null)
        {
            return null;
        }
        RequireExactFields(
            element,
            "statement_digest",
            "successor_epoch",
            "parent_epoch",
            "parent_root",
            "admitted_at_height",
            "action_index");
        var parentEpoch = RequireU64(element, "parent_epoch", allowZero: false);
        var successorEpoch = RequireU64(element, "successor_epoch", allowZero: false);
        if (parentEpoch == ulong.MaxValue || successorEpoch != parentEpoch + 1)
        {
            throw Invalid("pool transition epoch relationship");
        }
        return new PrivacyOrchardPoolTransitionViewV1(
            RequireFixed32Array(element, "statement_digest"),
            successorEpoch,
            parentEpoch,
            RequireFixed32Array(element, "parent_root"),
            RequireU64(element, "admitted_at_height", allowZero: false),
            RequireU32(element, "action_index", allowZero: true));
    }

    private static PrivacyAnonymousPgcPoolTransitionViewV1?
        RequireOptionalAnonymousPgcTransition(JsonElement root, string field)
    {
        var element = root.GetProperty(field);
        if (element.ValueKind == JsonValueKind.Null)
        {
            return null;
        }
        RequireExactFields(
            element,
            "statement_digest",
            "successor_epoch",
            "parent_epoch",
            "parent_root",
            "admitted_at_height",
            "action_index");
        var parentEpoch = RequireU64(element, "parent_epoch", allowZero: false);
        var successorEpoch = RequireU64(element, "successor_epoch", allowZero: false);
        if (parentEpoch == ulong.MaxValue || successorEpoch != parentEpoch + 1)
        {
            throw Invalid("Anonymous PGC transition epoch relationship");
        }
        return new PrivacyAnonymousPgcPoolTransitionViewV1(
            RequireFixed32Array(element, "statement_digest"),
            successorEpoch,
            parentEpoch,
            RequireFixed32Array(element, "parent_root"),
            RequireU64(element, "admitted_at_height", allowZero: false),
            RequireU32(element, "action_index", allowZero: true));
    }

    private static NetworkId RequireExpectedNetwork(
        JsonElement root,
        PrivacyAuthenticatedStateQueryV1 query)
    {
        var literal = RequireString(root, "network_id");
        NetworkId networkId;
        try
        {
            networkId = NetworkId.Parse(literal);
        }
        catch (FormatException error)
        {
            throw new InvalidDataException(
                "Authenticated privacy state query returned a non-canonical NetworkId.",
                error);
        }
        if (networkId != query.NetworkId)
        {
            throw Invalid("NetworkId binding");
        }
        return networkId;
    }

    private static PrivacyProtocolIdV1 RequireProtocol(JsonElement root, string field)
    {
        var element = root.GetProperty(field);
        RequireExactFields(element, "protocol", "value");
        if (element.GetProperty("value").ValueKind != JsonValueKind.Null)
        {
            throw Invalid("privacy protocol unit value");
        }
        try
        {
            return PrivacyProtocolsV1.ParseCanonicalLabel(
                RequireString(element, "protocol"));
        }
        catch (ArgumentException error)
        {
            throw new InvalidDataException(
                "Authenticated privacy state query returned an unsupported protocol.",
                error);
        }
    }

    private static PrivacyFinalizedRootRoleV1 RequireRootRole(
        JsonElement root,
        string field)
    {
        var element = root.GetProperty(field);
        RequireExactFields(element, "role", "value");
        if (element.GetProperty("value").ValueKind != JsonValueKind.Null)
        {
            throw Invalid("privacy root-role unit value");
        }
        return RequireString(element, "role") switch
        {
            "PgcAccountState" => PrivacyFinalizedRootRoleV1.PgcAccountState,
            "AccountRegistry" => PrivacyFinalizedRootRoleV1.AccountRegistry,
            "Revocation" => PrivacyFinalizedRootRoleV1.Revocation,
            "CertificateAuthorityMembership" =>
                PrivacyFinalizedRootRoleV1.CertificateAuthorityMembership,
            "NoteCommitmentAnchor" => PrivacyFinalizedRootRoleV1.NoteCommitmentAnchor,
            "OutputSet" => PrivacyFinalizedRootRoleV1.OutputSet,
            "ProgramState" => PrivacyFinalizedRootRoleV1.ProgramState,
            _ => throw Invalid("privacy root role"),
        };
    }

    private static PrivacyFinalizedAssetBalanceScopeV1 RequireBalanceScope(
        JsonElement root,
        string field)
    {
        var element = root.GetProperty(field);
        if (element.ValueKind != JsonValueKind.Object)
        {
            throw Invalid("asset balance scope");
        }
        RequireExactFields(element, "kind", "content");
        var kind = RequireString(element, "kind");
        if (string.Equals(kind, "Global", StringComparison.Ordinal))
        {
            if (element.GetProperty("content").ValueKind != JsonValueKind.Null)
            {
                throw Invalid("global asset balance scope content");
            }
            return PrivacyFinalizedAssetBalanceScopeV1.Global;
        }
        if (!string.Equals(kind, "Dataspace", StringComparison.Ordinal)
            || element.GetProperty("content").ValueKind != JsonValueKind.String)
        {
            throw Invalid("asset balance scope variant");
        }
        var id = RequireCanonicalU64String(
            element.GetProperty("content"),
            "asset balance scope content",
            allowZero: false);
        return PrivacyFinalizedAssetBalanceScopeV1.Dataspace(id);
    }

    private static byte[] RequireFixed32Array(JsonElement root, string field)
    {
        var element = root.GetProperty(field);
        if (element.ValueKind != JsonValueKind.Array || element.GetArrayLength() != 32)
        {
            throw Invalid($"{field} fixed32 byte array");
        }
        var output = new byte[32];
        var index = 0;
        foreach (var item in element.EnumerateArray())
        {
            if (item.ValueKind != JsonValueKind.Number
                || !item.TryGetByte(out var value)
                || !string.Equals(
                    item.GetRawText(),
                    value.ToString(CultureInfo.InvariantCulture),
                    StringComparison.Ordinal))
            {
                throw Invalid($"{field} byte value");
            }
            output[index++] = value;
        }
        if (!Array.Exists(output, static value => value != 0))
        {
            throw Invalid($"{field} nonzero fixed32 value");
        }
        return output;
    }

    private static byte[] RequireHashLiteral(JsonElement root, string field)
    {
        var literal = RequireString(root, field);
        if (literal.Length != 74
            || !literal.StartsWith("hash:", StringComparison.Ordinal)
            || literal[69] != '#')
        {
            throw Invalid($"{field} canonical hash literal");
        }
        var body = literal.Substring(5, 64);
        var checksum = literal.Substring(70, 4);
        if (body.Any(static value => value is not (>= '0' and <= '9' or >= 'A' and <= 'F'))
            || checksum.Any(static value => value is not (>= '0' and <= '9' or >= 'A' and <= 'F'))
            || !ushort.TryParse(
                checksum,
                NumberStyles.HexNumber,
                CultureInfo.InvariantCulture,
                out var supplied)
            || supplied != Crc16(Encoding.ASCII.GetBytes($"hash:{body}")))
        {
            throw Invalid($"{field} canonical hash checksum");
        }
        var output = Convert.FromHexString(body);
        if (!Array.Exists(output, static value => value != 0))
        {
            throw Invalid($"{field} nonzero hash");
        }
        return output;
    }

    private static ulong RequireU64(JsonElement root, string field, bool allowZero) =>
        RequireCanonicalU64String(root.GetProperty(field), field, allowZero);

    private static uint RequireU32(JsonElement root, string field, bool allowZero)
    {
        var value = RequireCanonicalU64String(root.GetProperty(field), field, allowZero);
        if (value > uint.MaxValue)
        {
            throw Invalid($"{field} u32 range");
        }
        return (uint)value;
    }

    private static ulong RequireCanonicalU64String(
        JsonElement element,
        string field,
        bool allowZero)
    {
        if (element.ValueKind != JsonValueKind.String)
        {
            throw Invalid($"{field} canonical decimal string");
        }
        var text = element.GetString();
        if (string.IsNullOrEmpty(text)
            || (text.Length > 1 && text[0] == '0')
            || text.Any(static value => value is < '0' or > '9')
            || !ulong.TryParse(text, NumberStyles.None, CultureInfo.InvariantCulture, out var value)
            || (!allowZero && value == 0))
        {
            throw Invalid($"{field} canonical unsigned integer");
        }
        return value;
    }

    private static string RequireIdentifierString(JsonElement root, string field)
    {
        var value = RequireString(root, field);
        if (value.Length == 0
            || Encoding.UTF8.GetByteCount(value) > 1_024
            || !string.Equals(value, value.Trim(), StringComparison.Ordinal)
            || value.Any(char.IsControl))
        {
            throw Invalid($"{field} canonical identifier");
        }
        return value;
    }

    private static string RequireString(JsonElement root, string field)
    {
        if (!root.TryGetProperty(field, out var element)
            || element.ValueKind != JsonValueKind.String)
        {
            throw Invalid($"{field} string");
        }
        return element.GetString() ?? throw Invalid($"{field} string");
    }

    private static void RequireBinding(
        PrivacyAuthenticatedStateQueryV1 query,
        params byte[][] chunks)
    {
        var expected = ConcatFixed32(chunks);
        var requested = query.RequestBinding;
        try
        {
            if (!CryptographicOperations.FixedTimeEquals(expected, requested))
            {
                throw Invalid("request selector binding");
            }
        }
        finally
        {
            CryptographicOperations.ZeroMemory(expected);
        }
    }

    private static void RequireExactFields(JsonElement root, params string[] expected)
    {
        if (root.ValueKind != JsonValueKind.Object)
        {
            throw Invalid("object projection");
        }
        var fields = new HashSet<string>(StringComparer.Ordinal);
        foreach (var property in root.EnumerateObject())
        {
            if (!fields.Add(property.Name))
            {
                throw Invalid("duplicate projection field");
            }
        }
        if (!fields.SetEquals(expected))
        {
            throw Invalid("projection field inventory");
        }
    }

    private static ushort Crc16(ReadOnlySpan<byte> value)
    {
        var crc = 0xffff;
        foreach (var item in value)
        {
            crc ^= item << 8;
            for (var bit = 0; bit < 8; bit++)
            {
                crc = (crc & 0x8000) != 0
                    ? ((crc << 1) ^ 0x1021) & 0xffff
                    : (crc << 1) & 0xffff;
            }
        }
        return (ushort)crc;
    }

    private static InvalidDataException Invalid(string field) =>
        new($"Authenticated privacy state-query projection has invalid {field}.");
}
