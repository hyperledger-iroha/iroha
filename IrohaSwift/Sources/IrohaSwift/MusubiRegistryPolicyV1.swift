import Foundation

/// Versioned first-release Musubi registry policy.
public struct MusubiRegistryPolicyV1: Hashable, Sendable {
  public let version: UInt8
  public let revision: UInt64
  public let mode: MusubiRegistryAdmissionModeV1
  public let allowlistedDataspaces: [UInt64]
  public let aliasPricing: MusubiAliasPricingPolicyV1

  public init(
    version: UInt8 = 1,
    revision: UInt64,
    mode: MusubiRegistryAdmissionModeV1,
    allowlistedDataspaces: [UInt64],
    aliasPricing: MusubiAliasPricingPolicyV1
  ) throws {
    guard version == 1, revision > 0,
      allowlistedDataspaces.count <= 1_024,
      zip(allowlistedDataspaces, allowlistedDataspaces.dropFirst())
        .allSatisfy({ $0.0 < $0.1 }),
      mode == .allowlisted || allowlistedDataspaces.isEmpty
    else {
      throw MusubiV1Error.invalidValue(
        "Musubi registry policy is invalid or noncanonical."
      )
    }
    self.version = version
    self.revision = revision
    self.mode = mode
    self.allowlistedDataspaces = allowlistedDataspaces
    self.aliasPricing = aliasPricing
  }
}

func musubiValidateVersionRequirementV1(_ requirement: MusubiVersionReqV1) throws {
  if case .comparators(let comparators) = requirement {
    guard !comparators.isEmpty, comparators.count <= 16,
      comparators == Array(Set(comparators)).sorted(),
      !(comparators.count == 1 && comparators[0].op == .equal),
      comparators.filter({ $0.op == .equal }).count <= 1
    else {
      throw MusubiV1Error.invalidValue("Musubi comparator AST is noncanonical.")
    }
  }
}

func musubiRequirementMatchesV1(
  _ requirement: MusubiVersionReqV1,
  version: MusubiVersionV1
) -> Bool {
  let prereleaseEligible: Bool = {
    guard !version.prerelease.isEmpty else { return true }
    func namesCore(_ candidate: MusubiVersionV1) -> Bool {
      !candidate.prerelease.isEmpty
        && candidate.major == version.major
        && candidate.minor == version.minor
        && candidate.patch == version.patch
    }
    switch requirement {
    case .caret(let base), .tilde(let base), .exact(let base): return namesCore(base)
    case .comparators(let values): return values.contains { namesCore($0.version) }
    default: return false
    }
  }()
  guard prereleaseEligible else { return false }
  switch requirement {
  case .any: return true
  case .exact(let expected): return version == expected
  case .majorWildcard(let major): return version.major == major
  case .minorWildcard(let major, let minor):
    return version.major == major && version.minor == minor
  case .comparators(let values):
    return values.allSatisfy { item in
      switch item.op {
      case .greater: return version > item.version
      case .greaterOrEqual: return version >= item.version
      case .less: return version < item.version
      case .lessOrEqual: return version <= item.version
      case .equal: return version == item.version
      }
    }
  case .caret(let base):
    guard version >= base else { return false }
    if base.major > 0 { return version.major == base.major }
    if base.minor > 0 { return version.major == 0 && version.minor == base.minor }
    return version.major == 0 && version.minor == 0 && version.patch == base.patch
  case .tilde(let base):
    return version >= base && version.major == base.major && version.minor == base.minor
  }
}

func musubiCompareStringV1(_ left: String, _ right: String) -> Int {
  let leftBytes = Array(left.utf8)
  let rightBytes = Array(right.utf8)
  for (leftByte, rightByte) in zip(leftBytes, rightBytes) where leftByte != rightByte {
    return leftByte < rightByte ? -1 : 1
  }
  if leftBytes.count == rightBytes.count { return 0 }
  return leftBytes.count < rightBytes.count ? -1 : 1
}

