import CryptoKit
import Foundation
import XCTest

@testable import IrohaSwift

final class PreparedTransactionSignatureV1Tests: XCTestCase {
  private struct Fixture: Decodable {
    let schema: String
    let signatureDomainHex: String
    let transcriptSchema: String
    let frameLengthEncoding: String
    let digestAlgorithm: String
    let vectors: [Vector]

    private enum CodingKeys: String, CodingKey {
      case schema, vectors
      case signatureDomainHex = "signature_domain_hex"
      case transcriptSchema = "transcript_schema"
      case frameLengthEncoding = "frame_length_encoding"
      case digestAlgorithm = "digest_algorithm"
    }
  }

  private struct Vector: Decodable {
    let name: String
    let signerPublicKey: String
    let signerAccountId: String
    let networkId: NetworkId
    let response: ToriiJSONValue
    let transcriptHex: String
    let digestHex: String
    let serverSignatureHex: String

    private enum CodingKeys: String, CodingKey {
      case name, response
      case signerPublicKey = "signer_public_key"
      case signerAccountId = "signer_account_id"
      case networkId = "network_id"
      case transcriptHex = "transcript_hex"
      case digestHex = "digest_hex"
      case serverSignatureHex = "server_signature_hex"
    }
  }

  func testRustGoldenAuthenticatesEverySwiftPreparedResponse() throws {
    let fixture = try loadFixture()
    XCTAssertEqual(
      fixture.schema,
      "iroha.taira.prepared-transaction-signature-fixture.v1"
    )
    XCTAssertEqual(
      fixture.signatureDomainHex,
      ToriiPreparedAccountProtocolV1.signatureDomain.hexEncodedString()
    )
    XCTAssertEqual(
      fixture.transcriptSchema,
      ToriiPreparedAccountProtocolV1.signatureTranscriptSchema
    )
    XCTAssertEqual(fixture.frameLengthEncoding, "u64_be")
    XCTAssertEqual(fixture.digestAlgorithm, "iroha_blake2b_256")
    XCTAssertEqual(
      Set(fixture.vectors.map(\.name)),
      ["onboarding_prepared", "onboarding_proof_required", "faucet_prepared"]
    )

    let onboardingVector = try XCTUnwrap(
      fixture.vectors.first { $0.name == "onboarding_prepared" }
    )
    let onboarding = try decode(
      ToriiAccountOnboardingPreparedTransactionV1.self,
      response: onboardingVector.response
    )
    XCTAssertEqual(onboarding.receipt.body.networkId, onboardingVector.networkId)
    XCTAssertEqual(onboarding.receipt.body.authority, onboardingVector.signerAccountId)
    let onboardingBody = try ToriiAccountOnboardingPlanBodyNorito.encode(
      onboarding.receipt.body
    )
    try ToriiAccountOnboardingReceiptVerifier.verify(
      onboarding.receipt,
      for: onboarding.receipt.body.request,
      canonicalBodyNorito: onboardingBody,
      expectedAuthority: onboardingVector.signerAccountId,
      expectedNetworkId: onboardingVector.networkId
    )
    let onboardingSemanticHash = try ToriiAccountOnboardingReceiptVerifier.canonicalHash(
      canonicalBodyNorito: onboardingBody
    ).hexEncodedString()
    XCTAssertEqual(onboarding.semanticHashHex, onboardingSemanticHash)
    try assertTranscript(
      onboarding.signatureTranscript(),
      serverSignature: onboarding.serverSignature,
      vector: onboardingVector
    )
    try onboarding.validate(
      receipt: onboarding.receipt,
      binding: onboarding.binding,
      semanticHashHex: onboardingSemanticHash,
      expectedAuthority: onboardingVector.signerAccountId,
      expectedNetworkId: onboardingVector.networkId
    )

    let proofRequiredVector = try XCTUnwrap(
      fixture.vectors.first { $0.name == "onboarding_proof_required" }
    )
    let proofRequired = try decode(
      ToriiAccountOnboardingProofRequiredPrepareResponseV1.self,
      response: proofRequiredVector.response
    )
    XCTAssertEqual(proofRequiredVector.networkId, onboardingVector.networkId)
    XCTAssertEqual(proofRequired.outcome, "ProofRequired")
    XCTAssertEqual(proofRequired.proofKind, "account_alias_current_state")
    try assertTranscript(
      proofRequired.signatureTranscript(),
      serverSignature: proofRequired.serverSignature,
      vector: proofRequiredVector
    )
    try proofRequired.validate(
      receipt: onboarding.receipt,
      binding: proofRequired.binding,
      semanticHashHex: proofRequired.semanticHashHex,
      expectedAuthority: proofRequiredVector.signerAccountId
    )

    let faucetVector = try XCTUnwrap(
      fixture.vectors.first { $0.name == "faucet_prepared" }
    )
    let faucet = try decode(
      ToriiAccountFaucetPreparedTransactionV1.self,
      response: faucetVector.response
    )
    try assertTranscript(
      faucet.signatureTranscript(),
      serverSignature: faucet.serverSignature,
      vector: faucetVector
    )
    let faucetWire = try XCTUnwrap(Data(hexString: faucet.signedTransactionWireHex))
    let inspected = try ToriiCanonicalTransactionDraft.inspectVersionedSignedTransaction(
      faucetWire,
      context: "prepared faucet golden"
    )
    XCTAssertEqual(faucetVector.networkId, onboardingVector.networkId)
    XCTAssertEqual(inspected.payload.networkId, faucetVector.networkId)
    try faucet.validate(
      claim: faucet.claim,
      binding: faucet.binding,
      expectedAuthority: faucetVector.signerAccountId,
      expectedNetworkId: faucetVector.networkId
    )
  }

