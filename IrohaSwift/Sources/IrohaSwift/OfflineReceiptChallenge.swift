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
        invoiceId: String,
        receiverAccountId: String,
        assetId: String,
        amount: String,
        nonceHex: String
    ) throws -> Result {
        do {
            guard let native = try NoritoNativeBridge.shared.offlineReceiptChallenge(
                invoiceId: invoiceId,
                receiverId: receiverAccountId,
                assetId: assetId,
                amount: amount,
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
                return try computeCanonical(invoiceId: invoiceId,
                                            receiverAccountId: receiverAccountId,
                                            assetId: assetId,
                                            amount: amount,
                                            nonceHex: nonceHex,
                                            status: code)
            }
        } catch Error.bridgeUnavailable {
            return try computeCanonical(invoiceId: invoiceId,
                                        receiverAccountId: receiverAccountId,
                                        assetId: assetId,
                                        amount: amount,
                                        nonceHex: nonceHex,
                                        status: nil)
        } catch {
            throw error
        }
    }

    private static func computeCanonical(
        invoiceId: String,
        receiverAccountId: String,
        assetId: String,
        amount: String,
        nonceHex: String,
        status: Int32?
    ) throws -> Result {
        let preimage = OfflineReceiptChallengePreimage(
            invoiceId: invoiceId,
            receiverAccountId: receiverAccountId,
            assetId: assetId,
            amount: amount,
            nonceHex: nonceHex
        )
        do {
            let payload = try preimage.noritoPayload()
            let bytes = OfflineNorito.wrap(typeName: OfflineReceiptChallengePreimage.noritoTypeName,
                                           payload: payload)
            let irohaHash = IrohaHash.hash(bytes)
            let client = Data(SHA256.hash(data: irohaHash))
            return Result(preimage: bytes, irohaHash: irohaHash, clientDataHash: client)
        } catch {
            let message = status.map { "\(error.localizedDescription) (bridge code \($0))" } ?? error.localizedDescription
            throw Error.invalidInput(message)
        }
    }
}
