---
lang: ur
direction: rtl
source: docs/source/bridge_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5e28e5c38283ad6be40a0fc48e0312797f490542a143f4cefdd209aaf8099ac5
source_last_modified: "2026-07-11T20:38:35.470900+00:00"
translation_last_reviewed: 2026-07-12
---

<div dir="rtl">

<!--
SPDX-License-Identifier: Apache-2.0
-->

# برج کی حتمیت کے ثبوت

یہ دستاویز پہلے اجرا کے لیے برج کی حتمیت کا format متعین کرتی ہے۔ ثبوت Sumeragi v2
کا تیار اور مستقل کیا ہوا عین finality evidence لے جاتا ہے۔ proof envelope کا schema
version `1` ہے، جبکہ اس کے اندر consensus protocol version `2` ہے۔ Sumeragi v1
certificate کا کوئی projection، decoder یا fallback راستہ موجود نہیں۔

## ثبوت کا عین format

Norito یا Norito JSON میں encoded `BridgeFinalityProof` کے صرف تین fields ہیں:

```text
{ version, block_header, finality_artifact }
```

- `version` لازماً `1` ہو؛
- `block_header` مطلوبہ height کا canonical `BlockHeader` ہے؛
- `finality_artifact` اس block کے لیے محفوظ عین `V2FinalityArtifact` ہے۔ یہ اپنے
  height-context roster کی ترتیب میں ہر validator کا BLS-normal PoP
  (`validator_set_pops`) مستقل طور پر اپنے اندر رکھتا ہے۔

artifact میں مکمل اور immutable `HeightContext`، عین `BlockSubject`، block hash،
CommitQC اور roster-aligned PoP شامل ہیں۔ height context chain، epoch، roster،
`DualQuorum`، DA layout، leader seed اور دیگر consensus data کو freeze کرتا ہے۔ epoch
ختم کرنے والے parent block کے context میں optional `next_epoch_snapshot` بھی ہوتا ہے؛
چونکہ یہ field context id کا حصہ ہے، parent CommitQC اسے child roster کی اجازت سے پہلے
authenticate کرتا ہے۔ Finalized snapshot اگلے epoch parameters کے ساتھ
`epoch_end_height` اور اگلے roster کے aligned `validator_set_pops` بھی authenticate کرتا ہے۔

## مستقل ذخیرہ اور verification

Sumeragi v2 apply path artifact کو verify کر کے immutable Kura sidecar کی صورت محفوظ
کرتا ہے۔ proof builder canonical block اور اس کا sidecar پڑھتا ہے اور موجودہ mutable
world state سے تاریخی PoP یا certificates دوبارہ نہیں بناتا۔ گم، خراب، متصادم یا ناقابل
verification sidecar fail closed ہوتا ہے؛ دستیابی حالیہ in-memory history window تک محدود
نہیں۔

stateless verifier version، chain، height، header hash، header کے canonical predecessor اور
view، context، subject اور CommitQC کو عین match کرتا اور artifact کے تمام PoP verify کرتا ہے۔
signer indices سختی سے بڑھتے ہوئے
اور range میں ہوں؛ CommitQC validator count اور voting power دونوں quorum پورے کرے، اور
عین Sumeragi v2 vote preimage پر BLS aggregate signature درست ہو۔

## Trust anchor اور successor verification

ایک الگ proof صرف اپنے ساتھ موجود roster کے تحت internal consistency ثابت کرتا ہے۔
`BridgeFinalityVerifier` پہلا proof قبول کرنے سے پہلے واضح طور پر trusted
`HeightContextId` مانگتا ہے۔ اس کے بعد وہ صرف فوراً اگلی height قبول کرتا اور child
context کا parent CommitQC پچھلے frozen roster اور PoP سے verify کرتا ہے۔ epoch کے اندر
child artifact پچھلے artifact کے PoP copy کرتا ہے؛ boundary پر epoch، roster، quorum، seed
اور PoP پچھلے parent context کے `next_epoch_snapshot` سے match ہوں، جس میں authenticated
`epoch_end_height` بھی شامل ہے اور جسے parent CommitQC authenticate کرتا ہے۔ پرانی، چھوڑی ہوئی یا unlinked heights مسترد ہوتی ہیں۔

SCCP یہی `BridgeFinalityProof` استعمال کرتا ہے۔ message کے دیے roster کے تحت signature
پر اکیلے بھروسا کافی نہیں؛ governed checkpoint context/artifact سے message artifact تک
ہر فوری successor verify کرنا لازم ہے۔

## Bundle اور API

`BridgeFinalityBundle` عین `{ commitment, finality_proof }` ہے۔ commitment یہ ہے:
`{ chain_id, height_context_id, block_height, block_hash }`۔

- `GET /v1/bridge/finality/{height}`، `BridgeFinalityProof` واپس کرتا ہے؛
- `GET /v1/bridge/finality/bundle/{height}`، `BridgeFinalityBundle` واپس کرتا ہے۔

block یا عین مستقل v2 artifact غائب یا invalid ہو تو دونوں endpoints fail closed ہوتے ہیں۔
نامعلوم fields، unsupported versions اور retired proof shapes لازماً مسترد کیے جائیں۔

</div>
