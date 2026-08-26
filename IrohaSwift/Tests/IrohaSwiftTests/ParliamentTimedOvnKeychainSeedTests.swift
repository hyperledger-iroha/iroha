// Tests for generation-bound Parliament timed-OVN Keychain custody.

import Foundation
import XCTest
@testable import IrohaSwift

final class ParliamentTimedOvnKeychainSeedTests: XCTestCase {
    func testCreateOpenAndBorrowExactSeed() throws {
        let backend = InMemoryParliamentTimedOvnSeedStorage()
        let random = DeterministicParliamentTimedOvnRandom()
        let store = ParliamentTimedOvnKeychainSeedStore(
            storage: backend,
            randomBytes: { count in try random.next(count: count) }
        )
        let created = try store.create(alias: "alice")
        let opened = try store.open(alias: "alice")
        XCTAssertEqual(created, opened)
        let seed = try opened.withUnsafeSeedBytes { Data($0) }
        XCTAssertEqual(seed.count, 32)
        XCTAssertEqual(seed, Data(repeating: 0x22, count: 32))
    }

    func testDeleteAndRecreateNeverRetargetsStaleHandle() throws {
        let backend = InMemoryParliamentTimedOvnSeedStorage()
        let random = DeterministicParliamentTimedOvnRandom()
        let store = ParliamentTimedOvnKeychainSeedStore(
            storage: backend,
            randomBytes: { count in try random.next(count: count) }
        )
        let old = try store.create(alias: "voter")
        try store.delete(old)
        let replacement = try store.create(alias: "voter")
        XCTAssertNotEqual(old, replacement)
        XCTAssertThrowsError(try old.withUnsafeSeedBytes { Data($0) }) { error in
            XCTAssertEqual(error as? ParliamentTimedOvnKeychainSeedError, .staleHandle)
        }
        XCTAssertThrowsError(try store.delete(old)) { error in
            XCTAssertEqual(error as? ParliamentTimedOvnKeychainSeedError, .staleHandle)
        }
        XCTAssertEqual(
            try replacement.withUnsafeSeedBytes { Data($0) },
            Data(repeating: 0x44, count: 32)
        )
    }

    func testAliasSwapFailsEnvelopeBinding() throws {
        let backend = InMemoryParliamentTimedOvnSeedStorage()
        let random = DeterministicParliamentTimedOvnRandom()
        let store = ParliamentTimedOvnKeychainSeedStore(
            storage: backend,
            randomBytes: { count in try random.next(count: count) }
        )
        let alice = try store.create(alias: "alice")
        _ = try store.create(alias: "bob")
        backend.swap(alias: "alice", with: "bob")
        XCTAssertThrowsError(try alice.withUnsafeSeedBytes { Data($0) }) { error in
            XCTAssertEqual(error as? ParliamentTimedOvnKeychainSeedError, .staleHandle)
        }
    }

    func testDeleteWaitsForInFlightSeedBorrow() throws {
        let backend = InMemoryParliamentTimedOvnSeedStorage()
        let random = DeterministicParliamentTimedOvnRandom()
        let store = ParliamentTimedOvnKeychainSeedStore(
            storage: backend,
            randomBytes: { count in try random.next(count: count) }
        )
        let handle = try store.create(alias: "serialized")
        let entered = expectation(description: "seed borrow entered")
        let release = DispatchSemaphore(value: 0)
        let operationDone = expectation(description: "seed borrow finished")
        DispatchQueue.global().async {
            defer { operationDone.fulfill() }
            _ = try? handle.withUnsafeSeedBytes { seed in
                XCTAssertEqual(seed.count, 32)
                entered.fulfill()
                release.wait()
                return Data()
            }
        }
        wait(for: [entered], timeout: 1)

        let deleteDone = expectation(description: "delete finished after borrow")
        DispatchQueue.global().async {
            defer { deleteDone.fulfill() }
            try? store.delete(handle)
        }
        let earlyDelete = XCTWaiter.wait(for: [deleteDone], timeout: 0.05)
        XCTAssertEqual(earlyDelete, .timedOut)
        release.signal()
        wait(for: [operationDone, deleteDone], timeout: 1)
        XCTAssertThrowsError(try handle.withUnsafeSeedBytes { Data($0) }) { error in
            XCTAssertEqual(error as? ParliamentTimedOvnKeychainSeedError, .staleHandle)
        }
    }

