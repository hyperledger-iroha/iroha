import Foundation

/// One retained attempt to confirm. Native derives its route selector from these exact bytes.
public enum KagemushaWalletLedgerAttemptV1: Sendable {
    case activation(signedWire: Data)
    case unload(claim: Data, signedWire: Data)
    /// Exact retained suffix stage. Only certified Finalize establishes payout.
    case suffixUnloadStage(requestId: Data, ordinal: KagemushaWalletUInt128V1, signedWire: Data)
}

/// Native outcome after one confirmation pass.
public struct KagemushaWalletLedgerConfirmationV1: Equatable, Sendable {
    public let outcome: KagemushaWalletLedgerOutcomeV1
    /// True only when this pass retained a new authenticated outcome or selected at least one
    /// epoch boundary. Polls and unchanged outcomes report false.
    public let advanced: Bool
}

private struct KagemushaWalletLedgerWaitV1: Error {}

extension KagemushaWalletExchangeCallV1 {
    /// Confirm one retained Activate or Unload attempt with at most one finality read.
    ///
    /// A retained outcome returns without network. Otherwise one account-signed GET reads the
    /// attempt's direct original by the entrypoint Native derived, and a typed wait returns
    /// `awaiting`. An unselected epoch synchronizes at most `maximumBoundaries` boundaries
    /// through the attempt's epoch route, then offers the same in-memory original once more.
    /// The SDK persists no bytes, signs nothing, and never infers non-inclusion or rejection
    /// from a status, expiry or absence.
    public func confirmLedgerAttempt(
        _ attempt: KagemushaWalletLedgerAttemptV1,
        schemeID: Data, walletID: Data,
        transport: ToriiClient,
        canonicalAuth: ToriiCanonicalRequestAuth,
        maximumBoundaries: Int = 64,
        requireCurrentOwner: @escaping @Sendable () async throws -> Void
    ) async throws -> KagemushaWalletLedgerConfirmationV1 {
        return try await task.withUse {
            try await kagemushaWalletConfirmLedgerAttemptV1(
                requireCurrentOwner: requireCurrentOwner,
                outcome: { try self.ledgerAttemptOutcome(attempt) },
                fetch: { entrypoint in
                    try await transport.getKagemushaWalletLedgerAttemptFinalityV1(
                        selection: .init(schemeID: schemeID, walletID: walletID, entrypointHash: entrypoint),
                        canonicalAuth: canonicalAuth, requireCurrentOwner: requireCurrentOwner)
                },
                ingest: { finality in try self.ingestLedgerAttempt(attempt, finality: finality) },
                synchronize: { entrypoint, targetHeight in
                    try await kagemushaWalletSynchronizeLedgerAttemptV1(
                        selection: .init(schemeID: schemeID, walletID: walletID, entrypointHash: entrypoint),
                        targetHeight: targetHeight, transport: transport, canonicalAuth: canonicalAuth,
                        maximumBoundaries: maximumBoundaries, requireCurrentOwner: requireCurrentOwner,
                        progress: { try self.wallet.epochProgress() },
                        ingest: { epoch, bytes in try self.ingestEpochBoundary(expectedEpoch: epoch, original: bytes) })
                })
        }
    }

    func ledgerAttemptOutcome(_ attempt: KagemushaWalletLedgerAttemptV1) throws -> KagemushaWalletLedgerOutcomeV1 {
        switch attempt {
        case .activation(let signedWire):
            return try wallet.activationAttemptOutcome(signedWire: signedWire)
        case .unload(let claim, let signedWire):
            return try wallet.unloadAttemptOutcome(claim: claim, signedWire: signedWire)
        case .suffixUnloadStage(let requestId, let ordinal, let signedWire):
            return try wallet.suffixUnloadAttemptOutcome(requestId: requestId, ordinal: ordinal, signedWire: signedWire)
        }
    }

