---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: SoraFS SF1 Determinismo Dry-Run
summary: Checklist e resumos esperados para validar o perfil chunker canonico `sorafs.sf1@1.0.0`.
---

# SoraFS Teste de determinismo SF1

Este relatorio captura o dry-run base para o perfil chunker canonico
`sorafs.sf1@1.0.0`. O Tooling WG deve reexecutar o checklist abaixo ao validar
atualizações de luminárias ou novos pipelines de consumo. Registrar o resultado de cada
comando na tabela para manter uma trilha auditável.

## Lista de verificação

| Passo | Comando | Resultado esperado | Notas |
|------|--------|------------------|-------|
| 1 | `cargo test -p sorafs_chunker` | Todos os testes passam; o teste de paridade `vectors` tem sucesso. | Confirma que fixtures canonicos compilam e exigem a implementação Rust. |
| 2 | `ci/check_sorafs_fixtures.sh` | O script sai com 0; reporte os resumos do manifesto abaixo. | Verifique se os equipamentos foram regenerados e se as assinaturas permaneceram anexadas. |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | A entrada `sorafs.sf1@1.0.0` corresponde ao descritor de registro (`profile_id=1`). | Garanta que os metadados do registro permaneçam sincronizados. |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | A regeneração ocorre sem `--allow-unsigned`; arquivos de manifesto e assinatura não mudam. | Fornece prova de determinismo para limites de chunk e manifestos. |
| 5 | `node scripts/check_sf1_vectors.mjs` | Reporta nenhuma diferença entre fixtures TypeScript e Rust JSON. | Auxiliar opcional; garantir a paridade entre os tempos de execução (script bloqueado pelo Tooling WG). |

## Digere os esperados

- Resumo de pedaços (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
-`manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
-`sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
-`sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
-`sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
-`sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## Registro de aprovação

| Dados | Engenheiro | Resultado do checklist | Notas |
|------|----------|------------------------|-------|
| 12/02/2026 | Ferramentas (LLM) | OK | Fixtures regeneradas via `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102...1f`, produzindo handle canonico + alias lists e um manifest digest novo `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55`. Verificado com `cargo test -p sorafs_chunker` e um `ci/check_sorafs_fixtures.sh` limpo (luminárias preparadas para a verificação). Passo 5 pendente até o ajudante de paridade Node chegar. |
| 20/02/2026 | Ferramentas de armazenamento CI | OK | Envelope do Parlamento (`fixtures/sorafs_chunker/manifest_signatures.json`) recebidos via `ci/check_sorafs_fixtures.sh`; o script regenerou fixtures, confirmou o manifest digest `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`, e reexecutou o Rust chicote (passos Go/Node executam quando disponíveis) sem diffs. |

O Tooling WG deve adicionar uma linha datada após executar o checklist. Se algum
passo falha, abra um problema relacionado aqui e incluindo detalhes de remediação antes
de aprovar novos dispositivos ou perfis.