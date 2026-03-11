---
lang: ur
direction: rtl
source: docs/source/README.md
status: complete
translator: manual
source_hash: 4a6e55a3232ff38c5c2f45b0a8a3d97471a14603bea75dc2034d7c9c4fb3f862
source_last_modified: "2025-11-10T19:43:50.185052+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

# Iroha VM اور Kotodama دستاویزی فہرست

یہ فہرست IVM، Kotodama اور IVM‑فرسٹ پائپ لائن سے متعلق بنیادی ڈیزائن اور
ریفرنس دستاویزات کے روابط فراہم کرتی ہے۔ جاپانی ورژن کے لیے
‎[`README.ja.md`](./README.ja.md)‎ دیکھیں۔

- IVM کی ساخت اور زبان کے ساتھ میپنگ: ‎`../../ivm.md`‎
- IVM syscalls کا ABI: ‎`ivm_syscalls.md`‎
- جنریٹڈ syscall کانسٹنٹس: ‎`ivm_syscalls_generated.md`‎ (اپ ڈیٹ کرنے کے لیے
  ‎`make docs-syscalls`‎ چلائیں)
- IVM بائٹ کوڈ ہیڈر: ‎`ivm_header.md`‎
- Kotodama کی گرامر اور سیمنٹکس: ‎`kotodama_grammar.md`‎
- Kotodama مثالیں اور syscalls کے ساتھ میپنگ: ‎`kotodama_examples.md`‎
- ٹرانزیکشن پائپ لائن (IVM‑first): ‎`../../new_pipeline.md`‎
- Torii کنٹریکٹس API (manifests): ‎`torii_contracts_api.md`‎
- JSON کوئری لفافہ (CLI / ٹولنگ): ‎`query_json.md`‎
- Norito streaming ماڈیول کی ریفرنس: ‎`norito_streaming.md`‎
- رن ٹائم ABI نمونے: ‎`samples/runtime_abi_active.md`‎،
  ‎`samples/runtime_abi_hash.md`‎، ‎`samples/find_active_abi_versions.md`‎
- ZK App API (اٹیچمنٹس، prover، ووٹ گنتی): ‎`zk_app_api.md`‎
- Torii میں ZK اٹیچمنٹس/prover کے لیے رن بک: ‎`zk/prover_runbook.md`‎
- Torii ZK App API آپریٹر گائیڈ (اٹیچمنٹس/prover؛ crate docs): ‎`../../crates/iroha_torii/docs/zk_app_api.md`‎
- Torii MCP API guide (agent/tool bridge; crate doc): `../../crates/iroha_torii/docs/mcp_api.md`
- VK/proofs کا لائف سائیکل (رجسٹری، ویریفکیشن، ٹیلی میٹری):
  ‎`zk/lifecycle.md`‎
- Torii آپریٹر ایڈز (visibility endpoints): ‎`references/operator_aids.md`‎
- Nexus ڈیفالٹ لین کوئک اسٹارٹ: ‎`quickstart/default_lane.md`‎
- MOCHI سپروائزر کوئک اسٹارٹ اور آرکیٹیکچر: ‎`mochi/index.md`‎
- JavaScript SDK گائیڈز (کوئک اسٹارٹ، کنفیگریشن، پبلشنگ):
  ‎`sdk/js/index.md`‎
- Swift SDK parity/CI ڈیش بورڈز: ‎`references/ios_metrics.md`‎
- گورننس: ‎`../../gov.md`‎
- کوآرڈینیشن/وضاحت کے پرامپٹس: ‎`coordination_llm_prompts.md`‎
- روڈمیپ: ‎`../../roadmap.md`‎
- Docker بلڈر امیج کا استعمال: ‎`docker_build.md`‎

استعمال سے متعلق نکات

- `examples/` فولڈر میں موجود مثالوں کو بیرونی ٹولز
  ‎(`koto_compile`, `ivm_run`)‎ کے ذریعے بلڈ اور رن کریں:
  - ‎`make examples-run`‎ (اور اگر ‎`ivm_tool`‎ دستیاب ہو تو ‎`make examples-inspect`‎)
- مثالوں اور ہیڈر چیکس کے لیے اختیاری انٹیگریشن ٹیسٹس (جو ڈیفالٹ کے طور پر
  ignore ہوتے ہیں) ‎`integration_tests/tests/`‎ میں موجود ہیں۔

