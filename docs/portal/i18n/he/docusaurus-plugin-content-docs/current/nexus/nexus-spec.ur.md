---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-spec.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-spec
title: Sora Nexus کی تکنیکی اسپیسفیکیشن
description: `docs/source/nexus.md` کی مکمل عکاسی، جو Iroha 3 (Sora Nexus) لیجر کی معماری اور ڈیزائن پابندیوں کا احاطہ کرتی ہے۔
---

:::note דף הבית
یہ صفحہ `docs/source/nexus.md` کی عکاسی کرتا ہے۔ ترجمے کا بیک لاگ پورٹل تک پہنچنے تک دونوں نقول ہم آہنگ رکھیں۔
:::

#! Iroha 3 - Sora Nexus Ledger: تکنیکی ڈیزائن اسپیسفیکیشن

یہ دستاویز Iroha 3 کے لئے Sora Nexus Ledger کی معماری تجویز کرتی ہے، جو Iroha 2 מרחבי נתונים (DS) ‏ ‏ ‏ ‏ Data Spaces مضبوط پرائیویسی ڈومینز ("private data spaces") اور کھلی شمولیت ("public data spaces") فراہم کرتے ہیں۔ یہ ڈیزائن عالمی لیجر میں composability کو برقرار رکھتے ہوئے private-DS ڈیٹا کے لئے سخت isolation اور confidentiality یقینی بناتا ہے، اور Kura (block storage) اور WSV (World State View) میں erasure coding کے ذریعے data availability کو scale کرتا ہے۔

یہی ریپوزٹری Iroha 2 (self-hosted networks) اور Iroha 3 (SORA Nexus) دونوں کو build کرتی ہے۔ اجرا مشترک Iroha Virtual Machine (IVM) اور Kotodama toolchain سے چلتا ہے، اس لئے contracts اور bytecode artifacts خود میزبان ڈپلائمنٹس اور Nexus عالمی لیجر کے درمیان portable رہتے ہیں۔

اہداف
- ایک عالمی منطقی لیجر جو بہت سے تعاون کرنے والے validators اور Data Spaces پر مشتمل ہو۔
- permissioned آپریشن (مثلاً CBDCs) کے لئے Private Data Spaces، جہاں ڈیٹا کبھی private DS سے باہر نہیں جاتا۔
- מרחבי נתונים ציבוריים.
- Data Spaces کے پار composable smart contracts، مگر private-DS assets تک رسائی کے لئے واضح اجازتوں کے ساتھ۔
- בידוד ביצועים.
- data availability at scale: erasure-coded Kura اور WSV تاکہ عملی طور پر لامحدود ڈیٹا سپورٹ ہو جبکہ private-DS ڈیٹا نجی رہے۔

غیر اہداف (ابتدائی مرحلہ)
- token economics یا validator incentives کی تعریف؛ scheduling اور staking پالیسیز plug-able ہیں۔
- نئی ABI ورژن متعارف کرانا؛ تبدیلیاں IVM پالیسی کے مطابق explicit syscall اور pointer-ABI extensions کے ساتھ ABI v1 کو ہدف بناتی ہیں۔اصطلاحات
- Nexus פנקס חשבונות: דף חשבונות או מרחב נתונים (DS) תקשורת תקשורת, התחייבות לחיבור המדינה. بنتا ہے۔
- מרחב נתונים (DS): ביצוע פעולות או אחסון מערכות אימות, ממשל, מחלקת פרטיות, מדיניות DA, מכסות, מדיניות עמלות ועוד. دو کلاسز ہیں: public DS اور private DS۔
- מרחב נתונים פרטי: מאמתים מורשים או בקרת גישה; ٹرانزیکشن ڈیٹا اور state کبھی DS سے باہر نہیں جاتے۔ صرف commitments/metadata عالمی طور پر anchor ہوتے ہیں۔
- Public Data Space: permissionless شمولیت؛ مکمل ڈیٹا اور state عوامی طور پر دستیاب ہیں۔
- Data Space Manifest (DS Manifest): Norito-encoded manifest جو DS parameters (validators/QC keys, privacy class, ISI policy, DA parameters, retention, quotas, ZK policy, fees) ظاہر کرتا ہے۔ manifest hash nexus chain پر anchor ہوتا ہے۔ جب تک override نہ ہو، DS quorum certificates ML-DSA-87 (Dilithium5-class) کو default post-quantum signature scheme کے طور پر استعمال کرتے ہیں۔
- ספריית שטח: חוזה ספריות על-שרשרת, מניפסטים של DS, גרסאות, אירועי ממשל/רוטציה, פתירות וביקורות.
- DSID: Data Space کے لئے عالمی طور پر منفرد شناخت۔ تمام objects اور references کے لئے namespacing میں استعمال ہوتا ہے۔
- Anchor: DS block/header کا cryptographic commitment جو DS history کو عالمی لیجر میں باندھنے کے لئے nexus chain میں شامل ہوتا ہے۔
- Kura: Iroha אחסון בלוק. یہاں اسے erasure-coded blob storage اور commitments کے ساتھ توسیع دی گئی ہے۔
- WSV: Iroha World State View. קטעי מצב מקודדים למחיקה, בעלי גרסאות, תצלומי מצב, קטעי מצב מקודדים .
- IVM: smart contract execution کے لئے Iroha Virtual Machine (Kotodama bytecode `.to`)۔
  - AIR: ייצוג ביניים אלגברי. STARK طرز کے proofs کے لئے computation کا algebraic view، جو execution کو field-based traces میں transition اور boundary constraints کے ساتھ بیان کرتا ہے۔Data Spaces משחק
