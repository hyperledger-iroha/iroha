---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: conformidade com chunker
título: Guia de conformidade do chunker da SoraFS
sidebar_label: Conformidade do chunker
descrição: Requisitos e fluxos para preservar o perfil determinístico do chunker SF1 em fixtures e SDKs.
---

:::nota Fonte canônica
Esta página espelha `docs/source/sorafs/chunker_conformance.md`. Mantenha ambas as cópias sincronizadas.
:::

Este guia codifica os requisitos que toda implementação deve seguir para permanecer
compatível com o perfil determinístico de chunker da SoraFS (SF1). Ele também
documenta o fluxo de regeneração, a política de assinaturas e os passos de verificação para que
os consumidores de luminárias nos SDKs ficam sincronizados.

## Perfil canônico

Alça do perfil: `sorafs.sf1@1.0.0`
- Semente de entrada (hex): `0000000000dec0ded`
- Tamanho alvo: 262144 bytes (256 KiB)
- Tamanho mínimo: 65536 bytes (64 KiB)
- Tamanho máximo: 524288 bytes (512 KiB)
- Polinômio de rolamento: `0x3DA3358B4DC173`
- Semente da tabela engrenagem: `sorafs-v1-gear`
Máscara de quebra: `0x0000FFFF`

Implementação de referência: `sorafs_chunker::chunk_bytes_with_digests_profile`.
Qualquer aceleração SIMD deve produzir limites e digestões idênticos.

## Pacote de luminárias

`cargo run --locked -p sorafs_chunker --bin export_vectors` regenera como
fixtures e emite os seguintes arquivos em `fixtures/sorafs_chunker/`:

- `sf1_profile_v1.{json,rs,ts,go}` - limites canônicos de chunk para consumidores
  Rust, TypeScript e Go. Cada arquivo anuncia o identificador canônico como o primeiro
  entrada em `profile_aliases`, seguido de quaisquer aliases alternativos (ex.,
  `sorafs.sf1@1.0.0`, depois `sorafs.sf1@1.0.0`). A ordem e imposta por
  `ensure_charter_compliance` e NAO DEVE ser alterado.
- `manifest_blake3.json` - manifesto verificado por BLAKE3 cobrindo cada arquivo de fixtures.
- `manifest_signatures.json` - assinaturas do conselho (Ed25519) sobre o resumo do manifesto.
- `sf1_profile_v1_backpressure.json` e corpora brutos dentro de `fuzz/` -
  cenários determinísticos de streaming usados por testes de contrapressão do chunker.

### Política de assinaturas

A regeneração de luminárias **deve** incluir uma assinatura valida do conselho. O gerador
rejeita saida sem assinatura a menos que `--allow-unsigned` seja passado explicitamente (destinado
apenas para experimentação local). Os envelopes de assinatura são somente anexos e
são desduplicados por signatário.

Para adicionar uma assinatura do conselho:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## Verificação

O helper de CI `ci/check_sorafs_fixtures.sh` reexecuta o gerador com
`--locked`. Se luminárias divergirem ou assinaturas faltarem, o trabalho falha. Usar
este script em fluxos de trabalho noturnos e antes de enviar mudanças de fixtures.

Passos de verificação manual:

1. Execute `cargo test -p sorafs_chunker`.
2. Execute `ci/check_sorafs_fixtures.sh` localmente.
3. Confirme que `git status -- fixtures/sorafs_chunker` está limpo.

## Manual de atualização

Ao propor um novo perfil de chunker ou atualizar o SF1:

Veja também: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) para
requisitos de metadados, templates de proposta e checklists de validação.1. Redija um `ChunkProfileUpgradeProposalV1` (veja RFC SF-1) com novos parâmetros.
2. Regenere fixtures via `export_vectors` e registre o novo resumo do manifesto.
3. Assinar o manifesto com o quórum do conselho exigido. Todas as assinaturas devem ser
   anexadas a `manifest_signatures.json`.
4. Atualize os fixtures de SDK afetados (Rust/Go/TS) e garanta paridade cross-runtime.
5. Regenere corpora fuzz se os parâmetros mudarem.
6. Atualize este guia com o novo identificador de perfil, sementes e resumo.
7. Envie a mudanca junto com testes atualizados e atualizações do roadmap.

Mudanças que afetem limites de chunk ou digests sem seguir este processo
são invalidas e não devem ser mescladas.