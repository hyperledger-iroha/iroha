import Foundation
import IrohaSwift
import Security

/// A disposable, locally verified proof. This example submits no transaction.
@main
enum ConfidentialRedemptionExample {
    static func main() async throws {
        // Applications obtain network, canonical asset and root from authenticated state.
        var networkBytes = try random32()
        networkBytes[31] |= 1
        let network = try NetworkId(bytes: networkBytes)
        var uuid = UUID().uuid
        let assetBytes = withUnsafeBytes(of: &uuid) { Data($0) }
        guard let asset = AssetDefinitionAddressCodec.definitionLiteral(uuidBytes: assetBytes) else {
            throw ConfidentialProverError.invalidInput("example asset")
        }
        var key = try random32()
        defer { key.resetBytes(in: 0..<key.count) }
        let rho = try random32()
        let diversifier = try ConfidentialOwnerTag.deriveDiversifier(random32())
        let owner = try ConfidentialOwnerTag.deriveFromSpendKeyWithDiversifier(key, diversifier: diversifier)
        let commitment = try ConfidentialNoteCommitment.derive(asset: asset, amount: 7, rho: rho, ownerTag: owner)
        let provider = try LocalZkAssetMerklePathProvider(rootHistory: [], commitmentHistory: [commitment])
        let path = try await provider.getMerklePathForCommitment(asset: asset, commitment: commitment)
        let prover = try ConfidentialProver(networkId: network, assetDefinitionId: asset, spendKey: key)
        key.resetBytes(in: 0..<key.count)
        defer { try? prover.close() }
        let input = try ConfidentialInput(amount: 7, rho: rho, diversifier: diversifier, leafIndex: 0)
        let proof = try await prover.proveUnshield(
            tree: .paths(root: path.rootAtHeight, paths: [path]), inputs: [input], publicAmount: 7
        )
        print("Locally verified \(proof.relation): \(proof.proof.count) bytes, \(proof.nullifiers.count) input note.")
    }

    private static func random32() throws -> Data {
        var bytes = Data(count: 32)
        let status = bytes.withUnsafeMutableBytes { SecRandomCopyBytes(kSecRandomDefault, $0.count, $0.baseAddress!) }
        guard status == errSecSuccess else {
            // Clear bytes that an unsuccessful entropy call may have partially filled.
            // This does not guarantee erasure of other Swift-managed copies.
            bytes.resetBytes(in: 0..<bytes.count)
            throw ConfidentialNoteError.cryptographyFailed
        }
        return bytes
    }
}
