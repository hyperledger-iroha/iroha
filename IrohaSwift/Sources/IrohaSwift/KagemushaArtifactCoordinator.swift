import CryptoKit
import Foundation

/// One one-shot source for a complete, published ABI-21 `KRV4KEY` artifact.
///
/// The source must invoke `stream` synchronously, in file order, and must not
/// retain its consumer. The coordinator independently verifies offsets, byte
/// count, and SHA-256 before allowing native finalization.
public struct KagemushaRecursiveSpendArtifactStream: Sendable {
    public let role: KagemushaRecursiveSpendArtifactRoleV4
    public let expectedSHA256: Data
    public let byteCount: UInt64

    private let streamBody: @Sendable (
        _ consume: (_ offset: UInt64, _ bytes: Data) throws -> Void
    ) throws -> Void

    public init(
        role: KagemushaRecursiveSpendArtifactRoleV4,
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
        self.role = role
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
    public let binding: KagemushaRecursiveSpendArtifactBindingV4
    public let manifestSHA256: Data
    /// The eight authenticated artifact digests, in canonical V4 role order.
    public let artifactSHA256s: [Data]

    private let token: UUID
    private let coordinator: KagemushaRecursiveSpendArtifactCoordinator

    fileprivate init(
        token: UUID,
        binding: KagemushaRecursiveSpendArtifactBindingV4,
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
        _ body: (KagemushaRecursiveSpendInstalledArtifactSetV4) throws -> T
    ) throws -> T {
        try coordinator.withInstalledArtifactSet(token: token, body)
    }
}

/// Process-wide owner for the single mode-free ABI-21 Kagemusha artifact set.
///
/// Candidate artifacts are completely streamed and authenticated before the
/// native atomic install. A failed candidate is cancelled without changing the
/// coordinator's prior generation. Reacquiring an older release performs an
/// explicit rollback and invalidates leases for the release it replaces.
public final class KagemushaRecursiveSpendArtifactCoordinator: @unchecked Sendable {
    public static let requiredArtifactCount =
        KagemushaRecursiveSpendArtifactRoleV4.allCases.count
    public static let shared = KagemushaRecursiveSpendArtifactCoordinator(
        sessionFactory: { _, _ in
            throw KagemushaRecursiveSpendError.invalidField(
                "release.authentication"
            )
        }
    )

    /// Create the process-wide installation owner from deployment-provisioned
    /// trust material. The unauthenticated `shared` sentinel cannot install.
    public static func authenticated(
        _ authentication: KagemushaRecursiveSpendReleaseAuthenticationV4
    ) -> KagemushaRecursiveSpendArtifactCoordinator {
        KagemushaRecursiveSpendArtifactCoordinator(
            sessionFactory: { manifest, binding in
                try KagemushaRecursiveSpendNativeArtifactInstallSessionDriver(
                    manifest: manifest,
                    binding: binding,
                    authentication: authentication
                )
            }
        )
    }

    private struct ArtifactIdentity: Equatable {
        let role: KagemushaRecursiveSpendArtifactRoleV4
        let sha256: Data
        let byteCount: UInt64
    }

    private struct ActiveGeneration {
        let token: UUID
        let binding: KagemushaRecursiveSpendArtifactBindingV4
        let manifestSHA256: Data
        let artifacts: [ArtifactIdentity]
        let session: any KagemushaRecursiveSpendArtifactInstallSessionDriver
        let installedSet: KagemushaRecursiveSpendInstalledArtifactSetV4
    }

    /// A completely streamed and finalized candidate whose atomic native
    /// install was deferred because another proof operation held the worker.
    private struct PendingCandidate {
        let manifest: KagemushaRecursiveSpendArtifactManifestArchive
        let binding: KagemushaRecursiveSpendArtifactBindingV4
        let artifacts: [ArtifactIdentity]
        let session: any KagemushaRecursiveSpendArtifactInstallSessionDriver
    }

    // One process-wide, non-blocking permit guards every expensive artifact
    // read and native lifecycle operation. The per-instance lock below only
    // protects small state snapshots and is never held across caller code or
    // native calls.
    private let stateLock = NSLock()
    private let sessionFactory: KagemushaRecursiveSpendArtifactSessionFactory
    private var active: ActiveGeneration?
    private var pending: PendingCandidate?
    private var publicCallbackActive = false
    private var publicCallbackViolation: KagemushaRecursiveSpendError?

    init(sessionFactory: @escaping KagemushaRecursiveSpendArtifactSessionFactory) {
        self.sessionFactory = sessionFactory
    }

