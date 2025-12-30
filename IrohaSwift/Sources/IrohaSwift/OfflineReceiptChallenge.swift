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
        nonceHex: String
    ) throws -> Result {
        if chainId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            throw Error.invalidInput("chainId must not be empty")
        }
        try validateAmount(amount)
        do {
            guard let native = try NoritoNativeBridge.shared.offlineReceiptChallenge(
                chainId: chainId,
                invoiceId: invoiceId,
                receiverId: receiverAccountId,
                assetId: assetId,
                amount: amount,
                issuedAtMs: issuedAtMs,
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
        nonceHex: String,
        status: Int32?
    ) throws -> Result {
        let preimage = OfflineReceiptChallengePreimage(
            invoiceId: invoiceId,
            receiverAccountId: receiverAccountId,
            assetId: assetId,
            amount: amount,
            issuedAtMs: issuedAtMs,
            nonceHex: nonceHex
        )
        do {
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

    private static func validateAmount(_ value: String) throws {
        do {
            _ = try OfflineNorito.encodeNumeric(value)
            let parsed = try OfflineDecimal.parse(value)
            if parsed.scale != 0 {
                throw Error.invalidInput("amount must use scale 0: \(value)")
            }
        } catch let error as OfflineNoritoError {
            throw Error.invalidInput(error.localizedDescription)
        } catch {
            throw Error.invalidInput("amount must be numeric: \(value)")
        }
    }
}
