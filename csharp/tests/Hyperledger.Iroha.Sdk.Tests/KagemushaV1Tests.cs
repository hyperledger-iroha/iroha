using System.Security.Cryptography;
using System.Text.Json;
using Hyperledger.Iroha.Kagemusha;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class KagemushaV1Tests
{
    [Fact]
    public void ConsumesCanonicalThreeMessageSharedFixture()
    {
        using var fixture = JsonDocument.Parse(File.ReadAllBytes(
            Path.Combine(AppContext.BaseDirectory, "Fixtures", "kagemusha_v1.json")));
        var root = fixture.RootElement;
        Assert.Equal(
            new[]
            {
                "acknowledgement", "canonical_source", "fixture_version", "payment",
                "payment_request", "protocol", "terminal_trio", "text_prefix",
            },
            root.EnumerateObject().Select(static property => property.Name)
                .Order(StringComparer.Ordinal));
        Assert.Equal(1, root.GetProperty("fixture_version").GetInt32());
        Assert.Equal("KAGEMUSHA V1", root.GetProperty("protocol").GetString());
        Assert.Equal(KagemushaV1.TextPrefix, root.GetProperty("text_prefix").GetString());

        var requestBytes = FixtureBytes(root, "payment_request");
        var paymentBytes = FixtureBytes(root, "payment");
        var acknowledgementBytes = FixtureBytes(root, "acknowledgement");
        var request = KagemushaV1.DecodePaymentRequest(requestBytes);
        var payment = KagemushaV1.DecodePayment(paymentBytes, request);
        var acknowledgement = KagemushaV1.DecodeAcknowledgement(
            acknowledgementBytes, request, payment);

        Assert.Equal(requestBytes, KagemushaV1.EncodePaymentRequest(request));
        Assert.Equal(paymentBytes, KagemushaV1.EncodePayment(payment, request));
        Assert.Equal(
            acknowledgementBytes,
            KagemushaV1.EncodeAcknowledgement(acknowledgement, request, payment));
        AssertFixtureText(root, "payment_request", KagemushaV1.PayloadKind.PaymentRequest, requestBytes);
        AssertFixtureText(root, "payment", KagemushaV1.PayloadKind.Payment, paymentBytes);
        AssertFixtureText(
            root, "acknowledgement", KagemushaV1.PayloadKind.Acknowledgement,
            acknowledgementBytes);

        var terminal = root.GetProperty("terminal_trio");
        Assert.Equal(terminal.GetProperty("raw_bytes").GetInt32(),
            KagemushaV1.ValidateSession(request, payment, acknowledgement));
        Assert.Equal(
            terminal.GetProperty("text_bytes").GetInt32(),
            new[] { "payment_request", "payment", "acknowledgement" }
                .Sum(name => root.GetProperty(name).GetProperty("kgm1").GetString()!.Length));
        Assert.True(terminal.GetProperty("within_raw_hard_cap").GetBoolean());
        Assert.True(terminal.GetProperty("within_text_hard_cap").GetBoolean());
    }

    [Fact]
    public void ThreeMessageExchangeRoundTripsAndFitsCompactEnvelope()
    {
        var context = TestContext.Create();
        var request = context.Request();
        var payment = context.Payment(request);
        var acknowledgement = context.Acknowledgement(request, payment);

        var requestBytes = KagemushaV1.EncodePaymentRequest(request);
        var paymentBytes = KagemushaV1.EncodePayment(payment, request);
        var acknowledgementBytes = KagemushaV1.EncodeAcknowledgement(
            acknowledgement, request, payment);

        Assert.Equal(
            requestBytes,
            KagemushaV1.EncodePaymentRequest(KagemushaV1.DecodePaymentRequest(requestBytes)));
        Assert.Equal(
            paymentBytes,
            KagemushaV1.EncodePayment(KagemushaV1.DecodePayment(paymentBytes, request), request));
        Assert.Equal(
            acknowledgementBytes,
            KagemushaV1.EncodeAcknowledgement(
                KagemushaV1.DecodeAcknowledgement(acknowledgementBytes, request, payment),
                request,
                payment));
        Assert.Equal(
            requestBytes.Length + paymentBytes.Length + acknowledgementBytes.Length,
            KagemushaV1.ValidateSession(request, payment, acknowledgement));
        Assert.StartsWith(
            KagemushaV1.TextPrefix,
            KagemushaV1.EncodeText(KagemushaV1.PayloadKind.Payment, paymentBytes));
    }

    [Fact]
    public void PeerCreditIdentityAndAadBindBothStateHeadsAndReceiverContext()
    {
        var context = TestContext.Create();
        var request = context.Request();
        var payment = context.Payment(request);
        var statement = payment.Statement;
        var peerContext = KagemushaV1.PeerCreditContext(statement, request);
        var aad = KagemushaV1.PeerEncryptedCreditAad(statement, request);

        Assert.Equal(statement.SenderBeforeCommitment, peerContext.SenderBeforeCommitment);
        Assert.Equal(statement.SenderAfterCommitment, peerContext.SenderAfterCommitment);
        Assert.Equal(request.RecipientLaneId, peerContext.RecipientLaneId);
        Assert.Equal(request.RecipientEncryptionKey, peerContext.RecipientEncryptionKey);
        Assert.Equal(statement.CommittedAtMilliseconds, peerContext.CommittedAtMilliseconds);
        Assert.Equal(statement.Lifecycle.CreditId, aad.CreditId);
        Assert.Equal(statement.CiphertextCommitment, aad.IssuanceOrTransitionCommitment);

        var substituted = statement with
        {
            SenderAfterCommitment = context.Pasta(0xe1, 0xe2),
        };
        Assert.NotEqual(KagemushaV1.CreditId(statement), KagemushaV1.CreditId(substituted));
        Assert.Throws<ArgumentException>(() => KagemushaV1.EncodePayment(
            payment with { Statement = substituted }, request));
    }

    [Fact]
    public void RequestScopedEncryptionKeyAndLaneAreMandatoryBindings()
    {
        var context = TestContext.Create();
        var request = context.Request();
        var payment = context.Payment(request);

        Assert.Throws<ArgumentException>(() => KagemushaV1.EncodePayment(
            payment with
            {
                Statement = payment.Statement with { RecipientLaneId = Repeat(0xe3) },
            },
            request));
        Assert.Throws<ArgumentException>(() => KagemushaV1.EncodePayment(
            payment with
            {
                Statement = payment.Statement with
                {
                    RecipientEncryptionKey = new KagemushaX25519PublicKeyV1(Repeat(0xe4)),
                },
            },
            request));
        Assert.Throws<ArgumentException>(() => KagemushaV1.EncodePayment(
            payment with
            {
                Statement = payment.Statement with
                {
                    CommittedAtMilliseconds = request.ExpiresAtMilliseconds,
                },
            },
            request));
        Assert.Throws<ArgumentException>(() => KagemushaV1.EncodePayment(
            payment with
            {
                Statement = payment.Statement with
                {
                    SenderAfterCommitment = new KagemushaPastaStateCommitmentV1(
                        new byte[32], new byte[32]),
                },
            },
            request));
    }

    [Fact]
    public void PeerWireExposesOnlyTheThreeV1PayloadKinds()
    {
        Assert.Equal(
            new byte[] { 1, 2, 3 },
            Enum.GetValues<IrohaPeerPayloadKindV1>().Select(static value => (byte)value));

        var context = TestContext.Create();
        var request = context.Request();
        var payment = context.Payment(request);
        var acknowledgement = context.Acknowledgement(request, payment);
        var payloads = new[]
        {
            (IrohaPeerPayloadKindV1.ReceiveRequest, KagemushaV1.EncodePaymentRequest(request)),
            (IrohaPeerPayloadKindV1.Payment, KagemushaV1.EncodePayment(payment, request)),
            (IrohaPeerPayloadKindV1.Acknowledgement,
                KagemushaV1.EncodeAcknowledgement(acknowledgement, request, payment)),
        };

        foreach (var (kind, canonical) in payloads)
        {
            var wire = IrohaPeerKagemushaAdapterV1.Wrap(kind, canonical);
            var decoded = IrohaPeerWireMessageV1.Decode(
                wire.Encode(), expectedKind: kind);
            Assert.Equal(canonical, IrohaPeerKagemushaAdapterV1.Decode(decoded));
        }
    }

    [Fact]
    public void RemovedPreCommitProtocolTypesAndCodecsAreAbsent()
    {
        var assembly = typeof(KagemushaV1).Assembly;
        foreach (var typeName in new[]
                 {
                     "KagemushaAcceptanceIntentV1",
                     "KagemushaAcceptanceIntentAuthorizationV1",
                     "KagemushaAcceptanceTicketV1",
                     "KagemushaNoCommitClosureV1",
                     "KagemushaCommitCertificateV1",
                     "KagemushaCommitWrapperProofV1",
                 })
        {
            Assert.Null(assembly.GetType($"Hyperledger.Iroha.Kagemusha.{typeName}"));
        }

        var publicMethodNames = typeof(KagemushaV1)
            .GetMethods(System.Reflection.BindingFlags.Public | System.Reflection.BindingFlags.Static)
            .Select(static method => method.Name)
            .ToArray();
        Assert.DoesNotContain(publicMethodNames, static name => name.Contains("Acceptance", StringComparison.Ordinal));
        Assert.DoesNotContain(publicMethodNames, static name => name.Contains("NoCommit", StringComparison.Ordinal));
        Assert.DoesNotContain(publicMethodNames, static name => name.Contains("CommitCertificate", StringComparison.Ordinal));
        Assert.DoesNotContain(publicMethodNames, static name => name.Contains("CommitWrapper", StringComparison.Ordinal));

        AssertPropertyNames<KagemushaLifecycleBindingV1>(
            "Version", "NetworkId", "ProtocolVersion", "SuiteId", "VkDigest", "ReleaseId",
            "Asset", "AssetIncarnation", "Scale", "LiabilityPoolId", "HardwareProfileId",
            "PolicyEpoch", "OperationKind", "RequestId", "CreditId", "CiphertextDigest");
        AssertPropertyNames<KagemushaPaymentRequestV1>(
            "Version", "ReleaseId", "NetworkId", "Asset", "AssetIncarnation", "Scale",
            "LiabilityPoolId", "Recipient", "RecipientLaneId", "RecipientEncryptionKey", "Amount",
            "HardwareCredential", "RequestId", "IssuedAtMilliseconds", "ExpiresAtMilliseconds",
            "Signature");
        AssertPropertyNames<KagemushaTransferStatementV1>(
            "Version", "Lifecycle", "Amount", "TransitionNullifier", "SenderBeforeCommitment",
            "SenderAfterCommitment", "RequestDigest", "RecipientLaneId", "RecipientEncryptionKey",
            "CiphertextCommitment", "CommittedAtMilliseconds", "HardwareTransitionCommitment");
        AssertPropertyNames<KagemushaPaymentV1>("Version", "Statement", "Proof", "EncryptedCredit");
        AssertPropertyNames<KagemushaPeerCreditContextV1>(
            "Version", "RequestDigest", "SenderBeforeCommitment", "SenderAfterCommitment",
            "LifecycleContextDigest", "RecipientLaneId", "RecipientEncryptionKey",
            "CommittedAtMilliseconds", "HardwareTransitionCommitment");
        AssertPropertyNames<KagemushaRedemptionStatementV1>(
            "Version", "Lifecycle", "Amount", "Beneficiary", "TerminalNullifier",
            "SenderBeforeCommitment", "SenderAfterCommitment", "RedemptionCommitment",
            "RedemptionId", "CommittedAtMilliseconds", "HardwareTransitionCommitment");
        AssertPropertyNames<KagemushaRedemptionVoucherV1>("Version", "Statement", "Proof");
    }

    private static void AssertPropertyNames<T>(params string[] expected) => Assert.Equal(
        expected,
        typeof(T).GetProperties(System.Reflection.BindingFlags.Public
            | System.Reflection.BindingFlags.Instance).Select(static property => property.Name));

    private static byte[] FixtureBytes(JsonElement root, string name)
    {
        var entry = root.GetProperty(name);
        var bytes = Convert.FromHexString(entry.GetProperty("norito_hex").GetString()!);
        Assert.Equal(entry.GetProperty("raw_bytes").GetInt32(), bytes.Length);
        Assert.Equal(
            entry.GetProperty("sha256").GetString(),
            Convert.ToHexString(SHA256.HashData(bytes)).ToLowerInvariant());
        return bytes;
    }

    private static void AssertFixtureText(
        JsonElement root,
        string name,
        KagemushaV1.PayloadKind kind,
        byte[] bytes)
    {
        var text = root.GetProperty(name).GetProperty("kgm1").GetString()!;
        Assert.Equal(text, KagemushaV1.EncodeText(kind, bytes));
        Assert.Equal(bytes, KagemushaV1.DecodeText(kind, text));
    }

    private static byte[] Repeat(byte value) => Enumerable.Repeat(value, 32).ToArray();

    private sealed class TestContext
    {
        private TestContext(
            NetworkId networkId,
            KagemushaAssetDefinitionIdV1 asset,
            KagemushaAssetIncarnationV1 incarnation,
            KagemushaAccountIdV1 account,
            KagemushaHardwareCredentialV1 credential,
            byte[] liabilityPoolId,
            byte[] releaseId)
        {
            NetworkId = networkId;
            Asset = asset;
            Incarnation = incarnation;
            Account = account;
            Credential = credential;
            LiabilityPoolId = liabilityPoolId;
            ReleaseId = releaseId;
        }

        private NetworkId NetworkId { get; }
        private KagemushaAssetDefinitionIdV1 Asset { get; }
        private KagemushaAssetIncarnationV1 Incarnation { get; }
        private KagemushaAccountIdV1 Account { get; }
        private KagemushaHardwareCredentialV1 Credential { get; }
        private byte[] LiabilityPoolId { get; }
        private byte[] ReleaseId { get; }

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
            var account = new KagemushaAccountIdV1(
                "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV");
            var sec1 = Enumerable.Repeat((byte)1, 65).ToArray();
            sec1[0] = 4;
            var publicKey = new KagemushaDevicePublicKeyV1(sec1);
            var signatureBytes = new byte[64];
            signatureBytes[31] = 1;
            signatureBytes[63] = 1;
            var signature = new KagemushaDeviceSignatureV1(signatureBytes);
            var profileId = Repeat(0x42);
            var laneId = Repeat(0x53);
            var credential = new KagemushaHardwareCredentialV1(
                1, Repeat(0x51), networkId, profileId, Repeat(0x52), Repeat(0x45), 9,
                laneId, Repeat(0x54), 1, publicKey, KagemushaV1.DeviceKeyReference(publicKey),
                100, 10_000, signature);
            return new TestContext(
                networkId,
                asset,
                incarnation,
                account,
                credential,
                KagemushaV1.LiabilityPoolId(networkId, asset, incarnation),
                Repeat(0x41));
        }

        internal KagemushaPaymentRequestV1 Request() => Request((UInt128)25);

        internal KagemushaPaymentRequestV1 Request(UInt128 amount) => new(
            1, ReleaseId, NetworkId, Asset, Incarnation, 4, LiabilityPoolId, Account,
            Credential.LaneCommitment, new KagemushaX25519PublicKeyV1(Repeat(0xb6)),
            amount, Credential, Repeat(0x61), 1_000, 2_000, Credential.GovernanceSignature);

        internal KagemushaPastaStateCommitmentV1 Pasta(byte eq, byte ep) =>
            new(Repeat(eq), Repeat(ep));

        internal KagemushaPaymentV1 Payment(KagemushaPaymentRequestV1 request)
        {
            var encryptedCredit = KagemushaV1.EncodeEncryptedCreditEnvelope(
                new KagemushaEncryptedCreditEnvelopeV1(
                    1,
                    new KagemushaX25519PublicKeyV1(Repeat(0xcc)),
                    Enumerable.Repeat((byte)0xcd, 24).ToArray(),
                    Enumerable.Repeat(
                        (byte)0xce,
                        KagemushaV1.EncryptedCreditCiphertextAndTagBytes).ToArray()));
            var lifecycle = new KagemushaLifecycleBindingV1(
                1, request.NetworkId, 1, request.HardwareCredential.SuiteId, Repeat(0xba),
                request.ReleaseId, request.Asset, request.AssetIncarnation, request.Scale,
                request.LiabilityPoolId, request.HardwareCredential.HardwareProfileId,
                request.HardwareCredential.PolicyEpoch, KagemushaOperationKindV1.SendSplit,
                request.RequestId, Repeat(0xb7), KagemushaV1.CiphertextDigest(encryptedCredit));
            var statement = new KagemushaTransferStatementV1(
                1, lifecycle, request.Amount, Repeat(0xb8), Pasta(0xd0, 0xd1),
                Pasta(0xd2, 0xd3), KagemushaV1.PaymentRequestDigest(request),
                request.RecipientLaneId, request.RecipientEncryptionKey, Repeat(0xbb),
                1_500, Repeat(0xbc));
            statement = statement with
            {
                Lifecycle = lifecycle with { CreditId = KagemushaV1.CreditId(statement) },
            };
            return new KagemushaPaymentV1(
                1, statement, Proof(KagemushaV1.TransferStatementDigestUnchecked(statement)),
                encryptedCredit);
        }

        internal KagemushaAcknowledgementV1 Acknowledgement(
            KagemushaPaymentRequestV1 request,
            KagemushaPaymentV1 payment) => new(
                1,
                KagemushaV1.PaymentRequestDigest(request),
                KagemushaV1.PaymentDigest(payment, request),
                new KagemushaInboxReceiptV1(
                    1, payment.Statement.Lifecycle.CreditId, Repeat(0xcb)),
                request.HardwareCredential.GovernanceSignature);

        private static KagemushaPairedProofV1 Proof(byte[] semanticDigest) => new(
            1, Repeat(0x81), Repeat(0x82), semanticDigest, Repeat(0x84), Repeat(0x85),
            Repeat(0x86), Repeat(0x87), new byte[] { 1 }, new byte[] { 2 },
            Enumerable.Repeat((byte)0x88, KagemushaV1.HistoryAccumulatorBytes).ToArray(),
            Enumerable.Repeat((byte)0x89, KagemushaV1.HistoryAccumulatorBytes).ToArray());
    }
}