پائپ لائن کنفیگریشن

- تمام رن ٹائم رویّہ ‎`iroha_config`‎ فائلوں کے ذریعے کنفیگر ہوتا ہے۔ آپریٹرز
  کے لیے اینوائرمنٹ ویریبلز استعمال نہیں کیے جاتے۔
- مناسب ڈیفالٹس فراہم کیے گئے ہیں؛ زیادہ تر ڈپلائمنٹس کو ان میں تبدیلی کی
  ضرورت نہیں پڑے گی۔
- `[pipeline]` سیکشن میں اہم keys:
  - ‎`dynamic_prepass`‎: IVM کی read‑only prepass کو فعال کرتا ہے تاکہ access
    sets نکالے جا سکیں (ڈیفالٹ: true)۔
  - ‎`access_set_cache_enabled`‎: `(code_hash, entrypoint)` کے حساب سے اخذ شدہ
    access sets کو cache کرتا ہے؛ hints کو ڈیبگ کرنے کے لیے اسے بند کریں
    (ڈیفالٹ: true)۔
  - ‎`parallel_overlay`‎: overlays کو متوازی طور پر بناتا ہے؛ commit اب بھی
    ڈیٹرمنسٹک رہتا ہے (ڈیفالٹ: true)۔
  - ‎`gpu_key_bucket`‎: شیڈیولر prepass کے لیے keys کی اختیاری bucket بندی،
    جو `(key, tx_idx, rw_flag)` پر stable radix استعمال کرتی ہے؛ CPU‑based
    ڈیٹرمنسٹک راستہ ہمیشہ فعال رہتا ہے (ڈیفالٹ: false)۔
  - ‎`cache_size`‎: IVM کے global pre‑decode cache کی capacity (ڈی کوڈ شدہ
    streams کے لیے). ڈیفالٹ: 128۔ زیادہ ویلیو ریپیٹڈ رنز کے لیے decode وقت
    کم کر سکتی ہے۔

ڈاکس سنک چیکس

- syscall کانسٹنٹس ‎(`docs/source/ivm_syscalls_generated.md`)‎
  - ری جنریشن: ‎`make docs-syscalls`‎
  - صرف چیک: ‎`bash scripts/check_syscalls_doc.sh`‎
- syscall ABI ٹیبل ‎(`crates/ivm/docs/syscalls.md`)‎
  - صرف چیک:
    ‎`cargo run -p ivm --bin gen_syscalls_doc -- --check --no-code`‎
  - جنریٹڈ سیکشن (اور کوڈ ڈاک ٹیبل) اپ ڈیٹ کرنے کے لیے:
    ‎`cargo run -p ivm --bin gen_syscalls_doc -- --write`‎
- pointer‑ABI ٹیبلز ‎(`crates/ivm/docs/pointer_abi.md`‎ اور ‎`ivm.md`‎)
  - صرف چیک:
    ‎`cargo run -p ivm --bin gen_pointer_types_doc -- --check`‎
  - سیکشنز اپ ڈیٹ کرنے کے لیے:
    ‎`cargo run -p ivm --bin gen_pointer_types_doc -- --write`‎
- IVM ہیڈر پالیسی اور ABI hashes ‎(`docs/source/ivm_header.md`)‎
  - صرف چیک:
    ‎`cargo run -p ivm --bin gen_header_doc -- --check`‎ اور
    ‎`cargo run -p ivm --bin gen_abi_hash_doc -- --check`‎
  - سیکشنز اپ ڈیٹ کرنے کے لیے:
    ‎`cargo run -p ivm --bin gen_header_doc -- --write`‎ اور
    ‎`cargo run -p ivm --bin gen_abi_hash_doc -- --write`‎

CI

- GitHub Actions ورک فلو ‎`.github/workflows/check-docs.yml`‎ ہر push/PR پر
  اِن چیکوں کو چلاتا ہے، اور اگر جنریٹڈ ڈاکس امپلی مینٹیشن کے ساتھ غیر
  ہم آہنگ ہو جائیں تو بلڈ فیل کر دیتا ہے۔
- ‎[گورننس پلی بک](governance_playbook.md)‎

</div>
