import CryptoKit
import Foundation

/// One one-shot source for a complete, published ABI-19 `KRV3KEY` artifact.
///
/// The source must invoke `stream` synchronously, in file order, and must not
/// retain its consumer. The coordinator independently verifies offsets, byte
/// count, and SHA-256 before allowing native finalization.
public struct KagemushaRecursiveSpendArtifactStream: Sendable {
    public let expectedSHA256: Data
    public let byteCount: UInt64

    private let streamBody: @Sendable (
        _ consume: (_ offset: UInt64, _ bytes: Data) throws -> Void
    ) throws -> Void

    public init(
        expectedSHA256: Data,
        byteCount: UInt64,
        stream: @escaping @Sendable (
            _ consume: (_ offset: UInt64, _ bytes: Data) throws -> Void
        ) throws -> Void
    ) throws {
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            expectedSHA256,
            field: "artifact.sha256"
        )
        guard byteCount > 0,
              byteCount <= UInt64(KagemushaRecursiveSpend.artifactMaximumFileBytes) else {
            throw KagemushaRecursiveSpendError.invalidField("artifact.byteCount")
        }
        self.expectedSHA256 = Data(expectedSHA256)
        self.byteCount = byteCount
        self.streamBody = stream
    }

    fileprivate func stream(
        _ consume: (_ offset: UInt64, _ bytes: Data) throws -> Void
    ) throws {
        try streamBody(consume)
    }
}

/// A generation-scoped capability for using the exact artifact set installed
/// by `KagemushaRecursiveSpendArtifactCoordinator`.
///
/// A lease becomes stale after a successful rotation, rollback, or explicit
/// uninstall. Keep every proof operation inside `withInstalledArtifactSet` so
/// another thread cannot rotate the process-wide native generation midway.
public final class KagemushaRecursiveSpendInstalledArtifactLease: @unchecked Sendable {
    public let binding: KagemushaRecursiveSpendArtifactBinding
    public let manifestSHA256: Data
    /// The six authenticated artifact digests, in lexicographic byte order.
    public let artifactSHA256s: [Data]

    private let token: UUID
    private let coordinator: KagemushaRecursiveSpendArtifactCoordinator

    fileprivate init(
        token: UUID,
        binding: KagemushaRecursiveSpendArtifactBinding,
        artifactSHA256s: [Data],
        coordinator: KagemushaRecursiveSpendArtifactCoordinator
    ) {
        self.token = token
        self.binding = binding
        self.manifestSHA256 = Data(binding.manifestSHA256)
        self.artifactSHA256s = artifactSHA256s.map { Data($0) }
        self.coordinator = coordinator
    }

    /// Serializes use with acquire, rotation, rollback, and uninstall.
    ///
    /// Do not call coordinator lifecycle methods recursively from `body`.
    @discardableResult
    public func withInstalledArtifactSet<T>(
        _ body: (KagemushaRecursiveSpendInstalledArtifactSet) throws -> T
    ) throws -> T {
        try coordinator.withInstalledArtifactSet(token: token, body)
    }
}

/// Process-wide owner for the single mode-free ABI-19 Kagemusha artifact set.
///
/// Candidate artifacts are completely streamed and authenticated before the
/// native atomic install. A failed candidate is cancelled without changing the
/// coordinator's prior generation. Reacquiring an older release performs an
/// explicit rollback and invalidates leases for the release it replaces.
public final class KagemushaRecursiveSpendArtifactCoordinator: @unchecked Sendable {
    public static let requiredArtifactCount = 6
    public static let shared = KagemushaRecursiveSpendArtifactCoordinator(
        sessionFactory: { manifest, binding in
            try KagemushaRecursiveSpendNativeArtifactInstallSessionDriver(
                manifest: manifest,
                binding: binding
            )
        }
    )

    private struct ArtifactIdentity: Equatable {
        let sha256: Data
        let byteCount: UInt64
    }

