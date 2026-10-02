import Foundation

// Public value types of the KAGEMUSHA attested-app suite
// (`iroha:kagemusha:v1:attested-app`).
//
// The suite authorizes every offline transition with a P-256 signature from an attested,
// app-bound hardware key over an issuer-certified device certificate. It claims bounded loss
// plus fork detection, attribution and revocation. It never claims a rollback-resistant
// counter, a trusted clock, one-use keys or fork prevention on a compromised device.

/// An amount in minor units of the scheme asset (`0 < minor <= 10^15` for transfers).
public struct KagemushaAmount: Sendable, Hashable, Comparable, CustomStringConvertible {
    /// Largest amount any single suite object may carry.
    public static let maximumMinor: UInt64 = 1_000_000_000_000_000

    /// Minor units at the asset scale.
    public let minor: UInt64

    public init(minor: UInt64) {
        self.minor = minor
    }

    /// Parse a plain non-negative decimal such as `"12.50"` at the given asset scale.
    ///
    /// Rejects signs, exponents, separators, more fractional digits than `scale` and values above
    /// ``maximumMinor``. Fewer fractional digits are zero-extended.
    public static func parse(_ decimal: String, scale: Int) throws -> KagemushaAmount {
        guard (0...18).contains(scale), !decimal.isEmpty else {
            throw KagemushaError.invalidAmount
        }
        let parts = decimal.split(separator: ".", omittingEmptySubsequences: false)
        guard parts.count <= 2 else { throw KagemushaError.invalidAmount }
        let whole = parts[0]
        let fraction = parts.count == 2 ? parts[1] : ""
        guard !whole.isEmpty, whole.utf8.allSatisfy({ $0 >= 0x30 && $0 <= 0x39 }),
              fraction.utf8.allSatisfy({ $0 >= 0x30 && $0 <= 0x39 }),
              parts.count == 1 || !fraction.isEmpty,
              fraction.count <= scale
        else { throw KagemushaError.invalidAmount }
        var value: UInt64 = 0
        let digits = Array(whole.utf8) + Array(fraction.utf8)
            + Array(repeating: UInt8(0x30), count: scale - fraction.count)
        for byte in digits {
            let (scaled, overflowA) = value.multipliedReportingOverflow(by: 10)
            let (sum, overflowB) = scaled.addingReportingOverflow(UInt64(byte - 0x30))
            guard !overflowA, !overflowB else { throw KagemushaError.invalidAmount }
            value = sum
        }
        guard value <= maximumMinor else { throw KagemushaError.invalidAmount }
        return KagemushaAmount(minor: value)
    }

    /// Format with exactly `scale` fractional digits, for example `"12.50"`.
    public func format(scale: Int) -> String {
        let digits = String(minor)
        guard scale > 0 else { return digits }
        let padded = String(repeating: "0", count: max(0, scale + 1 - digits.count)) + digits
        let split = padded.index(padded.endIndex, offsetBy: -scale)
        return String(padded[..<split]) + "." + String(padded[split...])
    }

    public var description: String { "\(minor)" }

    public static func < (lhs: KagemushaAmount, rhs: KagemushaAmount) -> Bool {
        lhs.minor < rhs.minor
    }
}

/// Wallet configuration. The scheme identity and root key are pinned by the application; the
/// signed scheme descriptor fetched from the issuer must match both.
public struct KagemushaConfig: Sendable {
    /// 32-byte scheme identity.
    public let schemeId: Data
    /// 65-byte uncompressed SEC1 P-256 scheme root public key.
    public let schemeRootPublicKey: Data
    /// Issuer base URL; the suite routes live under `v1/kagemusha/attested/`.
    public let issuerURL: URL
    /// Debug-only opt-in that admits a descriptor with `allow_test_devices = true`.
    /// Release builds always treat this as `false`.
    public let testing: Bool
    /// Optional storage root override. Defaults to Application Support.
    public let storageDirectory: URL?

    public init(
        schemeId: Data,
        schemeRootPublicKey: Data,
        issuerURL: URL,
        testing: Bool = false,
        storageDirectory: URL? = nil
    ) {
        self.schemeId = schemeId
        self.schemeRootPublicKey = schemeRootPublicKey
        self.issuerURL = issuerURL
        self.testing = testing
        self.storageDirectory = storageDirectory
    }

    /// Whether test descriptors are admitted by this build.
    var admitsTestDescriptor: Bool {
        #if DEBUG
        return testing
        #else
        return false
        #endif
    }
}

/// The account controller's signature that binds an enrollment to an Iroha account.
public struct KagemushaAccountProof: Sendable, Equatable {
    /// Algorithm of the single account controller key, for example `"ed25519"`.
    public let algorithm: String
    /// Raw public key bytes of the single account controller.
    public let publicKey: Data
    /// Signature over the exact message given to ``KagemushaLedgerPort/signAccountProof(_:)``.
    public let signature: Data

