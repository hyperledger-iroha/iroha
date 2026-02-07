---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/quickstart.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Início rápido de SoraFS

Este guia prático recorre ao perfil determinista do chunker SF-1,
a firma de manifestos e o fluxo de recuperação multiprovedor que sustenta o
pipeline de armazenamento de SoraFS. Complétala con el
[análise profunda do pipeline de manifestações](manifest-pipeline.md)
para notas de design e referência de sinalizadores da CLI.

## Requisitos anteriores

- Toolchain de Rust (`rustup update`), workspace clonado localmente.
- Opcional: [par de chaves Ed25519 geradas com OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  para firmar manifestos.
- Opcional: Node.js ≥ 18 se você planejar pré-visualizar o portal de Docusaurus.

Defina `export RUST_LOG=info` enquanto experimenta para mostrar mensagens úteis da CLI.

## 1. Atualizar os jogos deterministas

Regenera os vetores canônicos de chunking SF-1. O comando também emite
sobres de manifestos firmados quando fornecidos `--signing-key`; EUA
`--allow-unsigned` apenas durante o desenvolvimento local.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Saídas:

-`fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
-`fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (se for firmado)
-`fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Fragmentar uma carga útil e inspecionar o plano

Use `sorafs_chunker` para fragmentar um arquivo ou um arquivo compactado arbitrariamente:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Campos clave:

- `profile` / `break_mask` – confirma os parâmetros de `sorafs.sf1@1.0.0`.
- `chunks[]` – compensa ordenadas, longitudes e digere BLAKE3 dos pedaços.

Para luminárias maiores, execute a regressão respaldada por proptest para
garantir que o chunking em streaming e por lotes seja mantido sincronizado:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. Construir e firmar uma manifestação

Envolva o plano de pedaços, os pseudônimos e as firmas de governo em uma manifestação usando
`sorafs-manifest-stub`. O comando abaixo mostra uma carga útil de um único arquivo; pasa
uma rota de diretório para empaquetar uma árvore (a CLI recorre na ordem lexicográfica).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

Revisão `/tmp/docs.report.json` para:

- `chunking.chunk_digest_sha3_256` – resumo SHA3 de deslocamentos/longitudes, coincide com eles
  luminárias del chunker.
- `manifest.manifest_blake3` – resumo BLAKE3 firmado na parte superior do manifesto.
- `chunk_fetch_specs[]` – instruções de recuperação ordenadas para orquestradores.

Quando esta lista é para portar firmas reais, adicione os argumentos `--signing-key` e
`--signer`. O comando verifica cada firma Ed25519 antes de escrever o sobre.

## 4. Simula a recuperação multi-provedor

Use a CLI de busca de desenvolvimento para reproduzir o plano de pedaços contra um ou mais
provadores. É ideal para testes de fumaça de CI e protótipos de orquestrador.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Comprovações:- `payload_digest_hex` deve coincidir com o relatório do manifesto.
- `provider_reports[]` mostra conteúdo de sucesso/falha por provedor.
- Un `chunk_retry_total` distinto de cero destaca configurações de contrapressão.
- Pasa `--max-peers=<n>` para limitar o número de fornecedores programados para uma execução
  e manter as simulações de CI enfocadas nos candidatos principais.
- `--retry-budget=<n>` sobrescrever o relato por defeito de reintenções por pedaço (3) para
  detecte regressões mais rápidas do orquestrador ao injetor de falhas.

Añade `--expect-payload-digest=<hex>` e `--expect-payload-len=<bytes>` para cair rápido
quando a carga reconstruída for desviada da manifestação.

## 5. Próximos passos

- **Integração de governança** – canaliza o resumo do manifesto e
  `manifest_signatures.json` no fluxo do conselho para que o Pin Registry possa
  anunciar disponibilidade.
- **Negociação de registro** – consulta [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  antes de registrar novos perfis. A automação deve preferir manejadores canônicos
  (`namespace.name@semver`) sobre IDs numéricos.
- **Automatização de CI** – adiciona comandos anteriores aos pipelines de lançamento para que
  a documentação, luminárias e artefatos públicos manifestam deterministas junto com
  metadados firmados.