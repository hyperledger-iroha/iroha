namespace Hyperledger.Iroha.Kagemusha;

/// <summary>Hardware properties required together by every production Kagemusha V1 provider.</summary>
public enum KagemushaHardwareCapabilityV1
{
    ExactNextPredecessorConsumption,
    OneUseSuccessorAuthorization,
    RollbackResistantCounterAndJournal,
    SealedTransitionRecovery,
    DurableInboxReservation,
    AuthenticatedInboundStaging,
    AuthoritativeReplayRootRecovery,
    SenderOutboxReservation,
    AuthenticatedDurableRetryOutbox,
    AtomicVerifiedCandidateCommit,
    AtomicRecoverableTransitionCertificate,
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
        if (ProtocolVersion != KagemushaV1.WireVersion
            || Profile.Version != ProtocolVersion
            || Profile.ProtocolVersion != ProtocolVersion
            || Credential.Version != ProtocolVersion
            || !Profile.HardwareProfileId.Span.SequenceEqual(Credential.HardwareProfileId.Span)
            || Profile.PolicyEpoch != Credential.PolicyEpoch
            || capabilities.Count != Enum.GetValues<KagemushaHardwareCapabilityV1>().Length
            || Enum.GetValues<KagemushaHardwareCapabilityV1>().Any(value => !capabilities.Contains(value)))
        {
            throw new InvalidOperationException(
                "Kagemusha V1 requires the complete qualified non-forking native hardware capability set.");
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
        if (acknowledgement.IsEmpty || acknowledgement.Length > KagemushaV1.MaximumAcknowledgementBytes)
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
    byte[] CreatePaymentRequest(byte[] recipientAccount, UInt128 amount, ulong validityWindowMilliseconds);
    KagemushaHardwarePaymentStageV1 StagePayment(byte[] canonicalRequest, byte[] canonicalPayment);
    KagemushaHardwareMintStageV1 StageMintCredit(byte[] canonicalAuthorization, byte[] canonicalMintCredit);
    UInt128 PendingCreditWatermark();
    UInt128 JournalRevision();
    byte[]? FoldPendingCredit(UInt128 inboxSequenceInclusive);
    KagemushaHardwareTerminalResultV1 CommitPayment(byte[] canonicalRequest);
    byte[]? RecoverPayment(byte[] creditId);
    void RecordAcknowledgement(byte[] creditId, byte[] canonicalAcknowledgement);
    KagemushaHardwareTerminalResultV1 CommitRedemption(UInt128 amount, byte[] beneficiaryAccount);
    byte[]? RecoverRedemption(byte[] redemptionId);
    byte[] RotateHardwareEpoch();
}

/// <summary>Aggregate recursive-balance orchestration over the mandatory native provider.</summary>
public sealed class KagemushaWalletV1
{
    private readonly object transitionLock = new();
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
        var qualification = RequireQualified(provider.Qualification());
        var recovery = provider.Recover();
        var stateBytes = recovery.AggregateState() ?? provider.BootstrapState();
        var state = KagemushaV1.DecodeAggregateState(stateBytes);
        RequireStateQualification(state, qualification);
        if (provider.JournalRevision() != recovery.JournalRevision)
            throw new InvalidOperationException("Native recovery journal revision changed while opening the wallet.");
        return new KagemushaWalletV1(provider, qualification, state, recovery.JournalRevision);
    }

