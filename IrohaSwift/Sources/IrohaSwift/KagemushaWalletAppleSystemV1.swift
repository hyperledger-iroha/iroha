import CryptoKit
import Darwin
import Foundation
import Security
#if canImport(LocalAuthentication)
import LocalAuthentication
#endif

// Operating-system seams of the iPhone wallet platform adapter
// (`KagemushaWalletApplePlatformV1`), plus its custody root, boot identity and clock.
// Every seam is a value the tests replace with a fake; the live values call the system.

/// Keychain operations the adapter performs. Production uses
/// ``KagemushaWalletAppleSystemKeychainV1``; tests inject a fake.
protocol KagemushaWalletAppleKeychainV1: Sendable {
  /// `SecItemCopyMatching`.
  func copyMatching(_ query: [String: Any]) -> (status: OSStatus, result: CFTypeRef?)
  /// `SecItemAdd` without a returned result.
  func add(_ attributes: [String: Any]) -> OSStatus
  /// `SecItemUpdate`.
  func update(_ query: [String: Any], _ attributes: [String: Any]) -> OSStatus
  /// `SecItemDelete`.
  func delete(_ query: [String: Any]) -> OSStatus
  /// `SecKeyCreateRandomKey`; on failure the status is the `CFError` code.
  func createRandomKey(_ attributes: [String: Any]) -> (key: SecKey?, status: OSStatus)
}

/// The system keychain.
struct KagemushaWalletAppleSystemKeychainV1: KagemushaWalletAppleKeychainV1 {
  func copyMatching(_ query: [String: Any]) -> (status: OSStatus, result: CFTypeRef?) {
    var result: CFTypeRef?
    let status = SecItemCopyMatching(query as CFDictionary, &result)
    return (status, result)
  }

  func add(_ attributes: [String: Any]) -> OSStatus {
    SecItemAdd(attributes as CFDictionary, nil)
  }

  func update(_ query: [String: Any], _ attributes: [String: Any]) -> OSStatus {
    SecItemUpdate(query as CFDictionary, attributes as CFDictionary)
  }

  func delete(_ query: [String: Any]) -> OSStatus {
    SecItemDelete(query as CFDictionary)
  }

  func createRandomKey(_ attributes: [String: Any]) -> (key: SecKey?, status: OSStatus) {
    var error: Unmanaged<CFError>?
    guard let key = SecKeyCreateRandomKey(attributes as CFDictionary, &error) else {
      return (nil, KagemushaWalletAppleSystemV1.status(of: error?.takeRetainedValue()))
    }
    return (key, errSecSuccess)
  }
}

/// Mach continuous time and its timebase.
struct KagemushaWalletAppleContinuousTimeV1: Equatable, Sendable {
  /// `mach_continuous_time()` ticks (includes sleep).
  var ticks: UInt64
  /// `mach_timebase_info` numerator.
  var numer: UInt32
  /// `mach_timebase_info` denominator.
  var denom: UInt32
}

/// Operating-system services of the adapter.
struct KagemushaWalletAppleSystemV1: Sendable {
  /// Keychain and Secure Enclave key creation.
  var keychain: any KagemushaWalletAppleKeychainV1
  /// Whether this device has a Secure Enclave.
  var secureEnclaveAvailable: @Sendable () -> Bool
  /// `true` with a device passcode, `false` when the system reports none, `nil` when unknown.
  var passcodeSet: @Sendable () -> Bool?
  /// The app's `Library/Application Support` directory.
  var applicationSupportDirectory: @Sendable () -> Result<URL, KagemushaWalletAppleUnavailableV1>
  /// Open and read one byte of `path`; `0` on success, otherwise the `errno`.
  var probeReadable: @Sendable (String) -> Int32
  /// Raw bytes of a string `sysctl`.
  var sysctl: @Sendable (String) -> Result<[UInt8], KagemushaWalletAppleUnavailableV1>
  /// Mach continuous time.
  var continuousTime: @Sendable () -> KagemushaWalletAppleContinuousTimeV1
  /// Diagnostic sink for conditions reported as unavailable (never acted on).
  var diagnostic: @Sendable (String) -> Void

