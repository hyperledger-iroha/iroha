import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaArtifactCoordinatorTests: XCTestCase {
    func testProcessWidePermitRejectsContenderBeforeItReadsStreams() throws {
        let worldA = FakeArtifactWorld()
        let worldB = FakeArtifactWorld()
        let coordinatorA = makeCoordinator(world: worldA)
        let coordinatorB = makeCoordinator(world: worldB)
        let manifestA = try makeManifest(0x11)
        let bindingA = try makeBinding(0x11, manifest: manifestA)
        let manifestB = try makeManifest(0x12)
        let bindingB = try makeBinding(0x12, manifest: manifestB)
        let enteredStream = DispatchSemaphore(value: 0)
        let releaseStream = DispatchSemaphore(value: 0)
        let finished = DispatchSemaphore(value: 0)
        let resultLock = NSLock()
        var primaryResult: Result<KagemushaRecursiveSpendInstalledArtifactLease, Error>?
        var streamsA = try makeStreams(seed: 0x11)
        let firstBytes = makeArtifactBytes(seed: 0x11)[0]
        streamsA[0] = try KagemushaRecursiveSpendArtifactStream(
            role: .stepEqParamsIpa,
            expectedSHA256: Data(SHA256.hash(data: firstBytes)),
            byteCount: UInt64(firstBytes.count)
        ) { consume in
            enteredStream.signal()
            releaseStream.wait()
            try consume(0, firstBytes)
        }

        DispatchQueue.global().async {
            let result = Result {
                try coordinatorA.acquire(
                    manifest: manifestA,
                    binding: bindingA,
                    artifacts: streamsA
                )
            }
            resultLock.lock()
            primaryResult = result
            resultLock.unlock()
            finished.signal()
        }
        XCTAssertEqual(enteredStream.wait(timeout: .now() + 2), .success)

        let contenderReads = LockedCounter()
        XCTAssertThrowsError(try coordinatorB.acquire(
            manifest: manifestB,
            binding: bindingB,
            artifacts: makeStreams(seed: 0x12) { contenderReads.increment() }
        )) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendError, .proofWorkerBusy)
        }
        XCTAssertEqual(contenderReads.value, 0)
        XCTAssertEqual(worldB.sessionCount, 0)

        releaseStream.signal()
        XCTAssertEqual(finished.wait(timeout: .now() + 2), .success)
        resultLock.lock()
        let completed = primaryResult
        resultLock.unlock()
        XCTAssertEqual(try XCTUnwrap(completed).get().binding, bindingA)

        let leaseB = try coordinatorB.acquire(
            manifest: manifestB,
            binding: bindingB,
            artifacts: makeStreams(seed: 0x12)
        )
        XCTAssertEqual(leaseB.binding, bindingB)
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
                    role: streams[0].role,
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
                    role: streams[0].role,
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
                    role: streams[0].role,
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
            role: streams[0].role,
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
            role: streams[0].role,
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
            role: streamsB[2].role,
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
            role: streams[0].role,
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

    func testBusyInstallRetriesFinalizedSessionWithoutRestreamOrCancellation() throws {
        let world = FakeArtifactWorld()
        let coordinator = makeCoordinator(world: world)
        let manifest = try makeManifest(0x26)
        let binding = try makeBinding(0x26, manifest: manifest)
        let streamCalls = LockedCounter()
        world.makeNextInstallBusy()

        XCTAssertThrowsError(try coordinator.acquire(
            manifest: manifest,
            binding: binding,
            artifacts: makeStreams(seed: 0x26) { streamCalls.increment() }
        )) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .proofWorkerBusy
            )
        }
        XCTAssertEqual(
            streamCalls.value,
            KagemushaRecursiveSpendArtifactCoordinator.requiredArtifactCount
        )
        XCTAssertEqual(world.sessionCount, 1)
        XCTAssertEqual(world.installAttemptCount, 1)
        XCTAssertEqual(world.installCount, 0)
        XCTAssertEqual(world.cancelCount, 0)

        let lease = try coordinator.acquire(
            manifest: manifest,
            binding: binding,
            artifacts: makeStreams(seed: 0x26) { streamCalls.increment() }
        )
        XCTAssertEqual(
            streamCalls.value,
            KagemushaRecursiveSpendArtifactCoordinator.requiredArtifactCount
        )
        XCTAssertEqual(world.sessionCount, 1)
        XCTAssertEqual(world.installAttemptCount, 2)
        XCTAssertEqual(world.installCount, 1)
        XCTAssertEqual(world.cancelCount, 0)
        XCTAssertEqual(try lease.withInstalledArtifactSet(\.binding), binding)
    }

    func testDifferentAcquireCancelAndUninstallDiscardBusyPendingSession() throws {
        let world = FakeArtifactWorld()
        let coordinator = makeCoordinator(world: world)
        let manifestA = try makeManifest(0x27)
        let bindingA = try makeBinding(0x27, manifest: manifestA)
        world.makeNextInstallBusy()
        XCTAssertThrowsError(try coordinator.acquire(
            manifest: manifestA,
            binding: bindingA,
            artifacts: makeStreams(seed: 0x27)
        )) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendError, .proofWorkerBusy)
        }

        let manifestB = try makeManifest(0x28)
        let bindingB = try makeBinding(0x28, manifest: manifestB)
        let leaseB = try coordinator.acquire(
            manifest: manifestB,
            binding: bindingB,
            artifacts: makeStreams(seed: 0x28)
        )
        XCTAssertEqual(world.cancelCount, 1)
        XCTAssertEqual(world.activeBinding, bindingB)

        let manifestC = try makeManifest(0x29)
        let bindingC = try makeBinding(0x29, manifest: manifestC)
        world.makeNextInstallBusy()
        XCTAssertThrowsError(try coordinator.acquire(
            manifest: manifestC,
            binding: bindingC,
            artifacts: makeStreams(seed: 0x29)
        )) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendError, .proofWorkerBusy)
        }
        try coordinator.cancelPendingInstallation()
        XCTAssertEqual(world.cancelCount, 2)
        XCTAssertEqual(try leaseB.withInstalledArtifactSet(\.binding), bindingB)

        let manifestD = try makeManifest(0x2A)
        let bindingD = try makeBinding(0x2A, manifest: manifestD)
        world.makeNextInstallBusy()
        XCTAssertThrowsError(try coordinator.acquire(
            manifest: manifestD,
            binding: bindingD,
            artifacts: makeStreams(seed: 0x2A)
        )) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendError, .proofWorkerBusy)
        }
        try coordinator.uninstallCurrent()
        XCTAssertEqual(world.cancelCount, 3)
        XCTAssertEqual(world.uninstallCount, 1)
        XCTAssertNil(world.activeBinding)
        assertStale(leaseB)
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

    func testAcquireReturnsBusyWhileInstalledArtifactSetBodyOwnsPermit() throws {
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
        let resultLock = NSLock()
        var bodyError: Error?

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

        XCTAssertThrowsError(try coordinator.acquire(
            manifest: manifestB,
            binding: bindingB,
            artifacts: streamsB
        )) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendError, .proofWorkerBusy)
        }
        XCTAssertEqual(world.sessionCount, 1)
        XCTAssertEqual(world.activeBinding, bindingA)

        releaseBody.signal()
        XCTAssertEqual(bodyFinished.wait(timeout: .now() + 2), .success)

        resultLock.lock()
        let capturedBodyError = bodyError
        resultLock.unlock()
        XCTAssertNil(capturedBodyError)
        let leaseB = try coordinator.acquire(
            manifest: manifestB,
            binding: bindingB,
            artifacts: streamsB
        )
        XCTAssertEqual(try leaseB.withInstalledArtifactSet(\.binding), bindingB)
        assertStale(leaseA)
    }

    func testAcquireRejectsInvalidInventoryBeforeCreatingNativeSession() throws {
        let world = FakeArtifactWorld()
        let coordinator = makeCoordinator(world: world)
        let manifest = try makeManifest(0x61)
        let binding = try makeBinding(0x61, manifest: manifest)
        let streams = try makeStreams(seed: 0x61)

        let invalidCounts = [
            Array(streams.prefix(7)),
            streams + [streams[0]],
            streams + [streams[0], streams[1]],
        ]
        for invalidInventory in invalidCounts {
            XCTAssertThrowsError(try coordinator.acquire(
                manifest: manifest,
                binding: binding,
                artifacts: invalidInventory
            )) { error in
                XCTAssertEqual(
                    error as? KagemushaRecursiveSpendError,
                    .invalidField("artifactSet.count")
                )
            }
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

        var duplicateRole = streams
        let secondBytes = makeArtifactBytes(seed: 0x61)[1]
        duplicateRole[1] = try KagemushaRecursiveSpendArtifactStream(
            role: streams[0].role,
            expectedSHA256: Data(SHA256.hash(data: secondBytes)),
            byteCount: UInt64(secondBytes.count)
        ) { consume in
            try consume(0, secondBytes)
        }
        XCTAssertThrowsError(try coordinator.acquire(
            manifest: manifest,
            binding: binding,
            artifacts: duplicateRole
        )) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidField("artifactSet.roleOrder")
            )
        }

        var reordered = streams
        reordered.swapAt(0, 1)
        XCTAssertThrowsError(try coordinator.acquire(
            manifest: manifest,
            binding: binding,
            artifacts: reordered
        )) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidField("artifactSet.roleOrder")
            )
        }
        XCTAssertEqual(world.sessionCount, 0)
    }

    func testAcquireRejectsManifestGenerationMismatchBeforeNativeSession() throws {
        let world = FakeArtifactWorld()
        let coordinator = makeCoordinator(world: world)
        let manifest = try makeManifest(0x62)
        let wrongBinding = try KagemushaRecursiveSpendArtifactBindingV4(
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
                role: .stepEqParamsIpa,
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
                role: .stepEqParamsIpa,
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

    func testAcquireRejectsChunkLargerThanOneMiB() throws {
        let world = FakeArtifactWorld()
        let coordinator = makeCoordinator(world: world)
        let manifest = try makeManifest(0x70)
        let binding = try makeBinding(0x70, manifest: manifest)
        var streams = try makeStreams(seed: 0x70)
        let oversized = Data(
            repeating: 0x70,
            count: KagemushaRecursiveSpend.artifactMaximumChunkBytes + 1
        )
        streams[0] = try KagemushaRecursiveSpendArtifactStream(
            role: .stepEqParamsIpa,
            expectedSHA256: Data(SHA256.hash(data: oversized)),
            byteCount: UInt64(oversized.count)
        ) { consume in
            try consume(0, oversized)
        }

        XCTAssertThrowsError(try coordinator.acquire(
            manifest: manifest,
            binding: binding,
            artifacts: streams
        )) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidField("artifact.chunk")
            )
        }
        XCTAssertEqual(world.installCount, 0)
        XCTAssertEqual(world.cancelCount, 1)
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
            role: changed[0].role,
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
            KagemushaRecursiveSpend.artifactManifestSchemaV4
        ))
        payload.writeField(littleEndian(KagemushaRecursiveSpend.artifactManifestVersionV4))
        payload.writeField(littleEndian(
            KagemushaRecursiveSpend.requiredNativeBridgeAbiVersion
        ))
        payload.writeField(CompactNorito.encodeString(
            KagemushaRecursiveSpend.pastaCycleBackendV4
        ))
        payload.writeField(CompactNorito.encodeString(
            KagemushaRecursiveSpend.pastaCycleTranscriptV4
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
    ) throws -> KagemushaRecursiveSpendArtifactBindingV4 {
        try KagemushaRecursiveSpendArtifactBindingV4(
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
        try zip(
            KagemushaRecursiveSpendArtifactRoleV4.allCases,
            makeArtifactBytes(seed: seed)
        ).map { role, bytes in
            try KagemushaRecursiveSpendArtifactStream(
                role: role,
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
    private var installAttempts = 0
    private var installs = 0
    private var cancels = 0
    private var uninstalls = 0
    private var rejectedInstalls = 0
    private var busyInstalls = 0
    private var nextInstalledSet: KagemushaRecursiveSpendInstalledArtifactSetV4?

    var sessionCount: Int { synchronized { sessions.count } }
    var installAttemptCount: Int { synchronized { installAttempts } }
    var installCount: Int { synchronized { installs } }
    var cancelCount: Int { synchronized { cancels } }
    var uninstallCount: Int { synchronized { uninstalls } }
    var activeBinding: KagemushaRecursiveSpendArtifactBindingV4? {
        synchronized { active?.binding }
    }

    func makeSession(
        manifest: KagemushaRecursiveSpendArtifactManifestArchive,
        binding: KagemushaRecursiveSpendArtifactBindingV4
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
        installAttempts += 1
        if busyInstalls > 0 {
            busyInstalls -= 1
            lock.unlock()
            throw KagemushaRecursiveSpendError.proofWorkerBusy
        }
        if rejectedInstalls > 0 {
            rejectedInstalls -= 1
            lock.unlock()
            throw StreamFailure.installRejected
        }
        active = session
        installs += 1
        lock.unlock()
    }

    func makeNextInstallBusy() {
        lock.lock()
        busyInstalls += 1
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
        binding: KagemushaRecursiveSpendArtifactBindingV4,
        manifest: KagemushaRecursiveSpendArtifactManifestArchive
    ) {
        lock.lock()
        nextInstalledSet = try? KagemushaRecursiveSpendInstalledArtifactSetV4(
            binding: binding,
            manifest: manifest
        )
        lock.unlock()
    }

    func takeInstalledSet(
        fallbackBinding: KagemushaRecursiveSpendArtifactBindingV4,
        fallbackManifest: KagemushaRecursiveSpendArtifactManifestArchive
    ) throws -> KagemushaRecursiveSpendInstalledArtifactSetV4 {
        lock.lock()
        let replacement = nextInstalledSet
        nextInstalledSet = nil
        lock.unlock()
        if let replacement { return replacement }
        return try KagemushaRecursiveSpendInstalledArtifactSetV4(
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
        _ binding: KagemushaRecursiveSpendArtifactBindingV4
    ) -> Bool {
        synchronized { active?.binding == binding }
    }

    func uninstallManifest(
        _ binding: KagemushaRecursiveSpendArtifactBindingV4
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
    let binding: KagemushaRecursiveSpendArtifactBindingV4
    private let base: FakeArtifactSession
    private unowned let world: FakeArtifactWorld

    init(base: FakeArtifactSession, world: FakeArtifactWorld) {
        self.base = base
        self.world = world
        self.manifest = base.manifest
        self.binding = base.binding
    }

    func beginArtifact(
        role: KagemushaRecursiveSpendArtifactRoleV4,
        expectedArtifactSHA256: Data
    ) throws
        -> any KagemushaRecursiveSpendArtifactIngestDriver {
        try base.beginArtifact(
            role: role,
            expectedArtifactSHA256: expectedArtifactSHA256
        )
    }

    func install() throws -> KagemushaRecursiveSpendInstalledArtifactSetV4 {
        try base.install()
    }

    func isInstalled() throws -> Bool { world.isManifestInstalled(binding) }
    func uninstall() throws { world.uninstallManifest(binding) }
    func cancel() throws { try base.cancel() }
}

private final class FakeArtifactSession:
    KagemushaRecursiveSpendArtifactInstallSessionDriver {
    let manifest: KagemushaRecursiveSpendArtifactManifestArchive
    let binding: KagemushaRecursiveSpendArtifactBindingV4
    private unowned let world: FakeArtifactWorld
    private var ingestions: [KagemushaRecursiveSpendArtifactRoleV4:
        (digest: Data, ingest: FakeArtifactIngest)] = [:]
    private var cancelled = false
    private var installed = false

    init(
        manifest: KagemushaRecursiveSpendArtifactManifestArchive,
        binding: KagemushaRecursiveSpendArtifactBindingV4,
        world: FakeArtifactWorld
    ) {
        self.manifest = manifest
        self.binding = binding
        self.world = world
    }

    func beginArtifact(
        role: KagemushaRecursiveSpendArtifactRoleV4,
        expectedArtifactSHA256: Data
    ) throws
        -> any KagemushaRecursiveSpendArtifactIngestDriver {
        guard !cancelled, !installed,
              ingestions[role] == nil,
              !ingestions.values.contains(where: {
                  $0.digest == expectedArtifactSHA256
              }) else {
            throw KagemushaRecursiveSpendError.invalidField("fakeSession.state")
        }
        let ingest = FakeArtifactIngest()
        ingestions[role] = (Data(expectedArtifactSHA256), ingest)
        return ingest
    }

    func install() throws -> KagemushaRecursiveSpendInstalledArtifactSetV4 {
        guard !cancelled, !installed,
              ingestions.count
                == KagemushaRecursiveSpendArtifactCoordinator.requiredArtifactCount,
              ingestions.values.allSatisfy({ $0.ingest.isFinalized }) else {
            throw KagemushaRecursiveSpendError.invalidField("fakeSession.install")
        }
        try world.install(self)
        installed = true
        return try world.takeInstalledSet(
            fallbackBinding: binding,
            fallbackManifest: manifest
        )
    }

    func isInstalled() throws -> Bool { installed && world.isInstalled(self) }

    func uninstall() throws {
        guard installed else { return }
        world.uninstall(self)
        installed = false
    }

    func cancel() throws {
        guard !cancelled, !installed else { return }
        cancelled = true
        for ingest in ingestions.values {
            try ingest.ingest.cancel()
        }
        ingestions.removeAll()
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
        isFinalized = false
        bytes.removeAll()
    }
}
