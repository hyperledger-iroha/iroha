import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaArtifactCoordinatorTests: XCTestCase {
    func testConcurrentIdenticalAcquireInstallsOnceAndReusesGeneration() throws {
        let world = FakeArtifactWorld()
        let coordinator = makeCoordinator(world: world)
        let manifest = try makeManifest(0x11)
        let binding = try makeBinding(0x11, manifest: manifest)
        let streamCalls = LockedCounter()
        let allStreams = try (0..<16).map { _ in
            try makeStreams(seed: 0x11) { streamCalls.increment() }
        }
        let resultLock = NSLock()
        var leases: [KagemushaRecursiveSpendInstalledArtifactLease] = []
        var errors: [Error] = []

        DispatchQueue.concurrentPerform(iterations: allStreams.count) { index in
            do {
                let lease = try coordinator.acquire(
                    manifest: manifest,
                    binding: binding,
                    artifacts: allStreams[index]
                )
                resultLock.lock()
                leases.append(lease)
                resultLock.unlock()
            } catch {
                resultLock.lock()
                errors.append(error)
                resultLock.unlock()
            }
        }

        XCTAssertTrue(errors.isEmpty)
        XCTAssertEqual(leases.count, allStreams.count)
        XCTAssertEqual(world.sessionCount, 1)
        XCTAssertEqual(world.installCount, 1)
        XCTAssertEqual(streamCalls.value, 6)
        for lease in leases {
            XCTAssertEqual(lease.binding, binding)
            XCTAssertEqual(lease.manifestSHA256, manifest.sha256)
            XCTAssertEqual(lease.artifactSHA256s.count, 6)
            let installedBinding = try lease.withInstalledArtifactSet(\.binding)
            XCTAssertEqual(installedBinding, binding)
        }
    }

    func testAcquireRejectsDigestLengthAndOffsetMismatches() throws {
        try assertRejectedStream(
            expectedField: "artifact.digest",
            mutate: { streams, bytes in
                let original = bytes[0]
                var corrupt = original
                corrupt[corrupt.startIndex] ^= 0xff
                let corruptBytes = corrupt
                streams[0] = try KagemushaRecursiveSpendArtifactStream(
                    expectedSHA256: Data(SHA256.hash(data: original)),
                    byteCount: UInt64(original.count)
                ) { consume in
                    try consume(0, corruptBytes)
                }
            }
        )

        try assertRejectedStream(
            expectedField: "artifact.byteCount",
            mutate: { streams, bytes in
                let original = bytes[0]
                streams[0] = try KagemushaRecursiveSpendArtifactStream(
                    expectedSHA256: Data(SHA256.hash(data: original)),
                    byteCount: UInt64(original.count + 1)
                ) { consume in
                    try consume(0, original)
                }
            }
        )

        try assertRejectedStream(
            expectedField: "artifact.offset",
            mutate: { streams, bytes in
                let original = bytes[0]
                streams[0] = try KagemushaRecursiveSpendArtifactStream(
                    expectedSHA256: Data(SHA256.hash(data: original)),
                    byteCount: UInt64(original.count)
                ) { consume in
                    try consume(1, original)
                }
            }
        )
    }

    func testSwallowedConsumerViolationStillCancelsCandidate() throws {
        let world = FakeArtifactWorld()
        let coordinator = makeCoordinator(world: world)
        let manifest = try makeManifest(0x12)
        let binding = try makeBinding(0x12, manifest: manifest)
        let bytes = makeArtifactBytes(seed: 0x12)
        var streams = try makeStreams(seed: 0x12)
        let original = bytes[0]
        streams[0] = try KagemushaRecursiveSpendArtifactStream(
            expectedSHA256: Data(SHA256.hash(data: original)),
            byteCount: UInt64(original.count)
        ) { consume in
            XCTAssertThrowsError(try consume(1, original))
            // A hostile source cannot recover by catching the consumer error.
            try? consume(0, original)
        }

        XCTAssertThrowsError(try coordinator.acquire(
            manifest: manifest,
            binding: binding,
            artifacts: streams
        )) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidField("artifact.offset")
            )
        }
        XCTAssertEqual(world.installCount, 0)
        XCTAssertEqual(world.cancelCount, 1)
    }

    func testReentrantLifecycleCallCannotDeadlockOrBeSwallowed() throws {
        let world = FakeArtifactWorld()
        let coordinator = makeCoordinator(world: world)
        let manifest = try makeManifest(0x13)
        let binding = try makeBinding(0x13, manifest: manifest)
        let lease = try coordinator.acquire(
            manifest: manifest,
            binding: binding,
            artifacts: makeStreams(seed: 0x13)
        )

        XCTAssertThrowsError(try lease.withInstalledArtifactSet { _ in
            XCTAssertThrowsError(try coordinator.uninstallCurrent()) { error in
                XCTAssertEqual(
                    error as? KagemushaRecursiveSpendError,
                    .invalidField("artifactCoordinator.reentrant")
                )
            }
        }) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidField("artifactCoordinator.reentrant")
            )
        }
        XCTAssertEqual(world.uninstallCount, 0)
        XCTAssertEqual(try lease.withInstalledArtifactSet(\.binding), binding)
    }

    func testStreamReentrantAcquireCannotBeSwallowed() throws {
        let world = FakeArtifactWorld()
        let coordinator = makeCoordinator(world: world)
        let manifest = try makeManifest(0x14)
        let binding = try makeBinding(0x14, manifest: manifest)
        let bytes = makeArtifactBytes(seed: 0x14)
        var streams = try makeStreams(seed: 0x14)
        let original = bytes[0]
        streams[0] = try KagemushaRecursiveSpendArtifactStream(
            expectedSHA256: Data(SHA256.hash(data: original)),
            byteCount: UInt64(original.count)
        ) { consume in
            XCTAssertThrowsError(try coordinator.acquire(
                manifest: manifest,
                binding: binding,
                artifacts: []
            ))
            try consume(0, original)
        }

        XCTAssertThrowsError(try coordinator.acquire(
            manifest: manifest,
            binding: binding,
            artifacts: streams
        )) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidField("artifactCoordinator.reentrant")
            )
        }
        XCTAssertEqual(world.installCount, 0)
        XCTAssertEqual(world.cancelCount, 1)
    }

    func testPartialStreamFailurePreservesPriorGeneration() throws {
        let world = FakeArtifactWorld()
        let coordinator = makeCoordinator(world: world)
        let manifestA = try makeManifest(0x21)
        let bindingA = try makeBinding(0x21, manifest: manifestA)
        let leaseA = try coordinator.acquire(
            manifest: manifestA,
            binding: bindingA,
            artifacts: makeStreams(seed: 0x21)
        )

        let manifestB = try makeManifest(0x22)
        let bindingB = try makeBinding(0x22, manifest: manifestB)
        let bytesB = makeArtifactBytes(seed: 0x22)
        var streamsB = try makeStreams(seed: 0x22)
        let interrupted = bytesB[2]
        streamsB[2] = try KagemushaRecursiveSpendArtifactStream(
            expectedSHA256: Data(SHA256.hash(data: interrupted)),
            byteCount: UInt64(interrupted.count)
        ) { consume in
            let split = interrupted.count / 2
            try consume(0, Data(interrupted.prefix(split)))
            throw StreamFailure.interrupted
        }

        XCTAssertThrowsError(try coordinator.acquire(
            manifest: manifestB,
            binding: bindingB,
            artifacts: streamsB
        )) { error in
            XCTAssertEqual(error as? StreamFailure, .interrupted)
        }
        XCTAssertEqual(world.sessionCount, 2)
        XCTAssertEqual(world.installCount, 1)
        XCTAssertEqual(world.cancelCount, 1)
        XCTAssertEqual(world.activeBinding, bindingA)
        XCTAssertEqual(
            try leaseA.withInstalledArtifactSet(\.binding),
            bindingA
        )
    }

    func testFailedCandidateDoesNotUninstallExternallyActiveSameManifest() throws {
        let world = FakeArtifactWorld()
        let manifest = try makeManifest(0x25)
        let binding = try makeBinding(0x25, manifest: manifest)
        let external = try XCTUnwrap(world.makeSession(
            manifest: manifest,
            binding: binding
        ) as? FakeArtifactSession)
        world.forceExternalInstall(external)
        let coordinator = KagemushaRecursiveSpendArtifactCoordinator { manifest, binding in
            let base = try XCTUnwrap(world.makeSession(
                manifest: manifest,
                binding: binding
            ) as? FakeArtifactSession)
            return DigestScopedStatusSession(base: base, world: world)
        }
        let bytes = makeArtifactBytes(seed: 0x25)
        var streams = try makeStreams(seed: 0x25)
        let interrupted = bytes[0]
        streams[0] = try KagemushaRecursiveSpendArtifactStream(
            expectedSHA256: Data(SHA256.hash(data: interrupted)),
            byteCount: UInt64(interrupted.count)
        ) { _ in
            throw StreamFailure.interrupted
        }

        XCTAssertThrowsError(try coordinator.acquire(
            manifest: manifest,
            binding: binding,
            artifacts: streams
        )) { error in
            XCTAssertEqual(error as? StreamFailure, .interrupted)
        }
        XCTAssertEqual(world.activeBinding, binding)
        XCTAssertEqual(world.uninstallCount, 0)
        XCTAssertEqual(world.cancelCount, 1)
    }

    func testNativeInstallFailurePreservesPriorGenerationAtomically() throws {
        let world = FakeArtifactWorld()
        let coordinator = makeCoordinator(world: world)
        let manifestA = try makeManifest(0x23)
        let bindingA = try makeBinding(0x23, manifest: manifestA)
        let leaseA = try coordinator.acquire(
            manifest: manifestA,
            binding: bindingA,
            artifacts: makeStreams(seed: 0x23)
        )
        let manifestB = try makeManifest(0x24)
        let bindingB = try makeBinding(0x24, manifest: manifestB)
        world.rejectNextInstall()

        XCTAssertThrowsError(try coordinator.acquire(
            manifest: manifestB,
            binding: bindingB,
            artifacts: makeStreams(seed: 0x24)
        )) { error in
            XCTAssertEqual(error as? StreamFailure, .installRejected)
        }
        XCTAssertEqual(world.activeBinding, bindingA)
        XCTAssertEqual(try leaseA.withInstalledArtifactSet(\.binding), bindingA)
        XCTAssertEqual(world.installCount, 1)
        XCTAssertEqual(world.cancelCount, 1)
    }

    func testRotationRejectsStaleLeaseAndRollbackRestoresOldGeneration() throws {
        let world = FakeArtifactWorld()
        let coordinator = makeCoordinator(world: world)
        let manifestA = try makeManifest(0x31)
        let bindingA = try makeBinding(0x31, manifest: manifestA)
        let originalA = try coordinator.acquire(
            manifest: manifestA,
            binding: bindingA,
            artifacts: makeStreams(seed: 0x31)
        )
        let manifestB = try makeManifest(0x32)
        let bindingB = try makeBinding(0x32, manifest: manifestB)
        let leaseB = try coordinator.acquire(
            manifest: manifestB,
            binding: bindingB,
            artifacts: makeStreams(seed: 0x32)
        )

        assertStale(originalA)
        XCTAssertEqual(try leaseB.withInstalledArtifactSet(\.binding), bindingB)

        let rolledBackA = try coordinator.acquire(
            manifest: manifestA,
            binding: bindingA,
            artifacts: makeStreams(seed: 0x31)
        )
        assertStale(originalA)
        assertStale(leaseB)
        XCTAssertEqual(
            try rolledBackA.withInstalledArtifactSet(\.binding),
            bindingA
        )
        XCTAssertEqual(world.sessionCount, 3)
        XCTAssertEqual(world.installCount, 3)
        XCTAssertEqual(world.activeBinding, bindingA)
    }

    func testExplicitUninstallInvalidatesLeaseAndAllowsReinstall() throws {
        let world = FakeArtifactWorld()
        let coordinator = makeCoordinator(world: world)
        let manifest = try makeManifest(0x41)
        let binding = try makeBinding(0x41, manifest: manifest)
        let lease = try coordinator.acquire(
            manifest: manifest,
            binding: binding,
            artifacts: makeStreams(seed: 0x41)
        )

        try coordinator.uninstallCurrent()
        assertStale(lease)
        XCTAssertNil(world.activeBinding)
        XCTAssertEqual(world.uninstallCount, 1)

        // Explicit cleanup is idempotent once this coordinator has no owner.
        try coordinator.uninstallCurrent()
        XCTAssertEqual(world.uninstallCount, 1)

        let replacement = try coordinator.acquire(
            manifest: manifest,
            binding: binding,
            artifacts: makeStreams(seed: 0x41)
        )
        XCTAssertEqual(try replacement.withInstalledArtifactSet(\.binding), binding)
        XCTAssertEqual(world.installCount, 2)
    }

    func testExternalNativeReplacementMakesLeaseStaleWithoutRemovingReplacement() throws {
        let world = FakeArtifactWorld()
        let coordinator = makeCoordinator(world: world)
        let manifestA = try makeManifest(0x42)
        let bindingA = try makeBinding(0x42, manifest: manifestA)
        let leaseA = try coordinator.acquire(
            manifest: manifestA,
            binding: bindingA,
            artifacts: makeStreams(seed: 0x42)
        )
        let manifestB = try makeManifest(0x43)
        let bindingB = try makeBinding(0x43, manifest: manifestB)
        let external = try XCTUnwrap(world.makeSession(
            manifest: manifestB,
            binding: bindingB
        ) as? FakeArtifactSession)
        world.forceExternalInstall(external)

        assertStale(leaseA)
        XCTAssertEqual(world.activeBinding, bindingB)
        // Clearing this coordinator after detecting the replacement must not
        // digest-uninstall a generation it does not own.
        try coordinator.uninstallCurrent()
        XCTAssertEqual(world.activeBinding, bindingB)
        XCTAssertEqual(world.uninstallCount, 0)
    }

    func testAcquireSerializesWithInstalledArtifactSetBody() throws {
        let world = FakeArtifactWorld()
        let coordinator = makeCoordinator(world: world)
        let manifestA = try makeManifest(0x51)
        let bindingA = try makeBinding(0x51, manifest: manifestA)
        let leaseA = try coordinator.acquire(
            manifest: manifestA,
            binding: bindingA,
            artifacts: makeStreams(seed: 0x51)
        )
        let manifestB = try makeManifest(0x52)
        let bindingB = try makeBinding(0x52, manifest: manifestB)
        let streamsB = try makeStreams(seed: 0x52)

        let bodyEntered = DispatchSemaphore(value: 0)
        let releaseBody = DispatchSemaphore(value: 0)
        let bodyFinished = DispatchSemaphore(value: 0)
        let acquireAttempted = DispatchSemaphore(value: 0)
        let acquireFinished = DispatchSemaphore(value: 0)
        let resultLock = NSLock()
        var bodyError: Error?
        var acquireResult: Result<KagemushaRecursiveSpendInstalledArtifactLease, Error>?

        DispatchQueue.global().async {
            defer { bodyFinished.signal() }
            do {
                try leaseA.withInstalledArtifactSet { _ in
                    bodyEntered.signal()
                    releaseBody.wait()
                }
            } catch {
                resultLock.lock()
                bodyError = error
                resultLock.unlock()
            }
        }
        XCTAssertEqual(bodyEntered.wait(timeout: .now() + 2), .success)

        DispatchQueue.global().async {
            acquireAttempted.signal()
            let result = Result {
                try coordinator.acquire(
                    manifest: manifestB,
                    binding: bindingB,
                    artifacts: streamsB
                )
            }
            resultLock.lock()
            acquireResult = result
            resultLock.unlock()
            acquireFinished.signal()
        }
        XCTAssertEqual(acquireAttempted.wait(timeout: .now() + 2), .success)
        XCTAssertEqual(acquireFinished.wait(timeout: .now() + 0.1), .timedOut)
        XCTAssertEqual(world.sessionCount, 1)
        XCTAssertEqual(world.activeBinding, bindingA)

        releaseBody.signal()
        XCTAssertEqual(bodyFinished.wait(timeout: .now() + 2), .success)
        XCTAssertEqual(acquireFinished.wait(timeout: .now() + 2), .success)

        resultLock.lock()
        let capturedBodyError = bodyError
        let capturedAcquireResult = acquireResult
        resultLock.unlock()
        XCTAssertNil(capturedBodyError)
        let leaseB = try XCTUnwrap(capturedAcquireResult).get()
        XCTAssertEqual(try leaseB.withInstalledArtifactSet(\.binding), bindingB)
        assertStale(leaseA)
    }

    func testAcquireRejectsInvalidInventoryBeforeCreatingNativeSession() throws {
        let world = FakeArtifactWorld()
        let coordinator = makeCoordinator(world: world)
        let manifest = try makeManifest(0x61)
        let binding = try makeBinding(0x61, manifest: manifest)
        let streams = try makeStreams(seed: 0x61)

        XCTAssertThrowsError(try coordinator.acquire(
            manifest: manifest,
            binding: binding,
            artifacts: Array(streams.dropLast())
        )) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidField("artifactSet.count")
            )
        }

        var duplicate = streams
        duplicate[5] = streams[0]
        XCTAssertThrowsError(try coordinator.acquire(
            manifest: manifest,
            binding: binding,
            artifacts: duplicate
        )) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidField("artifactSet.duplicate")
            )
        }
        XCTAssertEqual(world.sessionCount, 0)
    }

    func testAcquireRejectsManifestGenerationMismatchBeforeNativeSession() throws {
        let world = FakeArtifactWorld()
        let coordinator = makeCoordinator(world: world)
        let manifest = try makeManifest(0x62)
        let wrongBinding = try KagemushaRecursiveSpendArtifactBinding(
            generation: "different-generation",
            manifestSHA256: manifest.sha256
        )

        XCTAssertThrowsError(try coordinator.acquire(
            manifest: manifest,
            binding: wrongBinding,
            artifacts: makeStreams(seed: 0x62)
        )) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidField("artifactBinding.generation")
            )
        }
        XCTAssertEqual(world.sessionCount, 0)
    }

    func testCoordinatorRejectsSessionAndInstalledIdentitySubstitution() throws {
        let world = FakeArtifactWorld()
        let manifestA = try makeManifest(0x63)
        let bindingA = try makeBinding(0x63, manifest: manifestA)
        let manifestB = try makeManifest(0x64)
        let bindingB = try makeBinding(0x64, manifest: manifestB)
        let substitutedSession = KagemushaRecursiveSpendArtifactCoordinator { _, _ in
            world.makeSession(manifest: manifestB, binding: bindingB)
        }

        XCTAssertThrowsError(try substitutedSession.acquire(
            manifest: manifestA,
            binding: bindingA,
            artifacts: makeStreams(seed: 0x63)
        )) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidField("artifactSession.identity")
            )
        }
        XCTAssertEqual(world.cancelCount, 1)
        XCTAssertEqual(world.installCount, 0)

        let coordinator = makeCoordinator(world: world)
        world.substituteNextInstalledSet(binding: bindingB, manifest: manifestB)
        XCTAssertThrowsError(try coordinator.acquire(
            manifest: manifestA,
            binding: bindingA,
            artifacts: makeStreams(seed: 0x63)
        )) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidField("artifactSession.installedIdentity")
            )
        }
        XCTAssertNil(world.activeBinding)
        XCTAssertEqual(world.installCount, 1)
        XCTAssertEqual(world.uninstallCount, 1)
    }

    func testArtifactStreamRejectsInvalidDigestAndIntegerBounds() throws {
        let digest = Data(repeating: 0xA5, count: 32)
        for invalidDigest in [Data(), Data(repeating: 0, count: 32)] {
            XCTAssertThrowsError(try KagemushaRecursiveSpendArtifactStream(
                expectedSHA256: invalidDigest,
                byteCount: 1
            ) { _ in })
        }
        for invalidCount in [
            UInt64(0),
            UInt64(KagemushaRecursiveSpend.artifactMaximumFileBytes) + 1,
            UInt64.max,
        ] {
            XCTAssertThrowsError(try KagemushaRecursiveSpendArtifactStream(
                expectedSHA256: digest,
                byteCount: invalidCount
            ) { _ in }) { error in
                XCTAssertEqual(
                    error as? KagemushaRecursiveSpendError,
                    .invalidField("artifact.byteCount")
                )
            }
        }
    }

    func testSameBindingCannotReuseDifferentDeclaredArtifactIdentity() throws {
        let world = FakeArtifactWorld()
        let coordinator = makeCoordinator(world: world)
        let manifest = try makeManifest(0x71)
        let binding = try makeBinding(0x71, manifest: manifest)
        let streams = try makeStreams(seed: 0x71)
        let lease = try coordinator.acquire(
            manifest: manifest,
            binding: binding,
            artifacts: streams
        )
        var changed = try makeStreams(seed: 0x71)
        let bytes = makeArtifactBytes(seed: 0x71)[0]
        changed[0] = try KagemushaRecursiveSpendArtifactStream(
            expectedSHA256: Data(SHA256.hash(data: bytes)),
            byteCount: UInt64(bytes.count + 1)
        ) { consume in
            try consume(0, bytes)
        }

        XCTAssertThrowsError(try coordinator.acquire(
            manifest: manifest,
            binding: binding,
            artifacts: changed
        )) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidField("artifactSet.identity")
            )
        }
        XCTAssertEqual(world.sessionCount, 1)
        XCTAssertEqual(try lease.withInstalledArtifactSet(\.binding), binding)
    }

    private func assertRejectedStream(
        expectedField: String,
        mutate: (
            _ streams: inout [KagemushaRecursiveSpendArtifactStream],
            _ bytes: [Data]
        ) throws -> Void,
        file: StaticString = #filePath,
        line: UInt = #line
    ) throws {
        let world = FakeArtifactWorld()
        let coordinator = makeCoordinator(world: world)
        let manifest = try makeManifest(0x81)
        let binding = try makeBinding(0x81, manifest: manifest)
        let bytes = makeArtifactBytes(seed: 0x81)
        var streams = try makeStreams(seed: 0x81)
        try mutate(&streams, bytes)

        XCTAssertThrowsError(try coordinator.acquire(
            manifest: manifest,
            binding: binding,
            artifacts: streams
        ), file: file, line: line) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidField(expectedField),
                file: file,
                line: line
            )
        }
        XCTAssertEqual(world.sessionCount, 1, file: file, line: line)
        XCTAssertEqual(world.installCount, 0, file: file, line: line)
        XCTAssertEqual(world.cancelCount, 1, file: file, line: line)
        XCTAssertNil(world.activeBinding, file: file, line: line)
    }

    private func assertStale(
        _ lease: KagemushaRecursiveSpendInstalledArtifactLease,
        file: StaticString = #filePath,
        line: UInt = #line
    ) {
        XCTAssertThrowsError(try lease.withInstalledArtifactSet { _ in () }, file: file, line: line) {
            error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidField("artifactLease.stale"),
                file: file,
                line: line
            )
        }
    }

    private func makeCoordinator(
        world: FakeArtifactWorld
    ) -> KagemushaRecursiveSpendArtifactCoordinator {
        KagemushaRecursiveSpendArtifactCoordinator { manifest, binding in
            world.makeSession(manifest: manifest, binding: binding)
        }
    }

    private func makeManifest(
        _ seed: UInt8
    ) throws -> KagemushaRecursiveSpendArtifactManifestArchive {
        var payload = CompactNoritoWriter()
        payload.writeField(CompactNorito.encodeString(
            KagemushaRecursiveSpend.artifactManifestSchema
        ))
        payload.writeField(littleEndian(KagemushaRecursiveSpend.artifactManifestVersion))
        payload.writeField(littleEndian(
            KagemushaRecursiveSpend.requiredNativeBridgeAbiVersion
        ))
        payload.writeField(CompactNorito.encodeString(
            KagemushaRecursiveSpend.pastaCycleBackend
        ))
        payload.writeField(CompactNorito.encodeString(
            KagemushaRecursiveSpend.pastaCycleTranscript
        ))
        payload.writeField(CompactNorito.encodeString("coordinator-test-\(seed)"))
        // The fake native driver does not inspect the remaining authenticated
        // manifest fields; production native validation remains exhaustive.
        payload.writeField(Data([seed, seed &+ 1]))
        let archive = noritoEncode(
            typeName: KagemushaRecursiveSpend.artifactManifestWireName,
            payload: payload.data,
            flags: NoritoHeader.compactLen
        )
        return try KagemushaRecursiveSpendArtifactManifestArchive(
            noritoArchive: archive,
            expectedSHA256: Data(SHA256.hash(data: archive))
        )
    }

    private func littleEndian<T: FixedWidthInteger>(_ value: T) -> Data {
        var encoded = value.littleEndian
        return withUnsafeBytes(of: &encoded) { Data($0) }
    }

    private func makeBinding(
        _ seed: UInt8,
        manifest: KagemushaRecursiveSpendArtifactManifestArchive
    ) throws -> KagemushaRecursiveSpendArtifactBinding {
        try KagemushaRecursiveSpendArtifactBinding(
            generation: "coordinator-test-\(seed)",
            manifestSHA256: manifest.sha256
        )
    }

    private func makeArtifactBytes(seed: UInt8) -> [Data] {
        (0..<KagemushaRecursiveSpendArtifactCoordinator.requiredArtifactCount).map {
            Data("artifact-\(seed)-\($0)-authenticated-content".utf8)
        }
    }

    private func makeStreams(
        seed: UInt8,
        onStream: (@Sendable () -> Void)? = nil
    ) throws -> [KagemushaRecursiveSpendArtifactStream] {
        try makeArtifactBytes(seed: seed).map { bytes in
            try KagemushaRecursiveSpendArtifactStream(
                expectedSHA256: Data(SHA256.hash(data: bytes)),
                byteCount: UInt64(bytes.count)
            ) { consume in
                onStream?()
                let split = bytes.count / 2
                try consume(0, Data(bytes.prefix(split)))
                try consume(UInt64(split), Data(bytes.dropFirst(split)))
            }
        }
    }
}

