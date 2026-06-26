import Foundation
import CryptoKit
import XCTest
@testable import IrohaSwift

final class KagemushaRecursiveSpendProverTests: XCTestCase {
    func testPreferredModePrefersRecursiveCompactWhenAvailable() {
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.preferredMode(
                recursiveCompactAvailable: true,
                recursiveSpendAvailable: true
            ),
            .recursiveCompactV1
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.preferredMode(
                recursiveCompactAvailable: true,
                recursiveSpendAvailable: false
            ),
            .recursiveCompactV1
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.preferredMode(recursiveSpendAvailable: true),
            .recursiveSpendV1
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.preferredMode(recursiveSpendAvailable: false),
            .checkedPrefoldV1
        )
        XCTAssertEqual(KagemushaOfflineSpendMode.recursiveCompactV1.rawValue, "recursive_compact_v1")
        XCTAssertEqual(KagemushaRecursiveCompactPaymentTokenProver.requiredNativeBridgeAbiVersion, 7)
        XCTAssertEqual(
            KagemushaRecursiveCompactPaymentTokenProver.recursiveCompactCircuitIdV1,
            "kagemusha-recursive-compact-v1"
        )
    }

    func testLineageKeyArtifactPackagesValidateReleaseProfiles() throws {
        XCTAssertTrue(KagemushaRecursiveSpendProver.isSupportedLineageKeyArtifactOpeningLen(2))
        XCTAssertTrue(KagemushaRecursiveSpendProver.isSupportedLineageKeyArtifactOpeningLen(128))
        XCTAssertFalse(KagemushaRecursiveSpendProver.isSupportedLineageKeyArtifactOpeningLen(3))
        XCTAssertFalse(KagemushaRecursiveSpendProver.isSupportedLineageKeyArtifactOpeningLen(0))

        let initVerifierKey = Self.lineageVerifierKey(
            circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1,
            seed: 0xA1
        )
        let initProvingKeyArchive = Self.lineageProvingKeyArchive(
            circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1,
            verifierKey: initVerifierKey,
            seed: 0xA2
        )
        let appendVerifierKey = Self.lineageVerifierKey(
            circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1,
            seed: 0xA3
        )
        let appendProvingKeyArchive = Self.lineageProvingKeyArchive(
            circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1,
            verifierKey: appendVerifierKey,
            seed: 0xA4
        )

        var verifierKey = initVerifierKey
        var provingKeyArchive = initProvingKeyArchive
        let initArtifacts = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
            verifierOpeningLen: 2,
            lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
            lineageVerifierKey: verifierKey,
            lineageProvingKeyArchive: provingKeyArchive
        )
        verifierKey[0] = 0
        provingKeyArchive[0] = 0
        XCTAssertTrue(initArtifacts.isInitArtifact)
        XCTAssertFalse(initArtifacts.isAppendArtifact)
        XCTAssertEqual(initArtifacts.lineageVerifierKey, initVerifierKey)
        XCTAssertEqual(initArtifacts.lineageProvingKeyArchive, initProvingKeyArchive)
        var exposedVerifierKey = initArtifacts.lineageVerifierKey
        var exposedProvingKeyArchive = initArtifacts.lineageProvingKeyArchive
        exposedVerifierKey[0] = 0
        exposedProvingKeyArchive[0] = 0
        XCTAssertEqual(initArtifacts.lineageVerifierKey[0], 0x5A)
        XCTAssertEqual(initArtifacts.lineageProvingKeyArchive, initProvingKeyArchive)
        XCTAssertEqual(
            try KagemushaRecursiveSpendProver.validateLineageKeyArtifacts(initArtifacts),
            initArtifacts
        )

        let appendArtifacts = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForAppend(
            verifierOpeningLen: 2,
            lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
            lineageVerifierKey: appendVerifierKey,
            lineageProvingKeyArchive: appendProvingKeyArchive
        )
        XCTAssertFalse(appendArtifacts.isInitArtifact)
        XCTAssertTrue(appendArtifacts.isAppendArtifact)

        try assertInvalidLineageKeyArtifact("lineage_verifier_key") {
            _ = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                verifierOpeningLen: 2,
                lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                lineageVerifierKey: appendVerifierKey,
                lineageProvingKeyArchive: appendProvingKeyArchive
            )
        }
        try assertInvalidLineageKeyArtifact("lineage_proving_key_archive") {
            _ = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                verifierOpeningLen: 2,
                lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                lineageVerifierKey: initVerifierKey,
                lineageProvingKeyArchive: appendProvingKeyArchive
            )
        }
        try assertInvalidLineageKeyArtifact("lineage_verifier_key") {
            _ = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                verifierOpeningLen: 2,
                lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                lineageVerifierKey: Data("not-zk1".utf8),
                lineageProvingKeyArchive: initProvingKeyArchive
            )
        }
        let duplicateCidVerifierKey = initVerifierKey + Self.zk1Tlv(
            tag: "CID1",
            payload: Data(KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1.utf8)
        )
        try assertInvalidLineageKeyArtifact("lineage_verifier_key") {
            _ = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                verifierOpeningLen: 2,
                lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                lineageVerifierKey: duplicateCidVerifierKey,
                lineageProvingKeyArchive: initProvingKeyArchive
            )
        }
        let whitespaceCidVerifierKey = Self.lineageVerifierKey(
            circuitId: " \(KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1) ",
            seed: 0xA5
        )
        let whitespaceCidProvingKeyArchive = Self.lineageProvingKeyArchive(
            circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1,
            verifierKey: whitespaceCidVerifierKey,
            seed: 0xA6
        )
        try assertInvalidLineageKeyArtifact("lineage_verifier_key") {
            _ = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                verifierOpeningLen: 2,
                lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                lineageVerifierKey: whitespaceCidVerifierKey,
                lineageProvingKeyArchive: whitespaceCidProvingKeyArchive
            )
        }
        try assertInvalidLineageKeyArtifact("lineage_proving_key_archive") {
            _ = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                verifierOpeningLen: 2,
                lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                lineageVerifierKey: initVerifierKey,
                lineageProvingKeyArchive: Data("not-norito".utf8)
            )
        }
        let missingCircuitArchive = Self.lineageProvingKeyArchiveRaw(
            version: 1,
            circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1,
            verifierKeyCommitment: Self.verifierKeyCommitment(verifierKey: initVerifierKey),
            provingKey: Data(repeating: 0xA5, count: 64)
        )
        try assertInvalidLineageKeyArtifact("lineage_proving_key_archive") {
            _ = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                verifierOpeningLen: 2,
                lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                lineageVerifierKey: initVerifierKey,
                lineageProvingKeyArchive: missingCircuitArchive
            )
        }
        var smuggledCircuitProvingKey = Data(
            KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1.utf8
        )
        smuggledCircuitProvingKey.append(Data(repeating: 0xA6, count: 64))
        let smuggledCircuitArchive = Self.lineageProvingKeyArchiveRaw(
            version: 1,
            circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1,
            verifierKeyCommitment: Self.verifierKeyCommitment(verifierKey: initVerifierKey),
            provingKey: smuggledCircuitProvingKey
        )
        try assertInvalidLineageKeyArtifact("lineage_proving_key_archive") {
            _ = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                verifierOpeningLen: 2,
                lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                lineageVerifierKey: initVerifierKey,
                lineageProvingKeyArchive: smuggledCircuitArchive
            )
        }
        let wrongCommitmentArchive = Self.lineageProvingKeyArchive(
            circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1,
            verifierKey: appendVerifierKey,
            seed: 0xA6
        )
        try assertInvalidLineageKeyArtifact("lineage_proving_key_archive") {
            _ = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                verifierOpeningLen: 2,
                lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                lineageVerifierKey: initVerifierKey,
                lineageProvingKeyArchive: wrongCommitmentArchive
            )
        }
        var smuggledCommitmentProvingKey = Self.verifierKeyCommitment(verifierKey: initVerifierKey)
        smuggledCommitmentProvingKey.append(Data(repeating: 0xA7, count: 64))
        let smuggledCommitmentArchive = Self.lineageProvingKeyArchiveRaw(
            version: 1,
            circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1,
            verifierKeyCommitment: Self.verifierKeyCommitment(verifierKey: appendVerifierKey),
            provingKey: smuggledCommitmentProvingKey
        )
        try assertInvalidLineageKeyArtifact("lineage_proving_key_archive") {
            _ = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                verifierOpeningLen: 2,
                lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                lineageVerifierKey: initVerifierKey,
                lineageProvingKeyArchive: smuggledCommitmentArchive
            )
        }
        let wrongVersionArchive = Self.lineageProvingKeyArchiveRaw(
            version: 2,
            circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1,
            verifierKeyCommitment: Self.verifierKeyCommitment(verifierKey: initVerifierKey),
            provingKey: Data(repeating: 0xA8, count: 64)
        )
        try assertInvalidLineageKeyArtifact("lineage_proving_key_archive") {
            _ = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                verifierOpeningLen: 2,
                lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                lineageVerifierKey: initVerifierKey,
                lineageProvingKeyArchive: wrongVersionArchive
            )
        }
        let emptyProvingKeyArchive = Self.lineageProvingKeyArchiveRaw(
            version: 1,
            circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1,
            verifierKeyCommitment: Self.verifierKeyCommitment(verifierKey: initVerifierKey),
            provingKey: Data()
        )
        try assertInvalidLineageKeyArtifact("lineage_proving_key_archive") {
            _ = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                verifierOpeningLen: 2,
                lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                lineageVerifierKey: initVerifierKey,
                lineageProvingKeyArchive: emptyProvingKeyArchive
            )
        }
        let trailingPayloadArchive = Self.lineageProvingKeyArchiveRaw(
            version: 1,
            circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1,
            verifierKeyCommitment: Self.verifierKeyCommitment(verifierKey: initVerifierKey),
            provingKey: Data(repeating: 0xA9, count: 64),
            trailingPayload: Data([0x7f])
        )
        try assertInvalidLineageKeyArtifact("lineage_proving_key_archive") {
            _ = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                verifierOpeningLen: 2,
                lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                lineageVerifierKey: initVerifierKey,
                lineageProvingKeyArchive: trailingPayloadArchive
            )
        }
        let oldSchemaArchive = Self.lineageProvingKeyArchiveRaw(
            version: 1,
            circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1,
            verifierKeyCommitment: Self.verifierKeyCommitment(verifierKey: initVerifierKey),
            provingKey: Data(repeating: 0xAA, count: 64),
            schemaHash: Self.oldKagemushaLineageProvingKeyArchiveSchemaHash
        )
        try assertInvalidLineageKeyArtifact("lineage_proving_key_archive") {
            _ = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                verifierOpeningLen: 2,
                lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                lineageVerifierKey: initVerifierKey,
                lineageProvingKeyArchive: oldSchemaArchive
            )
        }
        let packedStructArchive = Self.lineageProvingKeyArchiveRaw(
            version: 1,
            circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1,
            verifierKeyCommitment: Self.verifierKeyCommitment(verifierKey: initVerifierKey),
            provingKey: Data(repeating: 0xAB, count: 64),
            flags: NoritoHeader.compactLen | NoritoHeader.packedStruct
        )
        try assertInvalidLineageKeyArtifact("lineage_proving_key_archive") {
            _ = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                verifierOpeningLen: 2,
                lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                lineageVerifierKey: initVerifierKey,
                lineageProvingKeyArchive: packedStructArchive
            )
        }
        let fieldBitsetArchive = Self.lineageProvingKeyArchiveRaw(
            version: 1,
            circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1,
            verifierKeyCommitment: Self.verifierKeyCommitment(verifierKey: initVerifierKey),
            provingKey: Data(repeating: 0xAC, count: 64),
            flags: NoritoHeader.compactLen | NoritoHeader.fieldBitset
        )
        try assertInvalidLineageKeyArtifact("lineage_proving_key_archive") {
            _ = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                verifierOpeningLen: 2,
                lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                lineageVerifierKey: initVerifierKey,
                lineageProvingKeyArchive: fieldBitsetArchive
            )
        }
        let circuitIdBytes = Data(
            KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1.utf8
        )
        var overlongVersionLengthPayload = Data()
        overlongVersionLengthPayload.append(Self.noritoOverlongCompactLength(2))
        overlongVersionLengthPayload.append(contentsOf: [1, 0])
        overlongVersionLengthPayload.append(Self.noritoField(
            Self.noritoString(
                KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1,
                flags: NoritoHeader.compactLen
            ),
            flags: NoritoHeader.compactLen
        ))
        overlongVersionLengthPayload.append(Self.noritoField(
            Self.verifierKeyCommitment(verifierKey: initVerifierKey),
            flags: NoritoHeader.compactLen
        ))
        overlongVersionLengthPayload.append(Self.noritoField(
            Self.noritoByteVec(Data(repeating: 0xAD, count: 64)),
            flags: NoritoHeader.compactLen
        ))
        let overlongVersionLengthArchive = Self.noritoFrameFromSchemaHash(
            Self.kagemushaLineageProvingKeyArchiveSchemaHash,
            payload: overlongVersionLengthPayload,
            flags: NoritoHeader.compactLen
        )
        try assertInvalidLineageKeyArtifact("lineage_proving_key_archive") {
            _ = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                verifierOpeningLen: 2,
                lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                lineageVerifierKey: initVerifierKey,
                lineageProvingKeyArchive: overlongVersionLengthArchive
            )
        }
        var oversizedTerminalCompactLengthPayload = Data()
        oversizedTerminalCompactLengthPayload.append(Self.noritoOversizedTerminalCompactLength())
        oversizedTerminalCompactLengthPayload.append(contentsOf: [1, 0])
        oversizedTerminalCompactLengthPayload.append(Self.noritoField(
            Self.noritoString(
                KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1,
                flags: NoritoHeader.compactLen
            ),
            flags: NoritoHeader.compactLen
        ))
        oversizedTerminalCompactLengthPayload.append(Self.noritoField(
            Self.verifierKeyCommitment(verifierKey: initVerifierKey),
            flags: NoritoHeader.compactLen
        ))
        oversizedTerminalCompactLengthPayload.append(Self.noritoField(
            Self.noritoByteVec(Data(repeating: 0xB0, count: 64)),
            flags: NoritoHeader.compactLen
        ))
        let oversizedTerminalCompactLengthArchive = Self.noritoFrameFromSchemaHash(
            Self.kagemushaLineageProvingKeyArchiveSchemaHash,
            payload: oversizedTerminalCompactLengthPayload,
            flags: NoritoHeader.compactLen
        )
        try assertInvalidLineageKeyArtifact("lineage_proving_key_archive") {
            _ = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                verifierOpeningLen: 2,
                lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                lineageVerifierKey: initVerifierKey,
                lineageProvingKeyArchive: oversizedTerminalCompactLengthArchive
            )
        }
        var hugeCanonicalCompactLengthPayload = Data()
        hugeCanonicalCompactLengthPayload.append(Self.noritoHugeCanonicalCompactLength())
        hugeCanonicalCompactLengthPayload.append(contentsOf: [1, 0])
        hugeCanonicalCompactLengthPayload.append(Self.noritoField(
            Self.noritoString(
                KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1,
                flags: NoritoHeader.compactLen
            ),
            flags: NoritoHeader.compactLen
        ))
        hugeCanonicalCompactLengthPayload.append(Self.noritoField(
            Self.verifierKeyCommitment(verifierKey: initVerifierKey),
            flags: NoritoHeader.compactLen
        ))
        hugeCanonicalCompactLengthPayload.append(Self.noritoField(
            Self.noritoByteVec(Data(repeating: 0xB1, count: 64)),
            flags: NoritoHeader.compactLen
        ))
        let hugeCanonicalCompactLengthArchive = Self.noritoFrameFromSchemaHash(
            Self.kagemushaLineageProvingKeyArchiveSchemaHash,
            payload: hugeCanonicalCompactLengthPayload,
            flags: NoritoHeader.compactLen
        )
        try assertInvalidLineageKeyArtifact("lineage_proving_key_archive") {
            _ = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                verifierOpeningLen: 2,
                lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                lineageVerifierKey: initVerifierKey,
                lineageProvingKeyArchive: hugeCanonicalCompactLengthArchive
            )
        }
        var overflowingProvingKeyByteVec = Data()
        Self.appendUInt64LE(UInt64(Int.max), to: &overflowingProvingKeyByteVec)
        var overflowingProvingKeyByteVecPayload = Data()
        overflowingProvingKeyByteVecPayload.append(Self.noritoField(
            Data([1, 0]),
            flags: NoritoHeader.compactLen
        ))
        overflowingProvingKeyByteVecPayload.append(Self.noritoField(
            Self.noritoString(
                KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1,
                flags: NoritoHeader.compactLen
            ),
            flags: NoritoHeader.compactLen
        ))
        overflowingProvingKeyByteVecPayload.append(Self.noritoField(
            Self.verifierKeyCommitment(verifierKey: initVerifierKey),
            flags: NoritoHeader.compactLen
        ))
        overflowingProvingKeyByteVecPayload.append(Self.noritoField(
            overflowingProvingKeyByteVec,
            flags: NoritoHeader.compactLen
        ))
        let overflowingProvingKeyByteVecArchive = Self.noritoFrameFromSchemaHash(
            Self.kagemushaLineageProvingKeyArchiveSchemaHash,
            payload: overflowingProvingKeyByteVecPayload,
            flags: NoritoHeader.compactLen
        )
        try assertInvalidLineageKeyArtifact("lineage_proving_key_archive") {
            _ = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                verifierOpeningLen: 2,
                lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                lineageVerifierKey: initVerifierKey,
                lineageProvingKeyArchive: overflowingProvingKeyByteVecArchive
            )
        }
        var overlongCircuitString = Self.noritoOverlongCompactLength(circuitIdBytes.count)
        overlongCircuitString.append(circuitIdBytes)
        var overlongCircuitStringPayload = Data()
        overlongCircuitStringPayload.append(Self.noritoField(Data([1, 0]), flags: NoritoHeader.compactLen))
        overlongCircuitStringPayload.append(Self.noritoField(overlongCircuitString, flags: NoritoHeader.compactLen))
        overlongCircuitStringPayload.append(Self.noritoField(
            Self.verifierKeyCommitment(verifierKey: initVerifierKey),
            flags: NoritoHeader.compactLen
        ))
        overlongCircuitStringPayload.append(Self.noritoField(
            Self.noritoByteVec(Data(repeating: 0xAE, count: 64)),
            flags: NoritoHeader.compactLen
        ))
        let overlongCircuitStringArchive = Self.noritoFrameFromSchemaHash(
            Self.kagemushaLineageProvingKeyArchiveSchemaHash,
            payload: overlongCircuitStringPayload,
            flags: NoritoHeader.compactLen
        )
        try assertInvalidLineageKeyArtifact("lineage_proving_key_archive") {
            _ = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                verifierOpeningLen: 2,
                lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                lineageVerifierKey: initVerifierKey,
                lineageProvingKeyArchive: overlongCircuitStringArchive
            )
        }
        var invalidUtf8Circuit = Self.noritoLength(1, flags: NoritoHeader.compactLen)
        invalidUtf8Circuit.append(0xff)
        var invalidUtf8ProvingKey = circuitIdBytes
        invalidUtf8ProvingKey.append(Data(repeating: 0xAF, count: 64))
        var invalidUtf8CircuitPayload = Data()
        invalidUtf8CircuitPayload.append(Self.noritoField(Data([1, 0]), flags: NoritoHeader.compactLen))
        invalidUtf8CircuitPayload.append(Self.noritoField(invalidUtf8Circuit, flags: NoritoHeader.compactLen))
        invalidUtf8CircuitPayload.append(Self.noritoField(
            Self.verifierKeyCommitment(verifierKey: initVerifierKey),
            flags: NoritoHeader.compactLen
        ))
        invalidUtf8CircuitPayload.append(Self.noritoField(
            Self.noritoByteVec(invalidUtf8ProvingKey),
            flags: NoritoHeader.compactLen
        ))
        let invalidUtf8CircuitArchive = Self.noritoFrameFromSchemaHash(
            Self.kagemushaLineageProvingKeyArchiveSchemaHash,
            payload: invalidUtf8CircuitPayload,
            flags: NoritoHeader.compactLen
        )
        try assertInvalidLineageKeyArtifact("lineage_proving_key_archive") {
            _ = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                verifierOpeningLen: 2,
                lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                lineageVerifierKey: initVerifierKey,
                lineageProvingKeyArchive: invalidUtf8CircuitArchive
            )
        }
        try assertInvalidLineageKeyArtifact("lineage_proving_key_archive") {
            _ = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                verifierOpeningLen: 2,
                lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                lineageVerifierKey: initVerifierKey,
                lineageProvingKeyArchive: Self.emptyPayloadKagemushaNoritoArchive()
            )
        }

        try assertInvalidLineageKeyArtifact("proof_circuit_id") {
            _ = try KagemushaRecursiveSpendProver.lineageKeyArtifacts(
                proofCircuitId: "kagemusha-recursive-spend-lineage-forged-circuit",
                verifierOpeningLen: 2,
                lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                lineageVerifierKey: Data(repeating: 0xE7, count: 64),
                lineageProvingKeyArchive: Data(repeating: 0xE8, count: 64)
            )
        }
        try assertInvalidLineageKeyArtifact("verifier_opening_len") {
            _ = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                verifierOpeningLen: 3,
                lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                lineageVerifierKey: Data(repeating: 0xE7, count: 64),
                lineageProvingKeyArchive: Data(repeating: 0xE8, count: 64)
            )
        }
        for backend in ["halo2/kzg", " halo2/ipa", "halo2/ipa ", "HALO2/IPA"] {
            try assertInvalidLineageKeyArtifact("lineage_verifier_key") {
                _ = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                    verifierOpeningLen: 2,
                    lineageVerifierKeyBackend: backend,
                    lineageVerifierKey: Data(repeating: 0xE7, count: 64),
                    lineageProvingKeyArchive: Data(repeating: 0xE8, count: 64)
                )
            }
        }
        try assertInvalidLineageKeyArtifact("lineage_verifier_key") {
            _ = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                verifierOpeningLen: 2,
                lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                lineageVerifierKey: Data(),
                lineageProvingKeyArchive: Data(repeating: 0xE8, count: 64)
            )
        }
        try assertInvalidLineageKeyArtifact("lineage_proving_key_archive") {
            _ = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                verifierOpeningLen: 2,
                lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                lineageVerifierKey: Data(repeating: 0xE7, count: 64),
                lineageProvingKeyArchive: Data()
            )
        }
    }

    func testSharedRecursiveSpendAbi6FixtureMatchesSdkSurface() throws {
        let manifest = try Self.sharedRecursiveSpendManifest()
        XCTAssertEqual(
            manifest["schema"] as? String,
            "iroha.kagemusha.recursive_spend.abi6.fixture_manifest.v1"
        )
        XCTAssertEqual(
            manifest["native_bridge_abi_version"] as? Int,
            Int(KagemushaRecursiveSpendProver.requiredNativeBridgeAbiVersion)
        )

        let circuitIds = try XCTUnwrap(manifest["proof_circuit_ids"] as? [String: Any])
        XCTAssertEqual(
            circuitIds["recursive_aggregation"] as? String,
            KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1
        )
        XCTAssertEqual(
            circuitIds["reserved_lineage"] as? String,
            KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1
        )
        XCTAssertEqual(
            circuitIds["reserved_lineage_one_hop"] as? String,
            KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1
        )
        XCTAssertEqual(
            circuitIds["reserved_lineage_append"] as? String,
            KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1
        )

        let limits = try XCTUnwrap(manifest["limits"] as? [String: Any])
        XCTAssertEqual(
            limits["compact_token_max_hops"] as? Int,
            Int(KagemushaRecursiveSpendProver.compactTokenMaxHops)
        )
        XCTAssertEqual(
            limits["reserved_lineage_witnessless_max_hops"] as? Int,
            Int(KagemushaRecursiveSpendProver.recursiveSpendLineageWitnesslessMaxHopsV1)
        )
        XCTAssertEqual(
            limits["previous_proof_open_envelopes_required_count"] as? Int,
            Int(KagemushaRecursiveSpendProver.recursivePreviousProofOpenEnvelopesRequiredCountV1)
        )
        XCTAssertEqual(
            limits["previous_proof_open_envelopes_max_bytes"] as? Int,
            Int(KagemushaRecursiveSpendProver.recursivePreviousProofOpenEnvelopesMaxBytes)
        )
        XCTAssertEqual(
            limits["pallas_open_envelope_max_transcript_label_bytes"] as? Int,
            Int(KagemushaRecursiveSpendProver.recursivePallasOpenEnvelopeMaxTranscriptLabelBytes)
        )
        XCTAssertEqual(
            limits["native_archive_max_bytes"] as? Int,
            Int(KagemushaRecursiveSpendProver.nativeArchiveMaxBytes)
        )

        let domains = try XCTUnwrap(manifest["domains"] as? [String: Any])
        XCTAssertEqual(
            domains["transition_profile"] as? String,
            KagemushaRecursiveSpendProver.recursiveSpendTransitionProfileDomain
        )
        XCTAssertEqual(
            domains["lineage_append_boundary_final_note_binding"] as? String,
            KagemushaRecursiveSpendProver.recursiveSpendLineageAppendBoundaryFinalNoteBindingDomainV1
        )

        let operations = try XCTUnwrap(manifest["operations"] as? [[String: Any]])
        XCTAssertEqual(manifest["operation_count"] as? Int, operations.count)
        XCTAssertEqual(operations.count, 9)
        XCTAssertEqual(
            Set(operations.compactMap { $0["symbol"] as? String }),
            Set<String>([
                "connect_norito_kagemusha_recursive_spend_init",
                "connect_norito_kagemusha_recursive_spend_append",
                "connect_norito_kagemusha_recursive_spend_transition_profile_init",
                "connect_norito_kagemusha_recursive_spend_transition_profile_append",
                "connect_norito_kagemusha_recursive_spend_lineage_append_boundary",
                "connect_norito_kagemusha_recursive_spend_lineage_witness_from_init_result",
                "connect_norito_kagemusha_recursive_spend_lineage_witness_append_result",
                "connect_norito_kagemusha_recursive_spend_verify",
                "connect_norito_kagemusha_recursive_spend_redeem"
            ])
        )
        let appendWitness = try XCTUnwrap(operations.first { $0["name"] as? String == "lineage_witness_append_result" })
        XCTAssertEqual((appendWitness["input_archives"] as? [String])?.count, 3)
        XCTAssertEqual(appendWitness["output_archive"] as? String, "KagemushaRecursiveSpendLineageWitnessV1")

        let payloadBenchmarks = try XCTUnwrap(manifest["payload_benchmarks"] as? [String: Any])
        XCTAssertEqual(payloadBenchmarks["semantic_payload_bytes"] as? Int, 1751)
        XCTAssertEqual(payloadBenchmarks["reserved_lineage_payload_bytes"] as? Int, 3847)
        XCTAssertEqual(payloadBenchmarks["reserved_lineage_transition_profile_bytes"] as? Int, 2817)

        let archiveFixture = try Self.sharedRecursiveSpendArchives()
        XCTAssertEqual(
            archiveFixture["schema"] as? String,
            "iroha.kagemusha.recursive_spend.abi6.archive_fixtures.v1"
        )
        let archives = try XCTUnwrap(archiveFixture["archives"] as? [[String: Any]])
        XCTAssertEqual(archives.count, 13)
        XCTAssertEqual(
            Set(archives.compactMap { $0["name"] as? String }),
            Set<String>([
                "init_request",
                "init_bundle",
                "transition_profile_init",
                "append_request",
                "append_bundle",
                "transition_profile_append",
                "lineage_append_boundary",
                "lineage_witness_from_init_result",
                "lineage_witness_append_result",
                "verify_request",
                "verify_result",
                "redeem_request",
                "redeem_instruction"
            ])
        )
        let requestArchiveFields = try XCTUnwrap(archiveFixture["request_archive_fields"] as? [[String: Any]])
        let fieldRecordsByType = Dictionary(
            uniqueKeysWithValues: requestArchiveFields.compactMap { entry -> (String, [[String: Any]])? in
                guard
                    let type = entry["norito_type"] as? String,
                    let fields = entry["fields"] as? [[String: Any]]
                else {
                    return nil
                }
                return (type, fields)
            }
        )
        let fieldsByType = Dictionary(
            uniqueKeysWithValues: fieldRecordsByType.map { entry in
                (entry.key, entry.value.compactMap { $0["name"] as? String })
            }
        )
        XCTAssertEqual(
            Set(fieldsByType.keys),
            Set([
                "KagemushaRecursiveSpendInitRequestV1",
                "KagemushaRecursiveSpendAppendRequestV1",
                "KagemushaRecursiveSpendVerifyRequestV1",
                "KagemushaRecursiveSpendRedeemRequestV1"
            ])
        )
        XCTAssertEqual(
            fieldsByType["KagemushaRecursiveSpendInitRequestV1"],
            [
                "record_bundle",
                "pallas_open_envelopes_archive",
                "current_note",
                "lineage_verifier_key",
                "lineage_proving_key_archive",
                "block_height"
            ]
        )
        XCTAssertEqual(
            fieldsByType["KagemushaRecursiveSpendAppendRequestV1"],
            [
                "previous_bundle",
                "record_bundle",
                "pallas_open_envelopes_archive",
                "current_note",
                "output_proof_circuit_id",
                "previous_lineage_verifier_record",
                "previous_recursive_proof_open_envelopes_archive",
                "lineage_verifier_key",
                "lineage_proving_key_archive",
                "block_height"
            ]
        )
        XCTAssertEqual(
            fieldsByType["KagemushaRecursiveSpendVerifyRequestV1"],
            ["bundle", "lineage_verifier_record", "block_height"]
        )
        XCTAssertEqual(
            fieldsByType["KagemushaRecursiveSpendRedeemRequestV1"],
            [
                "bundle",
                "recipient",
                "public_amount",
                "redeem_proof",
                "lineage_witness",
                "change_output",
                "lineage_verifier_record",
                "block_height",
                "lineage_verifier_records"
            ]
        )
        for requestType in fieldsByType.keys {
            let blockHeight = try XCTUnwrap(
                fieldRecordsByType[requestType]?.first { $0["name"] as? String == "block_height" }
            )
            XCTAssertEqual(blockHeight["type"] as? String, "Option<u64>")
            XCTAssertEqual(blockHeight["norito_default"] as? Bool, true)
            XCTAssertEqual(blockHeight["semantics"] as? String, "verifier_record_activation_height")
        }
        let redeemArchive = try XCTUnwrap(archives.first { $0["name"] as? String == "redeem_request" })
        XCTAssertEqual(redeemArchive["operation"] as? String, "redeem")
        XCTAssertEqual(redeemArchive["norito_type"] as? String, "KagemushaRecursiveSpendRedeemRequestV1")
        XCTAssertGreaterThan(redeemArchive["byte_len"] as? Int ?? 0, 0)
        XCTAssertEqual((redeemArchive["sha256_hex"] as? String)?.count, 64)
        let redeemInstructionArchive = try XCTUnwrap(archives.first { $0["name"] as? String == "redeem_instruction" })
        XCTAssertEqual(redeemInstructionArchive["norito_type"] as? String, "RedeemKagemushaRecursive")

        XCTAssertEqual(
            KagemushaRecursiveSpendProver.preferredAppendOutputCircuitId(previousHopCount: 1),
            circuitIds["reserved_lineage_append"] as? String
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.preferredAppendOutputCircuitId(previousHopCount: 63),
            circuitIds["reserved_lineage_append"] as? String
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.preferredAppendOutputCircuitId(previousHopCount: 64),
            circuitIds["recursive_aggregation"] as? String
        )
        XCTAssertFalse(KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(previousHopCount: 0))
        XCTAssertTrue(KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(previousHopCount: 63))
        XCTAssertFalse(KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(previousHopCount: 64))
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.canRedeemWitnessless(
                circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1,
                hopCount: 2
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canRedeemWitnessless(
                circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                hopCount: 65
            )
        )
    }

    func testSharedRecursiveSpendAbi7ManifestMatchesArchiveFixture() throws {
        let manifest = try Self.sharedRecursiveSpendAbi7Manifest()
        XCTAssertEqual(
            Set(manifest.keys),
            Set([
                "schema",
                "fixture_kind",
                "archive_fixture",
                "native_bridge_abi_version",
                "operation_count",
                "generator",
                "domains",
                "operations"
            ])
        )
        XCTAssertEqual(
            manifest["schema"] as? String,
            "iroha.kagemusha.recursive_spend.abi7.fixture_manifest.v1"
        )
        XCTAssertEqual(manifest["fixture_kind"] as? String, "native_bridge_norito_archives")
        XCTAssertEqual(
            manifest["native_bridge_abi_version"] as? Int,
            Int(KagemushaRecursiveSpendProver.recursiveCompactRequiredNativeBridgeAbiVersion)
        )

        let archiveFixtureRef = try XCTUnwrap(manifest["archive_fixture"] as? [String: Any])
        XCTAssertEqual(Set(archiveFixtureRef.keys), Set(["path", "schema"]))
        XCTAssertEqual(
            archiveFixtureRef["path"] as? String,
            "fixtures/kagemusha_recursive_spend_abi7/archives.json"
        )
        XCTAssertEqual(
            archiveFixtureRef["schema"] as? String,
            "iroha.kagemusha.recursive_spend.abi7.archive_fixtures.v1"
        )

        let generator = try XCTUnwrap(manifest["generator"] as? [String: Any])
        XCTAssertEqual(Set(generator.keys), Set(["crate", "test", "print_env"]))
        XCTAssertEqual(generator["crate"] as? String, "iroha_python_rs")
        XCTAssertEqual(
            generator["test"] as? String,
            "kagemusha_recursive_spend_abi7_archive_fixture_matches_python_native_bridge"
        )
        XCTAssertEqual(
            generator["print_env"] as? String,
            "KAGEMUSHA_RECURSIVE_SPEND_PRINT_ABI7_ARCHIVES"
        )

        let domains = try XCTUnwrap(manifest["domains"] as? [String: Any])
        XCTAssertEqual(Set(domains.keys), Set(["lineage_accumulator", "fixture_label"]))
        XCTAssertEqual(
            domains["lineage_accumulator"] as? String,
            "iroha:kagemusha:v1:recursive-spend-accumulator"
        )
        XCTAssertEqual(domains["fixture_label"] as? String, "kagemusha-recursive-spend-python-real")

        let expectedOperations: [String: (operation: String, noritoType: String, archiveKind: String)] = [
            "append_bundle": ("append", "KagemushaRecursiveSpendBundleV1", "bundle"),
            "verify_request": ("verify", "KagemushaRecursiveSpendVerifyRequestV1", "request"),
            "verify_result": ("verify", "KagemushaRecursiveSpendVerifyResultV1", "result"),
            "redeem_request": ("redeem", "KagemushaRecursiveSpendRedeemRequestV1", "request"),
            "redeem_instruction": ("redeem", "RedeemKagemushaRecursive", "instruction")
        ]
        let operations = try XCTUnwrap(manifest["operations"] as? [[String: Any]])
        XCTAssertEqual(manifest["operation_count"] as? Int, expectedOperations.count)
        XCTAssertEqual(operations.count, expectedOperations.count)
        let operationsByName = Dictionary(
            uniqueKeysWithValues: operations.compactMap { operation -> (String, [String: Any])? in
                guard let name = operation["name"] as? String else {
                    return nil
                }
                return (name, operation)
            }
        )
        XCTAssertEqual(Set(operationsByName.keys), Set(expectedOperations.keys))
        for (name, expected) in expectedOperations {
            let operation = try XCTUnwrap(operationsByName[name])
            XCTAssertEqual(Set(operation.keys), Set(["name", "operation", "norito_type", "archive_kind"]))
            XCTAssertEqual(operation["operation"] as? String, expected.operation)
            XCTAssertEqual(operation["norito_type"] as? String, expected.noritoType)
            XCTAssertEqual(operation["archive_kind"] as? String, expected.archiveKind)
        }

        let archiveFixture = try Self.sharedRecursiveSpendAbi7Archives()
        XCTAssertEqual(
            Set(archiveFixture.keys),
            Set(["schema", "fixture_kind", "native_bridge_abi_version", "archives"])
        )
        XCTAssertEqual(archiveFixture["schema"] as? String, archiveFixtureRef["schema"] as? String)
        XCTAssertEqual(archiveFixture["fixture_kind"] as? String, "native_bridge_norito_archives")
        XCTAssertEqual(
            archiveFixture["native_bridge_abi_version"] as? Int,
            manifest["native_bridge_abi_version"] as? Int
        )
        let archives = try XCTUnwrap(archiveFixture["archives"] as? [[String: Any]])
        XCTAssertEqual(archives.count, expectedOperations.count)
        XCTAssertEqual(Set(archives.compactMap { $0["name"] as? String }), Set(expectedOperations.keys))
        for archive in archives {
            XCTAssertEqual(
                Set(archive.keys),
                Set(["name", "operation", "norito_type", "byte_len", "sha256_hex", "bytes_base64"])
            )
            let name = try XCTUnwrap(archive["name"] as? String)
            let expected = try XCTUnwrap(expectedOperations[name])
            XCTAssertEqual(archive["operation"] as? String, expected.operation)
            XCTAssertEqual(archive["norito_type"] as? String, expected.noritoType)
            let archiveBytes = try XCTUnwrap(Data(base64Encoded: try XCTUnwrap(archive["bytes_base64"] as? String)))
            XCTAssertEqual(archiveBytes.count, archive["byte_len"] as? Int)
            XCTAssertEqual(Self.hexString(SHA256.hash(data: archiveBytes)), archive["sha256_hex"] as? String)
        }
    }

    func testRedeemSpendBuildsAbi7FixtureInstructionWhenBridgeAvailable() throws {
        let archiveFixture = try Self.sharedRecursiveSpendAbi7Archives()
        XCTAssertEqual(
            archiveFixture["schema"] as? String,
            "iroha.kagemusha.recursive_spend.abi7.archive_fixtures.v1"
        )
        let archives = try XCTUnwrap(archiveFixture["archives"] as? [[String: Any]])
        let redeemRequest = try XCTUnwrap(archives.first { $0["name"] as? String == "redeem_request" })
        let redeemInstruction = try XCTUnwrap(archives.first { $0["name"] as? String == "redeem_instruction" })
        XCTAssertEqual(redeemRequest["norito_type"] as? String, "KagemushaRecursiveSpendRedeemRequestV1")
        XCTAssertEqual(redeemInstruction["norito_type"] as? String, "RedeemKagemushaRecursive")

        let requestArchive = try XCTUnwrap(Data(base64Encoded: try XCTUnwrap(redeemRequest["bytes_base64"] as? String)))
        let expectedInstructionArchive = try XCTUnwrap(
            Data(base64Encoded: try XCTUnwrap(redeemInstruction["bytes_base64"] as? String))
        )
        do {
            let output = try KagemushaRecursiveSpendProver.redeemSpend(requestArchive: requestArchive)
            XCTAssertEqual(output, expectedInstructionArchive)
        } catch KagemushaRecursiveSpendProverError.bridgeUnavailable {
            throw XCTSkip("Kagemusha recursive spend native bridge is unavailable.")
        }
    }

    func testExportsStableCircuitIds() {
        XCTAssertEqual(KagemushaRecursiveSpendProver.requiredNativeBridgeAbiVersion, 6)
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
            "kagemusha-recursive-aggregation-v1"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
            "kagemusha-recursive-spend-lineage-v1"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1,
            "kagemusha-recursive-spend-lineage-onehop-v1"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1,
            "kagemusha-recursive-spend-lineage-append-v1"
        )
        XCTAssertEqual(KagemushaRecursiveSpendProver.compactTokenMaxHops, 64)
        XCTAssertEqual(KagemushaRecursiveSpendProver.recursiveSpendLineageWitnesslessMaxHopsV1, 64)
        XCTAssertTrue(KagemushaRecursiveSpendProver.recursiveSpendLineageTransitionCircuitWiredV1)
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursivePreviousProofOpenEnvelopesRequiredCountV1,
            1
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursivePreviousProofOpenEnvelopesMaxBytes,
            8 * 1024 * 1024
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursivePallasOpenEnvelopeMaxTranscriptLabelBytes,
            128
        )
        XCTAssertEqual(KagemushaRecursiveSpendProver.nativeArchiveMaxBytes, 64 * 1024 * 1024)
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursiveSpendAccumulatorDomain,
            "iroha:kagemusha:v1:recursive-spend-accumulator"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursiveSpendTransitionProfileDomain,
            "iroha:kagemusha:v1:recursive-spend-transition-profile"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursiveSpendTransitionProfileDigestDomain,
            "iroha:kagemusha:v1:recursive-spend-transition-profile-digest"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursiveSpendTransitionProfileBindingDigestDomain,
            "iroha:kagemusha:v1:recursive-spend-transition-profile-binding-digest"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursiveSpendLineageAppendOpeningsPreflightDomainV1,
            "iroha:kagemusha:recursive-spend-lineage-append-openings-preflight:v1"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursiveSpendLineageAppendBoundaryDomainV1,
            "iroha:kagemusha:recursive-spend-lineage-append-boundary:v1"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursiveSpendLineageAppendBoundaryChainAssetBindingDomainV1,
            "iroha:kagemusha:recursive-spend-lineage-append-boundary-chain-asset:v1"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.recursiveSpendLineageAppendBoundaryFinalNoteBindingDomainV1,
            "iroha:kagemusha:recursive-spend-lineage-append-boundary-final-note:v1"
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.canRedeemWitnessless(
                circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                hopCount: 1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.canRedeemWitnessless(
                circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1,
                hopCount: 1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.canRedeemWitnessless(
                circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1,
                hopCount: 2
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.requiresLineageWitnessForRedeem(
                circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                hopCount: 1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.canRedeemWitnessless(
                circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                hopCount: KagemushaRecursiveSpendProver.recursiveSpendLineageWitnesslessMaxHopsV1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.requiresLineageWitnessForRedeem(
                circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                hopCount: KagemushaRecursiveSpendProver.recursiveSpendLineageWitnesslessMaxHopsV1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canRedeemWitnessless(
                circuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                hopCount: 1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canRedeemWitnessless(
                circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                hopCount: 0
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.requiresLineageWitnessForRedeem(
                circuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                hopCount: 2
            )
        )
        for (circuitId, hopCount) in [
            (KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1, UInt32.max),
            (KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1, 0),
            (KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1, UInt32.max),
            ("", 1),
            ("unknown-kagemusha-recursive-spend-circuit", UInt32.max),
        ] {
            XCTAssertFalse(
                KagemushaRecursiveSpendProver.canRedeemWitnessless(
                    circuitId: circuitId,
                    hopCount: hopCount
                )
            )
            XCTAssertTrue(
                KagemushaRecursiveSpendProver.requiresLineageWitnessForRedeem(
                    circuitId: circuitId,
                    hopCount: hopCount
                )
            )
        }
        XCTAssertFalse(KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(previousHopCount: 0))
        XCTAssertTrue(KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(previousHopCount: 1))
        XCTAssertTrue(KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(previousHopCount: 63))
        XCTAssertFalse(KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(previousHopCount: 64))
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(previousHopCount: UInt32.max)
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.normalizedAppendOutputCircuitId(nil),
            KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.normalizedAppendOutputCircuitId(""),
            KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.normalizedAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1
            ),
            KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.normalizedAppendOutputCircuitId(
                "unknown-kagemusha-recursive-spend-circuit"
            ),
            "unknown-kagemusha-recursive-spend-circuit"
        )
        let whitespaceLineageOutputCircuitId =
            " \(KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1) "
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.normalizedAppendOutputCircuitId(
                whitespaceLineageOutputCircuitId
            ),
            whitespaceLineageOutputCircuitId
        )
        XCTAssertTrue(KagemushaRecursiveSpendProver.isSupportedAppendOutputCircuitId(nil))
        XCTAssertTrue(KagemushaRecursiveSpendProver.isSupportedAppendOutputCircuitId(""))
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isSupportedAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isSupportedAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isSupportedAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.isSupportedAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isLineageProofCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isLineageProofCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isLineageProofCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.isLineageAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isLineageAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1
            )
        )
        XCTAssertTrue(KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForInit())
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput(
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput(
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput(
                outputCircuitId: nil
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput(
                outputCircuitId: ""
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput(
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput(
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput(
                outputCircuitId: "unknown-kagemusha-recursive-spend-circuit"
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.isSupportedAppendOutputCircuitId(
                "unknown-kagemusha-recursive-spend-circuit"
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.isSupportedAppendOutputCircuitId(
                whitespaceLineageOutputCircuitId
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(
                KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(
                "unknown-kagemusha-recursive-spend-circuit"
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(
                whitespaceLineageOutputCircuitId
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.requiresPreviousLineageVerifierRecordForAppend(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.requiresPreviousLineageVerifierRecordForAppend(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.requiresPreviousLineageVerifierRecordForAppend(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.requiresPreviousLineageVerifierRecordForAppend(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.requiresPreviousLineageVerifierRecordForAppend(
                previousProofCircuitId: "unknown-kagemusha-recursive-spend-circuit"
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isSupportedAppendProofTransition(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isSupportedAppendProofTransition(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                outputCircuitId: ""
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isSupportedAppendProofTransition(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.isSupportedAppendProofTransition(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.isSupportedAppendProofTransition(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1
            ),
            "semantic previous proofs cannot select Reserved-lineage output"
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.isSupportedAppendProofTransition(
                previousProofCircuitId: "unknown-kagemusha-recursive-spend-circuit",
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.isSupportedAppendProofTransition(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                outputCircuitId: "unknown-kagemusha-recursive-spend-circuit"
            )
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.preferredAppendOutputCircuitId(previousHopCount: 1),
            KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.preferredAppendOutputCircuitId(previousHopCount: 63),
            KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.preferredAppendOutputCircuitId(previousHopCount: 64),
            KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
            "preferred append selector falls back at the witnessless hop cap"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.preferredAppendOutputCircuitId(previousHopCount: 0),
            KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.preferredAppendOutputCircuitId(previousHopCount: UInt32.max),
            KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                previousHopCount: 1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(nil, previousHopCount: 1)
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                previousHopCount: KagemushaRecursiveSpendProver.compactTokenMaxHops - 1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                previousHopCount: 0
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                previousHopCount: KagemushaRecursiveSpendProver.compactTokenMaxHops
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                previousHopCount: UInt32.max
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                previousHopCount: 1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1,
                previousHopCount: 1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1,
                previousHopCount: 1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                previousHopCount: 63
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                previousHopCount: 64
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                previousHopCount: UInt32.max
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                "unknown-kagemusha-recursive-spend-circuit",
                previousHopCount: 1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                whitespaceLineageOutputCircuitId,
                previousHopCount: 1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                previousHopCount: 1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                previousHopCount: 1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
                previousProofCircuitId: "unknown-kagemusha-recursive-spend-circuit",
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                previousHopCount: 1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                previousHopCount: 1
            ),
            "semantic previous proofs cannot select Reserved-lineage output"
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                previousHopCount: 1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageOneHopProofCircuitIdV1,
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1,
                previousHopCount: 1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                outputCircuitId: "unknown-kagemusha-recursive-spend-circuit",
                previousHopCount: 1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                outputCircuitId: whitespaceLineageOutputCircuitId,
                previousHopCount: 1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
                previousProofCircuitId: whitespaceLineageOutputCircuitId,
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                previousHopCount: 1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                previousHopCount: 0
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
                previousProofCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                previousHopCount: UInt32.max
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                previousHopCount: 1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageAppendProofCircuitIdV1,
                previousHopCount: 1
            )
        )
        XCTAssertTrue(
            KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                previousHopCount: 64
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveSpendLineageProofCircuitIdV1,
                previousHopCount: 0
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
                outputCircuitId: KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1,
                previousHopCount: 1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
                outputCircuitId: nil,
                previousHopCount: 1
            )
        )
        XCTAssertFalse(
            KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
                outputCircuitId: "",
                previousHopCount: 1
            )
        )
    }

    func testRejectsEmptyRequestArchivesBeforeBridgeCall() {
        let validArchive = Self.validKagemushaNoritoArchive()
        let helpers: [(String, (Data) throws -> Data)] = [
            ("buildPallasOpenEnvelopesArchive", KagemushaRecursiveSpendProver.buildPallasOpenEnvelopesArchive),
            ("buildPreviousProofOpenEnvelopesArchive", KagemushaRecursiveSpendProver.buildPreviousProofOpenEnvelopesArchive),
            ("init", KagemushaRecursiveSpendProver.initSpend),
            ("append", KagemushaRecursiveSpendProver.appendSpend),
            ("transitionProfileInit", KagemushaRecursiveSpendProver.transitionProfileInit),
            ("transitionProfileAppend", KagemushaRecursiveSpendProver.transitionProfileAppend),
            ("lineageAppendBoundary", KagemushaRecursiveSpendProver.lineageAppendBoundary),
            ("verify", KagemushaRecursiveSpendProver.verifySpend),
            ("redeem", KagemushaRecursiveSpendProver.redeemSpend)
        ]

        for (label, helper) in helpers {
            XCTAssertThrowsError(try helper(Data()), "helper \(label) should reject empty archives") { error in
                XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .emptyRequestArchive)
            }
        }

        XCTAssertThrowsError(
            try KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                requestArchive: Data(),
                bundleArchive: validArchive
            )
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .emptyRequestArchive)
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                requestArchive: validArchive,
                bundleArchive: Data()
            )
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .emptyRequestArchive)
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                previousWitnessArchive: Data(),
                requestArchive: validArchive,
                bundleArchive: validArchive
            )
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .emptyRequestArchive)
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                previousWitnessArchive: validArchive,
                requestArchive: Data(),
                bundleArchive: validArchive
            )
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .emptyRequestArchive)
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                previousWitnessArchive: validArchive,
                requestArchive: validArchive,
                bundleArchive: Data()
            )
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .emptyRequestArchive)
        }
    }

    func testRejectsMalformedInputArchivesBeforeBridgeCall() {
        let validArchive = Self.validKagemushaNoritoArchive()
        let helpers: [(String, (Data) throws -> Data)] = [
            ("buildPallasOpenEnvelopesArchive", KagemushaRecursiveSpendProver.buildPallasOpenEnvelopesArchive),
            ("buildPreviousProofOpenEnvelopesArchive", KagemushaRecursiveSpendProver.buildPreviousProofOpenEnvelopesArchive),
            ("init", KagemushaRecursiveSpendProver.initSpend),
            ("append", KagemushaRecursiveSpendProver.appendSpend),
            ("transitionProfileInit", KagemushaRecursiveSpendProver.transitionProfileInit),
            ("transitionProfileAppend", KagemushaRecursiveSpendProver.transitionProfileAppend),
            ("lineageAppendBoundary", KagemushaRecursiveSpendProver.lineageAppendBoundary),
            ("verify", KagemushaRecursiveSpendProver.verifySpend),
            ("redeem", KagemushaRecursiveSpendProver.redeemSpend)
        ]

        for (archiveLabel, malformedArchive) in Self.malformedKagemushaNoritoArchives(validArchive) {
            for (label, helper) in helpers {
                assertRecursiveSpendInputError(
                    .invalidInputArchive,
                    "helper \(label) archive \(archiveLabel)"
                ) {
                    try helper(malformedArchive)
                }
            }

            assertRecursiveSpendInputError(
                .invalidInputArchive,
                "init witness request archive \(archiveLabel)"
            ) {
                try KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                    requestArchive: malformedArchive,
                    bundleArchive: validArchive
                )
            }
            assertRecursiveSpendInputError(
                .invalidInputArchive,
                "init witness bundle archive \(archiveLabel)"
            ) {
                try KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                    requestArchive: validArchive,
                    bundleArchive: malformedArchive
                )
            }
            assertRecursiveSpendInputError(
                .invalidInputArchive,
                "append witness previous witness archive \(archiveLabel)"
            ) {
                try KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                    previousWitnessArchive: malformedArchive,
                    requestArchive: validArchive,
                    bundleArchive: validArchive
                )
            }
            assertRecursiveSpendInputError(
                .invalidInputArchive,
                "append witness request archive \(archiveLabel)"
            ) {
                try KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                    previousWitnessArchive: validArchive,
                    requestArchive: malformedArchive,
                    bundleArchive: validArchive
                )
            }
            assertRecursiveSpendInputError(
                .invalidInputArchive,
                "append witness bundle archive \(archiveLabel)"
            ) {
                try KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                    previousWitnessArchive: validArchive,
                    requestArchive: validArchive,
                    bundleArchive: malformedArchive
                )
            }
        }
    }

    func testRejectsOversizedInputArchivesBeforeBridgeCall() {
        let validArchive = Self.validKagemushaNoritoArchive()
        let oversizedArchive = Data(
            repeating: 0x7f,
            count: KagemushaRecursiveSpendProver.nativeArchiveMaxBytes + 1
        )
        let oversizedMessage =
            "must not exceed \(KagemushaRecursiveSpendProver.nativeArchiveMaxBytes) bytes"
        let helpers: [(String, (Data) throws -> Data)] = [
            ("buildPallasOpenEnvelopesArchive", KagemushaRecursiveSpendProver.buildPallasOpenEnvelopesArchive),
            ("buildPreviousProofOpenEnvelopesArchive", KagemushaRecursiveSpendProver.buildPreviousProofOpenEnvelopesArchive),
            ("init", KagemushaRecursiveSpendProver.initSpend),
            ("append", KagemushaRecursiveSpendProver.appendSpend),
            ("transitionProfileInit", KagemushaRecursiveSpendProver.transitionProfileInit),
            ("transitionProfileAppend", KagemushaRecursiveSpendProver.transitionProfileAppend),
            ("lineageAppendBoundary", KagemushaRecursiveSpendProver.lineageAppendBoundary),
            ("verify", KagemushaRecursiveSpendProver.verifySpend),
            ("redeem", KagemushaRecursiveSpendProver.redeemSpend)
        ]

        for (label, helper) in helpers {
            assertRecursiveSpendInputError(
                .oversizedInputArchive,
                "helper \(label)",
                descriptionContains: oversizedMessage
            ) {
                try helper(oversizedArchive)
            }
        }

        assertRecursiveSpendInputError(
            .oversizedInputArchive,
            "init witness request",
            descriptionContains: oversizedMessage
        ) {
            try KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                requestArchive: oversizedArchive,
                bundleArchive: validArchive
            )
        }
        assertRecursiveSpendInputError(
            .oversizedInputArchive,
            "init witness bundle",
            descriptionContains: oversizedMessage
        ) {
            try KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                requestArchive: validArchive,
                bundleArchive: oversizedArchive
            )
        }
        assertRecursiveSpendInputError(
            .oversizedInputArchive,
            "append witness previous witness",
            descriptionContains: oversizedMessage
        ) {
            try KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                previousWitnessArchive: oversizedArchive,
                requestArchive: validArchive,
                bundleArchive: validArchive
            )
        }
        assertRecursiveSpendInputError(
            .oversizedInputArchive,
            "append witness request",
            descriptionContains: oversizedMessage
        ) {
            try KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                previousWitnessArchive: validArchive,
                requestArchive: oversizedArchive,
                bundleArchive: validArchive
            )
        }
        assertRecursiveSpendInputError(
            .oversizedInputArchive,
            "append witness bundle",
            descriptionContains: oversizedMessage
        ) {
            try KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                previousWitnessArchive: validArchive,
                requestArchive: validArchive,
                bundleArchive: oversizedArchive
            )
        }
    }

    func testRejectsEmptyPayloadInputArchivesBeforeBridgeCall() {
        let validArchive = Self.validKagemushaNoritoArchive()
        let emptyPayloadArchive = Self.emptyPayloadKagemushaNoritoArchive()
        let helpers: [(String, (Data) throws -> Data)] = [
            ("buildPallasOpenEnvelopesArchive", KagemushaRecursiveSpendProver.buildPallasOpenEnvelopesArchive),
            ("buildPreviousProofOpenEnvelopesArchive", KagemushaRecursiveSpendProver.buildPreviousProofOpenEnvelopesArchive),
            ("init", KagemushaRecursiveSpendProver.initSpend),
            ("append", KagemushaRecursiveSpendProver.appendSpend),
            ("transitionProfileInit", KagemushaRecursiveSpendProver.transitionProfileInit),
            ("transitionProfileAppend", KagemushaRecursiveSpendProver.transitionProfileAppend),
            ("lineageAppendBoundary", KagemushaRecursiveSpendProver.lineageAppendBoundary),
            ("verify", KagemushaRecursiveSpendProver.verifySpend),
            ("redeem", KagemushaRecursiveSpendProver.redeemSpend)
        ]

        for (label, helper) in helpers {
            assertRecursiveSpendInputError(.emptyInputPayload, "helper \(label)") {
                try helper(emptyPayloadArchive)
            }
        }

        assertRecursiveSpendInputError(.emptyInputPayload, "init witness request") {
            try KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                requestArchive: emptyPayloadArchive,
                bundleArchive: validArchive
            )
        }
        assertRecursiveSpendInputError(.emptyInputPayload, "init witness bundle") {
            try KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                requestArchive: validArchive,
                bundleArchive: emptyPayloadArchive
            )
        }
        assertRecursiveSpendInputError(.emptyInputPayload, "append witness previous witness") {
            try KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                previousWitnessArchive: emptyPayloadArchive,
                requestArchive: validArchive,
                bundleArchive: validArchive
            )
        }
        assertRecursiveSpendInputError(.emptyInputPayload, "append witness request") {
            try KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                previousWitnessArchive: validArchive,
                requestArchive: emptyPayloadArchive,
                bundleArchive: validArchive
            )
        }
        assertRecursiveSpendInputError(.emptyInputPayload, "append witness bundle") {
            try KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                previousWitnessArchive: validArchive,
                requestArchive: validArchive,
                bundleArchive: emptyPayloadArchive
            )
        }
    }

    func testRejectsEmptyNativeOutput() {
        let validArchive = Self.validKagemushaNoritoArchive()
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendProver.call(
                requestArchive: validArchive,
                bridgeAvailable: true
            ) { _ in
                Data()
            }
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .proofRejected)
        }
    }

    func testRejectsOversizedNativeOutput() {
        let validArchive = Self.validKagemushaNoritoArchive()
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendProver.call(
                requestArchive: validArchive,
                bridgeAvailable: true
            ) { _ in
                Data(
                    repeating: 0x7f,
                    count: KagemushaRecursiveSpendProver.nativeArchiveMaxBytes + 1
                )
            }
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .oversizedNativeOutput)
        }
    }

    func testRejectsMalformedNativeOutput() {
        let validArchive = Self.validKagemushaNoritoArchive()
        for (label, nativeOutput) in Self.malformedKagemushaNoritoArchives(validArchive) {
            XCTAssertThrowsError(
                try KagemushaRecursiveSpendProver.call(
                    requestArchive: validArchive,
                    bridgeAvailable: true
                ) { _ in
                    nativeOutput
                },
                "native output \(label) should be rejected"
            ) { error in
                XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .invalidNativeOutput)
            }
        }
    }

    func testRejectsEmptyPayloadNativeOutput() {
        let validArchive = Self.validKagemushaNoritoArchive()
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendProver.call(
                requestArchive: validArchive,
                bridgeAvailable: true
            ) { _ in
                Self.emptyPayloadKagemushaNoritoArchive()
            }
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .emptyNativeOutputPayload)
        }
    }

    func testReturnsValidNativeOutput() throws {
        let validArchive = Self.validKagemushaNoritoArchive()
        let output = try KagemushaRecursiveSpendProver.call(
            requestArchive: validArchive,
            bridgeAvailable: true
        ) { _ in
            validArchive
        }

        XCTAssertEqual(output, validArchive)
    }

    func testNilNativeOutputIsBridgeUnavailable() {
        let validArchive = Self.validKagemushaNoritoArchive()
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendProver.call(
                requestArchive: validArchive,
                bridgeAvailable: true
            ) { _ in
                nil
            }
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .bridgeUnavailable)
        }
    }

    func testNativeKagemushaRejectionMapsToProofRejected() {
        let validArchive = Self.validKagemushaNoritoArchive()
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendProver.call(
                requestArchive: validArchive,
                bridgeAvailable: true
            ) { _ in
                throw NativeBridgeError.kagemushaProve
            }
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .proofRejected)
        }
    }

    func testUnexpectedNativeRejectionMapsToProofRejected() {
        enum LocalError: Error {
            case rejected
        }

        let validArchive = Self.validKagemushaNoritoArchive()
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendProver.call(
                requestArchive: validArchive,
                bridgeAvailable: true
            ) { _ in
                throw LocalError.rejected
            }
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .proofRejected)
        }
    }

    func testNativeAvailabilityProbeRequiresMalformedArchiveRejection() {
        #if canImport(Darwin)
        XCTAssertTrue(
            NoritoNativeBridge.isExpectedKagemushaMalformedProbeResult(
                status: NoritoNativeBridge.expectedKagemushaProbeStatus,
                outPtr: nil,
                outLen: 0
            )
        )
        XCTAssertFalse(
            NoritoNativeBridge.isExpectedKagemushaMalformedProbeResult(
                status: 0,
                outPtr: nil,
                outLen: 0
            )
        )
        XCTAssertFalse(
            NoritoNativeBridge.isExpectedKagemushaMalformedProbeResult(
                status: -1,
                outPtr: nil,
                outLen: 0
            )
        )
        XCTAssertFalse(
            NoritoNativeBridge.isExpectedKagemushaMalformedProbeResult(
                status: NoritoNativeBridge.expectedKagemushaProbeStatus,
                outPtr: nil,
                outLen: 1
            )
        )
        let output = UnsafeMutablePointer<UInt8>.allocate(capacity: 1)
        defer { output.deallocate() }
        XCTAssertFalse(
            NoritoNativeBridge.isExpectedKagemushaMalformedProbeResult(
                status: NoritoNativeBridge.expectedKagemushaProbeStatus,
                outPtr: output,
                outLen: 0
            )
        )
        #endif
    }

    private static func sharedRecursiveSpendManifest() throws -> [String: Any] {
        try sharedRecursiveSpendFixture(named: "manifest.json")
    }

    private static func sharedRecursiveSpendArchives() throws -> [String: Any] {
        try sharedRecursiveSpendFixture(named: "archives.json")
    }

    private static func sharedRecursiveSpendAbi7Manifest() throws -> [String: Any] {
        try sharedRecursiveSpendFixture(named: "manifest.json", abiDirectoryName: "kagemusha_recursive_spend_abi7")
    }

    private static func sharedRecursiveSpendAbi7Archives() throws -> [String: Any] {
        try sharedRecursiveSpendFixture(named: "archives.json", abiDirectoryName: "kagemusha_recursive_spend_abi7")
    }

    private static func sharedRecursiveSpendFixture(
        named fileName: String,
        abiDirectoryName: String = "kagemusha_recursive_spend_abi6"
    ) throws -> [String: Any] {
        var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        for _ in 0..<10 {
            let candidate = directory
                .appendingPathComponent("fixtures")
                .appendingPathComponent(abiDirectoryName)
                .appendingPathComponent(fileName)
            if FileManager.default.fileExists(atPath: candidate.path) {
                let data = try Data(contentsOf: candidate)
                return try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
            }
            directory.deleteLastPathComponent()
        }
        throw NSError(
            domain: "KagemushaRecursiveSpendProverTests",
            code: 1,
            userInfo: [NSLocalizedDescriptionKey: "missing shared recursive spend fixture \(abiDirectoryName)/\(fileName)"]
        )
    }

    private func assertRecursiveSpendInputError(
        _ expected: KagemushaRecursiveSpendProverError,
        _ message: String,
        descriptionContains: String? = nil,
        file: StaticString = #filePath,
        line: UInt = #line,
        _ body: () throws -> Data
    ) {
        XCTAssertThrowsError(try body(), message, file: file, line: line) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, expected, file: file, line: line)
            if let descriptionContains {
                XCTAssertTrue(
                    error.localizedDescription.contains(descriptionContains),
                    file: file,
                    line: line
                )
            }
        }
    }

    private static func validKagemushaNoritoArchive() -> Data {
        noritoEncode(
            typeName: "KagemushaRecursiveSpendInputArchiveV1",
            payload: Data([0xa5, 0x5a, 0x11])
        )
    }

    private static func emptyPayloadKagemushaNoritoArchive() -> Data {
        noritoEncode(
            typeName: "KagemushaRecursiveSpendInputArchiveV1",
            payload: Data()
        )
    }

    private static func malformedKagemushaNoritoArchives(_ validArchive: Data) -> [(String, Data)] {
        var compressed = validArchive
        compressed[22] = 0x01
        var unsupportedFlags = validArchive
        unsupportedFlags[39] = NoritoHeader.varintOffsets
        var invalidFieldBitset = validArchive
        invalidFieldBitset[39] = NoritoHeader.fieldBitset
        return [
            ("truncated", Data([0x01])),
            ("compressed", compressed),
            ("unsupported flags", unsupportedFlags),
            ("invalid field bitset", invalidFieldBitset),
            (
                "nonzero header padding",
                Self.kagemushaNoritoFrameWithHeaderPadding(validArchive, padding: Data([0x7f]))
            ),
            (
                "excessive header padding",
                Self.kagemushaNoritoFrameWithHeaderPadding(
                    validArchive,
                    padding: Data(repeating: 0, count: 65)
                )
            ),
        ]
    }

    private static func kagemushaNoritoFrameWithHeaderPadding(_ archive: Data, padding: Data) -> Data {
        var padded = archive
        padded.insert(contentsOf: padding, at: NoritoHeader.encodedLength)
        return padded
    }

    private static func zk1Tlv(tag: String, payload: Data) -> Data {
        var encoded = Data(tag.utf8)
        appendUInt32LE(UInt32(payload.count), to: &encoded)
        encoded.append(payload)
        return encoded
    }

    private static func lineageVerifierKey(circuitId: String, seed: UInt8) -> Data {
        var verifierKey = Data([0x5A, 0x4B, 0x31, 0x00])
        verifierKey.append(zk1Tlv(tag: "IPAK", payload: Data([8, 0, 0, 0])))
        verifierKey.append(zk1Tlv(tag: "CID1", payload: Data(circuitId.utf8)))
        verifierKey.append(zk1Tlv(tag: "H2VK", payload: Data(repeating: seed, count: 32)))
        return verifierKey
    }

    private static let kagemushaLineageProvingKeyArchiveSchemaHash = Data([
        0xc8, 0x84, 0x89, 0x61, 0x8a, 0x01, 0x2c, 0x28,
        0x3f, 0xf3, 0xbb, 0x2e, 0xba, 0xbc, 0x77, 0x75,
    ])

    private static let oldKagemushaLineageProvingKeyArchiveSchemaHash = Data([
        0x11, 0x9f, 0x4d, 0xf3, 0x8a, 0x98, 0xef, 0x58,
        0x48, 0xad, 0x0a, 0xad, 0xb9, 0x71, 0x57, 0x79,
    ])

    private static func lineageProvingKeyArchive(
        circuitId: String,
        verifierKey: Data,
        seed: UInt8
    ) -> Data {
        lineageProvingKeyArchiveRaw(
            version: 1,
            circuitId: circuitId,
            verifierKeyCommitment: verifierKeyCommitment(verifierKey: verifierKey),
            provingKey: Data(repeating: seed, count: 64)
        )
    }

    private static func lineageProvingKeyArchiveRaw(
        version: UInt16,
        circuitId: String,
        verifierKeyCommitment: Data,
        provingKey: Data,
        flags: UInt8 = NoritoHeader.compactLen,
        schemaHash: Data = kagemushaLineageProvingKeyArchiveSchemaHash,
        trailingPayload: Data = Data()
    ) -> Data {
        var versionPayload = Data()
        appendUInt16LE(version, to: &versionPayload)
        var payload = Data()
        payload.append(noritoField(versionPayload, flags: flags))
        payload.append(noritoField(noritoString(circuitId, flags: flags), flags: flags))
        payload.append(noritoField(verifierKeyCommitment, flags: flags))
        payload.append(noritoField(noritoByteVec(provingKey), flags: flags))
        payload.append(trailingPayload)
        return noritoFrameFromSchemaHash(
            schemaHash,
            payload: payload,
            flags: flags
        )
    }

    private static func noritoFrameFromSchemaHash(
        _ schemaHash: Data,
        payload: Data,
        flags: UInt8
    ) -> Data {
        precondition(schemaHash.count == 16)
        var frame = Data()
        frame.append(NoritoHeader.magic)
        frame.append(NoritoHeader.versionMajor)
        frame.append(NoritoHeader.versionMinor)
        frame.append(schemaHash)
        frame.append(NoritoCompression.none.rawValue)
        appendUInt64LE(UInt64(payload.count), to: &frame)
        appendUInt64LE(crc64ECMA(payload), to: &frame)
        frame.append(flags)
        frame.append(payload)
        return frame
    }

    private static func noritoField(_ payload: Data, flags: UInt8) -> Data {
        var encoded = noritoLength(payload.count, flags: flags)
        encoded.append(payload)
        return encoded
    }

    private static func noritoString(_ value: String, flags: UInt8) -> Data {
        let bytes = Data(value.utf8)
        var encoded = noritoLength(bytes.count, flags: flags)
        encoded.append(bytes)
        return encoded
    }

    private static func noritoByteVec(_ bytes: Data) -> Data {
        var encoded = Data()
        appendUInt64LE(UInt64(bytes.count), to: &encoded)
        encoded.append(bytes)
        return encoded
    }

    private static func noritoLength(_ value: Int, flags: UInt8) -> Data {
        guard (flags & NoritoHeader.compactLen) != 0 else {
            var encoded = Data()
            appendUInt64LE(UInt64(value), to: &encoded)
            return encoded
        }
        var encoded = Data()
        var remaining = UInt64(value)
        while remaining >= 0x80 {
            encoded.append(UInt8(remaining & 0x7f) | 0x80)
            remaining >>= 7
        }
        encoded.append(UInt8(remaining))
        return encoded
    }

    private static func noritoOverlongCompactLength(_ value: Int) -> Data {
        precondition(value >= 0 && value < 0x80)
        return Data([UInt8(value) | 0x80, 0x00])
    }

    private static func noritoOversizedTerminalCompactLength() -> Data {
        Data(repeating: 0x80, count: 9) + Data([0x02])
    }

    private static func noritoHugeCanonicalCompactLength() -> Data {
        Data(repeating: 0x80, count: 9) + Data([0x01])
    }

    private static func verifierKeyCommitment(verifierKey: Data) -> Data {
        let backend = Data(KagemushaRecursiveSpendProver.recursiveAggregationProofBackend.utf8)
        var preimage = Data("iroha:zk:v1:vk".utf8)
        appendUInt64BE(UInt64(backend.count), to: &preimage)
        preimage.append(backend)
        appendUInt64BE(UInt64(verifierKey.count), to: &preimage)
        preimage.append(verifierKey)
        return Data(SHA256.hash(data: preimage))
    }

    private static func hexString<S: Sequence>(_ bytes: S) -> String where S.Element == UInt8 {
        bytes.map { String(format: "%02x", $0) }.joined()
    }

    private static func appendUInt32LE(_ value: UInt32, to data: inout Data) {
        var littleEndian = value.littleEndian
        withUnsafeBytes(of: &littleEndian) { bytes in
            data.append(contentsOf: bytes)
        }
    }

    private static func appendUInt16LE(_ value: UInt16, to data: inout Data) {
        var littleEndian = value.littleEndian
        withUnsafeBytes(of: &littleEndian) { bytes in
            data.append(contentsOf: bytes)
        }
    }

    private static func appendUInt64LE(_ value: UInt64, to data: inout Data) {
        var littleEndian = value.littleEndian
        withUnsafeBytes(of: &littleEndian) { bytes in
            data.append(contentsOf: bytes)
        }
    }

    private static func appendUInt64BE(_ value: UInt64, to data: inout Data) {
        var bigEndian = value.bigEndian
        withUnsafeBytes(of: &bigEndian) { bytes in
            data.append(contentsOf: bytes)
        }
    }

    private func assertInvalidLineageKeyArtifact(
        _ field: String,
        file: StaticString = #filePath,
        line: UInt = #line,
        _ body: () throws -> Void
    ) throws {
        XCTAssertThrowsError(try body(), file: file, line: line) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendProverError,
                .invalidLineageKeyArtifact(field),
                file: file,
                line: line
            )
        }
    }
}
