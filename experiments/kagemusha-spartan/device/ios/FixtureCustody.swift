import CryptoKit
import Foundation

/// Give each embedded fixture manifest a distinct, stable custody directory.
/// Previous diagnostics used the unsuffixed directory, which stays intact.
func fixtureCustodyDirectory(root: URL, manifest: Data) -> URL {
  let digest = SHA256.hash(data: manifest)
  let hex = digest.map { String(format: "%02x", $0) }.joined()
  return root.appendingPathComponent("spartan-runtime-fixture-" + hex, isDirectory: true)
}
