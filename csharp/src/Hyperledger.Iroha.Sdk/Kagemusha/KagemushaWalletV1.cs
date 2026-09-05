namespace Hyperledger.Iroha.Kagemusha;

/// <summary>Hardware properties required together by every production KAGEMUSHA V1 provider.</summary>
public enum KagemushaHardwareCapabilityV1
{
    ExactNextPredecessorConsumption,
    OneUseSuccessorAuthorization,
    RollbackResistantCounterAndJournal,
    SealedTransitionRecovery,
    ReceiverBoundCreditCommit,
    RollbackResistantAcceptedCreditInbox,
    AuthenticatedInboundStaging,
    AuthoritativeReplayRootRecovery,
    SenderOutboxReservation,
    AuthenticatedDurableRetryOutbox,
    AtomicVerifiedCandidateCommit,
    RecoverableTerminalCommitCertificate,
    TrustedTimeOrLease,
    OfflineHardwareEpochRotation,
    RollbackSafeCounterRollover,
    NoSoftwareFallback,
}

/// <summary>Authenticated release and credential returned by the audited native core.</summary>
public sealed class KagemushaHardwareQualificationV1
{
    private readonly byte[] releaseId;
    private readonly byte[] hardwarePolicyDigest;
    private readonly byte[] coreAuthorizationKeyReference;
    private readonly HashSet<KagemushaHardwareCapabilityV1> capabilities;

    public KagemushaHardwareQualificationV1(
        ushort protocolVersion,
        KagemushaHardwareProfileV1 profile,
        KagemushaHardwareCredentialV1 credential,
        ReadOnlySpan<byte> releaseId,
        ReadOnlySpan<byte> hardwarePolicyDigest,
        ReadOnlySpan<byte> coreAuthorizationKeyReference,
        IEnumerable<KagemushaHardwareCapabilityV1> capabilities)
    {
        ArgumentNullException.ThrowIfNull(profile);
        ArgumentNullException.ThrowIfNull(credential);
        ArgumentNullException.ThrowIfNull(capabilities);
        ProtocolVersion = protocolVersion;
        Profile = profile;
        Credential = credential;
        this.releaseId = KagemushaModelValidation.Fixed32(releaseId, nameof(releaseId));
        this.hardwarePolicyDigest = KagemushaModelValidation.Fixed32(
            hardwarePolicyDigest, nameof(hardwarePolicyDigest));
        this.coreAuthorizationKeyReference = KagemushaModelValidation.Fixed32(
            coreAuthorizationKeyReference, nameof(coreAuthorizationKeyReference));
        this.capabilities = [.. capabilities];
    }

    public ushort ProtocolVersion { get; }
    public KagemushaHardwareProfileV1 Profile { get; }
    public KagemushaHardwareCredentialV1 Credential { get; }
    public byte[] ReleaseId() => releaseId.ToArray();
    public byte[] HardwarePolicyDigest() => hardwarePolicyDigest.ToArray();
    public byte[] CoreAuthorizationKeyReference() => coreAuthorizationKeyReference.ToArray();
    public IReadOnlySet<KagemushaHardwareCapabilityV1> Capabilities => capabilities.ToHashSet();

    /// <summary>Reject a partial, old, expired, or software-backed provider.</summary>
    public void RequireProductionReady()
    {
        if (ProtocolVersion != Kagemusha.WireVersion
            || Profile.Version != ProtocolVersion
            || Profile.ProtocolVersion != ProtocolVersion
            || Credential.Version != ProtocolVersion
            || !Profile.HardwareProfileId.Span.SequenceEqual(Credential.HardwareProfileId.Span)
            || Profile.PolicyEpoch != Credential.PolicyEpoch
            || capabilities.Count != Enum.GetValues<KagemushaHardwareCapabilityV1>().Length
            || Enum.GetValues<KagemushaHardwareCapabilityV1>().Any(value => !capabilities.Contains(value)))
        {
            throw new InvalidOperationException(
                "KAGEMUSHA V1 requires the complete qualified non-forking native hardware capability set.");
        }
    }
}

public sealed class KagemushaHardwareRecoveryV1
{
    private readonly byte[]? aggregateState;

    public KagemushaHardwareRecoveryV1(
        ReadOnlySpan<byte> aggregateState,
        UInt128 journalRevision,
        UInt128 pendingCreditCount,
        UInt128 retryOutboxCount)
        : this(aggregateState.ToArray(), journalRevision, pendingCreditCount, retryOutboxCount)
    {
    }

    public KagemushaHardwareRecoveryV1(
        byte[]? aggregateState,
        UInt128 journalRevision,
        UInt128 pendingCreditCount,
        UInt128 retryOutboxCount)
    {
        this.aggregateState = aggregateState?.ToArray();
        JournalRevision = journalRevision;
        PendingCreditCount = pendingCreditCount;
        RetryOutboxCount = retryOutboxCount;
    }

    public UInt128 JournalRevision { get; }
    public UInt128 PendingCreditCount { get; }
    public UInt128 RetryOutboxCount { get; }
    public byte[]? AggregateState() => aggregateState?.ToArray();
}

public enum KagemushaHardwareStageDispositionV1
{
    Staged,
    ExactDuplicate,
}

public sealed class KagemushaHardwarePaymentStageV1
{
    private readonly byte[] creditId;
    private readonly byte[] acknowledgement;

    public KagemushaHardwarePaymentStageV1(
        KagemushaHardwareStageDispositionV1 disposition,
        ReadOnlySpan<byte> creditId,
        ReadOnlySpan<byte> acknowledgement)
    {
        if (acknowledgement.IsEmpty || acknowledgement.Length > Kagemusha.MaximumAcknowledgementBytes)
            throw new ArgumentOutOfRangeException(nameof(acknowledgement));
        Disposition = disposition;
        this.creditId = KagemushaModelValidation.Fixed32(creditId, nameof(creditId));
        this.acknowledgement = acknowledgement.ToArray();
    }

    public KagemushaHardwareStageDispositionV1 Disposition { get; }
    public byte[] CreditId() => creditId.ToArray();
    public byte[] Acknowledgement() => acknowledgement.ToArray();
}

