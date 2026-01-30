---
lang: es
direction: ltr
source: docs/source/sorafs_gateway_self_cert.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dea5a18153f27d4ff7d6f334c915591841900561d37d53ea104986fd0d3c26ef
source_last_modified: "2025-11-14T20:06:47.594457+00:00"
translation_last_reviewed: "2026-01-30"
---

# Kit de auto-certificacion del gateway SoraFS

Esta guia explica como los operadores ejecutan el harness de self-cert, producen
un bundle de atestacion firmado y archivan los resultados como parte del
checklist de onboarding.

## Entregables

- **Runner del harness:** `cargo xtask sorafs-gateway-attest` ejecuta escenarios de replay + carga, verifica exito y emite artefactos (`report.json`, atestacion `.to`, resumen humano).
- **Script wrapper:** `scripts/sorafs_gateway_self_cert.sh` envuelve el comando xtask con flags amigables para que Ops lo invoque desde CI o shell.
- **Plantilla de reporte:** `docs/source/examples/sorafs_gateway_self_cert_template.json` muestra la estructura JSON capturada en cada corrida (util para ingestion en dashboards o revisiones de compliance).

## Prerrequisitos

- Workspace con `xtask` disponible (`cargo xtask --help` debe listar `sorafs-gateway-attest`).
- Archivo de config (key=value) que registra la ruta de la key de firmado, cuenta firmante y cualquier input opcional de verificacion de manifiesto. Ver `docs/examples/sorafs_gateway_self_cert.conf` como plantilla.
- Acceso al endpoint de gateway staging/produccion que quieres certificar.
- Key de firmado Ed25519 en hex (sin prefijo) atada a la cuenta de admision del operador.
- Opcional: directorio de salida custom; por defecto `artifacts/sorafs_gateway_attest`.

## Ejecutar el kit

- Proveer opciones directamente o colocarlas en un archivo de config (ver `docs/examples/sorafs_gateway_self_cert.conf`). Los flags sobreescriben entradas de config.

```bash
./scripts/sorafs_gateway_self_cert.sh \
  --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest-bundle path/to/updated_manifest.bundle.json
```

- El script reenvia argumentos a `cargo xtask sorafs-gateway-attest …` y, cuando hay inputs de manifiesto, a `sorafs_cli manifest verify-signature`.
- `--gateway` es opcional; omitelo (o quitalo del archivo de config) para usar el target de fixtures por defecto del harness.
- Usa `--workspace` si la raiz del repositorio difiere de tu directorio actual.

## Artefactos de salida

La corrida crea tres archivos:

| Archivo | Descripcion |
|---------|-------------|
| `sorafs_gateway_report.json` | Reporte canonico Norito/JSON (coincide con la plantilla bajo `docs/source/examples/`). |
| `sorafs_gateway_attestation.to` | Sobre Norito firmado que contiene hash de payload, metadata del firmante y firma Ed25519. |
| `sorafs_gateway_attestation.txt` | Resumen legible para humanos, apto para tickets de cambio. |

El reporte JSON incluye:
- Metadata del gateway (`gateway.target`, `gateway.version` opcional cuando se provee via env/flags).
- Resultados de escenarios y metricas (ver plantilla).
- Contadores de rechazo de stream-token, tasa de retry de chunk, reportes de provider.
- Hash de payload + bloque de firma.

## Verificar la atestacion

1. Inspeccionar el resumen: `cat artifacts/.../sorafs_gateway_attestation.txt`.
2. Verificar la firma usando `norito::decode_from_bytes` o el helper en `xtask`:
   ```bash
   cargo xtask sorafs-verify-attestation \
     --envelope artifacts/.../sorafs_gateway_attestation.to
   ```
3. Archivar `report.json` y el resumen en el ticket de onboarding; enviar el sobre `.to` a tooling de governance si se requiere.

## Verificacion opcional de manifiestos

Si no se suministran flags o valores de config, el script cae a los fixtures de muestra en `fixtures/sorafs_manifest/ci_sample/` (incluyendo la key de muestra `gateway_attestor.hex`), permitiendo un dry-run listo para usar. Provee `--manifest` (sea en el archivo de config o por flags) junto con:

- `--manifest-bundle` (preferido, verifica metadata y firma del bundle), o
- `--manifest-signature` + `--public-key-hex` (flujo de firma separada).

El wrapper invoca `sorafs_cli manifest verify-signature` despues de completar el harness y escribe el resumen de verificacion en `<out>/manifest.verify.summary.json`. Puedes pasar `--chunk-plan`, `--chunk-summary` o `--chunk-digest-sha3` para que el CLI tambien valide digests de chunks y metadata embebida en el bundle.

## Evidencia de diff de denylist (MINFO-6)

Al rotar denylists del gateway SoraFS, governance espera un rastro before/after que resalte cada entrada cambiada. El wrapper de self-cert ahora se conecta directo al helper `cargo xtask sorafs-gateway denylist diff`:

- Proveer `--denylist-old <bundle.json>` y `--denylist-new <bundle.json>` (via flags o config). El script valida que ambas rutas existan y luego ejecuta el comando diff. Suministra `--denylist-report <path>` para overrides de la ruta del reporte JSON; de lo contrario usa `<out>/denylist_diff.json`.
- El comando imprime los conteos de entradas agregadas/eliminadas y deja un bundle JSON de evidencia que refleja el output del xtask (formato de auditoria MINFO-6). Adjunta esto a los paquetes de governance del Ministry junto a los artefactos de atestacion.
- Cuando solo una de las flags `--denylist-*` esta presente, el script omite el diff y emite una advertencia, evitando corridas parciales que produzcan evidencia engañosa.

## Troubleshooting

- Si cualquier escenario falla, el comando xtask aborta y no se produce una atestacion. Revisa el output del harness y sigue la guia de rechazos en `docs/source/sorafs_gateway_refusal_guidance.md` antes de re-ejecutar.
- 5xx persistentes o picos de rechazo deben tratarse como incidentes; recolecta telemetria de los dashboards listados en el manual de despliegue.

## Tips de automatizacion

- Integra el script en pipelines de CI (p. ej., GitHub Actions) para generar una atestacion fresca tras cada rollout de gateway.
- `.github/workflows/sorafs-gateway-self-cert.yml` consume un archivo de config y archiva tanto los outputs de atestacion como `manifest.verify.summary.json`, manteniendo la verificacion reproducible sin depender de variables de entorno.
- Disparar el workflow con:

  ```bash
  gh workflow run sorafs-gateway-self-cert \
    --ref main \
    --field config_path=docs/examples/sorafs_gateway_self_cert.conf
  ```
- Mantener los artefactos de manifiesto junto a los outputs del gateway para que CI invoque el script con `--manifest`/`--manifest-bundle` y falle rapido ante drift de firmas.
- Usar `--gateway-manifest-id` / flags relacionados en `sorafs-fetch` (ver el manual de despliegue) para smoke tests complementarios antes de ejecutar el suite completo de self-cert.

## Referencias

- Manual de despliegue y operaciones: `docs/source/sorafs_gateway_deployment_handbook.md`
- Harness de conformidad/carga: `docs/source/sorafs_gateway_conformance.md`
- Plantilla de reporte: `docs/source/examples/sorafs_gateway_self_cert_template.json`
- Plantilla de config: `docs/examples/sorafs_gateway_self_cert.conf`
