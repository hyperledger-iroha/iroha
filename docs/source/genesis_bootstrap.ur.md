---
lang: ur
direction: rtl
source: docs/source/genesis_bootstrap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6feb2b03bd8f6a41de693a0c3f3c4ffc058072bc7942e2bc50b3fd9770aa56d4
source_last_modified: "2026-01-03T18:08:01.368173+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

قابل اعتماد ساتھیوں سے # جینیس بوٹسٹریپ

Iroha مقامی `genesis.file` کے بغیر ہم خیال افراد قابل اعتماد ساتھیوں سے دستخط شدہ جینیس بلاک لاسکتے ہیں
Norito-encoded بوٹسٹریپ پروٹوکول کا استعمال کرتے ہوئے۔

۔
  `GenesisResponse` فریم `request_id` کے ذریعہ کلیدی۔ جواب دہندگان میں چین کی شناخت ، دستخط کنندہ پبکی شامل ہیں ،
  ہیش ، اور اختیاری سائز کا اشارہ۔ پے لوڈ صرف `Fetch` پر واپس کیے جاتے ہیں ، اور ڈپلیکیٹ درخواست IDs
  `DuplicateRequest` وصول کریں۔
- ** گارڈز: ** جواب دہندگان ایک اجازت فہرست (`genesis.bootstrap_allowlist` یا قابل اعتماد ساتھیوں کو نافذ کرتے ہیں
  سیٹ) ، چین-آئی ڈی/پبکی/ہیش مماثل ، شرح کی حد (`genesis.bootstrap_response_throttle`) ، اور a
  سائز کیپ (`genesis.bootstrap_max_bytes`)۔ اجازت کی فہرست سے باہر کی درخواستیں `NotAllowed` وصول کرتی ہیں ، اور
  غلط کلید کے ذریعہ دستخط شدہ پے لوڈ `MismatchedPubkey` وصول کریں۔
- ** مطلوبہ بہاؤ: ** جب اسٹوریج خالی ہو اور `genesis.file` غیر سیٹ ہو (اور
  `genesis.bootstrap_enabled=true`) ، نوڈ اختیاری کے ساتھ قابل اعتماد ساتھیوں کو پیش کرتا ہے
  `genesis.expected_hash` ، پھر پے لوڈ کو لاتا ہے ، `validate_genesis_block` کے ذریعے دستخطوں کی توثیق کرتا ہے ،
  اور بلاک لگانے سے پہلے کورا کے ساتھ ساتھ `genesis.bootstrap.nrt` بھی برقرار رہتا ہے۔ بوٹسٹریپ دوبارہ کوشش کرتا ہے
  اعزاز `genesis.bootstrap_request_timeout` ، `genesis.bootstrap_retry_interval` ، اور
  `genesis.bootstrap_max_attempts`۔
- ** ناکامی کے طریقوں: ** اجازت دی گئی لسٹ لسٹ مسز ، چین/پبکی/ہیش مماثل ، سائز
  CAP کی خلاف ورزی ، شرح کی حدود ، مقامی پیدائش سے محروم ، یا ڈپلیکیٹ درخواست IDs۔ متضاد ہیش
  ساتھیوں کے پار بازیافت کا خاتمہ ؛ کوئی جواب دہندگان/ٹائم آؤٹ مقامی ترتیب میں واپس نہیں آتا ہے۔
- ** آپریٹر اقدامات: ** یقینی بنائیں کہ کم از کم ایک قابل اعتماد ہم مرتبہ ایک درست جینیسس ، تشکیل کے ساتھ قابل رسائ ہے
  `bootstrap_allowlist`/`bootstrap_max_bytes`/`bootstrap_response_throttle` اور دوبارہ کوشش کرنا ، اور
  مماثل پے لوڈ کو قبول کرنے سے بچنے کے لئے اختیاری طور پر `expected_hash` کو پن کریں۔ مستقل پے لوڈ ہوسکتا ہے
  `genesis.file` کو `genesis.bootstrap.nrt` کی طرف اشارہ کرکے بعد کے جوتے پر دوبارہ استعمال کیا گیا۔