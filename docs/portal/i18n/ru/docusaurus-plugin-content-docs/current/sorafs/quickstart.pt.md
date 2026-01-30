---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/quickstart.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Início rápido do SoraFS

Este guia prático percorre o perfil determinístico de chunker SF-1,
a assinatura de manifestos e o fluxo de busca multi-provedor que sustentam o
pipeline de armazenamento do SoraFS. Combine-o com o
[mergulho profundo no pipeline de manifestos](manifest-pipeline.md)
para notas de design e referência de flags da CLI.

## Pré-requisitos

- Toolchain do Rust (`rustup update`), workspace clonado localmente.
- Opcional: [par de chaves Ed25519 compatível com OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  para assinar manifestos.
- Opcional: Node.js ≥ 18 se você pretende pré-visualizar o portal Docusaurus.

Defina `export RUST_LOG=info` durante os testes para expor mensagens úteis da CLI.

## 1. Atualize os fixtures determinísticos

Gere novamente os vetores canônicos de chunking SF-1. O comando também emite
envelopes de manifesto assinados quando `--signing-key` é fornecido; use
`--allow-unsigned` apenas no desenvolvimento local.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Saídas:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (se assinado)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Fragmente um payload e inspecione o plano

Use `sorafs_chunker` para fragmentar um arquivo ou um arquivo compactado arbitrário:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Campos-chave:

- `profile` / `break_mask` – confirma os parâmetros de `sorafs.sf1@1.0.0`.
- `chunks[]` – offsets ordenados, comprimentos e digests BLAKE3 dos chunks.

Para fixtures maiores, execute a regressão com proptest para garantir que o
chunking em streaming e em lote permaneça sincronizado:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. Construa e assine um manifesto

Empacote o plano de chunks, os aliases e as assinaturas de governança em um manifesto
usando `sorafs-manifest-stub`. O comando abaixo mostra um payload de arquivo único; passe
um caminho de diretório para empacotar uma árvore (a CLI percorre em ordem lexicográfica).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

Revise `/tmp/docs.report.json` para:

- `chunking.chunk_digest_sha3_256` – digest SHA3 de offsets/comprimentos, corresponde aos
  fixtures do chunker.
- `manifest.manifest_blake3` – digest BLAKE3 assinado no envelope do manifesto.
- `chunk_fetch_specs[]` – instruções de busca ordenadas para orquestradores.

Quando estiver pronto para fornecer assinaturas reais, adicione os argumentos
`--signing-key` e `--signer`. O comando verifica cada assinatura Ed25519 antes de gravar
o envelope.

## 4. Simule a recuperação multi-provedor

Use a CLI de fetch de desenvolvimento para reproduzir o plano de chunks contra um ou
mais provedores. Isso é ideal para smoke tests de CI e prototipagem de orquestrador.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Verificações:

- `payload_digest_hex` deve corresponder ao relatório do manifesto.
- `provider_reports[]` mostra contagens de sucesso/falha por provedor.
- `chunk_retry_total` diferente de zero destaca ajustes de back-pressure.
- Passe `--max-peers=<n>` para limitar o número de provedores programados para uma execução
  e manter as simulações de CI focadas nos candidatos principais.
- `--retry-budget=<n>` substitui a contagem padrão de tentativas por chunk (3) para expor
  regressões do orquestrador mais rápido ao injetar falhas.

Adicione `--expect-payload-digest=<hex>` e `--expect-payload-len=<bytes>` para falhar
rapidamente quando o payload reconstruído divergir do manifesto.

## 5. Próximos passos

- **Integração de governança** – envie o digest do manifesto e `manifest_signatures.json`
  para o fluxo do conselho para que o Pin Registry possa anunciar disponibilidade.
- **Negociação de registro** – consulte [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  antes de registrar novos perfis. A automação deve preferir identificadores canônicos
  (`namespace.name@semver`) em vez de IDs numéricos.
- **Automação de CI** – adicione os comandos acima aos pipelines de release para que a
  documentação, fixtures e artefatos publiquem manifestos determinísticos junto com
  metadados assinados.