    /// Acquire the exact authenticated eight-file generation, reusing an
    /// identical active generation without consuming the supplied streams.
    ///
    /// Installation and reuse checks are serialized process-wide by `shared`.
    /// Applications should not create independent native installation owners.
    public func acquire(
        manifest: KagemushaRecursiveSpendArtifactManifestArchive,
        binding: KagemushaRecursiveSpendArtifactBindingV4,
        artifacts: [KagemushaRecursiveSpendArtifactStream]
    ) throws -> KagemushaRecursiveSpendInstalledArtifactLease {
        try rejectReentrantLifecycleCall()
        return try KagemushaRecursiveSpendWorkerPermit.withPermit {
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
            guard artifacts.map(\.role)
                    == KagemushaRecursiveSpendArtifactRoleV4.allCases else {
                throw KagemushaRecursiveSpendError.invalidField("artifactSet.roleOrder")
            }
            let identity = artifacts.map {
                ArtifactIdentity(
                    role: $0.role,
                    sha256: $0.expectedSHA256,
                    byteCount: $0.byteCount
                )
            }

            if let pending = pendingSnapshot() {
                let sameRelease = pending.manifest == manifest
                    && pending.binding == binding
                if sameRelease, pending.artifacts == identity {
                    return try installFinalizedCandidate(
                        pending.session,
                        manifest: manifest,
                        binding: binding,
                        identity: identity
                    )
                }
                try cancelPendingCandidate()
                if sameRelease {
                    throw KagemushaRecursiveSpendError.invalidField("artifactSet.identity")
                }
            }

            if let active = activeSnapshot(),
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
                clearActive(ifToken: active.token)
            }

            return try installCandidate(
                manifest: manifest,
                binding: binding,
                artifacts: artifacts,
                identity: identity
            )
        }
    }

    /// Explicitly removes the coordinator's current native generation.
    /// Existing leases become stale only after removal succeeds (or native has
    /// already reported the generation absent).
    public func uninstallCurrent() throws {
        try rejectReentrantLifecycleCall()
        try KagemushaRecursiveSpendWorkerPermit.withPermit {
            try cancelPendingCandidate()
            guard let active = activeSnapshot() else { return }
            if try active.session.isInstalled() {
                try active.session.uninstall()
            }
            clearActive(ifToken: active.token)
        }
    }

    /// Cancel a fully finalized candidate retained after a busy native install.
    ///
    /// This does not alter the currently installed generation.
    public func cancelPendingInstallation() throws {
        try rejectReentrantLifecycleCall()
        try KagemushaRecursiveSpendWorkerPermit.withPermit {
            try cancelPendingCandidate()
        }
    }

    fileprivate func withInstalledArtifactSet<T>(
        token: UUID,
        _ body: (KagemushaRecursiveSpendInstalledArtifactSetV4) throws -> T
    ) throws -> T {
        try rejectReentrantLifecycleCall()
        return try KagemushaRecursiveSpendWorkerPermit.withPermit {
            guard let active = activeSnapshot(), active.token == token else {
                throw KagemushaRecursiveSpendError.invalidField("artifactLease.stale")
            }
            guard try active.session.isInstalled() else {
                clearActive(ifToken: active.token)
                throw KagemushaRecursiveSpendError.invalidField("artifactLease.stale")
            }
            return try withProtectedPublicCallback {
                try body(active.installedSet)
            }
        }
    }