public sealed class KagemushaHardwareMintStageV1
{
    private readonly byte[] creditId;

    public KagemushaHardwareMintStageV1(
        KagemushaHardwareStageDispositionV1 disposition,
        ReadOnlySpan<byte> creditId)
    {
        Disposition = disposition;
        this.creditId = KagemushaModelValidation.Fixed32(creditId, nameof(creditId));
    }

    public KagemushaHardwareStageDispositionV1 Disposition { get; }
    public byte[] CreditId() => creditId.ToArray();
}

public sealed class KagemushaHardwareTerminalResultV1
{
    private readonly byte[] canonicalEnvelope;
    private readonly byte[] aggregateState;

    public KagemushaHardwareTerminalResultV1(
        ReadOnlySpan<byte> canonicalEnvelope,
        ReadOnlySpan<byte> aggregateState)
    {
        if (canonicalEnvelope.IsEmpty || aggregateState.IsEmpty)
            throw new ArgumentException("Native terminal output cannot be empty.");
        this.canonicalEnvelope = canonicalEnvelope.ToArray();
        this.aggregateState = aggregateState.ToArray();
    }

    public byte[] CanonicalEnvelope() => canonicalEnvelope.ToArray();
    public byte[] AggregateState() => aggregateState.ToArray();
}

/// <summary>Authenticated inbox that owns a pending monetary credit.</summary>
public enum KagemushaPendingCreditKindV1
{
    Mint,
    Receive,
}

/// <summary>One globally ordered pending mint or peer credit.</summary>
public sealed class KagemushaPendingCreditSelectorV1
{
    private readonly byte[] creditId;

    public KagemushaPendingCreditSelectorV1(
        KagemushaPendingCreditKindV1 kind,
        ReadOnlySpan<byte> creditId)
    {
        if (!Enum.IsDefined(kind)) throw new ArgumentOutOfRangeException(nameof(kind));
        Kind = kind;
        this.creditId = KagemushaModelValidation.Fixed32(creditId, nameof(creditId));
    }

    public KagemushaPendingCreditKindV1 Kind { get; }
    public byte[] CreditId() => creditId.ToArray();

    public bool Matches(KagemushaPendingCreditSelectorV1 other) =>
        other is not null
        && Kind == other.Kind
        && creditId.AsSpan().SequenceEqual(other.creditId);
}

/// <summary>Epoch-qualified inclusive inbox boundary for one finite selection pass.</summary>
public sealed class KagemushaPendingCreditWatermarkV1
{
    private readonly byte[] hardwareEpochId;

    public KagemushaPendingCreditWatermarkV1(
        UInt128 hardwareEpochGeneration,
        ReadOnlySpan<byte> hardwareEpochId,
        UInt128 inboxRevision)
    {
        if (hardwareEpochGeneration == 0)
            throw new ArgumentOutOfRangeException(nameof(hardwareEpochGeneration));
        HardwareEpochGeneration = hardwareEpochGeneration;
        this.hardwareEpochId = KagemushaModelValidation.Fixed32(
            hardwareEpochId, nameof(hardwareEpochId));
        InboxRevision = inboxRevision;
    }

    public UInt128 HardwareEpochGeneration { get; }
    public byte[] HardwareEpochId() => hardwareEpochId.ToArray();
    public UInt128 InboxRevision { get; }

    public bool Matches(KagemushaPendingCreditWatermarkV1 other) =>
        other is not null
        && HardwareEpochGeneration == other.HardwareEpochGeneration
        && InboxRevision == other.InboxRevision
        && hardwareEpochId.AsSpan().SequenceEqual(other.hardwareEpochId);
}

/// <summary>Amount-aware objective for one pending-credit selection.</summary>
public sealed class KagemushaPendingCreditTargetV1
{
    private KagemushaPendingCreditTargetV1(bool drainAll, UInt128 requiredBalance)
    {
        if (!drainAll && requiredBalance == 0)
            throw new ArgumentOutOfRangeException(nameof(requiredBalance));
        IsDrainAll = drainAll;
        Amount = requiredBalance;
    }

    public static KagemushaPendingCreditTargetV1 DrainAll { get; } = new(true, 0);
    public static KagemushaPendingCreditTargetV1 RequiredBalance(UInt128 amount) =>
        new(false, amount);

    public bool IsDrainAll { get; }
    public UInt128 Amount { get; }
}

/// <summary>Authenticated hardware result for operation 18.</summary>
public sealed class KagemushaPendingCreditSelectionV1
{
    public KagemushaPendingCreditSelectionV1(
        KagemushaPendingCreditWatermarkV1 watermark,
        KagemushaPendingCreditSelectorV1? nextPending)
    {
        ArgumentNullException.ThrowIfNull(watermark);
        Watermark = watermark;
        NextPending = nextPending;
    }

    public KagemushaPendingCreditWatermarkV1 Watermark { get; }
    public KagemushaPendingCreditSelectorV1? NextPending { get; }
}

/// <summary>One exact-next transition that folds one durably staged credit.</summary>
public sealed class KagemushaHardwareReceiveFoldV1
{
    private readonly byte[] aggregateState;

    public KagemushaHardwareReceiveFoldV1(
        ReadOnlySpan<byte> aggregateState,
        KagemushaPendingCreditSelectorV1 selector)
    {
        if (aggregateState.IsEmpty)
            throw new ArgumentException("Native aggregate state cannot be empty.", nameof(aggregateState));
        ArgumentNullException.ThrowIfNull(selector);
        this.aggregateState = aggregateState.ToArray();
        Selector = selector;
    }

    public byte[] AggregateState() => aggregateState.ToArray();
    public KagemushaPendingCreditSelectorV1 Selector { get; }
}

/// <summary>Public result of installing one received credit into the aggregate state.</summary>
public sealed class KagemushaReceiveFoldResultV1
{
    public KagemushaReceiveFoldResultV1(
        KagemushaAggregateStateCommitmentV1 aggregateState,
        KagemushaPendingCreditSelectorV1 selector)
    {
        ArgumentNullException.ThrowIfNull(aggregateState);
        ArgumentNullException.ThrowIfNull(selector);
        AggregateState = aggregateState;
        Selector = selector;
    }

