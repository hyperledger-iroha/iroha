namespace Hyperledger.Iroha.Kagemusha;

/// <summary>Hardware properties required together by every production KAGEMUSHA V1 provider.</summary>
public enum KagemushaHardwareCapabilityV1
{
    ExactNextPredecessorConsumption,
    OneUseSuccessorAuthorization,
    RollbackResistantCounterAndJournal,
    SealedTransitionRecovery,
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
    private readonly HashSet<KagemushaHardwareCapabilityV1> capabilities;

    public KagemushaHardwareQualificationV1(
        ushort protocolVersion,
        KagemushaHardwareProfileV1 profile,
        KagemushaHardwareCredentialV1 credential,
        ReadOnlySpan<byte> releaseId,
        IEnumerable<KagemushaHardwareCapabilityV1> capabilities)
    {
        ArgumentNullException.ThrowIfNull(profile);
        ArgumentNullException.ThrowIfNull(credential);
        ArgumentNullException.ThrowIfNull(capabilities);
        ProtocolVersion = protocolVersion;
        Profile = profile;
        Credential = credential;
        this.releaseId = KagemushaModelValidation.Fixed32(releaseId, nameof(releaseId));
        this.capabilities = [.. capabilities];
    }

    public ushort ProtocolVersion { get; }
    public KagemushaHardwareProfileV1 Profile { get; }
    public KagemushaHardwareCredentialV1 Credential { get; }
    public byte[] ReleaseId() => releaseId.ToArray();
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

/// <summary>One exact-next transition that folds one durably staged credit.</summary>
public sealed class KagemushaHardwareReceiveFoldV1
{
    private readonly byte[] aggregateState;

    public KagemushaHardwareReceiveFoldV1(ReadOnlySpan<byte> aggregateState)
    {
        if (aggregateState.IsEmpty)
            throw new ArgumentException("Native aggregate state cannot be empty.", nameof(aggregateState));
        this.aggregateState = aggregateState.ToArray();
    }

    public byte[] AggregateState() => aggregateState.ToArray();
}

/// <summary>Public result of installing one received credit into the aggregate state.</summary>
public sealed class KagemushaReceiveFoldResultV1
{
    public KagemushaReceiveFoldResultV1(KagemushaAggregateStateCommitmentV1 aggregateState)
    {
        ArgumentNullException.ThrowIfNull(aggregateState);
        AggregateState = aggregateState;
    }

    public KagemushaAggregateStateCommitmentV1 AggregateState { get; }
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
        byte[] recipientAccount,
        UInt128 amount,
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
    UInt128 PendingCreditWatermark();
    /// <summary>Return the monetary transition revision, never the independent inbox revision.</summary>
    UInt128 JournalRevision();
    KagemushaHardwareReceiveFoldV1? FoldReceiveCredit(UInt128 inboxSequenceInclusive);
    /// <summary>
    /// Fold only required credits, prepare and verify the candidate, commit exactly once, then
    /// generate and persist the post-commit proof and payment. Unrelated inbox backlog remains.
    /// </summary>
    KagemushaHardwareTerminalResultV1 CommitPayment(byte[] canonicalRequest);
    byte[]? RecoverPayment(byte[] creditId);
    void RecordAcknowledgement(
        byte[] creditId,
        byte[] canonicalRequest,
        byte[] canonicalPayment,
        byte[] canonicalAcknowledgement);
    /// <summary>
    /// Fold only credits needed for <paramref name="amount"/>, then atomically prepare, prove,
    /// commit, and persist one redemption. Unrelated inbox backlog remains for background folding.
    /// </summary>
    KagemushaHardwareTerminalResultV1 CommitRedemption(UInt128 amount, byte[] beneficiaryAccount);
    byte[]? RecoverRedemption(byte[] redemptionId);
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
        _ = RequireQualified(provider.Qualification());
        var recovery = provider.Recover();
        // Recovery may complete a committed epoch rotation; bind its state to the recovered tuple.
        var qualification = RequireQualified(provider.Qualification());
        var stateBytes = recovery.AggregateState() ?? provider.BootstrapState();
        var state = Kagemusha.DecodeAggregateState(stateBytes);
        RequireStateQualification(state, qualification);
        if (provider.JournalRevision() != recovery.JournalRevision)
            throw new InvalidOperationException("Native recovery journal revision changed while opening the wallet.");
        return new KagemushaWalletV1(provider, qualification, state, recovery.JournalRevision);
    }

