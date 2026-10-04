import Foundation
import Security

// iPhone platform adapter of the KAGEMUSHA wallet Advance provider (G2 design rev 2, iOS;
// specs/kagemusha_single_design_proposal.md §§1.2, 2.2, 2.3, 4.2).
//
// The Rust provider (`crates/iroha_core_zk/src/kagemusha_v1_state/wallet_advance_v1`) owns
// custody bytes, marker selection, reconciliation, the role-checked signer and low-S
// normalization. This adapter supplies only what its `KagemushaWalletPlatformV1` trait needs
// from the phone: the Secure Enclave payment key (probe, generate, sign, delete), the
// keychain rollback anchor, protected-storage state, the custody root path, the boot
// identity and the sleep-inclusive monotonic clock.
//
// Every probe answers present, absent or unavailable. Absent is only the keychain's
// `errSecItemNotFound` observed while protected data was available before and after the
// query; every other error is unavailable and never read as absence.
//
// TODO(G2-bridge): connect_norito_bridge registers this adapter as the C vtable behind the
// Rust `KagemushaWalletPlatformV1` (wallet_advance_v1/platform.rs). The vtable-facing methods
// stay internal so app code reaches the payment key and the anchor only through the Rust
// provider: `keySign` with arbitrary bytes would bypass the role-checked receipt signer.

/// Reason a platform answer is not definitive; never absence.
///
/// Maps one-to-one onto Rust `KagemushaWalletUnavailableV1` (`Locked`, `Io`, `Platform`,
/// `KeyUnusable`).
public enum KagemushaWalletAppleUnavailableV1: Error, Equatable, Hashable, Sendable {
  /// Protected data or the keychain is locked (`errSecInteractionNotAllowed`, or the
  /// Complete-class canary refused with `EPERM`).
  case locked
  /// A POSIX error, or `0` when none is available.
  case io(Int32)
  /// An `OSStatus`, or a ``KagemushaWalletAppleStatusV1`` code.
  case platform(Int32)
  /// The payment key exists but cannot be used or refused to sign.
  case keyUnusable
}

/// Adapter-defined codes reported as ``KagemushaWalletAppleUnavailableV1/platform(_:)``. They lie
/// outside every `OSStatus` range.
public enum KagemushaWalletAppleStatusV1 {
  /// The device has no Secure Enclave; enrollment is refused.
  public static let secureEnclaveUnavailable: Int32 = 0x4B47_0001
  /// App Attest is unsupported; enrollment is refused (spec §2.2).
  public static let appAttestUnsupported: Int32 = 0x4B47_0002
  /// More than one payment key carries the slot's tag; none is selected.
  public static let ambiguousPaymentKey: Int32 = 0x4B47_0003
  /// A generated payment key could not be found again under its tag.
  public static let paymentKeyNotPersisted: Int32 = 0x4B47_0004
  /// The keychain returned a result of an unexpected shape.
  public static let malformedKeychainResult: Int32 = 0x4B47_0005
  /// No device passcode is set; the passcode-bound anchor cannot exist.
  public static let passcodeNotSet: Int32 = 0x4B47_0006
  /// The custody root or its canary is not the expected kind of entry.
  public static let invalidCustodyRoot: Int32 = 0x4B47_0007
  /// An anchor value is empty or larger than the Rust bound.
  public static let invalidAnchorValue: Int32 = 0x4B47_0008
  /// The Mach continuous clock has no usable timebase.
  public static let clockUnavailable: Int32 = 0x4B47_0009
}

/// Wallet slot identity: 32 nonzero bytes chosen by the Rust provider. Its keychain name
/// `kgm-w1-<64 lowercase hex>` is the payment key's application tag and the anchor's account.
public struct KagemushaWalletAppleSlotV1: Equatable, Hashable, Sendable {
  /// Slot bytes.
  public let bytes: Data

  /// `nil` unless `bytes` is 32 bytes and not all zero (as Rust `KagemushaWalletSlotIdV1`).
  public init?(_ bytes: Data) {
    guard bytes.count == 32, bytes.contains(where: { $0 != 0 }) else { return nil }
    self.bytes = Data(bytes)
  }