    public KagemushaAggregateStateCommitmentV1 AggregateState { get; }
    public KagemushaPendingCreditSelectorV1 Selector { get; }
}

/// <summary>
/// Complete immutable hardware-produced mint authorization and encrypted credit.
/// The host can validate and forward these bytes but cannot synthesize either value.
/// </summary>
public sealed class KagemushaMintConstructionBundleV1
{
    private readonly byte[] canonicalAuthorization;
    private readonly byte[] encryptedCredit;

    public KagemushaMintConstructionBundleV1(
        ReadOnlySpan<byte> canonicalAuthorization,
        ReadOnlySpan<byte> encryptedCredit)
    {
        if (canonicalAuthorization.IsEmpty || encryptedCredit.IsEmpty)
            throw new ArgumentException("KAGEMUSHA mint construction bytes cannot be empty.");
        Authorization = Kagemusha.DecodeMintAuthorization(canonicalAuthorization);
        _ = Kagemusha.DecodeEncryptedCreditEnvelope(encryptedCredit);
        if (!Authorization.Statement.CiphertextDigest.Span.SequenceEqual(
                Kagemusha.CiphertextDigest(encryptedCredit)))
            throw new ArgumentException("KAGEMUSHA mint encrypted-credit digest differs from authorization.");
        this.canonicalAuthorization = canonicalAuthorization.ToArray();
        this.encryptedCredit = encryptedCredit.ToArray();
    }

    public KagemushaMintAuthorizationV1 Authorization { get; }
    public byte[] CanonicalAuthorization() => canonicalAuthorization.ToArray();
    public byte[] EncryptedCredit() => encryptedCredit.ToArray();

    /// <summary>Build the exact reserve-facing request without replacing hardware-owned bytes.</summary>
    public KagemushaTopUpRequestV1 TopUpRequest(KagemushaHardwareCredentialV1 hardwareCredential)
    {
        ArgumentNullException.ThrowIfNull(hardwareCredential);
        var statement = Authorization.Statement;
        var context = statement.Context;
        if (!context.HardwareCredentialId.Span.SequenceEqual(hardwareCredential.CredentialId.Span)
            || !context.HardwareProfileId.Span.SequenceEqual(hardwareCredential.HardwareProfileId.Span)
            || !context.SuiteId.Span.SequenceEqual(hardwareCredential.SuiteId.Span)
            || context.PolicyEpoch != hardwareCredential.PolicyEpoch)
            throw new InvalidOperationException("KAGEMUSHA mint bundle does not match hardware qualification.");
        return new KagemushaTopUpRequestV1(
            Kagemusha.WireVersion,
            context.OperationId,
            statement.IssuanceCommitment,
            statement.CreditId,
            context.ReleaseId,
            context.SuiteId,
            context.VkDigest,
            context.NetworkId,
            context.Asset,
            context.AssetIncarnation,
            context.Scale,
            context.Amount,
            context.LiabilityPoolId,
            context.Payer,
            context.Recipient,
            hardwareCredential,
            context.RecipientCredentialCommitment,
            context.CreditCommitment,
            context.RecipientOneTimeKey,
            encryptedCredit,
            context.ArtifactManifestDigest,
            Authorization);
    }
}

public sealed class KagemushaStagedPaymentV1
{
    private readonly byte[] canonicalAcknowledgement;

    public KagemushaStagedPaymentV1(
        KagemushaHardwareStageDispositionV1 disposition,
        KagemushaAcknowledgementV1 acknowledgement,
        ReadOnlySpan<byte> canonicalAcknowledgement)
    {
        Disposition = disposition;
        Acknowledgement = acknowledgement;
        this.canonicalAcknowledgement = canonicalAcknowledgement.ToArray();
    }

    public KagemushaHardwareStageDispositionV1 Disposition { get; }
    public KagemushaAcknowledgementV1 Acknowledgement { get; }
    public byte[] CanonicalAcknowledgement() => canonicalAcknowledgement.ToArray();
}

/// <summary>
/// Mandatory audited-native-core and non-forking secure-device boundary.
/// Implementations must never fall back to managed cryptography, process memory, or application files.
/// </summary>
public interface IKagemushaNativeHardwareProviderV1
{
    KagemushaHardwareQualificationV1 Qualification();
    KagemushaHardwareRecoveryV1 Recover();
    byte[] BootstrapState();
    byte[] CreatePaymentRequest(
        byte[] operationId,
        byte[] recipientAccount,
        UInt128 amount,
        ulong validityWindowMilliseconds);
    /// <summary>Idempotently persist the exact caller-owned ID and request parameters.</summary>
    byte[] ReservePaymentRequestOperationId(
        byte[] operationId, byte[] recipientAccount, UInt128 amount,
        ulong validityWindowMilliseconds);
    /// <summary>
    /// Durably stage one credit and recover the same ACK on retry. Native inbox counters are
    /// independent of the monetary journal; staging must not advance <see cref="JournalRevision"/>.
    /// </summary>
    KagemushaHardwarePaymentStageV1 StagePayment(
        byte[] canonicalRequest,
        byte[] canonicalPayment);
    /// <summary>Stage without folding the balance or advancing the monetary journal.</summary>
    KagemushaHardwareMintStageV1 StageMintCredit(byte[] canonicalAuthorization, byte[] canonicalMintCredit);
    KagemushaPendingCreditSelectionV1 SelectPendingCredit(
        KagemushaPendingCreditWatermarkV1? watermark,
        KagemushaPendingCreditTargetV1 target);
    /// <summary>Return the monetary transition revision, never the independent inbox revision.</summary>
    UInt128 JournalRevision();
    KagemushaHardwareReceiveFoldV1 FoldPendingCredit(KagemushaPendingCreditSelectorV1 selector);
    byte[] ReservePaymentOperationId(byte[] operationId, byte[] canonicalRequest);
    /// <summary>
    /// Prepare and verify the candidate, commit exactly once, then generate and persist the
    /// post-commit proof and payment under the caller-persisted operation identity.
    /// </summary>
    KagemushaHardwareTerminalResultV1 CommitPayment(byte[] operationId, byte[] canonicalRequest);
    byte[]? RecoverPayment(byte[] creditId);
    byte[]? RecoverPaymentByOperationId(byte[] operationId, byte[] canonicalRequest);
    byte[] ReserveMintOperationId(byte[] operationId, UInt128 amount, byte[] payerAccount, byte[] recipientAccount);
    KagemushaMintConstructionBundleV1 PrepareMintConstructionBundle(
        byte[] operationId,
        UInt128 amount,
        byte[] payerAccount,
        byte[] recipientAccount);
    KagemushaMintConstructionBundleV1? RecoverMintConstructionBundle(byte[] operationId);
    void RecordAcknowledgement(
        byte[] creditId,
        byte[] canonicalRequest,
        byte[] canonicalPayment,
        byte[] canonicalAcknowledgement);
    /// <summary>
    /// Atomically prepare, prove, commit, and persist one redemption under the caller-persisted
    /// operation identity.
    /// </summary>
    byte[] ReserveRedemptionOperationId(byte[] operationId, UInt128 amount, byte[] beneficiaryAccount);
    KagemushaHardwareTerminalResultV1 CommitRedemption(
        byte[] operationId,
        UInt128 amount,
        byte[] beneficiaryAccount);
    byte[]? RecoverRedemption(byte[] redemptionId);
    byte[]? RecoverRedemptionByOperationId(byte[] operationId);
    /// <summary>Rotate the complete balance, replay root, and pending inbox without folding first.</summary>
    byte[] RotateHardwareEpoch();
}

