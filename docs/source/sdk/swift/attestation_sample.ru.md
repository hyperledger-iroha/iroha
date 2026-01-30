---
lang: ru
direction: ltr
source: docs/source/sdk/swift/attestation_sample.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4d29c35f048a7e46148e204e599647c6c68a406acfa50d795ba3e360355c787b
source_last_modified: "2026-01-03T18:08:01.233504+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Secure Enclave Attestation Sample (IOS4)

The snippet below captures the canonical payload produced by `swift
signing-attestation export --device-label="iPhone15Pro-01"` using the
forthcoming `SigningKey` Secure Enclave helper. Include this document in the
hardware review pre-read so stakeholders can confirm evidence formatting and
required fields.

## Sample (`secure_enclave_attestation_sample.json`)

```json
{
  "device_label": "iPhone15Pro-01",
  "platform": "iOS 18.0",
  "secure_enclave_firmware": "101.0.15",
  "key_identifier": "com.sora.sdk.signing.wallet",
  "nonce": "18d6c2e5314321b0c60c19d5d9d95233",
  "attestation": {
    "certificate_chain": [
      "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"
    ],
    "signature": "MEUCIQDi6dLpiQ5cwjDa4yMDjCWVd3ud2u7wCr1fU9GbHG5+7QIgfJ7P+uGg7ldPAfxYjX+26x2kFkG5zMd4ZPxXbtW9oEw=",
    "timestamp": "2026-04-24T12:34:56Z"
  }
}
```
