import Foundation

/// Shared exact Rust-produced native status rows; never hand-written wire goldens.
enum NativeStatusFixtures {
    static func rows() throws -> [(String, Data, Data)] {
        var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        while directory.path != "/" {
            let path = directory.appendingPathComponent("fixtures/sumeragi/native_status_v1.tsv")
            if FileManager.default.fileExists(atPath: path.path) {
                let rows: [(String, Data, Data)] = try String(contentsOf: path, encoding: .utf8).split(separator: "\n")
                    .filter { !$0.hasPrefix("#") }.map {
                        let fields = $0.split(separator: "\t", omittingEmptySubsequences: false)
                        guard fields.count == 3, !fields[2].isEmpty, fields[2].count % 2 == 0,
                              fields[2].allSatisfy({ "0123456789abcdef".contains($0) }),
                              let wire = Data(hexString: String(fields[2])) else {
                            throw CocoaError(.fileReadCorruptFile)
                        }
                        return (String(fields[0]), Data(fields[1].utf8), wire)
                    }
                let names = Set(rows.map { $0.0 })
                guard names.count == rows.count, names == Set([
                    "validator", "observer", "safety_record_corrupt", "safety_record_inconsistent",
                    "safety_violation", "apply_diverged", "publication_recovery_required", "driver_anomaly"
                ]) else { throw CocoaError(.fileReadCorruptFile) }
                return rows
            }
            directory.deleteLastPathComponent()
        }
        throw CocoaError(.fileNoSuchFile)
    }
    static func json(_ name: String = "validator") throws -> Data {
        guard let row = try rows().first(where: { $0.0 == name }) else { throw CocoaError(.fileNoSuchFile) }
        return row.1
    }
}
