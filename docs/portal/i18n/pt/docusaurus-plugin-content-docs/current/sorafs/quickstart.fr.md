---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/quickstart.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Desmarque rapidamente SoraFS

Este guia prático passa na revisão do perfil do pedaço SF-1 determinado,
a assinatura dos manifestos e o fluxo de recuperação multi-fornecedores que
sob a direção do pipeline de armazenamento SoraFS. Complete o par
l'[analyse approfondie du pipeline de manifestes](manifest-pipeline.md)
para notas de concepção e referência de sinalizadores CLI.

## Pré-requisito

- Toolchain Rust (`rustup update`), localização clonada do espaço de trabalho.
- Opcional: [par de chaves Ed25519 gerado por OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  para assinar os manifestos.
- Opcional: Node.js ≥ 18 se você quiser pré-visualizar o portal Docusaurus.

Defina `export RUST_LOG=info` enquanto escreve para exibir mensagens úteis CLI.

## 1. Rafraîchir os jogos determinados

Regenere os vetores de decupagem SF-1 canônicos. O comando do produto também é
envelopes de manifesto assinados até `--signing-key` são fornecidos; utilizar
`--allow-unsigned` exclusivo em desenvolvimento local.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Sorteios:

-`fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
-`fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (com assinatura)
-`fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Desconecte uma carga útil e inspecione o plano

Use `sorafs_chunker` para recuperar um arquivo ou arquivo arbitrário:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Campeonatos Clés:

- `profile` / `break_mask` – confirma os parâmetros de `sorafs.sf1@1.0.0`.
- `chunks[]` – compensa ordonnés, longueurs et empreintes BLAKE3 des chunks.

Para luminárias mais volumosas, execute a regressão com base no proptest afin
certifique-se de que a decupagem em streaming e muito sincronizada:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. Construa e assine um manifesto

Envelope o plano de pedaços, o apelido e as assinaturas de governo em um
manifesto via `sorafs-manifest-stub`. O comando ci-dessous ilustra uma carga útil para
arquivo mais exclusivo; passez un chemin de répertoire para empaqueter un arbre (la CLI le
parcourt en ordem lexicográfica).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

Examinez `/tmp/docs.report.json` para:

- `chunking.chunk_digest_sha3_256` – empreinte SHA3 des offsets/longueurs, correspond aux
  luminárias du chunker.
- `manifest.manifest_blake3` – impressão BLAKE3 assinada no envelope do manifesto.
- `chunk_fetch_specs[]` – instruções de recuperação ordenadas para orquestradores.

Quando você estiver pronto para fornecer várias assinaturas, adicione os argumentos
`--signing-key` e `--signer`. La commande vérifie chaque assinatura Ed25519 avant
escreva o envelope.

## 4. Simular uma recuperação de vários fornecedores

Use a CLI de busca de desenvolvimento para melhorar o plano de pedaços contra um ou
plusieurs fournisseurs. É ideal para testes de fumaça CI e prototipagem
do orquestrador.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Verificações:- `payload_digest_hex` corresponde ao relatório do manifesto.
- `provider_reports[]` expõe as contas de sucesso/cheque pelo fornecedor.
- Um `chunk_retry_total` não nulo encontrou evidências de ajustes de contrapressão.
- Passe `--max-peers=<n>` para limitar o nome dos fornecedores planejados para um
  execução e gerenciamento de simulações CI centradas nos candidatos principais.
- `--retry-budget=<n>` substitui o nome das tentativas por pedaço por padrão (3) depois de
  mostrar e evidenciar mais as regressões do orquestrador durante a injeção
  cheques.

Adicione `--expect-payload-digest=<hex>` e `--expect-payload-len=<bytes>` para ecoar
rapidamente quando a carga útil é reconstruída no cartão do manifesto.

## 5. Etapas seguintes

- **Integration gouvernance** – acheminer l'empreinte du manifeste et
  `manifest_signatures.json` no fluxo do conselho para que o Pin Registry possa ser usado
  anuncie a disponibilidade.
- **Negociação de registro** – consulte [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  antes de registrar novos perfis. A automação deve privilegiar as alças
  canônicos (`namespace.name@semver`) geralmente são ID numéricos.
- **Automatização CI** – adiciona os comandos ci-dessus aux pipelines de release para isso
  a documentação, os equipamentos e os artefatos públicos dos manifestos determinados
  com metadonos assinados.