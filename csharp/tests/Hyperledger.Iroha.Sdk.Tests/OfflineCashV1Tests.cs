using System.Buffers.Binary;
using System.Reflection;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.OfflineCash;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class OfflineCashV1Tests
{
    [Fact]
    public void CircuitBoundDigestsUseExactFixedSemanticTranscripts()
    {
        var context = TestContext.Create();
        var request = context.Request(25);
        var authorization = context.Authorization(request, 25);
        var intent = authorization.Statement.Intent;
        var ticket = context.Ticket(request, authorization);
        var payment = context.Payment(request, ticket, intent);

        var intentTranscript = JoinBytes(
            LittleEndian(intent.Version), intent.RequestDigest.ToArray(), intent.IntentId.ToArray(),
            LittleEndian(intent.ExactAmount), intent.SenderOneTimeCommitment.ToArray());
        Assert.Equal(114, intentTranscript.Length);
        Assert.Equal(
            SemanticDigest("iroha:offline-cash:v1:acceptance-intent", intentTranscript),
            OfflineCashV1.AcceptanceIntentDigest(intent, request));
        Assert.NotEqual(intentTranscript, OfflineCashV1.EncodeAcceptanceIntent(intent, request));

        var statement = authorization.Statement;
        var authorizationTranscript = JoinBytes(
            LittleEndian(statement.Version), intentTranscript, statement.ReleaseId.ToArray(),
            statement.SuiteId.ToArray(), statement.VkDigest.ToArray(),
            statement.ArtifactManifestDigest.ToArray());
        Assert.Equal(244, authorizationTranscript.Length);
        Assert.Equal(
            SemanticDigest(
                "iroha:offline-cash:v1:acceptance-intent-authorization-statement",
                authorizationTranscript),
            OfflineCashV1.AcceptanceIntentAuthorizationStatementDigest(statement, request));

        var reservation = new OfflineCashOutboxReservationV1(
            Repeat(0xd0), OfflineCashOperationKindV1.SendSplit,
            OfflineCashV1.PaymentOutboxMinimumBytes, 1_000, 2_000);
        var reservationTranscript = JoinBytes(
            reservation.ReservationId.ToArray(), LittleEndian((uint)reservation.OperationKind),
            LittleEndian(reservation.ReservedOutboxBytes),
            LittleEndian(reservation.IssuedAtMilliseconds),
            LittleEndian(reservation.ExpiresAtMilliseconds));
        Assert.Equal(56, reservationTranscript.Length);
        Assert.Equal(
            SemanticDigest("iroha:offline-cash:v1:outbox-reservation", reservationTranscript),
            OfflineCashV1.OutboxReservationCommitment(reservation));

        var certificate = payment.CommitCertificate;
        var evidenceTranscript = CommitEvidenceTranscript(certificate.CommitEvidence);
        Assert.Equal(36, evidenceTranscript.Length);
        var certificateIdTranscript = JoinBytes(
            LittleEndian(certificate.Version), certificate.CandidateEnvelopeDigest.ToArray(),
            certificate.LifecycleBindingDigest.ToArray(), certificate.TransitionNullifier.ToArray(),
            certificate.OutboxReservationCommitment.ToArray(), evidenceTranscript,
            certificate.HardwareProfileId.ToArray(), LittleEndian(certificate.PolicyEpoch),
            certificate.HardwareTerminalCommitment.ToArray());
        Assert.Equal(238, certificateIdTranscript.Length);
        Assert.Equal(
            SemanticDigest("iroha:offline-cash:v1:commit-certificate-id", certificateIdTranscript),
            certificate.CertificateId.ToArray());

        var certificateTranscript = JoinBytes(
            LittleEndian(certificate.Version), certificate.CertificateId.ToArray(),
            certificate.CandidateEnvelopeDigest.ToArray(), certificate.LifecycleBindingDigest.ToArray(),
            certificate.TransitionNullifier.ToArray(), certificate.OutboxReservationCommitment.ToArray(),
            evidenceTranscript, certificate.HardwareProfileId.ToArray(),
            LittleEndian(certificate.PolicyEpoch), certificate.HardwareTerminalCommitment.ToArray());
        Assert.Equal(270, certificateTranscript.Length);
        Assert.Equal(
            SemanticDigest("iroha:offline-cash:v1:commit-certificate", certificateTranscript),
            payment.Proof.CommitCertificateDigest.ToArray());
    }

    [Fact]
    public void NoCommitDigestUsesExactFixedSemanticTranscript()
    {
        using var fixture = JsonDocument.Parse(
            File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Fixtures", "offline_cash_v1.json")));
        var fixtureRoot = RequireFixtureVersionOne(fixture.RootElement);
        var closure = OfflineCashV1.DecodeNoCommitClosure(
            FixtureBytes(fixtureRoot, "no_commit_closure"));
        var closureStatement = closure.Statement;
        var noCommitTranscript = JoinBytes(
            LittleEndian(closureStatement.Version), closureStatement.ReleaseId.ToArray(),
            closureStatement.SuiteId.ToArray(), closureStatement.VkDigest.ToArray(),
            closureStatement.ArtifactManifestDigest.ToArray(),
            closureStatement.SenderHardwareBindingCommitment.ToArray(),
            closureStatement.RequestId.ToArray(), closureStatement.RequestDigest.ToArray(),
            closureStatement.AcceptanceTicketId.ToArray(), closureStatement.TicketDigest.ToArray(),
            closureStatement.IntentAuthorizationDigest.ToArray(), closureStatement.IntentDigest.ToArray(),
            LittleEndian(closureStatement.ExactAmount),
            closureStatement.SenderOneTimeCommitment.ToArray(), closureStatement.RecoveryId.ToArray(),
            closureStatement.CancellationNullifier.ToArray(),
            closureStatement.EquivalentDeliverySlotCommitment.ToArray());
        Assert.Equal(498, noCommitTranscript.Length);
        Assert.Equal(
            SemanticDigest("iroha:offline-cash:v1:no-commit-closure-statement", noCommitTranscript),
            OfflineCashV1.NoCommitClosureStatementDigest(closureStatement));
    }

    [Fact]
    public void RustFixtureVersionOneRoundTripsEveryCanonicalOfflineCashPayload()
    {
        using var fixture = JsonDocument.Parse(
            File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Fixtures", "offline_cash_v1.json")));
        var root = RequireFixtureVersionOne(fixture.RootElement);

        var requestBytes = FixtureBytes(root, "payment_request");
        var request = OfflineCashV1.DecodePaymentRequest(requestBytes);
        AssertFixture(root, "payment_request", OfflineCashV1.PayloadKind.PaymentRequest,
            requestBytes, OfflineCashV1.EncodePaymentRequest(request));

        var authorizationBytes = FixtureBytes(root, "acceptance_intent_authorization");
        var authorization = OfflineCashV1.DecodeAcceptanceIntentAuthorization(authorizationBytes, request);
        AssertFixture(root, "acceptance_intent_authorization",
            OfflineCashV1.PayloadKind.AcceptanceIntentAuthorization, authorizationBytes,
            OfflineCashV1.EncodeAcceptanceIntentAuthorization(authorization, request));

        var ticketBytes = FixtureBytes(root, "acceptance_ticket");
        var ticket = OfflineCashV1.DecodeAcceptanceTicket(ticketBytes, request, authorization);
        AssertFixture(root, "acceptance_ticket", OfflineCashV1.PayloadKind.AcceptanceTicket,
            ticketBytes, OfflineCashV1.EncodeAcceptanceTicket(ticket, request, authorization));

        var paymentBytes = FixtureBytes(root, "payment");
        var payment = OfflineCashV1.DecodePayment(paymentBytes, request);
        AssertFixture(root, "payment", OfflineCashV1.PayloadKind.Payment,
            paymentBytes, OfflineCashV1.EncodePayment(payment, request));

        var acknowledgementBytes = FixtureBytes(root, "acknowledgement");
        var acknowledgement = OfflineCashV1.DecodeAcknowledgement(acknowledgementBytes, request, payment);
        AssertFixture(root, "acknowledgement", OfflineCashV1.PayloadKind.Acknowledgement,
            acknowledgementBytes,
            OfflineCashV1.EncodeAcknowledgement(acknowledgement, request, payment));

        var closureBytes = FixtureBytes(root, "no_commit_closure");
        var closure = OfflineCashV1.DecodeNoCommitClosure(closureBytes);
        AssertFixture(root, "no_commit_closure", OfflineCashV1.PayloadKind.NoCommitClosure,
            closureBytes, OfflineCashV1.EncodeNoCommitClosure(closure));

        var mintAuthorizationBytes = FixtureBytes(root, "mint_authorization");
        var mintAuthorization = OfflineCashV1.DecodeMintAuthorization(mintAuthorizationBytes);
        AssertFixture(root, "mint_authorization", OfflineCashV1.PayloadKind.MintAuthorization,
            mintAuthorizationBytes, OfflineCashV1.EncodeMintAuthorization(mintAuthorization));

        var mintCreditBytes = FixtureBytes(root, "mint_credit");
        var mintCredit = OfflineCashV1.DecodeMintCredit(mintCreditBytes, mintAuthorization);
        AssertFixture(root, "mint_credit", OfflineCashV1.PayloadKind.MintCredit,
            mintCreditBytes, OfflineCashV1.EncodeMintCredit(mintCredit, mintAuthorization));

        var redemptionBytes = FixtureBytes(root, "redemption_voucher");
        var redemption = OfflineCashV1.DecodeRedemptionVoucher(redemptionBytes);
        AssertFixture(root, "redemption_voucher", OfflineCashV1.PayloadKind.RedemptionVoucher,
            redemptionBytes, OfflineCashV1.EncodeRedemptionVoucher(redemption));

        var openingBytes = FixtureBytes(root, "credit_opening");
        Assert.Equal(openingBytes,
            OfflineCashV1.EncodeCreditOpening(OfflineCashV1.DecodeCreditOpening(openingBytes)));
        var aadBytes = FixtureBytes(root, "encrypted_credit_aad");
        Assert.Equal(aadBytes,
            OfflineCashV1.EncodeEncryptedCreditAad(OfflineCashV1.DecodeEncryptedCreditAad(aadBytes)));
        var envelopeBytes = FixtureBytes(root, "encrypted_credit_envelope");
        Assert.Equal(envelopeBytes, OfflineCashV1.EncodeEncryptedCreditEnvelope(
            OfflineCashV1.DecodeEncryptedCreditEnvelope(envelopeBytes)));

        Assert.Equal(
            request.AssetIncarnation.Bytes(),
            ticket.AssetIncarnation.Bytes());
        Assert.Equal(
            request.AssetIncarnation.Bytes(),
            mintAuthorization.Statement.Context.AssetIncarnation.Bytes());
        Assert.Equal(
            request.AssetIncarnation.Bytes(),
            redemption.Statement.Lifecycle.AssetIncarnation.Bytes());
    }

    [Fact]
    public void TerminalCertificateWrapperAndRedemptionBindingsFailClosed()
    {
        using var fixture = JsonDocument.Parse(
            File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Fixtures", "offline_cash_v1.json")));
        var root = RequireFixtureVersionOne(fixture.RootElement);
        var request = OfflineCashV1.DecodePaymentRequest(FixtureBytes(root, "payment_request"));
        var payment = OfflineCashV1.DecodePayment(FixtureBytes(root, "payment"), request);

        var paymentSubstitutions = new[]
        {
            payment with
            {
                CommitCertificate = payment.CommitCertificate with { LifecycleBindingDigest = Repeat(0xe1) },
            },
            payment with
            {
                CommitCertificate = payment.CommitCertificate with { CertificateId = Repeat(0xe2) },
            },
            payment with { Proof = payment.Proof with { SemanticDigest = Repeat(0xe3) } },
            payment with { Proof = payment.Proof with { CandidateEnvelopeDigest = Repeat(0xe4) } },
            payment with { Proof = payment.Proof with { CommitCertificateDigest = Repeat(0xe5) } },
        };
        foreach (var substituted in paymentSubstitutions)
            Assert.Throws<ArgumentException>(() => OfflineCashV1.EncodePayment(substituted, request));

        var redemption = OfflineCashV1.DecodeRedemptionVoucher(FixtureBytes(root, "redemption_voucher"));
        Assert.Throws<ArgumentException>(() => OfflineCashV1.EncodeRedemptionVoucher(
            redemption with { Statement = redemption.Statement with { RedemptionId = Repeat(0xe6) } }));
        Assert.Throws<ArgumentException>(() => OfflineCashV1.EncodeRedemptionVoucher(
            redemption with { Proof = redemption.Proof with { SemanticDigest = Repeat(0xe7) } }));
        Assert.Throws<ArgumentException>(() => OfflineCashV1.EncodeRedemptionVoucher(
            redemption with { Proof = redemption.Proof with { CommitCertificateDigest = Repeat(0xe8) } }));
    }

    [Fact]
    public void PredecessorBareAssetIncarnationPayloadIsRejected()
    {
        const string requestSchema =
            "iroha_data_model::offline::offline_cash_v1::OfflineCashPaymentRequestV1";
        var canonical = OfflineCashV1.EncodePaymentRequest(TestContext.Create().Request(25));
        var (payload, flags) = NoritoCodec.Decode(requestSchema, canonical);
        var reader = new CanonicalNoritoReader(payload, "fixture request", nameof(canonical));
        var writer = new CanonicalNoritoWriter();
        for (var fieldIndex = 0; !reader.IsFinished; fieldIndex++)
        {
            var field = reader.ReadField($"field{fieldIndex}");
            if (fieldIndex == 4)
            {
                var incarnation = new CanonicalNoritoReader(
                    field, "fixture incarnation", nameof(canonical));
                var raw = incarnation.ReadField("hash");
                Assert.Equal(32, raw.Length);
                incarnation.RequireEnd();
                writer.WriteField(raw);
            }
            else
            {
                writer.WriteField(field);
            }
        }
        reader.RequireEnd();
        var predecessor = AlignedFrame(requestSchema, writer.ToArray(), flags, 16);

        Assert.Throws<ArgumentException>(() => OfflineCashV1.DecodePaymentRequest(predecessor));
    }

    [Fact]
    public void PositiveExactRequestAmountRoundTripsCanonically()
    {
        var request = TestContext.Create().Request(10);
        var encoded = OfflineCashV1.EncodePaymentRequest(request);
        Assert.Equal((UInt128)10, OfflineCashV1.DecodePaymentRequest(encoded).Amount);
        Assert.Throws<ArgumentException>(() =>
            OfflineCashV1.EncodePaymentRequest(request with { Amount = 0 }));
    }

    [Fact]
    public void RequestRawAndTextLimitsAreExactly1024And1370()
    {
        Assert.Equal(1_024, OfflineCashV1.MaximumRequestBytes);
        var maximum = Enumerable.Repeat((byte)0x5a, OfflineCashV1.MaximumRequestBytes).ToArray();
        var text = OfflineCashV1.EncodeText(OfflineCashV1.PayloadKind.PaymentRequest, maximum);
        Assert.Equal(1_370, text.Length);
        Assert.Equal(maximum, OfflineCashV1.DecodeText(OfflineCashV1.PayloadKind.PaymentRequest, text));

        Assert.Throws<ArgumentException>(() => OfflineCashV1.EncodeText(
            OfflineCashV1.PayloadKind.PaymentRequest,
            new byte[OfflineCashV1.MaximumRequestBytes + 1]));
        Assert.Throws<FormatException>(() => OfflineCashV1.DecodeText(
            OfflineCashV1.PayloadKind.PaymentRequest,
            "oc1:" + new string('A', 1_367)));
    }

    [Fact]
    public void CurrentRequestShapeFitsBothHardCaps()
    {
        var context = TestContext.Create();
        var request = context.Request(100);
        var raw = OfflineCashV1.EncodePaymentRequest(request);
        var text = OfflineCashV1.EncodeText(OfflineCashV1.PayloadKind.PaymentRequest, raw);
        Assert.InRange(raw.Length, 1, 1_024);
        Assert.InRange(text.Length, 5, 1_370);
        Assert.Equal(raw, OfflineCashV1.EncodePaymentRequest(OfflineCashV1.DecodePaymentRequest(raw)));
    }

    [Fact]
    public void TypedCreditOpeningAadAndEnvelopeRoundTrip()
    {
        var opening = new OfflineCashCreditOpeningV1(
            1, Repeat(0x11), 25, Repeat(0x12), Repeat(0x13), Repeat(0x14));
        var openingBytes = OfflineCashV1.EncodeCreditOpening(opening);
        Assert.Equal(OfflineCashV1.CreditOpeningCanonicalBytes, openingBytes.Length);
        Assert.Equal(openingBytes,
            OfflineCashV1.EncodeCreditOpening(OfflineCashV1.DecodeCreditOpening(openingBytes)));

        var aad = new OfflineCashEncryptedCreditAadV1(
            1, OfflineCashEncryptedCreditPurposeV1.Peer,
            Repeat(0x21), Repeat(0x22), Repeat(0x23), 25);
        var aadBytes = OfflineCashV1.EncodeEncryptedCreditAad(aad);
        Assert.Equal(aadBytes,
            OfflineCashV1.EncodeEncryptedCreditAad(OfflineCashV1.DecodeEncryptedCreditAad(aadBytes)));

        var envelope = new OfflineCashEncryptedCreditEnvelopeV1(
            1,
            new OfflineCashX25519PublicKeyV1(Repeat(0x31)),
            Enumerable.Repeat((byte)0x32, 24).ToArray(),
            Enumerable.Repeat((byte)0x33, OfflineCashV1.EncryptedCreditCiphertextAndTagBytes).ToArray());
        var envelopeBytes = OfflineCashV1.EncodeEncryptedCreditEnvelope(envelope);
        Assert.InRange(envelopeBytes.Length, 1, OfflineCashV1.MaximumEncryptedCreditBytes);
        Assert.Equal(envelopeBytes,
            OfflineCashV1.EncodeEncryptedCreditEnvelope(OfflineCashV1.DecodeEncryptedCreditEnvelope(envelopeBytes)));
    }

    [Fact]
    public void MintCreditIsDecodedOnlyAgainstItsExactAuthorizationWhenContextIsSupplied()
    {
        var context = TestContext.Create();
        var authorization = context.MintAuthorization();
        var credit = context.MintCredit(authorization);
        var bytes = OfflineCashV1.EncodeMintCredit(credit, authorization);

        Assert.Equal(bytes, OfflineCashV1.EncodeMintCredit(
            OfflineCashV1.DecodeMintCredit(bytes, authorization), authorization));

        var differentStatement = authorization.Statement with { IssuanceCommitment = Repeat(0x7e) };
        var different = authorization with
        {
            Statement = differentStatement,
            Proof = context.Proof(OfflineCashV1.MintAuthorizationStatementDigest(differentStatement)),
        };
        Assert.Throws<ArgumentException>(() => OfflineCashV1.DecodeMintCredit(bytes, different));
    }

    [Fact]
    public void NoCommitClosureRejectsEveryAuthorizationProtocolContextSubstitution()
    {
        using var fixture = JsonDocument.Parse(
            File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Fixtures", "offline_cash_v1.json")));
        var root = RequireFixtureVersionOne(fixture.RootElement);
        var closure = OfflineCashV1.DecodeNoCommitClosure(FixtureBytes(root, "no_commit_closure"));
        var substitutions = new[]
        {
            closure.Statement with { ReleaseId = Repeat(0xd1) },
            closure.Statement with { SuiteId = Repeat(0xd2) },
            closure.Statement with { VkDigest = Repeat(0xd3) },
            closure.Statement with { ArtifactManifestDigest = Repeat(0xd4) },
        };

        foreach (var statement in substitutions)
        {
            var proof = closure.Proof with
            {
                SemanticDigest = OfflineCashV1.NoCommitClosureStatementDigest(statement),
            };
            var substituted = closure with { Statement = statement, Proof = proof };
            Assert.Throws<ArgumentException>(() => OfflineCashV1.EncodeNoCommitClosure(substituted));
        }
    }

    [Fact]
    public void PaymentAndCurrentProofExposeNoSenderStateLinkageFields()
    {
        var forbidden = new[]
        {
            "senderbefore", "senderafter", "predecessor", "successor",
            "senderlane", "senderepoch", "sendersequence", "linkage",
        };
        foreach (var type in new[]
                 {
                     typeof(OfflineCashPaymentV1),
                     typeof(OfflineCashTransferStatementV1),
                     typeof(OfflineCashPairedProofV1),
                     typeof(OfflineCashCommitWrapperProofV1),
                 })
        {
            var names = type.GetProperties(BindingFlags.Public | BindingFlags.Instance)
                .Select(property => property.Name.ToLowerInvariant())
                .ToArray();
            Assert.DoesNotContain(names, name => forbidden.Any(name.Contains));
        }

        var closureFields = typeof(OfflineCashNoCommitClosureV1)
            .GetProperties(BindingFlags.Public | BindingFlags.Instance)
            .Select(property => property.Name)
            .ToHashSet(StringComparer.Ordinal);
        Assert.Contains(nameof(OfflineCashNoCommitClosureV1.Request), closureFields);
        Assert.Contains(nameof(OfflineCashNoCommitClosureV1.IntentAuthorization), closureFields);
        Assert.Contains(nameof(OfflineCashNoCommitClosureV1.AcceptanceTicket), closureFields);
        Assert.Equal(16_384, OfflineCashV1.MaximumNoCommitClosureBytes);
    }

    [Fact]
    public void PeerWireUsesExactFiveMessageKindsAndRoundTripsIpm1()
    {
        Assert.Equal(
            new byte[] { 1, 2, 3, 4, 5 },
            Enum.GetValues<IrohaPeerPayloadKindV1>().Select(value => (byte)value).ToArray());

        var context = TestContext.Create();
        var request = context.Request(25);
        var authorization = context.Authorization(request, 25);
        var ticket = context.Ticket(request, authorization);
        var payment = context.Payment(request, ticket, authorization.Statement.Intent);
        var acknowledgement = context.Acknowledgement(request, payment);
        Assert.InRange(
            OfflineCashV1.ValidateCompleteExchange(request, authorization, ticket, payment, acknowledgement),
            1,
            OfflineCashV1.MaximumCompleteExchangeRawBytes);

        var payloads = new (IrohaPeerPayloadKindV1 Kind, byte[] Canonical)[]
        {
            (IrohaPeerPayloadKindV1.ReceiveRequest, OfflineCashV1.EncodePaymentRequest(request)),
            (IrohaPeerPayloadKindV1.AcceptanceIntentAuthorization,
                OfflineCashV1.EncodeAcceptanceIntentAuthorization(authorization, request)),
            (IrohaPeerPayloadKindV1.AcceptanceTicket,
                OfflineCashV1.EncodeAcceptanceTicket(ticket, request, authorization)),
            (IrohaPeerPayloadKindV1.Payment, OfflineCashV1.EncodePayment(payment, request)),
            (IrohaPeerPayloadKindV1.Acknowledgement,
                OfflineCashV1.EncodeAcknowledgement(acknowledgement, request, payment)),
        };

        foreach (var (kind, canonical) in payloads)
        {
            var message = IrohaPeerOfflineCashAdapterV1.Wrap(kind, canonical);
            var frame = message.Encode();
            Assert.Equal("IPM1", System.Text.Encoding.ASCII.GetString(frame, 0, 4));
            Assert.Equal(IrohaPeerWireMessageV1.HeaderLength + canonical.Length, frame.Length);
            var decoded = IrohaPeerWireMessageV1.Decode(
                frame,
                IrohaPeerPayloadProfileV1.OfflineCashV1,
                kind);
            Assert.Equal(canonical, IrohaPeerOfflineCashAdapterV1.Decode(decoded));
            Assert.Equal(frame, decoded.Encode());
        }

        var tampered = IrohaPeerOfflineCashAdapterV1.Wrap(
            payloads[0].Kind, payloads[0].Canonical).Encode();
        tampered[52] ^= 1;
        Assert.Throws<ArgumentException>(() => IrohaPeerWireMessageV1.Decode(tampered));
    }

    [Fact]
    public void NativeShapeBoundaryIsExplicitlyNonAuthoritativeAndHasNoManagedFallback()
    {
        var publicMethods = typeof(OfflineCashNativeShapeV1)
            .GetMethods(BindingFlags.Public | BindingFlags.Static | BindingFlags.DeclaredOnly)
            .Where(method => method.Name.StartsWith("Validate", StringComparison.Ordinal))
            .ToArray();
        Assert.NotEmpty(publicMethods);
        Assert.All(publicMethods, method => Assert.EndsWith("Shape", method.Name, StringComparison.Ordinal));
        Assert.Contains(publicMethods, method => method.Name == "ValidateNoCommitClosureShape");
        Assert.Contains(publicMethods, method => method.Name == "ValidateCompleteExchangeShape");
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            OfflineCashNativeShapeV1.ValidatePaymentRequestShape([]));
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            OfflineCashNativeShapeV1.ValidateNoCommitClosureShape([]));
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            OfflineCashNativeShapeV1.ValidateCompleteExchangeShape([], [], [], [], []));
        Assert.Throws<FormatException>(() =>
            OfflineCashNativeShapeV1.ValidateNoCommitClosureTextShape("oc0:AA"));

        var concreteProviders = typeof(IOfflineCashNativeHardwareProviderV1).Assembly.GetTypes()
            .Where(type => !type.IsAbstract
                && !type.IsInterface
                && typeof(IOfflineCashNativeHardwareProviderV1).IsAssignableFrom(type));
        Assert.Empty(concreteProviders);
    }

    [Fact]
    public void WalletRejectsAnyMissingHardwareCapability()
    {
        var context = TestContext.Create();
        var incomplete = context.Qualification(
            Enum.GetValues<OfflineCashHardwareCapabilityV1>()
                .Where(value => value != OfflineCashHardwareCapabilityV1.NoSoftwareFallback));

        Assert.Throws<InvalidOperationException>(incomplete.RequireProductionReady);
        context.Qualification(Enum.GetValues<OfflineCashHardwareCapabilityV1>()).RequireProductionReady();
    }

    [Fact]
    public void WalletDrainsOneStableWatermarkInRepeatedSingleCreditFolds()
    {
        var context = TestContext.Create();
        var provider = new FoldOnlyNativeProvider(
            context, Enumerable.Repeat(true, 33).Append(false), 73);
        var wallet = OfflineCashWalletV1.Open(provider);

        Assert.Equal((UInt128)33, wallet.DrainPendingCredits());
        Assert.Equal(Enumerable.Repeat((UInt128)73, 34), provider.FoldWatermarks);
        Assert.Equal((UInt128)33, wallet.JournalRevision);
    }

    private static byte[] FixtureBytes(JsonElement root, string name)
    {
        var payload = root.GetProperty(name);
        var hex = payload.GetProperty("norito_hex").GetString()
            ?? throw new InvalidDataException($"Offline Cash fixture {name} has no canonical bytes.");
        var bytes = Convert.FromHexString(hex);
        Assert.Equal(payload.GetProperty("raw_bytes").GetInt32(), bytes.Length);
        return bytes;
    }

    private static void AssertFixture(
        JsonElement root,
        string name,
        OfflineCashV1.PayloadKind kind,
        byte[] canonical,
        byte[] reencoded)
    {
        Assert.Equal(canonical, reencoded);
        var text = root.GetProperty(name).GetProperty("oc1").GetString()
            ?? throw new InvalidDataException($"Offline Cash fixture {name} has no text shape.");
        Assert.Equal(text, OfflineCashV1.EncodeText(kind, canonical));
        Assert.Equal(canonical, OfflineCashV1.DecodeText(kind, text));
    }

    private static byte[] AlignedFrame(string schema, byte[] payload, byte flags, int alignment)
    {
        var archive = NoritoCodec.Encode(schema, payload, flags);
        var padding = (alignment - NoritoHeader.EncodedLength % alignment) % alignment;
        if (padding == 0) return archive;
        var result = new byte[archive.Length + padding];
        archive.AsSpan(0, NoritoHeader.EncodedLength).CopyTo(result);
        archive.AsSpan(NoritoHeader.EncodedLength)
            .CopyTo(result.AsSpan(NoritoHeader.EncodedLength + padding));
        return result;
    }

    private static JsonElement RequireFixtureVersionOne(JsonElement root)
    {
        if (!root.TryGetProperty("fixture_version", out var version)
            || version.ValueKind != JsonValueKind.Number
            || version.GetInt32() != 1)
            throw new InvalidDataException("Offline Cash parity requires fixture_version 1.");
        return root;
    }

    private static byte[] Repeat(byte value) => Enumerable.Repeat(value, 32).ToArray();

    private static byte[] CommitEvidenceTranscript(OfflineCashCommitEvidenceV1 evidence) => evidence switch
    {
        OfflineCashTrustedCommitTimeV1 trusted =>
            JoinBytes(LittleEndian(0U), trusted.TimeEvidenceCommitment.ToArray()),
        OfflineCashMonotonicCommitLeaseV1 lease =>
            JoinBytes(LittleEndian(1U), lease.LeaseEvidenceCommitment.ToArray()),
        _ => throw new ArgumentOutOfRangeException(nameof(evidence)),
    };

    private static byte[] SemanticDigest(string domain, byte[] transcript)
    {
        var length = new byte[8];
        BinaryPrimitives.WriteUInt64LittleEndian(length, (ulong)transcript.Length);
        return SHA256.HashData(JoinBytes(Encoding.ASCII.GetBytes(domain), [0], length, transcript));
    }

    private static byte[] LittleEndian(ushort value)
    {
        var bytes = new byte[2];
        BinaryPrimitives.WriteUInt16LittleEndian(bytes, value);
        return bytes;
    }

    private static byte[] LittleEndian(uint value)
    {
        var bytes = new byte[4];
        BinaryPrimitives.WriteUInt32LittleEndian(bytes, value);
        return bytes;
    }

    private static byte[] LittleEndian(ulong value)
    {
        var bytes = new byte[8];
        BinaryPrimitives.WriteUInt64LittleEndian(bytes, value);
        return bytes;
    }

    private static byte[] LittleEndian(UInt128 value)
    {
        var bytes = new byte[16];
        BinaryPrimitives.WriteUInt128LittleEndian(bytes, value);
        return bytes;
    }

    private static byte[] JoinBytes(params byte[][] values)
    {
        var result = new byte[values.Sum(value => value.Length)];
        var offset = 0;
        foreach (var value in values)
        {
            value.CopyTo(result, offset);
            offset += value.Length;
        }
        return result;
    }

    private sealed class TestContext
    {
        private TestContext(
            NetworkId networkId,
            OfflineCashAssetDefinitionIdV1 asset,
            OfflineCashAssetIncarnationV1 incarnation,
            OfflineCashAccountIdV1 account,
            OfflineCashDevicePublicKeyV1 publicKey,
            OfflineCashDeviceSignatureV1 signature,
            OfflineCashHardwareProfileV1 profile,
            OfflineCashHardwareCredentialV1 credential,
            byte[] liabilityPoolId,
            byte[] releaseId)
        {
            NetworkId = networkId;
            Asset = asset;
            Incarnation = incarnation;
            Account = account;
            PublicKey = publicKey;
            Signature = signature;
            Profile = profile;
            Credential = credential;
            LiabilityPoolId = liabilityPoolId;
            ReleaseId = releaseId;
        }

        internal NetworkId NetworkId { get; }
        internal OfflineCashAssetDefinitionIdV1 Asset { get; }
        internal OfflineCashAssetIncarnationV1 Incarnation { get; }
        internal OfflineCashAccountIdV1 Account { get; }
        internal OfflineCashDevicePublicKeyV1 PublicKey { get; }
        internal OfflineCashDeviceSignatureV1 Signature { get; }
        internal OfflineCashHardwareProfileV1 Profile { get; }
        internal OfflineCashHardwareCredentialV1 Credential { get; }
        internal byte[] LiabilityPoolId { get; }
        internal byte[] ReleaseId { get; }

        internal static TestContext Create()
        {
            var network = Enumerable.Range(1, 32).Select(value => (byte)value).ToArray();
            network[^1] |= 1;
            var networkId = NetworkId.FromBytes(network);
            var asset = new OfflineCashAssetDefinitionIdV1("6TEAJqbb8oEPmLncoNiMRbLEK6tw");
            var incarnationBytes = new byte[32];
            incarnationBytes[0] = 1;
            incarnationBytes[^1] = 1;
            var incarnation = new OfflineCashAssetIncarnationV1(incarnationBytes);
            var account = new OfflineCashAccountIdV1(
                "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV");
            var sec1 = Enumerable.Repeat((byte)1, 65).ToArray();
            sec1[0] = 4;
            var publicKey = new OfflineCashDevicePublicKeyV1(sec1);
            var rawSignature = new byte[64];
            rawSignature[31] = 1;
            rawSignature[63] = 1;
            var signature = new OfflineCashDeviceSignatureV1(rawSignature);
            var profileId = Repeat(0x42);
            var releaseId = Repeat(0x41);
            var keyReference = OfflineCashV1.DeviceKeyReference(publicKey);
            var profile = new OfflineCashHardwareProfileV1(
                1, 1, profileId, Repeat(0x43), OfflineCashHardwarePlatformClassV1.DedicatedSecureElement,
                Repeat(0x44), Repeat(0x45), Repeat(0x46), Repeat(0x47), Repeat(0x48), 9,
                publicKey, ushort.MaxValue, Repeat(0x49), 100, 10_000);
            var credential = new OfflineCashHardwareCredentialV1(
                1, Repeat(0x51), networkId, profileId, Repeat(0x52), Repeat(0x45), 9,
                Repeat(0x53), Repeat(0x54), 1, publicKey, keyReference, 100, 10_000, signature);
            return new TestContext(
                networkId, asset, incarnation, account, publicKey, signature, profile, credential,
                OfflineCashV1.LiabilityPoolId(networkId, asset, incarnation), releaseId);
        }

        internal OfflineCashPaymentRequestV1 Request(UInt128 amount = 25) => new(
            1, ReleaseId, NetworkId, Asset, Incarnation, 4, LiabilityPoolId, Account,
            amount, Credential, Repeat(0x61), 1_000, 2_000, Signature);

        internal OfflineCashAcceptanceIntentAuthorizationV1 Authorization(
            OfflineCashPaymentRequestV1 request,
            UInt128 exactAmount)
        {
            var intent = new OfflineCashAcceptanceIntentV1(
                1, OfflineCashV1.PaymentRequestDigest(request), Repeat(0xb1), exactAmount, Repeat(0xb2));
            var statement = new OfflineCashAcceptanceIntentAuthorizationStatementV1(
                1, intent, request.ReleaseId, request.HardwareCredential.SuiteId,
                Repeat(0xb3), Repeat(0xb4));
            return new OfflineCashAcceptanceIntentAuthorizationV1(
                1, statement,
                Proof(OfflineCashV1.AcceptanceIntentAuthorizationStatementDigest(statement, request)));
        }

        internal OfflineCashAcceptanceTicketV1 Ticket(
            OfflineCashPaymentRequestV1 request,
            OfflineCashAcceptanceIntentAuthorizationV1 authorization)
        {
            var intent = authorization.Statement.Intent;
            return new OfflineCashAcceptanceTicketV1(
                1, request.NetworkId, request.RequestId, OfflineCashV1.PaymentRequestDigest(request),
                Repeat(0xb5), request.Asset, request.AssetIncarnation, request.Scale,
                OfflineCashV1.AcceptanceIntentDigest(intent, request), intent.ExactAmount,
                OfflineCashV1.AcceptanceTicketMinimumReservedInboxBytes,
                new OfflineCashX25519PublicKeyV1(Repeat(0xb6)),
                request.HardwareCredential.HardwareProfileId, request.HardwareCredential.PolicyEpoch,
                1_100, 1_900, Signature);
        }

        internal OfflineCashPaymentV1 Payment(
            OfflineCashPaymentRequestV1 request,
            OfflineCashAcceptanceTicketV1 ticket,
            OfflineCashAcceptanceIntentV1 intent)
        {
            var encryptedCredit = PeerEncryptedCredit();
            var creditId = Repeat(0xb7);
            var transitionNullifier = Repeat(0xb8);
            var evidence = new OfflineCashTrustedCommitTimeV1(Repeat(0xb9));
            var lifecycle = new OfflineCashLifecycleBindingV1(
                1, request.NetworkId, 1, request.HardwareCredential.SuiteId, Repeat(0xba), request.ReleaseId,
                request.Asset, request.AssetIncarnation, request.Scale, request.LiabilityPoolId,
                request.HardwareCredential.HardwareProfileId, request.HardwareCredential.PolicyEpoch,
                OfflineCashOperationKindV1.SendSplit, request.RequestId, ticket.AcceptanceTicketId,
                creditId, OfflineCashV1.CiphertextDigest(encryptedCredit));
            var statement = new OfflineCashTransferStatementV1(
                1, lifecycle, ticket.ExactAmount, transitionNullifier,
                OfflineCashV1.PaymentRequestDigest(request),
                OfflineCashV1.AcceptanceTicketDigest(ticket, request, intent),
                ticket.RecipientOneTimeKey, Repeat(0xbb), evidence);
            var certificate = new OfflineCashCommitCertificateV1(
                1, Repeat(0xbc), Repeat(0xbd), OfflineCashV1.LifecycleBindingDigestUnchecked(lifecycle),
                transitionNullifier,
                Repeat(0xbf), evidence, lifecycle.HardwareProfileId, lifecycle.PolicyEpoch, Repeat(0xc0));
            certificate = certificate with
            {
                CertificateId = OfflineCashV1.CommitCertificateIdUnchecked(certificate),
            };
            var wrapper = new OfflineCashCommitWrapperProofV1(
                1, Repeat(0xc1), Repeat(0xc2), OfflineCashV1.TransferStatementDigestUnchecked(statement),
                certificate.CandidateEnvelopeDigest, OfflineCashV1.CommitCertificateDigestUnchecked(certificate),
                Repeat(0xc6), Repeat(0xc7), new byte[] { 3 }, new byte[] { 4 },
                Enumerable.Repeat((byte)0xc8, OfflineCashV1.HistoryAccumulatorBytes).ToArray(),
                Enumerable.Repeat((byte)0xc9, OfflineCashV1.HistoryAccumulatorBytes).ToArray());
            return new OfflineCashPaymentV1(
                1, statement, intent, ticket, certificate, wrapper, encryptedCredit, Repeat(0xca));
        }

        internal OfflineCashAcknowledgementV1 Acknowledgement(
            OfflineCashPaymentRequestV1 request,
            OfflineCashPaymentV1 payment) => new(
                1, OfflineCashV1.PaymentRequestDigest(request), OfflineCashV1.PaymentDigest(payment, request),
                new OfflineCashInboxReceiptV1(
                    1, payment.Statement.Lifecycle.CreditId, Repeat(0xcb)),
                Signature);

        internal OfflineCashAggregateStateCommitmentV1 Aggregate(UInt128 sequence) => new(
            1, ReleaseId, NetworkId, Asset, Incarnation, 4, LiabilityPoolId,
            Repeat(0x62), Repeat(0x54), Credential.DeviceKeyReference,
            Profile.HardwareProfileId, sequence, Repeat((byte)(0x70 + (byte)sequence)));

        internal OfflineCashHardwareQualificationV1 Qualification(
            IEnumerable<OfflineCashHardwareCapabilityV1> capabilities) => new(
                1, Profile, Credential, ReleaseId, capabilities);

        internal OfflineCashPairedProofV1 Proof(byte[]? semanticDigest = null) => new(
            1, Repeat(0x81), Repeat(0x82), semanticDigest ?? Repeat(0x83), Repeat(0x84), Repeat(0x85),
            Repeat(0x86), Repeat(0x87), new byte[] { 1 }, new byte[] { 2 },
            Enumerable.Repeat((byte)0x88, OfflineCashV1.HistoryAccumulatorBytes).ToArray(),
            Enumerable.Repeat((byte)0x89, OfflineCashV1.HistoryAccumulatorBytes).ToArray());

        internal OfflineCashMintAuthorizationV1 MintAuthorization()
        {
            var context = new OfflineCashMintAuthorizationContextV1(
                1, Repeat(0x91), ReleaseId, Credential.SuiteId, Repeat(0x92), Repeat(0x93),
                NetworkId, Asset, Incarnation, 4, LiabilityPoolId, 50, Account, Account,
                Credential.CredentialId, Profile.HardwareProfileId, Profile.PolicyEpoch,
                Repeat(0x94), Repeat(0x95), new OfflineCashX25519PublicKeyV1(Repeat(0x96)));
            var statement = new OfflineCashMintAuthorizationStatementV1(
                1, context, Repeat(0x97), Repeat(0x98),
                OfflineCashV1.CiphertextDigest(MintEncryptedCredit()));
            return new OfflineCashMintAuthorizationV1(
                1, statement, Proof(OfflineCashV1.MintAuthorizationStatementDigest(statement)));
        }

        internal OfflineCashMintCreditV1 MintCredit(OfflineCashMintAuthorizationV1 authorization)
        {
            var context = authorization.Statement.Context;
            var lifecycle = new OfflineCashLifecycleBindingV1(
                1, NetworkId, 1, context.SuiteId, context.VkDigest, context.ReleaseId,
                Asset, Incarnation, 4, LiabilityPoolId, context.HardwareProfileId,
                context.PolicyEpoch, OfflineCashOperationKindV1.MintFold,
                new byte[32], new byte[32], authorization.Statement.CreditId,
                authorization.Statement.CiphertextDigest);
            var envelope = MintEncryptedCredit();
            var statement = new OfflineCashMintCreditStatementV1(
                1, lifecycle, context.RecipientCredentialCommitment,
                OfflineCashV1.MintAuthorizationContextDigest(context),
                OfflineCashV1.MintAuthorizationDigest(authorization), context.Amount,
                authorization.Statement.IssuanceCommitment, context.Recipient,
                context.CreditCommitment, 1_500);
            return new OfflineCashMintCreditV1(
                1,
                statement, Proof(OfflineCashV1.MintCreditStatementDigest(statement)),
                Repeat(0xa6), Repeat(0xa7), Repeat(0xa8), Repeat(0xa9),
                envelope, context.ArtifactManifestDigest);
        }

        private static byte[] MintEncryptedCredit() => OfflineCashV1.EncodeEncryptedCreditEnvelope(
            new OfflineCashEncryptedCreditEnvelopeV1(
                1, new OfflineCashX25519PublicKeyV1(Repeat(0xa1)),
                Enumerable.Repeat((byte)0xa2, 24).ToArray(),
                Enumerable.Repeat((byte)0xa3,
                    OfflineCashV1.EncryptedCreditCiphertextAndTagBytes).ToArray()));

        private static byte[] PeerEncryptedCredit() => OfflineCashV1.EncodeEncryptedCreditEnvelope(
            new OfflineCashEncryptedCreditEnvelopeV1(
                1, new OfflineCashX25519PublicKeyV1(Repeat(0xcc)),
                Enumerable.Repeat((byte)0xcd, 24).ToArray(),
                Enumerable.Repeat((byte)0xce,
                    OfflineCashV1.EncryptedCreditCiphertextAndTagBytes).ToArray()));
    }

    private sealed class FoldOnlyNativeProvider : IOfflineCashNativeHardwareProviderV1
    {
        private readonly TestContext context;
        private readonly Queue<bool> folds;
        private readonly UInt128 watermark;
        private UInt128 revision;
        private UInt128 sequence = 1;

        internal FoldOnlyNativeProvider(TestContext context, IEnumerable<bool> folds, UInt128 watermark)
        {
            this.context = context;
            this.folds = new Queue<bool>(folds);
            this.watermark = watermark;
        }

        internal List<UInt128> FoldWatermarks { get; } = [];

        public OfflineCashHardwareQualificationV1 Qualification() =>
            context.Qualification(Enum.GetValues<OfflineCashHardwareCapabilityV1>());

        public OfflineCashHardwareRecoveryV1 Recover() => new(
            OfflineCashV1.EncodeAggregateState(context.Aggregate(sequence)), revision, 33, 0);

        public byte[] BootstrapState() => throw new InvalidOperationException();
        public UInt128 PendingCreditWatermark() => watermark;
        public UInt128 JournalRevision() => revision;

        public byte[]? FoldPendingCredit(UInt128 inboxSequenceInclusive)
        {
            FoldWatermarks.Add(inboxSequenceInclusive);
            if (!folds.Dequeue()) return null;
            revision++;
            sequence++;
            return OfflineCashV1.EncodeAggregateState(context.Aggregate(sequence));
        }

        public byte[] CreatePaymentRequest(byte[] recipientAccount, UInt128 amount, ulong validityWindowMilliseconds) =>
            throw new NotSupportedException();
        public byte[] CreateAcceptanceIntentAuthorization(byte[] canonicalRequest, UInt128 exactAmount) =>
            throw new NotSupportedException();
        public byte[] IssueAcceptanceTicket(byte[] canonicalRequest, byte[] canonicalAuthorization) =>
            throw new NotSupportedException();
        public OfflineCashHardwarePaymentStageV1 StagePayment(byte[] canonicalRequest, byte[] canonicalPayment) =>
            throw new NotSupportedException();
        public OfflineCashHardwareMintStageV1 StageMintCredit(byte[] canonicalAuthorization, byte[] canonicalMintCredit) =>
            throw new NotSupportedException();
        public OfflineCashHardwareTerminalResultV1 CommitPayment(
            byte[] canonicalRequest, byte[] canonicalAuthorization, byte[] canonicalTicket) =>
            throw new NotSupportedException();
        public byte[]? RecoverPayment(byte[] creditId) => throw new NotSupportedException();
        public void RecordAcknowledgement(byte[] creditId, byte[] canonicalAcknowledgement) =>
            throw new NotSupportedException();
        public OfflineCashHardwareTerminalResultV1 CommitRedemption(UInt128 amount, byte[] beneficiaryAccount) =>
            throw new NotSupportedException();
        public byte[]? RecoverRedemption(byte[] redemptionId) => throw new NotSupportedException();
        public byte[] RotateHardwareEpoch() => throw new NotSupportedException();
    }
}
