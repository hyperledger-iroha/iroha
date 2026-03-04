import Foundation

extension Data {
    public init?(hexString: String) {
        let len = hexString.count
        if len % 2 != 0 { return nil }
        var data = Data(capacity: len / 2)
        var idx = hexString.startIndex
        while idx < hexString.endIndex {
            let next = hexString.index(idx, offsetBy: 2)
            let byteStr = hexString[idx..<next]
            guard let b = UInt8(byteStr, radix: 16) else { return nil }
            data.append(b)
            idx = next
        }
        self = data
    }
}
