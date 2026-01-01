---
lang: ur
direction: rtl
source: docs/portal/docs/intro.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9f775ae297c910da91c6ce97e97ee36fb87f60218fcfb97639ace6eba39f2252
source_last_modified: "2025-11-21T17:05:19.972106+00:00"
translation_last_reviewed: 2025-12-30
---

# SORA Nexus Developer Portal میں خوش آمدید

SORA Nexus developer portal Nexus operators اور Hyperledger Iroha contributors کے لئے interactive documentation، SDK tutorials، اور API references کو یکجا کرتا ہے۔ یہ main docs site کو اس repository سے براہ راست generated specs اور hands-on guides سامنے لا کر مکمل کرتا ہے۔ landing page اب themed Norito/SoraFS entry points، signed OpenAPI snapshots، اور ایک dedicated Norito Streaming reference فراہم کرتا ہے تاکہ contributors root spec کھنگالے بغیر streaming control-plane contract تک پہنچ سکیں۔

## آپ یہاں کیا کر سکتے ہیں

- **Norito سیکھیں** - overview اور quickstart سے آغاز کریں تاکہ serialization model اور bytecode tooling سمجھ سکیں۔
- **SDKs bootstrap کریں** - JavaScript اور Rust کے quickstarts آج فالو کریں؛ Python، Swift، اور Android guides recipes migrate ہونے کے ساتھ شامل ہوں گے۔
- **API references دیکھیں** - Torii OpenAPI page تازہ ترین REST specification render کرتا ہے، اور configuration tables canonical Markdown sources کی طرف link کرتے ہیں۔
- **Deployments تیار کریں** - operational runbooks (telemetry, settlement, Nexus overlays) `docs/source/` سے port ہو رہے ہیں اور migration کے ساتھ اس سائٹ پر آئیں گے۔

## موجودہ حالت

- ✅ themed Docusaurus v3 landing جس میں refreshed typography، gradient-driven hero/cards، اور resource tiles شامل ہیں جو Norito Streaming summary رکھتے ہیں۔
- ✅ Torii OpenAPI plugin کو `npm run sync-openapi` سے wired کیا گیا ہے، signed snapshot checks اور CSP guards `buildSecurityHeaders` کے ذریعے نافذ ہیں۔
- ✅ Preview اور probe coverage CI (`docs-portal-preview.yml` + `scripts/portal-probe.mjs`) میں چلتی ہے، اور اب streaming doc، SoraFS quickstarts، اور reference checklists کو artifacts publish ہونے سے پہلے gate کرتی ہے۔
- ✅ Norito، SoraFS، اور SDK quickstarts کے ساتھ reference sections sidebar میں live ہیں؛ `docs/source/` سے نئی imports (streaming, orchestration, runbooks) جب لکھی جاتی ہیں تو یہاں شامل ہوتی ہیں۔

## شمولیت کیسے کریں

- لوکل development commands کے لئے `docs/portal/README.md` دیکھیں (`npm install`, `npm run start`, `npm run build`)۔
- Content migration tasks `DOCS-*` roadmap items کے ساتھ track کی جاتی ہیں۔ Contributions خوش آمدید ہیں - `docs/source/` سے sections port کریں اور page کو sidebar میں شامل کریں۔
- اگر آپ کوئی generated artifact (specs, config tables) شامل کریں تو build command دستاویز کریں تاکہ آئندہ contributors اسے آسانی سے refresh کر سکیں۔
