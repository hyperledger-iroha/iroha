---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-bootstrap-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plan-bootstrap-nexus
título: Bootstrap y наблюдаемость Sora Nexus
descripción: Plan de operación para validadores de armarios de base Nexus antes de los servicios de SoraFS y SoraNet.
---

:::nota Канонический источник
Esta página está escrita `docs/source/soranexus_bootstrap_plan.md`. Deje copias sincronizadas, pero las versiones localizadas no se pueden publicar en el portal.
:::

# Plan Bootstrap y Observabilidad Sora Nexus

## Цели
- Utilice la configuración básica de validadores/navegadores Sora Nexus con claves de gobernanza, API Torii y consenso de monitorización.
- Проверить ключевые сервисы (Torii, consenso, persistencia) до включения piggyback деплоев SoraFS/SoraNet.
- Creación de flujos de trabajo de CI/CD y paneles de control/alertas de observabilidad para cada conjunto.

## Предпосылки
- Claves de gobernanza materiales (multisig совета, claves de comité) disponibles en HSM o Vault.
- Infraestructura básica (clasterios de Kubernetes o dispositivos bare-metal) en regiones primarias/secundarias.
- Configuración de arranque predeterminada (`configs/nexus/bootstrap/*.toml`), configuración de parámetros reales consensuada.## Сетевые окружения
- Utilice la configuración Nexus según las siguientes preferencias:
- **Sora Nexus (mainnet)** - Ajustes de configuración del producto `nexus`, actualización de software y servicios a cuestas SoraFS/SoraNet (ID de cadena `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`).
- **Sora Testus (testnet)** - Configuraciones de puesta en escena precisas `testus`, configuraciones de mainnet personalizadas para pruebas integradas y validaciones previas al lanzamiento (UUID de cadena) `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- Держите отдельные genesis файлы, claves de gobernanza y huellas de infraestructura para каждого окружения. Testus se adapta a sus implementaciones SoraFS/SoraNet antes de su implementación en Nexus.
- Tuberías de CI/CD que se implementarán en Testus, realizarán pruebas de humo automáticas y se promocionarán mucho en Nexus después прохождения проверок.
- Los paquetes de configuración de referencia se conectan a `configs/soranexus/nexus/` (mainnet) y `configs/soranexus/testus/` (testnet), como base de operaciones `config.toml`, `genesis.json` y admisión de catálogos Torii.## Capítulo 1 - Configuración reciente
1. Auditar el contenido de los documentos:
   - `docs/source/nexus/architecture.md` (consenso, diseño Torii).
   - `docs/source/nexus/deployment_checklist.md` (requisitos de infraestructura).
   - `docs/source/nexus/governance_keys.md` (процедуры хранения ключей).
2. Proverite, что genesis файлы (`configs/nexus/genesis/*.json`) соответствуют текущему validadores de listas y apuestas de pesos.
3. Introduzca el conjunto de parámetros:
   - Размер консенсусного комитета и quórum.
   - Интервал блоков / пороги finalidad.
   - Puertos Torii para servicios y certificados TLS.

## Paso 2 - Implementación del grupo bootstrap
1. Подготовить валидаторские узлы:
   - Развернуть `irohad` инстансы (validadores) con volúmenes persistentes.
   - Убедиться, estas reglas de firewall eliminan el consenso y el tráfico Torii.
2. Introduzca el servicio Torii (REST/WebSocket) en el validador de código TLS.
3. Развернуть observador узлы (solo lectura) для дополнительной устойчивости.
4. Abra los scripts de arranque (`scripts/nexus_bootstrap.sh`) para iniciar el proceso de génesis, iniciar un acuerdo y registrar usuarios.
5. Realizar pruebas de humo:
   - Utilice estas transmisiones desde Torii (`iroha_cli tx submit`).
   - Проверить producción/finalidad bloques через телеметрию.
   - Проверить репликацию libro mayor между валидаторами/observador.## Paso 3 - Gobernanza y mejora de las claves
1. Загрузить multisig конфигурацию совета; подтвердить, что gobernancia предложения можно отправлять y ратифицировать.
2. Безопасно хранить claves de consenso/comité; настроить автоматические бэкапы с acceso al registro.
3. Configure el procedimiento de rotación variable (`docs/source/nexus/key_rotation.md`) y proporcione el runbook.

## Paso 4 - Integración CI/CD
1. Настроить tuberías:
   - Creación y publicación de imágenes del validador/Torii (GitHub Actions y GitLab CI).
   - Автоматическая валидация конфигурации (génesis de pelusa, verificación de firmas).
   - Canalizaciones de implementación (Helm/Kustomize) para los centros de puesta en escena y producción.
2. Realizar pruebas de humo en CI (combinar grupos efímeros y eliminar una suite de transferencia canónica).
3. Agregue scripts de reversión para los runbooks no implementados y almacenados.## Paso 5 - Observabilidad y alertas
1. Reduzca el sistema de monitorización (Prometheus + Grafana + Alertmanager) por región.
2. Собирать ключевые метрики:
  - `nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - Logotipos de Loki/ELK para Torii y servicios de consenso.
3. Дашборды:
   - Здоровье консенсуса (высота блоков, finalidad, статус pares).
   - Латентность/ошибки Torii API.
   - Gobernanza транзакции и статусы предложений.
4. Alertas:
   - Остановка производства блоков (>2 intervalos de bloque).
   - Mayor recuento de pares sin quórum.
   - Спайки ошибок Torii.
   - Backlog очереди gobernanza предложений.

## Paso 6 - Validación y traspaso
1. Completar validaciones de extremo a extremo:
   - Отправить propuesta de gobernanza (например изменение параметра).
   - Пропустить через одобрение совета, чтобы убедиться, что проботает la gobernanza de tuberías.
   - Запустить la diferencia de estado del libro mayor para comprobar la coherencia.
2. Documentar el runbook de guardia (respuesta a incidentes, conmutación por error, escalado).
3. Сообщить о готовности командам SoraFS/SoraNet; Después de todo, este piggyback puede implementarse en los usuarios Nexus.## Чеклист реализации
- [ ] Аудит génesis/configuración завершен.
- [ ] Validador y observador según el acuerdo de cada usuario.
- [ ] Claves de gobernanza загружены, propuesta протестирован.
- [ ] работают de tuberías de CI/CD (construcción + implementación + pruebas de humo).
- [] Paneles de observabilidad que se activan con alertas.
- [] Transferencia de documentos realizada por el comando descendente.