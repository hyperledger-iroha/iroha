import Foundation

enum IrohaPeerMobileFixtureSupport {
    static func loadNearby() throws -> PeerNearbyFixtureV1 {
        try load("peer_nearby_v1.json", as: PeerNearbyFixtureV1.self)
    }

    static func loadNfc() throws -> PeerNfcFixtureV1 {
        try load("peer_nfc_v1.json", as: PeerNfcFixtureV1.self)
    }

    static func hex(_ value: String) throws -> Data {
        guard value.count.isMultiple(of: 2),
              value.unicodeScalars.allSatisfy({
                  (48...57).contains($0.value)
                      || (65...70).contains($0.value)
                      || (97...102).contains($0.value)
              }) else {
            throw FixtureError.invalidHex
        }
        var bytes = Data()
        bytes.reserveCapacity(value.count / 2)
        var cursor = value.startIndex
        while cursor < value.endIndex {
            let next = value.index(cursor, offsetBy: 2)
            guard let byte = UInt8(value[cursor..<next], radix: 16) else {
                throw FixtureError.invalidHex
            }
            bytes.append(byte)
            cursor = next
        }
        return bytes
    }

    private static func load<T: Decodable>(
        _ name: String,
        as type: T.Type
    ) throws -> T {
        let packageRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let url = packageRoot
            .appendingPathComponent("Fixtures/offline")
            .appendingPathComponent(name)
        return try JSONDecoder().decode(T.self, from: Data(contentsOf: url))
    }

    private enum FixtureError: Error {
        case invalidHex
    }
}

struct PeerNearbyFixtureV1: Decodable {
    let serviceID: String
    let sessionHex: String
    let requestHashHex: String
    let discoveryReceiverHex: String
    let discoveryReceiverRadioBase64URL: String
    let senderHelloHex: String
    let receiverHelloHex: String
    let transcriptHashHex: String
    let senderAuthHex: String
    let encryptedRecordCodecHex: String
    let aesGcm: AESGCM

    struct AESGCM: Decodable {
        let sessionRepeatByte: UInt8
        let requestHashRepeatByte: UInt8
        let senderPrivateScalar: UInt8
        let receiverPrivateScalar: UInt8
        let senderNonceRepeatByte: UInt8
        let receiverNonceRepeatByte: UInt8
        let transcriptHashHex: String
        let senderPlaintextUTF8: String
        let senderRecordHex: String
        let receiverPlaintextUTF8: String
        let receiverRecordHex: String

        enum CodingKeys: String, CodingKey {
            case sessionRepeatByte = "session_repeat_byte"
            case requestHashRepeatByte = "request_hash_repeat_byte"
            case senderPrivateScalar = "sender_private_scalar"
            case receiverPrivateScalar = "receiver_private_scalar"
            case senderNonceRepeatByte = "sender_nonce_repeat_byte"
            case receiverNonceRepeatByte = "receiver_nonce_repeat_byte"
            case transcriptHashHex = "transcript_hash_hex"
            case senderPlaintextUTF8 = "sender_plaintext_utf8"
            case senderRecordHex = "sender_record_hex"
            case receiverPlaintextUTF8 = "receiver_plaintext_utf8"
            case receiverRecordHex = "receiver_record_hex"
        }
    }

    enum CodingKeys: String, CodingKey {
        case serviceID = "service_id"
        case sessionHex = "session_hex"
        case requestHashHex = "request_hash_hex"
        case discoveryReceiverHex = "discovery_receiver_hex"
        case discoveryReceiverRadioBase64URL = "discovery_receiver_radio_base64url"
        case senderHelloHex = "sender_hello_hex"
        case receiverHelloHex = "receiver_hello_hex"
        case transcriptHashHex = "transcript_hash_hex"
        case senderAuthHex = "sender_auth_hex"
        case encryptedRecordCodecHex = "encrypted_record_codec_hex"
        case aesGcm = "aes_gcm"
    }
}

struct PeerNfcFixtureV1: Decodable {
    let aidHex: String
    let sessionHex: String
    let infoHex: String
    let ackReadyStatusHex: String
    let messages: Messages
    let apduHex: APDU
    let checkpoint: Checkpoint
    let paymentAdmission: PaymentAdmission
    let durableAck: Digest

    struct Messages: Decodable {
        let request: Message
        let payment: Message
        let acknowledgement: Message
    }

    struct Message: Decodable {
        let profile: UInt16
        let kind: UInt8
        let schemaVersion: UInt16
        let repeatByte: UInt8
        let count: Int
        let wireHashHex: String

        enum CodingKeys: String, CodingKey {
            case profile
            case kind
            case schemaVersion = "schema_version"
            case repeatByte = "repeat_byte"
            case count
            case wireHashHex = "wire_hash_hex"
        }
    }

    struct APDU: Decodable {
        let select: String
        let getInfo: String
        let readRequest: String
        let beginPayment: String
        let write300: [String]
        let commit: String
        let readAck1024: String
        let confirmAck: String
        let getStatus: String

        enum CodingKeys: String, CodingKey {
            case select
            case getInfo = "get_info"
            case readRequest = "read_request"
            case beginPayment = "begin_payment"
            case write300 = "write_300"
            case commit
            case readAck1024 = "read_ack_1024"
            case confirmAck = "confirm_ack"
            case getStatus = "get_status"
        }
    }

    struct Checkpoint: Decodable {
        let withoutAckLength: Int
        let withoutAckBlake2b256Hex: String
        let withAckLength: Int
        let withAckBlake2b256Hex: String

        enum CodingKeys: String, CodingKey {
            case withoutAckLength = "without_ack_length"
            case withoutAckBlake2b256Hex = "without_ack_blake2b_256_hex"
            case withAckLength = "with_ack_length"
            case withAckBlake2b256Hex = "with_ack_blake2b_256_hex"
        }
    }

    struct Digest: Decodable {
        let length: Int
        let blake2b256Hex: String

        enum CodingKeys: String, CodingKey {
            case length
            case blake2b256Hex = "blake2b_256_hex"
        }
    }

    struct PaymentAdmission: Decodable {
        let length: Int
        let blake2b256Hex: String
        let encodedHex: String

        enum CodingKeys: String, CodingKey {
            case length
            case blake2b256Hex = "blake2b_256_hex"
            case encodedHex = "encoded_hex"
        }
    }

    enum CodingKeys: String, CodingKey {
        case aidHex = "aid_hex"
        case sessionHex = "session_hex"
        case infoHex = "info_hex"
        case ackReadyStatusHex = "ack_ready_status_hex"
        case messages
        case apduHex = "apdu_hex"
        case checkpoint
        case paymentAdmission = "payment_admission"
        case durableAck = "durable_ack"
    }
}
