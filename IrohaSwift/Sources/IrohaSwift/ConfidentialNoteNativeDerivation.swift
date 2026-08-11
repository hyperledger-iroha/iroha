import Foundation

/// Decoded output of the Rust-owned fixed-tree V3 path derivation.
struct ConfidentialNativeMerklePath {
    let root: Data
    let siblings: [Data]
    let directions: Data
}

/// Result of appending one commitment to an authenticated next-zero path.
struct ConfidentialNativeMerkleAdvance {
    let finalRoot: Data
    let nextZeroPath: ConfidentialNativeMerklePath
}

/// Native-only boundary for confidential V3 note and Merkle derivation.
///
/// The complete Poseidon permutation and every domain constant remain owned by
/// `iroha_core`; Swift intentionally has no local cryptographic substitute.
enum ConfidentialNoteNativeDerivation {
    static let contractRevisionV3: UInt32 = 1
    static let treeDepth = 16
    static let treeCapacity = 1 << treeDepth

    private static let digestBytes = 32
    private static let pathBytes = digestBytes + treeDepth * digestBytes + treeDepth
    private static let advanceBytes = digestBytes + pathBytes

    #if canImport(Darwin)
    private typealias RevisionFn = @convention(c) () -> UInt32
    private typealias DefaultDigestFn = @convention(c) (
        UnsafeMutablePointer<UInt8>?, CUnsignedLong
    ) -> Int32
    private typealias OneInputDigestFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UInt8>?, CUnsignedLong
    ) -> Int32
    private typealias TwoInputDigestFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UInt8>?, CUnsignedLong
    ) -> Int32
    private typealias FourInputDigestFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UInt8>?, CUnsignedLong
    ) -> Int32
    private typealias DerivePathFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong, UInt64,
        UnsafeMutablePointer<UInt8>?, CUnsignedLong
    ) -> Int32
    private typealias VerifyPathFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong, UInt64,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong
    ) -> Int32
    private typealias AdvancePathFn = @convention(c) (
        UInt64,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UInt8>?, CUnsignedLong
    ) -> Int32
    #endif

    static func loadedContractRevisionV3() -> UInt32? {
        #if canImport(Darwin)
        let function: RevisionFn? = resolve(
            "connect_norito_confidential_note_derivation_revision_v3",
            as: RevisionFn.self
        )
        return function?()
        #else
        return nil
        #endif
    }

    static func defaultDiversifierV3() throws -> Data {
        #if canImport(Darwin)
        try requireContract()
        guard let function: DefaultDigestFn = resolve(
            "connect_norito_confidential_default_diversifier_v3",
            as: DefaultDigestFn.self
        ) else {
            throw ConfidentialNoteError.bridgeUnavailable
        }
        var output = Data(count: digestBytes)
        let outputCount = output.count
        let status = output.withUnsafeMutableBytes { raw in
            function(raw.bindMemory(to: UInt8.self).baseAddress, CUnsignedLong(outputCount))
        }
        return try requireDigest(output, status: status, field: "defaultDiversifier")
        #else
        throw ConfidentialNoteError.bridgeUnavailable
        #endif
    }

    static func deriveDiversifierV3(seed: Data) throws -> Data {
        try deriveOne(
            symbol: "connect_norito_confidential_diversifier_derive_v3",
            input: seed,
            field: "diversifier"
        )
    }

    static func deriveOwnerTagV3(spendKey: Data, diversifier: Data) throws -> Data {
        try deriveTwo(
            symbol: "connect_norito_confidential_owner_tag_derive_v3",
            first: spendKey,
            second: diversifier,
            field: "ownerTag"
        )
    }

    static func deriveAssetTagV3(asset: Data) throws -> Data {
        try deriveOne(
            symbol: "connect_norito_confidential_asset_tag_derive_v3",
            input: asset,
            field: "assetTag"
        )
    }

    static func deriveNetworkTagV3(networkID: Data) throws -> Data {
        try deriveOne(
            symbol: "connect_norito_confidential_network_tag_derive_v3",
            input: networkID,
            field: "networkTag"
        )
    }

    static func deriveNoteCommitmentV3(
        asset: Data,
        amount: Data,
        rho: Data,
        ownerTag: Data
    ) throws -> Data {
        try deriveFour(
            symbol: "connect_norito_confidential_note_commitment_derive_v3",
            first: asset,
            second: amount,
            third: rho,
            fourth: ownerTag,
            field: "noteCommitment"
        )
    }

    static func deriveNullifierV3(
        networkID: Data,
        asset: Data,
        spendKey: Data,
        rho: Data
    ) throws -> Data {
        try deriveFour(
            symbol: "connect_norito_confidential_nullifier_derive_v3",
            first: networkID,
            second: asset,
            third: spendKey,
            fourth: rho,
            field: "nullifier"
        )
    }

    static func deriveMerklePathV3(
        commitments: [Data],
        leafIndex: UInt64
    ) throws -> ConfidentialNativeMerklePath {
        #if canImport(Darwin)
        try requireContract()
        guard commitments.count <= treeCapacity,
              leafIndex <= UInt64(commitments.count),
              leafIndex < UInt64(treeCapacity) else {
            throw ZkAssetMerklePathError.invalidField("leafIndex")
        }
        var packed = Data()
        packed.reserveCapacity(commitments.count * digestBytes)
        for (index, commitment) in commitments.enumerated() {
            guard commitment.count == digestBytes,
                  commitment.contains(where: { $0 != 0 }) else {
                throw ZkAssetMerklePathError.invalidField("commitments[\(index)]")
            }
            packed.append(commitment)
        }
        guard let function: DerivePathFn = resolve(
            "connect_norito_confidential_merkle_path_derive_v3",
            as: DerivePathFn.self
        ) else {
            throw ConfidentialNoteError.bridgeUnavailable
        }
        var output = Data(count: pathBytes)
        let outputCount = output.count
        let status = packed.withUnsafeBytes { inputRaw in
            output.withUnsafeMutableBytes { outputRaw in
                function(
                    inputRaw.bindMemory(to: UInt8.self).baseAddress,
                    CUnsignedLong(packed.count),
                    leafIndex,
                    outputRaw.bindMemory(to: UInt8.self).baseAddress,
                    CUnsignedLong(outputCount)
                )
            }
        }
        guard status == 0 else {
            throw ZkAssetMerklePathError.verificationFailed("nativeDerivation")
        }
        let siblings = (0..<treeDepth).map { level in
            let start = digestBytes + level * digestBytes
            return output.subdata(in: start..<(start + digestBytes))
        }
        let directionsStart = digestBytes + treeDepth * digestBytes
        return ConfidentialNativeMerklePath(
            root: output.subdata(in: 0..<digestBytes),
            siblings: siblings,
            directions: output.subdata(in: directionsStart..<output.count)
        )
        #else
        throw ConfidentialNoteError.bridgeUnavailable
        #endif
    }

    static func verifyMerklePathV3(
        commitment: Data,
        leafIndex: UInt64,
        siblings: [Data],
        directions: Data,
        root: Data
    ) throws -> Bool {
        #if canImport(Darwin)
        try requireContract()
        guard commitment.count == digestBytes,
              siblings.count == treeDepth,
              directions.count == treeDepth,
              root.count == digestBytes else {
            return false
        }
        var packedSiblings = Data()
        packedSiblings.reserveCapacity(treeDepth * digestBytes)
        for sibling in siblings {
            guard sibling.count == digestBytes else { return false }
            packedSiblings.append(sibling)
        }
        guard let function: VerifyPathFn = resolve(
            "connect_norito_confidential_merkle_path_verify_v3",
            as: VerifyPathFn.self
        ) else {
            throw ConfidentialNoteError.bridgeUnavailable
        }
        return commitment.withUnsafeBytes { commitmentRaw in
            packedSiblings.withUnsafeBytes { siblingsRaw in
                directions.withUnsafeBytes { directionsRaw in
                    root.withUnsafeBytes { rootRaw in
                        function(
                            commitmentRaw.bindMemory(to: UInt8.self).baseAddress,
                            CUnsignedLong(commitment.count),
                            leafIndex,
                            siblingsRaw.bindMemory(to: UInt8.self).baseAddress,
                            CUnsignedLong(packedSiblings.count),
                            directionsRaw.bindMemory(to: UInt8.self).baseAddress,
                            CUnsignedLong(directions.count),
                            rootRaw.bindMemory(to: UInt8.self).baseAddress,
                            CUnsignedLong(root.count)
                        ) == 0
                    }
                }
            }
        }
        #else
        throw ConfidentialNoteError.bridgeUnavailable
        #endif
    }

    static func advanceMerklePathV3(
        leafIndex: UInt64,
        siblings: [Data],
        directions: Data,
        root: Data,
        commitment: Data
    ) throws -> ConfidentialNativeMerkleAdvance {
        #if canImport(Darwin)
        try requireContract()
        guard leafIndex < UInt64(treeCapacity),
              siblings.count == treeDepth,
              directions.count == treeDepth,
              root.count == digestBytes,
              commitment.count == digestBytes,
              commitment.contains(where: { $0 != 0 }) else {
            throw ZkAssetMerklePathError.invalidField("nativeAdvance")
        }
        var packedSiblings = Data()
        packedSiblings.reserveCapacity(treeDepth * digestBytes)
        for sibling in siblings {
            guard sibling.count == digestBytes else {
                throw ZkAssetMerklePathError.invalidField("siblings")
            }
            packedSiblings.append(sibling)
        }
        guard let function: AdvancePathFn = resolve(
            "connect_norito_confidential_merkle_path_advance_v3",
            as: AdvancePathFn.self
        ) else {
            throw ConfidentialNoteError.bridgeUnavailable
        }
        var output = Data(count: advanceBytes)
        let outputCount = output.count
        let status = packedSiblings.withUnsafeBytes { siblingsRaw in
            directions.withUnsafeBytes { directionsRaw in
                root.withUnsafeBytes { rootRaw in
                    commitment.withUnsafeBytes { commitmentRaw in
                        output.withUnsafeMutableBytes { outputRaw in
                            function(
                                leafIndex,
                                siblingsRaw.bindMemory(to: UInt8.self).baseAddress,
                                CUnsignedLong(packedSiblings.count),
                                directionsRaw.bindMemory(to: UInt8.self).baseAddress,
                                CUnsignedLong(directions.count),
                                rootRaw.bindMemory(to: UInt8.self).baseAddress,
                                CUnsignedLong(root.count),
                                commitmentRaw.bindMemory(to: UInt8.self).baseAddress,
                                CUnsignedLong(commitment.count),
                                outputRaw.bindMemory(to: UInt8.self).baseAddress,
                                CUnsignedLong(outputCount)
                            )
                        }
                    }
                }
            }
        }
        guard status == 0 else {
            throw ZkAssetMerklePathError.verificationFailed("nativeAdvance")
        }
        let finalRoot = output.subdata(in: 0..<digestBytes)
        let nextRootStart = digestBytes
        let siblingsStart = nextRootStart + digestBytes
        let nextSiblings = (0..<treeDepth).map { level in
            let start = siblingsStart + level * digestBytes
            return output.subdata(in: start..<(start + digestBytes))
        }
        let directionsStart = siblingsStart + treeDepth * digestBytes
        return ConfidentialNativeMerkleAdvance(
            finalRoot: finalRoot,
            nextZeroPath: ConfidentialNativeMerklePath(
                root: output.subdata(in: nextRootStart..<siblingsStart),
                siblings: nextSiblings,
                directions: output.subdata(in: directionsStart..<output.count)
            )
        )
        #else
        throw ConfidentialNoteError.bridgeUnavailable
        #endif
    }

    #if canImport(Darwin)
    private static func requireContract() throws {
        guard loadedContractRevisionV3() == contractRevisionV3 else {
            throw ConfidentialNoteError.bridgeUnavailable
        }
    }

    private static func resolve<T>(_ symbol: String, as type: T.Type) -> T? {
        NoritoNativeBridge.shared.resolveKagemushaV2Symbol(symbol, as: type)
    }

    private static func deriveOne(symbol: String, input: Data, field: String) throws -> Data {
        try requireContract()
        guard let function: OneInputDigestFn = resolve(symbol, as: OneInputDigestFn.self) else {
            throw ConfidentialNoteError.bridgeUnavailable
        }
        var output = Data(count: digestBytes)
        let outputCount = output.count
        let status = input.withUnsafeBytes { inputRaw in
            output.withUnsafeMutableBytes { outputRaw in
                function(
                    inputRaw.bindMemory(to: UInt8.self).baseAddress,
                    CUnsignedLong(input.count),
                    outputRaw.bindMemory(to: UInt8.self).baseAddress,
                    CUnsignedLong(outputCount)
                )
            }
        }
        return try requireDigest(output, status: status, field: field)
    }

    private static func deriveTwo(
        symbol: String,
        first: Data,
        second: Data,
        field: String
    ) throws -> Data {
        try requireContract()
        guard let function: TwoInputDigestFn = resolve(symbol, as: TwoInputDigestFn.self) else {
            throw ConfidentialNoteError.bridgeUnavailable
        }
        var output = Data(count: digestBytes)
        let outputCount = output.count
        let status = first.withUnsafeBytes { firstRaw in
            second.withUnsafeBytes { secondRaw in
                output.withUnsafeMutableBytes { outputRaw in
                    function(
                        firstRaw.bindMemory(to: UInt8.self).baseAddress,
                        CUnsignedLong(first.count),
                        secondRaw.bindMemory(to: UInt8.self).baseAddress,
                        CUnsignedLong(second.count),
                        outputRaw.bindMemory(to: UInt8.self).baseAddress,
                        CUnsignedLong(outputCount)
                    )
                }
            }
        }
        return try requireDigest(output, status: status, field: field)
    }

    private static func deriveFour(
        symbol: String,
        first: Data,
        second: Data,
        third: Data,
        fourth: Data,
        field: String
    ) throws -> Data {
        try requireContract()
        guard let function: FourInputDigestFn = resolve(symbol, as: FourInputDigestFn.self) else {
            throw ConfidentialNoteError.bridgeUnavailable
        }
        var output = Data(count: digestBytes)
        let outputCount = output.count
        let status = first.withUnsafeBytes { firstRaw in
            second.withUnsafeBytes { secondRaw in
                third.withUnsafeBytes { thirdRaw in
                    fourth.withUnsafeBytes { fourthRaw in
                        output.withUnsafeMutableBytes { outputRaw in
                            function(
                                firstRaw.bindMemory(to: UInt8.self).baseAddress,
                                CUnsignedLong(first.count),
                                secondRaw.bindMemory(to: UInt8.self).baseAddress,
                                CUnsignedLong(second.count),
                                thirdRaw.bindMemory(to: UInt8.self).baseAddress,
                                CUnsignedLong(third.count),
                                fourthRaw.bindMemory(to: UInt8.self).baseAddress,
                                CUnsignedLong(fourth.count),
                                outputRaw.bindMemory(to: UInt8.self).baseAddress,
                                CUnsignedLong(outputCount)
                            )
                        }
                    }
                }
            }
        }
        return try requireDigest(output, status: status, field: field)
    }

    private static func requireDigest(
        _ digest: Data,
        status: Int32,
        field: String
    ) throws -> Data {
        guard status == 0,
              digest.count == digestBytes,
              digest.contains(where: { $0 != 0 }) else {
            throw ConfidentialNoteError.invalidField(field)
        }
        return digest
    }
    #else
    private static func deriveOne(symbol _: String, input _: Data, field _: String) throws -> Data {
        throw ConfidentialNoteError.bridgeUnavailable
    }

    private static func deriveTwo(
        symbol _: String,
        first _: Data,
        second _: Data,
        field _: String
    ) throws -> Data {
        throw ConfidentialNoteError.bridgeUnavailable
    }

    private static func deriveFour(
        symbol _: String,
        first _: Data,
        second _: Data,
        third _: Data,
        fourth _: Data,
        field _: String
    ) throws -> Data {
        throw ConfidentialNoteError.bridgeUnavailable
    }
    #endif
}