    public init(algorithm: String, publicKey: Data, signature: Data) {
        self.algorithm = algorithm
        self.publicKey = publicKey
        self.signature = signature
    }
}

/// The application's online account: it signs enrollment proofs and moves value to the reserve.
public protocol KagemushaLedgerPort: Sendable {
    /// Canonical Iroha account identifier of the enrolled account.
    var accountId: String { get }
    /// Sign `message` with the account controller key.
    func signAccountProof(_ message: Data) async throws -> KagemushaAccountProof
    /// Submit one `Transfer(asset, amount, account -> reserve)` carrying `metadata`, wait for
    /// commit and return the committed transaction hash as lowercase hex.
    func transferToReserve(
        asset: String,
        reserve: String,
        amount: KagemushaAmount,
        metadata: [String: String]
    ) async throws -> String
}

/// Device platform recorded in the certificate. There is deliberately no test value.
public enum KagemushaPlatform: UInt8, Sendable, CaseIterable {
    case androidStrongBox = 1
    case androidTee = 2
    case appleSecureEnclave = 3
}

/// Why the wallet cannot hold offline value on this installation.
public enum KagemushaUnsupportedReason: Sendable, Equatable {
    /// No Secure Enclave (for example the Simulator).
    case secureEnclaveUnavailable
    /// App Attest is not available to this app or OS.
    case appAttestUnavailable
    /// The OS cannot run the suite.
    case platformUnsupported
    /// The issuer's descriptor admits test devices and this build did not opt in.
    case testDescriptorRefused
    /// A testing wallet was opened against a production descriptor.
    case testingRequiresTestDescriptor
    /// The issuer rejected this device's attestation.
    case attestationRejected(code: String)
}

/// Why an enrolled wallet is frozen. Recovery from a frozen state is online only.
public enum KagemushaFrozenReason: Sendable, Equatable {
    /// The device key exists but the wallet state is missing or behind the issuer's head.
    case stateLost
    /// The wallet state exists but its device key is gone.
    case keyLost
    /// The issuer revoked this device.
    case revoked(KagemushaRevocationReason)
}

/// Issuer revocation reasons.
public enum KagemushaRevocationReason: UInt8, Sendable, CaseIterable {
    case fraud = 0
    case lost = 1
    case integrity = 2
    case superseded = 3
    case closed = 4

    /// Reasons under which a receiver refuses the payer's payments and the issuer never
    /// delivers them later.
    public var refusesPayments: Bool { self == .fraud || self == .lost }
    /// Reasons under which the device can still sync and redeem its own balance.
    public var permitsRedemption: Bool { self != .fraud && self != .lost }
}

/// Which certificate or account limit an operation would exceed.
public enum KagemushaLimit: String, Sendable, Equatable {
    case maxPayment
    case maxUnsyncedOut
    case maxBalance
    case requestHeadroom
    case requestUses
}

/// Snapshot of a ready wallet.
public struct KagemushaReadyState: Sendable, Equatable {
    public let balance: KagemushaAmount
    public let tier: UInt8
    public let leaseEndsAtMs: UInt64
    public let needsSync: Bool
    public let clockAnomaly: Bool
    public let deviceId: Data
    public let unsyncedOut: KagemushaAmount
    public let maxPayment: KagemushaAmount
    /// `false` once the lease has expired or a revocation blocks paying; receiving continues.
    public let canPay: Bool
}

/// Honest wallet status. There is never a software-key fallback.
public enum KagemushaStatus: Sendable, Equatable {
    case unsupported(KagemushaUnsupportedReason)
    case notEnrolled
    case ready(KagemushaReadyState)
    case frozen(KagemushaFrozenReason)
}

/// Why a received payment was refused. Receiver caps, request state and amount mismatches are
/// never refusal reasons.
public enum KagemushaRefusal: UInt8, Sendable, CaseIterable {
    case malformed = 1
    case invalidCert = 2
    case invalidSignature = 3
    case wrongReceiver = 4
    case revoked = 5
    case fork = 6
    case expired = 7
    case payerLimit = 8

    /// Whether the issuer may still deliver a valid payment at the receiver's next sync.
    public var deliveredLaterIfValid: Bool {
        switch self {
        case .fork, .expired, .payerLimit: return true
        case .malformed, .invalidCert, .invalidSignature, .wrongReceiver, .revoked: return false
        }
    }
}

/// Wallet errors.
public enum KagemushaError: Error, Equatable, Sendable, LocalizedError {
    case unsupported(KagemushaUnsupportedReason)
    case notEnrolled
    case alreadyEnrolled
    case frozen(KagemushaFrozenReason)
    case leaseExpired
    case insufficientBalance
    case limitExceeded(KagemushaLimit)
    case invalidAmount
    case invalidRequest(String)
    case invalidMessage(String)
    case wrongReceiver
    case selfPayment
    case revoked
    case invalidSignature
    case unknownPayment
    case stateLost
    case keyLost
    case issuerRejected(status: Int, code: String)
    case invalidIssuerResponse(String)
    case network(retryable: Bool, description: String)
    case storage(String)
    case keyStore(String)
    case attestation(String)
    case ledger(String)