/// <summary>Aggregate recursive-balance orchestration over the mandatory native provider.</summary>
public sealed class KagemushaWalletV1
{
    private readonly object transitionLock = new();
    private int waitingForegroundTransitions;
    private readonly IKagemushaNativeHardwareProviderV1 provider;
    private KagemushaHardwareQualificationV1 qualification;
    private KagemushaAggregateStateCommitmentV1 aggregateState;
    private UInt128 journalRevision;

    private KagemushaWalletV1(
        IKagemushaNativeHardwareProviderV1 provider,
        KagemushaHardwareQualificationV1 qualification,
        KagemushaAggregateStateCommitmentV1 aggregateState,
        UInt128 journalRevision)
    {
        this.provider = provider;
        this.qualification = qualification;
        this.aggregateState = aggregateState;
        this.journalRevision = journalRevision;
    }

    public KagemushaHardwareCredentialV1 HardwareCredential
    {
        get { lock (transitionLock) return qualification.Credential; }
    }

    public KagemushaAggregateStateCommitmentV1 AggregateState
    {
        get { lock (transitionLock) return aggregateState; }
    }

    public UInt128 JournalRevision
    {
        get { lock (transitionLock) return journalRevision; }
    }

    /// <summary>Open only after complete native qualification and recovery succeed.</summary>
    public static KagemushaWalletV1 Open(IKagemushaNativeHardwareProviderV1 provider)
    {
        ArgumentNullException.ThrowIfNull(provider);
        var snapshot = RecoverAuthoritativeSnapshot(provider, allowBootstrap: true);
        return new KagemushaWalletV1(
            provider, snapshot.Qualification, snapshot.State, snapshot.Recovery.JournalRevision);
    }

    public KagemushaHardwareRecoveryV1 Recover()
    {
        using (EnterForegroundTransition())
        {
            var snapshot = RecoverAuthoritativeSnapshot(provider, allowBootstrap: false);
            var recovery = snapshot.Recovery;
            var state = snapshot.State;
            var nextQualification = snapshot.Qualification;
            var revision = recovery.JournalRevision;
            RequireSameAsset(aggregateState, state, includingRelease: false);
            if (state.HardwareEpochId.Span.SequenceEqual(aggregateState.HardwareEpochId.Span))
            {
                if (nextQualification.Credential.HardwareEpochGeneration
                        != qualification.Credential.HardwareEpochGeneration
                    || !state.KeyReference.Span.SequenceEqual(aggregateState.KeyReference.Span)
                    || revision < journalRevision)
                    throw new InvalidOperationException("Native recovery rolled back the hardware epoch or journal.");
                if (revision == journalRevision
                    && !Kagemusha.EncodeAggregateState(state).AsSpan().SequenceEqual(
                        Kagemusha.EncodeAggregateState(aggregateState)))
                    throw new InvalidOperationException("Native recovery equivocated at the same journal revision.");
            }
            else if (nextQualification.Credential.HardwareEpochGeneration
                <= qualification.Credential.HardwareEpochGeneration)
                throw new InvalidOperationException("Native recovery did not advance the authenticated hardware epoch.");

            // Keep the published snapshot intact until every native response has been checked.
            qualification = nextQualification;
            aggregateState = state;
            journalRevision = revision;
            return new KagemushaHardwareRecoveryV1(
                Kagemusha.EncodeAggregateState(state),
                revision,
                recovery.PendingCreditCount,
                recovery.RetryOutboxCount);
        }
    }

    public KagemushaPaymentRequestV1 CreatePaymentRequest(
        ReadOnlySpan<byte> operationId,
        KagemushaAccountIdV1 recipient,
        UInt128 amount,
        ulong validityWindowMilliseconds)
    {
        ArgumentNullException.ThrowIfNull(recipient);
        if (amount == 0) throw new ArgumentOutOfRangeException(nameof(amount));
        if (validityWindowMilliseconds is 0 or > Kagemusha.RequestMaximumTtlMilliseconds)
            throw new ArgumentOutOfRangeException(nameof(validityWindowMilliseconds));
        var expected = KagemushaModelValidation.Fixed32(operationId, nameof(operationId));
        using (EnterForegroundTransition())
        {
            _ = ReservePaymentRequestOperationId(expected, recipient, amount, validityWindowMilliseconds);
            var request = Kagemusha.DecodePaymentRequest(provider.CreatePaymentRequest(
                expected.ToArray(), recipient.CanonicalPayload(), amount, validityWindowMilliseconds));
            if (!request.RequestId.Span.SequenceEqual(expected)
                || !request.Recipient.Equals(recipient)
                || request.Amount != amount
                || request.ExpiresAtMilliseconds - request.IssuedAtMilliseconds != validityWindowMilliseconds)
                throw new InvalidOperationException("Native request output does not match the requested amount or lifetime.");
            RequireStateRequestBinding(aggregateState, request);
            return request;
        }
    }