private enum StreamFailure: Error, Equatable {
    case interrupted
    case installRejected
}

private final class LockedCounter: @unchecked Sendable {
    private let lock = NSLock()
    private var storage = 0

    var value: Int {
        lock.lock()
        defer { lock.unlock() }
        return storage
    }

    func increment() {
        lock.lock()
        storage += 1
        lock.unlock()
    }
}

private final class FakeArtifactWorld: @unchecked Sendable {
    private let lock = NSLock()
    private var sessions: [FakeArtifactSession] = []
    private weak var active: FakeArtifactSession?
    private var installs = 0
    private var cancels = 0
    private var uninstalls = 0
    private var rejectedInstalls = 0
    private var nextInstalledSet: KagemushaRecursiveSpendInstalledArtifactSet?

    var sessionCount: Int { synchronized { sessions.count } }
    var installCount: Int { synchronized { installs } }
    var cancelCount: Int { synchronized { cancels } }
    var uninstallCount: Int { synchronized { uninstalls } }
    var activeBinding: KagemushaRecursiveSpendArtifactBinding? {
        synchronized { active?.binding }
    }

    func makeSession(
        manifest: KagemushaRecursiveSpendArtifactManifestArchive,
        binding: KagemushaRecursiveSpendArtifactBinding
    ) -> any KagemushaRecursiveSpendArtifactInstallSessionDriver {
        let session = FakeArtifactSession(
            manifest: manifest,
            binding: binding,
            world: self
        )
        lock.lock()
        sessions.append(session)
        lock.unlock()
        return session
    }

