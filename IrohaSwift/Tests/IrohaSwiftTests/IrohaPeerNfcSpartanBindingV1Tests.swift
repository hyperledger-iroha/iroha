import XCTest
@testable import IrohaSwift

/// Spartan Tap over the virtual NFC card. The originals are engineering fixtures with the schema-3 shapes;
/// every verdict comes from a substituted Native, exactly where the installed wallet decides it on a phone.
final class IrohaPeerNfcSpartanBindingV1Tests: XCTestCase {
    private let requestID = Data(repeating: 0x61, count: 32)
    private let sendOperation = Data(repeating: 0x62, count: 32)
    private let receiveOperation = Data(repeating: 0x63, count: 32)
    private let sessionID = Data(repeating: 0x64, count: 16)

    func testVerifiedTwoTapRoundTripDeliversTheExactRetainedPaymentAndVerifiedStatus() async throws {
        let exchange = try Exchange(test: self)
        let card = try exchange.card()

        // Tap 1 reads the hosted Request; a policy without a binding may read but moves no value.
        let read = try await IrohaPeerNfcReaderExchangeV1.readRequest(
            profilePolicy: .init(profile: .kagemushaSpartanPaymentV3),
            transceive: { try await card.transceive($0) })
        XCTAssertEqual(read.request, exchange.request)
        XCTAssertEqual(read.destinationAccountOriginal, SpartanNfcFixtureV1.account)
        XCTAssertEqual(read.requestEnvelope.count, 1_250)

        // The protected Send happened away from the card; tap 2 delivers only its retained Payment.
        let payer = try KagemushaSpartanPeerNfcBindingV1.payer(native: exchange.payerNative,
            sendOperationId: sendOperation, request: read.request)
        XCTAssertEqual(payer.payment, exchange.payment)
        let checkpoint = try payer.senderCheckpoint(sessionID: read.info.identity.sessionID)
        let persisted = Persisted()
        let result = try await IrohaPeerNfcReaderExchangeV1.run(
            restoredCheckpoint: checkpoint.encoded, profilePolicy: payer.profilePolicy,
            transceive: { try await card.transceive($0) },
            loadOrCreateDurableCheckpoint: { _, _ in XCTFail("tap 2 never creates value"); throw CancellationError() },
            updateDurableCheckpoint: { encoded in
                // The status was verified against the retained Payment before it could be persisted.
                XCTAssertEqual(exchange.payerNative.count("verifyCreditStatus"), 1)
                persisted.append(encoded)
            })

        XCTAssertEqual(result.acknowledgement, exchange.status)
        XCTAssertEqual(result.confirmationState, .confirmed)
        XCTAssertEqual(payer.verifiedOutcome(of: exchange.status), .credited)
        let received = await card.receivedPayment
        let phase = await card.phase
        let commits = await card.commits
        XCTAssertEqual(received, exchange.payment.encoded)
        XCTAssertEqual(phase, .complete)
        XCTAssertEqual(commits, 1)
        // APDU chunk framing is unchanged: a 10,711-byte Payment crosses in three 4,096-byte writes.
        let seen = await card.commands
        XCTAssertEqual(seen.filter { if case .write = $0 { return true }; return false }.count, 3)
        XCTAssertEqual(persisted.values.count, 1)
        let saved = try IrohaPeerNfcSenderCheckpointV1.decode(persisted.values[0], profilePolicy: payer.profilePolicy)
        XCTAssertEqual(saved.durableAcknowledgement, exchange.status)
        XCTAssertEqual(saved.payment, exchange.payment)
        XCTAssertEqual(exchange.payerNative.count("verifyCreditStatus"), 1)
    }

