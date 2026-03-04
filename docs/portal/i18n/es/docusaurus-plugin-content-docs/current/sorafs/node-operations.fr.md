---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/node-operations.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operaciones de nodo
título: Runbook d'exploitation du nœud
sidebar_label: Runbook de explotación del nuevo
descripción: Valide le déploiement embarqué de `sorafs-node` dans Torii.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Guarde las dos versiones sincronizadas justo cuando el conjunto Sphinx está retirado.
:::

## Vista del conjunto

Este runbook guía a los operadores para validar un despliegue `sorafs-node` embarcado en Torii. Cada sección corresponde a directement aux livrables SF-3: boucles pin/fetch, reprise après redémarrage, rejet de cuotas y échantillonnage PoR.

## 1. Requisitos previos

- Active el trabajador de almacenamiento en `torii.sorafs.storage`:

  ```toml
  [torii.sorafs.storage]
  enabled = true
  data_dir = "./storage/sorafs"
  max_capacity_bytes = 21474836480    # 20 GiB
  max_parallel_fetches = 32
  max_pins = 1000
  por_sample_interval_secs = 600

  [torii.sorafs.storage.metering_smoothing]
  gib_hours_enabled = true
  gib_hours_alpha = 0.25
  por_success_enabled = true
  por_success_alpha = 0.25
  ```

- Asegúrese de que el proceso Torii disponga de una lectura/escritura de acceso a `data_dir`.
- Confirme que le nœud annonce la capacidad de asistencia a través de `GET /v1/sorafs/capacity/state` una vez que haya declarado registrada.
- Cuando el sonido está activo, los paneles de control se exponen a las computadoras GiB·hour/PoR brutas y suaves para detectar tendencias sin fluctuaciones en el valor instantáneo.

### Ejecución en blanco de la CLI (opcional)

Antes de exponer los puntos finales HTTP, puede validar el backend de almacenamiento con el soporte CLI. 【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin

cargo run -p sorafs_node --bin sorafs-node export \
  --data-dir ./storage/sorafs \
  --manifest-id <hex> \
  --manifest-out ./out/manifest.to \
  --payload-out ./out/payload.bin
```Los comandos imprimen los currículums Norito JSON y rechazan las divergencias de perfil de fragmento o resumen, lo que los hace útiles para los controles de humo CI antes del cableado de Torii.【crates/sorafs_node/tests/cli.rs#L1】

### Répétition de preuve PoR

Los operadores pueden sufrir una alteración en la ubicación de los artefactos PoR emitidos por la gobernanza antes del televisor versión Torii. La CLI vuelve a utilizar el mismo camino de ingestión `sorafs-node`, de modo que las ejecuciones locales exponen exactamente los errores de validación que genera la API HTTP.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

El comando contiene un currículum JSON (resumen de manifiesto, identificación del facilitador, resumen previo, nombre de échantillons, veredicto opcional). Fournissez `--manifest-id=<hex>` para garantizar que el manifiesto almacenado corresponde al resumen del desafío, y `--json-out=<path>` si desea archivar el currículum con los artefactos de origen como antes de la auditoría. Incluir `--verdict` permite repetir el conjunto del desafío del ciclo → prueba → veredicto en local antes de llamar a la API HTTP.

Una vez Torii en línea, puedes recuperar los mismos artefactos a través de HTTP:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Los dos puntos finales son servidos por el trabajador de almacenamiento embarcado, después de que las pruebas de humo CLI y las sondas de puerta de enlace permanecen alineadas.## 2. Boucle Pin → Recuperar

1. Produzca un paquete de manifiesto + carga útil (por ejemplo a través de `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. Soumettez le manifest en base64:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   El JSON de solicitud debe contener `manifest_b64` e `payload_b64`. Una respuesta es el envío `manifest_id_hex` y el resumen de la carga útil.
3. Recupérez les données épinglées :

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Décoder en base64 le champ `data_b64` y verifique qué corresponden a los octetos originales.

## 3. Ejercicio de repetición después del nuevo matrimonio

1. Épinglez au moins un manifest comme ci-dessus.
2. Redémarrez le processus Torii (ou le nœud complet).
3. Renvoyez la requête fetch. La carga útil debe permanecer recuperable y el resumen renovado debe corresponderse con el valor de antes de volver a casarse.
4. Inspeccione `GET /v1/sorafs/storage/state` para confirmar que `bytes_used` refleja los manifiestos persistentes después del reinicio.

## 4. Prueba de rechazo de cuota

1. Abaissez temporairement `torii.sorafs.storage.max_capacity_bytes` à une valeur faible (par exemple la taille d’un seul manifest).
2. Épinglez un manifiesto; la requête doit réussir.
3. Tentez d'épingler un second manifest de taille similar. Torii debe rechazar la solicitud con HTTP `400` y un mensaje de error contenido `storage capacity exceeded`.
4. Restaure el límite de capacidad normal después de la prueba.

## 5. Sonda de échantillonnage PoR

1. Épinglez un manifiesto.
2. Demandez un échantillon PoR :

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```3. Verifique que la respuesta contenga `samples` con el nombre solicitado y que cada vez que preuve valide contra la racine du manifest stocké.

## 6. Ganchos de automatización

- Las pruebas CI/humo pueden reutilizar los controles ciblés ajustados en:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  que cubre `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection` y `por_sampling_returns_verified_proofs`.
- Los paneles de control deben seguir:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` y `torii_sorafs_storage_fetch_inflight`
  - les compteurs de succès/échec PoR exposés vía `/v1/sorafs/capacity/state`
  - les tentatives de publicación de liquidación vía `sorafs_node_deal_publish_total{result=success|failure}`

Suivre ces Drills garantiza que el trabajador de almacenamiento embarqué puede recibir los données, sobrevivir a los redémarrages, respetar las cuotas configuradas y generar preuves PoR determinados antes de que le nœud n'annonce sa capacidad au reste du réseau.