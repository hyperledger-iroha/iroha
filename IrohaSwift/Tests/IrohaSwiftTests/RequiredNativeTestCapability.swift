import Foundation

/// Fails a native-dependent test when its release artifact or symbol is absent.
///
/// Native-backed release tests must never turn missing coverage into an XCTest
/// capability skip. Throwing this error aborts the test as a failure while
/// preserving a useful diagnostic.
enum RequiredNativeTestCapabilityError: LocalizedError {
    case unavailable(String)

    var errorDescription: String? {
        switch self {
        case .unavailable(let message):
            message
        }
    }
}

/// Requires a native release capability for the remainder of the current test.
func requireNativeTestCapability(
    _ available: @autoclosure () throws -> Bool,
    _ message: @autoclosure () -> String
) throws {
    guard try available() else {
        throw RequiredNativeTestCapabilityError.unavailable(message())
    }
}

/// Unconditionally fails a native-dependent path and terminates the test.
func failRequiredNativeTestCapability(_ message: @autoclosure () -> String) throws -> Never {
    throw RequiredNativeTestCapabilityError.unavailable(message())
}
