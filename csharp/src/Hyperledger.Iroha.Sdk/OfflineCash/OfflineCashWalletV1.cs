namespace Hyperledger.Iroha.OfflineCash;

/// <summary>Hardware properties required together by every production Offline Cash V1 provider.</summary>
public enum OfflineCashHardwareCapabilityV1
{
    ExactNextPredecessorConsumption,
    OneUseSuccessorAuthorization,
    RollbackResistantCounterAndJournal,
    SealedTransitionRecovery,
    OneUseAcceptanceTickets,
    DurableInboxReservation,
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
public sealed class OfflineCashHardwareQualificationV1
{
    private readonly byte[] releaseId;
    private readonly HashSet<OfflineCashHardwareCapabilityV1> capabilities;

    public OfflineCashHardwareQualificationV1(
        ushort protocolVersion,
        OfflineCashHardwareProfileV1 profile,
        OfflineCashHardwareCredentialV1 credential,
        ReadOnlySpan<byte> releaseId,
        IEnumerable<OfflineCashHardwareCapabilityV1> capabilities)
    {
        ArgumentNullException.ThrowIfNull(profile);
        ArgumentNullException.ThrowIfNull(credential);
        ArgumentNullException.ThrowIfNull(capabilities);
        ProtocolVersion = protocolVersion;
        Profile = profile;
        Credential = credential;
        this.releaseId = OfflineCashModelValidation.Fixed32(releaseId, nameof(releaseId));
        this.capabilities = [.. capabilities];
    }

    public ushort ProtocolVersion { get; }
    public OfflineCashHardwareProfileV1 Profile { get; }
    public OfflineCashHardwareCredentialV1 Credential { get; }
    public byte[] ReleaseId() => releaseId.ToArray();
    public IReadOnlySet<OfflineCashHardwareCapabilityV1> Capabilities => capabilities.ToHashSet();

    /// <summary>Reject a partial, old, expired, or software-backed provider.</summary>
    public void RequireProductionReady()
    {
        if (ProtocolVersion != OfflineCashV1.WireVersion
            || Profile.Version != ProtocolVersion
            || Profile.ProtocolVersion != ProtocolVersion
            || Credential.Version != ProtocolVersion
            || !Profile.HardwareProfileId.Span.SequenceEqual(Credential.HardwareProfileId.Span)
            || Profile.PolicyEpoch != Credential.PolicyEpoch
            || capabilities.Count != Enum.GetValues<OfflineCashHardwareCapabilityV1>().Length
            || Enum.GetValues<OfflineCashHardwareCapabilityV1>().Any(value => !capabilities.Contains(value)))
        {
            throw new InvalidOperationException(
                "Offline Cash V1 requires the complete qualified non-forking native hardware capability set.");
        }
    }
}

public sealed class OfflineCashHardwareRecoveryV1
{
    private readonly byte[]? aggregateState;

    public OfflineCashHardwareRecoveryV1(
        ReadOnlySpan<byte> aggregateState,
        UInt128 journalRevision,
        UInt128 pendingCreditCount,
        UInt128 retryOutboxCount)
        : this(aggregateState.ToArray(), journalRevision, pendingCreditCount, retryOutboxCount)
    {
    }