    /// <summary>Persist only after the caller has saved this ID and its exact request parameters.</summary>
    public byte[] ReservePaymentRequestOperationId(
        ReadOnlySpan<byte> operationId, KagemushaAccountIdV1 recipient,
        UInt128 amount, ulong validityWindowMilliseconds)
    {
        ArgumentNullException.ThrowIfNull(recipient);
        if (amount == 0) throw new ArgumentOutOfRangeException(nameof(amount));
        if (validityWindowMilliseconds is 0 or > Kagemusha.RequestMaximumTtlMilliseconds)
            throw new ArgumentOutOfRangeException(nameof(validityWindowMilliseconds));
        return ReserveChecked(operationId, expected => provider.ReservePaymentRequestOperationId(
            expected, recipient.CanonicalPayload(), amount, validityWindowMilliseconds));
    }

    /// <summary>Persist the exact identity the caller saved before a sender mutation.</summary>
    public byte[] ReservePaymentOperationId(
        ReadOnlySpan<byte> operationId, KagemushaPaymentRequestV1 request)
    {
        ArgumentNullException.ThrowIfNull(request);
        var canonicalRequest = Kagemusha.EncodePaymentRequest(request);
        return ReserveChecked(operationId,
            expected => provider.ReservePaymentOperationId(expected, canonicalRequest));
    }

    /// <summary>Fold accepted credits, then commit a payment under a caller-persisted identity.</summary>
    public KagemushaPaymentV1 Send(
        KagemushaPaymentRequestV1 request,
        ReadOnlySpan<byte> operationId)
    {
        ArgumentNullException.ThrowIfNull(request);
        var expectedOperationId = KagemushaModelValidation.Fixed32(operationId, nameof(operationId));
        using (EnterForegroundTransition())
        {
            FoldRequiredCreditsLocked(request.Amount);
            var canonicalRequest = Kagemusha.EncodePaymentRequest(request);
            var result = provider.CommitPayment(expectedOperationId, canonicalRequest);
            var payment = Kagemusha.DecodePayment(result.CanonicalEnvelope(), request);
            InstallAuthoritativeState(result.AggregateState());
            return payment;
        }
    }

    public KagemushaStagedPaymentV1 StagePayment(
        KagemushaPaymentRequestV1 request,
        KagemushaPaymentV1 payment)
    {
        ArgumentNullException.ThrowIfNull(request);
        ArgumentNullException.ThrowIfNull(payment);
        using (EnterForegroundTransition())
        {
            var canonicalRequest = Kagemusha.EncodePaymentRequest(request);
            var canonicalPayment = Kagemusha.EncodePayment(payment, request);
            var before = provider.JournalRevision();
            var staged = provider.StagePayment(canonicalRequest, canonicalPayment);
            if (!staged.CreditId().AsSpan().SequenceEqual(payment.Output.CreditId.Span))
                throw new InvalidOperationException("Native staging returned a different credit id.");
            var canonicalAcknowledgement = staged.Acknowledgement();
            var acknowledgement = Kagemusha.DecodeAcknowledgement(
                canonicalAcknowledgement, request, payment);
            var after = provider.JournalRevision();
            RequireStagingJournalUnchanged(before, after);
            journalRevision = after;
            return new KagemushaStagedPaymentV1(
                staged.Disposition, acknowledgement, canonicalAcknowledgement);
        }
    }

    public KagemushaHardwareStageDispositionV1 StageMintCredit(
        KagemushaMintAuthorizationV1 authorization,
        KagemushaMintCreditV1 mintCredit)
    {
        using (EnterForegroundTransition())
        {
            var canonicalAuthorization = Kagemusha.EncodeMintAuthorization(authorization);
            var canonicalCredit = Kagemusha.EncodeMintCredit(mintCredit, authorization);
            var before = provider.JournalRevision();
            var staged = provider.StageMintCredit(canonicalAuthorization, canonicalCredit);
            if (!staged.CreditId().AsSpan().SequenceEqual(mintCredit.Statement.Lifecycle.CreditId.Span))
                throw new InvalidOperationException("Native mint staging returned a different credit id.");
            var after = provider.JournalRevision();
            RequireStagingJournalUnchanged(before, after);
            journalRevision = after;
            return staged.Disposition;
        }
    }

    /// <summary>Fold exactly one authenticated mint or peer selector.</summary>
    public KagemushaReceiveFoldResultV1 FoldPendingCredit(KagemushaPendingCreditSelectorV1 selector)
    {
        ArgumentNullException.ThrowIfNull(selector);
        using (EnterForegroundTransition()) return FoldPendingCreditLocked(selector);
    }

    /// <summary>
    /// Drain one epoch-bound inbox snapshot, yielding to queued foreground work between credits.
    /// If concurrent recovery or rotation changes the epoch, retry this operation with a new snapshot;
    /// credits already folded remain installed and the old watermark is never reused.
    /// </summary>
    /// <exception cref="InvalidOperationException">The epoch changed; retry to capture a new snapshot.</exception>
    public UInt128 DrainPendingCredits()
    {
        byte[] epochId;
        ulong epochGeneration;
        lock (transitionLock)
        {
            YieldToForegroundLocked();
            epochId = qualification.Credential.HardwareEpochId.ToArray();
            epochGeneration = qualification.Credential.HardwareEpochGeneration;
        }
        UInt128 total = 0;
        KagemushaPendingCreditWatermarkV1? watermark = null;
        while (true)
        {
            lock (transitionLock)
            {
                YieldToForegroundLocked();
                if (qualification.Credential.HardwareEpochGeneration != epochGeneration
                    || !qualification.Credential.HardwareEpochId.Span.SequenceEqual(epochId))
                    throw new InvalidOperationException(
                        "Hardware epoch changed during inbox drain; retry with a new snapshot.");
                var selection = provider.SelectPendingCredit(
                    watermark, KagemushaPendingCreditTargetV1.DrainAll);
                RequireCurrentWatermark(selection.Watermark);
                if (watermark is not null && !selection.Watermark.Matches(watermark))
                    throw new InvalidOperationException(
                        "Native pending-credit watermark changed during inbox drain.");
                watermark = selection.Watermark;
                if (selection.NextPending is null) return total;
                _ = FoldPendingCreditLocked(selection.NextPending);
                total = checked(total + 1);
            }
        }
    }