    func install(_ session: FakeArtifactSession) throws {
        lock.lock()
        if rejectedInstalls > 0 {
            rejectedInstalls -= 1
            lock.unlock()
            throw StreamFailure.installRejected
        }
        active = session
        installs += 1
        lock.unlock()
    }

    func rejectNextInstall() {
        lock.lock()
        rejectedInstalls += 1
        lock.unlock()
    }

    func forceExternalInstall(_ session: FakeArtifactSession) {
        lock.lock()
        active = session
        lock.unlock()
    }

    func substituteNextInstalledSet(
        binding: KagemushaRecursiveSpendArtifactBinding,
        manifest: KagemushaRecursiveSpendArtifactManifestArchive
    ) {
        lock.lock()
        nextInstalledSet = try? KagemushaRecursiveSpendInstalledArtifactSet(
            binding: binding,
            manifest: manifest
        )
        lock.unlock()
    }

    func takeInstalledSet(
        fallbackBinding: KagemushaRecursiveSpendArtifactBinding,
        fallbackManifest: KagemushaRecursiveSpendArtifactManifestArchive
    ) throws -> KagemushaRecursiveSpendInstalledArtifactSet {
        lock.lock()
        let replacement = nextInstalledSet
        nextInstalledSet = nil
        lock.unlock()
        if let replacement { return replacement }
        return try KagemushaRecursiveSpendInstalledArtifactSet(
            binding: fallbackBinding,
            manifest: fallbackManifest
        )
    }

