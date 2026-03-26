---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: desarrollador-cli
título: Recettes CLI SoraFS
sidebar_label: CLI de recetas
descripción: Recorridos orientados a las superficies consolidadas `sorafs_cli`.
---

:::nota Fuente canónica
:::

La superficie consolidada `sorafs_cli` (fournie par le crate `sorafs_car` con la característica `cli` activada) expone cada etapa necesaria para preparar los artefactos SoraFS. Utilice este libro de cocina para dirigirse a los flujos de trabajo actuales; Associez-le au pipeline de manifest et aux runbooks de l'orchestrateur pour le contexte opérationnel.

## Empaquetar las cargas útiles

Utilice `car pack` para producir archivos CAR determinados y planes de fragmentos. El comando selecciona automáticamente el fragmentador SF-1 para soltarlo si se coloca un mango.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Mango del triturador por defecto: `sorafs.sf1@1.0.0`.
- Les entrées de répertoire sont parcourues en ordre lexicographique afin que les checksums restent stables entre plateformes.
- El currículum JSON incluye los resúmenes de carga útil, los metadones por fragmentos y el CID racine reconnu por el registro y el orquestador.

## Construir los manifiestos

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```- Las opciones `--pin-*` están asignadas directamente a los campeones `PinPolicy` y `sorafs_manifest::ManifestBuilder`.
- Fournissez `--chunk-plan` cuando desee que la CLI recalcule el resumen SHA3 del fragmento antes de la entrega; Sinon il réutilise le digest intégré au résumé.
- La salida JSON refleja la carga útil Norito para las diferencias simples en las revistas.

## Signer les manifests sans clés de longue durée

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Acepta tokens en línea, variables de entorno o fuentes basadas en archivos.
- Ajoute des métadonnées de provenance (`token_source`, `token_hash_hex`, resumen de fragmentos) sin persistir le JWT brut sauf si `--include-token=true`.
- Funciona bien en CI: combina con OIDC de GitHub Actions y define `--identity-token-provider=github-actions`.

## Soumettre les manifests à Torii

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority <katakana-i105-account-id> \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- Efectúe la decodificación Norito de las pruebas de alias y verifique qué corresponden al resumen del manifiesto antes de la versión POST Torii.
- Vuelva a calcular el resumen SHA3 de fragmentos a partir del plan para evitar ataques por falta de coincidencia.
- Los currículums de respuesta capturan el estado HTTP, los encabezados y las cargas útiles del registro para una auditoría final.

## Verificador del contenido CAR y pruebas

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```- Reconstruir el árbol PoR y comparar los resúmenes de carga útil con el currículum del manifiesto.
- Capture los décomptes e identificadores necesarios para la entrega de pruebas de réplica a la gobernanza.

## Difusor la télémétrie des pruebas

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

- Active los elementos NDJSON para cada prueba transmitida (desactive la reproducción con `--emit-events=false`).
- Agrège les décomptes succès/échec, les histogramas de latencia y les échecs échantillonnés dans le currículum JSON para que los tableros puedan rastrear los resultados sin escrutar los registros.
- Salga con un código no nulo cuando la puerta de enlace señale des échecs o la verificación PoR local (a través de `--por-root-hex`) rechace las pruebas. Ajuste las configuraciones con `--max-failures` e `--max-verification-failures` para las ejecuciones de repetición.
- Supporte PoR aujourd'hui; PDP y PoTR recuperarán el mismo sobre con una hoja SF-13/SF-14 en su lugar.
- `--governance-evidence-dir` escribe el currículum vitae, los metadonnées (marca de tiempo, versión de CLI, URL de la puerta de enlace, resumen del manifiesto) y una copia del manifiesto en el repertorio disponible después de que los paquetes de gobierno puedan archivar la parte previa del flujo de prueba sin reiniciar la ejecución.

## Referencias complementarias- `docs/source/sorafs_cli.md` — documentación exhaustiva de banderas.
- `docs/source/sorafs_proof_streaming.md`: esquema de télémétrie de pruebas y plantilla de tablero Grafana.
- `docs/source/sorafs/manifest_pipeline.md` — plongée approfondie dans le fragmentación, la composición de manifiesto y la gestión de CAR.