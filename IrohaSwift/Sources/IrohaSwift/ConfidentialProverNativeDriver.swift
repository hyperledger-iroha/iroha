import Foundation

#if canImport(Darwin)
/// Resolves the exact current contract through the SDK's authenticated bridge loader.
/// Function pointers are immutable; native owner/job registries synchronize their handles.
final class ConfidentialProverNativeDriver: ConfidentialProverDriver, @unchecked Sendable {
    private typealias Revision = @convention(c) () -> UInt32
    private typealias Create = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong, UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong, UnsafeMutablePointer<UInt64>?
    ) -> Int32
    private typealias Close = @convention(c) (UInt64) -> Int32
    private typealias JobCreate = @convention(c) (
        UInt64, UInt8, UnsafePointer<UInt8>?, CUnsignedLong, UInt64, UInt64,
        UnsafeMutablePointer<UInt64>?
    ) -> Int32
    private typealias Input = @convention(c) (
        UInt64, UInt64, UInt64, UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong, UInt64
    ) -> Int32
    private typealias Output = @convention(c) (
        UInt64, UInt64, UInt64, UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong
    ) -> Int32
    private typealias Commitments = @convention(c) (UInt64, UnsafePointer<UInt8>?, CUnsignedLong) -> Int32
    private typealias Paths = @convention(c) (
        UInt64, UnsafePointer<UInt8>?, CUnsignedLong, UnsafePointer<UInt8>?, CUnsignedLong
    ) -> Int32
    private typealias Prove = @convention(c) (
        UInt64, UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias Free = @convention(c) (UnsafeMutablePointer<UInt8>?) -> Void
    private let createFn: Create
    private let closeFn: Close
    private let jobCreateFn: JobCreate
    private let inputFn: Input
    private let outputFn: Output
    private let commitmentsFn: Commitments
    private let pathsFn: Paths
    private let proveFn: Prove
    private let closeJobFn: Close
    private let freeFn: Free

    init() throws {
        func symbol<T>(_ suffix: String, as type: T.Type) throws -> T {
            guard let function = NoritoNativeBridge.shared.resolveNativeSymbol(
                "connect_norito_confidential_prover_\(suffix)_v1", as: type
            ) else { throw ConfidentialProverError.bridgeUnavailable }
            return function
        }
        let revision = try symbol("revision", as: Revision.self)
        guard revision() == 1,
              let free = NoritoNativeBridge.shared.resolveNativeSymbol("connect_norito_free", as: Free.self)
        else { throw ConfidentialProverError.bridgeUnavailable }
        createFn = try symbol("create", as: Create.self)
        closeFn = try symbol("close", as: Close.self)
        jobCreateFn = try symbol("job_create", as: JobCreate.self)
        inputFn = try symbol("job_input", as: Input.self)
        outputFn = try symbol("job_output", as: Output.self)
        commitmentsFn = try symbol("job_commitments", as: Commitments.self)
        pathsFn = try symbol("job_paths", as: Paths.self)
        proveFn = try symbol("job_prove", as: Prove.self)
        closeJobFn = try symbol("job_close", as: Close.self)
        freeFn = free
    }

    private func check(_ status: Int32) throws {
        if status == -2 { throw ConfidentialProverError.closed }
        if status != 0 { throw ConfidentialProverError.native(code: status) }
    }
    private func borrow<T>(_ data: Data, _ action: (UnsafePointer<UInt8>?, CUnsignedLong) throws -> T) rethrows -> T {
        try data.withUnsafeBytes { raw in
            try action(raw.bindMemory(to: UInt8.self).baseAddress, CUnsignedLong(data.count))
        }
    }
    func create(network: NetworkId, asset: String, key: Data) throws -> UInt64 {
        var handle: UInt64 = 0
        let status = borrow(network.bytes) { networkPtr, networkLen in
            borrow(Data(asset.utf8)) { assetPtr, assetLen in
                borrow(key) { keyPtr, keyLen in
                    createFn(networkPtr, networkLen, assetPtr, assetLen, keyPtr, keyLen, &handle)
                }
            }
        }
        try check(status)
        return handle
    }
    func close(_ handle: UInt64) throws { try check(closeFn(handle)) }
    func closeJob(_ job: UInt64) { _ = closeJobFn(job) }
    func createJob(_ handle: UInt64, transfer: Bool, root: Data, publicAmount: ConfidentialAmount) throws -> UInt64 {
        var job: UInt64 = 0
        let status = borrow(root) { rootPtr, rootLen in
            jobCreateFn(handle, transfer ? 0 : 1, rootPtr, rootLen,
                        publicAmount.low, publicAmount.high, &job)
        }
        try check(status)
        return job
    }
    func input(_ job: UInt64, note: ConfidentialInput) throws {
        let status = borrow(note.rho) { rho, rhoLen in
            borrow(note.diversifier) { diversifier, diversifierLen in
                inputFn(job, note.amount.low, note.amount.high, rho, rhoLen,
                        diversifier, diversifierLen, note.leafIndex)
            }
        }
        try check(status)
    }
    func output(_ job: UInt64, amount: ConfidentialAmount, rho: Data, owner: Data) throws {
        let status = borrow(rho) { rhoPtr, rhoLen in
            borrow(owner) { ownerPtr, ownerLen in
                outputFn(job, amount.low, amount.high, rhoPtr, rhoLen, ownerPtr, ownerLen)
            }
        }
        try check(status)
    }
    func evidence(_ job: UInt64, tree: ConfidentialTree) throws {
        switch tree {
        case let .commitments(_, leaves):
            var packed = Data()
            packed.reserveCapacity(leaves.count * 32)
            for leaf in leaves { packed.append(leaf) }
            defer { packed.resetBytes(in: 0..<packed.count) }
            try check(borrow(packed) { commitmentsFn(job, $0, $1) })
        case let .paths(_, paths):
            var siblings = Data()
            var directions = Data()
            siblings.reserveCapacity(paths.count * 16 * 32)
            directions.reserveCapacity(paths.count * 16)
            for path in paths {
                for sibling in path.siblings { siblings.append(sibling) }
                directions.append(path.directions)
            }
            defer {
                siblings.resetBytes(in: 0..<siblings.count)
                directions.resetBytes(in: 0..<directions.count)
            }
            let status = borrow(siblings) { siblingPtr, siblingLen in
                borrow(directions) { directionPtr, directionLen in
                    pathsFn(job, siblingPtr, siblingLen, directionPtr, directionLen)
                }
            }
            try check(status)
        }
    }
    func prove(_ job: UInt64) throws -> Data {
        var output: UnsafeMutablePointer<UInt8>?
        var count: CUnsignedLong = 0
        let status = proveFn(job, &output, &count)
        defer { if let output { freeFn(output) } }
        try check(status)
        guard let output, count > 0, count <= 16 * 1024 * 1024 else {
            throw ConfidentialProverError.invalidNativeOutput
        }
        return Data(bytes: output, count: Int(count))
    }
}
#endif