  /// `kgm-w1-<slot hex>`.
  public var keychainName: String { "kgm-w1-" + kagemushaWalletAppleHex(bytes) }

  /// Application tag of the payment key.
  var applicationTag: Data { Data(keychainName.utf8) }
}

/// Hardware key policy of one enrollment (Rust `KagemushaWalletKeyProfileV1`). On iPhone both
/// profiles generate a Secure Enclave key; the tag is recorded with the key.
public enum KagemushaWalletAppleKeyProfileV1: UInt8, Equatable, Hashable, Sendable {
  /// The dedicated secure element only.
  case secureElement = 1
  /// The secure element or, on Android, the TEE.
  case secureElementOrTee = 2
}

/// What a payment-key generation binds (Rust `KagemushaWalletKeyGenerationRequestV1`): the
/// issuer challenge digest that the App Attest `clientDataHash` carries at E5, and the profile.
/// Both are recorded in the key item's label, so E5 attests only the challenge the key was
/// generated for.
struct KagemushaWalletAppleKeyGenerationRequestV1: Equatable, Sendable {
  static let labelPrefix = "kgm-w1-binding:"

  let challengeDigest: Data
  let profile: KagemushaWalletAppleKeyProfileV1

  init?(challengeDigest: Data, profile: KagemushaWalletAppleKeyProfileV1) {
    guard challengeDigest.count == 32 else { return nil }
    self.challengeDigest = Data(challengeDigest)
    self.profile = profile
  }

  /// Strict parse of ``label``.
  init?(label: String) {
    let bytes = Array(label.utf8)
    let prefix = Array(Self.labelPrefix.utf8)
    guard bytes.count == prefix.count + 2 + 64, bytes.starts(with: prefix),
      bytes[prefix.count + 1] == UInt8(ascii: ":"),
      let profile = KagemushaWalletAppleKeyProfileV1(rawValue: bytes[prefix.count] &- 0x30),
      let digest = kagemushaWalletAppleParseHex(bytes[(prefix.count + 2)...])
    else { return nil }
    self.init(challengeDigest: digest, profile: profile)
  }

  /// `kgm-w1-binding:<profile tag>:<challenge digest hex>`.
  var label: String {
    Self.labelPrefix + String(profile.rawValue) + ":" + kagemushaWalletAppleHex(challengeDigest)
  }
}

/// Tri-state answer (Rust `KagemushaWalletProbeV1`).
enum KagemushaWalletAppleProbeV1<Value> {
  case present(Value)
  case absent
  case unavailable(KagemushaWalletAppleUnavailableV1)

  func map<Mapped>(_ transform: (Value) -> Mapped) -> KagemushaWalletAppleProbeV1<Mapped> {
    switch self {
    case .present(let value): return .present(transform(value))
    case .absent: return .absent
    case .unavailable(let reason): return .unavailable(reason)
    }
  }
}

extension KagemushaWalletAppleProbeV1: Equatable where Value: Equatable {}

/// Outcome of a payment-key generation (Rust `KagemushaWalletKeyGenerationV1`).
enum KagemushaWalletAppleKeyGenerationV1: Equatable {
  /// A new key exists under the slot's tag; its 65-byte X9.63 public key.
  case generated(publicKey: Data)
  /// A key already exists; it is never replaced.
  case alreadyPresent
  /// Unknown; the provider probes again before acting.
  case unavailable(KagemushaWalletAppleUnavailableV1)
}

/// Why a write did not publish (Rust `KagemushaWalletNotPublishedV1`).
enum KagemushaWalletAppleNotPublishedV1: Equatable {
  case destinationExists
  case destinationAbsent
  case noSpace
  case failed(KagemushaWalletAppleUnavailableV1)
}

/// Outcome of an anchor write (Rust `KagemushaWalletPublishOutcomeV1`).
enum KagemushaWalletApplePublishOutcomeV1: Equatable {
  case published
  case notPublished(KagemushaWalletAppleNotPublishedV1)
  case uncertain(KagemushaWalletAppleUnavailableV1)
}

