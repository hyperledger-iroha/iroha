---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: incorporación-de-operador-nexus
título: Integración del operador del espacio de datos Sora Nexus
descripción: Зеркало `docs/source/sora_nexus_operator_onboarding.md`, lista de verificación de extremo a extremo para los operadores Nexus.
---

:::nota Канонический источник
Esta página está escrita `docs/source/sora_nexus_operator_onboarding.md`. Deje copias sincronizadas, pero no las coloque en el portal.
:::

# Incorporación del operador de espacio de datos Sora Nexus

Esta operación de extremo a extremo está diseñada por el operador de espacio de datos Sora Nexus después de la resolución. En el runbook de doble pista (`docs/source/release_dual_track_runbook.md`) y en el archivo de artefactos (`docs/source/release_artifact_selection.md`), se muestran cómo descargar paquetes/imágenes, manifiestos y configuraciones de archivos globales disponibles en los carriles del usuario en línea.

## Auditoría y predposылки
- El programa nuevo Nexus y las políticas de espacio de datos (índice de carril, ID/alias del espacio de datos y tres políticas de enrutamiento).
- У вас есть доступ к подписанным lanzamiento de artefactos de Release Engineering (tarballs, imágenes, manifiestos, firmas, claves públicas).
- Вы сгенерировали или получили продакшн ключевой материал для роли validator/observer (Ed25519 идентичность узла; BLS консенсусный ключ + PoP para validadores; además, alterna funciones de confianza).
- Si puede descargar los pares de Sora Nexus, use bootstrap.## Paso 1 - Подтвердить профиль релиза
1. Seleccione conjuntos de alias o ID de cadena, según los nombres de los usuarios.
2. Introduzca `scripts/select_release_profile.py --network <alias>` (o `--chain-id <id>`) en este repositorio. El asistente utiliza `release/network_profiles.toml` y completa el perfil de implementación. Para Sora Nexus ответ должен быть `iroha3`. Todos los productos que se utilizan en el software se instalan y se utilizan en Ingeniería de lanzamiento.
3. Utilice la versión de la etiqueta para una versión anónima (nombre `iroha3-v3.2.0`); он понадобится для загрузки artefactos y manifiestos.

## Шаг 2 - Получить и проверить артефакты
1. Descargue el paquete `iroha3` (`<profile>-<version>-<os>.tar.zst`) y sus archivos adjuntos (`.sha256`, opcionalmente `.sig/.pub`, `<profile>-<version>-manifest.json`, y `<profile>-<version>-image.json`, o si desea implementar contenedores).
2. Проверьте целостность перед распаковкой:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   Introduzca `openssl` en un verificador, una organización independiente o un aparato KMS implementado.
3. Produzca `PROFILE.toml` en archivos tarball y manifiestos JSON, para saber:
   - `profile = "iroha3"`
   - Los polos `version`, `commit`, `built_at` se actualizan con una resolución anónima.
   - OS/arquitecto соответствуют цели deploya.
4. Al utilizar la imagen del contenedor, introduzca el hash/firma del archivo `<profile>-<version>-<os>-image.tar` y coloque el ID de la imagen en `<profile>-<version>-image.json`.## Paso 3 - Configuraciones de configuración de sablenov
1. Haga clic en el paquete y explore `config/` para que pueda usar esta configuración.
2. Utilice el archivo `config/` para cambiarlo:
   - Introduzca `public_key`/`private_key` en las teclas del producto Ed25519. Utilice llaves de disco privadas o utilice un HSM; Si desea configurar la configuración, deberá instalar el conector HSM.
   - Configure las etiquetas `trusted_peers`, `network.address` y `torii.address`, todas las opciones disponibles en interfaces de usuario y bootstrap. compañeros.
   - Mostrar `client.toml` con el terminal Torii orientado al operador (configuración TLS para nuevos usuarios) y otros datos операционного herramientas.
