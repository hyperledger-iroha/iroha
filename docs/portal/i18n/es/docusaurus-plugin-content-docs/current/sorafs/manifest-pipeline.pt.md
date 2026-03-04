---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Chunking de SoraFS → Canalización de manifiestos

Este complemento del inicio rápido acompaña la tubería de ponta a ponta que transforma bytes
brutos em manifestos Norito adecuados al Pin Registry do SoraFS. O conteúdo foi adaptador de
[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
Consulte este documento para la especificación canónica y el registro de cambios.

## 1. Fazer fragmentación de forma determinística

SoraFS usa el perfil SF-1 (`sorafs.sf1@1.0.0`): un hash rolante inspirado en FastCDC com
tamaño mínimo de trozo de 64 KiB, alvo de 256 KiB, máximo de 512 KiB y máscara de quebra
`0x0000ffff`. El perfil está registrado en `sorafs_manifest::chunker_registry`.

### Ayudantes en Rust

- `sorafs_car::CarBuildPlan::single_file` – Emite compensaciones de fragmentos, complementos y resúmenes
  BLAKE3 mientras prepara los metadados de CAR.
- `sorafs_car::ChunkStore` – Faz streaming de cargas útiles, metadados persistentes de fragmentos y derivaciones
  a árvore de amostragem Proof-of-Retrievability (PoR) de 64 KiB / 4 KiB.
- `sorafs_chunker::chunk_bytes_with_digests` – Ayudante de biblioteca por detrás de las dos CLI.

### Ferramentas de CLI

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

El JSON contiene compensaciones ordenadas, compensaciones y digestiones de dos fragmentos. Preservar el plano ao
construir manifiestos o especificaciones de búsqueda del orquestador.

### Testemunhas PoRO `ChunkStore` exponga `--por-proof=<chunk>:<segment>:<leaf>` e `--por-sample=<count>` para que
auditores possam solicitar conjuntos de pruebas determinísticas. Combinar eses banderas com
`--por-proof-out` o `--por-sample-out` para registrar o JSON.

## 2. Empacotar un manifiesto

`ManifestBuilder` combina metadados de chunks con anexos de gobierno:

- CID raiz (dag-cbor) e compromisos de CAR.
- Provas de alias e alegações de capacidade do provedor.
- Assinaturas do conselho e metadados opcionais (por ejemplo, ID de compilación).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

Dichos importantes:

- `payload.manifest` – Bytes del manifiesto codificados en Norito.
- `payload.report.json` – Resumen legal para humanos/automação, incluido `chunk_fetch_specs`,
  `payload_digest_hex`, resúmenes de CAR y metadados de alias.
- `payload.manifest_signatures.json` – Sobre contendo o resumen BLAKE3 del manifiesto, o
  digerir SHA3 del plano de trozos y assinaturas Ed25519 ordenadas.

Utilice `--manifest-signatures-in` para verificar sobres fornecidos por firmas externas
antes de gravá-los novamente e `--chunker-profile-id` ou `--chunker-profile=<handle>` para
fijar la selección del registro.

## 3. Publicar y pinear1. **Envio àgobernanza** – Forneça o digest do manifesto e o sobre de assinaturas ao
   consejo para que o pin possa ser admitido. Auditores externos deben guardar o digerir SHA3
   do plano de chunks junto ao digest do manifesto.
2. **Pinar cargas útiles** – Faça upload do arquivo CAR (e do índice CAR opcional) referenciado no
   manifiesto para el Registro de PIN. Garanta que o manifesto e o CAR compartilhem o mesmo CID raiz.
3. **Registrador de telemetría** – Preservar el informe JSON, como testemunhas PoR y quaisquer
   métricas de fetch nos artefatos de liberación. Esses registros alimentam paneles de control de
   Los operadores e ayudan a reproducir problemas sin bajar cargas útiles grandes.

## 4. Simulación de búsqueda de múltiples proveedores

`ejecución de carga -p sorafs_car --bin sorafs_fetch --plan=payload.report.json \
  --provider=alpha=proveedores/alpha.bin --provider=beta=proveedores/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`- `#<concurrency>` aumenta o paralelo por proveedor (`#4` acima).
- `@<weight>` ajusta o viés de agendamento; padrón é 1.
- `--max-peers=<n>` limita el número de proveedores agendados para una ejecución cuando a
  descoberta retorna mais candidatos do que o desejado.
- `--expect-payload-digest` e `--expect-payload-len` protegen contra la corrupción silenciosa.
- `--provider-advert=name=advert.to` verifica as capacidades do provedor antes de usá-lo na
  simulación.
- `--retry-budget=<n>` sustituye un contagio de tentativas por chunk (padrón: 3) para que CI
  Exponha regressões mais rápido ao testar cenários de falha.

`fetch_report.json` exposición métrica agregada (`chunk_retry_total`, `provider_failure_rate`,
etc.) adecuadas para afirmaciones de CI y observabilidade.

## 5. Actualizaciones de registro y gobernanza

Ao propor novos perfis de chunker:

1. Escreva o descriptor em `sorafs_manifest::chunker_registry_data`.
2. Actualizar `docs/source/sorafs/chunker_registry.md` e según cartas relacionadas.
3. Regenerar accesorios (`export_vectors`) y capturar manifiestos assinados.
4. Envie o relatório de conformidade do charter com assinaturas degobernanza.

La automatización debe preferir manejadores canónicos (`namespace.name@semver`) y recorrer ID
numéricos apenas cuando sea necesario el registro.