/// Outcome of a key deletion (Rust `KagemushaWalletRemoveOutcomeV1`).
enum KagemushaWalletAppleRemoveOutcomeV1: Equatable {
  case removed
  case notRemoved(KagemushaWalletAppleUnavailableV1)
  case uncertain(KagemushaWalletAppleUnavailableV1)
}

/// iPhone platform adapter of the KAGEMUSHA wallet Advance provider.
///
/// - The payment key is a Secure Enclave P-256 key (`kSecAttrTokenIDSecureEnclave`) stored
///   permanently under the slot's tag with `.privateKeyUsage` and
///   `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`. It is never bound to user presence,
///   biometry or the passcode (spec §2.3), never replaced, and signs only as DER through
///   `SecKeyCreateSignature(.ecdsaSignatureMessageX962SHA256)`; Rust normalizes to low S and
///   verifies before any byte is written.
/// - The rollback anchor is one generic-password item per slot in
///   `kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly`, created add-only and read inside
///   protected-data brackets. Its value is the Rust-encoded `{generation, marker_file_digest}`.
/// - Protected-data state is the readability of a Complete-class canary in the custody root.
///
/// App extensions cannot reach the custody root and must not use this adapter.
public final class KagemushaWalletApplePlatformV1: @unchecked Sendable {
  /// Keychain service of the rollback anchor items.
  public static let anchorService = "org.hyperledger.iroha.kagemusha.wallet.v1.marker-anchor"
  /// Rust `KAGEMUSHA_WALLET_ANCHOR_MAX_BYTES_V1`.
  static let anchorMaxBytes = 256
  /// Rust `KagemushaWalletAnchorPolicyV1::Keychain` tag: this platform keeps an anchor.
  static let anchorPolicyTag: UInt8 = 1
  /// Accessibility of the payment key: usable after the first unlock, never migrated.
  static let paymentKeyAccessibility = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
  /// Access-control flags of the payment key: Secure Enclave private-key use only, with no
  /// user-presence, biometry or passcode constraint (spec §2.3).
  static let paymentKeyAccessFlags: SecAccessControlCreateFlags = [.privateKeyUsage]
  /// Accessibility of the rollback anchor: never backed up, synced or escrowed; readable only
  /// while unlocked; rendered useless if the passcode is removed or reset (lost custody,
  /// warned before enrollment).
  // TODO(owner Q6): the owner has not confirmed this class. The alternative
  // kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly survives passcode removal but is copied
  // into same-device backups and relies on undocumented Secure Enclave restore behavior.
  static let anchorAccessibility = kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly

  let system: KagemushaWalletAppleSystemV1
  let appAttest: any KagemushaAppAttestServiceV1
  private let generationLock = NSLock()

  /// Adapter over the system keychain, Secure Enclave and file system. `appAttest` is the App
  /// Attest service of enrollment step E5 (``attestEnrollment(slot:paymentPublicKey:challengeDigest:keyBindingDigest:)``);
  /// generation is refused where it is unsupported.
  public convenience init(appAttest: any KagemushaAppAttestServiceV1) {
    self.init(appAttest: appAttest, system: .live)
  }

  #if os(iOS) && canImport(DeviceCheck)
  /// Adapter using the system App Attest service.
  public convenience init() {
    self.init(appAttest: KagemushaAppleAppAttestServiceV1())
  }
  #endif

  init(appAttest: any KagemushaAppAttestServiceV1, system: KagemushaWalletAppleSystemV1) {
    self.appAttest = appAttest
    self.system = system
  }

  // MARK: - Protected data

  /// Whether protected data is available now: the Complete-class canary opens and reads.
  /// `EPERM` is ``KagemushaWalletAppleUnavailableV1/locked``; a missing canary or any other
  /// error is `io` (prepare the root first with ``custodyRootPath()``).
  func storageState() -> Result<Void, KagemushaWalletAppleUnavailableV1> {
    switch custodyRootURL() {
    case .failure(let reason):
      return .failure(reason)
    case .success(let root):
      return Self.storageState(
        errno: system.probeReadable(root.appendingPathComponent(Self.canaryName).path))
    }
  }