    private struct ActiveGeneration {
        let token: UUID
        let binding: KagemushaRecursiveSpendArtifactBinding
        let manifestSHA256: Data
        let artifacts: [ArtifactIdentity]
        let session: any KagemushaRecursiveSpendArtifactInstallSessionDriver
        let installedSet: KagemushaRecursiveSpendInstalledArtifactSet
    }

    // Recursive only so a lifecycle call made from one of the public callback
    // surfaces can fail immediately. A plain NSLock would deadlock the calling
    // thread before the coordinator could report the contract violation.
    private let lock = NSRecursiveLock()
    private let sessionFactory: KagemushaRecursiveSpendArtifactSessionFactory
    private var active: ActiveGeneration?
    private var publicCallbackActive = false
    private var publicCallbackViolation: KagemushaRecursiveSpendError?

    init(sessionFactory: @escaping KagemushaRecursiveSpendArtifactSessionFactory) {
        self.sessionFactory = sessionFactory
    }

    /// Acquire the exact authenticated six-file generation, reusing an
    /// identical active generation without consuming the supplied streams.
    ///
    /// Installation and reuse checks are serialized process-wide by `shared`.
    /// Applications should not create independent native installation owners.
    public func acquire(
        manifest: KagemushaRecursiveSpendArtifactManifestArchive,
        binding: KagemushaRecursiveSpendArtifactBinding,
        artifacts: [KagemushaRecursiveSpendArtifactStream]
    ) throws -> KagemushaRecursiveSpendInstalledArtifactLease {
        lock.lock()
        defer { lock.unlock() }
        try rejectReentrantLifecycleCall()

        guard binding.manifestSHA256 == manifest.sha256 else {
            throw KagemushaRecursiveSpendError.invalidField(
                "artifactBinding.manifestSHA256"
            )
        }
        let manifestGeneration = try KagemushaRecursiveSpendCodecs
            .decodeArtifactManifestGeneration(manifest.noritoArchive)
        guard Data(manifestGeneration.utf8) == Data(binding.generation.utf8) else {
            throw KagemushaRecursiveSpendError.invalidField(
                "artifactBinding.generation"
            )
        }
        guard artifacts.count == Self.requiredArtifactCount else {
            throw KagemushaRecursiveSpendError.invalidField("artifactSet.count")
        }
        guard Set(artifacts.map(\.expectedSHA256)).count
                == Self.requiredArtifactCount else {
            throw KagemushaRecursiveSpendError.invalidField("artifactSet.duplicate")
        }

        let ordered = artifacts.sorted {
            $0.expectedSHA256.lexicographicallyPrecedes($1.expectedSHA256)
        }
        let identity = ordered.map {
            ArtifactIdentity(sha256: $0.expectedSHA256, byteCount: $0.byteCount)
        }

        if let active,
           active.binding == binding,
           active.manifestSHA256 == manifest.sha256 {
            guard active.artifacts == identity else {
                throw KagemushaRecursiveSpendError.invalidField("artifactSet.identity")
            }
            if try active.session.isInstalled() {
                return lease(for: active)
            }
            // Native state was removed or replaced outside this owner. The old
            // token must never regain authority over a newly installed set.
            self.active = nil
        }

        return try installCandidate(
            manifest: manifest,
            binding: binding,
            artifacts: ordered,
            identity: identity
        )
    }

    /// Explicitly removes the coordinator's current native generation.
    /// Existing leases become stale only after removal succeeds (or native has
    /// already reported the generation absent).
    public func uninstallCurrent() throws {
        lock.lock()
        defer { lock.unlock() }
        try rejectReentrantLifecycleCall()
        guard let active else { return }
        if try active.session.isInstalled() {
            try active.session.uninstall()
        }
        self.active = nil
    }

    fileprivate func withInstalledArtifactSet<T>(
        token: UUID,
        _ body: (KagemushaRecursiveSpendInstalledArtifactSet) throws -> T
    ) throws -> T {
        lock.lock()
        defer { lock.unlock() }
        try rejectReentrantLifecycleCall()
        guard let active, active.token == token else {
            throw KagemushaRecursiveSpendError.invalidField("artifactLease.stale")
        }
        guard try active.session.isInstalled() else {
            self.active = nil
            throw KagemushaRecursiveSpendError.invalidField("artifactLease.stale")
        }
        return try withProtectedPublicCallback {
            try body(active.installedSet)
        }
    }