    func testCrossProfileAndUnboundPoliciesFailClosedBeforeAnyValue() async throws {
        let exchange = try Exchange(test: self)
        let unbound = IrohaPeerNfcProfilePolicyV1(profile: .kagemushaSpartanPaymentV3)
        XCTAssertFalse(unbound.bindsExchanges)
        XCTAssertTrue(IrohaPeerNfcProfilePolicyV1(profile: .kagemushaWalletV1).bindsExchanges)

        // Shape-valid Spartan originals never bind without the wallet's verified binding.
        XCTAssertThrowsError(try IrohaPeerNfcSenderCheckpointV1(sessionID: sessionID,
            receiveRequest: exchange.request.encoded, payment: exchange.payment.encoded)) {
            XCTAssertEqual($0 as? IrohaPeerNfcErrorV1, .invalidProfile)
        }
        XCTAssertThrowsError(try IrohaPeerNfcReceiverSessionV1(sessionID: sessionID,
            receiveRequest: exchange.request.encoded)) {
            XCTAssertEqual($0 as? IrohaPeerNfcErrorV1, .invalidProfile)
        }
        let commands = Persisted()
        do {
            _ = try await IrohaPeerNfcReaderExchangeV1.run(profilePolicy: unbound,
                transceive: { _ in
                    commands.append(Data([1]))
                    return IrohaPeerNfcAPDUResponseV1(statusWord: .success)
                },
                loadOrCreateDurableCheckpoint: { _, _ in XCTFail("no value without a binding"); throw CancellationError() },
                updateDurableCheckpoint: { _ in XCTFail("no acknowledgement without a binding") })
            XCTFail("an unbound Spartan run must not start")
        } catch {
            XCTAssertEqual(error as? IrohaPeerNfcErrorV1, .invalidProfile)
        }
        XCTAssertTrue(commands.values.isEmpty)

        // The wallet V1 policy does not admit Spartan originals, and a Spartan binding does not admit wallet V1.
        XCTAssertThrowsError(try IrohaPeerNfcReceiverSessionV1(sessionID: sessionID,
            receiveRequest: exchange.request.encoded, profilePolicy: .init(profile: .kagemushaWalletV1))) {
            XCTAssertEqual($0 as? IrohaPeerNfcErrorV1, .invalidProfile)
        }
        let receiver = try exchange.receiverBinding()
        let walletRequest = try irohaPeerWalletExchangeMessageV1(kind: .request, payload: Data([2]))
        let walletPayment = try irohaPeerWalletExchangeMessageV1(kind: .payment, payload: Data([2]))
        XCTAssertThrowsError(try IrohaPeerNfcReceiverSessionV1(sessionID: sessionID,
            receiveRequest: walletRequest.encoded, profilePolicy: receiver.profilePolicy)) {
            XCTAssertEqual($0 as? IrohaPeerNfcErrorV1, .invalidProfile)
        }
        let payer = try exchange.payerBinding()
        XCTAssertThrowsError(try IrohaPeerNfcSenderCheckpointV1(sessionID: sessionID,
            receiveRequest: walletRequest.encoded, payment: walletPayment.encoded, profilePolicy: payer.profilePolicy)) {
            XCTAssertEqual($0 as? IrohaPeerNfcErrorV1, .invalidProfile)
        }
        // The earlier wallet V1 route keeps its structural binding unchanged.
        XCTAssertNoThrow(try IrohaPeerNfcSenderCheckpointV1(sessionID: sessionID,
            receiveRequest: walletRequest.encoded, payment: walletPayment.encoded))
        // A value validated under one binding is never accepted as validated under another.
        XCTAssertNotEqual(payer.profilePolicy, try exchange.payerBinding().profilePolicy)
        XCTAssertEqual(payer.profilePolicy, payer.profilePolicy)
    }