  static func storageState(errno code: Int32) -> Result<Void, KagemushaWalletAppleUnavailableV1> {
    switch code {
    case 0: return .success(())
    case EPERM: return .failure(.locked)
    default: return .failure(.io(code))
    }
  }

  /// Run `query` inside protected-data brackets: nothing is queried while storage is
  /// unavailable, and an absent answer counts only if storage was still available afterwards
  /// (a single check races with a lock event).
  func bracketed<Value>(_ query: () -> KagemushaWalletAppleProbeV1<Value>)
    -> KagemushaWalletAppleProbeV1<Value>
  {
    if case .failure(let reason) = storageState() { return .unavailable(reason) }
    let answer = query()
    guard case .absent = answer else { return answer }
    if case .failure(let reason) = storageState() { return .unavailable(reason) }
    return .absent
  }

  // MARK: - Payment key

  /// Keychain query matching the slot's payment key.
  static func paymentKeyQuery(_ slot: KagemushaWalletAppleSlotV1) -> [String: Any] {
    [
      kSecClass as String: kSecClassKey,
      kSecAttrKeyClass as String: kSecAttrKeyClassPrivate,
      kSecAttrKeyType as String: kSecAttrKeyTypeECSECPrimeRandom,
      kSecAttrApplicationTag as String: slot.applicationTag,
      kSecUseDataProtectionKeychain as String: true,
    ]
  }

  /// `SecKeyCreateRandomKey` attributes of the slot's Secure Enclave payment key.
  static func paymentKeyGenerationAttributes(
    _ slot: KagemushaWalletAppleSlotV1, _ request: KagemushaWalletAppleKeyGenerationRequestV1,
    accessControl: SecAccessControl
  ) -> [String: Any] {
    [
      kSecAttrKeyType as String: kSecAttrKeyTypeECSECPrimeRandom,
      kSecAttrKeySizeInBits as String: 256,
      kSecAttrTokenID as String: kSecAttrTokenIDSecureEnclave,
      kSecUseDataProtectionKeychain as String: true,
      kSecPrivateKeyAttrs as String: [
        kSecAttrIsPermanent as String: true,
        kSecAttrApplicationTag as String: slot.applicationTag,
        kSecAttrLabel as String: request.label,
        kSecAttrAccessControl as String: accessControl,
      ] as [String: Any],
    ]
  }

  /// Access control of a new payment key.
  static func paymentKeyAccessControl() -> Result<SecAccessControl, KagemushaWalletAppleUnavailableV1> {
    var error: Unmanaged<CFError>?
    guard
      let access = SecAccessControlCreateWithFlags(
        nil, paymentKeyAccessibility, paymentKeyAccessFlags, &error)
    else {
      return .failure(.platform(KagemushaWalletAppleSystemV1.status(of: error?.takeRetainedValue())))
    }
    return .success(access)
  }

  /// One payment key found under a slot's tag.
  struct PaymentKeyItem {
    let key: SecKey
    let publicKey: Data
    let label: String?
  }

  /// Look up the slot's payment key (not bracketed). More than one key under the tag is
  /// ambiguous and never selected; a key whose public key cannot be exported is unusable and
  /// is never deleted or regenerated.
  func lookupPaymentKey(_ slot: KagemushaWalletAppleSlotV1)
    -> KagemushaWalletAppleProbeV1<PaymentKeyItem>
  {
    var query = Self.paymentKeyQuery(slot)
    query[kSecReturnRef as String] = true
    query[kSecReturnAttributes as String] = true
    query[kSecMatchLimit as String] = kSecMatchLimitAll
    let (status, result) = system.keychain.copyMatching(query)
    switch status {
    case errSecSuccess: break
    case errSecItemNotFound: return .absent
    default: return .unavailable(Self.unavailable(status))
    }
    guard let items = result as? [[String: Any]], !items.isEmpty,
      let reference = items[0][kSecValueRef as String]
    else {
      system.diagnostic("kagemusha wallet: malformed payment-key lookup for \(slot.keychainName)")
      return .unavailable(.platform(KagemushaWalletAppleStatusV1.malformedKeychainResult))
    }
    guard items.count == 1 else {
      system.diagnostic("kagemusha wallet: \(items.count) payment keys under \(slot.keychainName)")
      return .unavailable(.platform(KagemushaWalletAppleStatusV1.ambiguousPaymentKey))
    }
    let object = reference as AnyObject
    guard CFGetTypeID(object) == SecKeyGetTypeID() else {
      return .unavailable(.platform(KagemushaWalletAppleStatusV1.malformedKeychainResult))
    }
    let key = object as! SecKey
    guard let publicKey = Self.x963PublicKey(of: key) else {
      system.diagnostic("kagemusha wallet: payment key \(slot.keychainName) is unusable")
      return .unavailable(.keyUnusable)
    }
    return .present(
      PaymentKeyItem(key: key, publicKey: publicKey, label: items[0][kSecAttrLabel as String] as? String))
  }

