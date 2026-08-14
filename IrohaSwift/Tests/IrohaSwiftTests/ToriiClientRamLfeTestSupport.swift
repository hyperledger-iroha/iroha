import Foundation

func ramLfeExecuteResponseJSON(
    programId: String = "identifier_lookup_retail",
    opaqueHash: String = String(repeating: "11", count: 32),
    receiptHash: String = String(repeating: "22", count: 32),
    outputCiphertext: String = "C0FFEE",
    outputHash: String = String(repeating: "44", count: 32),
    associatedDataHash: String = String(repeating: "55", count: 32),
    backend: String = "bfv-programmed-sha3-256-v1",
    verificationMode: String = "signed"
) -> Data {
    """
    {
      "program_id":"\(programId)",
      "opaque_hash":"\(opaqueHash)",
      "receipt_hash":"\(receiptHash)",
      "output_ciphertext":"\(outputCiphertext)",
      "output_hash":"\(outputHash)",
      "associated_data_hash":"\(associatedDataHash)",
      "executed_at_ms":42,
      "expires_at_ms":142,
      "backend":"\(backend)",
      "verification_mode":"\(verificationMode)",
      "receipt":{
        "payload":{
          "program_id":"identifier_lookup_retail",
          "program_digest":"\(String(repeating: "11", count: 32))",
          "backend":"bfv-programmed-sha3-256-v1",
          "verification_mode":"signed",
          "output_hash":"\(String(repeating: "22", count: 32))",
          "associated_data_hash":"\(String(repeating: "33", count: 32))",
          "executed_at_ms":42,
          "expires_at_ms":142
        },
        "attestation":{
          "kind":"signed",
          "signature":"\(String(repeating: "aa", count: 64))"
        }
      },
      "output_opening":{
        "payload":{
          "program_id":"identifier_lookup_retail",
          "input_ciphertext_hash":"\(String(repeating: "ab", count: 32))",
          "output_ciphertext_hash":"\(String(repeating: "bb", count: 32))",
          "parameter_digest":"\(String(repeating: "cd", count: 32))",
          "evaluation_key_digest":"\(String(repeating: "dd", count: 32))",
          "opened_output_hash":"\(String(repeating: "ee", count: 32))",
          "opened_at_ms":42,
          "expires_at_ms":142
        },
        "signature":"\(String(repeating: "ff", count: 64))"
      }
    }
    """.data(using: .utf8)!
}

func ramLfeReceiptVerifyResponseJSON(
    programId: String = "identifier_lookup_retail",
    backend: String = "bfv-programmed-sha3-256-v1",
    verificationMode: String = "signed",
    outputHash: String = String(repeating: "44", count: 32),
    associatedDataHash: String = String(repeating: "55", count: 32)
) -> Data {
    """
    {
      "valid":true,
      "program_id":"\(programId)",
      "backend":"\(backend)",
      "verification_mode":"\(verificationMode)",
      "output_hash":"\(outputHash)",
      "associated_data_hash":"\(associatedDataHash)",
      "output_hash_matches":true
    }
    """.data(using: .utf8)!
}
