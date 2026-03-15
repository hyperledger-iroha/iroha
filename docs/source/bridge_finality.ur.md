---
lang: ur
direction: rtl
source: docs/source/bridge_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2e4c6ed5974f623906f51259a634bcad5df703bcec899630ae29f4669b289ab6
source_last_modified: "2026-01-08T21:52:45.509525+00:00"
translation_last_reviewed: 2026-01-08
---

<div dir="rtl">

<!-- اردو ترجمہ برائے docs/source/bridge_finality.md -->

<!--
SPDX-License-Identifier: Apache-2.0
-->

# Bridge finality proofs

یہ دستاویز Iroha کے لیے bridge finality proof سطح کی ابتدائی وضاحت کرتی ہے۔
مقصد یہ ہے کہ بیرونی چینز یا light clients یہ تصدیق کر سکیں کہ Iroha بلاک
finalized ہے، بغیر off-chain computation یا trusted relays کے۔

## Proof format

`BridgeFinalityProof` (Norito/JSON) میں شامل ہے:

- `height`: بلاک کی height۔
- `chain_id`: Iroha chain identifier تاکہ cross-chain replay روکا جا سکے۔
- `block_header`: canonical `BlockHeader`۔
- `block_hash`: header کا hash (clients دوبارہ حساب کرتے ہیں تاکہ validate ہو)۔
- `commit_certificate`: validator set + signatures جو بلاک کو finalize کرتے ہیں۔
- `validator_set_pops`: PoP bytes جو validator set کے ترتیب کے مطابق ہوں
  (BLS aggregate verification کے لئے ضروری)۔

Proof خود کافی ہے؛ کسی بیرونی manifests یا opaque blobs کی ضرورت نہیں۔
Retention: Torii حالیہ commit-certificate window کیلئے finality proofs فراہم کرتا ہے
(configured history cap سے محدود؛ ڈیفالٹ 512 entries via
`sumeragi.commit_cert_history_cap` / `SUMERAGI_COMMIT_CERT_HISTORY_CAP`). اگر clients کو
زیادہ طویل horizons درکار ہوں تو proofs کو cache یا anchor کریں۔
Canonical tuple `(block_header, block_hash, commit_certificate)` ہے: header کا hash
commit certificate کے اندر موجود hash سے match ہونا چاہیے، اور chain id proof کو
ایک ہی ledger سے باندھتا ہے۔ جب certificate مختلف block hash کی طرف اشارہ کرے تو
servers `CommitCertificateHashMismatch` reject اور log کرتے ہیں۔

## Commitment bundle

`BridgeFinalityBundle` (Norito/JSON) بنیادی proof کو واضح commitment اور justification
کے ساتھ بڑھاتا ہے:

- `commitment`: `{ chain_id, authority_set { id, validator_set, validator_set_hash, validator_set_hash_version }, block_height, block_hash, mmr_root?, mmr_leaf_index?, mmr_peaks?, next_authority_set? }`
- `justification`: authority set کے signatures جو commitment payload پر ہوتے ہیں
  (commit-certificate signatures دوبارہ استعمال ہوتے ہیں)۔
- `block_header`, `commit_certificate`: بنیادی proof جیسے۔

موجودہ placeholder: `mmr_root`/`mmr_peaks` کو block-hash MMR کو میموری میں دوبارہ compute
کر کے نکالا جاتا ہے؛ inclusion proofs ابھی واپس نہیں کی جاتیں۔ Clients آج بھی commitment
payload کے ذریعے وہی hash verify کر سکتے ہیں۔

MMR peaks are ordered left to right. Recompute `mmr_root` by bagging peaks
from right to left: `root = H(p_n, H(p_{n-1}, ... H(p_1, p_0)))`.

API: `GET /v2/bridge/finality/bundle/{height}` (Norito/JSON).

Verification بنیادی proof جیسی ہے: header سے `block_hash` دوبارہ compute کریں، commit
certificate signatures verify کریں، اور commitment fields کو certificate اور block hash
کے مطابق چیک کریں۔ Bundle commitment/justification wrapper فراہم کرتا ہے اُن bridge
protocols کیلئے جو separation کو ترجیح دیتے ہیں۔

## Verification steps

