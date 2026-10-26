"""Run the maintained reader's recovery timer without a radio or monetary Native.

Only the platform session and progress observer are stand-ins. The retry policy,
ownership gates, restart method, cancellation and timers are production Swift.
"""

from pathlib import Path
import subprocess
import tempfile
import unittest

from test_nfc_callback_ownership import (
    CORE_NFC_SOURCE,
    PROGRESS_SOURCE,
    REPOSITORY_ROOT,
    SWIFTC,
    extract_swift_declaration,
)


@unittest.skipUnless(SWIFTC, "requires Swift")
class NfcReaderRecoveryTests(unittest.TestCase):
    def test_recovery_starts_immediately_survives_three_seconds_and_cancels(self):
        progress = (REPOSITORY_ROOT / PROGRESS_SOURCE).read_text()
        core = (REPOSITORY_ROOT / CORE_NFC_SOURCE).read_text()
        declarations = "\n".join(
            extract_swift_declaration(progress, name)
            for name in (
                "public struct IrohaPeerNfcReaderRetryPolicyV1:",
                "final class IrohaPeerNfcReaderPlatformCallGateV1:",
                "final class IrohaPeerNfcRetryGateV1:",
                "struct IrohaPeerNfcReaderOperationGateV1:",
            )
        )
        restart = extract_swift_declaration(core, "private func restartPollingWithDeadline(")
        source = "import Foundation\n" + declarations + r"""
final class NFCTagReaderSession {
    var polls = 0
    func restartPolling() { polls += 1 }
}
enum IrohaPeerNfcCoreNFCErrorV1: Error { case retryExhausted }
enum Stage { case contactLost }
final class Reporter {
    var lost = 0
    func emit(_ stage: Stage) { lost += 1 }
}
final class ReaderCut: @unchecked Sendable {
    let lock = NSLock()
    var completed = false
    let session = NFCTagReaderSession()
    var operationGate = IrohaPeerNfcReaderOperationGateV1()
    var retryGate: IrohaPeerNfcRetryGateV1? = .init(.default)
    var platformCallGate: IrohaPeerNfcReaderPlatformCallGateV1? = .init()
    var progressReporter: Reporter? = Reporter()
    var retryTimeoutTask: Task<Void, Never>?
    var retryPollingGate: IrohaPeerNfcReaderPlatformCallGateV1?
    var recoveryWait: UInt64?
    var recoveryWaitGeneration: UInt64 = 0
    var failures = 0

    func startRecovery() {
        let epoch = operationGate.beginOperation()
        precondition(retryGate!.claimContactAttempt(operationGate: &operationGate, capturedEpoch: epoch))
        precondition(operationGate.finishConnect(capturedEpoch: epoch))
        restartPollingWithDeadline(session, operationEpoch: epoch)
    }

    func finish(_ result: Result<Void, Error>, invalidating: Bool,
                expectedEpoch: UInt64, expectedSession: NFCTagReaderSession,
                expectedRecoveryWait: UInt64? = nil) {
        lock.lock(); defer { lock.unlock() }
        guard !completed, session === expectedSession,
              operationGate.mayMutate(capturedEpoch: expectedEpoch),
              expectedRecoveryWait.map({ recoveryWait == $0 }) ?? true else { return }
        completed = true
        failures += 1
    }

    func validateCurrentOperation(capturedEpoch: UInt64, expectedSession: NFCTagReaderSession) throws {
        lock.lock(); defer { lock.unlock() }
        guard !completed, session === expectedSession,
              operationGate.mayMutate(capturedEpoch: capturedEpoch) else { throw CancellationError() }
    }

    /// Mirrors the production detection path: claim the connect slot and retire the armed recovery wait.
    func didRedetect() -> Bool {
        lock.lock(); defer { lock.unlock() }
        guard !completed else { return false }
        let accepted = retryGate!.claimContactAttempt(operationGate: &operationGate,
                                                     capturedEpoch: operationGate.activeEpoch)
        if accepted {
            retryTimeoutTask?.cancel()
            recoveryWait = nil
            retryPollingGate?.invalidate(); retryPollingGate = nil
        }
        return accepted
    }

    func cancel() {
        lock.lock(); defer { lock.unlock() }
        completed = true
        _ = operationGate.finishOperation(capturedEpoch: operationGate.activeEpoch)
        retryTimeoutTask?.cancel()
        platformCallGate?.invalidate()
    }
""" + restart + r"""
}

@main struct Main {
    static func main() async throws {
        let recovering = ReaderCut()
        recovering.startRecovery()
        precondition(recovering.session.polls == 1, "Recovery must restart polling immediately")
        precondition(recovering.progressReporter!.lost == 1, "Contact loss must publish guidance immediately")
        let cancelled = ReaderCut()
        cancelled.startRecovery()
        cancelled.cancel()
        precondition(cancelled.retryTimeoutTask?.isCancelled == true, "Cancel must stop the wait immediately")
        try await Task.sleep(nanoseconds: 3_500_000_000)
        precondition(recovering.failures == 0, "Former three-second limit must not force manual Try again")
        precondition(recovering.didRedetect(), "A returning phone must resume immediately")
        precondition(cancelled.failures == 0 && !cancelled.didRedetect(), "Cancelled recovery cannot restart")
        recovering.cancel()

        // A recovery timer that already woke cannot end the contact that replaced its wait.
        let replaced = ReaderCut()
        replaced.startRecovery()
        let staleWait = replaced.recoveryWait!
        precondition(replaced.didRedetect(), "A returning phone claims the contact")
        replaced.finish(.failure(IrohaPeerNfcCoreNFCErrorV1.retryExhausted), invalidating: true,
                        expectedEpoch: replaced.operationGate.activeEpoch, expectedSession: replaced.session,
                        expectedRecoveryWait: staleWait)
        precondition(replaced.failures == 0 && !replaced.completed, "A stale recovery timer cannot finish its successor")
        replaced.cancel()
        print("PASS: immediate polling/guidance, recovery after 3.5s, immediate cancellation, stale retry refused, stale timer owns nothing")
    }
}
"""
        with tempfile.TemporaryDirectory(prefix="iroha-nfc-recovery-") as temporary:
            main = Path(temporary) / "main.swift"
            binary = Path(temporary) / "recovery"
            main.write_text(source)
            compile_result = subprocess.run(
                [SWIFTC, "-swift-version", "5", "-parse-as-library", str(main), "-o", str(binary)],
                capture_output=True, text=True, check=False,
            )
            self.assertEqual(compile_result.returncode, 0, compile_result.stdout + compile_result.stderr)
            result = subprocess.run([str(binary)], capture_output=True, text=True, check=False, timeout=15)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