  /// 65-byte uncompressed X9.63 public key of `privateKey`.
  static func x963PublicKey(of privateKey: SecKey) -> Data? {
    guard let publicKey = SecKeyCopyPublicKey(privateKey) else { return nil }
    var error: Unmanaged<CFError>?
    guard let data = SecKeyCopyExternalRepresentation(publicKey, &error) as Data? else {
      _ = error?.takeRetainedValue()
      return nil
    }
    guard data.count == 65, data.first == 0x04 else { return nil }
    return data
  }

  /// Probe the slot's payment key (Rust `key_probe`).
  func keyProbe(_ slot: KagemushaWalletAppleSlotV1) -> KagemushaWalletAppleProbeV1<Data> {
    bracketed { lookupPaymentKey(slot).map(\.publicKey) }
  }

  /// Generate the slot's payment key (Rust `key_generate`). The provider calls it only after a
  /// definitive absent probe; the adapter probes again under its lock and never replaces a
  /// key. Generation is refused without a Secure Enclave or App Attest support. Any unknown
  /// outcome is unavailable, so the provider probes again before acting.
  func keyGenerate(
    _ slot: KagemushaWalletAppleSlotV1, _ request: KagemushaWalletAppleKeyGenerationRequestV1
  ) -> KagemushaWalletAppleKeyGenerationV1 {
    generationLock.lock()
    defer { generationLock.unlock() }
    switch keyProbe(slot) {
    case .present: return .alreadyPresent
    case .unavailable(let reason): return .unavailable(reason)
    case .absent: break
    }
    guard system.secureEnclaveAvailable() else {
      return .unavailable(.platform(KagemushaWalletAppleStatusV1.secureEnclaveUnavailable))
    }
    guard appAttest.isSupported else {
      return .unavailable(.platform(KagemushaWalletAppleStatusV1.appAttestUnsupported))
    }
    let accessControl: SecAccessControl
    switch Self.paymentKeyAccessControl() {
    case .success(let access): accessControl = access
    case .failure(let reason): return .unavailable(reason)
    }
    let (createdKey, status) = system.keychain.createRandomKey(
      Self.paymentKeyGenerationAttributes(slot, request, accessControl: accessControl))
    guard let created = createdKey else {
      return status == errSecDuplicateItem ? .alreadyPresent : .unavailable(Self.unavailable(status))
    }
    guard let publicKey = Self.x963PublicKey(of: created) else { return .unavailable(.keyUnusable) }
    switch keyProbe(slot) {
    case .present(let stored) where stored == publicKey:
      return .generated(publicKey: publicKey)
    case .present:
      return .unavailable(.platform(KagemushaWalletAppleStatusV1.ambiguousPaymentKey))
    case .absent:
      return .unavailable(.platform(KagemushaWalletAppleStatusV1.paymentKeyNotPersisted))
    case .unavailable(let reason):
      return .unavailable(reason)
    }
  }

  /// Sign `preimage` with the slot's payment key (Rust `key_sign`): the Secure Enclave signs
  /// `SHA-256(preimage)` and returns strict DER. Only the Rust role-checked signers construct
  /// preimages; this method stays internal for that reason.
  func keySign(_ slot: KagemushaWalletAppleSlotV1, preimage: Data)
    -> Result<Data, KagemushaWalletAppleUnavailableV1>
  {
    switch lookupPaymentKey(slot) {
    case .present(let item): return Self.sign(item.key, preimage: preimage)
    case .absent: return .failure(.platform(errSecItemNotFound))
    case .unavailable(let reason): return .failure(reason)
    }
  }