1. `block_header` سے `block_hash` دوبارہ compute کریں؛ mismatch پر reject کریں۔
2. چیک کریں کہ `commit_certificate.block_hash` دوبارہ compute شدہ `block_hash` سے match کرتا ہے؛
   mismatched header/commit certificate جوڑوں کو reject کریں۔
3. `chain_id` کو متوقع Iroha chain سے match کریں۔
4. `commit_certificate.validator_set` سے `validator_set_hash` دوبارہ compute کریں اور
   اسے ریکارڈ شدہ hash/version سے match کریں۔
5. `validator_set_pops` کی لمبائی validator set سے match کریں اور ہر PoP کو اس کے BLS public key کے
   خلاف validate کریں۔
6. commit certificate کی signatures کو header hash کے خلاف verify کریں، متعلقہ validator
   public keys اور indices استعمال کرتے ہوئے؛ quorum نافذ کریں (`2f+1` جب `n>3`، ورنہ `n`)
   اور duplicate/out-of-range indices کو reject کریں۔
7. اختیاری طور پر trusted checkpoint سے bind کریں، validator set hash کو anchored value سے compare کر کے
   (weak-subjectivity anchor)۔
8. اختیاری طور پر متوقع epoch anchor سے bind کریں تاکہ پرانے/نئے epochs کے proofs تب تک reject رہیں
   جب تک anchor کو جان بوجھ کر rotate نہ کیا جائے۔

`BridgeFinalityVerifier` (`iroha_data_model::bridge` میں) یہ چیکس لاگو کرتا ہے، chain-id/height
 drift، validator-set hash/version mismatches، missing/invalid PoP، duplicate/out-of-range signers،
invalid signatures، اور unexpected epochs کو quorum گننے سے پہلے reject کرتا ہے تاکہ light clients
ایک ہی verifier استعمال کر سکیں۔

## Reference verifier

`BridgeFinalityVerifier` متوقع `chain_id` کے ساتھ optional trusted validator-set اور epoch anchors
قبول کرتا ہے۔ یہ header/block-hash/commit-certificate tuple کو enforce کرتا ہے، validator-set
hash/version کو validate کرتا ہے، signatures/quorum کو advertised validator roster کے خلاف
چیک کرتا ہے، اور latest height کو track کر کے stale/skipped proofs reject کرتا ہے۔ جب anchors
مہیا ہوں تو یہ epochs/rosters کے درمیان replays کو `UnexpectedEpoch`/`UnexpectedValidatorSet`
errors کے ساتھ reject کرتا ہے؛ anchors کے بغیر یہ پہلی proof کے validator-set hash اور epoch کو
اپنا لیتا ہے، پھر duplicate/out-of-range/insufficient signatures کیلئے deterministic errors
نافذ کرتا ہے۔

## API surface

- `GET /v2/bridge/finality/{height}` - مطلوبہ block height کیلئے `BridgeFinalityProof` واپس کرتا ہے۔
  `Accept` کے ذریعے content negotiation Norito یا JSON کو سپورٹ کرتی ہے۔
- `GET /v2/bridge/finality/bundle/{height}` - مطلوبہ height کیلئے `BridgeFinalityBundle`
  (commitment + justification + header/certificate) واپس کرتا ہے۔

## Notes and follow-ups

- Proofs فی الحال stored commit certificates سے derive ہوتی ہیں۔ محدود history commit certificate
  retention window کے مطابق چلتی ہے؛ اگر طویل horizons درکار ہوں تو clients کو anchor proofs
  cache کرنا چاہئیں۔ window سے باہر کی درخواستیں `CommitCertificateNotFound(height)` دیتی ہیں؛
  error دکھائیں اور anchored checkpoint پر fallback کریں۔
- Replayed یا forged proof جس میں `block_hash` mismatch ہو (header vs. certificate) اسے
  `CommitCertificateHashMismatch` کے ساتھ reject کیا جاتا ہے؛ clients کو signature verification
  سے پہلے وہی tuple check کرنا چاہیے اور mismatched payloads کو discard کرنا چاہیے۔
- مستقبل میں MMR/authority-set commitment chains شامل کر کے بہت طویل histories کیلئے proof size
  commitment envelopes میں wrap کیا جاتا ہے۔

</div>