    func ingestLedgerAttempt(_ attempt: KagemushaWalletLedgerAttemptV1, finality: Data) throws -> KagemushaWalletLedgerIngestV1 {
        switch attempt {
        case .activation(let signedWire):
            return try ingestActivationFinality(signedWire: signedWire, finality: finality)
        case .unload(let claim, let signedWire):
            return try ingestUnloadFinality(claim: claim, signedWire: signedWire, finality: finality)
        case .suffixUnloadStage(let requestId, let ordinal, let signedWire):
            return try ingestSuffixUnloadFinality(requestId: requestId, ordinal: ordinal, signedWire: signedWire, finality: finality)
        }
    }
}

// The callbacks separate transport scheduling tests from cryptographic authority: production
// always supplies the installed wallet's Native outcome, ingestion and epoch entry points above.
func kagemushaWalletConfirmLedgerAttemptV1(
    requireCurrentOwner: () async throws -> Void,
    outcome: () throws -> KagemushaWalletLedgerOutcomeV1,
    fetch: (Data) async throws -> KagemushaWalletLedgerReadV1,
    ingest: (Data) throws -> KagemushaWalletLedgerIngestV1,
    synchronize: (Data, UInt64) async throws -> (covered: Bool, selected: Bool)
) async throws -> KagemushaWalletLedgerConfirmationV1 {
    try Task.checkCancellation()
    try await requireCurrentOwner()
    let retained = try outcome()
    guard case .awaiting(let entrypoint) = retained else {
        return .init(outcome: retained, advanced: false)
    }
    let read = try await fetch(entrypoint)
    try Task.checkCancellation()
    try await requireCurrentOwner()
    guard case .original(let finality) = read else {
        return .init(outcome: retained, advanced: false)
    }
    switch try ingest(finality) {
    case .outcome(let decided):
        return .init(outcome: decided, advanced: true)
    case .epochRequired(let targetHeight):
        let synchronized = try await synchronize(entrypoint, targetHeight)
        guard synchronized.covered else {
            return .init(outcome: retained, advanced: synchronized.selected)
        }
        try Task.checkCancellation()
        try await requireCurrentOwner()
        guard case .outcome(let decided) = try ingest(finality) else {
            throw KagemushaWalletErrorV1.invalidNativeOutput
        }
        return .init(outcome: decided, advanced: true)
    }
}

private func kagemushaWalletSynchronizeLedgerAttemptV1(
    selection: ToriiKagemushaWalletLedgerSelectionV1, targetHeight: UInt64,
    transport: ToriiClient, canonicalAuth: ToriiCanonicalRequestAuth, maximumBoundaries: Int,
    requireCurrentOwner: @escaping @Sendable () async throws -> Void,
    progress: () throws -> KagemushaWalletEpochProgressV1,
    ingest: (UInt64, Data) throws -> KagemushaWalletEpochProgressV1
) async throws -> (covered: Bool, selected: Bool) {
    let before = try progress()
    do {
        let after = try await kagemushaWalletSynchronizeEpochsV1(targetHeight: targetHeight,
            maximumBoundaries: maximumBoundaries, requireCurrentOwner: requireCurrentOwner, progress: progress,
            fetch: { height in
                let read = try await transport.getKagemushaWalletLedgerAttemptEpochV1(selection: selection,
                    boundaryHeight: height, canonicalAuth: canonicalAuth, requireCurrentOwner: requireCurrentOwner)
                guard case .original(let bytes) = read else { throw KagemushaWalletLedgerWaitV1() }
                return bytes
            }, ingest: ingest)
        return (targetHeight <= after.boundaryHeight, after.epoch != before.epoch)
    } catch is KagemushaWalletLedgerWaitV1 {
        return (false, try progress().epoch != before.epoch)
    }
}

