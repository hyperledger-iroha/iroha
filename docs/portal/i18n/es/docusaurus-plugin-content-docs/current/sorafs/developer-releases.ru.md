---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Процесс релиза
resumen: Utilice CLI/SDK para acceder a la versión correcta, consulte la versión política y publique notas canónicas.
---

# Процесс релиза

Binarios SoraFS (`sorafs_cli`, `sorafs_fetch`, ayudantes) y cajas SDK
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) выпускаются вместе. Релизный
tubería de CLI y bibliotecas de software, obspechivaet покрытие lint/test
и фиксирует артефакты для aguas abajo потребителей. Выполните checklist ниже для
etiqueta candidata каждого.

## 0. Подтвердить cierre de sesión по безопасности

Antes de la revisión técnica de la puerta de liberación, consulte la revisión de seguridad de varios artefactos:

- Скачайте самый свежий меморандум SF-6 по безопасности ([reports/sf6-security-review](./reports/sf6-security-review.md))
  и зафиксируйте его SHA256 hash в lanzamiento ticket.
- Solicite un ticket de remediación (por ejemplo, `governance/tickets/SF6-SR-2026.md`) y elimine
  aprobar-ответственных из Grupo de trabajo de ingeniería de seguridad y herramientas.
- Asegúrese de que la lista de verificación de remediación esté en el memorándum; незакрытые пункты блокируют релиз.
- Подготовьте загрузку логов paridad arnés (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  вместе с paquete manifiesto.
- Убедитесь, что команда подписи, которую вы планируете выполнить, включает и `--identity-token-provider`, и
  явный `--identity-token-audience=<aud>`, чтобы alcance Fulcio был зафиксирован в релизных evidencia.

Utilice estos artefactos relacionados con la gestión pública y la publicación.## 1. Выполнить puerta de liberación/prueba

El ayudante `ci/check_sorafs_cli_release.sh` elimina el formato, Clippy y pruebas
по CLI y SDK crates en el directorio de destino local del espacio de trabajo (`.target`) que están disponibles
конфликтов прав при запуске внутри CI контейнеров.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

Script выполняет следующие проверки:

- `cargo fmt --all -- --check` (espacio de trabajo)
- `cargo clippy --locked --all-targets` para `sorafs_car` (con la característica `cli`),
  `sorafs_manifest` y `sorafs_chunker`
- `cargo test --locked --all-targets` para estas cajas

Si el cable de descarga se bloquea, no se debe realizar una regresión al etiquetado. Релизные сборки должны
идти непрерывно от principal; No elimine las películas Cherry-pick en Release-ветки. Puerta de entrada
проверяет наличие firma sin llave флагов (`--identity-token-issuer`,
`--identity-token-audience`) там, где требуется; отсутствующие аргументы валят запуск.

## 2. Применить политику версионирования

Otras cajas CLI/SDK SoraFS implementan SemVer:

- `MAJOR`: Вводится с первым релизом 1.0. До 1.0 versión menor `0.y`
  **означает изменения** в CLI Surface o схемах Norito.
  за опциональной политикой, добавления телеметрии).
- `PATCH`: Исправления багов, solo documentación релизы и обновления зависимостей,
  не меняющие наблюдаемое поведение.

Coloque `sorafs_car`, `sorafs_manifest` y `sorafs_chunker` en otras versiones, entre sí.
El SDK descendente puede funcionar principalmente con una cadena de versión nueva. Esta es la versión siguiente:1. Retire el polo `version =` en el cuadro `Cargo.toml`.
2. Conecte `Cargo.lock` a `cargo update -p <crate>@<new-version>` (espacio de trabajo
   требует явных версий).
3. Снова запустите compuerta de liberación, чтобы не осталось устаревших артефактов.

## 3. Подготовить notas de la versión

La versión actual del registro de cambios de Markdown se publica con el acceso a CLI, SDK
y gobernanza. Используйте шаблон `docs/examples/sorafs_release_notes.md` (скопируйте
его в директорию релизных артефактов и заполните секции конкретикой).

Nombre mínimo:

- **Aspectos destacados**: archivos adjuntos para CLI y SDK del usuario.
- **Pasos de actualización**: TL;DR команды для обновления cargo зависимостей и перезапуска
  accesorios de calefacción.
- **Verificación**: хэши или sobres вывода команд и точная ревизия
  `ci/check_sorafs_cli_release.sh`, которая была выполнена.

Aquí están las notas de la versión más populares (por ejemplo, en esta versión de GitHub) y
храните рядом с детерминированно сгенерированными артефактами.