  /// The live system.
  static let live = KagemushaWalletAppleSystemV1(
    keychain: KagemushaWalletAppleSystemKeychainV1(),
    secureEnclaveAvailable: { SecureEnclave.isAvailable },
    passcodeSet: { liveDevicePasscodeSet() },
    applicationSupportDirectory: { liveApplicationSupportDirectory() },
    probeReadable: { liveProbeReadable($0) },
    sysctl: { liveSysctlBytes($0) },
    continuousTime: { liveContinuousTime() },
    diagnostic: { _ in })

  /// `OSStatus` of a `CFError` (its code), or `errSecInternalComponent` when none fits.
  static func status(of error: CFError?) -> OSStatus {
    guard let error else { return errSecInternalComponent }
    return OSStatus(exactly: CFErrorGetCode(error)) ?? errSecInternalComponent
  }

  /// POSIX code of a Foundation error (directly or as its underlying error), or `0`.
  static func posixCode(_ error: Error) -> Int32 {
    let nsError = error as NSError
    if nsError.domain == NSPOSIXErrorDomain {
      return Int32(clamping: nsError.code)
    }
    if let underlying = nsError.userInfo[NSUnderlyingErrorKey] as? NSError,
      underlying.domain == NSPOSIXErrorDomain
    {
      return Int32(clamping: underlying.code)
    }
    return 0
  }

  private static func liveDevicePasscodeSet() -> Bool? {
    #if canImport(LocalAuthentication)
    let context = LAContext()
    var error: NSError?
    if context.canEvaluatePolicy(.deviceOwnerAuthentication, error: &error) {
      return true
    }
    if let error, (error as Error as? LAError)?.code == .passcodeNotSet {
      return false
    }
    return nil
    #else
    return nil
    #endif
  }

  private static func liveApplicationSupportDirectory()
    -> Result<URL, KagemushaWalletAppleUnavailableV1>
  {
    do {
      return .success(
        try FileManager.default.url(
          for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil,
          create: true))
    } catch {
      return .failure(.io(posixCode(error)))
    }
  }

  private static func liveProbeReadable(_ path: String) -> Int32 {
    var descriptor: Int32
    repeat {
      descriptor = open(path, O_RDONLY | O_CLOEXEC | O_NOFOLLOW)
    } while descriptor < 0 && errno == EINTR
    guard descriptor >= 0 else { return errno }
    defer { close(descriptor) }
    var byte: UInt8 = 0
    while true {
      if read(descriptor, &byte, 1) >= 0 { return 0 }
      let code = errno
      if code != EINTR { return code }
    }
  }

  private static func liveSysctlBytes(_ name: String)
    -> Result<[UInt8], KagemushaWalletAppleUnavailableV1>
  {
    var size = 0
    guard sysctlbyname(name, nil, &size, nil, 0) == 0 else { return .failure(.io(errno)) }
    guard size > 0, size <= 1_024 else { return .failure(.io(EINVAL)) }
    var buffer = [UInt8](repeating: 0, count: size)
    guard sysctlbyname(name, &buffer, &size, nil, 0) == 0 else { return .failure(.io(errno)) }
    return .success(Array(buffer.prefix(size)))
  }

  private static func liveContinuousTime() -> KagemushaWalletAppleContinuousTimeV1 {
    var timebase = mach_timebase_info_data_t()
    guard mach_timebase_info(&timebase) == KERN_SUCCESS else {
      return KagemushaWalletAppleContinuousTimeV1(ticks: 0, numer: 0, denom: 0)
    }
    return KagemushaWalletAppleContinuousTimeV1(
      ticks: mach_continuous_time(), numer: timebase.numer, denom: timebase.denom)
  }
}

// MARK: - Custody root

/// Kind of one path, from `lstat` (symbolic links are never followed).
enum KagemushaWalletAppleEntryKindV1: Equatable {
  case missing
  case directory
  case file
  case other
  case error(Int32)

