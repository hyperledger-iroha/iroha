using Hyperledger.Iroha.Kagemusha;
using Hyperledger.Iroha.Norito;
using KagemushaCodec = Hyperledger.Iroha.Kagemusha.Kagemusha;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>Focused first-release tests for the three-message KAGEMUSHA protocol.</summary>
public sealed class KagemushaV1Tests
{
    [Theory]
    [InlineData(1)]
    [InlineData(1_283)]
    [InlineData(KagemushaCodec.MaximumParityProofBytes)]
    public void ThreeMessageExchangeRoundTripsAtEveryProofSize(int parityProofBytes)
    {
        var context = TestContext.Create();
        var request = context.Request();
        var payment = context.Payment(request, parityProofBytes);
        var acknowledgement = context.Acknowledgement(request, payment);

        var requestBytes = KagemushaCodec.EncodePaymentRequest(request);
        var paymentBytes = KagemushaCodec.EncodePayment(payment, request);
        var acknowledgementBytes = KagemushaCodec.EncodeAcknowledgement(
            acknowledgement, request, payment);

        Assert.Equal(requestBytes, KagemushaCodec.EncodePaymentRequest(
            KagemushaCodec.DecodePaymentRequest(requestBytes)));
        Assert.Equal(paymentBytes, KagemushaCodec.EncodePayment(
            KagemushaCodec.DecodePayment(paymentBytes, request), request));
        Assert.Equal(acknowledgementBytes, KagemushaCodec.EncodeAcknowledgement(
            KagemushaCodec.DecodeAcknowledgement(acknowledgementBytes, request, payment),
            request, payment));

        Assert.Equal(
            requestBytes.Length + paymentBytes.Length + acknowledgementBytes.Length,
            KagemushaCodec.ValidateCompleteExchange(request, payment, acknowledgement));
        Assert.True(payment.Proof.EqProof.Length + payment.Proof.EpProof.Length
            <= KagemushaCodec.MaximumCurrentProofsBytes);
    }

    [Fact]
    public void PublicPeerProtocolHasOnlyRequestPaymentAndAcknowledgement()
    {
        Assert.Equal(new byte[] { 1, 2, 3 },
            Enum.GetValues<IrohaPeerPayloadKindV1>().Select(static value => (byte)value).ToArray());
        Assert.Equal("kgm1:", KagemushaCodec.TextPrefix);
    }

    [Fact]
    public void OperationSurfaceHasExactlySixHistoryIndependentTransitions()
    {
        Assert.Equal(
            new[]
            {
                KagemushaOperationKindV1.Bootstrap,
                KagemushaOperationKindV1.MintFold,
                KagemushaOperationKindV1.SendSplit,
                KagemushaOperationKindV1.ReceiveFold,
                KagemushaOperationKindV1.RedeemSplit,
                KagemushaOperationKindV1.Rotate,
            },
            Enum.GetValues<KagemushaOperationKindV1>());
        Assert.Equal(new uint[] { 0, 1, 2, 3, 4, 5 },
            Enum.GetValues<KagemushaOperationKindV1>().Select(static value => (uint)value).ToArray());
    }

