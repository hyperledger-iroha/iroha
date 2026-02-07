---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: SoraFS SF1 Determinismo Dry-Run
resumo: Verifique e ожидаемые resumos para o perfil canônico do chunker `sorafs.sf1@1.0.0`.
---

# SoraFS Teste de determinismo SF1

Este é um teste de simulação para um chunker de perfil canônico
`sorafs.sf1@1.0.0`. Tooling WG oferece uma boa seleção de serviços
luminárias обновлений ou novos pipelines de consumo. Записывайте результат каждой
comandos na tabela, existem trilhas auditáveis.

## Verifique

| Shag | Comando | Resultado final | Nomeação |
|------|--------|------------------|-------|
| 1 | `cargo test -p sorafs_chunker` | Todos os testes são produzidos; Teste de paridade `vectors` testado. | Подтверждает, что канонические fixtures компилируются и совпадают с Rust реализацией. |
| 2 | `ci/check_sorafs_fixtures.sh` | Script de segurança 0; сообщает digere manifestos não. | Проверяет, что fixtures регенерируются чисто и подписи остаются прикреплены. |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | A descrição de `sorafs.sf1@1.0.0` contém o descritor de registro (`profile_id=1`). | Гарантирует, esse registro de metadados é sincronizado. |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | A regeneração é fornecida por `--allow-unsigned`; O manifesto e a assinatura não são mencionados. | Ele fornece uma definição para limites de blocos e manifestos. |
| 5 | `node scripts/check_sf1_vectors.mjs` | Verifique o diff de fixtures TypeScript e Rust JSON. | Ajudante opcional; обеспечьте паритет между runtime (script поддерживает Tooling WG). |

## Resumos de Ожидаемые

- Resumo de pedaços (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
-`manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
-`sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
-`sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
-`sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
-`sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## Registro de aprovação

| Dados | Engenheiro | Результат чеклиста | Nomeação |
|------|----------|--------------------|-------|
| 12/02/2026 | Ferramentas (LLM) | ✅ Aprovado | Fixtures регенерированы через `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f`, com каноническим handle + списком aliases e novo manifest digest `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55`. Verifique `cargo test -p sorafs_chunker` e o programa `ci/check_sorafs_fixtures.sh` (luminárias подготовлены для проверки). Passo 5 é usado para auxiliar node parity helper. |
| 20/02/2026 | Ferramentas de armazenamento CI | ✅ Aprovado | Envelope do Parlamento (`fixtures/sorafs_chunker/manifest_signatures.json`) получен через `ci/check_sorafs_fixtures.sh`; скрипт регенерировал fixtures, подтвердил manifest digest `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`, e повторно запустил Rust chicote (шаги Go/Node выполняются при наличии) não diferenças. |

O Tooling WG fornece um conjunto completo de ferramentas para a configuração correta. Если
какой-либо шаг падает, заведите issue со ссылкой здесь и добавьте детали
remediação para утверждения novas luminárias ou perfis.