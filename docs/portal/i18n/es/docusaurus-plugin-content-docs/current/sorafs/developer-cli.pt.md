---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: desarrollador-cli
título: Recetas de CLI de SoraFS
sidebar_label: Recetas de CLI
descripción: Guía enfocada en tarefas da superficie consolidada de `sorafs_cli`.
---

:::nota Fuente canónica
Esta página espelha `docs/source/sorafs/developer/cli.md`. Mantenha ambas como copias sincronizadas.
:::

A superficie consolidada `sorafs_cli` (fornecida pelo crate `sorafs_car` com o recurso `cli` habilitado) expone cada etapa necesaria para preparar artefatos da SoraFS. Utilice este libro de cocina para ir directamente a flujos de trabajo comunes; Combine con el pipeline de manifest y los runbooks del orquestador para el contexto operacional.

## Cargas útiles de Empacotar

Utilice `car pack` para producir archivos CAR deterministas y planos de trozos. El comando selecciona automáticamente el fragmentador SF-1, a menos que un mango se haya fornecido.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Mango de padrao del triturador: `sorafs.sf1@1.0.0`.
- Entradas de directorio sao percorridas em orden lexicografica para que os checksums fiquem estaveis entre plataformas.
- El resumen JSON incluye resúmenes de la carga útil, metadados por fragmentos y el CID reconocido en el registro y en el orquestador.

## Construir manifiestos

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```- Las opciones `--pin-*` se asignan directamente a los campos `PinPolicy` y `sorafs_manifest::ManifestBuilder`.
- Forneca `--chunk-plan` cuando quiere que la CLI recalcule o digiera SHA3 del fragmento antes de enviarlo; caso contrario ele reutiliza o digiere embutido no resumo.
- Se dice JSON sobre la carga útil Norito para diferencias simples durante las revisiones.

## Assinar manifiesta sem chaves de longa duracao

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Aceita tokens en línea, variaciones de ambiente o fuentes basadas en archivos.
- Adiciona metadados de procedencia (`token_source`, `token_hash_hex`, digest de chunk) sin persistir o JWT bruto, a menos que `--include-token=true`.
- Funciona bien en CI: combine con OIDC y GitHub Actions definiendo `--identity-token-provider=github-actions`.

## Enviar manifiestos para o Torii

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

- Faz decodificacao Norito para alias pruebas y verifica se correspondem ao digest do manifest antes de enviar vía POST ao Torii.
- Vuelva a calcular o digerir SHA3 del fragmento a partir del plano para evitar ataques de divergencia.
- Los resúmenes de respuesta capturan el estado HTTP, los encabezados y las cargas útiles del registro para auditorías posteriores.

## Verificar conteudos de CAR y pruebas

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- Reconstruya un archivo PoR y compare resúmenes de la carga útil con el resumen del manifiesto.
- Captura de contagios e identificadores exigidos ao enviar pruebas de replicación para gobernancia.

## Transmitir telemetria de pruebas

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v2/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```- Emite elementos NDJSON para cada prueba transmitida (desativa o reproducción con `--emit-events=false`).
- Agrega contagios de éxito/fallos, histogramas de latencia y fallos mostrados en el resumen JSON para que los paneles de control puedan trazar resultados sin registros.
- Sai com codigo nao zero quando o gateway reporta falhas ou quando a verificacao PoR local (a través de `--por-root-hex`) rejeita pruebas. Ajuste los límites con `--max-failures` e `--max-verification-failures` para ejecutar el ensayo.
- Soporte PoR hoy; PDP y PoTR reutilizan el mismo sobre cuando se compran SF-13/SF-14.
- `--governance-evidence-dir` grava el resumen renderizado, metadados (marca de tiempo, verso de CLI, URL de puerta de enlace, resumen de manifiesto) y una copia del manifiesto en el directorio fornecido para que los pacotes de gobierno arquivem a evidencia doproof-stream sin repetir a execucao.

## Referencias adicionales

- `docs/source/sorafs_cli.md` - documentacao exaustiva de flags.
- `docs/source/sorafs_proof_streaming.md` - esquema de telemetría de pruebas y plantilla de tablero Grafana.
- `docs/source/sorafs/manifest_pipeline.md` - Mergulho profundo em fragmentación, composición de manifiesto y manejo de CAR.