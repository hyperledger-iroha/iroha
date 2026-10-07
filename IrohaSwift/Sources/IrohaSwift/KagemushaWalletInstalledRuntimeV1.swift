import Foundation
import NoritoBridge

/// Whole bounded DATA. Only Native compiled trust authenticates the installation.
public struct KagemushaWalletRuntimeOriginalsV1: Sendable, CustomStringConvertible {
  public var description: String { "KagemushaWalletRuntimeOriginalsV1(originals=[REDACTED])" }
  let originals: [Data]
  public init(appManifest: Data, envelope: Data, walletRuntime: Data, verifierPack: Data,
    producerInventory: Data, signedGenesis: Data, originalsRoot: String) throws {
    let inputs = [appManifest, envelope, walletRuntime, verifierPack, producerInventory, signedGenesis]
    let caps = [8_388_608, 2048, 131_072, 16_842_752, 16_777_216, 67_108_864]
    for (index, value) in inputs.enumerated() {
      guard !value.isEmpty, value.count <= caps[index] else { throw KagemushaWalletErrorV1.invalidInput }
    }
    guard !originalsRoot.isEmpty, originalsRoot.hasPrefix("/"), !originalsRoot.utf8.contains(0),
      originalsRoot.utf8.count <= 4096 else { throw KagemushaWalletErrorV1.invalidInput }
    originals = inputs.map { Data($0) } + [Data(originalsRoot.utf8)]
  }
}

/// A failed release retains the exact resource and blocks replacement admission.
public final class KagemushaWalletAdmissionCleanupErrorV1: Error, @unchecked Sendable {
  public let primary: Error, cleanup: Error
  private let retainedResource: AnyObject
  init(primary: Error, cleanup: Error, resource: AnyObject) {
    self.primary=primary; self.cleanup=cleanup
    // The cleanup lease predates wrapper deinit and preserves exact Native/platform lifetime.
    if let lease=(resource as? KagemushaWalletCleanupResourceV1)?.cleanupLease { retainedResource=lease }
    else { retainedResource=resource }
  }
  var resourceReleased: Bool { (retainedResource as? KagemushaWalletNativeLeaseV1)?.isReleased == true }
  /// Retry release of the same quarantined Native owner, including after wrapper destruction.
  /// No-owner/invalid status remains a failure; only actual Native zero acknowledges release.
  public func retryCleanup() throws {
    guard let lease=retainedResource as? KagemushaWalletNativeLeaseV1 else { throw cleanup }
    try lease.close()
  }
}