    func testRequestSwapIsRefusedByBothBindingsBeforeCommitOrCheckpoint() async throws {
        let exchange = try Exchange(test: self)
        let other = try SpartanNfcFixtureV1.request(nonce: 0x44)
        let otherPayment = try SpartanNfcFixtureV1.payment(mark: 0x45)
        exchange.payerNative.bind(otherPayment, to: other, payerWallet: SpartanNfcFixtureV1.payerWallet)
        exchange.receiverNative.bind(otherPayment, to: other, payerWallet: SpartanNfcFixtureV1.payerWallet)

        // Payer: the released Send binds one exact Request; another Request with any Payment is refused.
        let payer = try exchange.payerBinding()
        XCTAssertThrowsError(try IrohaPeerNfcSenderCheckpointV1(sessionID: sessionID,
            receiveRequest: other.encoded, payment: exchange.payment.encoded, profilePolicy: payer.profilePolicy)) {
            XCTAssertEqual($0 as? IrohaPeerNfcErrorV1, .continuityMismatch)
        }
        XCTAssertThrowsError(try IrohaPeerNfcSenderCheckpointV1(sessionID: sessionID,
            receiveRequest: exchange.request.encoded, payment: otherPayment.encoded, profilePolicy: payer.profilePolicy)) {
            XCTAssertEqual($0 as? IrohaPeerNfcErrorV1, .continuityMismatch)
        }
        // Native refuses to bind the operation's retained Payment to a swapped Request.
        XCTAssertThrowsError(try KagemushaSpartanPeerNfcBindingV1.payer(native: exchange.payerNative,
            sendOperationId: sendOperation, request: other))
        // A completed Request whose payer specialization names another wallet is refused.
        exchange.payerNative.bind(exchange.payment, to: exchange.request, payerWallet: Data(repeating: 0x77, count: 32))
        XCTAssertThrowsError(try exchange.payerBinding())
        exchange.payerNative.bind(exchange.payment, to: exchange.request, payerWallet: SpartanNfcFixtureV1.payerWallet)

        // Receiver: only Native's own Request for the identity is hosted.
        XCTAssertThrowsError(try KagemushaSpartanPeerNfcBindingV1.receiver(native: exchange.receiverNative,
            requestId: requestID, request: other))
        let receiver = try exchange.receiverBinding()
        XCTAssertThrowsError(try IrohaPeerNfcReceiverSessionV1(sessionID: sessionID,
            receiveRequest: other.encoded, profilePolicy: receiver.profilePolicy)) {
            XCTAssertEqual($0 as? IrohaPeerNfcErrorV1, .continuityMismatch)
        }
        // A Payment bound to another Request fails at COMMIT, before the durable handler, on every replay.
        var session = try IrohaPeerNfcReceiverSessionV1(sessionID: sessionID,
            receiveRequest: exchange.request.encoded, profilePolicy: receiver.profilePolicy)
        guard case let .requiresDurableAdmission(context) = try session.preparePaymentAdmission(
            .beginPayment(sessionID: sessionID, requestCanonicalHash: exchange.request.canonicalHash,
                          paymentHeader: otherPayment.header.bytes)) else { return XCTFail("expected admission") }
        try session.installPaymentAdmission(IrohaPeerNfcDurablePaymentAdmissionV1(context: context))
        var offset = 0
        while offset < otherPayment.encoded.count {
            let end = min(offset + 4_096, otherPayment.encoded.count)
            _ = try session.handle(.write(sessionID: sessionID, paymentWireHash: otherPayment.wireHash,
                offset: UInt32(offset), bytes: otherPayment.encoded.subdata(in: offset..<end)))
            offset = end
        }
        var handlerCalls = 0
        for _ in 0..<2 {
            let response = session.process(apdu: try IrohaPeerNfcAPDUCodecV1.encode(.commit(sessionID: sessionID,
                requestCanonicalHash: exchange.request.canonicalHash, paymentWireHash: otherPayment.wireHash)),
                durableCommit: { _ in handlerCalls += 1; throw IrohaPeerNfcErrorV1.stateMismatch })
            XCTAssertEqual(response.statusWord, .securityStatusNotSatisfied)
        }
        XCTAssertEqual(handlerCalls, 0)
        XCTAssertEqual(session.phase, .paymentReceiving)
        // An acknowledgement this card's Native did not issue is never installed.
        XCTAssertThrowsError(try receiver.durableAcknowledgement(sessionID: sessionID, payment: exchange.payment,
            acknowledgement: exchange.status))
    }

    func testDurableReplayResumesTheExactPaymentAndRestartedCardReplaysItsAcknowledgement() async throws {
        let exchange = try Exchange(test: self)
        let card = try exchange.card()
        let payer = try exchange.payerBinding()
        let checkpoint = try payer.senderCheckpoint(sessionID: sessionID).encoded
        let persisted = Persisted()
        let created = Persisted()
        func run(_ restored: Data, on card: SpartanNfcCardLoopbackV1) async throws -> IrohaPeerNfcReaderExchangeResultV1 {
            try await IrohaPeerNfcReaderExchangeV1.run(restoredCheckpoint: restored,
                profilePolicy: payer.profilePolicy, transceive: { try await card.transceive($0) },
                loadOrCreateDurableCheckpoint: { _, _ in created.append(Data([1])); throw CancellationError() },
                updateDurableCheckpoint: { persisted.append($0) })
        }

        // RF is lost once the receiver has durably committed, before the acknowledgement is read.
        await card.loseResponse { if case .commit = $0 { return true }; return false }
        do { _ = try await run(checkpoint, on: card); XCTFail("expected the lost response") }
        catch { XCTAssertTrue(error is SpartanNfcLoopbackLostV1) }
        XCTAssertTrue(persisted.values.isEmpty)
        XCTAssertEqual(exchange.payerNative.count("verifyCreditStatus"), 0)

        // The receiver's phone restarts: its durable record restores acknowledgementReady with no new Receive.
        let restoredReceiver = try exchange.receiverBinding()
        let status = try restoredReceiver.acknowledgement(receiveOperationId: receiveOperation, payment: exchange.payment)
        XCTAssertEqual(status.message, exchange.status)
        XCTAssertEqual(status.outcome, .credited)
        let record = try restoredReceiver.durableAcknowledgement(sessionID: sessionID, payment: exchange.payment,
                                                                 acknowledgement: status.message)
        let restartedCard = try exchange.card(binding: restoredReceiver, durableAcknowledgement: record)
        let restartedPhase = await restartedCard.phase
        XCTAssertEqual(restartedPhase, .acknowledgementReady)

        // The same retained checkpoint resumes: no new value, no new Payment bytes, no second COMMIT.
        let result = try await run(checkpoint, on: restartedCard)
        XCTAssertEqual(result.acknowledgement, exchange.status)
        XCTAssertTrue(created.values.isEmpty)
        let restartedCommands = await restartedCard.commands
        XCTAssertFalse(restartedCommands.contains { command in
            switch command { case .beginPayment, .write, .commit: return true; default: return false }
        })
        let restartedCommits = await restartedCard.commits
        let firstCommits = await card.commits
        XCTAssertEqual(firstCommits + restartedCommits, 1)
        XCTAssertEqual(persisted.values.count, 1)

        // A lost CONFIRM response is complete from the acknowledgement-bearing checkpoint, without re-verification.
        let saved = persisted.values[0]
        XCTAssertNotNil(try IrohaPeerNfcSenderCheckpointV1.decode(saved, profilePolicy: payer.profilePolicy)
            .durableAcknowledgement)
        let again = try await run(saved, on: restartedCard)
        XCTAssertEqual(again.acknowledgement, exchange.status)
        XCTAssertEqual(exchange.payerNative.count("verifyCreditStatus"), 1)
        XCTAssertEqual(persisted.values.count, 1)
    }

