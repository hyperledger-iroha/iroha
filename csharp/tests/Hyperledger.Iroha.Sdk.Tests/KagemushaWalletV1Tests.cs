using Hyperledger.Iroha.Kagemusha;
using KagemushaCodec = Hyperledger.Iroha.Kagemusha.Kagemusha;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>Managed orchestration tests; the synthetic provider grants no monetary qualification.</summary>
public sealed class KagemushaWalletV1Tests
{
    [Fact]
    public void BootstrapRequiresASecondMatchingDurableSnapshot()
    {
        var provider = new Provider { MissingState = true, Revision = 4 };
        var wallet = KagemushaWalletV1.Open(provider);
        Assert.Equal(2, provider.RecoverCalls);
        Assert.Equal(1, provider.BootstrapCalls);
        Assert.Equal((UInt128)5, wallet.JournalRevision);
        Assert.Equal(KagemushaCodec.EncodeAggregateState(provider.State),
            KagemushaCodec.EncodeAggregateState(wallet.AggregateState));

        var missing = new Provider { MissingState = true, PersistBootstrap = false };
        Assert.Throws<InvalidOperationException>(() => KagemushaWalletV1.Open(missing));
        Assert.Equal(1, missing.BootstrapCalls);

        var substituted = new Provider { MissingState = true, SubstituteBootstrapReturn = true };
        Assert.Throws<InvalidOperationException>(() => KagemushaWalletV1.Open(substituted));
    }

    [Fact]
    public void RecoveryNeverBootstrapsAnExistingWallet()
    {
        var provider = new Provider();
        var wallet = KagemushaWalletV1.Open(provider);
        var before = KagemushaCodec.EncodeAggregateState(wallet.AggregateState);
        provider.MissingState = true;
        Assert.Throws<InvalidOperationException>(() => wallet.Recover());
        Assert.Equal(0, provider.BootstrapCalls);
        Assert.Equal(before, KagemushaCodec.EncodeAggregateState(wallet.AggregateState));
    }

    [Theory]
    [InlineData("journal")]
    [InlineData("equivocation")]
    [InlineData("identity")]
    [InlineData("epoch")]
    [InlineData("live-revision")]
    public void InvalidRecoveryPreservesThePublishedSnapshot(string mutation)
    {
        var provider = new Provider { Revision = 4 };
        var wallet = KagemushaWalletV1.Open(provider);
        var before = KagemushaCodec.EncodeAggregateState(wallet.AggregateState);
        switch (mutation)
        {
            case "journal": provider.Revision = 3; break;
            case "equivocation": provider.State = provider.State with { StateCommitment = Digest(0x99) }; break;
            case "identity": provider.State = provider.State with { Scale = provider.State.Scale + 1 }; break;
            case "epoch":
                provider.Credential = provider.Credential with { HardwareEpochId = Digest(0x99) };
                provider.State = provider.State with { HardwareEpochId = provider.Credential.HardwareEpochId };
                break;
            case "live-revision": provider.LiveRevisionOffset = 1; break;
        }
        Assert.Throws<InvalidOperationException>(() => wallet.Recover());
        Assert.Equal(before, KagemushaCodec.EncodeAggregateState(wallet.AggregateState));
        Assert.Equal((UInt128)4, wallet.JournalRevision);
    }

    [Fact]
    public void RecoveryAdmitsAdvancingStateAndUnsignedEpochGeneration()
    {
        var provider = new Provider { Revision = 4 };
        provider.Credential = provider.Credential with { HardwareEpochGeneration = (1UL << 63) - 1 };
        var wallet = KagemushaWalletV1.Open(provider);
        provider.State = provider.State with { Sequence = 1, StateCommitment = Digest(0x98) };
        provider.Revision = 5;
        Assert.Equal((UInt128)5, wallet.Recover().JournalRevision);
        provider.Credential = provider.Credential with
        {
            HardwareEpochGeneration = 1UL << 63,
            HardwareEpochId = Digest(0x97),
        };
        provider.State = provider.State with
        {
            HardwareEpochId = provider.Credential.HardwareEpochId,
            Sequence = 0,
            StateCommitment = Digest(0x96),
        };
        provider.Revision = 0;
        Assert.Equal((UInt128)0, wallet.Recover().JournalRevision);
        Assert.Equal(1UL << 63, wallet.HardwareCredential.HardwareEpochGeneration);
    }

