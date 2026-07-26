---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: conformidade com chunker
título: Guia de conformidade do bloco SoraFS
sidebar_label: Conformidade do chunker
description: Requisitos e recursos para preservar o perfil determinista do chunker SF1 em fixtures e SDKs.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/chunker_conformance.md`. Mantenha ambas as versões sincronizadas até que os documentos herdados sejam retirados.
:::

Este guia codifica os requisitos que toda implementação deve seguir para mantê-lo
alineada com o perfil determinista de chunker de SoraFS (SF1). Também
documenta o fluxo de regeneração, a política de firmas e os passos de verificação para que
os consumidores de luminárias nos SDKs permanecem sincronizados.

## Perfil canônico

- Alça do perfil: `sorafs.sf1@1.0.0` (também conhecido como heredado `sorafs.sf1@1.0.0`)
- Semente de entrada (hex): `0000000000dec0ded`
- Tamanho objetivo: 262144 bytes (256 KiB)
- Tamanho mínimo: 65536 bytes (64 KiB)
- Tamanho máximo: 524288 bytes (512 KiB)
- Polinômio de rolamento: `0x3DA3358B4DC173`
- Semente da engrenagem da tabela: `sorafs-v1-gear`
Máscara de quebra: `0x0000FFFF`

Implementação de referência: `sorafs_chunker::chunk_bytes_with_digests_profile`.
Qualquer aceleração SIMD deve produzir limites e digestões idênticas.

## Pacote de luminárias

`cargo run --locked -p sorafs_chunker --bin export_vectors` regenera perda
fixtures e emite os seguintes arquivos abaixo de `fixtures/sorafs_chunker/`:

- `sf1_profile_v1.{json,rs,ts,go}` — limites de pedaços canônicos para consumidores
  Rust, TypeScript e Go. Cada arquivo anuncia o identificador canônico como a primeira
  entrada em `profile_aliases`, seguida por qualquer alias herdado (p. ej.,
  `sorafs.sf1@1.0.0`, depois `sorafs.sf1@1.0.0`). A ordem é imposta por
  `ensure_charter_compliance` e NÃO DEBE alterado.
- `manifest_blake3.json` — manifesto selecionado com BLAKE3 que contém cada arquivo de fixtures.
- `manifest_signatures.json` — firmas del consejo (Ed25519) sobre o resumo do manifesto.
- `sf1_profile_v1_backpressure.json` e corpo em bruto dentro de `fuzz/` —
  cenários deterministas de streaming usados por testes de contrapressão do chunker.

### Política de firmas

A regeneração de equipamentos **deve** incluir uma firma válida do consejo. O gerador
rechaza la salida sin firmar a menos que se passe explicitamente `--allow-unsigned` (pensado
solo para experimentação local). Os sobres de firma são apenas anexos e se
desduplicado por firmante.

Para adicionar uma firma de conselho:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## Verificação

El ajudante de CI `ci/check_sorafs_fixtures.sh` reejecuta o gerador com
`--locked`. Se os jogos divergirem ou faltarem firmas, o trabalho falhará. EUA
este script em fluxos de trabalho noturnos e antes de enviar mudanças de equipamentos.

Passos de verificação manual:

1. Ejecuta `cargo test -p sorafs_chunker`.
2. Invoque `ci/check_sorafs_fixtures.sh` localmente.
3. Confirme que `git status -- fixtures/sorafs_chunker` está limpo.

## Manual de atualização

Ao propor um novo perfil de chunker ou atualizações SF1:

Veja também: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) para
requisitos de metadados, padrões de proposta e listas de verificação de validação.1. Redija um `ChunkProfileUpgradeProposalV1` (ver RFC SF-1) com novos parâmetros.
2. Regenerar fixtures via `export_vectors` e registrar o novo resumo do manifesto.
3. Firme o manifesto com o quórum do conselho necessário. Todas as firmas devem
   anexar a `manifest_signatures.json`.
4. Atualize os equipamentos do SDK afetados (Rust/Go/TS) e garanta paridade entre tempo de execução.
5. Regenera os corpos fuzz e altera os parâmetros.
6. Atualize este guia com o novo identificador de perfil, sementes e resumo.
7. Envie a mudança junto com testes atualizados e atualizações do roteiro.

As mudanças que afetam os limites de pedaços ou os resumos sem seguir este processo
filhos inválidos e não deben fusionarse.