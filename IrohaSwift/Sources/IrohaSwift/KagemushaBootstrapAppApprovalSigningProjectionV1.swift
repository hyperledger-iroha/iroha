import Foundation

/// Exact zero-state Bootstrap W/S transport projection. It creates no captured authority.
struct KagemushaBootstrapAppApprovalSigningProjectionV1: Sendable {
  let wrapper: KagemushaAppOperationApprovalWrapperV1
  var canonicalSigningBytes: Data { wrapper.canonicalSigningBytes }
  var canonicalFinancialSubject: Data { wrapper.canonicalFinancialSubject }
  var operationID: Data { wrapper.operationID }
  var nonce: Data { wrapper.nonce }
  var accountBinding: Data { wrapper.accountBinding }
  var authorityPolicyDigest: Data { wrapper.authorityPolicyDigest }
  var attestedKeyID: Data { wrapper.attestedKeyID }
  var enrollmentDigest: Data { wrapper.enrollmentDigest }
  var subjectSigningDigest: Data { wrapper.subjectSigningDigest }
  var normalizedGuardDigest: Data { wrapper.normalizedGuardDigest }
  var clientDataHash: Data { wrapper.clientDataHash }
  init(nativeSigningBytes: Data, nativeFinancialSubject: Data, credentialDigest: Data) throws {
    let parsed = try KagemushaAppOperationApprovalWrapperV1(nativeSigningBytes: nativeSigningBytes,
      nativeFinancialSubject: nativeFinancialSubject)
    let subject = parsed.canonicalFinancialSubject
    guard subject[331] == 0, subject[364..<460].allSatisfy({ $0 == 0 }),
      KagemushaAppPlatformPreparedProjectionV1.digest(credentialDigest),
      parsed.enrollmentDigest == credentialDigest, Data(subject[155..<187]) == credentialDigest else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("Bootstrap approval differs from zero-state FI credential")
    }
    wrapper = parsed
  }
}
