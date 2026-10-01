// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import Foundation

/// Public work selected and proved by the retained native Core owner. These bounded
/// projections cannot construct a Core capability or authenticate a supplied proof.
public struct KagemushaNativeIncomingFoldWorkV1: Equatable, Sendable {
  public let kind: KagemushaPendingCreditKindV1
  public let creditID: Data
  public let historyID: Data
  public let hardwareTransitionStatement: Data
  public let proofStatementDigest: Data
  public let normalizedGuardDigest: Data
  public let rootSelectionSigningBytes: Data
  public let deviceKeyReference: Data
  public let hardwareEpochGeneration: KagemushaUInt128V1
  public let hardwareEpochID: Data
  public let nativePairedProof: Data

  public init(selector: KagemushaPendingCreditSelectorV1, nativeFields: [Data]) throws {
    let request = try KagemushaCoreCoordinatorFrameV1.encodeRequest(.prepareIncomingFold,
      fields: [KagemushaCoreCoordinatorFrameV1.u32(selector.kind.rawValue), selector.creditID])
    _ = try KagemushaCoreCoordinatorFrameV1.encodeResponse(.prepareIncomingFold,
      requestFrame: request, fields: nativeFields)
    kind = selector.kind
    historyID = Data(nativeFields[0])
    creditID = Data(nativeFields[1])
    hardwareTransitionStatement = Data(nativeFields[2])
    proofStatementDigest = Data(nativeFields[3])
    normalizedGuardDigest = Data(nativeFields[4])
    rootSelectionSigningBytes = Data(nativeFields[5])
    deviceKeyReference = Data(nativeFields[6])
    hardwareEpochGeneration = try KagemushaUInt128V1(littleEndianBytes: nativeFields[7])
    hardwareEpochID = Data(nativeFields[8])
    nativePairedProof = Data(nativeFields[9])
  }
}

/// Original physical evidence, still untrusted until native method 16 verifies the
/// selected device key, State Guard, history root and retained proof before publication.
public struct KagemushaOriginalIncomingFoldEvidenceV1: Equatable, Sendable {
  public let canonicalHardwareCertificate: Data
  public let deviceRootSelectionSignature: Data

  public init(canonicalHardwareCertificate: Data, deviceRootSelectionSignature: Data) throws {
    guard (1...KagemushaCoreCoordinatorFrameV1.maximumFieldBytes).contains(canonicalHardwareCertificate.count)
    else { throw KagemushaCoreCoordinatorErrorV1.invalidFrame("invalid incoming hardware certificate size") }
    _ = try KagemushaDeviceSignatureV1(rawBytes: deviceRootSelectionSignature)
    self.canonicalHardwareCertificate = Data(canonicalHardwareCertificate)
    self.deviceRootSelectionSignature = Data(deviceRootSelectionSignature)
  }
}

/// Independently installed qualified physical source bound to the same wallet, release
/// and native lease. An exact work retry must return its retained original evidence.
/// No ordinary device opcode or software signer substitutes for this source.
public protocol KagemushaIncomingFoldEvidenceProviderV1: AnyObject {
  func recheckOriginals(for work: KagemushaNativeIncomingFoldWorkV1) throws
  func originalEvidence(for work: KagemushaNativeIncomingFoldWorkV1) throws
    -> KagemushaOriginalIncomingFoldEvidenceV1
}

public enum KagemushaNativeIncomingStageKindV1: UInt32, Sendable {
  case reserveMint = 0, stageMint, stagePeer
}

/// Closed incoming methods on the same retained Core handle. Material and proof
/// production belong to native qualified owners; the managed caller supplies selectors.
public protocol KagemushaNativeIncomingCoreCoordinatorV1: KagemushaNativeCoreCoordinatorV1 {
  func prepareIncomingFold(selector: KagemushaPendingCreditSelectorV1) throws
    -> KagemushaNativeIncomingFoldWorkV1
  func completeIncomingFold(work: KagemushaNativeIncomingFoldWorkV1,
    evidence: KagemushaOriginalIncomingFoldEvidenceV1) throws
  func stageIncomingOriginal(kind: KagemushaNativeIncomingStageKindV1, creditID: Data) throws
}
