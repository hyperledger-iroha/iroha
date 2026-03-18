---
lang: es
direction: ltr
source: docs/portal/docs/sns/governance-playbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Канонический источник
Esta página está reemplazada por `docs/source/sns/governance_playbook.md` y un teclado.
служит канонической копией портала. Исходный файл остается для PR переводов.
:::

# Плейбук управления Servicio de nombres Sora (SN-6)

**Estado:** Подготовлен 2026-03-24 — живой справочник для готовности SN-1/SN-6  
**Hoja de ruta:** SN-6 "Cumplimiento y resolución de disputas", SN-7 "Resolver y sincronización de puerta de enlace", direcciones políticas ADDR-1/ADDR-5  
**Предпосылки:** Схема реестра в [`registry-schema.md`](./registry-schema.md), контракт registrador API в [`registrar-api.md`](./registrar-api.md), UX-руководство Dirección en [`address-display-guidelines.md`](./address-display-guidelines.md), y la estructura de la cuenta en [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md).

Estas descripciones de los órganos que mejoran el sistema de servicio de nombres de Sora (SNS)
Cartas, registros de registro completos, esporas de escalamiento y sincronización
состояний solucionador y puerta de enlace. En la hoja de ruta de tres versiones de la CLI
`sns governance ...`, manifiestos Norito y artefactos de auditoría utilizados en el hogar
операторский источник до N1 (публичного запуска).

## 1. Obligación y auditoría

Документ предназначен для:- Consejo de Gobierno de Членов, голосующих по чартеру, политикам суффиксов и
  результатам споров.
- Членов guardian board, которые вводят экстренные заморозки и пересматривают
  откаты.
- Steward по суффиксам, ведущих очереди registrador, утверждающих аукционы и
  управляющих распределением дохода.
- Operador de resolución/puerta de enlace, actualizado para la configuración de SoraDNS, novedad
  Barandillas GAR y telemétricas.
- Команд комплаенса, казначейства и поддержки, которые должны доказать, что
  каждое действие управления оставило аудируемые аудируемые артефакты Norito.

Он охватывает фазы закрытой беты (N0), публичного запуска (N1) y расширения (N2),
перечисленные в `roadmap.md`, связывая каждый flujo de trabajo с необходимыми доказательствами,
дашбордами и путями эскалации.

## 2. Rol y tarjeta de contacto| Роль | Artículos nuevos | Objetos nuevos y telemétricos | Эскалация |
|------|----------------------|----------------------------------|-----------|
| Consejo de Gobierno | Разрабатывает и утверждает charterы, polитики суффиксов, вердикты по спорам и ротации Steward. | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, бюллетени совета, сохраненные через `sns governance charter submit`. | Председатель совета + трекер повестки управления. |
| Junta de Guardianes | Выпускает suave/duro заморозки, экстренные каноны y 72 h обзоры. | Тикеты guardian, создаваемые `sns governance freeze`, override-manifestы в `artifacts/sns/guardian/*`. | Ротация guardián de guardia (<=15 min ACK). |
| Mayordomos de sufijo | Ведут очереди registrador, аукционы, ценовые уровни и коммуникации с клиентами; подтверждают комплаенс. | Políticas del administrador en `SuffixPolicyV1`, listas de novedades, agradecimientos del administrador рядом с регуляторными мемо. | Лид программы steward + PagerDuty по суффиксу. |
| Operaciones de registro y facturación | Consulte los dispositivos `/v1/sns/*`, varias placas, televisores públicos y imágenes prediseñadas CLI. | API de registrador ([`registrar-api.md`](./registrar-api.md)), métricas `sns_registrar_status_total`, placa incorporada en `artifacts/sns/payments/*`. | Gerente de turno, registrador y enlace казначейства. || Operadores de resolución y puerta de enlace | Puede instalar SoraDNS, GAR y configurar la puerta de enlace con el registrador de sincronización; стримят метрики прозрачности. | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | Resolver SRE de guardia + ops мост gateway. |
| Tesorería y Finanzas | Atención primaria 70/30, exclusiones de referencias, etiquetas de nombres/carcasas y certificados de SLA. | Manifestantes de artículos específicos, que incluyen Stripe/казначейства, una configuración de KPI individual en `docs/source/sns/regulatory/`. | Controlador financiero + controlador de compl. |
| Enlace regulatorio y de cumplimiento | Отслеживает глобальные обязательства (EU DSA and др.), обновляет KPI covenants and подает раскрытия. | Notas regulares en `docs/source/sns/regulatory/`, mazos de referencia, notas `ops/drill-log.md` y repetas de mesa. | Лид программы комплаенса. |
| Soporte / SRE de guardia | Обрабатывает инциденты (коллизии, дрейф биллинга, простои resolver), координирует сообщения клиентам and владеет runbook. | Шаблоны инцидентов, `ops/drill-log.md`, laboratorio доказательства, transcripciones Slack/war-room en `incident/`. | Ротация SNS de guardia + SRE менеджмент. |