- Identity: `DataSpaceId (DSID)` DS کی شناخت کرتا ہے اور ہر چیز کو namespace کرتا ہے۔ DS دو granularities پر instantiate ہو سکتے ہیں:
  - Domain-DS: `ds::domain::<domain_name>` - execution اور state ایک domain تک محدود۔
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - execution اور state ایک واحد asset definition تک محدود۔
  دونوں شکلیں ساتھ موجود ہیں؛ ٹرانزیکشنز atomically متعدد DSIDs کو touch کر سکتی ہیں۔
- Manifest lifecycle: DS creation، updates (key rotation، policy changes)، اور retirement Space Directory میں ریکارڈ ہوتے ہیں۔ ہر per-slot DS artifact تازہ ترین manifest hash کو reference کرتا ہے۔
- שיעורים: DS ציבורי (השתתפות פתוחה, DA ציבורי) או DS פרטי (מורשה, DA סודי). hybrid policies manifest flags کے ذریعے ممکن ہیں۔
- Policies per DS: ISI permissions، DA parameters `(k,m)`، encryption، retention، quotas (per block tx share کی min/max)، ZK/optimistic proof policy، fees۔
- ממשל: חברות ב-DS, מניפסט רוטציה של validator, ממשל, ניהול, ניהול, ניהול, ניהול, גישה, (הצעות על שרשרת, multisig, 1, עסקאות nexus, או אישורים חיצוניים)

