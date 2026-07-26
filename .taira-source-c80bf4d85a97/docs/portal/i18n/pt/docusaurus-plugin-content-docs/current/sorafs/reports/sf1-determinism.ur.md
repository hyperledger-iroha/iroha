---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: SoraFS SF1 Determinismo Dry-Run
resumo: perfil canônico `sorafs.sf1@1.0.0` chunker کو validar کرنے کے لئے lista de verificação e resumos esperados.
---

# SoraFS Teste de determinismo SF1

یہ رپورٹ perfil canônico `sorafs.sf1@1.0.0` chunker کے teste de linha de base کو
capturar کرتی ہے۔ Tooling WG کو atualizações de acessórios e pipelines de consumo کی
validação کے وقت نیچے والا checklist دوبارہ چلانا چاہیے۔ ہر کمانڈ کا نتیجہ
ٹیبل میں ریکارڈ کریں تاکہ trilha auditável برقرار رہے۔

## Lista de verificação

| Etapa | Comando | Resultado Esperado | Notas |
|------|--------|------------------|-------|
| 1 | `cargo test -p sorafs_chunker` | تمام testes پاس ہوں؛ Teste de paridade `vectors` کامیاب ہو۔ | تصدیق کرتا ہے کہ compilação de fixtures canônicas ہوتے ہیں اور Implementação de Rust سے match کرتے ہیں۔ |
| 2 | `ci/check_sorafs_fixtures.sh` | اسکرپٹ 0 کے ساتھ saída کرے؛ نیچے e والے resumos de manifesto رپورٹ کرے۔ | verificar کرتا ہے کہ fixtures صاف طور پر regenerar ہوں اور assinaturas anexadas رہیں۔ |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | `sorafs.sf1@1.0.0` کا descritor de registro de entrada (`profile_id=1`) سے correspondência کرے۔ | یقینی بناتا ہے کہ sincronização de metadados de registro رہے۔ |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | regeneração `--allow-unsigned` کے بغیر sucesso کرے؛ manifesto e assinatura فائلیں inalterado رہیں۔ | limites de pedaços اور manifestos کے لئے prova de determinismo فراہم کرتا ہے۔ |
| 5 | `node scripts/check_sf1_vectors.mjs` | Fixadores TypeScript e Rust JSON کے درمیان کوئی relatório diff نہ کرے۔ | ajudante opcional; tempos de execução کے درمیان paridade یقینی بنائیں (script Tooling WG mantém کرتا ہے)۔ |

## Resumos esperados

- Resumo de pedaços (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
-`manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
-`sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
-`sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
-`sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
-`sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## Registro de aprovação

| Data | Engenheiro | Resultado da lista de verificação | Notas |
|------|----------|-------|-------|
| 12/02/2026 | Ferramentas (LLM) | ✅ Aprovado | `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f` کے ذریعے fixtures regeneram ہوئیں، identificador canônico + listas de alias اور نیا resumo do manifesto `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55` بنا۔ `cargo test -p sorafs_chunker` اور صاف `ci/check_sorafs_fixtures.sh` executado para verificar کیا (verificação de acessórios کے لئے encenado تھیں). Step 5 تب تک pendente ہے جب تک Node parity helper نہ آ جائے۔ |
| 20/02/2026 | Ferramentas de armazenamento CI | ✅ Aprovado | Envelope do Parlamento (`fixtures/sorafs_chunker/manifest_signatures.json`) `ci/check_sorafs_fixtures.sh` کے ذریعے حاصل ہوا؛ اسکرپٹ نے fixtures regenerate کیے، manifest digest `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21` confirm کیا، اور Rust chicote دوبارہ چلایا (Go/Node steps دستیاب ہوں تو چلتے ہیں) بغیر diferenças کے۔ |

Tooling WG e lista de verificação عرض المزيد
کوئی قدم falhar ہو تو یہاں problema vinculado فائل کریں اور remediação تفصیلات شامل
کریں, پھر نئے luminárias یا perfis aprovados کریں۔