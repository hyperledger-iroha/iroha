import CryptoKit
import Foundation
import SpartanDeviceFixture

/// Development-only diagnostic namespace of one signed fixture build.
///
/// Retained fixtures scoped custody, key tags, the rollback anchor and the report by the signed
/// manifest digest alone (the first ones used the unsuffixed directory), so builds embedding the
/// same signed manifest shared one series. Those namespaces are never read, listed or removed here.
/// This build scopes all of them by the SHA-256 of the shared Native preimage
/// (`kagemusha_apple_fixture_namespace_preimage_v1`, which also scopes Native approval keys):
///
///     "kagemusha-spartan-fixture-build-namespace-v1" || 0x00
///     || SHA-256(signed main executable file) || SHA-256(native-device/parameter-manifest.norito)
///
/// Relaunching or reinstalling the same signed build restores its namespace; any other build, even
/// with the same manifest, gets a fresh one. `verify-returned-launches.py` recomputes it at intake.
struct FixtureNamespace: Sendable {
  /// SHA-256 of the signed main executable file: the build identity.
  let executable: Data
  let manifest: Data
  let scope: SHA256.Digest

  init?(preimage: Data) {
    guard preimage.count > 64 else { return nil }
    executable = Data(preimage.dropLast(32).suffix(32))
    manifest = Data(preimage.suffix(32))
    scope = SHA256.hash(data: preimage)
  }

  /// Namespace of the running build from the shared Native implementation.
  static func running() -> FixtureNamespace? {
    var preimage = [UInt8](repeating: 0, count: 256)
    var written = 0
    guard kagemusha_apple_fixture_namespace_preimage_v1(&preimage, preimage.count, &written) == 0,
      written <= preimage.count
    else { return nil }
    return FixtureNamespace(preimage: Data(preimage.prefix(written)))
  }

  var scopeHex: String { hex(scope) }
  var executableHex: String { hex(executable) }
  var manifestHex: String { hex(manifest) }
  var reportName: String { "spartan-runtime-result-" + scopeHex + ".json" }
  func custodyDirectory(root: URL) -> URL {
    root.appendingPathComponent("spartan-runtime-fixture-" + scopeHex, isDirectory: true)
  }
  private func hex<Bytes: Sequence>(_ bytes: Bytes) -> String where Bytes.Element == UInt8 {
    bytes.map { String(format: "%02x", $0) }.joined()
  }
}