func musubiRequireUniqueParentLocalAliasesV1(
  _ aliases: [String],
  field: String
) throws {
  guard Set(aliases).count == aliases.count else {
    throw MusubiV1Error.invalidValue(
      "\(field) must use unique parent-local aliases."
    )
  }
}

func musubiComparePackageV1(
  _ left: MusubiPackageIdV1,
  _ right: MusubiPackageIdV1
) -> Int {
  if left.homeDataspace != right.homeDataspace {
    return left.homeDataspace < right.homeDataspace ? -1 : 1
  }
  switch (left.scope, right.scope) {
  case (.dataspaceRoot, .domain): return -1
  case (.domain, .dataspaceRoot): return 1
  case (.dataspaceRoot, .dataspaceRoot): break
  case (.domain(let leftDomain), .domain(let rightDomain)):
    let comparison = musubiCompareStringV1(leftDomain, rightDomain)
    if comparison != 0 { return comparison }
  }
  return musubiCompareStringV1(left.name.value, right.name.value)
}

func musubiReleaseLessV1(
  _ left: MusubiReleaseIdV1,
  _ right: MusubiReleaseIdV1
) -> Bool {
  let packageComparison = musubiComparePackageV1(left.package, right.package)
  if packageComparison != 0 { return packageComparison < 0 }
  return left.version < right.version
}

func musubiRequirementRankV1(_ value: MusubiVersionReqV1) -> Int {
  switch value {
  case .any: return 0
  case .caret: return 1
  case .tilde: return 2
  case .majorWildcard: return 3
  case .minorWildcard: return 4
  case .exact: return 5
  case .comparators: return 6
  }
}

func musubiRequirementLessV1(
  _ left: MusubiVersionReqV1,
  _ right: MusubiVersionReqV1
) -> Bool {
  let leftRank = musubiRequirementRankV1(left)
  let rightRank = musubiRequirementRankV1(right)
  if leftRank != rightRank { return leftRank < rightRank }
  switch (left, right) {
  case (.any, .any): return false
  case (.caret(let left), .caret(let right)),
    (.tilde(let left), .tilde(let right)),
    (.exact(let left), .exact(let right)):
    return left < right
  case (.majorWildcard(let left), .majorWildcard(let right)):
    return left < right
  case (
    .minorWildcard(let leftMajor, let leftMinor),
    .minorWildcard(let rightMajor, let rightMinor)
  ):
    return leftMajor != rightMajor ? leftMajor < rightMajor : leftMinor < rightMinor
  case (.comparators(let left), .comparators(let right)):
    for index in 0..<min(left.count, right.count) where left[index] != right[index] {
      return left[index] < right[index]
    }
    return left.count < right.count
  default:
    return false
  }
}

func musubiDependencyReqLessV1(
  _ left: MusubiDependencyReqV1,
  _ right: MusubiDependencyReqV1
) -> Bool {
  let aliasComparison = musubiCompareStringV1(left.alias, right.alias)
  if aliasComparison != 0 { return aliasComparison < 0 }
  let packageComparison = musubiComparePackageV1(left.package, right.package)
  if packageComparison != 0 { return packageComparison < 0 }
  return musubiRequirementLessV1(left.requirement, right.requirement)
}

func musubiExactDependencyLessV1(
  _ left: MusubiExactDependencyEdgeV1,
  _ right: MusubiExactDependencyEdgeV1
) -> Bool {
  let aliasComparison = musubiCompareStringV1(left.alias, right.alias)
  if aliasComparison != 0 { return aliasComparison < 0 }
  if left.kind != right.kind { return left.kind.rawValue < right.kind.rawValue }
  let packageComparison = musubiComparePackageV1(left.package, right.package)
  if packageComparison != 0 { return packageComparison < 0 }
  if left.requirement != right.requirement {
    return musubiRequirementLessV1(left.requirement, right.requirement)
  }
  return musubiReleaseLessV1(left.selected, right.selected)
}
