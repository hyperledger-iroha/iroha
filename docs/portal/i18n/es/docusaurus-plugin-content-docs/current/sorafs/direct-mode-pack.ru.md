---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/direct-mode-pack.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: paquete de modo directo
título: Пакет отката прямого режима SoraFS (SNNet-5a)
sidebar_label: Paquete reducido
descripción: Configuración actualizada, configuración de configuración y configuración mediante el robot SoraFS en el modo inicial Torii/QUIC en el último momento SNNet-5a.
---

:::nota Канонический источник
:::

Los canales SoraNet instalados en el transporte según la configuración SoraFS no tienen ningún punto de conexión con el sistema de respaldo **SNNet-5a**, чтобы операторы могли сохранять детерминированный доступ на чтение, пока развертывание анонимности завершается. Estos paquetes de parámetros de configuración CLI/SDK, configuraciones de perfiles, archivos de configuración y lista de verificación de configuración y actualizaciones Los robots SoraFS en la configuración Torii/QUIC no están disponibles para el transporte privado.

Las opciones de respaldo para la puesta en escena y la regulación de los productos, ya que SNNet-5–SNNet-9 no utilizan sus puertas de enlace. Храните артефакты ниже вместе с обычными материалами развертывания SoraFS, чтобы операторы могли переключаться между анонимным и прямым режимами по требованию.

## 1. Banderas CLI y SDK- `sorafs_cli fetch --transport-policy=direct-only ...` отключает планирование реле и принудительно использует transporte Torii/QUIC. Presione la tecla CLI para conectar `direct-only` a la estación base.
- SDK para instalar el archivo `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)`, ya que está previamente configurado como "programa". Las vinculaciones genéricas de `iroha::ClientOptions` e `iroha_android` se muestran en la enumeración.
- Gateway-харнесс (`sorafs_fetch`, Python-binding) puede eliminar la versión solo directa de los archivos JSON Norito, чтобы автоматизация получала такое же поведение.

Utilice la bandera en el runbook para los socios y coloque los archivos adjuntos con `iroha_config`, pero no con los permanentes. окружения.

## 2. Puerta de enlace de perfiles políticos

Utilice Norito JSON para configurar la configuración del administrador. Primer perfil en el código `docs/examples/sorafs_direct_mode_policy.json`:

- `transport_policy: "direct_only"` — отклоняет провайдеров, которые рекламируют только транспорты реле SoraNet.
- `max_providers: 2`: los componentes originales están instalados en los terminales Torii/QUIC. Asegúrese de cumplir con los requisitos establecidos en cada región.
- `telemetry_region: "regulated-eu"` — métricas de marcado, tableros de instrumentos y auditorías de respaldo.
- Las conexiones de configuración (`retry_budget: 2`, `provider_failure_threshold: 3`) para esta puerta de enlace no están enmascaradas de forma no segura.Guarde JSON entre `sorafs_cli fetch --config` (automatización) o enlace SDK (`config_from_json`) para el operador de políticas públicas. Сохраняйте вывод marcador (`persist_path`) для аудиторских следов.

Los parámetros de configuración están configurados en las descripciones de la puerta de enlace principal en `docs/examples/sorafs_gateway_direct_mode.toml`. Шаблон отражает вывод `iroha app sorafs gateway direct-mode enable`, отключая проверки sobre/admisión, подключая defaults для rate-limit y заполняя таблицу `direct_mode` хостами из плана и digest значениями manifest. Utilice un marcador de posición para implementar un nuevo plan antes de actualizar la configuración del sistema.

## 3. Набор проверок соответствия

Готовность прямого режима теперь включает покрытие как в оркестраторе, так и в CLI-крейтах:- `direct_only_policy_rejects_soranet_only_providers` garantiza, что `TransportPolicy::DirectOnly` быстро падает, когда каждый кандидат advert поддерживает только реле SoraNet.【crates/sorafs_orchestrator/src/lib.rs:7238】
- `direct_only_policy_prefers_direct_transports_when_available` garantiza la utilización de Torii/QUIC, código de descarga y configuración de SoraNet сессии.【crates/sorafs_orchestrator/src/lib.rs:7285】
- `direct_mode_policy_example_is_valid` en lugar de `docs/examples/sorafs_direct_mode_policy.json`, qué documentos están disponibles en el momento утилитами-хелперами.【crates/sorafs_orchestrator/src/lib.rs:7509】【docs/examples/sorafs_direct_mode_policy.json:1】
- `fetch_command_respects_direct_transports` запускает `sorafs_cli fetch --transport-policy=direct-only` против мокнутого Torii gateway, previa prueba de humo para el registro regular, фиксирующих прямые транспорты.【crates/sorafs_car/tests/sorafs_cli.rs:2733】
- `scripts/sorafs_direct_mode_smoke.sh` implementa el comando JSON y el marcador de programación para la implementación automática.

Utilice estos datos antes de la publicación pública:

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

Si el espacio de trabajo del espacio de trabajo está configurado en sentido ascendente, inserte el bloque de bloqueo en `status.md` y envíe un mensaje de correo electrónico обновления зависимости.

## 4. Programas de humo automáticosОдна лишь проверка CLI не выявляет регрессии, специфичные для окружения (por ejemplo, dos puertas de enlace políticas o manifiestos no deseados). La nueva escritura de humo se incluye en `scripts/sorafs_direct_mode_smoke.sh` y se ejecuta `sorafs_cli fetch` en el programa de registro político del operador político. marcador и сбором resumen.

