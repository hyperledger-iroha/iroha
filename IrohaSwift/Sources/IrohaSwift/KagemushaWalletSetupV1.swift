import Foundation
import NoritoBridge

/// Opaque native direct-time exchange. It has no public initializer or serialized form.
public struct KagemushaWalletTimeExchangeV1: Sendable {
  let owner: UInt64
  let token: UInt64
  /// Fresh native nonce sent to the issuer; no local clock reading is exposed.
  public let nonce: Data
  init(owner: UInt64, token: UInt64, nonce: Data) {
    self.owner = owner; self.token = token; self.nonce = nonce
  }
}

/// Fixed typed bounds only. Rust authenticates every original and selects state and proof owners.
struct KagemushaWalletSetupInputV1 {
  let selector: UInt32
  let identity: Data
  let amount: KagemushaWalletUInt128V1
  let token: UInt64
  let first: Data
  let second: Data
  let third: Data
  init(selector: UInt32, identity: Data = Data(repeating: 0, count: 32),
       amount: KagemushaWalletUInt128V1 = .init(low: 0, high: 0), token: UInt64 = 0,
       first: Data = Data(), second: Data = Data(), third: Data = Data()) throws {
    let limits: [Int]
    switch selector {
    case 0, 1, 4, 6, 15: limits = [0, 0, 0]
    case 2: limits = [10_000, 1_024, 512]
    case 3, 7...14: limits = [10_000, 0, 0]
    case 5: limits = [512, 512, 0]
    default: throw KagemushaWalletErrorV1.invalidInput
    }
    guard identity.count == 32, ((1...2).contains(selector)) == identity.contains(where: { $0 != 0 }),
      (selector == 1) == (amount.low != 0 || amount.high != 0), ([5, 6].contains(selector)) == (token != 0),
      token <= UInt64(Int64.max), zip([first, second, third], limits).allSatisfy({ $0.count <= $1 }),
      selector != 2 || (!first.isEmpty && second.isEmpty == third.isEmpty),
      (selector != 3 && !(7...14).contains(selector)) || !first.isEmpty, selector != 5 || (!first.isEmpty && !second.isEmpty)
    else { throw KagemushaWalletErrorV1.invalidInput }
    self.selector = selector; self.identity = identity; self.amount = amount; self.token = token
    self.first = first; self.second = second; self.third = third
  }
  func withRequest<T>(_ body: (UnsafePointer<connect_norito_kagemusha_wallet_setup_request_v1>) -> T) -> T {
    identity.withUnsafeBytes { id in first.withUnsafeBytes { a in second.withUnsafeBytes { b in third.withUnsafeBytes { c in
      var value = connect_norito_kagemusha_wallet_setup_request_v1()
      value.setup_id = id.bindMemory(to: UInt8.self).baseAddress
      value.selector = selector; value.amount = .init(low: amount.low, high: amount.high); value.token = token
      value.first = a.bindMemory(to: UInt8.self).baseAddress; value.first_length = a.count
      value.second = b.bindMemory(to: UInt8.self).baseAddress; value.second_length = b.count
      value.third = c.bindMemory(to: UInt8.self).baseAddress; value.third_length = c.count
      return body(&value)
    }}}}
  }
}

/// Exact peer-envelope kind; it cannot select proof keys or operation authority.
public enum KagemushaWalletTransportKindV1: UInt32, Sendable {
  case offer = 1, request = 2, payment = 3, credited = 4
}