extension KagemushaSpartanWalletV1 {
    /// Confirm a released Unload through its account-authenticated Torii route. Pending or unavailable
    /// reads preserve the local claim and its last known payout.
    public func confirmUnload(operationId: Data, signedTransaction: Data, schemeID: Data, walletID: Data,
        transport: ToriiClient, canonicalAuth: ToriiCanonicalRequestAuth, maximumBoundaries: Int = 64,
        requireCurrentOwner: @escaping @Sendable () async throws -> Void
    ) async throws -> KagemushaSpartanUnloadSettlementV1 {
        let result = try await confirmSpartanLedgerAttempt(schemeID: schemeID, walletID: walletID, transport: transport,
            canonicalAuth: canonicalAuth, maximumBoundaries: maximumBoundaries, requireCurrentOwner: requireCurrentOwner) { proof in
                try self.unloadSettlement(operationId: operationId, signedTransaction: signedTransaction, finality: proof)
            }
        return result
    }

    /// Verify this account-signed Activate using account-authenticated finality and the wallet's certified epochs.
    public func confirmActivation(signedTransaction: Data, schemeID: Data, walletID: Data,
        transport: ToriiClient, canonicalAuth: ToriiCanonicalRequestAuth, maximumBoundaries: Int = 64,
        requireCurrentOwner: @escaping @Sendable () async throws -> Void
    ) async throws -> KagemushaSpartanActivationSettlementV1 {
        try await confirmSpartanLedgerAttempt(schemeID: schemeID, walletID: walletID, transport: transport,
            canonicalAuth: canonicalAuth, maximumBoundaries: maximumBoundaries, requireCurrentOwner: requireCurrentOwner) { proof in
                try self.activationSettlement(signedTransaction: signedTransaction, finality: proof)
            }
    }

    private func confirmSpartanLedgerAttempt<Value: SpartanCertifiedAttemptV1>(
        schemeID: Data, walletID: Data, transport: ToriiClient, canonicalAuth: ToriiCanonicalRequestAuth, maximumBoundaries: Int,
        requireCurrentOwner: @escaping @Sendable () async throws -> Void, read: (Data) throws -> Value
    ) async throws -> Value {
        guard (1...64).contains(maximumBoundaries) else { throw KagemushaWalletErrorV1.invalidInput }
        func retained(_ proof: Data = Data()) throws -> Value { try read(proof) }
        func outcome(_ value: Value) -> KagemushaWalletLedgerOutcomeV1 {
            guard value.height != 0 else { return .awaiting(entrypoint: value.entrypoint) }
            let commit = KagemushaWalletLedgerCommitV1(height: value.height, commitHash: value.block)
            return value.accepted ? .confirmed(commit) : .rejected(commit)
        }
        func route(_ entrypoint: Data) throws -> ToriiKagemushaWalletLedgerSelectionV1 {
            try .init(schemeID: schemeID, walletID: walletID, entrypointHash: entrypoint)
        }
        _ = try await kagemushaWalletConfirmLedgerAttemptV1(requireCurrentOwner: requireCurrentOwner,
            outcome: { try outcome(retained()) },
            fetch: { try await transport.getKagemushaWalletLedgerAttemptFinalityV1(selection: route($0),
                canonicalAuth: canonicalAuth, requireCurrentOwner: requireCurrentOwner) },
            ingest: { proof in
                let seen = try retained(proof)
                if let height = seen.requiredHeight { return .epochRequired(targetHeight: height) }
                return .outcome(outcome(seen))
            },
            synchronize: { entry, target in
                try await kagemushaWalletSynchronizeLedgerAttemptV1(selection: route(entry), targetHeight: target,
                    transport: transport, canonicalAuth: canonicalAuth, maximumBoundaries: maximumBoundaries,
                    requireCurrentOwner: requireCurrentOwner, progress: { try self.epochProgress() },
                    ingest: { epoch, bytes in try self.ingestEpochBoundary(expectedEpoch: epoch, original: bytes) })
            })
        try Task.checkCancellation()
        try await requireCurrentOwner()
        return try retained()
    }
}


private protocol SpartanCertifiedAttemptV1 {
    var height: UInt64 { get }
    var accepted: Bool { get }
    var block: Data { get }
    var entrypoint: Data { get }
    var requiredHeight: UInt64? { get }
}
extension KagemushaSpartanUnloadSettlementV1: SpartanCertifiedAttemptV1 {}
extension KagemushaSpartanActivationSettlementV1: SpartanCertifiedAttemptV1 {}