היכולת מתבטאת בשם UAID
- Universal accounts: ہر participant کو ایک deterministic UAID (`UniversalAccountId` in `crates/iroha_data_model/src/nexus/manifest.rs`) ملتا ہے جو تمام dataspaces پر محیط ہے۔ מניפסטים של יכולת (`AssetPermissionManifest`) UAID מרחב נתונים, תקופות הפעלה/תפוגה, או לאפשר/להכחיש `ManifestEntry` باندھتے ہیں جو `dataspace`, `program_id`, `method`, `asset` اور اختیاری AMX roles کو محدود کرتی ہے۔ Deny قواعد ہمیشہ غالب رہتے ہیں؛ evaluator یا تو audit reason کے ساتھ `ManifestVerdict::Denied` جاری کرتا ہے یا matching allowance metadata کے ساتھ `Allowed` grant دیتا ہے۔
- Allowances: ہر allow entry میں deterministic `AllowanceWindow` buckets (`PerSlot`, `PerMinute`, `PerDay`) کے ساتھ ایک اختیاری `max_amount` ہوتا ہے۔ Hosts اور SDKs ایک ہی Norito payload استعمال کرتے ہیں، اس لئے enforcement hardware اور SDK implementations میں یکساں رہتی ہے۔
- Audit telemetry: Space Directory جب بھی کسی manifest کا state بدلتا ہے تو `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) نشر کرتا ہے۔ מנויי `SpaceDirectoryEventFilter` משטח Torii/data-event, עדכוני מניפסט UAID, ביטולים, או הכחיש-זכיות, מערכות אינסטלציה מותאמת אישית دیتی ہے۔

End-to-end operator evidence، SDK migration notes، اور manifest publishing checklists کے لئے اس حصے کو Universal Account Guide (`docs/source/universal_accounts_guide.md`) کے ساتھ mirror کریں۔ מדיניות UAIDاعلی سطحی معماری
1) שכבת קומפוזיציה גלובלית (שרשרת Nexus)
- 1 קוביות Nexus בלוקים. زیادہ Data Spaces (DS) پر پھیلی atomic ٹرانزیکشنز کو finalize کرتا ہے۔ ہر committed ٹرانزیکشن متحد عالمی world state کو اپ ڈیٹ کرتی ہے (per-DS roots کا vector)۔
- کم سے کم metadata کے ساتھ aggregated proofs/QCs شامل ہوتے ہیں تاکہ composability، finality، اور fraud detection یقینی ہو (touch کیے گئے DSIDs، per-DS state roots پہلے/بعد، DA commitments، per-DS validity proofs، اور ML-DSA-87 والا DS quorum certificate)۔ کوئی private data شامل نہیں ہوتا۔
- Consensus: ایک واحد global، pipelined BFT committee جس کا سائز 22 (3f+1 with f=7) ہے، جو ~200k ممکنہ validators کے pool سے epochal VRF/stake mechanism کے ذریعے منتخب ہوتا ہے۔ nexus committee ٹرانزیکشنز کو sequence کرتا ہے اور بلاک کو 1s میں finalize کرتا ہے۔

2) שכבת מרחב נתונים (ציבורי/פרטי)
- global ٹرانزیکشنز کے per-DS fragments execute کرتا ہے، DS-local WSV کو update کرتا ہے، اور per-block validity artifacts (per-DS aggregated proofs اور DA commitments) بناتا ہے جو 1-second Nexus Block میں roll up ہوتے ہیں۔
- Private DS data-at-rest اور data-in-flight کو authorized validators کے درمیان encrypt کرتے ہیں؛ صرف commitments اور PQ validity proofs DS سے باہر جاتے ہیں۔
- Public DS مکمل data bodies (via DA) اور PQ validity proofs export کرتے ہیں۔

3) עסקאות אטומיות בין נתונים-חלל (AMX)
- Model: ہر user transaction متعدد DS کو touch کر سکتی ہے (مثلاً domain DS اور ایک یا زیادہ asset DS)۔ یہ ایک ہی Nexus Block میں atomically commit ہوتی ہے یا abort؛ جزوی اثرات نہیں۔
- Prepare-Commit within 1s: ہر candidate transaction کے لئے touched DS ایک ہی snapshot (slot کے آغاز کے DS roots) کے خلاف parallel execute کرتے ہیں اور per-DS PQ validity proofs (FASTPQ-ISI) اور DA commitments بناتے ہیں۔ nexus committee ٹرانزیکشن کو تبھی commit کرتا ہے جب تمام مطلوبہ DS proofs verify ہوں اور DA certificates وقت پر پہنچیں (ہدف <=300 ms)؛ ورنہ ٹرانزیکشن اگلے slot کے لئے re-schedule ہوتی ہے۔
- Consistency: Read-write sets declare ہوتے ہیں؛ conflict detection start-of-slot roots کے خلاف commit پر ہوتی ہے۔ lock-free optimistic execution per DS global stalls سے بچاتی ہے؛ atomicity nexus commit rule سے نافذ ہوتی ہے (DS کے درمیان سب یا کچھ نہیں)۔
- Privacy: Private DS صرف pre/post DS roots سے بندھے proofs/commitments export کرتے ہیں۔ کوئی raw private data DS سے باہر نہیں جاتا۔

4) זמינות נתונים (DA) עם קידוד מחיקה
- Kura block bodies اور WSV snapshots کو erasure-coded blobs کے طور پر ذخیرہ کرتا ہے۔ public blobs وسیع پیمانے پر shard ہوتے ہیں؛ private blobs صرف private-DS validators میں، encrypted chunks کے ساتھ محفوظ ہوتے ہیں۔
- DA commitments DS artifacts اور Nexus Blocks دونوں میں ریکارڈ ہوتے ہیں، جس سے sampling اور recovery guarantees ممکن ہوتی ہیں بغیر private contents ظاہر کیے۔بلاک اور کمٹ ڈھانچہ
- Data Space Proof Artifact (ہر 1s slot، ہر DS)
  - فیلڈز: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - Private-DS artifacts data bodies کے بغیر export کرتے ہیں؛ public DS DA کے ذریعے body retrieval کی اجازت دیتے ہیں۔

- בלוק Nexus (1 שניות)
  - فیلڈز: block_number, parent_hash, slot_time, tx_list (DSIDs touched کے ساتھ atomic cross-DS transactions), ds_artifacts[], nexus_qc.
  - فنکشن: وہ تمام atomic ٹرانزیکشنز finalize کرتا ہے جن کے مطلوبہ DS artifacts verify ہوں؛ global world state vector کے DS roots کو ایک ہی قدم میں update کرتا ہے۔

קונצנזוס או תזמון
- קונצנזוס שרשרת Nexus: BFT גלובלי יחיד בצינור (מחלקה Sumeragi) ועדת 22 צמתים (3f+1 עם f=7) 1s בלוקים או סופיות 1s ہے۔ committee members ~200k candidates کے pool سے epochal VRF/stake کے ذریعے منتخب ہوتے ہیں؛ rotation decentralization اور censorship resistance برقرار رکھتی ہے۔
- Data Space Consensus: ہر DS اپنے validators کے درمیان اپنا BFT چلاتا ہے تاکہ per-slot artifacts (proofs، DA commitments، DS QC) بنائے جا سکیں۔ ועדות ממסר נתיבים `3f+1` מרחב נתונים `fault_tolerance` מרחבי נתונים תקפים של פרק נתונים תקף פרק זמן תקף sample ہوتے ہیں، VRF epoch seed کو `(dataspace_id, lane_id)` کے ساتھ bind کر کے۔ Private DS permissioned ہیں؛ public DS anti-Sybil پالیسیز کے تحت open liveness دیتے ہیں۔ global nexus committee تبدیل نہیں ہوتا۔
- Transaction Scheduling: صارفین atomic ٹرانزیکشنز submit کرتے ہیں جن میں touched DSIDs اور read-write sets declare ہوتے ہیں۔ DS slot کے اندر parallel execute کرتے ہیں؛ nexus committee ٹرانزیکشن کو 1s بلاک میں تبھی شامل کرتا ہے جب تمام DS artifacts verify ہوں اور DA certificates بروقت ہوں (<=300 אלפיות השנייה)
- Performance Isolation: ہر DS کے پاس independent mempools اور execution ہوتی ہے۔ מכסות ל-DS سکتی ہیں تاکہ head-of-line blocking سے بچا جا سکے اور private DS latency محفوظ رہے۔

מרווח שמות
- DS-Qualified IDs: تمام entities (domains, accounts, assets, roles) `dsid` کے ساتھ qualify ہوتے ہیں۔ ערך: `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Global References: ایک global reference ایک tuple `(dsid, object_id, version_hint)` ہے اور اسے nexus layer پر on-chain یا cross-DS استعمال کے لئے AMX descriptors میں رکھا جا ‏
- Norito Serialization: تمام cross-DS messages (AMX descriptors، proofs) Norito codecs استعمال کرتے ہیں۔ production paths میں serde کا استعمال نہیں ہوتا۔Smart Contracts اور IVM توسیعات
- Execution Context: IVM execution context میں `dsid` شامل کریں۔ Kotodama contracts ہمیشہ کسی مخصوص Data Space کے اندر execute ہوتے ہیں۔
- פרימיטיבים אטומיים חוצה DS:
  - `amx_begin()` / `amx_commit()` IVM host میں atomic multi-DS ٹرانزیکشن کو delineate کرتے ہیں۔
  - `amx_touch(dsid, key)` conflict detection کے لئے slot snapshot roots کے خلاف read/write intent declare کرتا ہے۔
  - `verify_space_proof(dsid, proof, statement)` -> bool
  - `use_asset_handle(handle, op, amount)` -> result (operation تبھی permitted ہے جب policy اجازت دے اور handle valid ہو)
