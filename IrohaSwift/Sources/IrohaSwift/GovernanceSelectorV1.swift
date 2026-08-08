/// Canonical first-release governance selector grammar shared by transaction
/// and Torii request boundaries.
enum GovernanceSelectorV1 {
    static let maximumUTF8Bytes = 128

    static func isValid(_ value: String) -> Bool {
        let bytes = value.utf8
        guard !bytes.isEmpty,
              bytes.count <= maximumUTF8Bytes,
              let first = bytes.first,
              isUnreservedWithoutDot(first) else {
            return false
        }
        return bytes.dropFirst().allSatisfy { byte in
            isUnreservedWithoutDot(byte) || byte == 46
        }
    }

    private static func isUnreservedWithoutDot(_ byte: UInt8) -> Bool {
        (48...57).contains(byte)
            || (65...90).contains(byte)
            || (97...122).contains(byte)
            || byte == 45
            || byte == 95
            || byte == 126
    }
}