    func isInstalled(_ session: FakeArtifactSession) -> Bool {
        synchronized { active === session }
    }

    func cancel(_ session: FakeArtifactSession) {
        lock.lock()
        cancels += 1
        lock.unlock()
    }

    func uninstall(_ session: FakeArtifactSession) {
        lock.lock()
        if active === session {
            active = nil
            uninstalls += 1
        }
        lock.unlock()
    }

    func isManifestInstalled(
        _ binding: KagemushaRecursiveSpendArtifactBinding
    ) -> Bool {
        synchronized { active?.binding == binding }
    }

    func uninstallManifest(
        _ binding: KagemushaRecursiveSpendArtifactBinding
    ) {
        lock.lock()
        if active?.binding == binding {
            active = nil
            uninstalls += 1
        }
        lock.unlock()
    }

    private func synchronized<T>(_ body: () -> T) -> T {
        lock.lock()
        defer { lock.unlock() }
        return body()
    }
}

/// Models native status/uninstall semantics, which are scoped by exact
/// manifest identity rather than by the Swift session object's identity.
private final class DigestScopedStatusSession:
    KagemushaRecursiveSpendArtifactInstallSessionDriver {
    let manifest: KagemushaRecursiveSpendArtifactManifestArchive
    let binding: KagemushaRecursiveSpendArtifactBinding
    private let base: FakeArtifactSession
    private unowned let world: FakeArtifactWorld

    init(base: FakeArtifactSession, world: FakeArtifactWorld) {
        self.base = base
        self.world = world
        self.manifest = base.manifest
        self.binding = base.binding
    }

    func beginArtifact(expectedArtifactSHA256: Data) throws
        -> any KagemushaRecursiveSpendArtifactIngestDriver {
        try base.beginArtifact(expectedArtifactSHA256: expectedArtifactSHA256)
    }

    func install() throws -> KagemushaRecursiveSpendInstalledArtifactSet {
        try base.install()
    }

    func isInstalled() throws -> Bool { world.isManifestInstalled(binding) }
    func uninstall() throws { world.uninstallManifest(binding) }
    func cancel() throws { try base.cancel() }
}