    [Fact]
    public void RequestCarriesOnePositiveExactAmountAndRecipientKey()
    {
        var context = TestContext.Create();
        var request = context.Request((UInt128)1_000);
        var decoded = KagemushaCodec.DecodePaymentRequest(
            KagemushaCodec.EncodePaymentRequest(request));

        Assert.Equal((UInt128)1_000, decoded.Amount);
        Assert.Equal(request.RecipientEncryptionKey, decoded.RecipientEncryptionKey);
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            KagemushaCodec.EncodePaymentRequest(context.Request(0)));
        Assert.Throws<ArgumentException>(() => new KagemushaX25519PublicKeyV1(new byte[32]));
    }

    [Fact]
    public void PaymentBindsRequestStateTransitionCiphertextAndCommitWindow()
    {
        var context = TestContext.Create();
        var request = context.Request();
        var payment = context.Payment(request);

        Assert.Equal(KagemushaCodec.PaymentRequestDigest(request),
            payment.Output.RequestDigest.ToArray());
        Assert.Equal(
            KagemushaCodec.CreditId(payment.Output.TransitionNullifier,
                payment.Output.RequestDigest),
            payment.Output.CreditId.ToArray());
        Assert.Equal(request.Amount, payment.Output.Amount);
        Assert.False(payment.Output.SenderBeforeCommitment.Span.SequenceEqual(
            payment.Output.SenderAfterCommitment.Span));

        var stale = payment with
        {
            Output = payment.Output with { CommittedAtMilliseconds = request.ExpiresAtMilliseconds },
        };
        Assert.Throws<ArgumentException>(() => KagemushaCodec.EncodePayment(stale, request));
        var substituted = payment with
        {
            Output = payment.Output with { RequestDigest = Repeat(0xee) },
        };
        Assert.Throws<ArgumentException>(() => KagemushaCodec.EncodePayment(substituted, request));
    }

    [Fact]
    public void PeerContextAndAadBindRequestKeyAndBothStateHeads()
    {
        var context = TestContext.Create();
        var request = context.Request();
        var payment = context.Payment(request);
        var peerContext = KagemushaCodec.PeerCreditContext(payment.Output, request);
        var aad = KagemushaCodec.PeerEncryptedCreditAad(payment.Output, request);

        Assert.Equal(payment.Output.RequestDigest.ToArray(), peerContext.RequestDigest.ToArray());
        Assert.Equal(payment.Output.SenderBeforeCommitment.ToArray(),
            peerContext.SenderBeforeCommitment.ToArray());
        Assert.Equal(payment.Output.SenderAfterCommitment.ToArray(),
            peerContext.SenderAfterCommitment.ToArray());
        Assert.Equal(request.RecipientEncryptionKey, peerContext.RecipientEncryptionKey);
        Assert.Equal(payment.Output.CreditId.ToArray(), aad.CreditId.ToArray());
        Assert.Equal(request.Amount, aad.Amount);
    }

    [Fact]
    public void AcknowledgementBindsDurableReceiptToRequestAndPayment()
    {
        var context = TestContext.Create();
        var request = context.Request();
        var payment = context.Payment(request);
        var acknowledgement = context.Acknowledgement(request, payment);

        Assert.Equal(KagemushaCodec.PaymentRequestDigest(request),
            acknowledgement.RequestDigest.ToArray());
        Assert.Equal(KagemushaCodec.PaymentDigest(payment, request),
            acknowledgement.PaymentDigest.ToArray());
        Assert.Equal(payment.Output.CreditId.ToArray(),
            acknowledgement.InboxReceipt.CreditId.ToArray());
    }

    [Fact]
    public void TextAndPeerEnvelopesRoundTripAllThreeMessages()
    {
        var context = TestContext.Create();
        var request = context.Request();
        var payment = context.Payment(request);
        var acknowledgement = context.Acknowledgement(request, payment);
        var messages = new[]
        {
            (IrohaPeerPayloadKindV1.Request, KagemushaCodec.PayloadKind.PaymentRequest,
                KagemushaCodec.EncodePaymentRequest(request)),
            (IrohaPeerPayloadKindV1.Payment, KagemushaCodec.PayloadKind.Payment,
                KagemushaCodec.EncodePayment(payment, request)),
            (IrohaPeerPayloadKindV1.Acknowledgement, KagemushaCodec.PayloadKind.Acknowledgement,
                KagemushaCodec.EncodeAcknowledgement(acknowledgement, request, payment)),
        };

        foreach (var (kind, textKind, canonical) in messages)
        {
            var text = KagemushaCodec.EncodeText(textKind, canonical);
            Assert.Equal(canonical, KagemushaCodec.DecodeText(textKind, text));
            var wire = IrohaPeerKagemushaAdapterV1.Wrap(kind, canonical);
            var decodedWire = IrohaPeerWireMessageV1.Decode(wire.Encode(),
                IrohaPeerPayloadProfileV1.KagemushaV1, kind);
            Assert.Equal(canonical, IrohaPeerKagemushaAdapterV1.Decode(decodedWire));
        }
    }

    [Fact]
    public void HardwareQualificationRequiresCompleteNonForkingContract()
    {
        var context = TestContext.Create();
        var profile = context.Profile();
        var all = Enum.GetValues<KagemushaHardwareCapabilityV1>();
        var complete = new KagemushaHardwareQualificationV1(
            1, profile, context.Credential, context.ReleaseId, all);
        complete.RequireProductionReady();

        var incomplete = new KagemushaHardwareQualificationV1(
            1, profile, context.Credential, context.ReleaseId, all.Skip(1));
        Assert.Throws<InvalidOperationException>(incomplete.RequireProductionReady);
    }

    [Fact]
    public void ReceiveFoldRepresentsExactlyOneCredit()
    {
        Assert.NotEmpty(new KagemushaHardwareReceiveFoldV1(new byte[] { 1 }).AggregateState());
        Assert.Throws<ArgumentException>(() =>
            new KagemushaHardwareReceiveFoldV1(Array.Empty<byte>()));
    }

    private static byte[] Repeat(byte value, int count = 32) =>
        Enumerable.Repeat(value, count).ToArray();

    private sealed class TestContext
    {
        private const string AccountLiteral =
            "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";

        private TestContext(
            NetworkId networkId,
            KagemushaAssetDefinitionIdV1 asset,
            KagemushaAssetIncarnationV1 incarnation,
            KagemushaAccountIdV1 account,
            KagemushaHardwareCredentialV1 credential,
            byte[] liabilityPoolId,
            byte[] releaseId,
            KagemushaX25519PublicKeyV1 recipientEncryptionKey)
        {
            NetworkId = networkId;
            Asset = asset;
            Incarnation = incarnation;
            Account = account;
            Credential = credential;
            LiabilityPoolId = liabilityPoolId;
            ReleaseId = releaseId;
            RecipientEncryptionKey = recipientEncryptionKey;
        }

        internal NetworkId NetworkId { get; }
        internal KagemushaAssetDefinitionIdV1 Asset { get; }
        internal KagemushaAssetIncarnationV1 Incarnation { get; }
        internal KagemushaAccountIdV1 Account { get; }
        internal KagemushaHardwareCredentialV1 Credential { get; }
        internal byte[] LiabilityPoolId { get; }
        internal byte[] ReleaseId { get; }
        internal KagemushaX25519PublicKeyV1 RecipientEncryptionKey { get; }

        internal static TestContext Create()
        {
            var network = Enumerable.Range(1, 32).Select(static value => (byte)value).ToArray();
            network[^1] |= 1;
            var networkId = NetworkId.FromBytes(network);
            var asset = new KagemushaAssetDefinitionIdV1("6TEAJqbb8oEPmLncoNiMRbLEK6tw");
            var incarnationBytes = new byte[32];
            incarnationBytes[0] = 1;
            incarnationBytes[^1] = 1;
            var incarnation = new KagemushaAssetIncarnationV1(incarnationBytes);
            var account = new KagemushaAccountIdV1(AccountLiteral);
            var sec1 = Repeat(1, 65);
            sec1[0] = 4;
            var publicKey = new KagemushaDevicePublicKeyV1(sec1);
            var signatureBytes = new byte[64];
            signatureBytes[31] = 1;
            signatureBytes[63] = 1;
            var signature = new KagemushaDeviceSignatureV1(signatureBytes);
            var profileId = Repeat(0x42);
            var credential = new KagemushaHardwareCredentialV1(
                1, Repeat(0x51), networkId, profileId, Repeat(0x52), Repeat(0x45), 9,
                Repeat(0x53), Repeat(0x54), 1, publicKey,
                KagemushaCodec.DeviceKeyReference(publicKey), 100, 10_000, signature);
            return new TestContext(
                networkId,
                asset,
                incarnation,
                account,
                credential,
                KagemushaCodec.LiabilityPoolId(networkId, asset, incarnation),
                Repeat(0x41),
                new KagemushaX25519PublicKeyV1(Repeat(0xb6)));
        }

        internal KagemushaHardwareProfileV1 Profile() => new(
            1, 1, Credential.HardwareProfileId, Repeat(0x40),
            KagemushaHardwarePlatformClassV1.DedicatedSecureElement,
            Repeat(0x41), Credential.FirmwarePolicyDigest, Repeat(0x43), Repeat(0x44),
            Repeat(0x45), Credential.PolicyEpoch, Credential.DevicePublicKey,
            ushort.MaxValue, Repeat(0x46), 100, 10_000);

        internal KagemushaPaymentRequestV1 Request() => Request((UInt128)25);

        internal KagemushaPaymentRequestV1 Request(UInt128 amount) => new(
            1, ReleaseId, NetworkId, Asset, Incarnation, 4, LiabilityPoolId, Account,
            amount, RecipientEncryptionKey, Credential, Repeat(0x61), 1_000, 2_000,
            Credential.GovernanceSignature);

        internal KagemushaPaymentV1 Payment(
            KagemushaPaymentRequestV1 request,
            int parityProofBytes = 1)
        {
            var encryptedCredit = KagemushaCodec.EncodeEncryptedCreditEnvelope(
                new KagemushaEncryptedCreditEnvelopeV1(
                    1,
                    new KagemushaX25519PublicKeyV1(Repeat(0xcc)),
                    Repeat(0xcd, 24),
                    Repeat(0xce, KagemushaCodec.EncryptedCreditCiphertextAndTagBytes)));
            var evidence = new KagemushaTrustedCommitTimeV1(Repeat(0xbc));
            var requestDigest = KagemushaCodec.PaymentRequestDigest(request);
            var transitionNullifier = Repeat(0xb8);
            var output = new KagemushaPaymentOutputV1(
                1,
                requestDigest,
                request.Amount,
                Repeat(0xb9),
                Repeat(0xba),
                transitionNullifier,
                KagemushaCodec.CreditId(transitionNullifier, requestDigest),
                Repeat(0xbb),
                evidence,
                1_500);
            var certificate = new KagemushaCommitCertificateV1(
                1, Repeat(0xd1), Repeat(0xd2), Repeat(0xd3), transitionNullifier,
                Repeat(0xd4), evidence, Credential.HardwareProfileId,
                Credential.PolicyEpoch, Repeat(0xd5));
            certificate = certificate with
            {
                CertificateId = KagemushaCodec.ExpectedCommitCertificateId(certificate),
            };
            var proof = new KagemushaPaymentProofV1(
                1,
                Repeat(0x81),
                Repeat(0x82),
                KagemushaCodec.PaymentBodyDigest(output, encryptedCredit, request),
                certificate.CandidateEnvelopeDigest,
                KagemushaCodec.CommitCertificateDigest(certificate),
                Repeat(0x86),
                Repeat(0x87),
                Repeat(1, parityProofBytes),
                Repeat(2, parityProofBytes),
                Repeat(0x88, KagemushaCodec.HistoryAccumulatorBytes),
                Repeat(0x89, KagemushaCodec.HistoryAccumulatorBytes));
            return new KagemushaPaymentV1(1, output, encryptedCredit, certificate, proof);
        }

        internal KagemushaAcknowledgementV1 Acknowledgement(
            KagemushaPaymentRequestV1 request,
            KagemushaPaymentV1 payment) => new(
                1,
                KagemushaCodec.PaymentRequestDigest(request),
                KagemushaCodec.PaymentDigest(payment, request),
                new KagemushaInboxReceiptV1(1, payment.Output.CreditId, Repeat(0xcb)),
                request.HardwareCredential.GovernanceSignature);
    }
}