- עמלות מטפלות בנכס:
  - Asset operations DS کے ISI/role policies کے تحت authorize ہوتے ہیں؛ fees DS کے gas token میں ادا ہوتی ہیں۔ optional capability tokens اور زیادہ rich policies (multi-approver, rate-limits, geofencing) بعد میں atomic ماڈل بدلے بغیر شامل کی جا سکتی ہیں۔
- Determinism: تمام نئی syscalls inputs اور declared AMX read/write sets کے تحت pure اور deterministic ہیں۔ وقت یا ماحول کے hidden effects نہیں ہوتے۔הוכחות תוקף פוסט-קוונטי (ISI כללי)
- FASTPQ-ISI (PQ, no trusted setup): ایک kernelized، hash-based argument جو transfer ڈیزائن کو تمام ISI families تک عام کرتا ہے جبکہ GPU-class hardware پر 20k-scale batches کے لئے sub-second proving کو ہدف بناتا ہے۔
  - פרופיל תפעולי:
    - Production nodes prover کو `fastpq_prover::Prover::canonical` کے ذریعے construct کرتے ہیں، جو اب ہمیشہ production backend initialise کرتا ہے؛ deterministic mock ہٹا دیا گیا ہے۔ [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (קונפיגורציה) אופרטורים `irohad --fastpq-execution-mode` או ביצוע מעבד/GPU הצמד באופן דטרמיניסטי requested/resolved/backend triples کو ریکارڈ کرتا ہے۔ [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/888.rs:8
- אריתמטיזציה:
  - KV-Update AIR: WSV کو Poseidon2-SMT کے ذریعے committed typed key-value map کے طور پر treat کرتا ہے۔ ہر ISI keys (accounts, assets, roles, domains, metadata, supply) پر read-check-write rows کے چھوٹے سیٹ میں expand ہوتا ہے۔
  - Opcode-gated constraints: selector columns کے ساتھ ایک ہی AIR table per-ISI قواعد نافذ کرتی ہے (conservation, monotonic counters, permissions, range checks, bounded metadata updates)۔
  - Lookup arguments: permissions/roles، asset precisions، اور policy parameters کے لئے transparent hash-committed tables heavy bitwise constraints سے بچتی ہیں۔
- התחייבויות ועדכונים של המדינה:
  - הוכחת SMT מצטברת: מקשי נגיעה (לפני/פוסט) `old_root`/`new_root` האחים הוציאו מומים.
  - Invariants: global invariants (مثلاً ہر asset کی total supply) effect rows اور tracked counters کے درمیان multiset equality کے ذریعے نافذ ہوتے ہیں۔
- מערכת הוכחה:
  - FRI-style polynomial commitments (DEEP-FRI) high arity (8/16) اور blow-up 8-16 کے ساتھ؛ Hashs של פוסידון 2; Fiat-Shamir transcript SHA-2/3 کے ساتھ۔
  - Optional recursion: اگر ضرورت ہو تو micro-batches کو ایک proof فی slot compress کرنے کے لئے DS-local recursive aggregation۔
- היקף ודוגמאות מכוסות:
  - נכסים: העברה, טביעה, צריבה, רישום/ביטול רישום של הגדרות נכסים, הגדרת דיוק (מוגבל), הגדרת מטא נתונים.
  - Accounts/Domains: create/remove, set key/threshold, add/remove signatories (state-only؛ signature checks DS validators attest کرتے ہیں، AIR کے اندر prove نہیں ہوتے)۔
  - תפקידים/הרשאות (ISI): הענק/בטל תפקידים או הרשאות; lookup tables اور monotonic policy checks سے enforce ہوتے ہیں۔
  - Contracts/AMX: AMX begin/commit markers، capability mint/revoke اگر enabled ہو؛ state transitions اور policy counters کے طور پر prove ہوتے ہیں۔
- בדיקות מחוץ לאוויר כדי לשמור על זמן אחזור:
  - Signatures اور heavy cryptography (مثلاً ML-DSA user signatures) DS validators verify کرتے ہیں اور DS QC میں attest ہوتے ہیں؛ validity proof صرف state consistency اور policy compliance کو cover کرتا ہے۔ یہ proofs کو PQ اور تیز رکھتا ہے۔- יעדי ביצועים (ממחיש, מעבד 32 ליבות + GPU חדיש מודרני):
  - 20,000 ISI מעורבים עם מגע קטן (<=8 מקשים/ISI): ~0.4-0.9 שניות הוכחה, הוכחה של ~150-450 KB, אימות של ~5-15 אלפיות השנייה.
  - Heavier ISIs (more keys/rich constraints): micro-batch (مثلاً 10x2k) + recursion تاکہ per-slot <1 s رہے۔
- תצורת מניפסט DS:
  - `zk.policy = "fastpq_isi"`
  - `zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (signatures DS QC verify کرتا ہے)
  - `attestation.qc_signature = "ml_dsa_87"` (default؛ alternatives کو explicit طور پر declare کرنا ہوگا)
- נפילות:
  - Complex/custom ISIs عمومی STARK (`zk.policy = "stark_fri_general"`) استعمال کر سکتے ہیں جس میں deferred proof اور 1 s finality QC attestation + slashing کے ذریعے ہوتی ہے۔
  - Non-PQ options (مثلاً Plonk with KZG) trusted setup چاہتی ہیں اور default build میں اب supported نہیں ہیں۔

AIR تعارف (Nexus کے لئے)
- Execution trace: ایک matrix جس کی width (register columns) اور length (steps) ہوتی ہے۔ ہر row ISI processing کا منطقی قدم ہے؛ columns میں pre/post values، selectors، اور flags ہوتے ہیں۔
- אילוצים:
  - Transition constraints: row-to-row relations نافذ کرتی ہیں (مثلاً post_balance = pre_balance - amount جب `sel_transfer = 1` والی debit row ہو)۔
  - Boundary constraints: public I/O (old_root/new_root, counters) کو پہلی/آخری rows سے bind کرتی ہیں۔
  - Lookups/permutations: committed tables (permissions, asset params) کے خلاف membership اور multiset equalities کو یقینی بناتی ہیں بغیر bit-heavy circuits کے۔
- מחויבות ואימות:
  - Prover traces کو hash-based encodings کے ذریعے commit کرتا ہے اور low-degree polynomials بناتا ہے جو تبھی valid ہوں جب constraints پوری ہوں۔
  - Verifier FRI (hash-based, post-quantum) کے ذریعے low-degree check کرتا ہے، چند Merkle openings کے ساتھ؛ cost steps کے log پر منحصر ہے۔
- Example (Transfer): registers میں pre_balance، amount، post_balance، nonce، اور selectors شامل ہیں۔ אילוצים לא שליליות/טווח, שימור, או מונוטוניות בלתי רגילה

ABI اور Syscall ارتقا (ABI v1)
- סיסמאות להוספה (שמות להמחשה):
  - `SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- סוגי Pointer-ABI להוספה:
  - `PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- עדכונים נדרשים:
  - `ivm::syscalls::abi_syscall_list()` میں شامل کریں (ordering برقرار رکھیں)، policy کے ذریعے gate کریں۔
  - unknown numbers کو hosts میں `VMError::UnknownSyscall` پر map کریں۔
  - עדכון בדיקות: רשימת syscall golden, ABI hash, זיהוי סוג מצביע goldens, או בדיקות מדיניות.
  - מסמכים: `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.קובץ פרטיות
- Private Data Containment: private DS کے لئے transaction bodies، state diffs، اور WSV snapshots کبھی private validator subset سے باہر نہیں جاتے۔
- Public Exposure: صرف headers، DA commitments، اور PQ validity proofs export ہوتے ہیں۔
- Optional ZK Proofs: Private DS ZK proofs (مثلاً balance کافی ہے، policy پوری ہوئی) بنا سکتے ہیں تاکہ internal state ظاہر کئے بغیر cross-DS actions ممکن ہوں۔
- Access Control: authorization DS کے اندر ISI/role policies کے ذریعے نافذ ہوتی ہے۔ capability tokens اختیاری ہیں اور بعد میں شامل کئے جا سکتے ہیں۔

בידוד ביצועים או QoS
- ہر DS کے لئے consensus، mempools، اور storage الگ۔
- Nexus מכסות תזמון לכל זמן הכללת עוגן DS
- Contract resource budgets per DS (compute/memory/IO) IVM host کے ذریعے نافذ ہوتے ہیں۔ Public-DS contention private-DS budgets استعمال نہیں کر سکتی۔
- Asynchronous cross-DS calls private-DS execution کے اندر لمبی synchronous waits سے بچاتی ہیں۔

זמינות נתונים או עיצוב אחסון
1) קידוד מחיקה
- Kura blocks اور WSV snapshots کے blob-level erasure coding کے لئے systematic Reed-Solomon (مثلاً GF(2^16)) استعمال کریں: parameters `(k, m)` جہاں `n = k + m` shards ہیں۔
- Default parameters (proposed, public DS): `k=32, m=16` (n=48)، جس سے ~1.5x expansion کے ساتھ 16 shards تک recovery ممکن ہے۔ Private DS کے لئے: `k=16, m=8` (n=24) permissioned set کے اندر۔ دونوں DS Manifest کے ذریعے configurable ہیں۔
- Public Blobs: shards بہت سے DA nodes/validators میں distribute ہوتے ہیں اور sampling-based availability checks ہوتے ہیں۔ headers میں DA commitments light clients کو verify کرنے دیتے ہیں۔
- Private Blobs: shards encrypt ہوتے ہیں اور صرف private-DS validators (یا designated custodians) کے اندر distribute ہوتے ہیں۔ global chain صرف DA commitments رکھتی ہے (shard locations یا keys نہیں)۔

2) התחייבויות ודגימה
- ہر blob کے لئے shards پر Merkle root نکال کر `*_da_commitment` میں شامل کریں۔ elliptic-curve commitments سے بچ کر PQ رہیں۔
- DA Attesters: VRF-sampled regional attesters (مثلاً ہر region میں 64) successful shard sampling کی attestation کے لئے ML-DSA-87 certificate جاری کرتے ہیں۔ ہدف DA attestation latency <=300 ms ہے۔ nexus committee shards لینے کے بجائے certificates validate کرتی ہے۔

