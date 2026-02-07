---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: SoraFS SF1 Determinismo Dry-Run
resumo: Lista de verificação e resumos presentes para validar o bloco de perfil canônico `sorafs.sf1@1.0.0`.
---

# SoraFS Teste de determinismo SF1

Este rapport captura o teste de base para o perfil canônico
`sorafs.sf1@1.0.0`. Tooling WG doit relancer le checklist ci-dessous lors de la
validação de atualizações de fixtures ou novos pipelines de consumidores.
Transmita o resultado de cada comando no quadro para manter um
rastreamento auditável.

## Lista de verificação

| Etapa | Comando | Resultado atendido | Notas |
|------|--------|------------------|-------|
| 1 | `cargo test -p sorafs_chunker` | Todos os testes foram aprovados; teste de paridade `vectors` reusado. | Confirme que os fixtures canônicos foram compilados e correspondentes à implementação Rust. |
| 2 | `ci/check_sorafs_fixtures.sh` | O script classifica em 0; rapporte les digests de manifest ci-dessous. | Verifique se os equipamentos estão devidamente registrados e se as assinaturas permanecem anexadas. |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | A entrada para `sorafs.sf1@1.0.0` corresponde ao descritor do registro (`profile_id=1`). | Certifique-se de que os metadados do registro estejam sincronizados. |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | A regeneração foi recuperada sem `--allow-unsigned` ; os arquivos de manifesto e assinatura permanecem alterados. | Fournit une preuve de déterminisme pour les limites de chunk et les manifests. |
| 5 | `node scripts/check_sf1_vectors.mjs` | Nenhuma diferença entre os fixtures TypeScript e JSON Rust. | Opção auxiliar; garantir a paridade em tempo de execução cruzado (script mantido pelo Tooling WG). |

## Resumo do atendimento

- Resumo de pedaços (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
-`manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
-`sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
-`sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
-`sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
-`sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## Diário de assinatura

| Data | Engenheiro | Resultado da lista de verificação | Notas |
|------|----------|------------|-------|
| 12/02/2026 | Ferramentas (LLM) | ✅Réussi | Fixtures registrados via `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f`, produzindo a lista canônica + aliases e um resumo manifesto do `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55`. Verificado com `cargo test -p sorafs_chunker` e um `ci/check_sorafs_fixtures.sh` próprio (dispositivos escalonados para verificação). Etapa 5, atente até a chegada do ajudante de paridade Node. |
| 20/02/2026 | Ferramentas de armazenamento CI | ✅Réussi | Envelope do Parlamento (`fixtures/sorafs_chunker/manifest_signatures.json`) recuperado via `ci/check_sorafs_fixtures.sh`; o script foi criado para os fixtures, confirmou o resumo do manifesto `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21` e relançou o chicote Rust (as etapas Go/Node são executadas quando disponíveis) sem diferenças. |

Tooling WG deve adicionar uma linha de data após a execução da lista de verificação. Se um
étape échoue, abra um problema aqui e inclua os detalhes de remédiação
antes de aprovar novos equipamentos ou perfis.