private final class FakeArtifactSession:
    KagemushaRecursiveSpendArtifactInstallSessionDriver {
    let manifest: KagemushaRecursiveSpendArtifactManifestArchive
    let binding: KagemushaRecursiveSpendArtifactBinding
    private unowned let world: FakeArtifactWorld
    private var ingestions: [Data: FakeArtifactIngest] = [:]
    private var cancelled = false

    init(
        manifest: KagemushaRecursiveSpendArtifactManifestArchive,
        binding: KagemushaRecursiveSpendArtifactBinding,
        world: FakeArtifactWorld
    ) {
        self.manifest = manifest
        self.binding = binding
        self.world = world
    }

    func beginArtifact(expectedArtifactSHA256: Data) throws
        -> any KagemushaRecursiveSpendArtifactIngestDriver {
        guard !cancelled, ingestions[expectedArtifactSHA256] == nil else {
            throw KagemushaRecursiveSpendError.invalidField("fakeSession.state")
        }
        let ingest = FakeArtifactIngest()
        ingestions[Data(expectedArtifactSHA256)] = ingest
        return ingest
    }

    func install() throws -> KagemushaRecursiveSpendInstalledArtifactSet {
        guard ingestions.count
                == KagemushaRecursiveSpendArtifactCoordinator.requiredArtifactCount,
              ingestions.values.allSatisfy(\.isFinalized) else {
            throw KagemushaRecursiveSpendError.invalidField("fakeSession.install")
        }
        try world.install(self)
        return try world.takeInstalledSet(
            fallbackBinding: binding,
            fallbackManifest: manifest
        )
    }

    func isInstalled() throws -> Bool { world.isInstalled(self) }

    func uninstall() throws { world.uninstall(self) }

    func cancel() throws {
        guard !cancelled else { return }
        cancelled = true
        for ingest in ingestions.values {
            try ingest.cancel()
        }
        world.cancel(self)
    }
}

private final class FakeArtifactIngest:
    KagemushaRecursiveSpendArtifactIngestDriver {
    private(set) var isFinalized = false
    private var isCancelled = false
    private var bytes = Data()

    func write(_ chunk: Data) throws {
        guard !isFinalized, !isCancelled, !chunk.isEmpty else {
            throw KagemushaRecursiveSpendError.invalidField("fakeIngest.state")
        }
        bytes.append(chunk)
    }

    func finalize() throws {
        guard !isFinalized, !isCancelled, !bytes.isEmpty else {
            throw KagemushaRecursiveSpendError.invalidField("fakeIngest.finalize")
        }
        isFinalized = true
    }

    func cancel() throws {
        isCancelled = true
        bytes.removeAll()
    }
}
