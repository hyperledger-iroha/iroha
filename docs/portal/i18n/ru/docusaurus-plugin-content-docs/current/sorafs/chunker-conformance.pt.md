---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: chunker-conformance
title: Guia de conformidade do chunker da SoraFS
sidebar_label: Conformidade de chunker
description: Requisitos e fluxos para preservar o perfil deterministico de chunker SF1 em fixtures e SDKs.
---

:::note Fonte canonica
Esta pagina espelha `docs/source/sorafs/chunker_conformance.md`. Mantenha ambas as copias sincronizadas.
:::

Este guia codifica os requisitos que toda implementacao deve seguir para permanecer
compativel com o perfil deterministico de chunker da SoraFS (SF1). Ele tambem
documenta o fluxo de regeneracao, a politica de assinaturas e os passos de verificacao para que
os consumidores de fixtures nos SDKs fiquem sincronizados.

## Perfil canonico

- Handle do perfil: `sorafs.sf1@1.0.0`
- Seed de entrada (hex): `0000000000dec0ded`
- Tamanho alvo: 262144 bytes (256 KiB)
- Tamanho minimo: 65536 bytes (64 KiB)
- Tamanho maximo: 524288 bytes (512 KiB)
- Polinomio de rolling: `0x3DA3358B4DC173`
- Seed da tabela gear: `sorafs-v1-gear`
- Break mask: `0x0000FFFF`

Implementacao de referencia: `sorafs_chunker::chunk_bytes_with_digests_profile`.
Qualquer aceleracao SIMD deve produzir limites e digests identicos.

## Bundle de fixtures

`cargo run --locked -p sorafs_chunker --bin export_vectors` regenera as
fixtures e emite os seguintes arquivos em `fixtures/sorafs_chunker/`:

- `sf1_profile_v1.{json,rs,ts,go}` - limites canonicos de chunk para consumidores
  Rust, TypeScript e Go. Cada arquivo anuncia o handle canonico como a primeira
  entrada em `profile_aliases`, seguido de quaisquer aliases alternativos (ex.,
  `sorafs.sf1@1.0.0`, depois `sorafs.sf1@1.0.0`). A ordem e imposta por
  `ensure_charter_compliance` e NAO DEVE ser alterada.
- `manifest_blake3.json` - manifest verificado por BLAKE3 cobrindo cada arquivo de fixtures.
- `manifest_signatures.json` - assinaturas do conselho (Ed25519) sobre o digest do manifest.
- `sf1_profile_v1_backpressure.json` e corpora brutos dentro de `fuzz/` -
  cenarios deterministicos de streaming usados por testes de back-pressure do chunker.

### Politica de assinaturas

A regeneracao de fixtures **deve** incluir uma assinatura valida do conselho. O gerador
rejeita saida sem assinatura a menos que `--allow-unsigned` seja passado explicitamente (destinado
apenas para experimentacao local). Os envelopes de assinatura sao append-only e
sao deduplicados por signatario.

Para adicionar uma assinatura do conselho:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## Verificacao

O helper de CI `ci/check_sorafs_fixtures.sh` reexecuta o gerador com
`--locked`. Se fixtures divergirem ou assinaturas faltarem, o job falha. Use
este script em workflows noturnos e antes de enviar mudancas de fixtures.

Passos de verificacao manual:

1. Execute `cargo test -p sorafs_chunker`.
2. Execute `ci/check_sorafs_fixtures.sh` localmente.
3. Confirme que `git status -- fixtures/sorafs_chunker` esta limpo.

## Playbook de upgrade

Ao propor um novo perfil de chunker ou atualizar o SF1:

Veja tambem: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) para
requisitos de metadados, templates de proposta e checklists de validacao.

1. Redija um `ChunkProfileUpgradeProposalV1` (veja RFC SF-1) com novos parametros.
2. Regenere fixtures via `export_vectors` e registre o novo digest do manifest.
3. Assine o manifest com o quorum do conselho exigido. Todas as assinaturas devem ser
   anexadas a `manifest_signatures.json`.
4. Atualize as fixtures de SDK afetadas (Rust/Go/TS) e garanta paridade cross-runtime.
5. Regenere corpora fuzz se os parametros mudarem.
6. Atualize este guia com o novo handle de perfil, seeds e digest.
7. Envie a mudanca junto com testes atualizados e atualizacoes do roadmap.

Mudancas que afetem limites de chunk ou digests sem seguir este processo
sao invalidas e nao devem ser mergeadas.
