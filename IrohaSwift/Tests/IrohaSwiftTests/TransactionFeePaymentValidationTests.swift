import XCTest
@testable import IrohaSwift

final class TransactionFeePaymentValidationTests: XCTestCase {
    private let sponsor =
        "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53"
    private let assetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"

    private func chargeLimits() throws -> [FeeChargeLimit] {
        [
            try FeeChargeLimit(
                kind: .nexus,
                assetDefinitionId: assetDefinitionId,
                maxAmount: "3.25"
            ),
            try FeeChargeLimit(
                kind: .pipelineGas,
                assetDefinitionId: assetDefinitionId,
                maxAmount: "9"
            ),
        ]
    }

    private func sponsorIntent() throws -> FeePaymentIntent {
        .sponsor(
            programId: try FeeSponsorProgramId(sponsor: sponsor, name: "wallet_fx"),
            programRevision: 7,
            chargeLimits: try chargeLimits(),
            gasLimit: 750_000
        )
    }

    /// Compact `FeePaymentIntent` frame: u32 payer tag followed by the variant body field.
    private func rawFeePayment(payer: UInt32, body: Data) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeUInt32LE(payer)
        writer.writeField(body)
        return writer.data
    }

    private func rawAuthorityBody(
        chargeLimits: Data = CompactNorito.encodeUInt64(0),
        gasLimit: Data = Data([0])
    ) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(chargeLimits)
        writer.writeField(gasLimit)
        return writer.data
    }

    private func rawSponsorBody(
        name: String = "wallet_fx",
        revision: UInt64 = 7,
        chargeLimits: Data = CompactNorito.encodeUInt64(0),
        gasLimit: Data = Data([0])
    ) throws -> Data {
        var program = CompactNoritoWriter()
        program.writeField(
            try exactCanonicalToriiAccountAddress(sponsor).address
                .compactNoritoAccountControllerPayload()
        )
        program.writeField(CompactNorito.encodeString(name))
        var sponsored = CompactNoritoWriter()
        sponsored.writeField(program.data)
        sponsored.writeField(CompactNorito.encodeUInt64(revision))
        sponsored.writeField(chargeLimits)
        sponsored.writeField(gasLimit)
        return sponsored.data
    }

    /// Compact charge-limit sequence: fixed-width u64 count, then one field per item.
    private func rawChargeLimits(_ items: [Data]) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeUInt64LE(UInt64(items.count))
        for item in items {
            writer.writeField(item)
        }
        return writer.data
    }

    private func rawGasLimit(_ value: UInt64) throws -> Data {
        try CompactNorito.encodeOption(value, encode: CompactNorito.encodeUInt64)
    }

    /// Splits the typed compact encoding into its charge-limit items so tests can reorder them.
    private func encodedChargeLimitItems() throws -> [Data] {
        let encoded = try FeePaymentIntent.authority(
            chargeLimits: chargeLimits(),
            gasLimit: nil
        ).compactNorito()
        var intent = ToriiVerifyingKeyCompactReader(encoded)
        let payer = try intent.takeUInt32("payer")
        XCTAssertEqual(payer, 0)
        var body = ToriiVerifyingKeyCompactReader(try intent.takeField("body"))
        var limits = ToriiVerifyingKeyCompactReader(try body.takeField("charge_limits"))
        let count = try limits.takeUInt64("charge_limits.count")
        var items: [Data] = []
        for _ in 0..<count {
            items.append(try limits.takeField("charge_limits.item"))
        }
        XCTAssertTrue(limits.isFinished)
        XCTAssertEqual(items.count, 2)
        return items
    }

    private func assertInvalidPayload(
        _ expression: @autoclosure () throws -> Void,
        containing fragment: String,
        file: StaticString = #filePath,
        line: UInt = #line
    ) {
        XCTAssertThrowsError(try expression(), file: file, line: line) { error in
            guard case let ToriiClientError.invalidPayload(message) = error else {
                return XCTFail("unexpected error: \(error)", file: file, line: line)
            }
            XCTAssertTrue(
                message.contains(fragment),
                "expected \(fragment) in \(message)",
                file: file,
                line: line
            )
        }
    }

    private func assertFeePaymentRejected(
        _ payload: Data,
        containing fragment: String,
        file: StaticString = #filePath,
        line: UInt = #line
    ) {
        assertInvalidPayload(
            try TransactionFeePaymentValidation.requireCanonicalTransactionFeePayment(payload),
            containing: fragment,
            file: file,
            line: line
        )
    }

    func testAcceptsTypedAuthorityAndSponsorIntents() throws {
        let intents: [FeePaymentIntent] = [
            .authority(chargeLimits: [], gasLimit: nil),
            .authority(chargeLimits: try chargeLimits(), gasLimit: 500_000),
            .sponsor(
                programId: try FeeSponsorProgramId(sponsor: sponsor, name: "wallet_fx"),
                programRevision: 1,
                chargeLimits: [],
                gasLimit: nil
            ),
            try sponsorIntent(),
        ]
        for intent in intents {
            XCTAssertNoThrow(
                try TransactionFeePaymentValidation.requireCanonicalTransactionFeePayment(
                    intent.compactNorito()
                ),
                "\(intent)"
            )
        }
    }

    func testRawBuildersMatchTypedCompactEncoding() throws {
        // The rejection cases below mutate these raw frames, so they must first
        // reproduce the typed encoder byte for byte.
        XCTAssertEqual(
            rawFeePayment(payer: 0, body: rawAuthorityBody()),
            try FeePaymentIntent.authority(chargeLimits: [], gasLimit: nil).compactNorito()
        )
        XCTAssertEqual(
            rawFeePayment(
                payer: 1,
                body: try rawSponsorBody(
                    chargeLimits: rawChargeLimits(encodedChargeLimitItems()),
                    gasLimit: rawGasLimit(750_000)
                )
            ),
            try sponsorIntent().compactNorito()
        )
    }

    func testRejectsNonNfcSponsorProgramNameInRawCompactPayload() throws {
        let precomposed = rawFeePayment(
            payer: 1,
            body: try rawSponsorBody(name: "\u{e9}", revision: 1)
        )
        XCTAssertNoThrow(
            try TransactionFeePaymentValidation.requireCanonicalTransactionFeePayment(precomposed)
        )

        let decomposed = rawFeePayment(
            payer: 1,
            body: try rawSponsorBody(name: "e\u{301}", revision: 1)
        )
        assertFeePaymentRejected(decomposed, containing: "sponsor program name is invalid")
    }

    func testRejectsUnknownPayerTag() throws {
        var payload = try FeePaymentIntent.authority(chargeLimits: [], gasLimit: nil)
            .compactNorito()
        payload[payload.startIndex] = 2
        assertFeePaymentRejected(payload, containing: "unknown payer variant")
    }

    func testRejectsTrailingBytes() throws {
        var outer = try sponsorIntent().compactNorito()
        outer.append(0)
        assertFeePaymentRejected(outer, containing: "fee_payment contains trailing bytes")

        var body = rawAuthorityBody()
        body.append(0)
        assertFeePaymentRejected(
            rawFeePayment(payer: 0, body: body),
            containing: "fee_payment contains trailing bytes"
        )
    }

    func testRejectsZeroProgramRevision() throws {
        assertFeePaymentRejected(
            rawFeePayment(payer: 1, body: try rawSponsorBody(revision: 0)),
            containing: "fee_payment.program_revision must be a positive canonical UInt64"
        )
    }

    func testRejectsZeroOrMalformedGasLimit() throws {
        assertFeePaymentRejected(
            rawFeePayment(payer: 0, body: rawAuthorityBody(gasLimit: try rawGasLimit(0))),
            containing: "fee_payment.gas_limit must be a positive canonical UInt64"
        )
        assertFeePaymentRejected(
            rawFeePayment(payer: 1, body: try rawSponsorBody(gasLimit: rawGasLimit(0))),
            containing: "fee_payment.gas_limit must be a positive canonical UInt64"
        )
        assertFeePaymentRejected(
            rawFeePayment(payer: 0, body: rawAuthorityBody(gasLimit: Data([2]))),
            containing: "fee_payment.gas_limit contains an invalid option tag"
        )
    }

    func testRejectsOutOfOrderDuplicateAndExcessChargeKinds() throws {
        let items = try encodedChargeLimitItems()
        XCTAssertNoThrow(
            try TransactionFeePaymentValidation.requireCanonicalTransactionFeePayment(
                rawFeePayment(payer: 0, body: rawAuthorityBody(chargeLimits: rawChargeLimits(items)))
            )
        )
        // pipelineGas before nexus, then a duplicated nexus limit.
        for reordered in [[items[1], items[0]], [items[0], items[0]]] {
            assertFeePaymentRejected(
                rawFeePayment(
                    payer: 0,
                    body: rawAuthorityBody(chargeLimits: rawChargeLimits(reordered))
                ),
                containing: "unique canonical charge-kind order"
            )
            assertFeePaymentRejected(
                rawFeePayment(
                    payer: 1,
                    body: try rawSponsorBody(chargeLimits: rawChargeLimits(reordered))
                ),
                containing: "unique canonical charge-kind order"
            )
        }
        assertFeePaymentRejected(
            rawFeePayment(
                payer: 0,
                body: rawAuthorityBody(chargeLimits: rawChargeLimits(items + [items[1]]))
            ),
            containing: "duplicate or unknown charge kinds"
        )
    }

    func testEmptyMetadataRequiresExactEncoding() throws {
        XCTAssertNoThrow(
            try TransactionFeePaymentValidation.requireEmptyTransactionMetadata(
                Data(repeating: 0, count: 8)
            )
        )
        assertInvalidPayload(
            try TransactionFeePaymentValidation.requireEmptyTransactionMetadata(
                CompactNorito.encodeUInt64(1)
            ),
            containing: "exact empty encoding"
        )
        assertInvalidPayload(
            try TransactionFeePaymentValidation.requireEmptyTransactionMetadata(
                Data(repeating: 0, count: 9)
            ),
            containing: "exact empty encoding"
        )
        assertInvalidPayload(
            try TransactionFeePaymentValidation.requireEmptyTransactionMetadata(
                Data(repeating: 0, count: 7)
            ),
            containing: "metadata.count is truncated"
        )
    }
}
