import Foundation
import XCTest
@testable import IrohaSwift

/// Public policy projection correlations only; no fixture here admits a release catalog.
final class KagemushaHardwarePolicyV1Tests: XCTestCase {
  func testCatalogKeepsQualificationDigestAndProviderRootIndependent() throws {
    let f = try AuthenticatedProviderFixtureV1(), qualification = try f.qualification
    let root = Data(repeating: 0x91, count: 32)
    let policy = try KagemushaHardwarePolicyV1(releaseID: qualification.releaseID,
      hardwarePolicyDigest: qualification.hardwarePolicyDigest, providerPolicyRoot: root)
    XCTAssertNotEqual(policy.providerPolicyRoot, qualification.hardwarePolicyDigest)
    let state = try f.aggregate(policy: root)
    let aggregate = try KagemushaNoritoV1.decodeAggregateStateShapeExact(state)
    XCTAssertTrue(policy.matches(qualification: qualification, aggregateState: aggregate))
    let oldDigestAsRoot = try KagemushaNoritoV1.decodeAggregateStateShapeExact(f.aggregate())
    XCTAssertFalse(policy.matches(qualification: qualification, aggregateState: oldDigestAsRoot))
  }

  func testSubstitutedReleaseDigestOrRootCannotMatchOriginalSelection() throws {
    let f = try AuthenticatedProviderFixtureV1(), q = try f.qualification
    let original = try KagemushaNoritoV1.decodeAggregateStateShapeExact(f.aggregate())
    let root = original.hardwarePolicyID
    let valid = try KagemushaHardwarePolicyV1(releaseID: q.releaseID,
      hardwarePolicyDigest: q.hardwarePolicyDigest, providerPolicyRoot: root)
    XCTAssertTrue(valid.matches(qualification: q, aggregateState: original))
    for tuple in [(Data(repeating: 0x92, count: 32), q.hardwarePolicyDigest, root),
      (q.releaseID, Data(repeating: 0x93, count: 32), root),
      (q.releaseID, q.hardwarePolicyDigest, Data(repeating: 0x94, count: 32))] {
      let changed = try KagemushaHardwarePolicyV1(releaseID: tuple.0,
        hardwarePolicyDigest: tuple.1, providerPolicyRoot: tuple.2)
      XCTAssertFalse(changed.matches(qualification: q, aggregateState: original))
    }
  }

  func testNativeCatalogMethodIsClosedAndAllThreeDigestsAreNonzeroExactWidth() throws {
    XCTAssertEqual(KagemushaCoreCoordinatorMethodV1.authenticatedHardwarePolicy.rawValue, 18)
    let request = try KagemushaCoreCoordinatorFrameV1.encodeRequest(.authenticatedHardwarePolicy, fields: [])
    XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeRequest(.authenticatedHardwarePolicy, fields: [Data([1])]))
    for i in 0...2 {
      for bad in [Data(), Data(repeating: 0, count: 32), Data(repeating: 1, count: 31), Data(repeating: 1, count: 33)] {
        var fields = [Data(repeating: 1, count: 32), Data(repeating: 2, count: 32), Data(repeating: 3, count: 32)]
        fields[i] = bad
        XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeResponse(.authenticatedHardwarePolicy,
          requestFrame: request, fields: fields))
        XCTAssertThrowsError(try KagemushaHardwarePolicyV1(releaseID: fields[0],
          hardwarePolicyDigest: fields[1], providerPolicyRoot: fields[2]))
      }
    }
  }

  func testProjectionOwnsOriginalBytesDefensively() throws {
    var root = Data(repeating: 3, count: 32)
    let policy = try KagemushaHardwarePolicyV1(releaseID: Data(repeating: 1, count: 32),
      hardwarePolicyDigest: Data(repeating: 2, count: 32), providerPolicyRoot: root)
    root[0] = 4
    var copy = policy.providerPolicyRoot; copy[0] = 5
    XCTAssertEqual(policy.providerPolicyRoot, Data(repeating: 3, count: 32))
  }
}
