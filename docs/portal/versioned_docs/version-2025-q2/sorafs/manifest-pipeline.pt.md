---
lang: pt
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/manifest-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e77b792e19fbfa8e1efeddd042adbe68a48287a582a1be76aa518af7830774e2
source_last_modified: "2026-01-04T10:50:53.604570+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS Chunking → Pipeline de manifesto

Este complemento do início rápido rastreia o pipeline de ponta a ponta que se torna bruto
bytes em manifestos Norito adequados para o registro de pinos SoraFS. O conteúdo é
adaptado de [`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
consulte esse documento para obter a especificação canônica e o changelog.

## 1. Pedaço determinístico

SoraFS usa o perfil SF-1 (`sorafs.sf1@1.0.0`): um perfil rolante inspirado no FastCDC
hash com tamanho mínimo de bloco de 64 KiB, destino de 256 KiB, máximo de 512 KiB e um
Máscara de quebra `0x0000ffff`. O perfil está cadastrado em
`sorafs_manifest::chunker_registry`.

### Ajudantes de ferrugem

- `sorafs_car::CarBuildPlan::single_file` – Emite deslocamentos de pedaços, comprimentos e
  BLAKE3 faz a digestão enquanto prepara os metadados do CAR.
- `sorafs_car::ChunkStore` – Transmite cargas úteis, persiste metadados de pedaços e
  deriva a árvore de amostragem de prova de recuperação (PoR) de 64KiB/4KiB.
- `sorafs_chunker::chunk_bytes_with_digests` – Auxiliar de biblioteca por trás de ambas as CLIs.

### Ferramentas CLI

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

O JSON contém os deslocamentos ordenados, comprimentos e resumos de pedaços. Persista o
planejar ao construir manifestos ou especificações de busca do orquestrador.

### Testemunhas de PoR

`ChunkStore` expõe `--por-proof=<chunk>:<segment>:<leaf>` e
`--por-sample=<count>` para que os auditores possam solicitar conjuntos de testemunhas determinísticas. Par
esses sinalizadores com `--por-proof-out` ou `--por-sample-out` para registrar o JSON.

## 2. Embrulhar um manifesto

`ManifestBuilder` combina metadados de blocos com anexos de governança:

- Compromissos Root CID (dag-cbor) e CAR.
- Provas de alias e declarações de capacidade do provedor.
- Assinaturas do conselho e metadados opcionais (por exemplo, IDs de construção).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

Resultados importantes:

- `payload.manifest` – bytes de manifesto codificados em Norito.
- `payload.report.json` – Resumo legível por humanos/automação, incluindo
  `chunk_fetch_specs`, `payload_digest_hex`, resumos CAR e metadados de alias.
- `payload.manifest_signatures.json` – Envelope contendo manifesto BLAKE3
  resumo, resumo SHA3 de plano de bloco e assinaturas Ed25519 classificadas.

Use `--manifest-signatures-in` para verificar envelopes fornecidos por terceiros
signatários antes de devolvê-los, e `--chunker-profile-id` ou
`--chunker-profile=<handle>` para bloquear a seleção do registro.

## 3. Publicar e fixar

1. **Envio de governança** – Forneça o resumo e a assinatura do manifesto
   envelope ao conselho para que o distintivo seja admitido. Os auditores externos devem
   armazene o resumo SHA3 do plano de bloco junto com o resumo do manifesto.
2. **Pin payloads** – Faça upload do arquivo CAR (e índice CAR opcional) referenciado
   no manifesto do Pin Registry. Garantir que o manifesto e o CAR compartilhem o
   mesmo CID raiz.
3. **Registrar telemetria** – Persistir o relatório JSON, testemunhas PoR e qualquer busca
   métricas em artefatos de lançamento. Esses registros alimentam os painéis do operador e
   ajude a reproduzir problemas sem baixar grandes cargas.

## 4. Simulação de busca de vários provedores

`cargo run -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=provedores/alpha.bin --provider=beta=provedores/beta.bin#4@3 \
  --output = carga útil.bin --json-out = fetch_report.json`- `#<concurrency>` aumenta o paralelismo por provedor (`#4` acima).
- `@<weight>` ajusta o viés de agendamento; o padrão é 1.
- `--max-peers=<n>` limita o número de provedores agendados para uma execução quando
  a descoberta produz mais candidatos do que o desejado.
- `--expect-payload-digest` e `--expect-payload-len` protegem contra silêncio
  corrupção.
- `--provider-advert=name=advert.to` verifica os recursos do provedor antes
  usá-los na simulação.
- `--retry-budget=<n>` substitui a contagem de novas tentativas por bloco (padrão: 3) para que CI
  pode revelar regressões mais rapidamente ao testar cenários de falha.

`fetch_report.json` apresenta métricas agregadas (`chunk_retry_total`,
`provider_failure_rate`, etc.) adequado para afirmações e observabilidade de CI.

## 5. Atualizações e governança de registro

Ao propor novos perfis de chunker:

1. Crie o descritor em `sorafs_manifest::chunker_registry_data`.
2. Atualize `docs/source/sorafs/chunker_registry.md` e regulamentos relacionados.
3. Gere novamente os fixtures (`export_vectors`) e capture manifestos assinados.
4. Apresentar o relatório de conformidade do estatuto com assinaturas de governança.

A automação deve preferir identificadores canônicos (`namespace.name@semver`) e cair