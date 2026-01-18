---
lang: ar
direction: rtl
source: docs/portal/docs/norito/streaming.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 10ba7b91d73d6723c4b66491951c3257c48557273ab5424d81119e01c8f2c6e3
source_last_modified: "2025-12-09T06:48:00.858874+00:00"
translation_last_reviewed: 2025-12-30
---

# Norito Streaming

يعرّف Norito Streaming صيغة on-wire واطارات التحكم والcodec المرجعي المستخدم لتدفقات الوسائط الحية عبر Torii و SoraNet. المواصفة القياسية موجودة في `norito_streaming.md` بجذر workspace؛ هذه الصفحة تلخص الاجزاء التي يحتاجها المشغلون ومؤلفو SDK مع نقاط التهيئة.

## صيغة on-wire وخطة التحكم

- **Manifests و frames.** `ManifestV1` و `PrivacyRoute*` تصف خط الزمن للقطاعات ووصفات chunks وتلميحات المسارات. اطارات التحكم (`KeyUpdate` و `ContentKeyUpdate` و feedback الخاص بالcadence) تعيش بجانب manifest حتى يتمكن المشاهدون من التحقق من commitments قبل فك الترميز.
- **Codec الاساسي.** `BaselineEncoder`/`BaselineDecoder` تفرض معرفات chunk رتيبة، وحساب timestamps، والتحقق من commitments. يجب على المضيفين استدعاء `EncodedSegment::verify_manifest` قبل تقديم البيانات للمشاهدين او relays.
- **Feature bits.** تفاوض القدرات يعلن `streaming.feature_bits` (الافتراضي `0b11` = baseline feedback + privacy route provider) حتى تتمكن relays والعملاء من رفض peers غير المتوافقين بشكل حتمي.

## المفاتيح و suites و cadence

- **متطلبات الهوية.** اطارات التحكم في البث توقع دائما ب Ed25519. يمكن توفير مفاتيح مخصصة عبر `streaming.identity_public_key`/`streaming.identity_private_key`؛ والا تعاد هوية العقدة.
- **Suites الخاصة ب HPKE.** `KeyUpdate` يختار اقل suite مشتركة؛ suite رقم 1 الزاميه (`AuthPsk` و `Kyber768` و `HKDF-SHA3-256` و `ChaCha20-Poly1305`)، مع مسار ترقية اختياري الى `Kyber1024`. يتم حفظ اختيار suite على الجلسة والتحقق منه في كل تحديث.
- **التدوير.** يصدر الناشرون `KeyUpdate` موقعة كل 64 MiB او 5 دقائق. يجب ان يزيد `key_counter` بشكل صارم؛ التراجع خطا حرج. `ContentKeyUpdate` يوزع Group Content Key المتدوّرة، ملفوفة تحت suite HPKE المتفاوض عليها، ويقيد فك تشفير القطاعات حسب المعرف + نافذة الصلاحية.
- **Snapshots.** `StreamingSession::snapshot_state` و `restore_from_snapshot` تحفظ `{session_id, key_counter, suite, sts_root, cadence state}` تحت `streaming.session_store_dir` (الافتراضي `./storage/streaming`). يعاد اشتقاق مفاتيح النقل عند الاستعادة حتى لا تؤدي الاعطال الى تسريب اسرار الجلسة.

## تهيئة وقت التشغيل

- **مادة المفاتيح.** وفر مفاتيح مخصصة عبر `streaming.identity_public_key`/`streaming.identity_private_key` (multihash Ed25519) ومادة Kyber اختيارية عبر `streaming.kyber_public_key`/`streaming.kyber_secret_key`. يجب توفر الاربعة عند تجاوز الافتراضات؛ `streaming.kyber_suite` يقبل `mlkem512|mlkem768|mlkem1024` (الاسماء البديلة `kyber512/768/1024`، الافتراضي `mlkem768`).
- **حواجز codec.** يبقى CABAC معطلا ما لم تفعله عملية البناء؛ rANS المجمع يتطلب `ENABLE_RANS_BUNDLES=1`. يتم فرض ذلك عبر `streaming.codec.{entropy_mode,bundle_width,bundle_accel}` و `streaming.codec.rans_tables_path` الاختيارية عند تقديم جداول مخصصة. يجب ان يكون `bundle_width` المجمع بين 2 و 3 (شامل)؛ العرض 1 مخصص للارث.
- **مسارات SoraNet.** `streaming.soranet.*` يتحكم بالنقل المجهول: `exit_multiaddr` (الافتراضي `/dns/torii/udp/9443/quic`)، `padding_budget_ms` (الافتراضي 25 ms)، `access_kind` (`authenticated` مقابل `read-only`)، و `channel_salt` الاختياري، و `provision_spool_dir` (الافتراضي `./storage/streaming/soranet_routes`)، و `provision_spool_max_bytes` (الافتراضي 0، غير محدود)، و `provision_window_segments` (الافتراضي 4)، و `provision_queue_capacity` (الافتراضي 256).
- **بوابة المزامنة.** `streaming.sync` تبدل فرض الانحراف لتدفقات الصوت/الصورة: `enabled` و `observe_only` و `ewma_threshold_ms` و `hard_cap_ms` تحكم متى ترفض القطاعات بسبب انحراف التوقيت.

## التحقق وال fixtures

- تعاريف الانواع القياسية والمساعدات موجودة في `crates/iroha_crypto/src/streaming.rs`.
- تغطية التكامل تختبر HPKE handshake وتوزيع content-key ودورة حياة snapshots (`crates/iroha_crypto/tests/streaming_handshake.rs`). شغّل `cargo test -p iroha_crypto streaming_handshake` للتحقق من سطح البث محليا.
- للتعمق في layout ومعالجة الاخطاء والترقيات المستقبلية، اقرأ `norito_streaming.md` في جذر المستودع.
