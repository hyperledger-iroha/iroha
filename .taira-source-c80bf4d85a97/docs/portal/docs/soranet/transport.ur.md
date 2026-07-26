---
lang: ur
direction: rtl
source: docs/portal/docs/soranet/transport.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3d384345dcbe8c537c5be3e6b1877f3df6aa779096fc53db714b45e7124dd636
source_last_modified: "2025-11-11T12:56:25.187116+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: transport
title: SoraNet transport overview
sidebar_label: Transport overview
description: SoraNet anonymity overlay کے لئے handshake، salt rotation، اور capability guidance۔
---

:::note Canonical Source
یہ صفحہ `docs/source/soranet/spec.md` میں SNNet-1 transport specification کی عکاسی کرتا ہے۔ جب تک پرانا documentation set retire نہ ہو، دونوں کاپیاں sync رکھیں۔
:::

SoraNet وہ anonymity overlay ہے جو SoraFS کے range fetches، Norito RPC streaming، اور مستقبل کے Nexus data lanes کو سپورٹ کرتا ہے۔ transport program (roadmap items **SNNet-1**, **SNNet-1a**, اور **SNNet-1b**) نے deterministic handshake، post-quantum (PQ) capability negotiation، اور salt rotation plan متعین کیا تاکہ ہر relay، client، اور gateway ایک ہی security posture observe کرے۔

## Goals اور network model

- QUIC v1 پر three-hop circuits (entry -> middle -> exit) بنانا تاکہ abusive peers کبھی Torii تک براہ راست نہ پہنچیں۔
- QUIC/TLS کے اوپر Noise XX *hybrid* handshake (Curve25519 + Kyber768) layering تاکہ session keys TLS transcript سے bind ہوں۔
- capability TLVs لازمی کریں جو PQ KEM/signature support، relay role، اور protocol version advertise کریں؛ unknown types کو GREASE کریں تاکہ future extensions deployable رہیں۔
- blinded-content salts کو روزانہ rotate کریں اور guard relays کو 30 دن pin کریں تاکہ directory churn clients کو deanonymize نہ کر سکے۔
- cells کو 1024 B پر fixed رکھیں، padding/dummy cells inject کریں، اور deterministic telemetry export کریں تاکہ downgrade attempts جلد پکڑی جائیں۔

## Handshake pipeline (SNNet-1a)

1. **QUIC/TLS envelope** - clients QUIC v1 پر relays سے connect کرتے ہیں اور Ed25519 certificates کے ساتھ TLS 1.3 handshake complete کرتے ہیں جو governance CA نے sign کئے ہوتے ہیں۔ TLS exporter (`tls-exporter("soranet handshake", 64)`) Noise layer کو seed کرتا ہے تاکہ transcripts inseparable رہیں۔
2. **Noise XX hybrid** - protocol string `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` with prologue = TLS exporter۔ Message flow:

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   Curve25519 DH output اور دونوں Kyber encapsulations final symmetric keys میں mix ہوتے ہیں۔ PQ material negotiate نہ ہو تو handshake مکمل طور پر abort ہوتا ہے - classical-only fallback کی اجازت نہیں۔

3. **Puzzle tickets & tokens** - relays `ClientHello` سے پہلے Argon2id proof-of-work ticket مانگ سکتے ہیں۔ Tickets length-prefixed frames ہیں جو hashed Argon2 solution لے جاتے ہیں اور policy bounds کے اندر expire ہوتے ہیں:

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   `SNTK` prefix والے admission tokens puzzles bypass کرتے ہیں جب issuer کی ML-DSA-44 signature active policy اور revocation list کے خلاف validate ہو جائے۔

4. **Capability TLV exchange** - final Noise payload نیچے بیان کردہ capability TLVs لے جاتا ہے۔ اگر کوئی لازمی capability (PQ KEM/signature، role، یا version) missing ہو یا directory entry سے mismatch کرے تو clients connection abort کرتے ہیں۔

5. **Transcript logging** - relays transcript hash، TLS fingerprint، اور TLV contents log کرتے ہیں تاکہ downgrade detectors اور compliance pipelines کو feed کیا جا سکے۔