    public KagemushaPaymentV1? RecoverPayment(
        KagemushaPaymentRequestV1 request,
        ReadOnlySpan<byte> creditId)
    {
        ArgumentNullException.ThrowIfNull(request);
        var expected = KagemushaModelValidation.Fixed32(creditId, nameof(creditId));
        var canonical = provider.RecoverPayment(expected);
        if (canonical is null) return null;
        var payment = Kagemusha.DecodePayment(canonical, request);
        if (!payment.Output.CreditId.Span.SequenceEqual(expected))
            throw new InvalidOperationException("Recovered payment has a different credit id.");
        return payment;
    }

    /// <summary>Recover a payment after a crash before its credit ID reached the caller.</summary>
    public KagemushaPaymentV1? RecoverPaymentByOperationId(
        KagemushaPaymentRequestV1 request,
        ReadOnlySpan<byte> operationId)
    {
        ArgumentNullException.ThrowIfNull(request);
        var expected = KagemushaModelValidation.Fixed32(operationId, nameof(operationId));
        var canonicalRequest = Kagemusha.EncodePaymentRequest(request);
        var canonical = provider.RecoverPaymentByOperationId(expected, canonicalRequest);
        if (canonical is null) return null;
        var payment = Kagemusha.DecodePayment(canonical, request);
        if (!Kagemusha.EncodePayment(payment, request).AsSpan().SequenceEqual(canonical))
            throw new InvalidOperationException("Recovered payment is not byte-identical.");
        return payment;
    }

    /// <summary>Persist the exact identity the caller saved before mint preparation.</summary>
    public byte[] ReserveMintOperationId(
        ReadOnlySpan<byte> operationId,
        UInt128 amount,
        KagemushaAccountIdV1 payer,
        KagemushaAccountIdV1 recipient)
    {
        if (amount == 0) throw new ArgumentOutOfRangeException(nameof(amount));
        ArgumentNullException.ThrowIfNull(payer);
        ArgumentNullException.ThrowIfNull(recipient);
        return ReserveChecked(operationId, expected => provider.ReserveMintOperationId(
            expected, amount, payer.CanonicalPayload(), recipient.CanonicalPayload()));
    }

    public KagemushaMintConstructionBundleV1 PrepareMintConstructionBundle(
        ReadOnlySpan<byte> operationId,
        UInt128 amount,
        KagemushaAccountIdV1 payer,
        KagemushaAccountIdV1 recipient)
    {
        if (amount == 0) throw new ArgumentOutOfRangeException(nameof(amount));
        ArgumentNullException.ThrowIfNull(payer);
        ArgumentNullException.ThrowIfNull(recipient);
        var expected = KagemushaModelValidation.Fixed32(operationId, nameof(operationId));
        var bundle = provider.PrepareMintConstructionBundle(
            expected,
            amount,
            payer.CanonicalPayload(),
            recipient.CanonicalPayload());
        if (!bundle.Authorization.Statement.Context.OperationId.Span.SequenceEqual(expected))
            throw new InvalidOperationException("Native mint bundle substituted the operation ID.");
        return bundle;
    }

    public KagemushaMintConstructionBundleV1? RecoverMintConstructionBundle(
        ReadOnlySpan<byte> operationId)
    {
        var expected = KagemushaModelValidation.Fixed32(operationId, nameof(operationId));
        var bundle = provider.RecoverMintConstructionBundle(expected);
        if (bundle is not null
            && !bundle.Authorization.Statement.Context.OperationId.Span.SequenceEqual(expected))
            throw new InvalidOperationException("Recovered mint bundle substituted the operation ID.");
        return bundle;
    }

    /// <summary>Construct a reserve request only from the hardware-owned mint bundle.</summary>
    public KagemushaTopUpRequestV1 PrepareTopUpRequest(
        ReadOnlySpan<byte> operationId,
        UInt128 amount,
        KagemushaAccountIdV1 payer,
        KagemushaAccountIdV1 recipient) =>
        PrepareMintConstructionBundle(operationId, amount, payer, recipient)
            .TopUpRequest(qualification.Credential);

    public void RecordAcknowledgement(
        KagemushaPaymentRequestV1 request,
        KagemushaPaymentV1 payment,
        KagemushaAcknowledgementV1 acknowledgement)
    {
        ArgumentNullException.ThrowIfNull(request);
        ArgumentNullException.ThrowIfNull(payment);
        ArgumentNullException.ThrowIfNull(acknowledgement);
        var canonicalRequest = Kagemusha.EncodePaymentRequest(request);
        var canonicalPayment = Kagemusha.EncodePayment(payment, request);
        var canonicalAcknowledgement = Kagemusha.EncodeAcknowledgement(
            acknowledgement, request, payment);
        _ = Kagemusha.ValidateCompleteExchange(request, payment, acknowledgement);
        provider.RecordAcknowledgement(
            payment.Output.CreditId.ToArray(),
            canonicalRequest,
            canonicalPayment,
            canonicalAcknowledgement);
    }

    /// <summary>Persist the exact identity the caller saved before redemption.</summary>
    public byte[] ReserveRedemptionOperationId(
        ReadOnlySpan<byte> operationId,
        UInt128 amount,
        KagemushaAccountIdV1 beneficiary)
    {
        if (amount == 0) throw new ArgumentOutOfRangeException(nameof(amount));
        ArgumentNullException.ThrowIfNull(beneficiary);
        return ReserveChecked(operationId, expected => provider.ReserveRedemptionOperationId(
            expected, amount, beneficiary.CanonicalPayload()));
    }

