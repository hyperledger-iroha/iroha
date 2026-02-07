---
lang: pt
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 79a048e6061f7054e14a471004cf7da0dddd3f9bf627d9f1d20ff63803cb0979
source_last_modified: "2026-01-04T17:06:14.405886+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS Início rápido

Este guia prático percorre o perfil determinístico do chunker SF-1,
assinatura de manifesto e fluxo de busca de vários provedores que sustentam o SoraFS
pipeline de armazenamento. Combine-o com o [aprofundamento do pipeline de manifesto](manifest-pipeline.md)
para notas de design e material de referência de sinalizador CLI.

## Pré-requisitos

- Conjunto de ferramentas Rust (`rustup update`), espaço de trabalho clonado localmente.
- Opcional: [par de chaves Ed25519 gerado por OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  para assinar manifestos.
- Opcional: Node.js ≥ 18 se você planeja visualizar o portal Docusaurus.

Defina `export RUST_LOG=info` enquanto faz experiências para exibir mensagens CLI úteis.

## 1. Atualize os equipamentos determinísticos

Regenere os vetores canônicos de fragmentação SF-1. O comando também emite sinais assinados
envelopes de manifesto quando `--signing-key` for fornecido; usar `--allow-unsigned`
apenas durante o desenvolvimento local.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Saídas:

-`fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
-`fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (se assinado)
-`fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Divida uma carga útil e inspecione o plano

Use `sorafs_chunker` para agrupar um arquivo ou arquivo arbitrário:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Campos principais:

- `profile` / `break_mask` – confirma os parâmetros `sorafs.sf1@1.0.0`.
- `chunks[]` – deslocamentos ordenados, comprimentos e resumos de pedaços BLAKE3.

Para equipamentos maiores, execute a regressão apoiada por proptest para garantir streaming e
a fragmentação em lote permanece sincronizada:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. Crie e assine um manifesto

Agrupe o plano de bloco, os aliases e as assinaturas de governança em um manifesto usando
`sorafs-manifest-stub`. O comando abaixo mostra uma carga útil de arquivo único; passar
um caminho de diretório para empacotar uma árvore (a CLI percorre lexicograficamente).

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

- `chunking.chunk_digest_sha3_256` – resumo SHA3 de deslocamentos/comprimentos, corresponde ao
  luminárias chunker.
- `manifest.manifest_blake3` – resumo BLAKE3 assinado no envelope do manifesto.
- `chunk_fetch_specs[]` – instruções de busca ordenadas para orquestradores.

Quando estiver pronto para fornecer assinaturas reais, adicione `--signing-key` e `--signer`
argumentos. O comando verifica cada assinatura Ed25519 antes de escrever o
envelope.

## 4. Simule a recuperação de vários provedores

Use a CLI de busca do desenvolvedor para reproduzir o plano de bloco em um ou mais
fornecedores. Isso é ideal para testes de fumaça de CI e prototipagem de orquestrador.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Afirmações:

- `payload_digest_hex` deve corresponder ao relatório do manifesto.
- `provider_reports[]` apresenta contagens de sucesso/falha por provedor.
- `chunk_retry_total` diferente de zero destaca ajustes de contrapressão.
- Passe `--max-peers=<n>` para limitar o número de provedores agendados para uma execução
  e manter as simulações de CI focadas nos principais candidatos.
- `--retry-budget=<n>` substitui a contagem padrão de novas tentativas por bloco (3) para que você
  pode revelar regressões do orquestrador mais rapidamente ao injetar falhas.

Adicione `--expect-payload-digest=<hex>` e `--expect-payload-len=<bytes>` para falhar
rápido quando a carga reconstruída se desvia do manifesto.

## 5. Próximas etapas- **Integração de governança** – canalize o resumo do manifesto e
  `manifest_signatures.json` no fluxo de trabalho do conselho para que o Pin Registry possa
  anuncie a disponibilidade.
- **Negociação de cadastro** – consultar [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  antes de registrar novos perfis. A automação deve preferir identificadores canônicos
  (`namespace.name@semver`) sobre IDs numéricos.
- **Automação de CI** – adicione os comandos acima para liberar pipelines, então documentos,
  fixtures e artefatos publicam manifestos determinísticos junto com assinados
  metadados.