3) שילוב קורה
- Blocks transaction bodies کو Merkle commitments کے ساتھ erasure-coded blobs کے طور پر store کرتے ہیں۔
- Headers blob commitments رکھتے ہیں؛ bodies public DS کے لئے DA network اور private DS کے لئے private channels کے ذریعے قابلِ بازیافت ہیں۔4) שילוב WSV
- צילום Snapshot WSV: מצב DS תקופתי, תמונות מצב מקודדות מחיקות, כותרות מחויבויות מחויבות מקודדות מחיקות. snapshots کے درمیان change logs برقرار رہتے ہیں۔ public snapshots وسیع پیمانے پر shard ہوتے ہیں؛ private snapshots private validators کے اندر رہتے ہیں۔
- גישה נושאת הוכחות: חוזים הוכחות מדינה (Merkle/Verkle) Private DS خام proofs کے بجائے zero-knowledge attestations دے سکتے ہیں۔

5) שמירה וגיזום
- Public DS کے لئے pruning نہیں: تمام Kura bodies اور WSV snapshots DA کے ذریعے retain کریں (horizontal scaling)۔ Private DS internal retention define کر سکتے ہیں، مگر exported commitments immutable رہتے ہیں۔ Nexus layer تمام Nexus Blocks اور DS artifact commitments برقرار رکھتی ہے۔

