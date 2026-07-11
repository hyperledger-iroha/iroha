import Foundation
@testable import IrohaSwift

/// Builds a complete, canonically framed verifier-record archive for tests that
/// exercise APIs requiring an already validated registry record.
func canonicalKagemushaVerifierRecordArchive(
    seed: UInt8 = 0x5d,
    verifierKeyLength: Int = 96
) throws -> Data {
    precondition(verifierKeyLength > 0)
    let verifierKey = Data((0..<verifierKeyLength).map { index in
        UInt8((Int(seed) + index * 29) & 0xff)
    })
    return try KagemushaRecursiveSpendRequestCodecs
        .encodeConfidentialTransferV2VerifierRecordArchive(verifierKey: verifierKey)
}

/// Binds a canonical test archive to the registry identifier used by the test.
func canonicalKagemushaVerifierRecordRef(
    verifierKeyId: String,
    seed: UInt8 = 0x5d,
    verifierKeyLength: Int = 96
) throws -> KagemushaRecursiveSpendVerifierRecordRef {
    try KagemushaRecursiveSpendVerifierRecordRef(
        verifierKeyId: verifierKeyId,
        recordBytes: canonicalKagemushaVerifierRecordArchive(
            seed: seed,
            verifierKeyLength: verifierKeyLength
        )
    )
}