  func testGoldenRejectsSignaturePinAndFeeSubstitution() throws {
    let fixture = try loadFixture()
    let vector = try XCTUnwrap(
      fixture.vectors.first { $0.name == "onboarding_prepared" }
    )
    let valid = try decode(
      ToriiAccountOnboardingPreparedTransactionV1.self,
      response: vector.response
    )

    var outerSignatureObject = try responseObject(vector.response)
    var outerSignature = try XCTUnwrap(outerSignatureObject["server_signature"] as? String)
    let last = outerSignature.removeLast()
    outerSignature.append(last == "0" ? "1" : "0")
    outerSignatureObject["server_signature"] = outerSignature
    let substitutedOuter = try JSONDecoder().decode(
      ToriiAccountOnboardingPreparedTransactionV1.self,
      from: JSONSerialization.data(withJSONObject: outerSignatureObject)
    )
    XCTAssertThrowsError(
      try substitutedOuter.validate(
        receipt: valid.receipt,
        binding: valid.binding,
        semanticHashHex: valid.semanticHashHex,
        expectedAuthority: vector.signerAccountId,
        expectedNetworkId: vector.networkId
      )
    )

    var innerSignatureObject = try responseObject(vector.response)
    let wireHex = try XCTUnwrap(innerSignatureObject["signed_transaction_wire_hex"] as? String)
    let wire = try XCTUnwrap(Data(hexString: wireHex))
    let substitutedWire = try zeroInnerSignatureScalar(wire)
    innerSignatureObject["signed_transaction_wire_hex"] = substitutedWire.hexEncodedString()
    innerSignatureObject["signed_transaction_wire_sha256"] = Data(
      SHA256.hash(data: substitutedWire)
    ).hexEncodedString()
    XCTAssertThrowsError(
      try JSONDecoder().decode(
        ToriiAccountOnboardingPreparedTransactionV1.self,
        from: JSONSerialization.data(withJSONObject: innerSignatureObject)
      )
    )

    let faucetVector = try XCTUnwrap(
      fixture.vectors.first { $0.name == "faucet_prepared" }
    )
    let faucet = try decode(
      ToriiAccountFaucetPreparedTransactionV1.self,
      response: faucetVector.response
    )
    XCTAssertThrowsError(
      try faucet.validate(
        claim: faucet.claim,
        binding: faucet.binding,
        expectedAuthority: vector.signerAccountId,
        expectedNetworkId: faucetVector.networkId
      )
    )
    var wrongNetworkBytes = faucetVector.networkId.bytes
    wrongNetworkBytes[0] ^= 0x80
    let wrongNetwork = try NetworkId(bytes: wrongNetworkBytes)
    XCTAssertThrowsError(
      try faucet.validate(
        claim: faucet.claim,
        binding: faucet.binding,
        expectedAuthority: faucetVector.signerAccountId,
        expectedNetworkId: wrongNetwork
      )
    )

    var feeObject = try responseObject(faucetVector.response)
    var feePayment = try XCTUnwrap(feeObject["fee_payment"] as? [String: Any])
    var feeValue = try XCTUnwrap(feePayment["value"] as? [String: Any])
    feeValue["gas_limit"] = 1
    feePayment["value"] = feeValue
    feeObject["fee_payment"] = feePayment
    let substitutedFee = try JSONDecoder().decode(
      ToriiAccountFaucetPreparedTransactionV1.self,
      from: JSONSerialization.data(withJSONObject: feeObject)
    )
    XCTAssertThrowsError(
      try substitutedFee.validate(
        claim: faucet.claim,
        binding: faucet.binding,
        expectedAuthority: faucetVector.signerAccountId,
        expectedNetworkId: faucetVector.networkId
      )
    )
  }

