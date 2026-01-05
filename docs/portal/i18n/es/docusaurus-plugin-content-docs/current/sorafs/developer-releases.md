<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
id: developer-releases
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


# Proceso de lanzamiento

Los binarios de SoraFS (`sorafs_cli`, `sorafs_fetch`, helpers) y los crates de SDK
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) se publican juntos. El pipeline
de lanzamiento mantiene el CLI y las bibliotecas alineados, asegura cobertura de
lint/test y captura artefactos para consumidores downstream. Ejecuta la lista de
verificación de abajo para cada tag candidato.

## 0. Confirmar la aprobación de la revisión de seguridad

Antes de ejecutar el gate técnico de lanzamiento, captura los artefactos más
recientes de la revisión de seguridad:

- Descarga el memo más reciente de revisión de seguridad SF-6 ([reports/sf6-security-review](./reports/sf6-security-review.md))
  y registra su hash SHA256 en el ticket de lanzamiento.
- Adjunta el enlace del ticket de remediación (por ejemplo, `governance/tickets/SF6-SR-2026.md`) y anota
  los aprobadores de Security Engineering y del Tooling Working Group.
- Verifica que la lista de remediación del memo esté cerrada; los ítems sin resolver bloquean el lanzamiento.
- Prepárate para subir los logs del harness de paridad (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  junto con el bundle del manifest.
- Confirma que el comando de firma que planeas ejecutar incluya tanto `--identity-token-provider` como
  un `--identity-token-audience=<aud>` explícito para que el alcance de Fulcio quede capturado en la evidencia del release.

Incluye estos artefactos al notificar a gobernanza y publicar el lanzamiento.

## 1. Ejecutar el gate de lanzamiento/pruebas

El helper `ci/check_sorafs_cli_release.sh` ejecuta formateo, Clippy y tests
sobre los crates de CLI y SDK con un directorio target local al workspace (`.target`)
para evitar conflictos de permisos al ejecutarse dentro de contenedores CI.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

El script realiza las siguientes comprobaciones:

- `cargo fmt --all -- --check` (workspace)
- `cargo clippy --locked --all-targets` para `sorafs_car` (con la feature `cli`),
  `sorafs_manifest` y `sorafs_chunker`
- `cargo test --locked --all-targets` para esos mismos crates

Si algún paso falla, corrige la regresión antes de etiquetar. Los builds de release
deben estar continuos con main; no hagas cherry-pick de fixes en ramas de release.
El gate también comprueba que los flags de firma sin claves (`--identity-token-issuer`,
`--identity-token-audience`) se proporcionen donde corresponda; los argumentos
faltantes hacen fallar la ejecución.

## 2. Aplicar la política de versionado

Todos los crates de CLI/SDK de SoraFS usan SemVer:

- `MAJOR`: Se introduce para el primer release 1.0. Antes de 1.0 el incremento
  menor `0.y` **indica cambios con ruptura** en la superficie del CLI o en los
  esquemas Norito.
- `MINOR`: Trabajo de funciones sin ruptura (nuevos comandos/flags, nuevos
  campos Norito protegidos por política opcional, adiciones de telemetría).
- `PATCH`: Correcciones de bugs, releases solo de documentación y actualizaciones de
  dependencias que no cambian el comportamiento observable.

Siempre mantén `sorafs_car`, `sorafs_manifest` y `sorafs_chunker` en la misma versión
para que los consumidores de SDK downstream puedan depender de una única cadena
alineada. Al incrementar versiones:

1. Actualiza los campos `version =` en cada `Cargo.toml`.
2. Regenera el `Cargo.lock` vía `cargo update -p <crate>@<new-version>` (el workspace
   exige versiones explícitas).
3. Ejecuta el gate de lanzamiento otra vez para asegurar que no queden artefactos
   obsoletos.

## 3. Preparar notas de lanzamiento

Cada release debe publicar un changelog en markdown que resalte cambios que
impacten CLI, SDK y gobernanza. Usa la plantilla en
`docs/examples/sorafs_release_notes.md` (cópiala a tu directorio de artefactos de
release y completa las secciones con detalles concretos).

Contenido mínimo:

- **Destacados**: titulares de funciones para consumidores de CLI y SDK.
- **Impacto**: cambios con ruptura, upgrades de políticas, requisitos
  mínimos de gateway/nodo.
- **Pasos de actualización**: comandos TL;DR para actualizar dependencias cargo y
  reejecutar fixtures deterministas.
- **Verificación**: hashes o envoltorios de salida de comandos y la revisión exacta
  de `ci/check_sorafs_cli_release.sh` ejecutada.

Adjunta las notas de lanzamiento completas al tag (por ejemplo, el cuerpo del release
en GitHub) y guárdalas junto a los artefactos generados de forma determinista.

## 4. Ejecutar los hooks de release

Ejecuta `scripts/release_sorafs_cli.sh` para generar el bundle de firmas y el
resumen de verificación que se envía con cada release. El wrapper compila el CLI
cuando es necesario, llama a `sorafs_cli manifest sign` y reproduce de inmediato
`manifest verify-signature` para que los fallos aparezcan antes de etiquetar.
Ejemplo:

```bash
scripts/release_sorafs_cli.sh \
  --manifest artifacts/site.manifest.to \
  --chunk-plan artifacts/site.chunk_plan.json \
  --chunk-summary artifacts/site.car.json \
  --bundle-out artifacts/release/manifest.bundle.json \
  --signature-out artifacts/release/manifest.sig \
  --identity-token-provider=github-actions \
  --identity-token-audience=sorafs-release \
  --expect-token-hash "$(cat .release/token.hash)"
```

Consejos:

- Registra los inputs del release (payload, plans, summaries, hash de token esperado)
  en tu repo o config de despliegue para que el script sea reproducible. El bundle
  de fixtures en `fixtures/sorafs_manifest/ci_sample/` muestra el layout canónico.
- Basa la automatización de CI en `.github/workflows/sorafs-cli-release.yml`; ejecuta
  el gate de release, invoca el script anterior y archiva bundles/firmas como
  artefactos del workflow. Refleja el mismo orden de comandos (gate de release → firmar
  → verificar) en otros sistemas CI para que los logs de auditoría coincidan con los
  hashes generados.
- Mantén juntos `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json`
  y `manifest.verify.summary.json`; forman el paquete referenciado en la notificación
  de gobernanza.
- Cuando el release actualice fixtures canónicos, copia el manifest actualizado,
  el chunk plan y los summaries en `fixtures/sorafs_manifest/ci_sample/` (y actualiza
  `docs/examples/sorafs_ci_sample/manifest.template.json`) antes de etiquetar.
  Los operadores downstream dependen de los fixtures versionados para reproducir
  el bundle de release.
- Captura el log de ejecución de la verificación de canales acotados de
  `sorafs_cli proof stream` y adjúntalo al paquete del release para demostrar que
  las salvaguardas de proof streaming siguen activas.
- Registra el `--identity-token-audience` exacto usado durante la firma en las
  notas de lanzamiento; gobernanza verifica el audience contra la política de Fulcio
  antes de aprobar la publicación.

Usa `scripts/sorafs_gateway_self_cert.sh` cuando el release también incluya un
rollout de gateway. Apunta al mismo bundle de manifest para probar que la
atestación coincide con el artefacto candidato:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. Etiquetar y publicar

Después de que los checks pasen y los hooks se completen:

1. Ejecuta `sorafs_cli --version` y `sorafs_fetch --version` para confirmar que los binarios
   reportan la nueva versión.
2. Prepara la configuración del release en un `sorafs_release.toml` versionado (preferido)
   o en otro archivo de config rastreado por tu repo de despliegue. Evita depender de
   variables de entorno ad-hoc; pasa rutas al CLI con `--config` (o equivalente) para que
   los inputs del release sean explícitos y reproducibles.
3. Crea un tag firmado (preferido) o un tag anotado:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. Sube los artefactos (bundles CAR, manifests, resúmenes de proofs, notas de release,
   outputs de atestación) al registry del proyecto siguiendo el checklist de gobernanza
   en la [guía de despliegue](./developer-deployment.md). Si el release generó nuevos
   fixtures, súbelos al repositorio de fixtures compartido o al object store para que la
   automatización de auditoría pueda comparar el bundle publicado con el control de
   versiones.
5. Notifica al canal de gobernanza con enlaces al tag firmado, notas de release, hashes
   del bundle/firmas del manifest, resúmenes archivados de `manifest.sign/verify` y
   cualquier envoltorio de atestación. Incluye la URL del job CI (o archivo de logs) que
   ejecutó `ci/check_sorafs_cli_release.sh` y `scripts/release_sorafs_cli.sh`. Actualiza
   el ticket de gobernanza para que los auditores puedan trazar las aprobaciones a los
   artefactos; cuando el job `.github/workflows/sorafs-cli-release.yml` publique
   notificaciones, enlaza los hashes registrados en lugar de pegar resúmenes ad-hoc.

## 6. Seguimiento posterior al release

- Asegura que la documentación que apunta a la nueva versión (quickstarts, plantillas de CI)
  esté actualizada o confirma que no se requieren cambios.
- Registra entradas de roadmap si se requiere trabajo posterior (por ejemplo, flags de
- Archiva los logs de salida del gate de release para auditoría: guárdalos junto a los
  artefactos firmados.

Seguir este pipeline mantiene el CLI, los crates del SDK y el material de gobernanza
alineados para cada ciclo de release.
