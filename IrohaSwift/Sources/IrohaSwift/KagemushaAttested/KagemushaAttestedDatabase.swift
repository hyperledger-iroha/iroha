import Foundation
import SQLite3

/// One SQLite column value.
enum KagemushaSQLiteValue: Equatable, Sendable {
    case null
    case integer(Int64)
    case blob(Data)
    case text(String)

    static func unsigned(_ value: UInt64) -> Self { .integer(Int64(bitPattern: value)) }
    static func bool(_ value: Bool) -> Self { .integer(value ? 1 : 0) }

    var data: Data? {
        if case .blob(let value) = self { return value }
        return nil
    }

    var string: String? {
        if case .text(let value) = self { return value }
        return nil
    }

    var int64: Int64? {
        if case .integer(let value) = self { return value }
        return nil
    }

    var uint64: UInt64? { int64.map { UInt64(bitPattern: $0) } }
    var boolValue: Bool { int64 == 1 }
}

/// Minimal durable SQLite connection: WAL, `synchronous=FULL` and `fullfsync`, so every commit
/// reaches stable storage before a verb returns. One connection is owned by one wallet actor.
final class KagemushaAttestedDatabase {
    private var handle: OpaquePointer?
    let url: URL

    private static let transient = unsafeBitCast(-1, to: sqlite3_destructor_type.self)

    init(url: URL) throws {
        self.url = url
        var opened: OpaquePointer?
        let flags = SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE | SQLITE_OPEN_FULLMUTEX
        guard sqlite3_open_v2(url.path, &opened, flags, nil) == SQLITE_OK, let opened else {
            let message = opened.map { String(cString: sqlite3_errmsg($0)) } ?? "open failed"
            sqlite3_close_v2(opened)
            throw KagemushaError.storage(message)
        }
        handle = opened
        do {
            for pragma in [
                "PRAGMA journal_mode=WAL", "PRAGMA synchronous=FULL", "PRAGMA fullfsync=ON",
                "PRAGMA checkpoint_fullfsync=ON", "PRAGMA foreign_keys=ON", "PRAGMA secure_delete=ON",
            ] {
                _ = try query(pragma)
            }
        } catch {
            close()
            throw error
        }
    }

    deinit { close() }

    func close() {
        if let handle {
            sqlite3_close_v2(handle)
            self.handle = nil
        }
    }

    func execute(_ sql: String, _ arguments: [KagemushaSQLiteValue] = []) throws {
        _ = try query(sql, arguments)
    }

    @discardableResult
    func query(_ sql: String, _ arguments: [KagemushaSQLiteValue] = []) throws -> [[KagemushaSQLiteValue]] {
        guard let handle else { throw KagemushaError.storage("database closed") }
        var statement: OpaquePointer?
        guard sqlite3_prepare_v2(handle, sql, -1, &statement, nil) == SQLITE_OK, let statement else {
            throw KagemushaError.storage(String(cString: sqlite3_errmsg(handle)))
        }
        defer { sqlite3_finalize(statement) }
        for (offset, argument) in arguments.enumerated() {
            let index = Int32(offset + 1)
            let status: Int32
            switch argument {
            case .null:
                status = sqlite3_bind_null(statement, index)
            case .integer(let value):
                status = sqlite3_bind_int64(statement, index, value)
            case .blob(let value):
                status = value.withUnsafeBytes { buffer in
                    sqlite3_bind_blob(statement, index, buffer.baseAddress ?? UnsafeRawPointer(bitPattern: 1),
                                      Int32(buffer.count), Self.transient)
                }
            case .text(let value):
                status = sqlite3_bind_text(statement, index, value, -1, Self.transient)
            }
            guard status == SQLITE_OK else {
                throw KagemushaError.storage(String(cString: sqlite3_errmsg(handle)))
            }
        }
        var rows: [[KagemushaSQLiteValue]] = []
        while true {
            let step = sqlite3_step(statement)
            if step == SQLITE_DONE { break }
            guard step == SQLITE_ROW else {
                throw KagemushaError.storage(String(cString: sqlite3_errmsg(handle)))
            }
            var row: [KagemushaSQLiteValue] = []
            for column in 0..<sqlite3_column_count(statement) {
                switch sqlite3_column_type(statement, column) {
                case SQLITE_INTEGER:
                    row.append(.integer(sqlite3_column_int64(statement, column)))
                case SQLITE_BLOB:
                    let count = Int(sqlite3_column_bytes(statement, column))
                    if count == 0 {
                        row.append(.blob(Data()))
                    } else if let pointer = sqlite3_column_blob(statement, column) {
                        row.append(.blob(Data(bytes: pointer, count: count)))
                    } else {
                        row.append(.null)
                    }
                case SQLITE_TEXT:
                    row.append(.text(String(cString: sqlite3_column_text(statement, column))))
                case SQLITE_NULL:
                    row.append(.null)
                default:
                    throw KagemushaError.storage("unsupported column type")
                }
            }
            rows.append(row)
        }
        return rows
    }