    private func installCandidate(
        manifest: KagemushaRecursiveSpendArtifactManifestArchive,
        binding: KagemushaRecursiveSpendArtifactBindingV4,
        artifacts: [KagemushaRecursiveSpendArtifactStream],
        identity: [ArtifactIdentity]
    ) throws -> KagemushaRecursiveSpendInstalledArtifactLease {
        let candidate = try sessionFactory(manifest, binding)
        guard candidate.manifest == manifest, candidate.binding == binding else {
            try? candidate.cancel()
            throw KagemushaRecursiveSpendError.invalidField("artifactSession.identity")
        }
        do {
            for artifact in artifacts {
                let ingestion = try candidate.beginArtifact(
                    role: artifact.role,
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
                            guard !bytes.isEmpty,
                                  bytes.count <= KagemushaRecursiveSpend
                                    .artifactMaximumChunkBytes else {
                                throw KagemushaRecursiveSpendError.invalidField(
                                    "artifact.chunk"
                                )
                            }
                            guard offset == expectedOffset,
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
        } catch {
            try? candidate.cancel()
            throw error
        }
        return try installFinalizedCandidate(
            candidate,
            manifest: manifest,
            binding: binding,
            identity: identity
        )
    }

    private func installFinalizedCandidate(
        _ candidate: any KagemushaRecursiveSpendArtifactInstallSessionDriver,
        manifest: KagemushaRecursiveSpendArtifactManifestArchive,
        binding: KagemushaRecursiveSpendArtifactBindingV4,
        identity: [ArtifactIdentity]
    ) throws -> KagemushaRecursiveSpendInstalledArtifactLease {
        var installReturned = false
        do {
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
            stateLock.lock()
            pending = nil
            active = generation
            stateLock.unlock()
            return lease(for: generation)
        } catch KagemushaRecursiveSpendError.proofWorkerBusy where !installReturned {
            // Busy is the one native failure that leaves every finalized spool
            // valid. Retain this exact identity so a matching acquire retries
            // only the atomic install and never invokes its streams again.
            let retained = PendingCandidate(
                manifest: manifest,
                binding: binding,
                artifacts: identity,
                session: candidate
            )
            stateLock.lock()
            pending = retained
            stateLock.unlock()
            throw KagemushaRecursiveSpendError.proofWorkerBusy
        } catch {
            stateLock.lock()
            pending = nil
            if installReturned {
                active = nil
            }
            stateLock.unlock()
            // Native install is atomic. Digest-guarded uninstall is used only
            // if this candidate became active; otherwise its finalized streams
            // are cancelled and the prior generation remains usable.
            if installReturned {
                // Once native reports a successful atomic install the prior
                // generation has been replaced. If a postcondition then fails,
                // invalidate its app-layer token immediately rather than leave
                // a phantom prior generation for a later call to discover.
                try? candidate.uninstall()
            } else {
                // Do not use manifest-scoped `isInstalled()` to infer that a
                // pending candidate owns native state. The same exact manifest
                // may already be active outside a reconstructed coordinator;
                // digest-uninstalling it after a failed install would destroy
                // a generation this candidate never installed.
                try? candidate.cancel()
            }
            throw error
        }
    }

    private func cancelPendingCandidate() throws {
        stateLock.lock()
        let pending = self.pending
        self.pending = nil
        stateLock.unlock()
        guard let pending else { return }
        // Session cancellation is terminal even when one native handle reports
        // an error: the production driver closes the coordinator and drains all
        // remaining handles before returning its first failure.
        try pending.session.cancel()
    }

    private func activeSnapshot() -> ActiveGeneration? {
        stateLock.lock()
        defer { stateLock.unlock() }
        return active
    }

    private func pendingSnapshot() -> PendingCandidate? {
        stateLock.lock()
        defer { stateLock.unlock() }
        return pending
    }

    private func clearActive(ifToken token: UUID) {
        stateLock.lock()
        defer { stateLock.unlock() }
        if active?.token == token {
            active = nil
        }
    }

    /// Execute one caller-controlled synchronous callback while making any
    /// attempted recursive lifecycle call terminal for the outer operation,
    /// even when the callback catches the immediate error.
    private func withProtectedPublicCallback<T>(
        _ body: () throws -> T
    ) throws -> T {
        stateLock.lock()
        precondition(!publicCallbackActive)
        publicCallbackActive = true
        publicCallbackViolation = nil
        stateLock.unlock()
        do {
            let result = try body()
            stateLock.lock()
            let violation = publicCallbackViolation
            publicCallbackActive = false
            publicCallbackViolation = nil
            stateLock.unlock()
            if let violation { throw violation }
            return result
        } catch {
            stateLock.lock()
            let violation = publicCallbackViolation
            publicCallbackActive = false
            publicCallbackViolation = nil
            stateLock.unlock()
            throw violation ?? error
        }
    }

    private func rejectReentrantLifecycleCall() throws {
        stateLock.lock()
        defer { stateLock.unlock() }
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
    var binding: KagemushaRecursiveSpendArtifactBindingV4 { get }

    func beginArtifact(
        role: KagemushaRecursiveSpendArtifactRoleV4,
        expectedArtifactSHA256: Data
    ) throws
        -> any KagemushaRecursiveSpendArtifactIngestDriver
    func install() throws -> KagemushaRecursiveSpendInstalledArtifactSetV4
    func isInstalled() throws -> Bool
    func uninstall() throws
    func cancel() throws
}

typealias KagemushaRecursiveSpendArtifactSessionFactory = (
    _ manifest: KagemushaRecursiveSpendArtifactManifestArchive,
    _ binding: KagemushaRecursiveSpendArtifactBindingV4
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
    let binding: KagemushaRecursiveSpendArtifactBindingV4
    private let native: KagemushaRecursiveSpendArtifactInstallSessionV4

    init(
        manifest: KagemushaRecursiveSpendArtifactManifestArchive,
        binding: KagemushaRecursiveSpendArtifactBindingV4,
        authentication: KagemushaRecursiveSpendReleaseAuthenticationV4
    ) throws {
        self.manifest = manifest
        self.binding = binding
        self.native = try KagemushaRecursiveSpendArtifactInstallSessionV4(
            manifest: manifest,
            binding: binding,
            authentication: authentication
        )
    }

    func beginArtifact(
        role: KagemushaRecursiveSpendArtifactRoleV4,
        expectedArtifactSHA256: Data
    ) throws
        -> any KagemushaRecursiveSpendArtifactIngestDriver {
        KagemushaRecursiveSpendNativeArtifactIngestDriver(
            try native.beginArtifact(
                role: role,
                expectedArtifactSHA256: expectedArtifactSHA256
            )
        )
    }

    func install() throws -> KagemushaRecursiveSpendInstalledArtifactSetV4 {
        try native.install()
    }

    func isInstalled() throws -> Bool { try native.isInstalled() }
    func uninstall() throws { try native.uninstall() }
    func cancel() throws { try native.cancel() }
}