  init(path: String) {
    var info = stat()
    guard lstat(path, &info) == 0 else {
      let code = errno
      self = code == ENOENT ? .missing : .error(code)
      return
    }
    switch info.st_mode & S_IFMT {
    case S_IFDIR: self = .directory
    case S_IFREG: self = .file
    default: self = .other
    }
  }
}

extension KagemushaWalletApplePlatformV1 {
  /// Directory name of the custody root (Rust `KAGEMUSHA_WALLET_ROOT_DIR_NAME_V1`).
  static let custodyRootName = "kagemusha-wallet-v1"
  /// Protected-data canary name (Rust `KAGEMUSHA_WALLET_CANARY_NAME_V1`).
  static let canaryName = "canary"
  /// Default protection class of the custody root, so every file the Rust store creates in it
  /// is readable after the first unlock (design: completeUntilFirstUserAuthentication).
  static let custodyRootProtection = FileProtectionType.completeUntilFirstUserAuthentication
  /// Protection class of the canary: readable only while the device is unlocked.
  static let canaryProtection = FileProtectionType.complete

  /// Attributes (re)applied to the custody root on every preparation.
  static var custodyRootAttributes: [FileAttributeKey: Any] {
    var attributes: [FileAttributeKey: Any] = [.posixPermissions: NSNumber(value: Int16(0o700))]
    #if os(iOS)
    attributes[.protectionKey] = custodyRootProtection
    #endif
    return attributes
  }

  /// Create-new write options of the canary.
  static var canaryWriteOptions: Data.WritingOptions {
    #if os(iOS)
    return [.withoutOverwriting, .completeFileProtection]
    #else
    return [.withoutOverwriting]
    #endif
  }

  /// `Library/Application Support/kagemusha-wallet-v1`, without touching the filesystem.
  func custodyRootURL() -> Result<URL, KagemushaWalletAppleUnavailableV1> {
    system.applicationSupportDirectory().map {
      $0.appendingPathComponent(Self.custodyRootName, isDirectory: true)
    }
  }

  /// Prepare the custody root and return its path for the Rust store.
  ///
  /// The root lives in `Library/Application Support` (never `Documents` or an App Group
  /// container, which app extensions could reach). It is created with mode `0700`; its mode,
  /// its default protection class and `isExcludedFromBackup` are re-applied on every call
  /// (exclusion is Apple guidance only, so safety never depends on it: the keychain anchor
  /// detects restored files). The Complete-class canary is created once, create-new, and is
  /// never rewritten. A root or canary that is a symbolic link or another kind of entry is
  /// refused; nothing is ever removed.
  func custodyRootPath() -> Result<String, KagemushaWalletAppleUnavailableV1> {
    prepareCustodyRoot().map(\.path)
  }

  func prepareCustodyRoot() -> Result<URL, KagemushaWalletAppleUnavailableV1> {
    let root: URL
    switch custodyRootURL() {
    case .success(let url): root = url
    case .failure(let reason): return .failure(reason)
    }
    let manager = FileManager.default
    switch KagemushaWalletAppleEntryKindV1(path: root.path) {
    case .directory:
      break
    case .missing:
      do {
        try manager.createDirectory(
          at: root, withIntermediateDirectories: false, attributes: Self.custodyRootAttributes)
      } catch {
        // A concurrent creator may have won; only a directory is accepted below.
        guard KagemushaWalletAppleEntryKindV1(path: root.path) == .directory else {
          return .failure(.io(KagemushaWalletAppleSystemV1.posixCode(error)))
        }
      }
    case .file, .other:
      return .failure(.platform(KagemushaWalletAppleStatusV1.invalidCustodyRoot))
    case .error(let code):
      return .failure(.io(code))
    }
    guard KagemushaWalletAppleEntryKindV1(path: root.path) == .directory else {
      return .failure(.platform(KagemushaWalletAppleStatusV1.invalidCustodyRoot))
    }
    do {
      try manager.setAttributes(Self.custodyRootAttributes, ofItemAtPath: root.path)
      var excluded = root
      var values = URLResourceValues()
      values.isExcludedFromBackup = true
      try excluded.setResourceValues(values)
    } catch {
      return .failure(.io(KagemushaWalletAppleSystemV1.posixCode(error)))
    }
    let canary = root.appendingPathComponent(Self.canaryName, isDirectory: false)
    switch KagemushaWalletAppleEntryKindV1(path: canary.path) {
    case .file:
      break
    case .missing:
      do {
        try Data([0x01]).write(to: canary, options: Self.canaryWriteOptions)
        try manager.setAttributes(
          [.posixPermissions: NSNumber(value: Int16(0o600))], ofItemAtPath: canary.path)
      } catch {
        guard KagemushaWalletAppleEntryKindV1(path: canary.path) == .file else {
          return .failure(.io(KagemushaWalletAppleSystemV1.posixCode(error)))
        }
      }
    case .directory, .other:
      return .failure(.platform(KagemushaWalletAppleStatusV1.invalidCustodyRoot))
    case .error(let code):
      return .failure(.io(code))
    }
    return .success(root)
  }
}