    func testAcknowledgementLossesNeverRedebitPersistTwiceOrSkipVerification() async throws {
        let exchange = try Exchange(test: self)
        let card = try exchange.card()
        let payer = try exchange.payerBinding()
        let checkpoint = try payer.senderCheckpoint(sessionID: sessionID).encoded
        let persisted = Persisted()
        func run(_ restored: Data) async throws -> IrohaPeerNfcReaderExchangeResultV1 {
            try await IrohaPeerNfcReaderExchangeV1.run(restoredCheckpoint: restored,
                profilePolicy: payer.profilePolicy, transceive: { try await card.transceive($0) },
                loadOrCreateDurableCheckpoint: { _, _ in XCTFail("no value after the Send"); throw CancellationError() },
                updateDurableCheckpoint: { persisted.append($0) })
        }

        // RF is lost partway through the 9,400-byte status: partial bytes are never verified or persisted.
        await card.loseResponse { command in
            if case .readAcknowledgement(_, _, let offset, _) = command { return offset > 0 }
            return false
        }
        do { _ = try await run(checkpoint); XCTFail("expected the lost status chunk") }
        catch { XCTAssertTrue(error is SpartanNfcLoopbackLostV1) }
        XCTAssertTrue(persisted.values.isEmpty)
        XCTAssertEqual(exchange.payerNative.count("verifyCreditStatus"), 0)

        // The same checkpoint reads the status again from its start, has Native verify it once and persists it; then
        // CONFIRM's response is lost. The durable verified status makes the exchange complete.
        await card.loseResponse(ambiguously: true) { command in
            if case .confirmAcknowledgement = command { return true }
            return false
        }
        let unknown = try await run(checkpoint)
        XCTAssertEqual(unknown.confirmationState, .responseUnknown)
        XCTAssertEqual(unknown.acknowledgement, exchange.status)
        XCTAssertEqual(persisted.values.count, 1)
        XCTAssertEqual(exchange.payerNative.count("verifyCreditStatus"), 1)

        // Resuming the status-bearing checkpoint observes COMPLETE: nothing is written, verified or persisted again.
        let complete = try await run(persisted.values[0])
        XCTAssertEqual(complete.acknowledgement, exchange.status)
        XCTAssertEqual(persisted.values.count, 1)
        XCTAssertEqual(exchange.payerNative.count("verifyCreditStatus"), 1)
        let commands = await card.commands
        XCTAssertEqual(commands.filter { if case .beginPayment = $0 { return true }; return false }.count, 1)
        XCTAssertEqual(commands.filter { if case .commit = $0 { return true }; return false }.count, 1)
        let received = await card.receivedPayment
        let commits = await card.commits
        let phase = await card.phase
        XCTAssertEqual(received, exchange.payment.encoded)
        XCTAssertEqual(commits, 1)
        XCTAssertEqual(phase, .complete)
    }

