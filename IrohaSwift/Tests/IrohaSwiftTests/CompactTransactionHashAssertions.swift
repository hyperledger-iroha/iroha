import XCTest
@testable import IrohaSwift

func XCTAssertCanonicalExternalEntrypointHash(
    _ envelope: SignedTransactionEnvelope,
    file: StaticString = #filePath,
    line: UInt = #line
) {
    var compact = CompactNoritoWriter()
    compact.writeUInt32LE(0)
    compact.writeField(envelope.signedTransaction)
    XCTAssertEqual(
        envelope.transactionHash,
        IrohaHash.hash(compact.data),
        "transaction hash must use compact TransactionEntrypoint::External framing",
        file: file,
        line: line
    )

    var fixedWidth = CanonicalNoritoWriter()
    fixedWidth.writeUInt32LE(0)
    fixedWidth.writeField(envelope.signedTransaction)
    XCTAssertNotEqual(
        envelope.transactionHash,
        IrohaHash.hash(fixedWidth.data),
        "fixed-u64 entrypoint framing must not alias the canonical transaction hash",
        file: file,
        line: line
    )
}