  /// DER ECDSA P-256 / SHA-256 over `preimage` with `key`.
  static func sign(_ key: SecKey, preimage: Data) -> Result<Data, KagemushaWalletAppleUnavailableV1> {
    let algorithm = SecKeyAlgorithm.ecdsaSignatureMessageX962SHA256
    guard SecKeyIsAlgorithmSupported(key, .sign, algorithm) else { return .failure(.keyUnusable) }
    var error: Unmanaged<CFError>?
    guard let signature = SecKeyCreateSignature(key, algorithm, preimage as CFData, &error) as Data?
    else {
      let status = KagemushaWalletAppleSystemV1.status(of: error?.takeRetainedValue())
      return .failure(status == errSecInteractionNotAllowed ? .locked : .keyUnusable)
    }
    return .success(signature)
  }

  /// Delete the slot's payment key (Rust `key_delete`, custody deletion step D3 only).
  /// Removed only when a bracketed probe afterwards finds no key.
  func keyDelete(_ slot: KagemushaWalletAppleSlotV1) -> KagemushaWalletAppleRemoveOutcomeV1 {
    if case .failure(let reason) = storageState() { return .notRemoved(reason) }
    let status = system.keychain.delete(Self.paymentKeyQuery(slot))
    guard status == errSecSuccess || status == errSecItemNotFound else {
      return Self.definitelyNotPerformed(status)
        ? .notRemoved(Self.unavailable(status)) : .uncertain(Self.unavailable(status))
    }
    switch keyProbe(slot) {
    case .absent: return .removed
    case .present: return .uncertain(.platform(errSecDuplicateItem))
    case .unavailable(let reason): return .uncertain(reason)
    }
  }

  // MARK: - Rollback anchor

  /// Keychain query matching the slot's anchor item.
  static func anchorQuery(_ slot: KagemushaWalletAppleSlotV1) -> [String: Any] {
    [
      kSecClass as String: kSecClassGenericPassword,
      kSecAttrService as String: anchorService,
      kSecAttrAccount as String: slot.keychainName,
      kSecAttrSynchronizable as String: false,
      kSecUseDataProtectionKeychain as String: true,
    ]
  }

  /// `SecItemAdd` attributes of the slot's anchor item holding `value`.
  static func anchorAddAttributes(_ slot: KagemushaWalletAppleSlotV1, value: Data) -> [String: Any] {
    var attributes = anchorQuery(slot)
    attributes[kSecAttrAccessible as String] = anchorAccessibility
    attributes[kSecValueData as String] = value
    return attributes
  }

  private static func validAnchorValue(_ value: Data) -> Bool {
    !value.isEmpty && value.count <= anchorMaxBytes
  }

  /// Add the slot's anchor item (Rust `anchor_create`; add-only). Refused while locked or
  /// without a device passcode.
  func anchorCreate(_ slot: KagemushaWalletAppleSlotV1, value: Data)
    -> KagemushaWalletApplePublishOutcomeV1
  {
    guard Self.validAnchorValue(value) else {
      return .notPublished(.failed(.platform(KagemushaWalletAppleStatusV1.invalidAnchorValue)))
    }
    if case .failure(let reason) = storageState() { return .notPublished(.failed(reason)) }
    if system.passcodeSet() == false {
      return .notPublished(.failed(.platform(KagemushaWalletAppleStatusV1.passcodeNotSet)))
    }
    return Self.addOutcome(system.keychain.add(Self.anchorAddAttributes(slot, value: value)))
  }

