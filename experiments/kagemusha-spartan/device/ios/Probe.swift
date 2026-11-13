import Foundation
import Security
import SwiftUI

// Isolated hardware boundary probe: no ledger, account, wallet state or value.
// The private key never leaves the Secure Enclave and is retained only in this
// test app's Keychain namespace for exact native-receipt signing on a later run.
private enum Probe {
    static let tag = Data("jp.co.soramitsu.kagemusha.spartanfixture.payment.v1".utf8)
    static func hex(_ data: Data) -> String { data.map { String(format: "%02x", $0) }.joined() }
    static func unhex(_ text: String) throws -> Data {
        let bytes = Array(text.utf8)
        guard bytes.count % 2 == 0 else { throw Failure("hex length") }
        func digit(_ x: UInt8) throws -> UInt8 {
            switch x { case 48...57: return x - 48; case 97...102: return x - 87; default: throw Failure("hex digit") }
        }
        var out = Data()
        for i in stride(from: 0, to: bytes.count, by: 2) { out.append(try digit(bytes[i]) * 16 + digit(bytes[i + 1])) }
        return out
    }
    struct Failure: Error, CustomStringConvertible {
        let description: String
        init(_ text: String) { description = text }
    }
    static func key() throws -> SecKey {
        var result: CFTypeRef?
        let query: [CFString: Any] = [kSecClass: kSecClassKey, kSecAttrApplicationTag: tag,
            kSecAttrKeyType: kSecAttrKeyTypeECSECPrimeRandom, kSecReturnRef: true]
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        if status == errSecSuccess { return result as! SecKey }
        guard status == errSecItemNotFound else { throw Failure("Keychain lookup \(status)") }
        var error: Unmanaged<CFError>?
        guard let access = SecAccessControlCreateWithFlags(nil, kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly,
            .privateKeyUsage, &error) else { throw Failure("Secure Enclave access control") }
        let attributes: [CFString: Any] = [kSecAttrKeyType: kSecAttrKeyTypeECSECPrimeRandom,
            kSecAttrKeySizeInBits: 256, kSecAttrTokenID: kSecAttrTokenIDSecureEnclave,
            kSecPrivateKeyAttrs: [kSecAttrIsPermanent: true, kSecAttrApplicationTag: tag, kSecAttrAccessControl: access]]
        guard let key = SecKeyCreateRandomKey(attributes as CFDictionary, &error) else {
            throw Failure("Secure Enclave generation: \(String(describing: error?.takeRetainedValue()))")
        }
        return key
    }
    static func rawLowS(_ der: Data) throws -> Data {
        let b = Array(der); var i = 0
        func next() throws -> Int { guard i < b.count else { throw Failure("DER truncation") }; defer { i += 1 }; return Int(b[i]) }
        guard try next() == 48, try next() == b.count - 2, try next() == 2 else { throw Failure("DER header") }
        func integer() throws -> [UInt8] {
            let count = try next(); guard count > 0, i + count <= b.count else { throw Failure("DER integer") }
            var value = Array(b[i..<i+count]); i += count
            if value.count == 33 && value[0] == 0 { value.removeFirst() }
            guard value.count <= 32 else { throw Failure("DER scalar width") }
            return [UInt8](repeating: 0, count: 32 - value.count) + value
        }
        let r = try integer(); guard try next() == 2 else { throw Failure("DER S") }
        var s = try integer(); guard i == b.count else { throw Failure("DER trailing bytes") }
        let n = Array(try unhex("ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551"))
        let half = Array(try unhex("7fffffff800000007fffffffffffffffde737d56d38bcf4279dce5617e3192a8"))
        guard r.contains(where: { $0 != 0 }), s.contains(where: { $0 != 0 }), r.lexicographicallyPrecedes(n), s.lexicographicallyPrecedes(n) else { throw Failure("DER scalar range") }
        if half.lexicographicallyPrecedes(s) {
            var borrow = 0
            for j in (0..<32).reversed() { var value = Int(n[j]) - Int(s[j]) - borrow; borrow = value < 0 ? 1 : 0; if value < 0 { value += 256 }; s[j] = UInt8(value) }
        }
        return Data(r + s)
    }
    static func run() throws -> String {
        let key = try key(); var error: Unmanaged<CFError>?
        guard SecKeyCopyExternalRepresentation(key, &error) == nil else { throw Failure("Private key was exportable") }
        let attrs = SecKeyCopyAttributes(key)! as NSDictionary
        guard attrs[kSecAttrTokenID] as? String == kSecAttrTokenIDSecureEnclave as String else { throw Failure("Key is not Secure Enclave") }
        guard let publicKey = SecKeyCopyPublicKey(key), let sec1 = SecKeyCopyExternalRepresentation(publicKey, &error) as Data? else { throw Failure("public key") }
        let args = ProcessInfo.processInfo.arguments
        var message = Data(repeating: 0, count: 32)
        if let i = args.firstIndex(of: "-spartan-message"), i + 1 < args.count { message = try unhex(args[i + 1]) }
        else { guard message.withUnsafeMutableBytes({ SecRandomCopyBytes(kSecRandomDefault, 32, $0.baseAddress!) }) == errSecSuccess else { throw Failure("entropy") } }
        guard message.count == 32 else { throw Failure("exact native message length") }
        guard let signature = SecKeyCreateSignature(key, .ecdsaSignatureMessageX962SHA256, message as CFData, &error) as Data? else { throw Failure("Secure Enclave signing") }
        guard SecKeyVerifySignature(publicKey, .ecdsaSignatureMessageX962SHA256, message as CFData, signature as CFData, &error) else { throw Failure("signature verification") }
        let output: [String: Any] = ["schema": "kagemusha-spartan-hardware-probe-v1", "secure_enclave": true,
            "nonexportable": true, "public_key_sec1": hex(sec1), "message": hex(message),
            "signature_der": hex(signature), "signature_raw_low_s": hex(try rawLowS(signature))]
        let data = try JSONSerialization.data(withJSONObject: output, options: [.prettyPrinted, .sortedKeys])
        let file = URL.documentsDirectory.appending(path: "spartan-hardware-probe.json")
        try data.write(to: file, options: .atomic)
        return String(decoding: data, as: UTF8.self)
    }
}

@main struct SpartanHardwareProbe: App {
    @State private var result = "Running isolated Secure Enclave fixture"
    var body: some Scene { WindowGroup { ScrollView { Text(result).font(.system(.caption, design: .monospaced)).padding() }
        .task { do { result = try Probe.run() } catch { result = "FAILED: \(error)"; try? Data(result.utf8).write(to: URL.documentsDirectory.appending(path: "spartan-hardware-error.txt"), options: .atomic) } }
    } }
}
