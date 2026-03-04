---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: autoria de perfil chunker
título: Guia de autoria de perfis de chunker da SoraFS
sidebar_label: Guia de autoria de chunker
description: Checklist para propor novos perfis e luminárias de chunker da SoraFS.
---

:::nota Fonte canônica
Esta página espelha `docs/source/sorafs/chunker_profile_authoring.md`. Mantenha ambas as cópias sincronizadas.
:::

# Guia de autoria de perfis de chunker da SoraFS

Este guia explica como propor e publicar novos perfis de chunker para SoraFS.
Ele complementa a RFC de arquitetura (SF-1) e a referência de registro (SF-2a)
com requisitos concretos de autoria, etapas de validação e modelos de proposta.
Para um exemplo canônico, veja
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
e o log de simulação associado em
`docs/source/sorafs/reports/sf1_determinism.md`.

## Visão geral

Cada perfil que entra no registro deve:

- anunciar parâmetros CDC determinísticos e configurações de multihash idênticos entre
  arquiteturas;
- entregar fixtures reproduziveis (JSON Rust/Go/TS + corpora fuzz + testemunhas PoR) que
  os SDKs downstream podem verificar sem ferramentas sob medida;
- incluir metadados prontos para governança (namespace, name, semver) junto com orientação
  de rollout e operações operacionais; e
- passar pela suíte de diferença determinista antes da revisão do conselho.

Siga o checklist abaixo para preparar uma proposta que atenda a essas regras.

## Resumo da carta de registro

Antes de redigir uma proposta, confirme que ela atende a carta de registro aplicada por
`sorafs_manifest::chunker_registry::ensure_charter_compliance()`:

- IDs de perfil são inteiros positivos que aumentam de forma monótona sem lacunas.
- O identificador canônico (`namespace.name@semver`) deve aparecer na lista de alias e
  **deve** ser a primeira entrada. Aliases alternativos (ex., `sorafs.sf1@1.0.0`) vêm depois.
- Nenhum alias pode colidir com outro identificador canônico ou aparecer mais de uma vez.
- Aliases não devem ser vazios e aparados de espaços em branco.

Ajudantes de CLI:

```bash
# Listagem JSON de todos os descritores registrados (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Emitir metadados para um perfil default candidato (handle canonico + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

Esses comandos mantêm as propostas aprovadas com a carta do registro e fornecimento dos
metadados canônicos necessários nas discussões de governança.

## Metadados necessários| Campo | Descrição | Exemplo (`sorafs.sf1@1.0.0`) |
|-------|-----------|-----------------------------|
| `namespace` | Agrupamento lógico para perfis relacionados. | `sorafs` |
| `name` | Rótulo legítimo para humanos. | `sf1` |
| `semver` | Cadeia de versão semântica para o conjunto de parâmetros. | `1.0.0` |
| `profile_id` | Identificador numérico monótono atribuído quando o perfil entra. Reserve o próximo id mas não reutilize números existentes. | `1` |
| `profile_aliases` | Trata de outras peculiaridades (nomes alternativos, abreviações) expostas a clientes durante uma negociação. Inclui sempre o identificador canônico como primeira entrada. | `["sorafs.sf1@1.0.0"]` |
| `profile.min_size` | Comprimento mínimo do chunk em bytes. | `65536` |
| `profile.target_size` | Comprimento alvo do pedaço em bytes. | `262144` |
| `profile.max_size` | Comprimento máximo do chunk em bytes. | `524288` |
| `profile.break_mask` | Máscara adaptativa usada pelo rolamento hash (hex). | `0x0000ffff` |
| `profile.polynomial` | Engrenagem Constante do polinomio (hex). | `0x3da3358b4dc173` |
| `gear_seed` | Semente usada para derivar a tabela gear de 64 KiB. | `sorafs-v1-gear` |
| `chunk_multihash.code` | Código multihash para resumos por pedaço. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | Resumo do pacote canônico de fixtures. | `13fa...c482` |
| `fixtures_root` | Diretorio relativo contendo os fixtures regenerados. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Semente para amostragem PoR determinística (`splitmix64`). | `0xfeedbeefcafebabe` (exemplo) |

Os metadados deverão aparecer tanto no documento de proposta quanto dentro dos fixtures gerados
para que o registro, o tooling de CLI e a automação de governança confirmem os valores sem
cruzamentos manuais. Em caso de dúvida, execute os CLIs de chunk-store e manifest com
`--json-out=-` para transmitir os metadados calculados para notas de revisão.

### Pontos de contato de CLI e registro

- `sorafs_manifest_chunk_store --profile=<handle>` - reexecutar metadados de chunk,
  digest do manifest e checks PoR com os parâmetros propostos.
- `sorafs_manifest_chunk_store --json-out=-` - transmitir o relatorio do chunk-store para
  stdout para comparações automatizadas.
- `sorafs_manifest_stub --chunker-profile=<handle>` - confirmar que manifestos e planos CAR
  embutem o handle canonico mais aliases.
- `sorafs_manifest_stub --plan=-` - reenviar o `chunk_fetch_specs` anterior para
  verifique compensações/resumos após a mudança.

Registre a saida dos comandos (digests, raízes PoR, hashes de manifest) na proposta para que
os revisores podem reproduzi-los literalmente.

## Checklist de determinismo e validação1. **Regenerar equipamentos**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **Executar um conjunto de paridade** - `cargo test -p sorafs_chunker` e o diferencial de chicote
   cross-lingual (`crates/sorafs_chunker/tests/vectors.rs`) deve ficar verde com os
   novas luminárias no lugar.
3. **Reexecutar corpora fuzz/contrapressão** - execute `cargo fuzz list` e o chicote de
   streaming (`fuzz/sorafs_chunker`) contra os ativos regenerados.
4. **Verificar testemunhas de Prova de Recuperabilidade** - executar
   `sorafs_manifest_chunk_store --por-sample=<n>` usando o perfil proposto e confirmado
   que as raízes envolvem ao manifesto de fixtures.
5. **Teste de CI** - execute `ci/check_sorafs_fixtures.sh` localmente; ó roteiro
   deve ter sucesso com os novos equipamentos e o `manifest_signatures.json` existente.
6. **Confirmação cross-runtime** - certifique-se de que os bindings Go/TS consomem o JSON
   regenerado e emitam limites e digestões idênticas.

Documente os comandos e os resumos resultantes da proposta para que o Tooling WG possa
reexecuta-los sem adivinhações.

### Confirmação de manifesto / PoR

Depois de regenerar fixtures, execute o pipeline completo de manifesto para garantir que
metadados CAR e provas PoR continuam consistentes:

```bash
# Validar metadados de chunk + PoR com o novo perfil
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Gerar manifest + CAR e capturar chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Reexecutar usando o plano de fetch salvo (evita offsets obsoletos)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

Substitua o arquivo de entrada por qualquer corpus representativo usado em seus fixtures
(ex., o stream determinístico de 1 GiB) e anexo dos resumos resultantes da proposta.

## Modelo de proposta

As propostas são submetidas como registros Norito `ChunkerProfileProposalV1` registrados em
`docs/source/sorafs/proposals/`. O template JSON abaixo ilustra o formato esperado
(substitua seus valores conforme necessário):


Forneça um relatorio Markdown correspondente (`determinism_report`) que captura um
disse os comandos, resumos de chunk e quaisquer desvios encontrados durante a validação.

## Fluxo de governança

1. **Submeter PR com proposta + fixtures.** Inclui os ativos gerados, a proposta
   Norito e atualizações em `chunker_registry_data.rs`.
2. **Revisão do Tooling WG.** Os revisores reexecutaram um checklist de validação e confirmaram
   que a proposta segue as regras do registro (sem reutilização de id, determinismo satisfeito).
3. **Envelope do conselho.** Uma vez aprovado, os membros do conselho assinam o resumo da
   proposta (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) e seu exame
   assinaturas ao envelope do perfil armazenado junto aos fixtures.
4. **Publicação do registro.** O merge atualiza o registro, documentos e fixtures. O CLI
   default permanece no perfil anterior até que a governança declare uma migração pronta.
5. **Rastreamento de descontinuação.** Após a janela de migração, atualize o registro para

## Dicas de autoria

- Prefira limites pares de potência de dois para minimizar o comportamento de chunking em bordas.
- Evite alterar o código multihash sem coordenar consumidores de manifesto e gateway; incluindo uma
  nota operacional quando fizer isso.
- Manter as sementes da tabela gear legíveis para humanos, mas globalmente únicas para simplificar auditorias.
- Armazene artistas de benchmarking (ex., comparações de throughput) em
  `docs/source/sorafs/reports/` para referência futura.Para expectativas operacionais durante a implementação, consulte o registro de migração
(`docs/source/sorafs/migration_ledger.md`). Para regras de conformidade em tempo de execução, veja
`docs/source/sorafs/chunker_conformance.md`.