Primera aplicación:

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```- Pantallas de script y CLI, y clave=valor de configuración (como `docs/examples/sorafs_direct_mode_smoke.conf`). Antes de publicar el manifiesto de resumen y publicar el anuncio, se mostrarán los productos.
- `--policy` sobre la base de datos `docs/examples/sorafs_direct_mode_policy.json`, no puede permitirse el registro JSON, el generador `sorafs_orchestrator::bindings::config_to_json`. La política de CLI para el `--orchestrator-config=PATH`, que obliga a los usuarios a utilizar las banderas más fuertes.
- Если `sorafs_cli` нет в `PATH`, помощник собирает его из крейта `sorafs_orchestrator` (perfil de liberación), чтобы smoke-progonы проверяли поставляемую схему прямого режима.
- Выходные данные:
  - Carga útil sobrante (`--output`, по умолчанию `artifacts/sorafs_direct_mode/payload.bin`).
  - Recuperación de resumen (`--summary`, según la carga útil de la región), televisores regionales actualizados y dispositivos disponibles para la implementación de bases digitales.
  - Cuadro de indicadores, actualizado según las políticas JSON (por ejemplo, `fetch_state/direct_mode_scoreboard.json`). Архивируйте его вместе с resumen в тикетах изменений.- Puerta de adopción automatizada: después de buscar el script de búsqueda `cargo xtask sorafs-adoption-check`, muestra el marcador y el resumen. Требуемый кворум по умолчанию равен числу провайдеров в командной строке; Utilice `--min-providers=<n>` antes de un gran ejemplo. Adopción-отчеты пишутся рядом с resumen (`--adoption-report=<path>` позволяет задать место), а скрипт по умолчанию добавляет `--require-direct-only` (в соответствии с fallback) и `--require-telemetry`, если вы передаете соответствующий флаг. Utilice `XTASK_SORAFS_ADOPTION_FLAGS` para los argumentos anteriores de xtask (por ejemplo, `--allow-single-source` en una degradación actual, чтобы gate и терпел, и respaldo forzado). Utilice la puerta de adopción con `--skip-adoption-check` según el diagnóstico local; Hoja de ruta требует, чтобы каждый регулируемый запуск в прямом режиме включал paquete de adopción-отчета.

## 5. Чек-лист развертывания1. **Configuración de configuración:** agregue el perfil JSON a la lista de repositorios `iroha_config` y cierre los archivos en el ticket. изменения.
2. **Puerta de enlace:** consulte los dispositivos Torii que requieren TLS, capacidad TLV y el registro de auditoría para la configuración en el programa. режим. Publique el perfil político de la puerta de enlace de los operadores.
3. **Cumplimiento de la clasificación:** поделитесь обновленным playbook содобрения на работу вне анонимного оверлея.
4. **Ejecución en seco:** se activan las pruebas de cumplimiento y la recuperación de preparación de las pruebas Torii. Obtenga resultados del marcador y resumen de CLI.
5. **PREключение в прод:** объявите окно изменений, переключите `transport_policy` na `direct_only` (если выбрали `soranet-first`) и следите за дашбордами прямого режима (latencia `sorafs_fetch`, счетчики сбоев провайдеров). Consulte el plan de SoraNet-first después de esto, como SNNet-4/5/5a/5b/6a/7/8/12/13 en `roadmap.md:532`.
6. **Actualización:** utilice el marcador de pantalla, la búsqueda de resumen y la monitorización de resultados de las entradas. Desactive `status.md` con datos de instalación en sistemas y anomalías de líquidos.

Siga esta lista de verificación con el runbook `sorafs_node_ops`, ya que los operadores pueden realizar un proceso de evaluación de la experiencia de usuario. Una vez que SNNet-5 esté en GA, utilice el respaldo después de poder compartir con otros televisores.## 6. Требования к доказательствам и puerta de adopción

Захваты прямого режима по-прежнему должны проходить SF-6c adopt gate. Собирайте marcador, resumen, sobre de manifiesto y adopción-отчет для каждого запуска, чтобы `cargo xtask sorafs-adoption-check` мог проверить fallback-позицию. Si se conecta a una puerta de acceso, el dispositivo se puede conectar a una entrada incorrecta.- **Transporte automático:** `scoreboard.json` para usar `transport_policy="direct_only"` (y peregrinar `transport_policy_override=true`, como para el transporte) degradación). Utilice políticas anónimas que se ajusten a los valores predeterminados y a los proveedores que bloquean o desconectan el dispositivo. плана анонимности.
- **Proveedores de aplicaciones:** Las sesiones de solo puerta de enlace conectan `provider_count=0` y conectan `gateway_provider_count=<n>` con Torii. No utilice JSON: CLI/SDK para los usuarios, una puerta de adopción está disponible para su uso.
- **Manifiesto del fabricante:** Когда участвуют Torii gateways, передайте подписанный `--gateway-manifest-envelope <path>` (или SDK-эквивалент), чтобы `gateway_manifest_provided` y `gateway_manifest_id`/`gateway_manifest_cid` están conectados a `scoreboard.json`. Tenga en cuenta que `summary.json` combina el `manifest_id`/`manifest_cid`; проверка adopción провалится, если любой файл не содержит пару.
- **Ожидания телеметрии:** Когда телеметрия сопровождает захват, запускайте gate с `--require-telemetry`, чтобы отчет подтвердил отправку метрик. Las repeticiones con espacio de aire pueden mostrar la bandera, no CI y los boletos de entrada que no están documentados correctamente.

Ejemplo:

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
```

Utilice `adoption_report.json` como marcador, resumen, sobre de manifiesto y paquete de logotipos de humo. Estos artefactos generan tres trabajos de adopción en CI (`ci/check_sorafs_orchestrator_adoption.sh`) y auditan la degradación de los programas.