    [Theory]
    [InlineData("request")]
    [InlineData("payment")]
    [InlineData("mint")]
    [InlineData("redemption")]
    public void EveryReservationRequiresExactCallerIdentityAndDefensiveCopies(string kind)
    {
        var provider = new Provider();
        var wallet = KagemushaWalletV1.Open(provider);
        byte[] Reserve(byte[] id) => kind switch
        {
            "request" => wallet.ReservePaymentRequestOperationId(id, provider.Context.Account, 25, 1_000),
            "payment" => wallet.ReservePaymentOperationId(id, provider.Context.Request()),
            "mint" => wallet.ReserveMintOperationId(id, 25, provider.Context.Account, provider.Context.Account),
            _ => wallet.ReserveRedemptionOperationId(id, 25, provider.Context.Account),
        };
        var expected = Digest(0x55);
        var returned = Reserve(expected);
        Assert.Equal(expected, returned);
        returned[0] ^= 0xff;
        Assert.Equal(Digest(0x55), expected);
        Assert.Equal(expected, Reserve(expected));
        provider.SubstituteReservation = true;
        Assert.Throws<InvalidOperationException>(() => Reserve(expected));
        Assert.Equal(Digest(0x55), expected);
        var calls = provider.ReserveCalls;
        Assert.Throws<ArgumentException>(() => Reserve(new byte[32]));
        Assert.Equal(calls, provider.ReserveCalls);
    }

    [Fact]
    public void RequestReservesBeforeMutationAndChecksReturnedIdentity()
    {
        var provider = new Provider();
        var wallet = KagemushaWalletV1.Open(provider);
        var expected = Digest(0x55);
        var request = wallet.CreatePaymentRequest(expected, provider.Context.Account, 25, 1_000);
        Assert.Equal(expected, request.RequestId.ToArray());
        Assert.Equal(new[] { "reserve-request", "create-request" }, provider.RequestCalls);
        provider.RequestCalls.Clear();
        provider.SubstituteReservation = true;
        Assert.Throws<InvalidOperationException>(() =>
            wallet.CreatePaymentRequest(expected, provider.Context.Account, 25, 1_000));
        Assert.Equal(new[] { "reserve-request" }, provider.RequestCalls);
        provider.SubstituteReservation = false;
        provider.SubstituteRequest = true;
        Assert.Throws<InvalidOperationException>(() =>
            wallet.CreatePaymentRequest(expected, provider.Context.Account, 25, 1_000));
    }

    private static byte[] Digest(byte value) => Enumerable.Repeat(value, 32).ToArray();

    private sealed class Provider : IKagemushaNativeHardwareProviderV1
    {
        internal KagemushaV1Tests.TestContext Context { get; } = KagemushaV1Tests.TestContext.Create();
        internal KagemushaHardwareCredentialV1 Credential { get; set; }
        internal KagemushaAggregateStateCommitmentV1 State { get; set; }
        internal UInt128 Revision { get; set; }
        internal UInt128 LiveRevisionOffset { get; set; }
        internal bool MissingState { get; set; }
        internal bool PersistBootstrap { get; set; } = true;
        internal bool SubstituteBootstrapReturn { get; set; }
        internal bool SubstituteReservation { get; set; }
        internal bool SubstituteRequest { get; set; }
        internal int RecoverCalls { get; private set; }
        internal int BootstrapCalls { get; private set; }
        internal int ReserveCalls { get; private set; }
        internal List<string> RequestCalls { get; } = [];

