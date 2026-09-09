# Canonical multisig instruction hash fixture

`instruction_batch_hash_v1.json` represents one `iroha.custom` instruction whose
JSON value is `null`. The complete bare Norito vector is 79 bytes, including the
nested framed `CustomInstruction`. Layout flags are `2` (compact lengths).

The vector comes from Rust's `Encode::encode` for
`Vec<InstructionBox>` containing `CustomInstruction::new(Json::new(()))`.
Its hash comes from `HashOf::new` for that vector. Independent Blake2b-256
reconstruction, including Iroha's low-bit marker on the final hash byte,
reproduces `2899a33e1a9cd672ec9ded63b45b22fc51e780b61f069297f2b48dec79356b21`.
Both the original capture and the declared-identity candidate produce these bytes.

The rejected custom frame has an extra JSON field-length prefix. It captures the
corrected Kotlin defect and is a negative fixture only. Its length and checksum
are internally consistent, so rejection tests exercise the payload structure.

Consumers:

- Rust `iroha_executor_data_model::isi::multisig` compares the complete vector and hash.
- Kotlin `CustomInstructionCanonicalParityTest` compares the nested frame, vector
  and hash, rejects the extra prefix and truncations, and checks length boundaries.
- Java `InstructionBatchHashJavaConsumerTest` calls the canonical Kotlin public
  encoder, decoder and hash operation with the same fixture.

The duplicate Java implementation's older hash does not define canonical bytes.
Its remaining assertions and capabilities must migrate into Kotlin-owned
consumers before that implementation is removed. This fixture does not qualify
native account validation, JNI, Android devices or release artifacts.