    public KagemushaHardwareRecoveryV1 Recover()
    {
        using (EnterForegroundTransition())
        {
            _ = RequireQualified(provider.Qualification());
            var recovery = provider.Recover();
            var nextQualification = RequireQualified(provider.Qualification());
            var stateBytes = recovery.AggregateState() ?? provider.BootstrapState();
            var state = Kagemusha.DecodeAggregateState(stateBytes);
            RequireStateQualification(state, nextQualification);
            var revision = provider.JournalRevision();
            if (revision != recovery.JournalRevision)
                throw new InvalidOperationException("Native recovery journal revision changed during recovery.");
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
        KagemushaAccountIdV1 recipient,
        UInt128 amount,
        ulong validityWindowMilliseconds)
    {
        ArgumentNullException.ThrowIfNull(recipient);
        if (amount == 0) throw new ArgumentOutOfRangeException(nameof(amount));
        if (validityWindowMilliseconds is 0 or > Kagemusha.RequestMaximumTtlMilliseconds)
            throw new ArgumentOutOfRangeException(nameof(validityWindowMilliseconds));
        using (EnterForegroundTransition())
        {
            var request = Kagemusha.DecodePaymentRequest(provider.CreatePaymentRequest(
                recipient.CanonicalPayload(), amount, validityWindowMilliseconds));
            if (!request.Recipient.Equals(recipient)
                || request.Amount != amount
                || request.ExpiresAtMilliseconds - request.IssuedAtMilliseconds != validityWindowMilliseconds)
                throw new InvalidOperationException("Native request output does not match the requested amount or lifetime.");
            RequireStateRequestBinding(aggregateState, request);
            return request;
        }
    }

    public KagemushaPaymentV1 Send(KagemushaPaymentRequestV1 request)
    {
        ArgumentNullException.ThrowIfNull(request);
        using (EnterForegroundTransition())
        {
            var canonicalRequest = Kagemusha.EncodePaymentRequest(request);
            var result = provider.CommitPayment(canonicalRequest);
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

    /// <summary>Fold one durably staged credit, if one is pending.</summary>
    public KagemushaReceiveFoldResultV1? FoldReceiveCredit()
    {
        using (EnterForegroundTransition()) return FoldAtWatermarkLocked(provider.PendingCreditWatermark());
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
        UInt128 watermark;
        lock (transitionLock)
        {
            YieldToForegroundLocked();
            epochId = qualification.Credential.HardwareEpochId.ToArray();
            epochGeneration = qualification.Credential.HardwareEpochGeneration;
            watermark = provider.PendingCreditWatermark();
        }
        UInt128 total = 0;
        while (true)
        {
            lock (transitionLock)
            {
                YieldToForegroundLocked();
                if (qualification.Credential.HardwareEpochGeneration != epochGeneration
                    || !qualification.Credential.HardwareEpochId.Span.SequenceEqual(epochId))
                    throw new InvalidOperationException(
                        "Hardware epoch changed during inbox drain; retry with a new snapshot.");
                var fold = FoldAtWatermarkLocked(watermark);
                if (fold is null) return total;
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

    public KagemushaRedemptionVoucherV1 Redeem(UInt128 amount, KagemushaAccountIdV1 beneficiary)
    {
        if (amount == 0) throw new ArgumentOutOfRangeException(nameof(amount));
        ArgumentNullException.ThrowIfNull(beneficiary);
        using (EnterForegroundTransition())
        {
            var result = provider.CommitRedemption(amount, beneficiary.CanonicalPayload());
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

    private KagemushaReceiveFoldResultV1? FoldAtWatermarkLocked(UInt128 watermark)
    {
        var before = journalRevision;
        var previousState = aggregateState;
        var hardwareFold = provider.FoldReceiveCredit(watermark);
        var after = provider.JournalRevision();
        KagemushaAggregateStateCommitmentV1? successor = null;
        if (hardwareFold is not null)
        {
            successor = Kagemusha.DecodeAggregateState(hardwareFold.AggregateState());
            RequireSameAsset(previousState, successor);
            RequireStateQualification(successor, qualification);
            if (successor.StateCommitment.Span.SequenceEqual(previousState.StateCommitment.Span)
                || previousState.Sequence == UInt128.MaxValue
                || successor.Sequence != previousState.Sequence + 1)
                throw new InvalidOperationException("A receive fold did not produce an exact next aggregate state.");
        }
        if (hardwareFold is null ? after != before : before == UInt128.MaxValue || after != before + 1)
            throw new InvalidOperationException("A receive fold did not consume exactly one journal revision.");

        // No validation or provider call follows publication, and every reader holds this gate.
        if (successor is not null) aggregateState = successor;
        journalRevision = after;
        return hardwareFold is null
            ? null
            : new KagemushaReceiveFoldResultV1(aggregateState);
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
            || !state.HardwarePolicyId.Span.SequenceEqual(qualification.Profile.HardwareProfileId.Span))
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
        KagemushaAggregateStateCommitmentV1 after)
    {
        if (after.Version != before.Version
            || !after.ReleaseId.Span.SequenceEqual(before.ReleaseId.Span)
            || after.NetworkId != before.NetworkId
            || !after.Asset.Equals(before.Asset)
            || !after.AssetIncarnation.Equals(before.AssetIncarnation)
            || after.Scale != before.Scale
            || !after.LiabilityPoolId.Span.SequenceEqual(before.LiabilityPoolId.Span)
            || !after.LaneId.Span.SequenceEqual(before.LaneId.Span))
            throw new InvalidOperationException("Native successor changed the aggregate asset lane.");
    }
}