    private func installCandidate(
        manifest: KagemushaRecursiveSpendArtifactManifestArchive,
        binding: KagemushaRecursiveSpendArtifactBinding,
        artifacts: [KagemushaRecursiveSpendArtifactStream],
        identity: [ArtifactIdentity]
    ) throws -> KagemushaRecursiveSpendInstalledArtifactLease {
        let candidate = try sessionFactory(manifest, binding)
        guard candidate.manifest == manifest, candidate.binding == binding else {
            try? candidate.cancel()
            throw KagemushaRecursiveSpendError.invalidField("artifactSession.identity")
        }
        var installReturned = false
        do {
            for artifact in artifacts {
                let ingestion = try candidate.beginArtifact(
                    expectedArtifactSHA256: artifact.expectedSHA256
                )
                var expectedOffset: UInt64 = 0
                var hasher = SHA256()
                let streamLock = NSLock()
                var streamFailure: Error?
                try withProtectedPublicCallback {
                    try artifact.stream { offset, bytes in
                        streamLock.lock()
                        defer { streamLock.unlock() }
                        if let streamFailure {
                            throw streamFailure
                        }
                        do {
                            guard offset == expectedOffset,
                                  !bytes.isEmpty,
                                  expectedOffset <= artifact.byteCount,
                                  UInt64(bytes.count)
                                    <= artifact.byteCount - expectedOffset else {
                                throw KagemushaRecursiveSpendError.invalidField(
                                    "artifact.offset"
                                )
                            }
                            hasher.update(data: bytes)
                            try ingestion.write(bytes)
                            expectedOffset += UInt64(bytes.count)
                        } catch {
                            // A source must not be able to catch a rejected
                            // consumer call and resume the same stream.
                            streamFailure = error
                            throw error
                        }
                    }
                }
                if let streamFailure { throw streamFailure }
                guard expectedOffset == artifact.byteCount else {
                    throw KagemushaRecursiveSpendError.invalidField("artifact.byteCount")
                }
                guard Data(hasher.finalize()) == artifact.expectedSHA256 else {
                    throw KagemushaRecursiveSpendError.invalidField("artifact.digest")
                }
                try ingestion.finalize()
            }

            let installedSet = try candidate.install()
            installReturned = true
            guard installedSet.binding == binding,
                  installedSet.manifest == manifest else {
                throw KagemushaRecursiveSpendError.invalidField(
                    "artifactSession.installedIdentity"
                )
            }
            guard try candidate.isInstalled() else {
                throw KagemushaRecursiveSpendError.proofBackendUnavailable
            }
            let generation = ActiveGeneration(
                token: UUID(),
                binding: binding,
                manifestSHA256: Data(manifest.sha256),
                artifacts: identity,
                session: candidate,
                installedSet: installedSet
            )
            active = generation
            return lease(for: generation)
        } catch {
            // Native install is atomic. Digest-guarded uninstall is used only
            // if this candidate became active; otherwise pending streams are
            // cancelled and the prior generation remains usable.
            if installReturned {
                // Once native reports a successful atomic install the prior
                // generation has been replaced. If a postcondition then fails,
                // invalidate its app-layer token immediately rather than leave
                // a phantom prior generation for a later call to discover.
                active = nil
                try? candidate.uninstall()
            } else {
                // Do not use manifest-scoped `isInstalled()` to infer that a
                // pending candidate owns native state. The same exact manifest
                // may already be active outside a reconstructed coordinator;
                // digest-uninstalling it after a stream failure would destroy
                // a generation this candidate never installed.
                try? candidate.cancel()
            }
            throw error
        }
    }

