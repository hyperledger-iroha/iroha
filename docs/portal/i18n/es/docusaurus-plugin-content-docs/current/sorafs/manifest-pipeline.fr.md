---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Chunking SoraFS → Canalización de manifiesto

Este complemento en el inicio rápido recorre la tubería de combate y combate que transforma los octetos brutos.
en manifestes Norito adaptés au Pin Registry de SoraFS. El contenido está adaptado a
[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
Consulte este documento para las especificaciones canónicas y el registro de cambios.

## 1. Fragmento de manera determinada

SoraFS utiliza el perfil SF-1 (`sorafs.sf1@1.0.0`): un hash roulant inspirado en FastCDC con
un tamaño mínimo de 64 KiB, un tamaño de 256 KiB, un máximo de 512 KiB y una máscara
de ruptura `0x0000ffff`. El perfil está registrado en `sorafs_manifest::chunker_registry`.

### Ayudantes del óxido

- `sorafs_car::CarBuildPlan::single_file` – Émet les offsets, longueurs et empreintes BLAKE3
  des chunks hanging la préparation des métadonnées CAR.
- `sorafs_car::ChunkStore`: transmite las cargas útiles, persiste las mezcladas de fragmentos y deriva
  l'arbre d'échantillonnage Prueba de recuperabilidad (PoR) de 64 KiB / 4 KiB.
- `sorafs_chunker::chunk_bytes_with_digests` – Ayudante de biblioteca detrás de las dos CLI.

### Herramientas CLI

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

El JSON contiene los desplazamientos ordenados, los largos y los empreintes de los fragmentos. Conserve le
planifique lors de la construcción de manifiestos o des especificaciones de búsqueda para el orquestador.

### Mensajes PoR`ChunkStore` exponen `--por-proof=<chunk>:<segment>:<leaf>` e `--por-sample=<count>` después de que les
auditeurs puissent demander des ensembles de témoins déterministes. Asociez ces flags à
`--por-proof-out` o `--por-sample-out` para registrar el JSON.

## 2. Envolver un manifiesto

`ManifestBuilder` combine les métadonnées de chunks con des pièces de gouvernance :

- CID racine (dag-cbor) y compromisos CAR.
- Preuves d'alias et declaraciones de capacidad de los proveedores.
- Signatures du conseil et métadonnées optionnelles (por ejemplo, ID de construcción).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

Salidas importantes:

- `payload.manifest` – Octetos de manifiesto codificados en Norito.
- `payload.report.json` – Currículum vitae legal para personas/automatización, incluido
  `chunk_fetch_specs`, `payload_digest_hex`, empreintes CAR y métadonnées d'alias.
- `payload.manifest_signatures.json` – Sobre que contiene el documento BLAKE3 del manifiesto,
  Empreinte SHA3 du plan de chunks et des firmas Ed25519 triées.

Utilice `--manifest-signatures-in` para verificar los sobres cubiertos por los firmantes.
externos antes de la grabación, et `--chunker-profile-id` o `--chunker-profile=<handle>` para
bloquear la selección del registro.

## 3. Publicador y fijador1. **Soumission à la gouvernance** – Fournissez l'empreinte du manifeste et l'enveloppe de
   firmas en consejo para que le pin puisse être admis. Los auditores externos hacen
   conserve el empreinte SHA3 del plan de trozos con el empreinte du manifeste.
2. **Fijar cargas útiles** – Cargar el archivo CAR (y el índice CAR opcional) referenciados en
   le manifeste vers le Pin Registro. Assurez-vous que le manifeste et le CAR partagent le
   Même CID Racine.
3. **Registrar la télémétrie** – Conservar la relación JSON, las témoins PoR y todas las mediciones
   de fetch dans les artefactos de liberación. Estos registros alimentan los paneles de control.
   Operadores y asistentes para reproducir incidentes sin descargador de cargas voluminosas.

## 4. Simulación de búsqueda de múltiples proveedores

`ejecución de carga -p sorafs_car --bin sorafs_fetch --plan=payload.report.json \
  --provider=alpha=proveedores/alpha.bin --provider=beta=proveedores/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`- `#<concurrency>` aumenta el paralelo por el proveedor (`#4` ci-dessus).
- `@<weight>` ajuste le biais d'ordonnancement ; el valor por defecto es 1.
- `--max-peers=<n>` limite el número de proveedores planificados para una ejecución cuando la
  Découverte renvoie plus de candidats que souhaité.
- `--expect-payload-digest` e `--expect-payload-len` protegen contra la corrupción silenciosa.
- `--provider-advert=name=advert.to` verifique las capacidades del proveedor antes del uso
  en la simulación.
- `--retry-budget=<n>` reemplaza el nombre de tentativos por fragmento (por defecto: 3) afin que la
  CI revela más rápidamente las regresiones de los escenarios de panne.

`fetch_report.json` exponen las medidas agregadas (`chunk_retry_total`, `provider_failure_rate`,
etc.) adaptées aux afirmaciones CI et à l'observabilité.

## 5. Mises à jour du registre et gouvernance

Lors de la propuesta de nuevos perfiles de chunker:

1. Rédigez le descripteur dans `sorafs_manifest::chunker_registry_data`.
2. Mettez à jour `docs/source/sorafs/chunker_registry.md` et les chartes associées.
3. Régénérez les fixtures (`export_vectors`) et capturez des manifestes signés.
4. Soumettez le rapport de conformité de la charte avec des Signatures de Gouvernance.

L'automatisation doit privilégier les handles canonices (`namespace.name@semver`) et ne
Revenir aux IDs numéricos que son necesarios para el registro.