    public KagemushaRedemptionVoucherV1 Redeem(
        UInt128 amount,
        KagemushaAccountIdV1 beneficiary,
        ReadOnlySpan<byte> operationId)
    {
        if (amount == 0) throw new ArgumentOutOfRangeException(nameof(amount));
        ArgumentNullException.ThrowIfNull(beneficiary);
        var expectedOperationId = KagemushaModelValidation.Fixed32(operationId, nameof(operationId));
        using (EnterForegroundTransition())
        {
            FoldRequiredCreditsLocked(amount);
            var result = provider.CommitRedemption(
                expectedOperationId,
                amount,
                beneficiary.CanonicalPayload());
            var voucher = Kagemusha.DecodeRedemptionVoucher(result.CanonicalEnvelope());
            if (voucher.Statement.Amount != amount || !voucher.Statement.Beneficiary.Equals(beneficiary))
                throw new InvalidOperationException("Native redemption output differs from the request.");
            InstallAuthoritativeState(result.AggregateState());
            return voucher;
        }
    }

    public KagemushaRedemptionVoucherV1? RecoverRedemption(ReadOnlySpan<byte> redemptionId)
    {
        var expected = KagemushaModelValidation.Fixed32(redemptionId, nameof(redemptionId));
        var canonical = provider.RecoverRedemption(expected);
        if (canonical is null) return null;
        var voucher = Kagemusha.DecodeRedemptionVoucher(canonical);
        if (!voucher.Statement.RedemptionId.Span.SequenceEqual(expected))
            throw new InvalidOperationException("Recovered redemption has a different id.");
        return voucher;
    }

    /// <summary>Recover a redemption after a crash before its terminal ID reached the caller.</summary>
    public KagemushaRedemptionVoucherV1? RecoverRedemptionByOperationId(
        ReadOnlySpan<byte> operationId)
    {
        var expected = KagemushaModelValidation.Fixed32(operationId, nameof(operationId));
        var canonical = provider.RecoverRedemptionByOperationId(expected);
        if (canonical is null) return null;
        var voucher = Kagemusha.DecodeRedemptionVoucher(canonical);
        if (!Kagemusha.EncodeRedemptionVoucher(voucher).AsSpan().SequenceEqual(canonical))
            throw new InvalidOperationException("Recovered redemption is not byte-identical.");
        return voucher;
    }

    /// <summary>Rotate even when the old epoch's counters are saturated and credits remain pending.</summary>
    public KagemushaAggregateStateCommitmentV1 RotateHardwareEpoch()
    {
        using (EnterForegroundTransition())
        {
            var previousState = aggregateState;
            var previousCredential = qualification.Credential;
            if (previousCredential.HardwareEpochGeneration == ulong.MaxValue)
                throw new InvalidOperationException("Hardware epoch generation is exhausted.");

            // Pending receipts must survive rotation: folding first would overflow the old
            // epoch's journal or state sequence precisely when rollover is required.
            var state = Kagemusha.DecodeAggregateState(provider.RotateHardwareEpoch());
            var nextQualification = RequireQualified(provider.Qualification());
            var credential = nextQualification.Credential;
            var revision = provider.JournalRevision();
            RequireSameAsset(previousState, state);
            RequireStateQualification(state, nextQualification);
            if (credential.NetworkId != previousCredential.NetworkId
                || !credential.LaneCommitment.Span.SequenceEqual(previousCredential.LaneCommitment.Span)
                || credential.HardwareEpochGeneration != previousCredential.HardwareEpochGeneration + 1
                || credential.HardwareEpochId.Span.SequenceEqual(previousCredential.HardwareEpochId.Span)
                || state.StateCommitment.Span.SequenceEqual(previousState.StateCommitment.Span)
                || state.Sequence != 0
                || revision != 0)
                throw new InvalidOperationException("Hardware rotation did not return an exact next epoch with reset counters.");

            // Publish the host cache only after every returned binding has been validated.
            qualification = nextQualification;
            aggregateState = state;
            journalRevision = revision;
            return state;
        }
    }

    private KagemushaReceiveFoldResultV1 FoldPendingCreditLocked(
        KagemushaPendingCreditSelectorV1 selector)
    {
        var before = journalRevision;
        var previousState = aggregateState;
        var hardwareFold = provider.FoldPendingCredit(selector);
        if (!hardwareFold.Selector.Matches(selector))
            throw new InvalidOperationException("Native pending fold substituted the selector.");
        var after = provider.JournalRevision();
        var successor = Kagemusha.DecodeAggregateState(hardwareFold.AggregateState());
        RequireSameAsset(previousState, successor);
        RequireStateQualification(successor, qualification);
        if (successor.StateCommitment.Span.SequenceEqual(previousState.StateCommitment.Span)
            || previousState.Sequence == UInt128.MaxValue
            || successor.Sequence != previousState.Sequence + 1)
            throw new InvalidOperationException("A receive fold did not produce an exact next aggregate state.");
        if (before == UInt128.MaxValue || after != before + 1)
            throw new InvalidOperationException("A receive fold did not consume exactly one journal revision.");

        // No validation or provider call follows publication, and every reader holds this gate.
        aggregateState = successor;
        journalRevision = after;
        return new KagemushaReceiveFoldResultV1(aggregateState, selector);
    }

    private void FoldRequiredCreditsLocked(UInt128 requiredBalance)
    {
        while (true)
        {
            var selection = provider.SelectPendingCredit(
                null, KagemushaPendingCreditTargetV1.RequiredBalance(requiredBalance));
            RequireCurrentWatermark(selection.Watermark);
            if (selection.NextPending is null) return;
            _ = FoldPendingCreditLocked(selection.NextPending);
        }
    }

    private void RequireCurrentWatermark(KagemushaPendingCreditWatermarkV1 watermark)
    {
        ArgumentNullException.ThrowIfNull(watermark);
        if (watermark.HardwareEpochGeneration
                != (UInt128)qualification.Credential.HardwareEpochGeneration
            || !watermark.HardwareEpochId().AsSpan().SequenceEqual(
                qualification.Credential.HardwareEpochId.Span))
            throw new InvalidOperationException(
                "Native pending-credit watermark belongs to another hardware epoch.");
    }

    private void InstallAuthoritativeState(byte[] bytes)
    {
        var state = Kagemusha.DecodeAggregateState(bytes);
        RequireSameAsset(aggregateState, state);
        RequireStateQualification(state, qualification);
        var revision = provider.JournalRevision();
        aggregateState = state;
        journalRevision = revision;
    }