    func testReceiverReceiptRequiresTheExactPaymentTheCurrentReceiveTook() throws {
        let exchange = try Exchange(test: self)
        // Another Payment Native binds to the same valid Request: eligible, but not the one this Receive took.
        let other = try SpartanNfcFixtureV1.payment(mark: 0x46)
        exchange.receiverNative.bind(other, to: exchange.request, payerWallet: SpartanNfcFixtureV1.payerWallet)
        let receiver = try exchange.receiverBinding()
        XCTAssertThrowsError(try receiver.acknowledgement(receiveOperationId: receiveOperation, payment: other))
        XCTAssertThrowsError(try receiver.durableAcknowledgement(sessionID: sessionID, payment: other,
                                                                 acknowledgement: exchange.status))
        // Another current Receive operation gets no receipt either.
        XCTAssertThrowsError(try receiver.acknowledgement(receiveOperationId: Data(repeating: 0x65, count: 32),
                                                          payment: exchange.payment))
        XCTAssertThrowsError(try receiver.durableAcknowledgement(sessionID: sessionID, payment: exchange.payment,
                                                                 acknowledgement: exchange.status))
        // Only the exact Payment of the current Receive gets its receipt, which then restores the card.
        let status = try receiver.acknowledgement(receiveOperationId: receiveOperation, payment: exchange.payment)
        XCTAssertEqual(status.message, exchange.status)
        XCTAssertNoThrow(try receiver.durableAcknowledgement(sessionID: sessionID, payment: exchange.payment,
                                                             acknowledgement: status.message))
        XCTAssertEqual(exchange.receiverNative.count("receivedCreditStatus"), 3)
    }

    func testUnverifiedOrForeignStatusIsNeverPersistedOrConfirmed() async throws {
        let exchange = try Exchange(test: self)
        // The receiver's Native issues a status this payer's Native does not verify for its Send.
        let foreign = try SpartanNfcFixtureV1.creditStatus(mark: 0x19)
        exchange.receiverNative.receipts[receiveOperation] = foreign
        exchange.payerNative.credits = [:]
        let card = try exchange.card()
        let payer = try exchange.payerBinding()
        let persisted = Persisted()
        do {
            _ = try await IrohaPeerNfcReaderExchangeV1.run(
                restoredCheckpoint: try payer.senderCheckpoint(sessionID: sessionID).encoded,
                profilePolicy: payer.profilePolicy, transceive: { try await card.transceive($0) },
                loadOrCreateDurableCheckpoint: { _, _ in throw CancellationError() },
                updateDurableCheckpoint: { persisted.append($0) })
            XCTFail("an unverified status must not complete the exchange")
        } catch {
            XCTAssertEqual(error as? IrohaPeerNfcErrorV1, .continuityMismatch)
        }
        XCTAssertTrue(persisted.values.isEmpty)
        XCTAssertNil(payer.verifiedOutcome(of: foreign))
        let commands = await card.commands
        XCTAssertFalse(commands.contains { if case .confirmAcknowledgement = $0 { return true }; return false })
        XCTAssertThrowsError(try payer.senderCheckpoint(sessionID: sessionID, acknowledgement: foreign)) {
            XCTAssertEqual($0 as? IrohaPeerNfcErrorV1, .continuityMismatch)
        }

        // A verified burn is evidence of delivery, never a refund: it is persisted with its Native outcome.
        let burned = try SpartanNfcFixtureV1.creditStatus(burned: true, mark: 0x1b)
        exchange.payerNative.credit(burned, operation: sendOperation, request: exchange.request, outcome: .burned)
        _ = try payer.senderCheckpoint(sessionID: sessionID, acknowledgement: burned)
        XCTAssertEqual(payer.verifiedOutcome(of: burned), .burned)
    }

    func testRealSizesFitTheProfileAndPaymentsArePreflightBounded() throws {
        XCTAssertEqual(IrohaPeerNfcV1.maximumPaymentMessageBytes, 12_182)
        let current = try SpartanNfcFixtureV1.payment(totalBytes: 10_711)
        let largest = try SpartanNfcFixtureV1.payment(totalBytes: 12_182)
        let status = try SpartanNfcFixtureV1.creditStatus(totalBytes: 9_400)
        XCTAssertEqual(current.encoded.count, 10_711)
        XCTAssertEqual(largest.encoded.count, 12_182)
        XCTAssertEqual(status.encoded.count, 9_400)
        let request = try SpartanNfcFixtureV1.request()
        for message in [current, largest, status, request] {
            XCTAssertLessThanOrEqual(message.encoded.count, IrohaPeerNfcV1.maximumMessageBytes)
            XCTAssertEqual(try IrohaPeerNfcPaymentDescriptorV1Probe.decode(message), message)
        }
        XCTAssertNoThrow(try IrohaPeerNfcPaymentDescriptorV1(paymentHeader: largest.header.bytes))
        // No Spartan Payment beyond the complete carrier exists to deliver.
        XCTAssertThrowsError(try SpartanNfcFixtureV1.payment(totalBytes: 12_183))

        // The payer's binding refuses delivery of anything but the bounded retained Payment.
        let exchange = try Exchange(test: self, payment: largest)
        let payer = try exchange.payerBinding()
        XCTAssertEqual(payer.payment?.encoded.count, 12_182)
        XCTAssertEqual(exchange.payerNative.count("output"), 1)
    }

    // MARK: - Exchange