    /// Run `body` in one `BEGIN IMMEDIATE` transaction. The commit is durable on return.
    func transaction<T>(_ body: () throws -> T) throws -> T {
        try execute("BEGIN IMMEDIATE")
        do {
            let result = try body()
            try execute("COMMIT")
            return result
        } catch {
            try? execute("ROLLBACK")
            throw error
        }
    }
}

/// Storage location policy: Application Support, excluded from backup, protected until first
/// unlock. Wallet state never goes to CloudKit or any synchronizing store.
enum KagemushaAttestedStorageLocation {
    static let rootName = "kagemusha-attested"
    static let databaseName = "wallet.sqlite"

    static func directory(root: URL?, schemeId: Data, accountDigest: Data) throws -> URL {
        let base: URL
        if let root {
            base = root
        } else {
            base = try FileManager.default.url(
                for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
        }
        return base.appendingPathComponent(rootName, isDirectory: true)
            .appendingPathComponent(schemeId.kagemushaHex, isDirectory: true)
            .appendingPathComponent(accountDigest.kagemushaHex, isDirectory: true)
    }

    static func databaseExists(in directory: URL) -> Bool {
        FileManager.default.fileExists(atPath: directory.appendingPathComponent(databaseName).path)
    }

    /// Create the directory with backup exclusion and file protection applied to every level
    /// this suite owns.
    static func prepare(_ directory: URL) throws {
        let manager = FileManager.default
        var attributes: [FileAttributeKey: Any] = [:]
        #if os(iOS) || os(tvOS) || os(watchOS) || os(visionOS)
        attributes[.protectionKey] = FileProtectionType.completeUntilFirstUserAuthentication
        #endif
        do {
            try manager.createDirectory(at: directory, withIntermediateDirectories: true, attributes: attributes)
            var owned = directory
            for _ in 0..<3 {
                try excludeFromBackup(owned)
                owned.deleteLastPathComponent()
            }
        } catch let error as KagemushaError {
            throw error
        } catch {
            throw KagemushaError.storage("cannot prepare wallet directory: \(error.localizedDescription)")
        }
    }

    /// Apply protection and backup exclusion to the database and its WAL side files.
    static func protectFiles(in directory: URL) {
        for suffix in ["", "-wal", "-shm"] {
            let url = directory.appendingPathComponent(databaseName + suffix)
            guard FileManager.default.fileExists(atPath: url.path) else { continue }
            #if os(iOS) || os(tvOS) || os(watchOS) || os(visionOS)
            try? FileManager.default.setAttributes(
                [.protectionKey: FileProtectionType.completeUntilFirstUserAuthentication],
                ofItemAtPath: url.path)
            #endif
            try? excludeFromBackup(url)
        }
    }

    static func excludeFromBackup(_ url: URL) throws {
        var mutable = url
        var values = URLResourceValues()
        values.isExcludedFromBackup = true
        do {
            try mutable.setResourceValues(values)
        } catch {
            throw KagemushaError.storage("cannot exclude wallet state from backup: \(error.localizedDescription)")
        }
    }

    /// Permanently remove the wallet state for one scheme and account.
    static func remove(_ directory: URL) throws {
        guard FileManager.default.fileExists(atPath: directory.path) else { return }
        do {
            try FileManager.default.removeItem(at: directory)
        } catch {
            throw KagemushaError.storage("cannot remove wallet state: \(error.localizedDescription)")
        }
    }
}