  /// Read the slot's anchor item (Rust `anchor_read`) inside protected-data brackets.
  func anchorRead(_ slot: KagemushaWalletAppleSlotV1) -> KagemushaWalletAppleProbeV1<Data> {
    bracketed {
      var query = Self.anchorQuery(slot)
      query[kSecReturnData as String] = true
      query[kSecMatchLimit as String] = kSecMatchLimitOne
      let (status, result) = system.keychain.copyMatching(query)
      switch status {
      case errSecSuccess:
        guard let data = result as? Data else {
          return .unavailable(.platform(KagemushaWalletAppleStatusV1.malformedKeychainResult))
        }
        return .present(data)
      case errSecItemNotFound:
        return .absent
      default:
        return .unavailable(Self.unavailable(status))
      }
    }
  }

  /// Replace the value of the slot's anchor item (Rust `anchor_update`). The provider raises
  /// it only to a durable marker and re-reads it after an uncertain outcome.
  func anchorUpdate(_ slot: KagemushaWalletAppleSlotV1, value: Data)
    -> KagemushaWalletApplePublishOutcomeV1
  {
    guard Self.validAnchorValue(value) else {
      return .notPublished(.failed(.platform(KagemushaWalletAppleStatusV1.invalidAnchorValue)))
    }
    if case .failure(let reason) = storageState() { return .notPublished(.failed(reason)) }
    return Self.updateOutcome(
      system.keychain.update(Self.anchorQuery(slot), [kSecValueData as String: value]))
  }

  // MARK: - Status mapping

  /// Reason of a non-success keychain status: `errSecInteractionNotAllowed` is locked, every
  /// other status (including `errSecItemNotFound` outside a probe) is `platform(status)`.
  static func unavailable(_ status: OSStatus) -> KagemushaWalletAppleUnavailableV1 {
    status == errSecInteractionNotAllowed ? .locked : .platform(status)
  }

  /// Statuses with which the keychain refused before changing anything. Every other failure
  /// of a write is uncertain and reconciled by re-reading.
  static func definitelyNotPerformed(_ status: OSStatus) -> Bool {
    [
      errSecInteractionNotAllowed, errSecParam, errSecMissingEntitlement, errSecNotAvailable,
      errSecUnimplemented, errSecNoSuchAttr, errSecDiskFull,
    ].contains(status)
  }

  static func addOutcome(_ status: OSStatus) -> KagemushaWalletApplePublishOutcomeV1 {
    switch status {
    case errSecSuccess: return .published
    case errSecDuplicateItem: return .notPublished(.destinationExists)
    default: return writeFailure(status)
    }
  }

  static func updateOutcome(_ status: OSStatus) -> KagemushaWalletApplePublishOutcomeV1 {
    switch status {
    case errSecSuccess: return .published
    case errSecItemNotFound: return .notPublished(.destinationAbsent)
    default: return writeFailure(status)
    }
  }

  private static func writeFailure(_ status: OSStatus) -> KagemushaWalletApplePublishOutcomeV1 {
    if status == errSecDiskFull { return .notPublished(.noSpace) }
    return definitelyNotPerformed(status)
      ? .notPublished(.failed(unavailable(status))) : .uncertain(unavailable(status))
  }
}

/// Lowercase hex of `bytes`.
func kagemushaWalletAppleHex<Bytes: Sequence>(_ bytes: Bytes) -> String where Bytes.Element == UInt8 {
  let digits = Array("0123456789abcdef".utf8)
  var text = [UInt8]()
  for byte in bytes {
    text.append(digits[Int(byte >> 4)])
    text.append(digits[Int(byte & 0x0f)])
  }
  return String(decoding: text, as: UTF8.self)
}

/// Strict lowercase hex; `nil` for an odd length or any other character.
func kagemushaWalletAppleParseHex<Text: Collection>(_ text: Text) -> Data? where Text.Element == UInt8 {
  guard text.count % 2 == 0 else { return nil }
  func value(_ digit: UInt8) -> UInt8? {
    switch digit {
    case 0x30...0x39: return digit - 0x30
    case 0x61...0x66: return digit - 0x61 + 10
    default: return nil
    }
  }
  var bytes = Data()
  var high: UInt8?
  for digit in text {
    guard let nibble = value(digit) else { return nil }
    if let first = high {
      bytes.append(first << 4 | nibble)
      high = nil
    } else {
      high = nibble
    }
  }
  return bytes
}