    private final class Exchange {
        let request: IrohaPeerWireMessageV1
        let payment: IrohaPeerWireMessageV1
        let status: IrohaPeerWireMessageV1
        let payerNative: SpartanNfcNativeProbeV1
        let receiverNative: SpartanNfcNativeProbeV1
        private unowned let test: IrohaPeerNfcSpartanBindingV1Tests

        init(test: IrohaPeerNfcSpartanBindingV1Tests, payment: IrohaPeerWireMessageV1? = nil) throws {
            self.test = test
            request = try SpartanNfcFixtureV1.request()
            self.payment = try payment ?? SpartanNfcFixtureV1.payment()
            status = try SpartanNfcFixtureV1.creditStatus()
            payerNative = SpartanNfcNativeProbeV1(wallet: SpartanNfcFixtureV1.payerWallet)
            receiverNative = SpartanNfcNativeProbeV1(wallet: SpartanNfcFixtureV1.receiverWallet)
            payerNative.retained[test.sendOperation] = self.payment.encoded
            payerNative.bind(self.payment, to: request, payerWallet: SpartanNfcFixtureV1.payerWallet)
            payerNative.credit(status, operation: test.sendOperation, request: request, outcome: .credited)
            receiverNative.issued[test.requestID] = request
            receiverNative.bind(self.payment, to: request, payerWallet: SpartanNfcFixtureV1.payerWallet)
            receiverNative.receipts[test.receiveOperation] = status
            receiverNative.received[test.receiveOperation] = self.payment.encoded
        }

        func payerBinding() throws -> KagemushaSpartanPeerNfcBindingV1 {
            try .payer(native: payerNative, sendOperationId: test.sendOperation, request: request)
        }

        func receiverBinding() throws -> KagemushaSpartanPeerNfcBindingV1 {
            try .receiver(native: receiverNative, requestId: test.requestID, request: request)
        }

        func card(binding: KagemushaSpartanPeerNfcBindingV1? = nil,
                  durableAcknowledgement: IrohaPeerNfcDurableAcknowledgementV1? = nil) throws -> SpartanNfcCardLoopbackV1 {
            let binding = try binding ?? receiverBinding()
            let receiver = try IrohaPeerNfcReceiverSessionV1(sessionID: test.sessionID,
                receiveRequest: request.encoded, durableAcknowledgement: durableAcknowledgement,
                profilePolicy: binding.profilePolicy)
            return SpartanNfcCardLoopbackV1(endpoint: .init(receiver: receiver), binding: binding,
                                            receiveOperation: test.receiveOperation)
        }
    }

    private final class Persisted: @unchecked Sendable {
        private let lock = NSLock()
        private var stored: [Data] = []
        func append(_ value: Data) { lock.lock(); stored.append(value); lock.unlock() }
        var values: [Data] { lock.lock(); defer { lock.unlock() }; return stored }
    }
}

/// Exact re-decode of a fixture through the IPM1 codec the NFC transport uses.
private enum IrohaPeerNfcPaymentDescriptorV1Probe {
    static func decode(_ message: IrohaPeerWireMessageV1) throws -> IrohaPeerWireMessageV1 {
        try IrohaPeerWireMessageV1.decode(message.encoded, expectedProfile: .kagemushaSpartanPaymentV3)
    }
}

/// Schema-3 engineering fixtures: exact Spartan shapes, no proof. Only a substituted Native decides them.
enum SpartanNfcFixtureV1 {
    static let scheme = Data(repeating: 0x5a, count: 32)
    static let asset = Data(repeating: 0xa5, count: 32)
    static let receiverWallet = Data(repeating: 0x31, count: 32)
    static let payerWallet = Data(repeating: 0x42, count: 32)
    static let account = Data("receiver-account".utf8)

    /// A `KWRQAC1` open Request with a 1,250-byte envelope (no fee schedule) and the receiver's account.
    static func request(nonce: UInt8 = 4, amount: UInt8 = 7) throws -> IrohaPeerWireMessageV1 {
        var body = Data(repeating: 0, count: 482)
        body[0] = 1
        body.replaceSubrange(2..<34, with: scheme)
        body.replaceSubrange(34..<66, with: asset)
        body.replaceSubrange(130..<162, with: receiverWallet)
        body[162] = 0x33
        body[242] = amount
        body[386] = nonce
        body[418] = 9
        var credential = Data(repeating: 0, count: 596)
        credential.replaceSubrange(0..<66, with: body.prefix(66))
        credential.replaceSubrange(66..<130, with: body.subdata(in: 130..<194))
        credential[130] = 4
        var certificate = Data(repeating: 0, count: 172)
        certificate.replaceSubrange(0..<34, with: body.prefix(34))
        certificate[34] = 1
        certificate[35] = 4
        let envelope = body + credential + certificate
        let payload = Data("KWRQAC1\0".utf8) + be32(envelope.count) + be32(account.count) + envelope + account
        return try IrohaPeerWireMessageV1(profile: .kagemushaSpartanPaymentV3, kind: .request,
                                          schemaVersion: 3, canonicalPayload: payload)
    }