    private ForegroundTransitionLease EnterForegroundTransition()
    {
        // Register before Monitor.Enter so a draining thread cannot repeatedly overtake this work.
        Interlocked.Increment(ref waitingForegroundTransitions);
        try { Monitor.Enter(transitionLock); }
        catch
        {
            CancelQueuedForegroundTransition();
            throw;
        }
        Interlocked.Decrement(ref waitingForegroundTransitions);
        return new ForegroundTransitionLease(transitionLock);
    }

    private void CancelQueuedForegroundTransition()
    {
        // Deregister under the same gate as the drain's check-and-wait, then wake it. Removing
        // a failed entrant outside this gate could leave the drain asleep without a lease to pulse.
        while (true)
        {
            try { Monitor.Enter(transitionLock); break; }
            catch (ThreadInterruptedException)
            {
                // Finish queue cleanup even if interrupted again; the original entry error wins.
            }
        }
        try
        {
            Interlocked.Decrement(ref waitingForegroundTransitions);
            Monitor.PulseAll(transitionLock);
        }
        finally { Monitor.Exit(transitionLock); }
    }

    private void YieldToForegroundLocked()
    {
        while (Volatile.Read(ref waitingForegroundTransitions) != 0)
            Monitor.Wait(transitionLock);
    }

    private readonly struct ForegroundTransitionLease(object gate) : IDisposable
    {
        public void Dispose()
        {
            Monitor.PulseAll(gate);
            Monitor.Exit(gate);
        }
    }

    private static KagemushaHardwareQualificationV1 RequireQualified(
        KagemushaHardwareQualificationV1 value)
    {
        ArgumentNullException.ThrowIfNull(value);
        value.RequireProductionReady();
        return value;
    }

    private static byte[] ReserveChecked(ReadOnlySpan<byte> operationId, Func<byte[], byte[]> reserve)
    {
        var expected = KagemushaModelValidation.Fixed32(operationId, nameof(operationId));
        // Give the provider a copy so it cannot rewrite the expected identity during its call.
        var returned = KagemushaModelValidation.Fixed32(reserve(expected.ToArray()).AsSpan(), "reservedOperationId");
        if (!returned.AsSpan().SequenceEqual(expected))
            throw new InvalidOperationException("Native reservation substituted the caller-owned operation ID.");
        return expected;
    }

    private static (KagemushaHardwareQualificationV1 Qualification,
        KagemushaAggregateStateCommitmentV1 State, KagemushaHardwareRecoveryV1 Recovery)
        RecoverAuthoritativeSnapshot(IKagemushaNativeHardwareProviderV1 provider, bool allowBootstrap)
    {
        _ = RequireQualified(provider.Qualification());
        var recovery = provider.Recover();
        var qualification = RequireQualified(provider.Qualification());
        var stateBytes = recovery.AggregateState();
        if (stateBytes is null)
        {
            if (!allowBootstrap)
                throw new InvalidOperationException("Native recovery lost an existing aggregate state.");
            var bootstrapped = provider.BootstrapState().ToArray();
            recovery = provider.Recover();
            qualification = RequireQualified(provider.Qualification());
            stateBytes = recovery.AggregateState();
            if (stateBytes is null || !stateBytes.AsSpan().SequenceEqual(bootstrapped))
                throw new InvalidOperationException("Native bootstrap differs from its durable recovery snapshot.");
        }
        var state = Kagemusha.DecodeAggregateState(stateBytes);
        RequireStateQualification(state, qualification);
        if (provider.JournalRevision() != recovery.JournalRevision)
            throw new InvalidOperationException("Native recovery journal revision changed during recovery.");
        return (qualification, state, recovery);
    }

    private static void RequireStagingJournalUnchanged(UInt128 before, UInt128 after)
    {
        // Inbox exact-next/replay accounting is native-authoritative and separate from the
        // monetary journal. Staging (including mint staging) is not a balance-fold transition.
        if (after != before)
            throw new InvalidOperationException("Native inbox staging changed the monetary journal revision.");
    }

    private static void RequireStateQualification(
        KagemushaAggregateStateCommitmentV1 state,
        KagemushaHardwareQualificationV1 qualification)
    {
        if (!state.ReleaseId.Span.SequenceEqual(qualification.ReleaseId())
            || state.NetworkId != qualification.Credential.NetworkId
            || !state.LaneId.Span.SequenceEqual(qualification.Credential.LaneCommitment.Span)
            || !state.HardwareEpochId.Span.SequenceEqual(qualification.Credential.HardwareEpochId.Span)
            || !state.KeyReference.Span.SequenceEqual(qualification.Credential.DeviceKeyReference.Span)
            || !state.HardwarePolicyId.Span.SequenceEqual(qualification.HardwarePolicyDigest()))
            throw new InvalidOperationException("Aggregate state does not match native qualification.");
    }

    private static void RequireStateRequestBinding(
        KagemushaAggregateStateCommitmentV1 state,
        KagemushaPaymentRequestV1 request)
    {
        if (!request.ReleaseId.Span.SequenceEqual(state.ReleaseId.Span)
            || request.NetworkId != state.NetworkId
            || !request.Asset.Equals(state.Asset)
            || !request.AssetIncarnation.Equals(state.AssetIncarnation)
            || request.Scale != state.Scale
            || !request.LiabilityPoolId.Span.SequenceEqual(state.LiabilityPoolId.Span))
            throw new InvalidOperationException("Payment request does not match the aggregate state.");
    }

    private static void RequireSameAsset(
        KagemushaAggregateStateCommitmentV1 before,
        KagemushaAggregateStateCommitmentV1 after,
        bool includingRelease = true)
    {
        if (after.Version != before.Version
            || (includingRelease && !after.ReleaseId.Span.SequenceEqual(before.ReleaseId.Span))
            || after.NetworkId != before.NetworkId
            || !after.Asset.Equals(before.Asset)
            || !after.AssetIncarnation.Equals(before.AssetIncarnation)
            || after.Scale != before.Scale
            || !after.LiabilityPoolId.Span.SequenceEqual(before.LiabilityPoolId.Span)
            || !after.LaneId.Span.SequenceEqual(before.LaneId.Span))
            throw new InvalidOperationException("Native successor changed the aggregate asset lane.");
    }
}