    /// Execute one caller-controlled synchronous callback while making any
    /// attempted recursive lifecycle call terminal for the outer operation,
    /// even when the callback catches the immediate error.
    private func withProtectedPublicCallback<T>(
        _ body: () throws -> T
    ) throws -> T {
        precondition(!publicCallbackActive)
        publicCallbackActive = true
        publicCallbackViolation = nil
        do {
            let result = try body()
            let violation = publicCallbackViolation
            publicCallbackActive = false
            publicCallbackViolation = nil
            if let violation { throw violation }
            return result
        } catch {
            let violation = publicCallbackViolation
            publicCallbackActive = false
            publicCallbackViolation = nil
            throw violation ?? error
        }
    }

    private func rejectReentrantLifecycleCall() throws {
        guard publicCallbackActive else { return }
        let violation = KagemushaRecursiveSpendError.invalidField(
            "artifactCoordinator.reentrant"
        )
        if publicCallbackViolation == nil {
            publicCallbackViolation = violation
        }
        throw violation
    }

    private func lease(
        for generation: ActiveGeneration
    ) -> KagemushaRecursiveSpendInstalledArtifactLease {
        KagemushaRecursiveSpendInstalledArtifactLease(
            token: generation.token,
            binding: generation.binding,
            artifactSHA256s: generation.artifacts.map(\.sha256),
            coordinator: self
        )
    }
}

// Internal drivers keep native behavior behind a narrow seam so the lifecycle
// and concurrency contract can be tested without an installed XCFramework.
protocol KagemushaRecursiveSpendArtifactIngestDriver: AnyObject {
    func write(_ chunk: Data) throws
    func finalize() throws
    func cancel() throws
}

protocol KagemushaRecursiveSpendArtifactInstallSessionDriver: AnyObject {
    var manifest: KagemushaRecursiveSpendArtifactManifestArchive { get }
    var binding: KagemushaRecursiveSpendArtifactBinding { get }

    func beginArtifact(expectedArtifactSHA256: Data) throws
        -> any KagemushaRecursiveSpendArtifactIngestDriver
    func install() throws -> KagemushaRecursiveSpendInstalledArtifactSet
    func isInstalled() throws -> Bool
    func uninstall() throws
    func cancel() throws
}

typealias KagemushaRecursiveSpendArtifactSessionFactory = (
    _ manifest: KagemushaRecursiveSpendArtifactManifestArchive,
    _ binding: KagemushaRecursiveSpendArtifactBinding
) throws -> any KagemushaRecursiveSpendArtifactInstallSessionDriver

private final class KagemushaRecursiveSpendNativeArtifactIngestDriver:
    KagemushaRecursiveSpendArtifactIngestDriver {
    private let native: KagemushaRecursiveSpendArtifactIngest

    init(_ native: KagemushaRecursiveSpendArtifactIngest) {
        self.native = native
    }

    func write(_ chunk: Data) throws { try native.write(chunk) }
    func finalize() throws { try native.finalize() }
    func cancel() throws { try native.cancel() }
}

private final class KagemushaRecursiveSpendNativeArtifactInstallSessionDriver:
    KagemushaRecursiveSpendArtifactInstallSessionDriver {
    let manifest: KagemushaRecursiveSpendArtifactManifestArchive
    let binding: KagemushaRecursiveSpendArtifactBinding
    private let native: KagemushaRecursiveSpendArtifactInstallSessionV3

    init(
        manifest: KagemushaRecursiveSpendArtifactManifestArchive,
        binding: KagemushaRecursiveSpendArtifactBinding
    ) throws {
        self.manifest = manifest
        self.binding = binding
        self.native = try KagemushaRecursiveSpendArtifactInstallSessionV3(
            manifest: manifest,
            binding: binding
        )
    }

    func beginArtifact(expectedArtifactSHA256: Data) throws
        -> any KagemushaRecursiveSpendArtifactIngestDriver {
        KagemushaRecursiveSpendNativeArtifactIngestDriver(
            try native.beginArtifact(expectedArtifactSHA256: expectedArtifactSHA256)
        )
    }

    func install() throws -> KagemushaRecursiveSpendInstalledArtifactSet {
        try native.install()
    }

    func isInstalled() throws -> Bool { try native.isInstalled() }
    func uninstall() throws { try native.uninstall() }
    func cancel() throws { try native.cancel() }
}