    /// A `KWPYPC1` Payment of exactly `totalBytes` IPM1 bytes.
    static func payment(totalBytes: Int = 10_711, mark: UInt8 = 1) throws -> IrohaPeerWireMessageV1 {
        let canonical = totalBytes - IrohaPeerWireMessageV1.headerBytes
        let companion = min(2_048, canonical - 17)
        let envelopeCount = canonical - 16 - companion
        var envelope = Data(repeating: 0x11, count: envelopeCount)
        envelope[0] = mark
        let payload = Data("KWPYPC1\0".utf8) + be32(envelopeCount) + be32(companion)
            + envelope + Data(repeating: 0x22, count: companion)
        return try IrohaPeerWireMessageV1(profile: .kagemushaSpartanPaymentV3, kind: .payment,
                                          schemaVersion: 3, canonicalPayload: payload)
    }

    /// A `KGSCST3` CreditStatus of exactly `totalBytes` IPM1 bytes. Its folded body carries no readable scheme.
    static func creditStatus(totalBytes: Int = 9_400, burned: Bool = false, mark: UInt8 = 7) throws -> IrohaPeerWireMessageV1 {
        var bytes = Data("KGSCST3\0".utf8) + Data([burned ? 1 : 0]) + Data(repeating: 0, count: 9 + 160 + 6 + 8)
        bytes += Data(repeating: mark, count: totalBytes - IrohaPeerWireMessageV1.headerBytes - bytes.count)
        return try IrohaPeerWireMessageV1(profile: .kagemushaSpartanPaymentV3, kind: .credited,
                                          schemaVersion: 3, canonicalPayload: bytes)
    }

    static func completed(_ request: IrohaPeerWireMessageV1, payerWallet: Data) throws -> Data {
        var body = try IrohaPeerSpartanOpenRequestV3(canonicalPayload: request.canonicalPayload).request
        body.replaceSubrange(66..<98, with: payerWallet)
        body[98] = 0x44
        body[194] = 1
        return body
    }

    private static func be32(_ value: Int) -> Data {
        Data([UInt8(truncatingIfNeeded: value >> 24), UInt8(truncatingIfNeeded: value >> 16),
              UInt8(truncatingIfNeeded: value >> 8), UInt8(truncatingIfNeeded: value)])
    }
}

/// Substituted Native for one wallet: every verdict is a recorded table entry, refused otherwise.
final class SpartanNfcNativeProbeV1: KagemushaSpartanPeerNfcNativeV1, @unchecked Sendable {
    private let lock = NSLock()
    private let wallet: Data
    var issued: [Data: IrohaPeerWireMessageV1] = [:]
    var retained: [Data: Data] = [:]
    var bindings: [Data: (request: Data, completed: Data)] = [:]
    var credits: [Data: (operation: Data, request: Data, outcome: KagemushaSpartanCreditOutcomeV1)] = [:]
    var receipts: [Data: IrohaPeerWireMessageV1] = [:]
    /// The complete Payment bytes each released Receive took.
    var received: [Data: Data] = [:]
    private var calls: [String] = []

    init(wallet: Data) { self.wallet = wallet }

    func bind(_ payment: IrohaPeerWireMessageV1, to request: IrohaPeerWireMessageV1, payerWallet: Data) {
        bindings[payment.wireHash] = (request.wireHash, try! SpartanNfcFixtureV1.completed(request, payerWallet: payerWallet))
    }

    func credit(_ status: IrohaPeerWireMessageV1, operation: Data, request: IrohaPeerWireMessageV1,
                outcome: KagemushaSpartanCreditOutcomeV1) {
        credits[status.wireHash] = (operation, request.wireHash, outcome)
    }

    func count(_ name: String) -> Int {
        lock.lock(); defer { lock.unlock() }
        return calls.filter { $0 == name }.count
    }

    private func record(_ name: String) { lock.lock(); calls.append(name); lock.unlock() }

    func snapshot() throws -> KagemushaSpartanSnapshotV1 {
        record("snapshot")
        return try KagemushaSpartanSnapshotV1(KagemushaSpartanReplyV1(status: 101, sequenceLow: 0, sequenceHigh: 0,
            detail: 0, bytes: Data([1, 0, 1, 2]) + wallet + SpartanNfcFixtureV1.asset))
    }

    func issueOpenRequest(requestId: Data, amount: KagemushaWalletUInt128V1,
                          feeSchedule: Data, feeCertificate: Data) throws -> IrohaPeerWireMessageV1 {
        record("issueOpenRequest")
        guard let request = issued[requestId], feeSchedule.isEmpty, feeCertificate.isEmpty,
              amount == .init(low: 7, high: 0) else { throw KagemushaWalletErrorV1.invalidInput }
        return request
    }

