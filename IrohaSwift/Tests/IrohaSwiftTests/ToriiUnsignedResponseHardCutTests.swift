import Foundation
import XCTest

@testable import IrohaSwift

final class ToriiUnsignedResponseHardCutTests: XCTestCase {
  func testMultisigResponseHardCutRejectsAliasesAndPhaseSubstitutions() throws {
    let account = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
    let otherAccount = "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
    let hash = String(repeating: "a", count: 64)
    let payload = try CanonicalUnsignedTransactionTestSupport.genericPayload(
      authority: account
    )
    let signingMessage = IrohaHash.hash(payload)
    let unsigned: [String: Any] = [
      "ok": true,
      "resolved_multisig_account_id": account,
      "submitted": false,
      "proposal_id": hash,
      "instructions_hash": hash,
      "transaction_payload_b64": payload.base64EncodedString(),
      "signing_message_b64": signingMessage.base64EncodedString(),
    ]

    XCTAssertNoThrow(try decodeMultisigResponse(unsigned))
    let unsignedMutations: [(inout [String: Any]) throws -> Void] = [
      { $0["transaction_scaffold_b64"] = "AQ==" },
      { $0["signed_transaction_b64"] = "AQ==" },
      { $0["signing_payload"] = ["payload_base64": "AQ=="] },
      { $0["tx_hash_hex"] = hash },
      { $0.removeValue(forKey: "transaction_payload_b64") },
      { $0.removeValue(forKey: "signing_message_b64") },
      { $0["signing_message_b64"] = Data(repeating: 7, count: 32).base64EncodedString() },
      { $0["transaction_payload_b64"] = "AQI" },
      {
        let wrong = try CanonicalUnsignedTransactionTestSupport.genericPayload(
          authority: otherAccount
        )
        $0["transaction_payload_b64"] = wrong.base64EncodedString()
        $0["signing_message_b64"] = IrohaHash.hash(wrong).base64EncodedString()
      },
      {
        var trailing = payload
        trailing.append(0)
        $0["transaction_payload_b64"] = trailing.base64EncodedString()
        $0["signing_message_b64"] = IrohaHash.hash(trailing).base64EncodedString()
      },
    ]
    for (index, mutation) in unsignedMutations.enumerated() {
      var candidate = unsigned
      try mutation(&candidate)
      XCTAssertThrowsError(
        try decodeMultisigResponse(candidate),
        "unsigned multisig mutation \(index) must fail"
      )
    }

    let submitted: [String: Any] = [
      "ok": true,
      "resolved_multisig_account_id": account,
      "submitted": true,
      "proposal_id": hash,
      "instructions_hash": hash,
      "tx_hash_hex": hash,
    ]
    XCTAssertNoThrow(try decodeMultisigResponse(submitted))
    for field in ["transaction_payload_b64", "signing_message_b64"] {
      var candidate = submitted
      candidate[field] =
        field == "transaction_payload_b64"
        ? payload.base64EncodedString()
        : signingMessage.base64EncodedString()
      XCTAssertThrowsError(try decodeMultisigResponse(candidate))
    }
    var missingHash = submitted
    missingHash.removeValue(forKey: "tx_hash_hex")
    XCTAssertThrowsError(try decodeMultisigResponse(missingHash))
  }

  func testMultisigResponsesRejectNonExactResolvedAccountIds() {
    let accountId = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
    let paddedAccountId = "\(accountId) "
    let proposalId = String(repeating: "f", count: 64)
    func data(_ body: String) -> Data {
      body.data(using: .utf8)!
    }

    XCTAssertThrowsError(
      try JSONDecoder().decode(
        ToriiMultisigContractCallResponse.self,
        from: data(#"{"ok":true,"resolved_multisig_account_id":"\#(paddedAccountId)"}"#)
      )
    )
    XCTAssertThrowsError(
      try JSONDecoder().decode(
        ToriiMultisigSpecResponse.self,
        from: data(#"{"resolved_multisig_account_id":"\#(paddedAccountId)","spec":{"quorum":2}}"#)
      )
    )
    XCTAssertThrowsError(
      try JSONDecoder().decode(
        ToriiMultisigProposalsQueryResponse.self,
        from: data(#"{"resolved_multisig_account_id":"\#(paddedAccountId)","proposals":[]}"#)
      )
    )
    XCTAssertThrowsError(
      try JSONDecoder().decode(
        ToriiMultisigProposalResolveResponse.self,
        from: data(
          #"{"resolved_multisig_account_id":"\#(paddedAccountId)","proposal_id":"\#(proposalId)","instructions_hash":"\#(proposalId)","proposal":{"approvals":[]}}"#
        )
      )
    )
  }

  private func decodeMultisigResponse(
    _ object: [String: Any]
  ) throws -> ToriiMultisigContractCallResponse {
    try JSONDecoder().decode(
      ToriiMultisigContractCallResponse.self,
      from: JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])
    )
  }
}
