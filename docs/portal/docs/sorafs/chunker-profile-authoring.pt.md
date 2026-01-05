<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4795260e6c2c0341efa3f77e9571d485cde2994819bafefb2e8757588b29ed80
source_last_modified: "2025-11-08T20:18:57.416803+00:00"
translation_last_reviewed: 2025-12-29
---

---
id: chunker-profile-authoring
title: Guia de autoria de perfis de chunker da SoraFS
sidebar_label: Guia de autoria de chunker
description: Checklist para propor novos perfis e fixtures de chunker da SoraFS.
---

:::note Fonte canonica
Esta pagina espelha `docs/source/sorafs/chunker_profile_authoring.md`. Mantenha ambas as copias sincronizadas.
:::

# Guia de autoria de perfis de chunker da SoraFS

Este guia explica como propor e publicar novos perfis de chunker para a SoraFS.
Ele complementa o RFC de arquitetura (SF-1) e a referencia do registro (SF-2a)
com requisitos concretos de autoria, etapas de validacao e modelos de proposta.
Para um exemplo canonico, veja
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
e o log de dry-run associado em
`docs/source/sorafs/reports/sf1_determinism.md`.

## Visao geral

Cada perfil que entra no registro deve:

- anunciar parametros CDC deterministicos e configuracoes de multihash identicas entre
  arquiteturas;
- entregar fixtures reproduziveis (JSON Rust/Go/TS + corpora fuzz + testemunhas PoR) que
  os SDKs downstream possam verificar sem tooling sob medida;
- incluir metadados prontos para governanca (namespace, name, semver) junto com orientacao
  de rollout e janelas operacionais; e
- passar pela suite de diff determinista antes da revisao do conselho.

Siga a checklist abaixo para preparar uma proposta que atenda a essas regras.

## Resumo da carta do registro

Antes de redigir uma proposta, confirme que ela atende a carta do registro aplicada por
`sorafs_manifest::chunker_registry::ensure_charter_compliance()`:

- IDs de perfil sao inteiros positivos que aumentam de forma monotona sem lacunas.
- O handle canonico (`namespace.name@semver`) deve aparecer na lista de alias e
  **deve** ser a primeira entrada. Aliases alternativos (ex., `sorafs.sf1@1.0.0`) vem depois.
- Nenhum alias pode colidir com outro handle canonico ou aparecer mais de uma vez.
- Aliases devem ser nao vazios e aparados de espacos em branco.

Helpers de CLI:

```bash
# Listagem JSON de todos os descritores registrados (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Emitir metadados para um perfil default candidato (handle canonico + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

Esses comandos mantem as propostas alinhadas com a carta do registro e fornecem os
metadados canonicos necessarios nas discussoes de governanca.

## Metadados requeridos

| Campo | Descricao | Exemplo (`sorafs.sf1@1.0.0`) |
|-------|-----------|------------------------------|
| `namespace` | Agrupamento logico para perfis relacionados. | `sorafs` |
| `name` | Rotulo legivel para humanos. | `sf1` |
| `semver` | Cadeia de versao semantica para o conjunto de parametros. | `1.0.0` |
| `profile_id` | Identificador numerico monotono atribuido quando o perfil entra. Reserve o proximo id mas nao reutilize numeros existentes. | `1` |
| `profile_aliases` | Handles adicionais opcionais (nomes alternativos, abreviacoes) expostos a clientes durante a negociacao. Inclua sempre o handle canonico como primeira entrada. | `["sorafs.sf1@1.0.0"]` |
| `profile.min_size` | Comprimento minimo do chunk em bytes. | `65536` |
| `profile.target_size` | Comprimento alvo do chunk em bytes. | `262144` |
| `profile.max_size` | Comprimento maximo do chunk em bytes. | `524288` |
| `profile.break_mask` | Mascara adaptativa usada pelo rolling hash (hex). | `0x0000ffff` |
| `profile.polynomial` | Constante do polinomio gear (hex). | `0x3da3358b4dc173` |
| `gear_seed` | Seed usada para derivar a tabela gear de 64 KiB. | `sorafs-v1-gear` |
| `chunk_multihash.code` | Codigo multihash para digests por chunk. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | Digest do bundle canonico de fixtures. | `13fa...c482` |
| `fixtures_root` | Diretorio relativo contendo os fixtures regenerados. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Seed para amostragem PoR deterministica (`splitmix64`). | `0xfeedbeefcafebabe` (exemplo) |

Os metadados devem aparecer tanto no documento de proposta quanto dentro dos fixtures gerados
para que o registro, o tooling de CLI e a automacao de governanca confirmem os valores sem
cruzamentos manuais. Em caso de duvida, execute os CLIs de chunk-store e manifest com
`--json-out=-` para transmitir os metadados calculados para notas de revisao.

### Pontos de contato de CLI e registro

- `sorafs_manifest_chunk_store --profile=<handle>` - reexecutar metadados de chunk,
  digest do manifest e checks PoR com os parametros propostos.
- `sorafs_manifest_chunk_store --json-out=-` - transmitir o relatorio do chunk-store para
  stdout para comparacoes automatizadas.
- `sorafs_manifest_stub --chunker-profile=<handle>` - confirmar que manifests e planos CAR
  embutem o handle canonico mais aliases.
- `sorafs_manifest_stub --plan=-` - reenviar o `chunk_fetch_specs` anterior para
  verificar offsets/digests apos a mudanca.

Registre a saida dos comandos (digests, raizes PoR, hashes de manifest) na proposta para que
os revisores possam reproduzi-los literalmente.

## Checklist de determinismo e validacao

1. **Regenerar fixtures**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **Executar a suite de paridade** - `cargo test -p sorafs_chunker` e o harness diff
   cross-language (`crates/sorafs_chunker/tests/vectors.rs`) devem ficar verdes com os
   novos fixtures no lugar.
3. **Reexecutar corpora fuzz/back-pressure** - execute `cargo fuzz list` e o harness de
   streaming (`fuzz/sorafs_chunker`) contra os assets regenerados.
4. **Verificar testemunhas Proof-of-Retrievability** - execute
   `sorafs_manifest_chunk_store --por-sample=<n>` usando o perfil proposto e confirme
   que as raizes correspondem ao manifest de fixtures.
5. **Dry run de CI** - execute `ci/check_sorafs_fixtures.sh` localmente; o script
   deve ter sucesso com os novos fixtures e o `manifest_signatures.json` existente.
6. **Confirmacao cross-runtime** - assegure que os bindings Go/TS consumam o JSON
   regenerado e emitam limites e digests identicos.

Documente os comandos e os digests resultantes na proposta para que o Tooling WG possa
reexecuta-los sem adivinhacoes.

### Confirmacao de manifest / PoR

Depois de regenerar fixtures, execute o pipeline completo de manifest para garantir que
metadados CAR e provas PoR continuem consistentes:

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

Substitua o arquivo de entrada por qualquer corpus representativo usado nos seus fixtures
(ex., o stream deterministico de 1 GiB) e anexe os digests resultantes a proposta.

## Modelo de proposta

As propostas sao submetidas como registros Norito `ChunkerProfileProposalV1` registrados em
`docs/source/sorafs/proposals/`. O template JSON abaixo ilustra o formato esperado
(substitua seus valores conforme necessario):


Forneca um relatorio Markdown correspondente (`determinism_report`) que capture a
saida dos comandos, digests de chunk e quaisquer desvios encontrados durante a validacao.

## Fluxo de governanca

1. **Submeter PR com proposta + fixtures.** Inclua os assets gerados, a proposta
   Norito e atualizacoes em `chunker_registry_data.rs`.
2. **Revisao do Tooling WG.** Revisores reexecutam a checklist de validacao e confirmam
   que a proposta segue as regras do registro (sem reutilizacao de id, determinismo satisfeito).
3. **Envelope do conselho.** Uma vez aprovado, membros do conselho assinam o digest da
   proposta (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) e anexam suas
   assinaturas ao envelope do perfil armazenado junto aos fixtures.
4. **Publicacao do registro.** O merge atualiza o registro, docs e fixtures. O CLI
   default permanece no perfil anterior ate que a governanca declare a migracao pronta.
5. **Rastreamento de deprecacao.** Apos a janela de migracao, atualize o registro para

## Dicas de autoria

- Prefira limites pares de potencia de dois para minimizar comportamento de chunking em bordas.
- Evite mudar o codigo multihash sem coordenar consumidores de manifest e gateway; inclua uma
  nota operacional quando fizer isso.
- Mantenha as seeds da tabela gear legiveis para humanos, mas globalmente unicas para simplificar auditorias.
- Armazene artefatos de benchmarking (ex., comparacoes de throughput) em
  `docs/source/sorafs/reports/` para referencia futura.

Para expectativas operacionais durante o rollout, consulte o migration ledger
(`docs/source/sorafs/migration_ledger.md`). Para regras de conformidade em runtime, veja
`docs/source/sorafs/chunker_conformance.md`.
