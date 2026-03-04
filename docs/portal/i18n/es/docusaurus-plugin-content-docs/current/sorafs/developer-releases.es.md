---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Proceso de lanzamiento
resumen: Ejecuta la puerta de lanzamiento de CLI/SDK, aplica la política de versionado compartido y publica notas de lanzamiento canónicas.
---

#Proceso de lanzamiento

Los binarios de SoraFS (`sorafs_cli`, `sorafs_fetch`, helpers) y las cajas de SDK
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) se publican juntos. El oleoducto
de lanzamiento mantiene el CLI y las bibliotecas alineadas, asegura la cobertura de
lint/test y captura artefactos para consumidores posteriores. Ejecuta la lista de
verificación de abajo para cada etiqueta candidato.

## 0. Confirmar la aprobación de la revisión de seguridad

Antes de ejecutar el gate técnico de lanzamiento, capture los artefactos más
recientes de la revisión de seguridad:- Descarga el memo más reciente de revisión de seguridad SF-6 ([reports/sf6-security-review](./reports/sf6-security-review.md))
  y registre su hash SHA256 en el ticket de lanzamiento.
- Adjunta el enlace del ticket de remediación (por ejemplo, `governance/tickets/SF6-SR-2026.md`) y anota
  los aprobadores de Security Engineering y del Tooling Working Group.
