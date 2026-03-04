---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/quickstart.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Inicio rápido de SoraFS

Esta guía práctica percorre o perfil determinístico de chunker SF-1,
a assinatura de manifestos e o fluxo de busca multi-provedor que sustentam o
tubería de armazenamento do SoraFS. Combinar-o com o
[mergulho profundo no pipeline de manifestos](manifest-pipeline.md)
para notas de diseño y referencia de banderas de CLI.

## Requisitos previos

- Toolchain do Rust (`rustup update`), espacio de trabajo clonado localmente.
- Opcional: [par de chaves Ed25519 compatible con OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  manifiestos para assinar.
- Opcional: Node.js ≥ 18 si desea visualizar previamente el portal Docusaurus.

Defina `export RUST_LOG=info` durante los testículos para exportar mensajes utilizados por CLI.

## 1. Actualizar los calendarios determinísticos

Gere novamente os vetores canônicos de fragmentación SF-1. El comando también emite
sobres de manifiesto assinados quando `--signing-key` é fornecido; utilizar
`--allow-unsigned` apenas no hay desarrollo local.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Saidas:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (se asesinó)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Fragmento de la carga útil e inspección del plano

Utilice `sorafs_chunker` para fragmentar un archivo o un archivo compacto arbitrario:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Campos-chave:

- `profile` / `break_mask` – confirma los parámetros de `sorafs.sf1@1.0.0`.
- `chunks[]` – compensa ordenados, comprimentos y digiere BLAKE3 dos fragmentos.Para los partidos mayores, ejecute una regresión con proptest para garantizar que o
fragmentación en streaming y en lotes permanentemente sincronizados:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. Construa e assine um manifiesto

Empacote o plano de trozos, os alias y assinaturas de gobernanza en un manifiesto
usando `sorafs-manifest-stub`. El comando abaixo mostró una carga útil de archivo único; pasar
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

Revisar `/tmp/docs.report.json` párrafo:

- `chunking.chunk_digest_sha3_256` – resumen SHA3 de compensaciones/comprimentos, corresponde aos
  los accesorios se fragmentan.
- `manifest.manifest_blake3` – resumen BLAKE3 assinado no sobre do manifesto.
- `chunk_fetch_specs[]` – instrucciones de búsqueda ordenadas para orquestadores.

Cuando estiver pronto para fornecer assinaturas reales, adición de argumentos
`--signing-key` e `--signer`. O comando verifica cada assinatura Ed25519 antes de gravar
o sobre.

## 4. Simule a recuperação multi-provedor

Utilice una CLI de recuperación de desarrollo para reproducir el plano de fragmentos contra uno o
mais provedores. Esto es ideal para pruebas de humo de CI y prototipos de orquestador.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Verificaciones:- `payload_digest_hex` debe corresponder al relato del manifiesto.
- `provider_reports[]` mostra contagens de sucesso/falha por provedor.
- `chunk_retry_total` diferente de cero destaca ajustes de contrapresión.
- Passe `--max-peers=<n>` para limitar el número de proveedores programados para una ejecución
  e manter as simulações de CI focadas nos candidatos principais.
- `--retry-budget=<n>` reemplaza el padrón de tentativas por trozo (3) para exportar
  Regressões do orquestador mais rápido ao injetar falhas.

Agregar `--expect-payload-digest=<hex>` e `--expect-payload-len=<bytes>` para falhar
Rápidamente cuando la carga útil se reconstruyó divergir del manifiesto.

## 5. Próximos pasos

- **Integração degobernanza** – envie o digest do manifesto e `manifest_signatures.json`
  para o fluxo do conselho para que o Pin Registry possa anunciar disponibilidade.
- **Negociação de registro** – consultar [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  antes de registrador novos perfis. La automatización debe preferir identificadores canónicos
  (`namespace.name@semver`) en vez de ID numéricos.
- **Automatización de CI** – adición de comandos acima aos pipelines de release para que a
  documentación, accesorios y artefatos publiquem manifiestos determinísticos junto com
  metadados asesinados.