  private func assertTranscript(
    _ transcript: Data,
    serverSignature: String,
    vector: Vector,
    file: StaticString = #filePath,
    line: UInt = #line
  ) throws {
    let authority = try AccountAddress.parseEncoded(vector.signerAccountId)
    let controller = try XCTUnwrap(
      authority.singleControllerInfo(),
      file: file,
      line: line
    )
    XCTAssertEqual(controller.algorithm, .ed25519, file: file, line: line)
    XCTAssertEqual(
      vector.signerPublicKey,
      "ed0120\(controller.publicKey.hexUppercased())",
      file: file,
      line: line
    )
    XCTAssertEqual(transcript.hexEncodedString(), vector.transcriptHex, file: file, line: line)
    XCTAssertEqual(
      IrohaHash.hash(transcript).hexEncodedString(),
      vector.digestHex,
      file: file,
      line: line
    )
    XCTAssertEqual(
      serverSignature,
      vector.serverSignatureHex.uppercased(),
      file: file,
      line: line
    )
  }

  private func decode<T: Decodable>(
    _ type: T.Type,
    response: ToriiJSONValue
  ) throws -> T {
    try JSONDecoder().decode(type, from: response.encodedData())
  }

  private func responseObject(_ response: ToriiJSONValue) throws -> [String: Any] {
    try XCTUnwrap(
      JSONSerialization.jsonObject(with: response.encodedData()) as? [String: Any]
    )
  }

  private func zeroInnerSignatureScalar(_ wire: Data) throws -> Data {
    guard wire.first == 1 else {
      throw ToriiClientError.invalidPayload("golden signed transaction is not fixed V1")
    }
    var signed = CanonicalNoritoReader(data: Data(wire.dropFirst()))
    let signatureWrapper = try signed.readCompactField()
    let payload = try signed.readCompactField()
    let attachments = try signed.readCompactField()
    guard signed.remaining() == 0 else {
      throw ToriiClientError.invalidPayload("golden signed transaction contains trailing bytes")
    }
    var wrapper = CanonicalNoritoReader(data: signatureWrapper)
    let encodedSignature = try wrapper.readCompactField()
    guard wrapper.remaining() == 0 else {
      throw ToriiClientError.invalidPayload("golden signature wrapper contains trailing bytes")
    }
    var signatureReader = CanonicalNoritoReader(data: encodedSignature)
    guard try signatureReader.readUInt64LE() == 64 else {
      throw ToriiClientError.invalidPayload("golden inner signature is not Ed25519")
    }
    var signature = Data()
    for _ in 0..<64 {
      let element = try signatureReader.readCompactField()
      guard element.count == 1 else {
        throw ToriiClientError.invalidPayload("golden signature element is noncanonical")
      }
      signature.append(element)
    }
    guard signatureReader.remaining() == 0 else {
      throw ToriiClientError.invalidPayload("golden inner signature contains trailing bytes")
    }
    signature.replaceSubrange(32..<64, with: repeatElement(UInt8(0), count: 32))
    var canonicalWrapper = CompactNoritoWriter()
    canonicalWrapper.writeField(CompactNorito.encodeConstVec(signature))
    var canonicalSigned = CompactNoritoWriter()
    canonicalSigned.writeField(canonicalWrapper.data)
    canonicalSigned.writeField(payload)
    canonicalSigned.writeField(attachments)
    var substituted = Data([1])
    substituted.append(canonicalSigned.data)
    return substituted
  }

  private func loadFixture() throws -> Fixture {
    let repositoryRoot = URL(fileURLWithPath: #filePath)
      .deletingLastPathComponent()
      .deletingLastPathComponent()
      .deletingLastPathComponent()
      .deletingLastPathComponent()
    let fixtureURL =
      repositoryRoot
      .appendingPathComponent("fixtures")
      .appendingPathComponent("prepared_transactions")
      .appendingPathComponent("prepared_transaction_signature_v1.json")
    return try JSONDecoder().decode(Fixture.self, from: Data(contentsOf: fixtureURL))
  }
}