        internal Provider()
        {
            Credential = Context.Credential;
            State = new(1, Context.ReleaseId, Context.NetworkId, Context.Asset, Context.Incarnation,
                4, Context.LiabilityPoolId, Credential.LaneCommitment, Credential.HardwareEpochId,
                Credential.DeviceKeyReference, Digest(0x47), 0, Digest(0x95));
        }
        public KagemushaHardwareQualificationV1 Qualification() => new(
            1, Context.Profile(), Credential, Context.ReleaseId, Digest(0x47), Digest(0x48),
            Enum.GetValues<KagemushaHardwareCapabilityV1>());
        public KagemushaHardwareRecoveryV1 Recover()
        {
            RecoverCalls++;
            return new(MissingState ? null : KagemushaCodec.EncodeAggregateState(State), Revision, 0, 0);
        }
        public byte[] BootstrapState()
        {
            BootstrapCalls++;
            if (PersistBootstrap) { MissingState = false; Revision++; }
            return KagemushaCodec.EncodeAggregateState(SubstituteBootstrapReturn
                ? State with { StateCommitment = Digest(0x94) } : State);
        }
        public UInt128 JournalRevision() => Revision + LiveRevisionOffset;
        private byte[] Reserve(byte[] id)
        {
            ReserveCalls++;
            if (SubstituteReservation) id[0] ^= 0xff;
            return id;
        }
        public byte[] ReservePaymentRequestOperationId(byte[] operationId, byte[] recipientAccount,
            UInt128 amount, ulong validityWindowMilliseconds)
        { RequestCalls.Add("reserve-request"); return Reserve(operationId); }
        public byte[] CreatePaymentRequest(byte[] operationId, byte[] recipientAccount,
            UInt128 amount, ulong validityWindowMilliseconds)
        {
            RequestCalls.Add("create-request");
            return KagemushaCodec.EncodePaymentRequest(Context.Request(amount) with
            {
                RequestId = SubstituteRequest ? Digest(0x93) : operationId,
                ExpiresAtMilliseconds = 1_000 + validityWindowMilliseconds,
            });
        }
        public byte[] ReservePaymentOperationId(byte[] id, byte[] request) => Reserve(id);
        public byte[] ReserveMintOperationId(byte[] id, UInt128 amount, byte[] payer, byte[] recipient) => Reserve(id);
        public byte[] ReserveRedemptionOperationId(byte[] id, UInt128 amount, byte[] beneficiary) => Reserve(id);
        public KagemushaHardwarePaymentStageV1 StagePayment(byte[] request, byte[] payment) => throw new NotSupportedException();
        public KagemushaHardwareMintStageV1 StageMintCredit(byte[] authorization, byte[] credit) => throw new NotSupportedException();
        public KagemushaPendingCreditSelectionV1 SelectPendingCredit(KagemushaPendingCreditWatermarkV1? watermark,
            KagemushaPendingCreditTargetV1 target) => throw new NotSupportedException();
        public KagemushaHardwareReceiveFoldV1 FoldPendingCredit(KagemushaPendingCreditSelectorV1 selector) => throw new NotSupportedException();
        public KagemushaHardwareTerminalResultV1 CommitPayment(byte[] id, byte[] request) => throw new NotSupportedException();
        public byte[]? RecoverPayment(byte[] id) => throw new NotSupportedException();
        public byte[]? RecoverPaymentByOperationId(byte[] id, byte[] request) => throw new NotSupportedException();
        public KagemushaMintConstructionBundleV1 PrepareMintConstructionBundle(byte[] id, UInt128 amount,
            byte[] payer, byte[] recipient) => throw new NotSupportedException();
        public KagemushaMintConstructionBundleV1? RecoverMintConstructionBundle(byte[] id) => throw new NotSupportedException();
        public void RecordAcknowledgement(byte[] id, byte[] request, byte[] payment, byte[] acknowledgement) => throw new NotSupportedException();
        public KagemushaHardwareTerminalResultV1 CommitRedemption(byte[] id, UInt128 amount, byte[] beneficiary) => throw new NotSupportedException();
        public byte[]? RecoverRedemption(byte[] id) => throw new NotSupportedException();
        public byte[]? RecoverRedemptionByOperationId(byte[] id) => throw new NotSupportedException();
        public byte[] RotateHardwareEpoch() => throw new NotSupportedException();
    }
}
