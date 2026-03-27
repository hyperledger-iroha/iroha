---
id: developer-cli
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note Fuente canónica
Esta página refleja `docs/source/sorafs/developer/cli.md`. Mantén ambas copias sincronizadas.
:::

La superficie consolidada de `sorafs_cli` (proporcionada por el crate `sorafs_car` con la feature `cli` habilitada) expone cada paso necesario para preparar artefactos de SoraFS. Usa este recetario para saltar directamente a flujos comunes; combínalo con el pipeline de manifest y los runbooks del orquestador para contexto operativo.

## Empaquetar payloads

Usa `car pack` para producir archivos CAR deterministas y planes de chunk. El comando selecciona automáticamente el chunker SF-1 salvo que se proporcione un handle.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Handle de chunker predeterminado: `sorafs.sf1@1.0.0`.
- Las entradas de directorio se recorren en orden lexicográfico para que los checksums se mantengan estables entre plataformas.
- El resumen JSON incluye digests de payload, metadatos por chunk y el CID raíz reconocido por el registro y el orquestador.

## Construir manifests

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- Las opciones `--pin-*` se asignan directamente a los campos `PinPolicy` en `sorafs_manifest::ManifestBuilder`.
- Usa `--chunk-plan` cuando quieras que el CLI recalcule el digest SHA3 de chunk antes del envío; de lo contrario reutiliza el digest incrustado en el resumen.
- La salida JSON refleja el payload Norito para diffs simples durante las revisiones.

## Firmar manifests sin claves de larga duración

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Acepta tokens inline, variables de entorno o fuentes basadas en archivos.
- Añade metadatos de procedencia (`token_source`, `token_hash_hex`, digest de chunk) sin persistir el JWT en bruto salvo que `--include-token=true`.
- Funciona bien en CI: combínalo con OIDC de GitHub Actions configurando `--identity-token-provider=github-actions`.

## Enviar manifests a Torii

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority <i105-account-id> \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- Realiza decodificación Norito para alias proofs y verifica que coincidan con el digest del manifest antes de POSTear a Torii.
- Recalcula el digest SHA3 de chunk desde el plan para prevenir ataques de desajuste.
- Los resúmenes de respuesta capturan estado HTTP, headers y payloads del registro para auditorías posteriores.

## Verificar contenidos de CAR y proofs

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- Reconstruye el árbol PoR y compara los digests del payload con el resumen del manifest.
- Captura conteos e identificadores requeridos al enviar proofs de replicación a gobernanza.

## Transmitir telemetría de proofs

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

- Emite elementos NDJSON por cada proof transmitido (desactiva el replay con `--emit-events=false`).
- Agrega conteos de éxito/fallo, histogramas de latencia y fallos muestreados en el resumen JSON para que los dashboards puedan graficar resultados sin leer logs.
- Sale con código distinto de cero cuando el gateway reporta fallos o la verificación PoR local (vía `--por-root-hex`) rechaza proofs. Ajusta los umbrales con `--max-failures` y `--max-verification-failures` para ejecuciones de ensayo.
- Soporta PoR hoy; PDP y PoTR reutilizan el mismo envoltorio cuando lleguen SF-13/SF-14.
- `--governance-evidence-dir` escribe el resumen renderizado, metadatos (timestamp, versión de CLI, URL del gateway, digest del manifest) y una copia del manifest en el directorio suministrado para que los paquetes de gobernanza archiven la evidencia del proof-stream sin repetir la ejecución.

## Referencias adicionales

- `docs/source/sorafs_cli.md` — documentación exhaustiva de flags.
- `docs/source/sorafs_proof_streaming.md` — esquema de telemetría de proofs y plantilla de dashboard Grafana.
- `docs/source/sorafs/manifest_pipeline.md` — profundización en chunking, composición de manifest y manejo de CAR.