    func verifyPayment(_ message: IrohaPeerWireMessageV1,
                       matching hosted: IrohaPeerWireMessageV1) throws -> KagemushaSpartanVerifiedPaymentV1 {
        record("verifyPayment")
        guard let binding = bindings[message.wireHash], binding.request == hosted.wireHash else {
            throw KagemushaWalletErrorV1.invalidInput
        }
        return .init(digest: Data(repeating: 9, count: 32), request: binding.completed)
    }

    func output(operationId: Data, part: KagemushaSpartanOutputV1) throws -> KagemushaSpartanRetainedOutputV1 {
        record("output")
        guard part == .payment, let bytes = retained[operationId] else { return .unknown }
        return .released(bytes)
    }

    func verifyCreditStatus(sendOperationId: Data, request: IrohaPeerWireMessageV1?,
                            status: IrohaPeerWireMessageV1) throws -> KagemushaSpartanCreditOutcomeV1 {
        record("verifyCreditStatus")
        guard let credit = credits[status.wireHash], credit.operation == sendOperationId,
              credit.request == request?.wireHash else { throw KagemushaWalletErrorV1.invalidInput }
        return credit.outcome
    }

    func receivedCreditStatus(requestId: Data, receiveOperationId: Data,
                              payment: IrohaPeerWireMessageV1) throws -> KagemushaSpartanReceiverCreditStatusV1 {
        record("receivedCreditStatus")
        // Native compares the complete Payment bytes with the ones its released Receive took.
        guard let status = receipts[receiveOperationId], received[receiveOperationId] == payment.encoded else {
            throw KagemushaWalletErrorV1.invalidInput
        }
        return try KagemushaSpartanReceiverCreditStatusV1(KagemushaSpartanReplyV1(status: 104, sequenceLow: 0,
            sequenceHigh: 0, detail: 0, bytes: status.encoded))
    }
}

struct SpartanNfcLoopbackLostV1: Error {}
/// The response's delivery is unknowable (RF lost while CONFIRM was in flight).
struct SpartanNfcLoopbackAmbiguousV1: IrohaPeerNfcAmbiguousResponseErrorV1 {}

/// The receiver's card behind a virtual RF link. Its COMMIT handler is the receiver's Native: the Receive is
/// substituted, and the acknowledgement is obtained only through the receiver binding.
actor SpartanNfcCardLoopbackV1 {
    private var endpoint: IrohaPeerNfcCardEndpointV1
    private let binding: KagemushaSpartanPeerNfcBindingV1
    private let receiveOperation: Data
    private var lose: (@Sendable (IrohaPeerNfcCommandV1) -> Bool)?
    private var loseAmbiguously = false
    private(set) var commits = 0
    private(set) var receivedPayment: Data?
    private(set) var commands: [IrohaPeerNfcCommandV1] = []

    init(endpoint: IrohaPeerNfcCardEndpointV1, binding: KagemushaSpartanPeerNfcBindingV1, receiveOperation: Data) {
        self.endpoint = endpoint; self.binding = binding; self.receiveOperation = receiveOperation
    }

    var phase: IrohaPeerNfcPhaseV1 { endpoint.receiver.phase }

    /// The next matching command reaches the card, but its response is lost on the way back. An ambiguous loss is the
    /// transport's "delivery unknowable" error.
    func loseResponse(ambiguously: Bool = false, _ when: @escaping @Sendable (IrohaPeerNfcCommandV1) -> Bool) {
        lose = when
        loseAmbiguously = ambiguously
    }

    func transceive(_ command: IrohaPeerNfcCommandV1) throws -> IrohaPeerNfcAPDUResponseV1 {
        commands.append(command)
        let binding = binding, operation = receiveOperation
        var committed = 0
        var received: Data?
        let response = endpoint.process(apdu: try IrohaPeerNfcAPDUCodecV1.encode(command),
            admitPayment: { try IrohaPeerNfcDurablePaymentAdmissionV1(context: $0) },
            durableCommit: { context in
                committed += 1
                received = context.payment.encoded
                let status = try binding.acknowledgement(receiveOperationId: operation, payment: context.payment)
                return try IrohaPeerNfcDurableAcknowledgementV1(context: context, acknowledgement: status.message.encoded)
            })
        commits += committed
        if let received { receivedPayment = received }
        if let lose, lose(command) {
            self.lose = nil
            if loseAmbiguously { throw SpartanNfcLoopbackAmbiguousV1() }
            throw SpartanNfcLoopbackLostV1()
        }
        return response
    }
}
