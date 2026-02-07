---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Chunking de SoraFS → Pipeline de manifestos

Este complemento do quickstart acompanha o pipeline de ponta a ponta que transforma bytes
brutos em manifestos Norito adequados ao Pin Registry do SoraFS. O conteúdo foi adaptado de
[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
consulte esse documento para a previsão canônica e o changelog.

## 1. Fazer chunking de forma determinística

SoraFS usa o perfil SF-1 (`sorafs.sf1@1.0.0`): um hash rolante inspirado no FastCDC com
tamanho mínimo de pedaço de 64 KiB, alvo de 256 KiB, máximo de 512 KiB e máscara de quebra
`0x0000ffff`. O perfil está registrado em `sorafs_manifest::chunker_registry`.

### Ajudantes em Rust

- `sorafs_car::CarBuildPlan::single_file` – Emite compensações de pedaços, comprimentos e resumos
  BLAKE3 enquanto prepara os metadados do CAR.
- `sorafs_car::ChunkStore` – Faz streaming de payloads, persiste metadados de chunks e derivados
  a árvore de amostragem Prova de Recuperabilidade (PoR) de 64 KiB / 4 KiB.
- `sorafs_chunker::chunk_bytes_with_digests` – Auxiliar de biblioteca por trás das duas CLIs.

### Ferramentas de CLI

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

O JSON contém deslocamentos ordenados, comprimentos e resumos dos pedaços. Preservar o plano ao
construir manifestos ou especificações de busca do orquestrador.

### Testemunhas PoR

O `ChunkStore` expõe `--por-proof=<chunk>:<segment>:<leaf>` e `--por-sample=<count>` para que
os auditores podem solicitar conjuntos de testemunhas determinísticos. Combine esses flags com
`--por-proof-out` ou `--por-sample-out` para registrador ou JSON.

## 2. Empacotar um manifesto

`ManifestBuilder` combina metadados de chunks com anexos de governança:

- CID raiz (dag-cbor) e compromissos do CAR.
- Provas de alias e especificações de capacidade do provedor.
- Assinaturas do conselho e metadados detalhados (por exemplo, IDs de build).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

Saídas importantes:

- `payload.manifest` – Bytes do manifesto codificados em Norito.
- `payload.report.json` – Resumo legível para humanos/automação, incluindo `chunk_fetch_specs`,
  `payload_digest_hex`, resumos de CAR e metadados de alias.
- `payload.manifest_signatures.json` – Envelope contendo o resumo BLAKE3 do manifesto, o
  digest SHA3 do plano de pedaços e assinaturas Ed25519 ordenadas.

Use `--manifest-signatures-in` para verificar envelopes fornecidos por signatários externos
antes de gravá-los novamente e `--chunker-profile-id` ou `--chunker-profile=<handle>` para
fixar a seleção do registro.

## 3. Publicar e fixar

1. **Envio à governança** – Forneça o resumo do manifesto e o envelope de assinaturas ao
   conselho para que o pino possa ser admitido. Auditores externos devem salvar o resumo SHA3
   do plano de chunks junto ao digest do manifesto.
2. **Pinear payloads** – Faça upload do arquivo CAR (e do índice CAR opcional) referenciado no
   manifesto para o Pin Registry. Garanta que o manifesto e o CAR compartilhem o mesmo CID raiz.
3. **Registrar telemetria** – Preservar o relatório JSON, as testemunhas PoR e quaisquer
   métricas de busca nos artefatos de lançamento. Esses registros alimentam dashboards de
   operadores e ajudam a reproduzir problemas sem baixar cargas grandes.## 4. Simulação de busca de vários provedores

`cargo run -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=provedores/alpha.bin --provider=beta=provedores/beta.bin#4@3 \
  --output = carga útil.bin --json-out = fetch_report.json`

- `#<concurrency>` aumenta o paralelismo pelo provedor (`#4` acima).
- `@<weight>` ajusta o horário de agendamento; padrão é 1.
- `--max-peers=<n>` limita o número de provedores agendados para uma execução quando um
  descoberta retorna mais candidatos do que o desejado.
- `--expect-payload-digest` e `--expect-payload-len` protegem contra corrupção silenciosa.
- `--provider-advert=name=advert.to` verifica as capacidades do provedor antes de usá-lo na
  simulação.
- `--retry-budget=<n>` substitui a contagem de tentativa por chunk (padrão: 3) para que o CI
  exponha regressões mais rápidas ao testar cenários de falha.

`fetch_report.json` expõe métricas agregadas (`chunk_retry_total`, `provider_failure_rate`,
etc.) adequadas para asserções de CI e observabilidade.

## 5. Atualizações de registro e governança

Ao propor novos perfis de chunker:

1. Escreva o descritor em `sorafs_manifest::chunker_registry_data`.
2. Atualizar `docs/source/sorafs/chunker_registry.md` e as cartas relacionadas.
3. Regenere fixtures (`export_vectors`) e capture manifestos contratados.
4. Enviar o relatório de conformidade do charter com assinaturas de governança.

A automação deve preferir cabos canônicos (`namespace.name@semver`) e recorrer a IDs
numéricos apenas quando necessário pelo registro.