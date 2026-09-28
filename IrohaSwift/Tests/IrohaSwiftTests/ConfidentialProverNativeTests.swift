import Foundation
import XCTest
import IrohaSwift

/// Requires the current authenticated native artifact; never substitutes a mock or skips it.
final class ConfidentialProverNativeTests: XCTestCase {
    func testRetainedChangeCanBeRedeemedWithItsDefaultOwner() async throws {
        let asset = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
        let key = Data(repeating: 9, count: 32)
        let rho = Data(repeating: 10, count: 32)
        let diversifier = Data(repeating: 7, count: 32)
        let owner = try ConfidentialOwnerTag.deriveFromSpendKeyWithDiversifier(key, diversifier: diversifier)
        let commitment = try ConfidentialNoteCommitment.derive(asset: asset, amount: 7, rho: rho, ownerTag: owner)
        let provider = try LocalZkAssetMerklePathProvider(rootHistory: [], commitmentHistory: [commitment])
        let path = try await provider.getMerklePathForCommitment(asset: asset, commitment: commitment)
        let prover = try ConfidentialProver(networkId: TestNetworkIds.canonical,
                                           assetDefinitionId: asset, spendKey: key)
        defer { try? prover.close() }
        let input = try ConfidentialInput(amount: 7, rho: rho, diversifier: diversifier, leafIndex: 0)
        // Retain the opening before proving. This test's next tree is local;
        // a wallet must authenticate its real change index and root separately.
        let change = try ConfidentialChange(amount: 3, rho: Data(repeating: 11, count: 32))
        let partial = try await prover.proveUnshield(tree: .paths(root: path.rootAtHeight, paths: [path]),
                                                     inputs: [input], publicAmount: 4, change: change)
        XCTAssertEqual(partial.relation, .redemptionWithChange)
        let nextInput = try change.asInput(leafIndex: 0)
        XCTAssertNotEqual(nextInput.diversifier, diversifier)
        let changeOwner = try ConfidentialOwnerTag.deriveFromSpendKeyWithDiversifier(
            key, diversifier: nextInput.diversifier)
        let expected = try ConfidentialNoteCommitment.derive(asset: asset, amount: 3,
                                                            rho: change.rho, ownerTag: changeOwner)
        XCTAssertEqual(partial.outputCommitments, [expected])
        let nextProvider = try LocalZkAssetMerklePathProvider(rootHistory: [], commitmentHistory: [expected])
        let nextPath = try await nextProvider.getMerklePathForCommitment(asset: asset, commitment: expected)
        let redeemed = try await prover.proveUnshield(
            tree: .paths(root: nextPath.rootAtHeight, paths: [nextPath]), inputs: [nextInput], publicAmount: 3)
        XCTAssertEqual(redeemed.relation, .fullRedemption)
        XCTAssertEqual(redeemed.root, nextPath.rootAtHeight)
        XCTAssertTrue(redeemed.outputCommitments.isEmpty)
    }

    func testRealRedemptionThroughPublicNativeBridge() async throws {
        let asset = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
        var key = Data(repeating: 2, count: 32)
        let rho = Data(repeating: 4, count: 32)
        let diversifier = try ConfidentialOwnerTag.defaultDiversifier()
        let owner = try ConfidentialOwnerTag.deriveFromSpendKeyWithDiversifier(key, diversifier: diversifier)
        let commitment = try ConfidentialNoteCommitment.derive(asset: asset, amount: 7, rho: rho, ownerTag: owner)
        let provider = try LocalZkAssetMerklePathProvider(rootHistory: [], commitmentHistory: [commitment])
        let path = try await provider.getMerklePathForCommitment(asset: asset, commitment: commitment)
        let prover = try ConfidentialProver(networkId: TestNetworkIds.canonical,
                                           assetDefinitionId: asset, spendKey: key)
        key.resetBytes(in: 0..<key.count)
        defer { try? prover.close() }
        let input = try ConfidentialInput(amount: 7, rho: rho, diversifier: diversifier, leafIndex: 0)
        let tree = ConfidentialTree.paths(root: path.rootAtHeight, paths: [path])
        let proof = try await prover.proveUnshield(tree: tree, inputs: [input], publicAmount: 7)
        XCTAssertEqual(proof.relation, .fullRedemption)
        XCTAssertEqual(proof.backend, "halo2/ipa")
        XCTAssertFalse(proof.proof.isEmpty)
        XCTAssertEqual(proof.root, path.rootAtHeight)
        XCTAssertEqual(proof.nullifiers.count, 1)
        XCTAssertTrue(proof.outputCommitments.isEmpty)
        do {
            _ = try await prover.proveUnshield(tree: tree, inputs: [input], publicAmount: 8)
            XCTFail("native preflight accepted an unconserved redemption")
        } catch { XCTAssertEqual(error as? ConfidentialProverError, .native(code: -21)) }
        try prover.close()
        do {
            _ = try await prover.proveUnshield(tree: tree, inputs: [input], publicAmount: 7)
            XCTFail("closed native owner accepted new work")
        } catch { XCTAssertEqual(error as? ConfidentialProverError, .closed) }
    }
}