3. Сохраняйте ID de cadena из paquete, если только Gobernanza явно не указала иначе - carril global ожидает единый канонический identificador de cadena.
4. Planifique el uso del código con el perfil de bandera Sora: `irohad --sora --config <path>`. El cargador de configuración está desconectado de la configuración SoraFS o de varios carriles o de una bandera desactivada.## Paso 4 - Cómo configurar el espacio de datos y el enrutamiento
1. Отредактируйте `config/config.toml`, чтобы раздел `[nexus]` соответствовал каталогу data-space, выданному Nexus Consejo:
   - `lane_count` должен равняться общему числу carriles, включенных в текущей эпохе.
   - Каждая запись в `[[nexus.lane_catalog]]` и `[[nexus.dataspace_catalog]]` должна содержать уникальный `index`/`id` и согласованные alias. No utilices datos globales; добавьте делегированные alias, если совет назначил дополнительные espacios de datos.
   - Убедитесь, что каждая запись dataspace включает `fault_tolerance (f)`; комитеты lane-relay имеют размер `3f+1`.
2. Обновите `[[nexus.routing_policy.rules]]`, чтобы отразить выданную политику. Шаблон по умолчанию направляет gobernabilidad de las instrucciones en el carril `1` y la implementación de contratos en el carril `2`; Puede utilizar esta función, el tráfico de un espacio de datos en el carril y alias correspondientes. Utilice Release Engineering antes de realizar la actualización.
3. Pruebe los valores `[nexus.da]`, `[nexus.da.audit]` y `[nexus.da.recovery]`. Ожидается, что операторы сохраняют значения, одобренные советом; изменяйте их только при ратификации обновленной политики.
4. Realice la configuración final en el modo operativo. El runbook de doble pista utiliza el efecto `config.toml` (con secretos de redacción) en el ticket integrado.## Paso 5 - Validación previa
1. Asegúrese de configurar todas las configuraciones del validador antes de las siguientes configuraciones:
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   Команда выводит разрешенную конфигурацию и падает рано, если записи каталога/маршрутизации неконсистентны и ли genesis и config расходятся.
2. Cuando implemente contenedores, utilice el comando `docker load -i <profile>-<version>-<os>-image.tar` (no almacenado) `--sora`).
3. Pruebe el logotipo de configuración del identificador de marcador de posición de carril/espacio de datos. Entonces, consulte el Paso 4: la implementación del producto no debe incluir ID de marcador de posición ni etiquetas.
4. Utilice un escenario de humo local (por ejemplo, active `FindNetworkStatus` en lugar de `iroha_cli`, proteja los puntos finales de telemetría publique `nexus_lane_state_total` y consulte las claves de transmisión disponibles en dispositivos móviles o importados).## Paso 6 - Transición y transferencia
1. Сохраните проверенный `manifest.json` и firma артефакты в релизном тикете, чтобы аудиторы могли воспроизвести проверки.
2. Уведомите Nexus Operations, что узел готов к вводу; включите:
   - Идентичность узла (ID de par, nombres de host, punto final Torii).
   - Эффективные значения каталога lane/data-space y política de enrutamiento.
   - Хэши проверенных бинарников/образов.
3. Скоординируйте финальный прием peers (gossip seeds и назначение lane) с `@nexus-core`. Ne подключайтесь к сети, пока не получите одобрение; Sora Nexus требует детерминированной carriles de ocupación y обновленного manifiesto de admisiones.
4. Después de cambiar el uso de runbooks con varias anulaciones y eliminar etiquetas, cómo iniciar iteraciones de inicio Esta línea de base.

## Справочный чеклист
- [ ] Perfil de salida según `iroha3`.
- [ ] Хэши и подписи paquete/imagen проверены.
- [ ] Claves, direcciones de pares y puntos finales Torii disponibles para el producto.
- [] Catálogo de carriles/espacio de datos y política de enrutamiento Nexus соответствуют назначению совета.
- [ ] La configuración del validador (`irohad --sora --config ... --trace-config`) se realiza sin previo aviso.
- [ ] Manifiestos/firmas заархивированы в тикете онбординга и Ops уведомлен.

Для более широкого контекста о фазах миграции Nexus y ожиданиях по телеметрии см. [Notas de transición Nexus](./nexus-transition-notes).