רשת או תפקידי צומת
- Global Validators: nexus consensus میں حصہ لیتے ہیں، Nexus Blocks اور DS artifacts validate کرتے ہیں، public DS کے لئے DA checks کرتے ہیں۔
- Data Space Validators: DS consensus چلاتے ہیں، contracts execute کرتے ہیں، local Kura/WSV manage کرتے ہیں، اپنے DS کے لئے DA handle کرتے ہیں۔
- DA Nodes (optional): public blobs store/publish کرتے ہیں، sampling میں مدد دیتے ہیں۔ Private DS کے لئے، DA nodes validators یا trusted custodians کے ساتھ co-locate ہوتے ہیں۔שיפורים ברמת המערכת
- Sequencing/mempool decoupling: DAG mempool (مثلاً Narwhal-style) اپنائیں جو nexus layer میں pipelined BFT کو feed کرے تاکہ latency کم اور throughput بہتر ہو، بغیر منطقی ماڈل بدلے۔
- מכסות DS והוגנות: מכסות ל-DS לכל בלוק או מכסות משקל חוסמות ראש קו וסתייגויות DS פרטיות ‏ لئے۔
- DS attestation (PQ): default DS quorum certificates ML-DSA-87 (Dilithium5-class) استعمال کرتے ہیں۔ یہ post-quantum ہے اور EC signatures سے بڑا ہے مگر ایک QC فی slot قابلِ قبول ہے۔ DS اگر DS Manifest میں declare کریں تو ML-DSA-65/44 (چھوٹا) یا EC signatures منتخب کر سکتے ہیں؛ public DS کے لئے ML-DSA-87 برقرار رکھنے کی سخت سفارش ہے۔
- DA attesters: public DS کے لئے VRF-sampled regional attesters DA certificates جاری کرتے ہیں۔ nexus committee raw shard sampling کے بجائے certificates validate کرتی ہے؛ private DS اندرونی DA attestations رکھتے ہیں۔
- הוכחות רקורסיה ועידן: הוכחה רקורסיבית לכל משבצת/עידן גודל הוכחה מצטבר time high load پر مستحکم رہیں۔
- Lane scaling (اگر ضرورت ہو): اگر ایک global committee bottleneck بن جائے تو K parallel sequencing lanes متعارف کریں جن کی deterministic merge ہو۔ اس سے single global order برقرار رہتا ہے جبکہ horizontal scaling ہو جاتی ہے۔
- Deterministic acceleration: hashing/FFT کے لئے SIMD/CUDA feature-gated kernels دیں اور bit-exact CPU fallback رکھیں تاکہ cross-hardware determinism برقرار رہے۔
- Lane activation thresholds (proposal): 2-4 lanes فعال کریں اگر (a) p95 finality 1.2 s سے زیادہ ہو >3 مسلسل منٹ، یا (b) per-block occupancy 85% سے زیادہ ہو >5 منٹ، یا (c) incoming tx rate sustained سطحوں پر block capacity کی >1.2x ضرورت رکھتی ہو۔ lanes deterministic طور پر DSID hash کے ذریعے transactions کو bucket کرتی ہیں اور nexus block میں merge ہوتی ہیں۔