    public KagemushaHardwareRecoveryV1 Recover()
    {
        lock (transitionLock)
        {
            var nextQualification = RequireQualified(provider.Qualification());
            var recovery = provider.Recover();
            var stateBytes = recovery.AggregateState() ?? provider.BootstrapState();
            var state = KagemushaV1.DecodeAggregateState(stateBytes);
            RequireStateQualification(state, nextQualification);
            var revision = provider.JournalRevision();
            if (revision != recovery.JournalRevision)
                throw new InvalidOperationException("Native recovery journal revision changed during recovery.");
            qualification = nextQualification;
            aggregateState = state;
            journalRevision = revision;
            return new KagemushaHardwareRecoveryV1(
                KagemushaV1.EncodeAggregateState(state),
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
        if (amount == 0)
            throw new ArgumentOutOfRangeException(nameof(amount));
        if (validityWindowMilliseconds is 0 or > KagemushaV1.RequestMaximumTtlMilliseconds)
            throw new ArgumentOutOfRangeException(nameof(validityWindowMilliseconds));
        lock (transitionLock)
        {
            var request = KagemushaV1.DecodePaymentRequest(provider.CreatePaymentRequest(
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
        lock (transitionLock)
        {
            DrainPendingCreditsLocked();
            var canonicalRequest = KagemushaV1.EncodePaymentRequest(request);
            var result = provider.CommitPayment(canonicalRequest);
            var payment = KagemushaV1.DecodePayment(result.CanonicalEnvelope(), request);
            InstallAuthoritativeState(result.AggregateState());
            return payment;
        }
    }

    public KagemushaStagedPaymentV1 StagePayment(
        KagemushaPaymentRequestV1 request,
        KagemushaPaymentV1 payment)
    {
        lock (transitionLock)
        {
            var canonicalRequest = KagemushaV1.EncodePaymentRequest(request);
            var canonicalPayment = KagemushaV1.EncodePayment(payment, request);
            var before = journalRevision;
            var staged = provider.StagePayment(canonicalRequest, canonicalPayment);
            if (!staged.CreditId().AsSpan().SequenceEqual(payment.Statement.Lifecycle.CreditId.Span))
                throw new InvalidOperationException("Native staging returned a different credit id.");
            var canonicalAcknowledgement = staged.Acknowledgement();
            var acknowledgement = KagemushaV1.DecodeAcknowledgement(
                canonicalAcknowledgement, request, payment);
            var after = provider.JournalRevision();
            RequireJournalDisposition(before, after, staged.Disposition);
            journalRevision = after;
            return new KagemushaStagedPaymentV1(
                staged.Disposition, acknowledgement, canonicalAcknowledgement);
        }
    }

    public KagemushaHardwareStageDispositionV1 StageMintCredit(
        KagemushaMintAuthorizationV1 authorization,
        KagemushaMintCreditV1 mintCredit)
    {
        lock (transitionLock)
        {
            var canonicalAuthorization = KagemushaV1.EncodeMintAuthorization(authorization);
            var canonicalCredit = KagemushaV1.EncodeMintCredit(mintCredit, authorization);
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

    public bool FoldPendingCredit()
    {
        lock (transitionLock) return FoldAtWatermarkLocked(provider.PendingCreditWatermark());
    }

    /// <summary>Drain one stable snapshot using repeated single-credit folds; no count cap is imposed.</summary>
    public UInt128 DrainPendingCredits()
    {
        lock (transitionLock) return DrainPendingCreditsLocked();
    }

    public KagemushaPaymentV1? RecoverPayment(
        KagemushaPaymentRequestV1 request,
        ReadOnlySpan<byte> creditId)
    {
        var expected = KagemushaModelValidation.Fixed32(creditId, nameof(creditId));
        var canonical = provider.RecoverPayment(expected);
        if (canonical is null) return null;
        var payment = KagemushaV1.DecodePayment(canonical, request);
        if (!payment.Statement.Lifecycle.CreditId.Span.SequenceEqual(expected))
            throw new InvalidOperationException("Recovered payment has a different credit id.");
        return payment;
    }

    public void RecordAcknowledgement(
        KagemushaPaymentRequestV1 request,
        KagemushaPaymentV1 payment,
        KagemushaAcknowledgementV1 acknowledgement)
    {
        var canonical = KagemushaV1.EncodeAcknowledgement(acknowledgement, request, payment);
        provider.RecordAcknowledgement(payment.Statement.Lifecycle.CreditId.ToArray(), canonical);
    }

    public KagemushaRedemptionVoucherV1 Redeem(UInt128 amount, KagemushaAccountIdV1 beneficiary)
    {
        if (amount == 0) throw new ArgumentOutOfRangeException(nameof(amount));
        ArgumentNullException.ThrowIfNull(beneficiary);
        lock (transitionLock)
        {
            DrainPendingCreditsLocked();
            var result = provider.CommitRedemption(amount, beneficiary.CanonicalPayload());
            var voucher = KagemushaV1.DecodeRedemptionVoucher(result.CanonicalEnvelope());
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
        var voucher = KagemushaV1.DecodeRedemptionVoucher(canonical);
        if (!voucher.Statement.RedemptionId.Span.SequenceEqual(expected))
            throw new InvalidOperationException("Recovered redemption has a different id.");
        return voucher;
    }

    public KagemushaAggregateStateCommitmentV1 RotateHardwareEpoch()
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
            if (!FoldAtWatermarkLocked(watermark)) return total;
            total = checked(total + 1);
        }
    }

    private bool FoldAtWatermarkLocked(UInt128 watermark)
    {
        var before = journalRevision;
        var beforeCommitment = aggregateState.StateCommitment.ToArray();
        var successor = provider.FoldPendingCredit(watermark);
        if (successor is not null)
        {
            InstallAuthoritativeState(successor);
            if (aggregateState.StateCommitment.Span.SequenceEqual(beforeCommitment))
                throw new InvalidOperationException("A receive fold made no aggregate-state progress.");
        }
        var after = provider.JournalRevision();
        var expected = successor is null ? before : checked(before + 1);
        if (after != expected)
            throw new InvalidOperationException("A receive fold did not consume exactly one journal revision.");
        journalRevision = after;
        return successor is not null;
    }

    private void InstallAuthoritativeState(byte[] bytes)
    {
        var state = KagemushaV1.DecodeAggregateState(bytes);
        RequireSameAsset(aggregateState, state);
        aggregateState = state;
        journalRevision = provider.JournalRevision();
    }

    private static KagemushaHardwareQualificationV1 RequireQualified(
        KagemushaHardwareQualificationV1 value)
    {
        ArgumentNullException.ThrowIfNull(value);
        value.RequireProductionReady();
        return value;
    }

    private static void RequireJournalDisposition(
        UInt128 before,
        UInt128 after,
        KagemushaHardwareStageDispositionV1 disposition)
    {
        var expected = disposition == KagemushaHardwareStageDispositionV1.Staged
            ? checked(before + 1)
            : before;
        if (after != expected)
            throw new InvalidOperationException("Native staging returned an invalid journal revision.");
    }

    private static void RequireStateQualification(
        KagemushaAggregateStateCommitmentV1 state,
        KagemushaHardwareQualificationV1 qualification)
    {
        if (!state.ReleaseId.Span.SequenceEqual(qualification.ReleaseId())
            || state.NetworkId != qualification.Credential.NetworkId
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
