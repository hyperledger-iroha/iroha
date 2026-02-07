---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: conformidade com chunker
título: Guia de conformidade do bloco SoraFS
sidebar_label: Bloco de conformidade
descrição: Exigências e fluxos de trabalho para preservar o bloco de perfil SF1 determinado nos equipamentos e SDKs.
---

:::nota Fonte canônica
:::

Este guia codifica as exigências que cada implementação deve seguir para restaurar
alinhado com o pedaço de perfil determinado de SoraFS (SF1). O documento também
o fluxo de trabalho de regeneração, a política de assinaturas e as etapas de verificação para que
os consumidores de fixtures nos SDKs ainda sincronizados.

## Perfil canônico

- Semente de entrada (hex): `0000000000dec0ded`
- Código máximo: 262144 bytes (256 KiB)
- Tamanho mínimo: 65536 bytes (64 KiB)
- Tamanho máximo: 524288 bytes (512 KiB)
- Polinômio de rolamento: `0x3DA3358B4DC173`
- Semente de engrenagem de mesa: `sorafs-v1-gear`
- Máscara de ruptura: `0x0000FFFF`

Implementação de referência: `sorafs_chunker::chunk_bytes_with_digests_profile`.
Toda aceleração SIMD produz limites e resumos idênticos.

## Pacote de luminárias

`cargo run --locked -p sorafs_chunker --bin export_vectors` regenera os
fixtures e inseriu os seguintes arquivos em `fixtures/sorafs_chunker/` :

- `sf1_profile_v1.{json,rs,ts,go}` — limites de pedaços canônicos para les
  consumidores Rust, TypeScript e Go. Cada arquivo anuncia o identificador canônico
  `sorafs.sf1@1.0.0`, depois `sorafs.sf1@1.0.0`). A ordem é imposta por
  `ensure_charter_compliance` e NE DOIT PAS foram modificados.
- `manifest_blake3.json` — manifesto verificado BLAKE3 cobrindo cada arquivo de fixtures.
- `manifest_signatures.json` — assinaturas do conselho (Ed25519) no resumo do manifesto.
- `sf1_profile_v1_backpressure.json` e corpo bruto em `fuzz/` —
  cenários de streaming determinados utilizados pelos testes de contrapressão do chunker.

### Política de assinatura

A regeneração dos equipamentos **doit** inclui uma assinatura válida do conselho. O gerente
rejeite a saída não assinada, exceto se `--allow-unsigned` for explicitamente explicitado (prévu
exclusivo para o local de experiência). Os envelopes de assinatura são apenas anexados e
são duplicados pelo signatário.

Para adicionar uma assinatura do conselho:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## Verificação

Le helper CI `ci/check_sorafs_fixtures.sh` recarrega o gerador com
`--locked`. Se as luminárias forem divergentes ou se as assinaturas forem mantidas, o trabalho será ecoado. Utilizar
este script nos fluxos de trabalho da noite e antes de iniciar as mudanças de equipamentos.

Etapas de verificação manuelle:

1. Execute `cargo test -p sorafs_chunker`.
2. Localização Lancez `ci/check_sorafs_fixtures.sh`.
3. Confirme se `git status -- fixtures/sorafs_chunker` está correto.

## Manual de mise à nivel

Lorsqu'on propõe um novo pedaço de perfil ou o que encontrou no dia SF1:

Veja também: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) para os
exigências de metadonées, modelos de proposição e listas de verificação de validação.1. Redija um `ChunkProfileUpgradeProposalV1` (veja RFC SF-1) com novos parâmetros.
2. Gerencie os fixtures via `export_vectors` e envie o novo resumo do manifesto.
3. Assine o manifesto com o quórum necessário. Todas as assinaturas devem existir
   apêndices à `manifest_signatures.json`.
4. Atualize os equipamentos SDK em questão (Rust/Go/TS) e garanta a paridade em tempo de execução cruzado.
5. Gerencie os corpos fuzz se as configurações forem alteradas.
6. Prepare hoje este guia com o novo identificador de perfil, as sementes e o resumo.
7. Faça a modificação com os testes e as mises do dia do roteiro.

As alterações que afetam os limites de pedaços ou os resumos sem seguir este processo
sont invalides et ne doivent pas être fusionnés.