// MARK: - Boot identity and clock

extension KagemushaWalletApplePlatformV1 {
  /// Sysctl naming the boot session (read-only in XNU).
  static let bootSessionSysctl = "kern.bootsessionuuid"

  /// Lowercase boot session UUID of the current boot (`kern.bootsessionuuid`).
  ///
  /// Rust hashes this text to the 32-byte boot identity with
  /// `kagemusha_wallet_boot_id_from_text_v1`. Whether the sysctl is readable from the iOS app
  /// sandbox is unverified; when it is not, the answer is unavailable and the provider treats
  /// every file as written in the current boot.
  // TODO(G2-iOS): device-test `kern.bootsessionuuid` from the app sandbox.
  func bootSessionUUID() -> Result<String, KagemushaWalletAppleUnavailableV1> {
    system.sysctl(Self.bootSessionSysctl).flatMap(Self.bootSessionUUID(fromSysctl:))
  }

  /// Parse the NUL-terminated sysctl value: a trimmed 36-character hyphenated UUID, returned
  /// in lowercase; anything else is `io(0)` (as Rust `kagemusha_wallet_boot_id_from_text_v1`).
  static func bootSessionUUID(fromSysctl bytes: [UInt8])
    -> Result<String, KagemushaWalletAppleUnavailableV1>
  {
    let text = bytes.prefix { $0 != 0 }
    guard let decoded = String(bytes: text, encoding: .ascii) else { return .failure(.io(0)) }
    let uuid = Array(decoded.trimmingCharacters(in: .whitespacesAndNewlines).utf8)
    let wellFormed = uuid.count == 36
      && uuid.enumerated().allSatisfy { index, byte in
        [8, 13, 18, 23].contains(index)
          ? byte == UInt8(ascii: "-")
          : (byte >= 0x30 && byte <= 0x39) || (byte >= 0x41 && byte <= 0x46)
            || (byte >= 0x61 && byte <= 0x66)
      }
    guard wellFormed else { return .failure(.io(0)) }
    return .success(String(decoding: uuid, as: UTF8.self).lowercased())
  }

  /// Sleep-inclusive monotonic milliseconds from `mach_continuous_time`.
  func monotonicMilliseconds() -> Result<UInt64, KagemushaWalletAppleUnavailableV1> {
    guard let milliseconds = Self.milliseconds(system.continuousTime()) else {
      return .failure(.platform(KagemushaWalletAppleStatusV1.clockUnavailable))
    }
    return .success(milliseconds)
  }

  /// `floor(ticks * numer / (denom * 1_000_000))` without intermediate overflow; `nil` for a
  /// zero timebase or a result beyond `UInt64`.
  static func milliseconds(_ time: KagemushaWalletAppleContinuousTimeV1) -> UInt64? {
    guard time.numer != 0, time.denom != 0 else { return nil }
    let product = time.ticks.multipliedFullWidth(by: UInt64(time.numer))
    let divisor = UInt64(time.denom) * 1_000_000
    guard product.high < divisor else { return nil }
    return divisor.dividingFullWidth(product).quotient
  }
}
