import XCTest
@testable import IrohaSwift

func canonicalSignedTransactionPayload(_ signedTransaction: Data) throws -> Data {
    var reader = CanonicalNoritoReader(data: signedTransaction)
    _ = try reader.readField()
    let payload = try reader.readField()
    _ = try reader.readField()
    guard reader.remaining() == 0 else {
        throw CanonicalNoritoDecodingError.invalidField(
            "signed transaction must contain exactly three fields"
        )
    }
    return payload
}

func XCTAssertCanonicalExternalEntrypointHash(
    _ envelope: SignedTransactionEnvelope,
    file: StaticString = #filePath,
    line: UInt = #line
) {
    let transactionPayload: Data
    do {
        transactionPayload = try canonicalSignedTransactionPayload(envelope.signedTransaction)
    } catch {
        XCTFail(
            "invalid canonical signed transaction: \(error)",
            file: file,
            line: line
        )
        return
    }

    var compact = CompactNoritoWriter()
    compact.writeUInt32LE(0)
    compact.writeField(transactionPayload)
    XCTAssertEqual(
        envelope.transactionHash,
        IrohaHash.hash(compact.data),
        "transaction hash must use compact TransactionEntrypoint::External intent framing",
        file: file,
        line: line
    )

    var fixedWidth = CanonicalNoritoWriter()
    fixedWidth.writeUInt32LE(0)
    fixedWidth.writeField(transactionPayload)
    XCTAssertNotEqual(
        envelope.transactionHash,
        IrohaHash.hash(fixedWidth.data),
        "fixed-u64 entrypoint framing must not alias the canonical transaction hash",
        file: file,
        line: line
    )
}
