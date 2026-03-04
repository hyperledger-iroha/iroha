import Foundation
import CryptoKit

public struct OfflineReceiptChallenge: Sendable, Equatable {
    public struct Result: Sendable, Equatable {
        public let preimage: Data
        public let irohaHash: Data
        public let clientDataHash: Data
    }

    public enum Error: Swift.Error {
        case bridgeUnavailable
        case bridgeFailure(Int32)
        case invalidInput(String)
    }

    public static func encode(
        chainId: String,
        invoiceId: String,
        receiverAccountId: String,
        assetId: String,
        amount: String,
        issuedAtMs: UInt64,
        senderCertificateIdHex: String,
        nonceHex: String,
        expectedScale: Int? = nil
    ) throws -> Result {
        if chainId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            throw Error.invalidInput("chainId must not be empty")
        }
        try validateAmount(amount, expectedScale: expectedScale)
        _ = try parseHashHex(senderCertificateIdHex, field: "senderCertificateIdHex")
        _ = try parseHashHex(nonceHex, field: "nonceHex")
        do {
            guard let native = try NoritoNativeBridge.shared.offlineReceiptChallenge(
                chainId: chainId,
                invoiceId: invoiceId,
                receiverId: receiverAccountId,
                assetId: assetId,
                amount: amount,
                issuedAtMs: issuedAtMs,
                senderCertificateIdHex: senderCertificateIdHex,
                nonceHex: nonceHex
            ) else {
                throw Error.bridgeUnavailable
            }
            return Result(
                preimage: native.preimage,
                irohaHash: native.irohaHash,
                clientDataHash: native.clientHash
            )
        } catch let bridgeError as NoritoNativeBridge.OfflineReceiptChallengeBridgeError {
            switch bridgeError {
            case .callFailed(let code):
                return try computeCanonical(chainId: chainId,
                                            invoiceId: invoiceId,
                                            receiverAccountId: receiverAccountId,
                                            assetId: assetId,
                                            amount: amount,
                                            issuedAtMs: issuedAtMs,
                                            senderCertificateIdHex: senderCertificateIdHex,
                                            nonceHex: nonceHex,
                                            status: code)
            }
        } catch Error.bridgeUnavailable {
            return try computeCanonical(chainId: chainId,
                                        invoiceId: invoiceId,
                                        receiverAccountId: receiverAccountId,
                                        assetId: assetId,
                                        amount: amount,
                                        issuedAtMs: issuedAtMs,
                                        senderCertificateIdHex: senderCertificateIdHex,
                                        nonceHex: nonceHex,
                                        status: nil)
        } catch {
            throw error
        }
    }

    private static func computeCanonical(
        chainId: String,
        invoiceId: String,
        receiverAccountId: String,
        assetId: String,
        amount: String,
        issuedAtMs: UInt64,
        senderCertificateIdHex: String,
        nonceHex: String,
        status: Int32?
    ) throws -> Result {
        do {
            let senderCertificateId = try parseHashHex(senderCertificateIdHex, field: "senderCertificateIdHex")
            let nonce = try parseHashHex(nonceHex, field: "nonceHex")
            let preimage = OfflineReceiptChallengePreimage(
                invoiceId: invoiceId,
                receiverAccountId: receiverAccountId,
                assetId: assetId,
                amount: amount,
                issuedAtMs: issuedAtMs,
                senderCertificateId: senderCertificateId,
                nonce: nonce
            )
            let payload = try preimage.noritoPayload()
            let bytes = OfflineNorito.wrap(typeName: OfflineReceiptChallengePreimage.noritoTypeName,
                                           payload: payload)
            let context = IrohaHash.hash(Data(chainId.utf8))
            var hashInput = Data()
            hashInput.reserveCapacity(context.count + bytes.count)
            hashInput.append(context)
            hashInput.append(bytes)
            let irohaHash = IrohaHash.hash(hashInput)
            let client = Data(SHA256.hash(data: irohaHash))
            return Result(preimage: bytes, irohaHash: irohaHash, clientDataHash: client)
        } catch {
            let message = status.map { "\(error.localizedDescription) (bridge code \($0))" } ?? error.localizedDescription
            throw Error.invalidInput(message)
        }
    }

    private static func parseHashHex(_ value: String, field: String) throws -> Data {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let bytes = Data(hexString: trimmed), bytes.count == 32 else {
            throw Error.invalidInput("\(field) must be 64 hex characters")
        }
        return bytes
    }

    private static func validateAmount(_ value: String, expectedScale: Int?) throws {
        do {
            _ = try OfflineNorito.encodeNumeric(value)
        } catch let error as OfflineNoritoError {
            throw Error.invalidInput(error.localizedDescription)
        } catch {
            throw Error.invalidInput("amount must be numeric: \(value)")
        }
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        var index = trimmed.startIndex
        if index < trimmed.endIndex && (trimmed[index] == "-" || trimmed[index] == "+") {
            index = trimmed.index(after: index)
        }
        var seenDigit = false
        var seenDot = false
        var scale = 0
        while index < trimmed.endIndex {
            let ch = trimmed[index]
            if ch == "." {
                if seenDot {
                    throw Error.invalidInput("amount must be numeric: \(value)")
                }
                seenDot = true
                index = trimmed.index(after: index)
                continue
            }
            guard ch.wholeNumberValue != nil else {
                throw Error.invalidInput("amount must be numeric: \(value)")
            }
            seenDigit = true
            if seenDot {
                scale += 1
            }
            index = trimmed.index(after: index)
        }
        if !seenDigit {
            throw Error.invalidInput("amount must be numeric: \(value)")
        }
        if scale > 28 {
            throw Error.invalidInput("amount scale exceeds 28: \(value)")
        }
        if let expectedScale, scale != expectedScale {
            throw Error.invalidInput("amount must use scale \(expectedScale): \(value)")
        }
    }
}