## Capability TLVs (SNNet-1c)

Capabilities ایک مقررہ `typ/length/value` TLV envelope reuse کرتی ہیں:

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

آج کے defined types:

- `snnet.pqkem` - Kyber level (`kyber768` موجودہ rollout کے لئے).
- `snnet.pqsig` - PQ signature suite (`ml-dsa-44`).
- `snnet.role` - relay role (`entry`, `middle`, `exit`, `gateway`).
- `snnet.version` - protocol version identifier.
- `snnet.grease` - reserved range میں random filler entries تاکہ future TLVs tolerate ہوں۔

Clients required TLVs کی allow-list رکھتے ہیں اور missing یا downgrade ہونے پر handshake fail کرتے ہیں۔ Relays یہی set اپنی directory microdescriptor میں publish کرتے ہیں تاکہ validation deterministic رہے۔

## Salt rotation & CID blinding (SNNet-1b)

- Governance `SaltRotationScheduleV1` record publish کرتی ہے جس میں `(epoch_id, salt, valid_after, valid_until)` values ہوتے ہیں۔ Relays اور gateways signed schedule کو directory publisher سے fetch کرتے ہیں۔
- Clients `valid_after` پر نیا salt apply کرتے ہیں، پچھلا salt 12 h grace period کے لئے رکھتے ہیں، اور delayed updates کے لئے 7 epochs history retain کرتے ہیں۔
- Canonical blinded identifiers یوں بنتے ہیں:

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)
  ```

  Gateways `Sora-Req-Blinded-CID` کے ذریعے blinded key قبول کرتے ہیں اور `Sora-Content-CID` میں echo کرتے ہیں۔ Circuit/request blinding (`CircuitBlindingKey::derive`) `iroha_crypto::soranet::blinding` میں موجود ہے۔
- اگر relay کوئی epoch miss کرے تو وہ نئے circuits روک دیتا ہے جب تک schedule download نہ کر لے اور `SaltRecoveryEventV1` emit کرتا ہے، جسے on-call dashboards paging signal کے طور پر treat کرتے ہیں۔

## Directory data اور guard policy

- Microdescriptors relay identity (Ed25519 + ML-DSA-65)، PQ keys، capability TLVs، region tags، guard eligibility، اور موجودہ advertised salt epoch لے جاتے ہیں۔
- Clients guard sets کو 30 دن pin کرتے ہیں اور `guard_set` caches کو signed directory snapshot کے ساتھ persist کرتے ہیں۔ CLI اور SDK wrappers cache fingerprint surface کرتے ہیں تاکہ rollout evidence change reviews کے ساتھ attach ہو سکے۔

## Telemetry & rollout checklist

- Production سے پہلے export کرنے والی metrics:
  - `soranet_handshake_success_total{role}`
  - `soranet_handshake_failure_total{reason}`
  - `soranet_handshake_latency_seconds`
  - `soranet_capability_mismatch_total`
  - `soranet_salt_rotation_lag_seconds`
- Alert thresholds salt rotation SOP SLO matrix (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) کے ساتھ رہتی ہیں اور network promote کرنے سے پہلے Alertmanager میں mirror ہونی چاہئیں۔
- Alerts: 5 منٹ میں >5% failure rate، salt lag >15 minutes، یا production میں capability mismatches.
- Rollout steps:
  1. Staging میں hybrid handshake اور PQ stack enable کر کے relay/client interoperability tests چلائیں۔
  2. Salt rotation SOP (`docs/source/soranet_salt_plan.md`) rehearse کریں اور drill artifacts change record کے ساتھ attach کریں۔
  3. Directory میں capability negotiation enable کریں، پھر entry relays، middle relays، exit relays، اور آخر میں clients تک rollout کریں۔
  4. ہر phase کے لئے guard cache fingerprints، salt schedules، اور telemetry dashboards record کریں؛ evidence bundle کو `status.md` کے ساتھ attach کریں۔

یہ checklist follow کرنے سے operator، client، اور SDK teams SoraNet transports کو یکساں رفتار سے adopt کر سکتے ہیں اور SNNet roadmap میں موجود determinism اور audit requirements پوری کرتے ہیں۔