    func testRejectsNoncanonicalAliasAndForeignHandle() throws {
        let firstBackend = InMemoryParliamentTimedOvnSeedStorage()
        let secondBackend = InMemoryParliamentTimedOvnSeedStorage()
        let firstRandom = DeterministicParliamentTimedOvnRandom()
        let secondRandom = DeterministicParliamentTimedOvnRandom()
        let first = ParliamentTimedOvnKeychainSeedStore(
            storage: firstBackend,
            randomBytes: { count in try firstRandom.next(count: count) }
        )
        let second = ParliamentTimedOvnKeychainSeedStore(
            storage: secondBackend,
            randomBytes: { count in try secondRandom.next(count: count) }
        )
        XCTAssertThrowsError(try first.create(alias: " bad")) { error in
            XCTAssertEqual(error as? ParliamentTimedOvnKeychainSeedError, .invalidAlias)
        }
        XCTAssertThrowsError(try first.create(alias: "votér")) { error in
            XCTAssertEqual(error as? ParliamentTimedOvnKeychainSeedError, .invalidAlias)
        }
        let handle = try first.create(alias: "valid")
        XCTAssertThrowsError(try second.delete(handle)) { error in
            XCTAssertEqual(error as? ParliamentTimedOvnKeychainSeedError, .foreignHandle)
        }
    }

    func testDeleteIsGenerationConditionalAcrossProcessReplacement() throws {
        let backend = InMemoryParliamentTimedOvnSeedStorage()
        let random = DeterministicParliamentTimedOvnRandom()
        let store = ParliamentTimedOvnKeychainSeedStore(
            storage: backend,
            randomBytes: { count in try random.next(count: count) }
        )
        let stale = try store.create(alias: "shared-extension")
        backend.replaceOnNextDelete = true
        XCTAssertThrowsError(try store.delete(stale)) { error in
            XCTAssertEqual(error as? ParliamentTimedOvnKeychainSeedError, .staleHandle)
        }
        let replacement = try store.open(alias: "shared-extension")
        XCTAssertNotEqual(stale, replacement)
        XCTAssertEqual(
            try replacement.withUnsafeSeedBytes { Data($0) },
            Data(repeating: 0xEF, count: 32)
        )
    }
}

private final class InMemoryParliamentTimedOvnSeedStorage:
    ParliamentTimedOvnSeedStorage,
    @unchecked Sendable
{
    private struct StoredItem {
        var data: Data
        var generation: Data
    }

    private var values: [String: StoredItem] = [:]
    var replaceOnNextDelete = false

    func load(service: String, alias: String, accessGroup: String?) throws -> Data? {
        values[key(service: service, alias: alias, accessGroup: accessGroup)]?.data
    }

    func insert(
        _ data: Data,
        generation: Data,
        service: String,
        alias: String,
        accessGroup: String?
    ) throws {
        let key = key(service: service, alias: alias, accessGroup: accessGroup)
        guard values[key] == nil else {
            throw ParliamentTimedOvnKeychainSeedError.aliasAlreadyExists
        }
        values[key] = StoredItem(data: data, generation: generation)
    }

    func delete(
        service: String,
        alias: String,
        accessGroup: String?,
        generation: Data
    ) throws {
        let key = key(service: service, alias: alias, accessGroup: accessGroup)
        if replaceOnNextDelete {
            let replacementGeneration = Data(repeating: 0xEE, count: 16)
            values[key] = StoredItem(
                data: ParliamentTimedOvnSeedEnvelope.make(
                    service: service,
                    alias: alias,
                    generation: replacementGeneration,
                    seed: Data(repeating: 0xEF, count: 32)
                ),
                generation: replacementGeneration
            )
            replaceOnNextDelete = false
        }
        guard values[key]?.generation == generation else {
            throw ParliamentTimedOvnKeychainSeedError.staleHandle
        }
        values.removeValue(forKey: key)
    }

    func swap(alias lhs: String, with rhs: String) {
        let service = "org.hyperledger.iroha.parliament-timed-ovn-seed.v1"
        let lhsKey = key(service: service, alias: lhs, accessGroup: nil)
        let rhsKey = key(service: service, alias: rhs, accessGroup: nil)
        let value = values[lhsKey]
        values[lhsKey] = values[rhsKey]
        values[rhsKey] = value
    }

    private func key(service: String, alias: String, accessGroup: String?) -> String {
        "\(service)|\(accessGroup ?? "")|\(alias)"
    }
}

private final class DeterministicParliamentTimedOvnRandom: @unchecked Sendable {
    private var call = 0
    private let lock = NSLock()

    func next(count: Int) throws -> Data {
        lock.lock()
        defer { lock.unlock() }
        call += 1
        return Data(repeating: UInt8(truncatingIfNeeded: call * 0x11), count: count)
    }
}