    public OfflineCashHardwareRecoveryV1(
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

public enum OfflineCashHardwareStageDispositionV1
{
    Staged,
    ExactDuplicate,
}

public sealed class OfflineCashHardwarePaymentStageV1
{
    private readonly byte[] creditId;
    private readonly byte[] acknowledgement;

    public OfflineCashHardwarePaymentStageV1(
        OfflineCashHardwareStageDispositionV1 disposition,
        ReadOnlySpan<byte> creditId,
        ReadOnlySpan<byte> acknowledgement)
    {
        if (acknowledgement.IsEmpty || acknowledgement.Length > OfflineCashV1.MaximumAcknowledgementBytes)
            throw new ArgumentOutOfRangeException(nameof(acknowledgement));
        Disposition = disposition;
        this.creditId = OfflineCashModelValidation.Fixed32(creditId, nameof(creditId));
        this.acknowledgement = acknowledgement.ToArray();
    }

    public OfflineCashHardwareStageDispositionV1 Disposition { get; }
    public byte[] CreditId() => creditId.ToArray();
    public byte[] Acknowledgement() => acknowledgement.ToArray();
}

public sealed class OfflineCashHardwareMintStageV1
{
    private readonly byte[] creditId;

    public OfflineCashHardwareMintStageV1(
        OfflineCashHardwareStageDispositionV1 disposition,
        ReadOnlySpan<byte> creditId)
    {
        Disposition = disposition;
        this.creditId = OfflineCashModelValidation.Fixed32(creditId, nameof(creditId));
    }

    public OfflineCashHardwareStageDispositionV1 Disposition { get; }
    public byte[] CreditId() => creditId.ToArray();
}

public sealed class OfflineCashHardwareFoldBatchV1
{
    private readonly byte[]? aggregateState;

    public OfflineCashHardwareFoldBatchV1(int foldedCredits, byte[]? aggregateState)
    {
        if (foldedCredits is < 0 or > 16 || (foldedCredits == 0) != (aggregateState is null))
            throw new ArgumentException("A fixed Offline Cash V1 fold batch must contain zero through sixteen credits.");
        FoldedCredits = foldedCredits;
        this.aggregateState = aggregateState?.ToArray();
    }

    public int FoldedCredits { get; }
    public byte[]? AggregateState() => aggregateState?.ToArray();
}

public sealed class OfflineCashHardwareTerminalResultV1
{
    private readonly byte[] canonicalEnvelope;
    private readonly byte[] aggregateState;

    public OfflineCashHardwareTerminalResultV1(
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

public sealed class OfflineCashStagedPaymentV1
{
    private readonly byte[] canonicalAcknowledgement;

    public OfflineCashStagedPaymentV1(
        OfflineCashHardwareStageDispositionV1 disposition,
        OfflineCashAcknowledgementV1 acknowledgement,
        ReadOnlySpan<byte> canonicalAcknowledgement)
    {
        Disposition = disposition;
        Acknowledgement = acknowledgement;
        this.canonicalAcknowledgement = canonicalAcknowledgement.ToArray();
    }

    public OfflineCashHardwareStageDispositionV1 Disposition { get; }
    public OfflineCashAcknowledgementV1 Acknowledgement { get; }
    public byte[] CanonicalAcknowledgement() => canonicalAcknowledgement.ToArray();
}

/// <summary>
/// Mandatory audited-native-core and non-forking secure-device boundary.
/// Implementations must never fall back to managed cryptography, process memory, or application files.
/// </summary>
public interface IOfflineCashNativeHardwareProviderV1
{
    OfflineCashHardwareQualificationV1 Qualification();
    OfflineCashHardwareRecoveryV1 Recover();
    byte[] BootstrapState();
    byte[] CreatePaymentRequest(byte[] recipientAccount, byte[] requestMode, ulong validityWindowMilliseconds);
    byte[] CreateAcceptanceIntentAuthorization(byte[] canonicalRequest, UInt128 exactAmount);
    byte[] IssueAcceptanceTicket(byte[] canonicalRequest, byte[] canonicalAuthorization);
    OfflineCashHardwarePaymentStageV1 StagePayment(byte[] canonicalRequest, byte[] canonicalPayment);
    OfflineCashHardwareMintStageV1 StageMintCredit(byte[] canonicalAuthorization, byte[] canonicalMintCredit);
    UInt128 PendingCreditWatermark();
    UInt128 JournalRevision();
    OfflineCashHardwareFoldBatchV1 FoldPendingCreditBatch(UInt128 inboxSequenceInclusive, int maximumCredits);
    OfflineCashHardwareTerminalResultV1 CommitPayment(
        byte[] canonicalRequest,
        byte[] canonicalAuthorization,
        byte[] canonicalTicket);
    byte[]? RecoverPayment(byte[] creditId);
    void RecordAcknowledgement(byte[] creditId, byte[] canonicalAcknowledgement);
    OfflineCashHardwareTerminalResultV1 CommitRedemption(UInt128 amount, byte[] beneficiaryAccount);
    byte[]? RecoverRedemption(byte[] redemptionId);
    byte[] RotateHardwareEpoch();
}

/// <summary>Aggregate recursive-balance orchestration over the mandatory native provider.</summary>
public sealed class OfflineCashWalletV1
{
    private readonly object transitionLock = new();
    private readonly IOfflineCashNativeHardwareProviderV1 provider;
    private OfflineCashHardwareQualificationV1 qualification;
    private OfflineCashAggregateStateCommitmentV1 aggregateState;
    private UInt128 journalRevision;

    private OfflineCashWalletV1(
        IOfflineCashNativeHardwareProviderV1 provider,
        OfflineCashHardwareQualificationV1 qualification,
        OfflineCashAggregateStateCommitmentV1 aggregateState,
        UInt128 journalRevision)
    {
        this.provider = provider;
        this.qualification = qualification;
        this.aggregateState = aggregateState;
        this.journalRevision = journalRevision;
    }

    public OfflineCashHardwareCredentialV1 HardwareCredential
    {
        get { lock (transitionLock) return qualification.Credential; }
    }

    public OfflineCashAggregateStateCommitmentV1 AggregateState
    {
        get { lock (transitionLock) return aggregateState; }
    }

    public UInt128 JournalRevision
    {
        get { lock (transitionLock) return journalRevision; }
    }

    /// <summary>Open only after complete native qualification and recovery succeed.</summary>
    public static OfflineCashWalletV1 Open(IOfflineCashNativeHardwareProviderV1 provider)
    {
        ArgumentNullException.ThrowIfNull(provider);
        var qualification = RequireQualified(provider.Qualification());
        var recovery = provider.Recover();
        var stateBytes = recovery.AggregateState() ?? provider.BootstrapState();
        var state = OfflineCashV1.DecodeAggregateState(stateBytes);
        RequireStateQualification(state, qualification);
        if (provider.JournalRevision() != recovery.JournalRevision)
            throw new InvalidOperationException("Native recovery journal revision changed while opening the wallet.");
        return new OfflineCashWalletV1(provider, qualification, state, recovery.JournalRevision);
    }

    public OfflineCashHardwareRecoveryV1 Recover()
    {
        lock (transitionLock)
        {
            var nextQualification = RequireQualified(provider.Qualification());
            var recovery = provider.Recover();
            var stateBytes = recovery.AggregateState() ?? provider.BootstrapState();
            var state = OfflineCashV1.DecodeAggregateState(stateBytes);
            RequireStateQualification(state, nextQualification);
            var revision = provider.JournalRevision();
            if (revision != recovery.JournalRevision)
                throw new InvalidOperationException("Native recovery journal revision changed during recovery.");
            qualification = nextQualification;
            aggregateState = state;
            journalRevision = revision;
            return new OfflineCashHardwareRecoveryV1(
                OfflineCashV1.EncodeAggregateState(state),
                revision,
                recovery.PendingCreditCount,
                recovery.RetryOutboxCount);
        }
    }

    public OfflineCashPaymentRequestV1 CreatePaymentRequest(
        OfflineCashAccountIdV1 recipient,
        OfflineCashPaymentRequestModeV1 requestMode,
        ulong validityWindowMilliseconds)
    {
        ArgumentNullException.ThrowIfNull(recipient);
        if (validityWindowMilliseconds is 0 or > OfflineCashV1.RequestMaximumTtlMilliseconds)
            throw new ArgumentOutOfRangeException(nameof(validityWindowMilliseconds));
        lock (transitionLock)
        {
            var canonicalMode = OfflineCashV1.EncodePaymentRequestMode(requestMode);
            var request = OfflineCashV1.DecodePaymentRequest(provider.CreatePaymentRequest(
                recipient.CanonicalPayload(), canonicalMode, validityWindowMilliseconds));
            if (!request.Recipient.Equals(recipient)
                || request.ExpiresAtMilliseconds - request.IssuedAtMilliseconds != validityWindowMilliseconds)
                throw new InvalidOperationException("Native request output does not match its requested policy.");
            RequireStateRequestBinding(aggregateState, request);
            return request;
        }
    }

    public OfflineCashAcceptanceIntentAuthorizationV1 AuthorizeAcceptanceIntent(
        OfflineCashPaymentRequestV1 request,
        UInt128 exactAmount)
    {
        if (!request.RequestMode.Accepts(exactAmount))
            throw new ArgumentOutOfRangeException(nameof(exactAmount));
        lock (transitionLock)
        {
            var canonicalRequest = OfflineCashV1.EncodePaymentRequest(request);
            return OfflineCashV1.DecodeAcceptanceIntentAuthorization(
                provider.CreateAcceptanceIntentAuthorization(canonicalRequest, exactAmount), request);
        }
    }

    public OfflineCashAcceptanceTicketV1 IssueAcceptanceTicket(
        OfflineCashPaymentRequestV1 request,
        OfflineCashAcceptanceIntentAuthorizationV1 authorization)
    {
        lock (transitionLock)
        {
            var canonicalRequest = OfflineCashV1.EncodePaymentRequest(request);
            var canonicalAuthorization = OfflineCashV1.EncodeAcceptanceIntentAuthorization(authorization, request);
            return OfflineCashV1.DecodeAcceptanceTicket(
                provider.IssueAcceptanceTicket(canonicalRequest, canonicalAuthorization),
                request,
                authorization);
        }
    }

    public OfflineCashPaymentV1 Send(
        OfflineCashPaymentRequestV1 request,
        OfflineCashAcceptanceIntentAuthorizationV1 authorization,
        OfflineCashAcceptanceTicketV1 ticket)
    {
        lock (transitionLock)
        {
            DrainPendingCreditsLocked();
            var canonicalRequest = OfflineCashV1.EncodePaymentRequest(request);
            var canonicalAuthorization = OfflineCashV1.EncodeAcceptanceIntentAuthorization(authorization, request);
            var canonicalTicket = OfflineCashV1.EncodeAcceptanceTicket(ticket, request, authorization);
            var result = provider.CommitPayment(canonicalRequest, canonicalAuthorization, canonicalTicket);
            var payment = OfflineCashV1.DecodePayment(result.CanonicalEnvelope(), request);
            if (!payment.AcceptanceTicket.AcceptanceTicketId.Span.SequenceEqual(ticket.AcceptanceTicketId.Span))
                throw new InvalidOperationException("Native payment consumed a different acceptance ticket.");
            InstallAuthoritativeState(result.AggregateState());
            return payment;
        }
    }

    public OfflineCashStagedPaymentV1 StagePayment(
        OfflineCashPaymentRequestV1 request,
        OfflineCashPaymentV1 payment)
    {
        lock (transitionLock)
        {
            var canonicalRequest = OfflineCashV1.EncodePaymentRequest(request);
            var canonicalPayment = OfflineCashV1.EncodePayment(payment, request);
            var before = journalRevision;
            var staged = provider.StagePayment(canonicalRequest, canonicalPayment);
            if (!staged.CreditId().AsSpan().SequenceEqual(payment.Statement.Lifecycle.CreditId.Span))
                throw new InvalidOperationException("Native staging returned a different credit id.");
            var canonicalAcknowledgement = staged.Acknowledgement();
            var acknowledgement = OfflineCashV1.DecodeAcknowledgement(
                canonicalAcknowledgement, request, payment);
            var after = provider.JournalRevision();
            RequireJournalDisposition(before, after, staged.Disposition);
            journalRevision = after;
            return new OfflineCashStagedPaymentV1(
                staged.Disposition, acknowledgement, canonicalAcknowledgement);
        }
    }

    public OfflineCashHardwareStageDispositionV1 StageMintCredit(
        OfflineCashMintAuthorizationV1 authorization,
        OfflineCashMintCreditV1 mintCredit)
    {
        lock (transitionLock)
        {
            var canonicalAuthorization = OfflineCashV1.EncodeMintAuthorization(authorization);
            var canonicalCredit = OfflineCashV1.EncodeMintCredit(mintCredit, authorization);
            var before = journalRevision;
            var staged = provider.StageMintCredit(canonicalAuthorization, canonicalCredit);
            if (!staged.CreditId().AsSpan().SequenceEqual(mintCredit.Statement.Lifecycle.CreditId.Span))
                throw new InvalidOperationException("Native mint staging returned a different credit id.");
            var after = provider.JournalRevision();
            RequireJournalDisposition(before, after, staged.Disposition);
            journalRevision = after;
            return staged.Disposition;
        }
    }

    public int FoldPendingCreditBatch()
    {
        lock (transitionLock) return FoldBatchAtWatermarkLocked(provider.PendingCreditWatermark());
    }

    /// <summary>Drain one stable snapshot using repeated fixed batches; no cumulative cap is imposed.</summary>
    public UInt128 DrainPendingCredits()
    {
        lock (transitionLock) return DrainPendingCreditsLocked();
    }

    public OfflineCashPaymentV1? RecoverPayment(
        OfflineCashPaymentRequestV1 request,
        ReadOnlySpan<byte> creditId)
    {
        var expected = OfflineCashModelValidation.Fixed32(creditId, nameof(creditId));
        var canonical = provider.RecoverPayment(expected);
        if (canonical is null) return null;
        var payment = OfflineCashV1.DecodePayment(canonical, request);
        if (!payment.Statement.Lifecycle.CreditId.Span.SequenceEqual(expected))
            throw new InvalidOperationException("Recovered payment has a different credit id.");
        return payment;
    }

    public void RecordAcknowledgement(
        OfflineCashPaymentRequestV1 request,
        OfflineCashPaymentV1 payment,
        OfflineCashAcknowledgementV1 acknowledgement)
    {
        var canonical = OfflineCashV1.EncodeAcknowledgement(acknowledgement, request, payment);
        provider.RecordAcknowledgement(payment.Statement.Lifecycle.CreditId.ToArray(), canonical);
    }

    public OfflineCashRedemptionVoucherV1 Redeem(UInt128 amount, OfflineCashAccountIdV1 beneficiary)
    {
        if (amount == 0) throw new ArgumentOutOfRangeException(nameof(amount));
        ArgumentNullException.ThrowIfNull(beneficiary);
        lock (transitionLock)
        {
            DrainPendingCreditsLocked();
            var result = provider.CommitRedemption(amount, beneficiary.CanonicalPayload());
            var voucher = OfflineCashV1.DecodeRedemptionVoucher(result.CanonicalEnvelope());
            if (voucher.Statement.Amount != amount || !voucher.Statement.Beneficiary.Equals(beneficiary))
                throw new InvalidOperationException("Native redemption output differs from the request.");
            InstallAuthoritativeState(result.AggregateState());
            return voucher;
        }
    }

    public OfflineCashRedemptionVoucherV1? RecoverRedemption(ReadOnlySpan<byte> redemptionId)
    {
        var expected = OfflineCashModelValidation.Fixed32(redemptionId, nameof(redemptionId));
        var canonical = provider.RecoverRedemption(expected);
        if (canonical is null) return null;
        var voucher = OfflineCashV1.DecodeRedemptionVoucher(canonical);
        if (!voucher.Statement.RedemptionId.Span.SequenceEqual(expected))
            throw new InvalidOperationException("Recovered redemption has a different id.");
        return voucher;
    }

    public OfflineCashAggregateStateCommitmentV1 RotateHardwareEpoch()
    {
        lock (transitionLock)
        {
            DrainPendingCreditsLocked();
            InstallAuthoritativeState(provider.RotateHardwareEpoch());
            qualification = RequireQualified(provider.Qualification());
            RequireStateQualification(aggregateState, qualification);
            return aggregateState;
        }
    }

    private UInt128 DrainPendingCreditsLocked()
    {
        var watermark = provider.PendingCreditWatermark();
        UInt128 total = 0;
        while (true)
        {
            var folded = FoldBatchAtWatermarkLocked(watermark);
            if (folded == 0) return total;
            total = checked(total + (uint)folded);
        }
    }

    private int FoldBatchAtWatermarkLocked(UInt128 watermark)
    {
        var before = journalRevision;
        var beforeCommitment = aggregateState.StateCommitment.ToArray();
        var folded = provider.FoldPendingCreditBatch(watermark, 16);
        var successor = folded.AggregateState();
        if (folded.FoldedCredits > 0)
        {
            if (successor is null)
                throw new InvalidOperationException("A non-empty native fold did not return its successor.");
            InstallAuthoritativeState(successor);
            if (aggregateState.StateCommitment.Span.SequenceEqual(beforeCommitment))
                throw new InvalidOperationException("A fixed-shape fold made no aggregate-state progress.");
        }
        var after = provider.JournalRevision();
        var expected = folded.FoldedCredits == 0 ? before : checked(before + 1);
        if (after != expected)
            throw new InvalidOperationException("A fixed-shape fold did not consume exactly one journal revision.");
        journalRevision = after;
        return folded.FoldedCredits;
    }

    private void InstallAuthoritativeState(byte[] bytes)
    {
        var state = OfflineCashV1.DecodeAggregateState(bytes);
        RequireSameAsset(aggregateState, state);
        aggregateState = state;
        journalRevision = provider.JournalRevision();
    }

    private static OfflineCashHardwareQualificationV1 RequireQualified(
        OfflineCashHardwareQualificationV1 value)
    {
        ArgumentNullException.ThrowIfNull(value);
        value.RequireProductionReady();
        return value;
    }

    private static void RequireJournalDisposition(
        UInt128 before,
        UInt128 after,
        OfflineCashHardwareStageDispositionV1 disposition)
    {
        var expected = disposition == OfflineCashHardwareStageDispositionV1.Staged
            ? checked(before + 1)
            : before;
        if (after != expected)
            throw new InvalidOperationException("Native staging returned an invalid journal revision.");
    }

    private static void RequireStateQualification(
        OfflineCashAggregateStateCommitmentV1 state,
        OfflineCashHardwareQualificationV1 qualification)
    {
        if (!state.ReleaseId.Span.SequenceEqual(qualification.ReleaseId())
            || state.NetworkId != qualification.Credential.NetworkId
            || !state.KeyReference.Span.SequenceEqual(qualification.Credential.DeviceKeyReference.Span)
            || !state.HardwarePolicyId.Span.SequenceEqual(qualification.Profile.HardwareProfileId.Span))
            throw new InvalidOperationException("Aggregate state does not match native qualification.");
    }

    private static void RequireStateRequestBinding(
        OfflineCashAggregateStateCommitmentV1 state,
        OfflineCashPaymentRequestV1 request)
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
        OfflineCashAggregateStateCommitmentV1 before,
        OfflineCashAggregateStateCommitmentV1 after)
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