- Verifica que la lista de remediación del memo esté cerrada; los elementos sin resolver bloquean el lanzamiento.
- Prepárate para subir los registros del arnés de paridad (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  junto con el paquete del manifiesto.
- Confirma que el comando de firma que planeas ejecutar incluye tanto `--identity-token-provider` como
  un `--identity-token-audience=<aud>` explícito para que el alcance de Fulcio quede capturado en la evidencia del lanzamiento.

Incluye estos artefactos al notificar a gobernanza y publicar el lanzamiento.

## 1. Ejecutar la puerta de lanzamiento/pruebas

El ayudante `ci/check_sorafs_cli_release.sh` ejecuta formato, Clippy y pruebas
sobre las cajas de CLI y SDK con un directorio de destino local al espacio de trabajo (`.target`)
para evitar conflictos de permisos al ejecutarse dentro de contenedores CI.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

El guión realiza las siguientes comprobaciones:

- `cargo fmt --all -- --check` (espacio de trabajo)
- `cargo clippy --locked --all-targets` para `sorafs_car` (con la característica `cli`),
  `sorafs_manifest` y `sorafs_chunker`
- `cargo test --locked --all-targets` para esos mismos cajonesSi algún paso falla, corrija la regresión antes de etiquetar. Las compilaciones de lanzamiento
deben estar continuos con main; no hagas cherry-pick de fixes en ramas de liberación.
El gate también comprueba que las banderas de firma sin claves (`--identity-token-issuer`,
`--identity-token-audience`) se proporcionen donde corresponda; los argumentos
faltantes hacen fallar la ejecución.

## 2. Aplicar la política de versionado

Todas las cajas de CLI/SDK de SoraFS usan SemVer:

- `MAJOR`: Se introduce el primer lanzamiento 1.0. Antes de 1.0 el incremento
  menor `0.y` **indica cambios con ruptura** en la superficie del CLI o en los
  esquemas Norito.
- `MINOR`: Trabajo de funciones sin ruptura (nuevos comandos/flags, nuevos
  campos Norito protegidos por política opcional, adiciones de telemetría).
- `PATCH`: Correcciones de errores, lanzamientos solo de documentación y actualizaciones de
  dependencias que no cambian el comportamiento observable.

Siempre mantén `sorafs_car`, `sorafs_manifest` y `sorafs_chunker` en la misma versión
para que los consumidores de SDK downstream puedan depender de una única cadena
alineada. Al incrementar versiones:1. Actualiza los campos `version =` en cada `Cargo.toml`.
2. Regenera el `Cargo.lock` vía `cargo update -p <crate>@<new-version>` (el espacio de trabajo
   versiones explícitas exiges).
3. Ejecuta la puerta de lanzamiento otra vez para asegurar que no queden artefactos
   obsoletos.

## 3. Preparar notas de lanzamiento

Cada lanzamiento debe publicar un registro de cambios en markdown que resalte cambios que
impactan CLI, SDK y gobernanza. Usa la plantilla en
`docs/examples/sorafs_release_notes.md` (cópiala a tu directorio de artefactos de
comunicado y completa las secciones con detalles concretos).

Contenido mínimo:

- **Destacados**: titulares de funciones para consumidores de CLI y SDK.
- **Impacto**: cambios con ruptura, actualizaciones de políticas, requisitos
  mínimos de gateway/nodo.
- **Pasos de actualización**: comandos TL;DR para actualizar dependencias carga y
  reejecutar accesorios deterministas.
- **Verificación**: hashes o envoltorios de salida de comandos y la revisión exacta
  de `ci/check_sorafs_cli_release.sh` ejecutada.

Adjunta las notas de lanzamiento completas al tag (por ejemplo, el cuerpo del lanzamiento
en GitHub) y guárdalas junto a los artefactos generados de forma determinista.

## 4. Ejecutar los ganchos de liberaciónEjecuta `scripts/release_sorafs_cli.sh` para generar el paquete de firmas y el
resumen de verificación que se envía con cada lanzamiento. El contenedor compila el CLI
cuando sea necesario, llama a `sorafs_cli manifest sign` y reproduce de inmediato
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

Consejos:- Registra los inputs del lanzamiento (carga útil, planes, resúmenes, hash de token esperado)
  en tu repositorio o configuración de implementación para que el script sea reproducible. El paquete
  de accesorios en `fixtures/sorafs_manifest/ci_sample/` muestra el diseño canónico.
- Basa la automatización de CI en `.github/workflows/sorafs-cli-release.yml`; ejecuta
  el gate de release, invoca el script anterior y archiva bundles/firmas como
  artefactos del flujo de trabajo. Refleja el mismo orden de comandos (gate de liberación → firmar
  → verificar) en otros sistemas CI para que los logs de auditoría coincidan con los
  hashes generados.
- Mantén juntos `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json`
  y `manifest.verify.summary.json`; forman el paquete referenciado en la notificación
  de gobernanza.
- Cuando el lanzamiento actualiza los accesorios canónicos, copia el manifiesto actualizado,
  el plan fragmentado y los resúmenes en `fixtures/sorafs_manifest/ci_sample/` (y actualiza
  `docs/examples/sorafs_ci_sample/manifest.template.json`) antes de etiquetar.
  Los operadores downstream dependen de los dispositivos versionados para reproducir.
  el paquete de lanzamiento.
- Captura el registro de ejecución de la verificación de canales acotados de
  `sorafs_cli proof stream` y adjuntalo al paquete del release para demostrar que
  las salvaguardas de prueba streaming siguen activas.
- Registra el `--identity-token-audience` exacto usado durante la firma en las
  notas de lanzamiento; Gobernanza verifica la audiencia contra la política de Fulcio.
  antes de aprobar la publicación.Usa `scripts/sorafs_gateway_self_cert.sh` cuando el lanzamiento también incluye un
despliegue de puerta de enlace. Apunta al mismo paquete de manifiesto para probar que la
atestación coincide con el artefacto candidato:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. Etiquetar y publicar

Después de que los cheques pasen y los ganchos se completen:1. Ejecuta `sorafs_cli --version` y `sorafs_fetch --version` para confirmar que los binarios
   reportan la nueva versión.
2. Prepare la configuración del lanzamiento en un `sorafs_release.toml` versionado (preferido)
   o en otro archivo de configuración rastreado por tu repositorio de despliegue. evita dependiente de
   variables de entorno ad-hoc; pasan rutas al CLI con `--config` (o equivalente) para que
   los inputs del release sean explícitos y reproducibles.
3. Crea una etiqueta firmada (preferido) o una etiqueta anotada:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. Sube los artefactos (paquetes CAR, manifiestos, resúmenes de pruebas, notas de liberación,
   salidas de atestación) al registro del proyecto siguiendo el checklist de gobernanza
   en la [guía de implementación](./developer-deployment.md). Si el lanzamiento generó nuevos
   luminarias, súbelos al repositorio de luminarias compartido o al almacén de objetos para que la
   automatización de auditoría pueda comparar el paquete publicado con el control de
   versiones.
5. Notifica al canal de gobernanza con enlaces al tag firmado, notas de liberación, hashes
   del paquete/firmas del manifiesto, resúmenes archivados de `manifest.sign/verify` y
   cualquier envoltorio de atestación. Incluye la URL del CI del trabajo (o archivo de registros) que
   ejecutó `ci/check_sorafs_cli_release.sh` y `scripts/release_sorafs_cli.sh`. Actualiza
   el ticket de gobernanza para que los auditores puedan trazar las aprobaciones a los
   artefactos; cuando el trabajo `.github/workflows/sorafs-cli-release.yml` publiconotificaciones, enlaza los hashes registrados en lugar de pegar resúmenes ad-hoc.

## 6. Seguimiento posterior al lanzamiento

- Asegura que la documentación que apunta a la nueva versión (quickstarts, plantillas de CI)
  Esté actualizado o confirme que no se requieren cambios.
- Registra entradas de roadmap si se requiere trabajo posterior (por ejemplo, flags de
- Archiva los logs de salida del gate de liberación para auditoría: guárdalos junto a los
  artefactos firmados.

Seguir este pipeline mantiene el CLI, las cajas del SDK y el material de gobernanza
alineados para cada ciclo de liberación.