## 3. Канонические артефакты и источники данных| Artefacto | Расположение | Назначение |
|----------|-------------|-----------|
| Чартер + KPI приложения | `docs/source/sns/governance_addenda/` | Se pueden consultar las normas de control, los convenios de KPI y la mejora de la resolución de acuerdo con la CLI-голоса. |
| Схема реестра | [`registry-schema.md`](./registry-schema.md) | Estructura canónica Norito (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`). |
| Registrador de contactos | [`registrar-api.md`](./registrar-api.md) | Cargas útiles REST/gRPC, parámetros `sns_registrar_status_total` y gancho de gobernanza de software. |
| UX-гайд адресов | [`address-display-guidelines.md`](./address-display-guidelines.md) | Канонические отображения I105 (предпочтительно) и сжатые (второй выбор), используемые кошельками/эксплорерами. |
| Documentos SoraDNS / GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | Детерминированное вычисление host, поток работы transparencia tailer y правила алертов. |
| Nota reglamentaria | `docs/source/sns/regulatory/` | Заметки приема по юрисдикциям (например, EU DSA), administrador de agradecimientos, шаблонные приложения. |
| Registro de perforación | `ops/drill-log.md` | Записи хаос- и IR-репетиций перед выходом из фаз. |
| Хранилище артефактов | `artifacts/sns/` | Placa base, guardián de etiquetas, resolución de diferencias, configuración de KPI y CLI compatible con `sns governance ...`. |

Все действия управления должны ссылаться minimos on оdin артефакт из таблицы
выше, чтобы аудиторы могли восстановить цепочку решений в течение 24 часов.## 4. Плейбуки жизненного цикла

### 4.1 Cartas y azafatas

| Шаг | Владелец | CLI / Доказательства | Примечания |
|-----|----------|----------------------|-----------|
| Estadísticas de Chernovik y delta de KPI | Докладчик совета + лидер mayordomo | Markdown шаблон en `docs/source/sns/governance_addenda/YY/` | Включить ID KPI covenant, ganchos telemétricos y actividades adicionales. |
| Подача предложения | Председатель совета | `sns governance charter submit --input SN-CH-YYYY-NN.md` (создает `CharterMotionV1`) | CLI muestra el manifiesto Norito en `artifacts/sns/governance/<id>/charter_motion.json`. |
| Голосование y reconocimiento de tutor | Совет + guardianes | `sns governance ballot cast --proposal <id>` y `sns governance guardian-ack --proposal <id>` | Utilice protocolos de seguridad y requisitos de seguridad. |
| Принятие mayordomo | Administrador del programa | `sns governance steward-ack --proposal <id> --signature <file>` | Требуется до смены политик суффикса; сохранить конверт в `artifacts/sns/governance/<id>/steward_ack.json`. |
| Activación | Operaciones de registro | Abra `SuffixPolicyV1`, abra el registrador y publique el archivo en `status.md`. | La activación del sello se realiza en `sns_governance_activation_total`. |
| Registro de auditoría | Complementos | Inserte un archivo en `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` y en un registro de perforación, o en un tablero de mesa. | Agregue información sobre paneles telemétricos y diferencias de política. |

### 4.2 Одобрение регистрации, аукциона и цен1. **Comprobación previa:** el registrador запрашивает `SuffixPolicyV1`, чтобы подтвердить ценовой
   уровень, доступные сроки и окна gracia/redención. Поддерживайте ценовые листы
   sincronizar con la tabla de 3/4/5/6-9/10+ (bazovy уровень +
   коэффициенты суффикса), descripción detallada en la hoja de ruta.
2. **Ofertas en sobre cerrado:** Los circuitos premium tienen un compromiso de 72 h / 24 h
   revelar через `sns governance auction commit` / `... reveal`. Опубликуйте список
   commit (только хеши) en `artifacts/sns/auctions/<name>/commit.json`, чтобы
   аудиторы могли проверить случайность.
3. **Proverka платежа:** registrador validado `PaymentProofV1` против распределения
   казначейства (70% tesorería / 30% administrador con exclusión de referencia <=10%). Сохраните
   Norito JSON en `artifacts/sns/payments/<tx>.json` y acceda a su cuenta
   registrador (`RevenueAccrualEventV1`).
4. **Gancho de gobernanza:** Добавьте `GovernanceHookV1` для premium/guarded имен с
   ссылкой на IDs предложений совета и подписи Steward. Отсутствие gancho приводит к
   `sns_err_governance_missing`.
5. **Actividad + resolución de sincronización:** Después de que Torii realice la restauración de sincronización,
   запустите el solucionador de cola de transparencia, чтобы подтвердить распространение нового
   состояния GAR/zona (см. 4.5).
6. **Clientes de registro:** Consultar el libro mayor de clientes (billetera/explorador)
   общие accesorios en [`address-display-guidelines.md`](./address-display-guidelines.md),
   убедившись, что I105 and сжатые отображения совпадают с copy/QAR гайдами.### 4.3 Productos, facturación y servicios de pago- **Proceso de flujo de trabajo:** registrador применяют окно gracia 30 días + окно canje
  60 días, actualizado en `SuffixPolicyV1`. Через 60 дней автоматически запускается
  Reapertura posterior del país (7 días, comisión 10x con уменьшением 15%/día)
  por favor `sns governance reopen`.
- **Распределение доходов:** Каждое продление или трансфер создает
  `RevenueAccrualEventV1`. Выгрузки казначейства (CSV/Parquet) должны
  сопоставляться с этими событиями ежедневно; прикладывайте доказательства в
  `artifacts/sns/treasury/<date>.json`.
- **Exclusiones de referencia:** Необязательные проценты referencia отслеживаются по
  суффиксу через `referral_share` в политике Steward. registrador публикуют итоговое
  распределение и хранят манифесты рядом с доказательством оплаты.
- **Novedades:** Publicaciones financieras basadas en KPI
  (registración, productos, ARPU, использование споров/bond) en
  `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. Дашборды должны опираться на
  те же выгруженные таблицы, чтобы числа Grafana совпадали с доказательствами libro mayor.
- **Observador de KPI:** Ежемесячный обзор:** Чекпоинт первого вторника объединяет финансового лида,
  azafato de turno и programa PM. Seleccionar [panel de KPI de SNS](./kpi-dashboard.md)
  (портальный incrustar `sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json`),
  экспортируйте таблицы rendimiento + registrador de ingresos, зафиксируйте дельты в
  приложении и приложите артефакты к мемо. Запускайте инцидент при обнаруженииSLA нарушений (окна congelación >72 h, всплески ошибок registrador, дрейф ARPU).

### 4.4 Заморозки, споры и апелляции

| Faza | Владелец | Dispositivos y dispositivos electrónicos | Acuerdo de Nivel de Servicio |
|------|----------|----------------------|-----|
| Запрос congelación suave | Mayordomo / поддержка | Coloque el billete `SNS-DF-<id>` en las placas de conexión, en las esporas de bonos y en los selectores de corriente. | <=4 h от поступления. |
| Boleto de guardián | Junta de guardianes | `sns governance freeze --selector <I105> --reason <text> --until <ts>` contiene `GuardianFreezeTicketV1`. Configure JSON en `artifacts/sns/guardian/<id>.json`. | <=30 min ACK, <=2 h выполнение. |
| Ратификация совета | Consejo de gobierno | Para eliminar o bloquear los archivos adjuntos, guarde las cuentas de guardián y digerir las partes de bonos. | Следующее заседание совета или асинхронное голосование. |
| Panel de arbitraje | Комплаенс + mayordomo | Coloque el panel en 7 páginas (hoja de ruta actualizada) con archivos adjuntos que se encuentran en `sns governance dispute ballot`. Pruebe las entradas anónimas de un paquete inesperado. | Вердикт <=7 días después del bono de suscripción. |
| Apelsia | Guardián + совет | Апелляции удваивают bond и повторяют процесс присяжных; записать манифест Norito `DisputeAppealV1` and сослаться на первичный тикет. | <=10 días. |
| Разморозка y ремедиация | Registrador + operaciones de resolución | Abra `sns governance unfreeze --selector <I105> --ticket <id>`, abra el registrador de estado y rastree diff GAR/resolver. | Сразу после вердикта. |Экстренные каноны (заморозки, иницированные guardian <=72 h) следуют тому же
потоку, но требуют ретроспективного обзора совета и заметки о прозрачности в
`docs/source/sns/regulatory/`.

### 4.5 Solución de resolución y puerta de enlace

1. **Gancho de evento:** каждое событие рееестра отправляется в поток событий solucionador
   (`tools/soradns-resolver` SSE). Operaciones de resolución de problemas y diferencias de diferencias
   cola de transparencia (`scripts/telemetry/run_soradns_transparency_tail.sh`).
2. **Обновление GAR шаблона:** gateways должны обновить GAR шаблоны, на которые
   ссылается `canonical_gateway_suffix()`, y переподписать список `host_pattern`.
   Сохранить diff en `artifacts/sns/gar/<date>.patch`.
3. **Archivo de zona de archivo:** Utilice el archivo de zona de esqueleto, descrito en `roadmap.md`
   (nombre, ttl, cid, prueba), y se escribe en Torii/SoraFS. Archivo Norito JSON
   en `artifacts/sns/zonefiles/<name>/<version>.json`.
4. **Proverka прозрачности:** Запустите `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   чтобы убедиться, что алерты зеленые. Texto de escritura Prometheus k
   еженедельному отчету о прозрачности.
5. **Puerta de enlace:** Запишите образцы заголовков `Sora-*` (política de caché, CSP, resumen de GAR)
   и приложите их к журналу управления, чтобы операторы могли доказать, что gateway
   обслужил новое имя с нужными barandillas.

## 5. Telemetría y observación| Señal | Источник | Descripción / Diseño |
|--------|----------|----------------------|
| `sns_registrar_status_total{result,suffix}` | Registrador de registros Torii | Счетчик успех/ошибка для регистраций, продлений, заморозок, трансферов; алерт при росте `result="error"` по суффиксу. |
| `torii_request_duration_seconds{route="/v1/sns/*"}` | Métricas Torii | SLO по латентности для API обработчиков; Está instalado en el tablero de instrumentos según `torii_norito_rpc_observability.json`. |
| `soradns_bundle_proof_age_seconds` y `soradns_bundle_cid_drift_total` | Tailer de transparencia de resolución | Выявляют устаревшие доказательства или дрейф GAR; barandillas instaladas en `dashboards/alerts/soradns_transparency_rules.yml`. |
| `sns_governance_activation_total` | CLI de gobernanza | Счетчик, увеличивающийся при активации чартеров/приложений; используется для сверки решений совета и опубликованных addenda. |
| Calibre `guardian_freeze_active` | CLI de guardián | Seleccione la opción de congelación suave/dura en el selector; Utilice SRE o `1` para obtener SLA. |
| KPI de aplicación de tableros | Finanzas / Documentación | El resumen completo de publicaciones se completa con notas reglamentarias; портал встраивает их через [SNS KPI panel](./kpi-dashboard.md), чтобы azafatas y reguladores видели одинаковый Grafana vid. |

## 6. Требования к доказательствам и аудиту| Действие | Доказательства для архива | Хранилище |
|----------|----------------------|-----------|
| Cartas/políticas de información | Manifestante Norito, transcripción CLI, diferencia de KPI, reconocimiento de administración. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| Registro / productos | Carga útil `RegisterNameRequestV1`, `RevenueAccrualEventV1`, placa de almacenamiento. | `artifacts/sns/payments/<tx>.json`, API de registro de registros. |
| Auxion | Confirmar/revelar манифесты, seed случайности, таблица расчета победителя. | `artifacts/sns/auctions/<name>/`. |
| Заморозка / разморозка | Boleto de tutor, хеш голосования совета, URL инцидент лога, шаблон коммуникации с клиентом. | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| Resolución de problemas | Zonefile/GAR diff, JSONL de cola, nombre Prometheus. | `artifacts/sns/resolver/<date>/` + отчеты о прозрачности. |
| Ingesta regular | Nota de admisión, трекер дедлайнов, administrador de reconocimiento, сводка KPI изменений. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. Чеклист фазовых ворот| Faza | Criterios de valoración | Paquetes de almacenamiento |
|------|------------------|--------------------|
| N0 — Закрытая бета | Схема реестра SN-1/SN-2, ручной registrador CLI, завершенный guardian drill. | Charte motion + steward ACK, registrador de registros de ejecución en seco, отчет о прозрачности resolver, запись в `ops/drill-log.md`. |
| N1 — Публичный запуск | Аукционы + фиксированные ценовые уровни для `.sora`/`.nexus`, registrador de autoservicio, resolución de sincronización automática, tableros de facturación. | Listas de diferencias, registro de CI de resultados, configuración de placas/KPI, seguimiento de transparencia y ajustes de incidencias repetidas. |
| N2 — Расширение | `.dao`, API de revendedor, soportes portátiles, cuadros de mando de administrador, paneles de control analíticos. | Portal de códigos de barras, indicadores de SLA, cuadros de mando de administrador de sitios web, estatutos actualizados con revendedores políticos. |

Выход из фаз требует записанных taladros de mesa (счастливый путь регистрации,
заморозка, solucionador de apagones) с артефактами в `ops/drill-log.md`.

## 8. Реагирование на инциденты и эскалация| Trigger | Уровень | Немедленный владелец | Artículos de diseño |
|---------|---------|----------------------|-----------------------|
| Дрейф resolutor/GAR или устаревшие доказательства | Septiembre 1 | Resolver SRE + tablero guardián | Пейджить resolutor de guardia, собрать вывод tailer, решить вопрос о заморозке затронутых имен, публиковать статус каждые 30 min. |
| Registrador de Avaria, facturación y operaciones masivas de API | Septiembre 1 | Gerente de servicio de registrador | Остановить новые аукционы, перейти на ручной CLI, уведомить stewards/казначейство, приложить логи Torii к и инциденту. |
| Con todos los iconos, placas no deseadas o clientes escalados | Septiembre 2 | Steward + soporte líder | Retire la placa de almacenamiento, elimine la congelación suave no deseada, elimine el almacenamiento en SLA, elimine el resultado en el proceso de recuperación. |
| Замечание комплаенс-аудита | Septiembre 2 | Enlace de cumplimiento | Para realizar un plan de solución, guarde la nota en `docs/source/sns/regulatory/` y realice una nueva sesión. |
| Taladro o repetición | Septiembre 3 | Programa PM | Siga el ejemplo con `ops/drill-log.md`, guarde artefactos y solucione problemas con la hoja de ruta actual. |

Otros incidentes relacionados con las tablas `incident/YYYY-MM-DD-sns-<slug>.md`
владения, журналами команд и ссылками на доказательства, собранные по этому
плейбуку.

## 9. Ссылки-[`registry-schema.md`](./registry-schema.md)
-[`registrar-api.md`](./registrar-api.md)
-[`address-display-guidelines.md`](./address-display-guidelines.md)
-[`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
-[`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
- `ops/drill-log.md`
- `roadmap.md` (SNS, DG, ADDR разделы)

Haga clic aquí para descargar este archivo de texto actual, CLI поверхностей
или контрактов телеметрии; hoja de ruta de elementos, ссылающиеся на
`docs/source/sns/governance_playbook.md`, una nueva versión del sistema operativo
последней редакции.