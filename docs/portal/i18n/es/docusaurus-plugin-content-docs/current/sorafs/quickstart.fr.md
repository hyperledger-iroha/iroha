---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/quickstart.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Démarrage rapide SoraFS

Esta guía práctica pasa en revista el perfil del fragmentador SF-1 determinado,
la firma de los manifiestos y el flujo de recuperación de múltiples proveedores que
sous-tendent le pipe de stockage SoraFS. Complétez-le par
l'[analyse approfondie du pipeline de manifestes](manifest-pipeline.md)
Para las notas de concepción y la referencia de las banderas CLI.

## Requisitos previos

- Toolchain Rust (`rustup update`), ubicación clonada del espacio de trabajo.
- Opcional: [par de claves Ed25519 generadas por OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  pour signer les manifestes.
- Opcional: Node.js ≥ 18 si desea visualizar el portal Docusaurus.

Définissez `export RUST_LOG=info` durante los ensayos para mostrar mensajes útiles CLI.

## 1. Rafraîchir les fixtures déterministes

Regénérez les vecteurs de découpage SF-1 canonices. El comando del producto aussi des
sobres de manifiesto firmados lorsque `--signing-key` est fourni ; utilisé
`--allow-unsigned` Único y desarrollado localmente.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Salidas :

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (si firmado)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Descubra una carga útil e inspeccione el plan

Utilice `sorafs_chunker` para descubrir un archivo o archivo arbitrario:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Campos clés:- `profile` / `break_mask` – confirme los parámetros de `sorafs.sf1@1.0.0`.
- `chunks[]` – compensa los trozos ordenados, largos y empreintes BLAKE3.

Para accesorios más voluminosos, ejecute la regresión base sobre proptest afin
Asegúrese de que el découpage en streaming y de que el resto esté sincronizado:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. Construir y firmar un manifiesto

Enveloppez le plan de chunks, les alias et les firmas de gobierno en un
manifiesto a través de `sorafs-manifest-stub`. La orden ci-dessous ilustra una carga útil
archivo único; pase un camino de repertorio para empaquetar un árbol (la CLI le
parcourt en ordre lexicographique).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

Examine `/tmp/docs.report.json` para:

- `chunking.chunk_digest_sha3_256` – empreinte SHA3 des offsets/longueurs, corresponden aux
  accesorios del trozo.
- `manifest.manifest_blake3` – empreinte BLAKE3 firmado en el sobre del manifiesto.
- `chunk_fetch_specs[]` – instrucciones de recuperación ordenadas para los orquestadores.

Quand vous êtes prêt à fournir de vraies firmas, ajoutez les arguments
`--signing-key` y `--signer`. La commande vérifie chaque firma Ed25519 avant
d'écrire l'enveloppe.

## 4. Simulador de recuperación de múltiples proveedores

Utilice la CLI de recuperación de desarrollo para reanudar el plan de fragmentos con uno o
plusieurs fournisseurs. Es ideal para las pruebas de humo CI y la página de prototipos
d'orquestador.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Verificaciones :- `payload_digest_hex` corresponde al informe del manifiesto.
- `provider_reports[]` exponen les comptes de succès/échec par fournisseur.
- Un `chunk_retry_total` no nulo cumple con los ajustes de contrapresión.
- Passez `--max-peers=<n>` para limitar el número de proveedores planificados para una
  Ejecución y mantenimiento de las simulaciones CI centradas en los candidatos principales.
- `--retry-budget=<n>` reemplaza el nombre de tentativos por fragmento por defecto (3) afin de
  Mettre en évidence plus vite les régressions de l'orchestrateur lors de l'injection
  d'échecs.

Ajuste `--expect-payload-digest=<hex>` e `--expect-payload-len=<bytes>` para escuchar
Rapidement lorsque le payload reconstruit s'écarte du manifeste.

## 5. Étapes siguientes

- **Gobernanza de la integración** – acheminer l'empreinte du manifeste et
  `manifest_signatures.json` dans le flux du conseil afin que le Pin Registry puisse
  anunciar la disponibilidad.
- **Négociación del registro** – consulte [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  avant d'enregistrer de nouveaux profils. La automatización debe privilegier las manijas.
  Canoniques (`namespace.name@semver`) junto con los ID numéricos.
- **Automatización CI**: agregue los comandos ci-dessus aux pipelines de release pour que
  la documentación, los accesorios y los artefactos públicos de los manifiestos determinantes
  avec des métadonnées signées.