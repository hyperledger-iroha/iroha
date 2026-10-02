// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
import Foundation
@testable import IrohaSwift

/// Refusing test fixture: supplies no original evidence or physical/monetary admission.
func testRequiredIncomingOwner() -> any KagemushaIncomingFoldEvidenceProviderV1 {
  TestOnlyRequiredIncomingEvidenceOwnerV1()
}
private final class TestOnlyRequiredIncomingEvidenceOwnerV1: KagemushaIncomingFoldEvidenceProviderV1 {
  func recheckOriginals(for work: KagemushaNativeIncomingFoldWorkV1) throws {
    throw KagemushaCoreCoordinatorErrorV1.unavailable
  }
  func originalEvidence(for work: KagemushaNativeIncomingFoldWorkV1) throws -> KagemushaOriginalIncomingFoldEvidenceV1 {
    throw KagemushaCoreCoordinatorErrorV1.unavailable
  }
}