## 4. Выполнить ganchos de liberación

Introduzca `scripts/release_sorafs_cli.sh`, cómo generar el paquete de firmas y
resumen de verificación, которые отгружаются с каждым релизом. Envoltorio при необходимости
собирает CLI, вызывает `sorafs_cli manifest sign` y сразу воспроизводит
`manifest verify-signature`, чтобы сбои проявились до etiquetado. Ejemplo:

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

Подсказки:- Отслеживайте релизные entradas (carga útil, planes, resúmenes, hash de token esperado)
  En los repositorios o en la configuración de implementación, estos scripts están instalados. CI
  paquete под `fixtures/sorafs_manifest/ci_sample/` показывает канонический diseño.
- Стройте CI automatización en `.github/workflows/sorafs-cli-release.yml`; en выполняет
  puerta de liberación, вызывает скрипт выше и архивирует paquetes/firmas como artefactos de flujo de trabajo.
  Повторяйте тот же порядок команд (liberar puerta → firmar → verificar) в других CI системах,
  Estos registros de auditoría se actualizan con hashes de generación.
- Храните `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json` y
  `manifest.verify.summary.json` вместе: este paquete, en una notificación de gobernanza de este tipo.
- Если релиз обновляет accesorios canónicos, скопируйте обновленный manifiesto, plan de fragmentos y
  resúmenes en `fixtures/sorafs_manifest/ci_sample/` (и обновите
  `docs/examples/sorafs_ci_sample/manifest.template.json`) para etiquetar. Operadores aguas abajo
  зависят от закоммиченных accesorios для воспроизводимости paquete de lanzamiento.
- Зафиксируйте лог выполнения проверки-bounded-channel для `sorafs_cli proof stream` и
  Utilice un paquete de seguridad, que puede utilizar para proteger las actividades de transmisión a prueba.
- El texto original `--identity-token-audience`, incluido en las notas de la versión;
  gobernanza сверяет audiencia с политикой Fulcio перед одобрением публикации.Utilice `scripts/sorafs_gateway_self_cert.sh` o envíe una puerta de enlace de implementación.
Siga el paquete de manifiesto y agregue la atestación que corresponde al artefacto candidato:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. Etiquetas y publicaciones

После прохождения controles y завершения ganchos:1. Introduzca `sorafs_cli --version` e `sorafs_fetch --version`, qué datos tienen, qué
   бинарники показывают новую версию.
2. Configure la versión de lanzamiento en `sorafs_release.toml` en la versión del control del módulo (previamente)
   En una configuración de configuración, se eliminan varios repositorios de implementación. Избегайте
   operación permanente ad-hoc; Conecte la entrada a CLI con `--config` (o un análogo)
   чтобы liberan entradas были явными и воспроизводимыми.
3. Создайте подписанный тег (предпочтительно) o аннотированный тег:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. Загрузите артефакты (paquetes CAR, manifiestos, resúmenes de pruebas, notas de la versión,
   resultados de la atestación) en el registro de proyectos, en la lista de verificación de gobernanza en
   [guía de implementación](./developer-deployment.md). Если релиз создал новые accesorios,
   отправьте их в общий repositorio de accesorios o tienda de objetos, чтобы auditoría automatización могла
   сравнить опубликованный paquete с контролем версий.
5. Utilice canales de gobernanza basados en etiquetas actualizadas, notas de la versión y hashes.
   manifiesto de paquete/подписей, resúmenes y libros de arхивированные `manifest.sign/verify`
   sobres de atestación. Приложите URL CI job (o arхив логов), который выполнил
   `ci/check_sorafs_cli_release.sh` y `scripts/release_sorafs_cli.sh`. Обновите la gobernanza
   billete, чтобы аудиторы могли связать aprobaciones с артефактами; cogda
   `.github/workflows/sorafs-cli-release.yml` Publicación completa, связывайте
   hashes personalizados junto con resúmenes ad-hoc.

## 6. Пост-релизные действия- Убедитесь, что документация, указывающая на новую версию (inicios rápidos, plantillas de CI),
  обновлена, либо подтвердите отсутствие изменений.
- Заведите hoja de ruta записи, если нужна последующая работа (por ejemplo, banderas de migración,
- Архивируйте логи вывода liberación de la puerta para los auditores - храните их рядом с подписанными
  artefaktami.

Este canal está conectado a CLI, cajas SDK y materiales de gobernanza sincronizados
в каждом релизном цикле.