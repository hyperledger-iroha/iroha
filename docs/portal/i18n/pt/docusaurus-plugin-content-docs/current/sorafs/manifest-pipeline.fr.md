---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Chunking SoraFS → Pipeline de manifesto

Este complemento ao início rápido retrace o pipeline de luta em luta que transforma os octetos brutos
no manifesto Norito adaptado ao Pin Registry de SoraFS. O conteúdo foi adaptado de
[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
consulte este documento para a especificação canônica e o changelog.

## 1. Pedaço de maneira determinada

SoraFS utiliza o perfil SF-1 (`sorafs.sf1@1.0.0`): um hash rodante inspirado em FastCDC com
um tamanho mínimo de 64 KiB, um limite de 256 KiB, um máximo de 512 KiB e uma máscara
de ruptura `0x0000ffff`. O perfil está registrado em `sorafs_manifest::chunker_registry`.

### Ajudantes Ferrugem

- `sorafs_car::CarBuildPlan::single_file` – Estabelece deslocamentos, comprimentos e impressões BLAKE3
  des chunks pendente la preparação des métadonnées CAR.
- `sorafs_car::ChunkStore` – Transmita cargas úteis, persista metadados de pedaços e deriva
  a árvore de echantillonnage Prova de Recuperabilidade (PoR) de 64 KiB / 4 KiB.
- `sorafs_chunker::chunk_bytes_with_digests` – Auxiliar de biblioteca atrás dos dois CLIs.

### CLI de utilitários

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

O JSON contém deslocamentos ordenados, longos e impressões de pedaços. Conserve-o
planeje a construção dos manifestos ou as especificações de busca do orquestrador.

### Témoins PoR

`ChunkStore` expõe `--por-proof=<chunk>:<segment>:<leaf>` e `--por-sample=<count>` para que eles
auditores podem exigir conjuntos de números determinados. Associar estas bandeiras a
`--por-proof-out` ou `--por-sample-out` para registrar o JSON.

## 2. Envelope e manifesto

`ManifestBuilder` combine os metadados de pedaços com peças de governo:

- CID racine (dag-cbor) e engajamentos CAR.
- Testes de pseudônimos e declarações de capacidade dos fornecedores.
- Assinaturas do conselho e opções de métodos (por exemplo, IDs de construção).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

Sorteios importantes:

- `payload.manifest` – Octetos de manifesto codificados em Norito.
- `payload.report.json` – Currículo lisível para humanos/automatização, incluindo
  `chunk_fetch_specs`, `payload_digest_hex`, empreintes CAR e metadonnées d'alias.
- `payload.manifest_signatures.json` – Enveloppe contendo a marca BLAKE3 do manifesto,
  A marca SHA3 do plano de pedaços e assinaturas Ed25519 triées.

Use `--manifest-signatures-in` para verificar os envelopes fornecidos pelos signatários
externos antes da gravação, e `--chunker-profile-id` ou `--chunker-profile=<handle>` para
alterar a seleção do registro.

## 3. Publicador e Pinador1. **Soumission à gouvernance** – Forneça a impressão do manifesto e o envelope de
   assinaturas au conseil pour que le pin puisse être admis. Os auditores externos devem
   salve a impressão SHA3 do plano de pedaços com a impressão do manifesto.
2. **Fixar cargas úteis** – Carregar o arquivo CAR (e o índice CAR opcional) referenciado em
   o manifesto do Pin Registry. Certifique-se de que o manifesto e o CAR compartilhem
   meme CID racine.
3. **Registre a transmissão** – Mantenha o relacionamento JSON, os termos PoR e todas as métricas
   de fetch nos artefatos de lançamento. Estes registros alimentam os painéis
   operadores e ajuda a reproduzir incidentes sem carregar cargas volumosas.

## 4. Simulação de busca de vários fornecedores

`cargo run -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=provedores/alpha.bin --provider=beta=provedores/beta.bin#4@3 \
  --output = carga útil.bin --json-out = fetch_report.json`

- `#<concurrency>` aumenta o paralelismo pelo fornecedor (`#4` ci-dessus).
- `@<weight>` ajuste da orientação de ordenação; o valor por padrão é 1.
- `--max-peers=<n>` limita o número de fornecedores planejados para uma execução durante o
  descobriu o reenvio mais os candidatos que desejam.
- `--expect-payload-digest` e `--expect-payload-len` protegem contra a corrupção silenciosa.
- `--provider-advert=name=advert.to` verifique as capacidades do fornecedor antes de usá-lo
  na simulação.
- `--retry-budget=<n>` substitua o nome das tentativas por pedaço (por padrão: 3) depois que la
  CI revela mais rapidamente as regressões nos cenários do cenário.

`fetch_report.json` expõe as métricas aprovadas (`chunk_retry_total`, `provider_failure_rate`,
etc.) adaptados às afirmações CI e à observabilidade.

## 5. Mises à jour du registre et gouvernance

Pela proposta de novos perfis de chunker:

1. Digite o descritor em `sorafs_manifest::chunker_registry_data`.
2. Leia hoje `docs/source/sorafs/chunker_registry.md` e os gráficos associados.
3. Gerencie os equipamentos (`export_vectors`) e capture os manifestos assinados.
4. Faça o relatório de conformidade da carta com as assinaturas de governo.

A automação deve privilegiar os cabos canônicos (`namespace.name@semver`) e não
reenviar IDs numéricos que forem necessários quando for necessário registrá-los.