Fees اور Economics (ابتدائی defaults)
- Gas unit: per-DS gas token جس میں compute/IO metered ہوتا ہے؛ fees DS کے native gas asset میں ادا ہوتی ہیں۔ DS کے درمیان conversion application کی ذمہ داری ہے۔
- Inclusion priority: round-robin across DS کے ساتھ per-DS quotas تاکہ fairness اور 1s SLOs برقرار رہیں؛ DS کے اندر fee bidding tie-break کر سکتی ہے۔
- Future: optional global fee market یا MEV-minimizing policies explore کی جا سکتی ہیں بغیر atomicity یا PQ proof design بدلے۔Cross-Data-Space workflow (مثال)
1) ایک صارف AMX ٹرانزیکشن submit کرتا ہے جو public DS P اور private DS S کو touch کرتی ہے: asset X کو S سے beneficiary B تک منتقل کریں جس کی account P میں ہے۔
2) slot کے اندر، P اور S ہر ایک اپنا fragment slot snapshot کے خلاف execute کرتے ہیں۔ S authorization اور availability verify کرتا ہے، اپنا internal state update کرتا ہے، اور PQ validity proof اور DA commitment بناتا ہے (کوئی private data leak نہیں ہوتا)۔ P متعلقہ state update تیار کرتا ہے (مثلاً policy کے مطابق P میں mint/burn/locking) اور اپنی proof۔
3) nexus committee دونوں DS proofs اور DA certificates verify کرتی ہے؛ اگر دونوں slot کے اندر verify ہوں تو ٹرانزیکشن 1s Nexus Block میں atomically commit ہوتی ہے، global world state vector میں دونوں DS roots update ہوتے ہیں۔
4) اگر کوئی proof یا DA certificate غائب/invalid ہو تو ٹرانزیکشن abort ہو جاتی ہے (کوئی اثر نہیں)، اور client اگلے slot کے لئے دوبارہ بھیج سکتا ہے۔ کسی مرحلے پر S سے کوئی private data باہر نہیں جاتا۔

- سکیورٹی پر غور و فکر
- Deterministic Execution: IVM syscalls deterministic رہتے ہیں؛ cross-DS Name
- בקרת גישה: הרשאות DS פרטיות של ISI. capability tokens cross-DS استعمال کے لئے fine-grained حقوق encode کرتے ہیں۔
- Confidentiality: private-DS data کے لئے end-to-end encryption، erasure-coded shards صرف authorized members میں ذخیرہ، بیرونی attestations کے لئے optional ZK proofs۔
- DoS Resistance: mempool/consensus/storage layers میں isolation public congestion کو private-DS progress پر اثر انداز ہونے سے روکتی ہے۔

רכיבי Iroha
- iroha_data_model: `DataSpaceId`, DS-qualified identifiers, AMX descriptors (read/write sets), proof/DA commitment types متعارف کریں۔ serialization صرف Norito۔
- ivm: AMX (`amx_begin`, `amx_commit`, `amx_touch`) اور DA proofs کے لئے syscalls اور pointer-ABI types شامل کریں؛ ABI tests/docs کو v1 پالیسی کے مطابق اپ ڈیٹ کریں۔