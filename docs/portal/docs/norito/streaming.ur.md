---
lang: ur
direction: rtl
source: docs/portal/docs/norito/streaming.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 10ba7b91d73d6723c4b66491951c3257c48557273ab5424d81119e01c8f2c6e3
source_last_modified: "2025-12-09T06:48:00.858874+00:00"
translation_last_reviewed: 2025-12-30
---

# Norito Streaming

Norito Streaming on-wire فارمیٹ، کنٹرول فریمز اور ریفرنس codec کی وضاحت کرتا ہے جو Torii اور SoraNet میں live media flows کے لئے استعمال ہوتے ہیں۔ کینونیکل سپیک `norito_streaming.md` میں workspace کے روٹ پر موجود ہے؛ یہ صفحہ وہ حصے نچوڑتا ہے جو operators اور SDK authors کو کنفیگریشن ٹچ پوائنٹس کے ساتھ درکار ہوتے ہیں۔

## Wire format اور control plane

- **Manifests اور frames.** `ManifestV1` اور `PrivacyRoute*` segment timeline، chunk descriptors اور route hints بیان کرتے ہیں۔ کنٹرول فریمز (`KeyUpdate`, `ContentKeyUpdate`, اور cadence feedback) manifest کے ساتھ رہتے ہیں تاکہ viewers decoding سے پہلے commitments validate کر سکیں۔
- **Baseline codec.** `BaselineEncoder`/`BaselineDecoder` monotonic chunk ids، timestamp arithmetic اور commitment verification نافذ کرتے ہیں۔ hosts کو viewers یا relays کو serve کرنے سے پہلے `EncodedSegment::verify_manifest` کال کرنا لازم ہے۔
- **Feature bits.** capability negotiation `streaming.feature_bits` (ڈیفالٹ `0b11` = baseline feedback + privacy route provider) کا اعلان کرتی ہے تاکہ relays اور clients incompatible peers کو deterministically reject کر سکیں۔

## Keys, suites, اور cadence

- **Identity requirements.** streaming کنٹرول فریمز ہمیشہ Ed25519 سے sign ہوتے ہیں۔ dedicated keys `streaming.identity_public_key`/`streaming.identity_private_key` کے ذریعے فراہم کی جا سکتی ہیں؛ بصورت دیگر node identity دوبارہ استعمال ہوتی ہے۔
- **HPKE suites.** `KeyUpdate` سب سے کم مشترک suite منتخب کرتا ہے؛ suite #1 لازمی ہے (`AuthPsk`, `Kyber768`, `HKDF-SHA3-256`, `ChaCha20-Poly1305`)، اور `Kyber1024` کا optional upgrade path موجود ہے۔ suite انتخاب session پر محفوظ ہوتا ہے اور ہر update پر validate ہوتا ہے۔
- **Rotation.** publishers ہر 64 MiB یا 5 منٹ بعد signed `KeyUpdate` جاری کرتے ہیں۔ `key_counter` کو سختی سے بڑھنا چاہیے؛ regression ایک سخت غلطی ہے۔ `ContentKeyUpdate` rolling Group Content Key تقسیم کرتا ہے، negotiated HPKE suite کے تحت wrap ہوتا ہے، اور ID + validity window کے ذریعے segment decryption کو gate کرتا ہے۔
- **Snapshots.** `StreamingSession::snapshot_state` اور `restore_from_snapshot` `{session_id, key_counter, suite, sts_root, cadence state}` کو `streaming.session_store_dir` (ڈیفالٹ `./storage/streaming`) کے تحت محفوظ کرتے ہیں۔ restore پر transport keys دوبارہ derive ہوتی ہیں تاکہ crashes session secrets لیک نہ کریں۔

## Runtime configuration

- **Key material.** dedicated keys کو `streaming.identity_public_key`/`streaming.identity_private_key` (Ed25519 multihash) کے ذریعے فراہم کریں، اور optional Kyber material `streaming.kyber_public_key`/`streaming.kyber_secret_key` سے دیں۔ defaults override کرتے وقت چاروں کی موجودگی ضروری ہے؛ `streaming.kyber_suite` `mlkem512|mlkem768|mlkem1024` قبول کرتا ہے (aliases `kyber512/768/1024`, ڈیفالٹ `mlkem768`).
- **SoraNet routes.** `streaming.soranet.*` anonymous transport کو کنٹرول کرتا ہے: `exit_multiaddr` (ڈیفالٹ `/dns/torii/udp/9443/quic`)، `padding_budget_ms` (ڈیفالٹ 25 ms)، `access_kind` (`authenticated` بمقابلہ `read-only`)، optional `channel_salt`، اور `provision_spool_dir` (ڈیفالٹ `./storage/streaming/soranet_routes`).
- **Sync gate.** `streaming.sync` audiovisual streams کے لئے drift enforcement ٹوگل کرتا ہے: `enabled`, `observe_only`, `ewma_threshold_ms`, اور `hard_cap_ms` طے کرتے ہیں کہ timing drift کی وجہ سے segments کب reject ہوں۔

## Validation اور fixtures

- canonical type definitions اور helpers `crates/iroha_crypto/src/streaming.rs` میں موجود ہیں۔
- integration coverage HPKE handshake، content-key distribution اور snapshot lifecycle (`crates/iroha_crypto/tests/streaming_handshake.rs`) کو exercise کرتی ہے۔ streaming surface کو مقامی طور پر verify کرنے کے لئے `cargo test -p iroha_crypto streaming_handshake` چلائیں۔
- layout، error handling اور مستقبل کی upgrades کی گہری وضاحت کے لئے ریپو روٹ میں `norito_streaming.md` پڑھیں۔
