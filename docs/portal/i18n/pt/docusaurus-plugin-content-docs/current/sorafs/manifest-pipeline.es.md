---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Chunking de SoraFS → Pipeline de manifestação

Este complemento do início rápido recorre ao pipeline de extremo a extremo que converte
bytes brutos em manifestos Norito apropriados para o Pin Registry de SoraFS. O conteúdo está
adaptado de [`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
consulte este documento para a especificação canônica e o changelog.

## 1. Fragmentar a forma determinista

SoraFS usa o perfil SF-1 (`sorafs.sf1@1.0.0`): um hash rodante inspirado em FastCDC com um
tamanho mínimo de 64 KiB, uma meta de 256 KiB, um máximo de 512 KiB e uma máscara
de corte `0x0000ffff`. O perfil está registrado em `sorafs_manifest::chunker_registry`.

### Ajudantes de Ferrugem

- `sorafs_car::CarBuildPlan::single_file` – Emite deslocamentos de pedaços, longitudes e resumos
  BLAKE3 enquanto prepara os metadados do CAR.
- `sorafs_car::ChunkStore` – Cargas úteis do Streamea, persistem metadados de pedaços e derivam a árvore
  do mapa Prova de Recuperabilidade (PoR) de 64 KiB / 4 KiB.
- `sorafs_chunker::chunk_bytes_with_digests` – Auxiliar de biblioteca além de ambas CLIs.

### Ferramentas de CLI

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

O JSON contém os deslocamentos ordenados, as longitudes e os resumos dos pedaços. Guarda
o plano de construção manifesta as especificações de busca do orquestrador.

###Testigos PoR

`ChunkStore` expõe `--por-proof=<chunk>:<segment>:<leaf>` e `--por-sample=<count>` para que
os auditores podem solicitar conjuntos de testes deterministas. Combinar bandeiras esos com
`--por-proof-out` ou `--por-sample-out` para registrar o JSON.

## 2. Envolver um manifesto

`ManifestBuilder` combina os metadados de chunks com acessórios de governança:

- CID raíz (dag-cbor) e compromissos de CAR.
- Testes de pseudônimo e reivindicações de capacidade de provedores.
- Firmas do conselho e metadados opcionais (p. ej., IDs de build).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

Salidas importantes:

- `payload.manifest` – Bytes dos manifestos codificados em Norito.
- `payload.report.json` – Resumo legível para humanos/automatização, incluído
  `chunk_fetch_specs`, `payload_digest_hex`, digere CAR e metadados de alias.
- `payload.manifest_signatures.json` – Sobre o que contém o resumo BLAKE3 do manifesto, o
  digerir SHA3 do plano de pedaços e firmas Ed25519 ordenadas.

Use `--manifest-signatures-in` para verificar sobre os dados fornecidos por empresas externas
antes de voltar a escrevê-los, e `--chunker-profile-id` ou `--chunker-profile=<handle>` para
faça a seleção do registro.

## 3. Publicar y pinear1. **Envio a gobernanza** – Proporciona o resumo do manifesto e o sobre de firmas al
   conselho para que o pino possa ser admitido. Os auditores externos devem armazenar o
   resumo SHA3 do plano de pedaços junto com o resumo do manifesto.
2. **Pinear payloads** – Sube o arquivo CAR (e o índice CAR) referenciado opcionalmente no
   manifesta-se no Registro Pin. Certifique-se de que o manifesto e o CAR compartilhem o mesmo CID raiz.
3. **Registrar telemetria** – Conserva o relatório JSON, os testes PoR e qualquer métrica
   de fetch nos artefatos de liberação. Esses registros alimentam os painéis de controle
   operadores e ajudam a reproduzir incidentes sem baixar cargas úteis grandes.

## 4. Simulação de busca de vários provedores

`cargo run -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=provedores/alpha.bin --provider=beta=provedores/beta.bin#4@3 \
  --output = carga útil.bin --json-out = fetch_report.json`

- `#<concurrency>` aumenta o paralelismo pelo provedor (`#4` para cima).
- `@<weight>` ajusta a sessão de planejamento; por defeito é 1.
- `--max-peers=<n>` limita o número de provedores programados para uma execução quando o
  a descoberta produz mais candidatos dos desejados.
- `--expect-payload-digest` e `--expect-payload-len` protegem contra corrupção silenciosa.
- `--provider-advert=name=advert.to` verifica as capacidades do fornecedor antes de usá-las
  na simulação.
- `--retry-budget=<n>` substitui o registro de reintenções por pedaço (por defeito: 3) para que
  CI pode expor regressões mais rápidas em cenários de falha.

`fetch_report.json` mostra métricas agregadas (`chunk_retry_total`, `provider_failure_rate`,
etc.) adequadas para aserções de CI e observabilidade.

## 5. Atualizações de registro e governança

Ao propor novos perfis de chunker:

1. Redija o descritor em `sorafs_manifest::chunker_registry_data`.
2. Atualização `docs/source/sorafs/chunker_registry.md` e a carta relacionada.
3. Regenera fixtures (`export_vectors`) e captura manifestos firmados.
4. Enviar o relatório de cumprimento da carta às firmas de governo.

A automação deve preferir lidar com canônicos (`namespace.name@semver`) e recorrer a IDs
números apenas quando necessário para o registro.