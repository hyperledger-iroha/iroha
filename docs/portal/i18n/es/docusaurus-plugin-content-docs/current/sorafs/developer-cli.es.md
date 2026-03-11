---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: desarrollador-cli
título: Recetario de CLI de SoraFS
sidebar_label: Recetario de CLI
descripción: Recorrido orientado a tareas de la superficie consolidada de `sorafs_cli`.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/developer/cli.md`. Mantén ambas copias sincronizadas.
:::

La superficie consolidada de `sorafs_cli` (proporcionada por el crate `sorafs_car` con la característica `cli` habilitada) exponen cada paso necesario para preparar artefactos de SoraFS. Usa este recetario para saltar directamente a flujos comunes; combínalo con el pipeline de manifest y los runbooks del orquestador para contexto operativo.

## Empaquetar cargas útiles

Usa `car pack` para producir archivos CAR deterministas y planos de trozos. El comando selecciona automáticamente el fragmentador SF-1 salvo que se proporciona un mango.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Mango de fragmentador predeterminado: `sorafs.sf1@1.0.0`.
- Las entradas de directorio se corren en orden lexicográfico para que las sumas de comprobación se mantengan estables entre las plataformas.
- El resumen JSON incluye resúmenes de carga útil, metadatos por fragmento y la raíz CID reconocida por el registro y el orquestador.

## Construir manifiestos

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```- Las opciones `--pin-*` se asignan directamente a los campos `PinPolicy` en `sorafs_manifest::ManifestBuilder`.
- Usa `--chunk-plan` cuando quieras que el CLI recalcule el digest SHA3 de chunk antes del envío; de lo contrario reutiliza el digest incrustado en el resumen.
- La salida JSON refleja la carga útil Norito para diferencias simples durante la revisión.

## Firmar manifiesta sin claves de larga duración

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Acepta tokens inline, variables de entorno o fuentes basadas en archivos.
- Agregue metadatos de procedencia (`token_source`, `token_hash_hex`, resumen de fragmentos) sin persistir el JWT en bruto salvo que `--include-token=true`.
- Funciona bien en CI: combínalo con OIDC de GitHub Actions configurando `--identity-token-provider=github-actions`.

## Enviar manifiesta un Torii

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority i105... \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- Realiza decodificación Norito para aliasproofs y verifica que coinciden con el digest del manifest antes de POSTear a Torii.
- Recalcula el digest SHA3 de chunk desde el plan para prevenir ataques de desajuste.
- Los resúmenes de respuesta capturan estado HTTP, headers y payloads del registro para análisis posteriores.

## Verificar contenidos de CAR y pruebas

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- Reconstruye el árbol PoR y compara los resúmenes del payload con el resumen del manifest.
- Captura de contenidos e identificadores requeridos al enviar pruebas de replicación a gobernanza.## Transmitir telemetría de pruebas

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```

- Emite elementos NDJSON por cada prueba transmitida (desactiva la reproducción con `--emit-events=false`).
- Agrega recuentos de éxito/fallo, histogramas de latencia y fallos muestreados en el resumen JSON para que los paneles puedan graficar resultados sin leer logs.
- Sale con código distinto de cero cuando el gateway reporta fallos o la verificación PoR local (vía `--por-root-hex`) rechaza pruebas. Ajusta los umbrales con `--max-failures` y `--max-verification-failures` para ejecuciones de ensayo.
- Soporta PoR hoy; PDP y PoTR reutilizan el mismo envoltorio cuando llegan SF-13/SF-14.
- `--governance-evidence-dir` escribe el resumen renderizado, metadatos (marca de tiempo, versión de CLI, URL del gateway, resumen del manifiesto) y una copia del manifiesto en el directorio suministrado para que los paquetes de gobernanza archiven la evidencia delproof-stream sin repetir la ejecución.

## Referencias adicionales

- `docs/source/sorafs_cli.md` — documentación exhaustiva de banderas.
- `docs/source/sorafs_proof_streaming.md` — esquema de telemetría de pruebas y plantilla de tablero Grafana.
- `docs/source/sorafs/manifest_pipeline.md` — profundización en fragmentación, composición de manifiesto y manejo de CAR.