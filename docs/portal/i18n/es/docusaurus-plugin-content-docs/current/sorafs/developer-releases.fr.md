---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Proceso de liberación
Resumen: Ejecute la puerta de publicación CLI/SDK, aplique la política de versiones compartidas y publique las notas de publicación canónicas.
---

# Proceso de liberación

Los binarios SoraFS (`sorafs_cli`, `sorafs_fetch`, ayudantes) y las cajas SDK
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) sont livrés ensemble. El oleoducto
de liberación garde le CLI y les bibliothèques alignés, asegúrese de que la cobertura tenga pelusa/prueba
et capture des artefactos para los consumadores aguas abajo. Ejecute la lista de verificación
ci-dessous pour cada etiqueta candidat.

## 0. Confirmar la validación de la revista de seguridad

Avant d'exécuter le gate técnica de liberación, capturez les derniers artefactos de
revista de seguridad :- Téléchargez le mémo de revue sécurité SF-6 le plus récent ([reports/sf6-security-review](./reports/sf6-security-review.md))
  Y registre el hash SHA256 en el ticket de liberación.
- Joignez le lien du ticket de remédiation (par ex. `governance/tickets/SF6-SR-2026.md`) et notez
  los aprobadores de Ingeniería de Seguridad y del Grupo de Trabajo de Herramientas.
