---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.es.md
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

Este relatório captura o dry-run base para o perfil chunker canonico
`sorafs.sf1@1.0.0`. Tooling WG deve reexecutar a lista de verificação abaixo quando
atualizações válidas de luminárias ou novos pipelines de consumidores. Registre-se
resultado de cada comando na tabela para manter uma trilha auditável.

## Lista de verificação

| Passo | Comando | Resultado esperado | Notas |
|------|--------|------------------|-------|
| 1 | `cargo test -p sorafs_chunker` | Todos os testes realizados; o teste de paridade `vectors` foi concluído. | Confirme que os fixtures canônicos foram compilados e coincidem com a implementação Rust. |
| 2 | `ci/check_sorafs_fixtures.sh` | A venda do script com 0; relata os resumos do manifesto abaixo. | Verifique se os equipamentos se regeneram limpiamente e se as firmas permanecem adjuntas. |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | A entrada de `sorafs.sf1@1.0.0` coincide com o descritor de registro (`profile_id=1`). | Certifique-se de que os metadados do registro sejam sincronizados. |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | A regeneração foi concluída sem `--allow-unsigned`; os arquivos de manifesto e firma no cambian. | Experimente um teste de determinismo para limites de pedaços e manifestos. |
| 5 | `node scripts/check_sf1_vectors.mjs` | Reporta que não há diferença entre fixtures TypeScript e Rust JSON. | Auxiliar opcional; garantir paridade entre tempos de execução (script mantido pelo Tooling WG). |

## Digere os esperados

- Resumo de pedaços (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
-`manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
-`sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
-`sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
-`sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
-`sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## Registro de aprovação

| Data | Engenheiro | Resultado da lista de verificação | Notas |
|------|----------|----------------------------|-------|
| 12/02/2026 | Ferramentas (LLM) | OK | Fixtures regenerados via `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102...1f`, produzindo handle canonico + alias lists e um manifest digest fresco `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55`. Verificado com `cargo test -p sorafs_chunker` e uma corrida limpa de `ci/check_sorafs_fixtures.sh` (acessórios preparados para a verificação). Passo 5 pendente até que o ajudante de paridade Node seja ligado. |
| 20/02/2026 | Ferramentas de armazenamento CI | OK | Envelope do Parlamento (`fixtures/sorafs_chunker/manifest_signatures.json`) obtido via `ci/check_sorafs_fixtures.sh`; o script regenera fixtures, confirma o resumo do manifesto `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21` e executa novamente o chicote Rust (passos Go/Node são executados quando estão disponíveis) sem diferenças. |

O Tooling WG deve anexar uma fila fechada após executar a lista de verificação. Si
algun paso falla, abre um problema enlazado aqui e inclui detalhes de remediação
antes de aprovar novas luminárias ou perfis.