/// Installation joins the single existing Native Runtime/PendingOpen owner. No second registry.
public final class KagemushaWalletInstalledRuntimeV1: KagemushaWalletCleanupResourceV1, @unchecked Sendable {
  private let condition=NSCondition()
  private let runtime: KagemushaWalletRuntimeV1
  private var fenced=false, closing=false, closed=false
  private let admission=KagemushaWalletInstalledAdmissionV1()
  // Runtime caches Pending weakly; this strong reference preserves its real challenge on ordinary failure.
  private var pendingOriginal: KagemushaWalletPendingOpenV1?
  private var closeFailure: Error?
  private static let failuresLock=NSLock()
  private static var failedCleanup: [KagemushaWalletAdmissionCleanupErrorV1]=[]
  private init(runtime: KagemushaWalletRuntimeV1) { self.runtime=runtime }
  var cleanupLease: KagemushaWalletNativeLeaseV1? { runtime.cleanupLease }
  /// All seven originals are required; Native authenticates the complete installation before returning an owner.
  public static func install(platform: KagemushaWalletApplePlatformV1, originals: KagemushaWalletRuntimeOriginalsV1) throws -> KagemushaWalletInstalledRuntimeV1 {
    try requireNoUnreleasedAdmissions()
    // Resolve all actual wallet functions before Native custody registration, so later adoption cannot fail.
    let walletDriver=try KagemushaWalletNativeDriverV1(), installDriver=try KagemushaWalletInstallDriverV1()
    var handle: UInt64=0, callbacks=kagemushaWalletCallbacksV1(platform)
    let status=withExtendedLifetime(platform) {
      withPinnedWalletOriginalsV1(originals.originals) { ptr in
        var value=connect_norito_kagemusha_wallet_runtime_originals_v1()
        value.app_manifest=ptr[0]; value.app_manifest_length=originals.originals[0].count
        value.envelope=ptr[1]; value.envelope_length=originals.originals[1].count
        value.wallet_runtime=ptr[2]; value.wallet_runtime_length=originals.originals[2].count
        value.verifier_pack=ptr[3]; value.verifier_pack_length=originals.originals[3].count
        value.producer_inventory=ptr[4]; value.producer_inventory_length=originals.originals[4].count
        value.signed_genesis=ptr[5]; value.signed_genesis_length=originals.originals[5].count
        value.originals_root=ptr[6]; value.originals_root_length=originals.originals[6].count
        return installDriver.install(&value,&callbacks,&handle)
      }
    }
    try KagemushaWalletNativeDriverV1.check(status)
    guard handle>0 && handle<=UInt64(Int64.max) else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    // Strong ownership survives begin, asynchronous Ed signing, finish and wallet close/quarantine.
    return .init(runtime: try KagemushaWalletRuntimeV1(nativeRuntimeHandle:handle,driver:walletDriver,platformOwner:platform))
  }
  public static func requireNoUnreleasedAdmissions() throws {
    failuresLock.lock(); defer{failuresLock.unlock()}
    failedCleanup.removeAll { $0.resourceReleased }
    if let first=failedCleanup.first { throw first }
  }
  private func requireLive() throws {
    condition.lock(); defer{condition.unlock()}; guard !fenced else { throw closeFailure ?? KagemushaWalletErrorV1.closed }
  }
  private func fenceCancellation() {
    condition.lock(); fenced=true; condition.unlock()
  }
  private func startOpen(_ originals: KagemushaWalletOpenOriginalsV1) throws {
    condition.lock(); defer { condition.unlock() }
    guard !fenced else { throw closeFailure ?? KagemushaWalletErrorV1.closed }
    try admission.start(originals.originals)
  }
  private func failedOpen() {
    condition.lock(); defer { condition.unlock() }; admission.failed()
  }
  private func completedOpen() throws {
    condition.lock(); defer { condition.unlock() }
    try admission.completed(); pendingOriginal=nil
  }
  private func pendingForOpen(_ originals: KagemushaWalletOpenOriginalsV1) throws -> KagemushaWalletPendingOpenV1 {
    condition.lock()
    guard !fenced else {
      let failure=closeFailure ?? KagemushaWalletErrorV1.closed
      condition.unlock(); throw failure
    }
    if let retained=pendingOriginal { condition.unlock(); return retained }
    condition.unlock()
    // No InstalledRuntime condition is held across Native I/O or another owner's close lock.
    let selected=try runtime.begin(originals)
    condition.lock(); defer { condition.unlock() }
    guard !fenced else { throw closeFailure ?? KagemushaWalletErrorV1.closed }
    pendingOriginal=selected
    return selected
  }
  private func retainedSignature() throws -> Data? {
    condition.lock(); defer { condition.unlock() }
    guard !fenced else { throw closeFailure ?? KagemushaWalletErrorV1.closed }
    return admission.signatureOriginal
  }
  private func freezeSignature(_ original: Data) throws -> Data {
    condition.lock(); defer { condition.unlock() }
    guard !fenced else { throw closeFailure ?? KagemushaWalletErrorV1.closed }
    return try admission.retainSignature(original)
  }
  /// Retry ordinary refusal with the same original frames, Native Pending and exact signature.
  /// Explicit Task cancellation retires custody; successful admission transfers it once.
  public func open(originals: KagemushaWalletOpenOriginalsV1,
    signExistingAccount: @escaping @Sendable (Data) async throws -> Data,
    requireCurrent: @escaping @Sendable () throws -> Void) async throws -> KagemushaWalletV1 {
    try await withTaskCancellationHandler(operation:{
      var wallet: KagemushaWalletV1?
      var started=false
      do {
        try Task.checkCancellation(); try Self.requireNoUnreleasedAdmissions(); try requireCurrent(); try startOpen(originals)
        started=true
        let pending=try await walletAdmissionWorkerV1 { try requireCurrent(); return try self.pendingForOpen(originals) }
        try Task.checkCancellation(); try requireCurrent(); try requireLive()
        guard pending.challenge.count==32 && pending.challenge.contains(where:{$0 != 0}) else { throw KagemushaWalletErrorV1.invalidNativeOutput }
        let signature: Data
        if let retained=try retainedSignature() { signature=retained }
        else {
          let delivered=try await signExistingAccount(pending.challenge)
          // Bound and copy producer DATA before asynchronous dispatch. It supplies no verdict.
          signature=try freezeSignature(delivered)
        }
        try Task.checkCancellation(); try requireCurrent(); try requireLive()
        wallet=try await walletAdmissionWorkerV1 { try requireCurrent(); try self.requireLive(); return try pending.finish(accountSignature:signature) }
        try completedOpen(); started=false
        try Task.checkCancellation(); try requireCurrent(); try requireLive()
        guard let admitted=wallet else { throw KagemushaWalletErrorV1.invalidNativeOutput }; return admitted
      } catch {
        if started { failedOpen(); started=false }
        let primary=error, admitted=wallet
        // A producer may throw CancellationError without cancelling this Task. Retain custody then.
        guard admitted != nil || Task.isCancelled else { throw primary }
        throw await walletAdmissionWorkerResultV1 {
          var reported: Error=primary
          if let admitted { reported=Self.cleanup(reported,resource:admitted){try admitted.close()} }
          return Self.cleanup(reported,resource:self){try self.close()}
        }
      }
    },onCancel:{
      // Fence new operations synchronously; release still requires the actual same-owner acknowledgement.
      self.fenceCancellation()
      DispatchQueue.global(qos:.utility).async { do{try self.close()}catch{/* actual close failure retained */} }
    })
  }
  /// Close unadmitted custody; successful finish transfers exactly the same ID to its wallet.
  public func close() throws {
    condition.lock(); var waited=false
    while closing { waited=true; condition.wait() }
    if closed { condition.unlock(); return }
    if waited, let failure=closeFailure { condition.unlock(); throw failure }
    // A new explicit close retries; every operation remains fenced after the first attempt.
    fenced=true; closing=true; condition.unlock()
    do {
      try runtime.close()
      condition.lock(); closed=true; closeFailure=nil; closing=false; condition.broadcast(); condition.unlock()
    } catch {
      let failure=Self.retain(error,cleanup:error,resource:runtime)
      condition.lock(); closeFailure=failure; closing=false; condition.broadcast(); condition.unlock(); throw failure
    }
  }
  public func closeAsync() async throws { try await walletAdmissionWorkerV1 { try self.close() } }
  static func retain(_ primary: Error,cleanup: Error,resource: AnyObject) -> KagemushaWalletAdmissionCleanupErrorV1 {
    let failure=KagemushaWalletAdmissionCleanupErrorV1(primary:primary,cleanup:cleanup,resource:resource)
    failuresLock.lock(); failedCleanup.append(failure); failuresLock.unlock(); return failure
  }
  private static func cleanup(_ primary: Error,resource: AnyObject,release: () throws -> Void) -> Error {
    do{try release();return primary}catch{return retain(primary,cleanup:error,resource:resource)}
  }
}
/// Managed DATA sequencing only. The InstalledRuntime condition serializes every production call.
/// This type cannot choose a Native owner, authenticate originals, or acknowledge close.
internal final class KagemushaWalletInstalledAdmissionV1 {
  private var originalFrames: [Data]?
  private var retainedSignature: Data?
  private var active=false, transferred=false
  func start(_ frames: [Data]) throws {
    guard !active && !transferred else { throw KagemushaWalletErrorV1.closed }
    if let retained=originalFrames {
      guard retained==frames else { throw KagemushaWalletErrorV1.invalidInput }
    } else { originalFrames=frames.map { Data([UInt8]($0)) } }
    active=true
  }
  // Only managed sequencing resets. Exact original frames, signature and real Pending remain retained.
  func failed() { active=false }
  var signatureOriginal: Data? { retainedSignature.map { Data([UInt8]($0)) } }
  func retainSignature(_ original: Data) throws -> Data {
    guard active && !transferred, original.count==64 else { throw KagemushaWalletErrorV1.invalidInput }
    let frozen=Data([UInt8](original))
    if let retained=retainedSignature {
      guard retained==frozen else { throw KagemushaWalletErrorV1.invalidInput }
      return Data([UInt8](retained))
    }
    retainedSignature=frozen
    return Data([UInt8](frozen))
  }
  func completed() throws {
    guard active && !transferred else { throw KagemushaWalletErrorV1.closed }
    transferred=true; active=false; retainedSignature=nil
  }
}
private final class KagemushaWalletInstallDriverV1 {
  typealias Install = @convention(c) (UnsafePointer<connect_norito_kagemusha_wallet_runtime_originals_v1>?,UnsafePointer<connect_norito_kagemusha_platform_v1>?,UnsafeMutablePointer<UInt64>?) -> Int32
  let install: Install
  init() throws {
    guard let value=NoritoNativeBridge.shared.resolveNativeSymbol("connect_norito_kagemusha_wallet_install_runtime_v1",as:Install.self) else { throw KagemushaWalletErrorV1.bridgeUnavailable }; install=value
  }
}
private func withPinnedWalletOriginalsV1<T>(_ originals:[Data],_ body:([UnsafePointer<UInt8>?])->T)->T {
  func pin(_ index:Int,_ pointers:[UnsafePointer<UInt8>?])->T {
    if index==originals.count{return body(pointers)}
    return originals[index].withUnsafeBytes{pin(index+1,pointers+[$0.bindMemory(to:UInt8.self).baseAddress])}
  };return pin(0,[])
}
private func walletAdmissionWorkerV1<T:Sendable>(_ operation:@escaping @Sendable () throws->T) async throws->T {
  try await withCheckedThrowingContinuation{continuation in DispatchQueue.global(qos:.userInitiated).async{
    do{continuation.resume(returning:try operation())}catch{continuation.resume(throwing:error)}
  }}
}
private func walletAdmissionWorkerResultV1(_ operation:@escaping @Sendable ()->Error) async->Error {
  await withCheckedContinuation{continuation in DispatchQueue.global(qos:.userInitiated).async{continuation.resume(returning:operation())}}
}