- Verifique que la lista de verificación de reparación del memorándum esté cerrada; Los elementos no bloquean la liberación.
- Prepare la carga de registros del arnés de paridad (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  con el paquete de manifiesto.
- Confirme que el comando de firma que cuenta con el ejecutor incluye las hojas `--identity-token-provider` y
  un `--identity-token-audience=<aud>` explícitamente para capturar el alcance de Fulcio en las preuves de liberación.

Incluez ces artefactos lors de la notificación à la gouvernance et de la publicación.

## 1. Ejecutar la puerta de lanzamiento/pruebas

El ayudante `ci/check_sorafs_cli_release.sh` ejecuta el formateo, Clippy y las pruebas.
En las cajas CLI y SDK con un directorio de destino local en el espacio de trabajo (`.target`)
Para evitar conflictos de permisos durante la ejecución en los contenidos CI.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

El script efectúa las afirmaciones siguientes:

- `cargo fmt --all -- --check` (espacio de trabajo)
- `cargo clippy --locked --all-targets` para `sorafs_car` (con la característica `cli`),
  `sorafs_manifest` y `sorafs_chunker`
- `cargo test --locked --all-targets` para estas mismas cajasSi une étape échoue, corrigez la régression avant de tagger. Les builds de lanzamiento
doivent être continúa con main; ne cherry-pickez pas de correctifs dans des sucursales
liberación. La puerta verifica además las banderas de firma sin llave (`--identity-token-issuer`,
`--identity-token-audience`) Sont fournis quand requis ; fuente les arguments manquants
échouer l'exécution.

## 2. Aplicar la política de versiones

Todas las cajas CLI/SDK SoraFS utilizando SemVer:

- `MAJOR`: Introducción para la versión preliminar 1.0. Avant 1.0, el golpe minero `0.y`
  **indique des changements cassants** dans la superficie del CLI o los esquemas Norito.
- `MINOR` : Nuevas funciones (nuevos comandos/banderas, nuevos campeones Norito
  (Derrière une politique optionnelle, ajouts de télémétrie).
- `PATCH`: Correcciones de errores, publicaciones de documentación única y actualizaciones del día
  Dependencias que no modifican el comportamiento observable.

Gardez toujours `sorafs_car`, `sorafs_manifest` e `sorafs_chunker` a la misma versión
Para los desarrolladores SDK downstream pueden depender de una sola cadena de versión.
alineado. Lors des Bumps de la versión:1. Mettez à jour les champs `version =` dans cada `Cargo.toml`.
2. Regénérez le `Cargo.lock` vía `cargo update -p <crate>@<new-version>` (el espacio de trabajo
   imponer des versiones explícitas).
3. Relancez le gate de liberación afin d'éviter les artefactos périmés.

## 3. Preparar las notas de lanzamiento

Cada vez que se libere, publique un registro de cambios y rebaje antes de los cambios.
Impactantes CLI, SDK y gobernanza. Utilice la plantilla en
`docs/examples/sorafs_release_notes.md` (copia en su repertorio de artefactos
Release et remplissez lessections avec des détails concrets).

Contenido mínimo:

- **Aspectos destacados**: títulos de funciones para los expertos CLI y SDK.
- **Compatibilidad**: cambios cassants, actualizaciones de políticas, exigencias mínimas
  puerta de enlace/nœud.
- **Étapes d'upgrade**: comandos TL;DR para actualizar al día las dependencias de carga y
  relancer les fixes déterministes.
- **Verificación**: hashes de clasificación o sobres y revisión exacta de
  `ci/check_sorafs_cli_release.sh` ejecutado.

Únase a las notas de lanzamiento respondidas en la etiqueta (por ejemplo, cuerpo de lanzamiento de GitHub) y
stockez-les à côté des artefactos générés de façon deterministe.

## 4. Ejecutar los ganchos de liberaciónEjecute `scripts/release_sorafs_cli.sh` para generar el paquete de firmas y el
currículum vitae de verificación de libros con cada lanzamiento. El contenedor construye la CLI si
nécessaire, llame `sorafs_cli manifest sign` y rejoue inmediatamente
`manifest verify-signature` para volver a montar los cheques antes de la etiqueta. Ejemplo:

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

Consejos:- Suivez les inputs de release (carga útil, planes, resúmenes, hash de token de asistencia)
  En su repositorio o configuración de implementación para garantizar que el script sea reproducible.
  El paquete CI bajo `fixtures/sorafs_manifest/ci_sample/` montre el diseño canónico.
- Base la automatización CI en `.github/workflows/sorafs-cli-release.yml`; ella ejecuta
  le gate de release, invoque le script ci-dessus y archive bundles/signatures comme
  artefactos de flujo de trabajo. Reproduisez le même ordre de commandes (puerta → firma →
  verificación) en otros sistemas CI para alinear los registros de auditoría con hashes.
- Gardez `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json` y
  `manifest.verify.summary.json` conjunto: ils forment le paquet référencé dans la
  notificación de gobierno.
- Después de la publicación del día de los accesorios canónicos, copie el manifiesto rafraîchi,
  le chunk plan et les resúmenes en `fixtures/sorafs_manifest/ci_sample/` (et mettez
  à jour `docs/examples/sorafs_ci_sample/manifest.template.json`) antes de la etiqueta. les
  Los operadores posteriores dependen de los accesorios comprometidos para reproducir el paquete.
- Capture el registro de ejecución de la verificación de canales delimitados de
  `sorafs_cli proof stream` y únete al paquete de liberación para descubrir que les
  Garde-fous de prueba de transmisión de activos en reposo.
- Notez l'`--identity-token-audience` exacto utilizado lors de la firma en las notas
  de liberación; la gouvernance recoupe l'audience avec la politique Fulcio avant approbation.Utilice `scripts/sorafs_gateway_self_cert.sh` cuando el lanzamiento incluya además un lanzamiento
puerta de enlace. Pointez-le sur le même bundle de manifest pour prouver que l'attestation
corresponde al artefacto candidato :

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. Etiquetador y publicador

Después del paso de los cheques y de la fin de los ganchos:1. Ejecute `sorafs_cli --version` e `sorafs_fetch --version` para confirmar que los binarios
   Reportent versión la nouvelle.
2. Prepare la configuración de lanzamiento en una versión `sorafs_release.toml` (preferida)
   O algún otro archivo de configuración posterior a su repositorio de implementación. Évitez de dependencia
   de variables de entorno ad-hoc; pase los caminos por CLI con `--config` (ou
   équivalent) afin que les inputs soient explicites et reproductibles.
3. Cree una etiqueta firmada (préféré) o una etiqueta anotada:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. Uploadez les artefactos (paquetes CAR, manifiestos, currículums de pruebas, notas de lanzamiento,
   salidas de atestación) frente al registro del proyecto según la lista de verificación de gobierno
   en la [guía de implementación](./developer-deployment.md). Si la lanzan un producto
   de accesorios nuevos, poussez-les vers le repo de accesorios partagé ou l'object store
   Para que la automatización de auditoría pueda comparar el paquete publicado en el control de fuente.
5. Notificar le canal de gouvernance avec les gravámenes vers le tag signé, les notes de liberación,
   los hashes del paquete/firmas del manifiesto, los currículums archivados `manifest.sign/verify`
   et tout sobre de atestación. Incluya la URL del trabajo CI (o el archivo de registros) aquí
   Ejecute `ci/check_sorafs_cli_release.sh` y `scripts/release_sorafs_cli.sh`. Mettezà
   jour le ticket de gouvernance pour que les auditeurs puissent relier les approbationsartefactos auxiliares; lorsque `.github/workflows/sorafs-cli-release.yml` enviado de notificaciones,
   Liez les hashes registrados en lugar de recopilar currículums ad-hoc.

## 6. Suivi después del lanzamiento

- Asegúrese de que la documentación indique la nueva versión (inicios rápidos, plantillas CI)
  est à jour ou confirmez qu'aucun changement n'est requis.
- Créez des entrées de roadmap si un travail de suivi est nécessaire (por ejemplo, banderas de migración,
- Archivar los registros de salida de la puerta de liberación para los auditores: stockez-les à côté des
  artefactos signados.

Supervise este proceso de mantenimiento de la CLI, el SDK de cajas y los elementos de gobierno
alignés à cada ciclo de liberación.