    public var errorDescription: String? {
        switch self {
        case .unsupported(let reason): return "Offline wallet unsupported: \(reason)."
        case .notEnrolled: return "The offline wallet is not enrolled."
        case .alreadyEnrolled: return "The offline wallet is already enrolled."
        case .frozen(let reason): return "The offline wallet is frozen: \(reason)."
        case .leaseExpired: return "The device certificate lease has expired; sync to renew it."
        case .insufficientBalance: return "The offline balance is insufficient."
        case .limitExceeded(let limit): return "The \(limit.rawValue) limit would be exceeded."
        case .invalidAmount: return "The amount is invalid."
        case .invalidRequest(let reason): return "Invalid payment request: \(reason)."
        case .invalidMessage(let reason): return "Invalid peer message: \(reason)."
        case .wrongReceiver: return "The message is addressed to another device."
        case .selfPayment: return "A device cannot pay itself."
        case .revoked: return "The counterparty device is revoked."
        case .invalidSignature: return "A signature is invalid."
        case .unknownPayment: return "The acknowledgement matches no outgoing payment."
        case .stateLost: return "The offline wallet state was lost."
        case .keyLost: return "The offline device key was lost."
        case .issuerRejected(let status, let code): return "The issuer rejected the request (\(status) \(code))."
        case .invalidIssuerResponse(let reason): return "Invalid issuer response: \(reason)."
        case .network(_, let description): return "Network failure: \(description)."
        case .storage(let reason): return "Wallet storage failure: \(reason)."
        case .keyStore(let reason): return "Device key failure: \(reason)."
        case .attestation(let reason): return "Attestation failure: \(reason)."
        case .ledger(let reason): return "Ledger failure: \(reason)."
        }
    }

    /// Whether retrying the same verb later can succeed without user action.
    public var isRetryable: Bool {
        switch self {
        case .network(let retryable, _): return retryable
        case .issuerRejected(let status, _): return status == 503 || status == 429
        default: return false
        }
    }
}

/// Result of ``KagemushaWallet/load(_:)``.
public struct KagemushaLoadResult: Sendable, Equatable {
    public let amount: KagemushaAmount
    public let voucherId: Data
    public let transactionHash: String
    public let balance: KagemushaAmount
}

/// A final outgoing payment. It is irrevocable once ``KagemushaWallet/pay(_:amount:)`` returns.
public struct KagemushaOutgoingPayment: Sendable, Equatable {
    public let paymentId: Data
    public let amount: KagemushaAmount
    public let receiverDeviceId: Data
    public let seq: UInt64
    /// The Payment message to show the receiver; it may be re-displayed until acknowledged.
    public let message: KagemushaPeerMessage
    /// Whether the receiver's courtesy acknowledgement has arrived.
    public let acknowledged: Bool
}

/// Outcome of ``KagemushaWallet/receive(_:)``.
public enum KagemushaReceiveResult: Sendable, Equatable {
    /// Final credit. Received value can be spent again offline immediately.
    case credited(amount: KagemushaAmount, paymentId: Data, ack: KagemushaPeerMessage)
    /// Refused. A valid payment may still be delivered by the issuer at the next sync.
    case refused(reason: KagemushaRefusal, deliveredLaterIfValid: Bool)
}

/// Issuer-side state of a redemption to the enrolled account.
public enum KagemushaRedemptionStatus: Sendable, Equatable {
    /// Committed offline; not yet reported to the issuer.
    case pendingSync
    /// Accepted; waiting for the daily unload cap.
    case queued
    /// Paid to the enrolled account.
    case paid(transactionHash: String?)
    /// Frozen by the issuer on fraud.
    case frozen
}

/// Result of ``KagemushaWallet/redeem(_:)``.
public struct KagemushaRedeemResult: Sendable, Equatable {
    public let redemptionId: Data
    public let amount: KagemushaAmount
    public let status: KagemushaRedemptionStatus
}

/// A payment delivered by the issuer at sync because the peer exchange never completed.
public struct KagemushaDeliveredPayment: Sendable, Equatable {
    public let paymentId: Data
    public let amount: KagemushaAmount
    public let payerDeviceId: Data
}

/// Result of ``KagemushaWallet/sync()``. Sync never confirms, holds or reverses a payment.
public struct KagemushaSyncResult: Sendable, Equatable {
    public let ackedSeq: UInt64
    public let balance: KagemushaAmount
    public let delivered: [KagemushaDeliveredPayment]
    public let redemptions: [KagemushaRedeemResult]
    public let certificateRenewed: Bool
    public let crlEpoch: UInt64
    public let revoked: KagemushaRevocationReason?
}
