import Foundation

/// A reader result does not release the application to present authentication
/// until CoreNFC invalidates that exact session. The owner serializes access.
struct IrohaPeerNfcReaderInvalidationV1<Value> {
    private var pending: (session: ObjectIdentifier, result: Result<Value, Error>, retainsSuccess: Bool)?

    var isPending: Bool { pending != nil }

    /// `retainsSuccess`: the success is a finished payment whose acknowledgement is
    /// already durable and whose completion is already published. Neither a late
    /// cancellation nor a missing platform callback can report it as failed.
    mutating func begin(session: ObjectIdentifier, result: Result<Value, Error>,
                        retainsSuccess: Bool = false) {
        precondition(pending == nil)
        pending = (session, result, retainsSuccess)
    }

    /// Cancellation can retire a successful read while its sheet is closing.
    /// It never turns an interrupted read into authorization to move value.
    mutating func cancel(with error: Error) {
        guard let saved = pending else { return }
        if saved.retainsSuccess, case .success = saved.result { return }
        pending = (saved.session, .failure(error), saved.retainsSuccess)
    }

    mutating func invalidated(session: ObjectIdentifier) -> Result<Value, Error>? {
        guard let saved = pending, saved.session == session else { return nil }
        pending = nil
        return saved.result
    }

    /// A missing platform callback is a failure, never permission to show the
    /// next system prompt. Preserve an earlier failure when there was one.
    mutating func timedOut(session: ObjectIdentifier, error: Error) -> Result<Value, Error>? {
        guard let saved = pending, saved.session == session else { return nil }
        pending = nil
        if saved.retainsSuccess, case .success = saved.result { return saved.result }
        if case .failure(let failure) = saved.result,
           !(failure is IrohaPeerNfcReaderContinuationErrorV1) { return saved.result }
        // Renewal requires the actual matching delegate callback, never a timer.
        return .failure(error)
    }
}
