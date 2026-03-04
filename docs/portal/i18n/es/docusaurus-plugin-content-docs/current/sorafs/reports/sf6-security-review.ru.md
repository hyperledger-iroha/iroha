---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/sf6-security-review.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Отчет по безопасности SF-6
resumen: Recursos y dispositivos de configuración necesarios para la firma sin llave, la transmisión de pruebas y los manifiestos de pago.
---

# Отчет по безопасности SF-6

**Okno оценки:** 2026-02-10 → 2026-02-18  
**Лиды проверки:** Gremio de ingeniería de seguridad (`@sec-eng`), Grupo de trabajo de herramientas (`@tooling-wg`)  
**Blastь:** SoraFS CLI/SDK (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`), API de transmisión de prueba, manifiestos de actualización en Torii, integración Sigstore/OIDC, ganchos de liberación CI.  
**Artículos:**  
- CLI y pruebas de dispositivos (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Manifiesto/prueba de manejadores Torii (`crates/iroha_torii/src/sorafs/api.rs`)  
- Liberación de automatización (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- Arnés de paridad determinista (`crates/sorafs_car/tests/sorafs_cli.rs`, [Отчет о паритете GA SoraFS Orchestrator](./orchestrator-ga-parity.md))

## Metodología1. **Talleres de modelado de amenazas** отразили возможности атакующих для рабочих станций разработчиков, CI систем и Torii узлов.  
2. **Revisión de código** сфокусировался на поверхностях учетных данных (обмен токенами OIDC, firma sin llave), validaciones de manifiestos Norito y contrapresión в transmisión de prueba.  
3. **Pruebas dinámicas** manifiestos de dispositivos integrados y pruebas simuladas (repetición de tokens, manipulación de manifiestos, transmisiones a prueba de uso) con un arnés de paridad mejorado y unidades fuzz especiales.  
4. **Inspección de configuración** muestra los valores predeterminados `iroha_config`, muestra los indicadores CLI y los scripts de lanzamiento, verifica los parámetros y los programas de audio.  
5. **Entrevista de proceso** incluye el flujo de remediación, las rutas de escalamiento y la evidencia de auditoría de los propietarios de la versión Tooling WG.

## Сводка находок| identificación | Gravedad | Área | Encontrar | Resolución |
|----|----------|------|---------|------------|
| SF6-SR-01 | Alto | Firma sin llave | Los tokens de auditoría predeterminados OIDC no están disponibles en las plantillas de CI, lo que aumenta el riesgo de reproducción entre inquilinos. | Добавлено явное требование `--identity-token-audience` en ganchos de liberación y plantillas de CI ([proceso de liberación](../developer-releases.md), `docs/examples/sorafs_ci.md`). CI теперь падает, если аудитория не указана. |
| SF6-SR-02 | Medio | Transmisión de prueba | Rutas de contrapresión принимали неограниченные буферы подписчиков, что позволяло исчерпать память. | `sorafs_cli proof stream` configura los canales de los canales con el truncamiento determinado, registra los resúmenes Norito y cancela el proceso; Torii espejo activado, que organiza fragmentos de respuesta (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | Medio | Отправка manifiestos | La CLI muestra manifiestos basados ​​en planes de fragmentos integrados, como `--plan`. | `sorafs_cli manifest submit` revisa y revisa los resúmenes de CAR, no incluye `--expect-plan-digest`, elimina discrepancias y sugiere sugerencias de corrección. Тесты покрывают успех/ошибки (`crates/sorafs_car/tests/sorafs_cli.rs`). |
| SF6-SR-04 | Bajo | Pista de auditoría | La lista de verificación de liberación no debe incluirse en la revisión de seguridad general. | Добавлен раздел [proceso de lanzamiento](../developer-releases.md), требующий приложить revisión de memorando de hashes y cierre de sesión de URL antes de GA. |Todas las condiciones altas/medias se aplican en un arnés de paridad de acuerdo con los requisitos actuales. Los problemas críticos no se solucionan.

## Control de validez

- **Alcance de la credencial:** Las plantillas de CI incluyen la audiencia y el emisor; CLI y Release Helper están disponibles, y `--identity-token-audience` no está conectado a `--identity-token-provider`.  
- **Repetición determinista:** Pruebas no coincidentes con manifiestos, garantías y resúmenes no coincidentes. недетерминированными ошибками и выявляются до обращения к сети.  
- **Prueba de contrapresión de transmisión:** Torii controla los elementos PoR/PoTR de los canales específicos, y la CLI permite usar latencia de muestras + usar los primeros controles, предотвращая неограниченный рост подписчиков и сохраняя детерминированные resúmenes.  
- **Observabilidad:** Transmisión de prueba de secuencias (`torii_sorafs_proof_stream_*`) y resúmenes de CLI que permiten cancelar, y el operador audita las rutas de navegación.  
- **Documentación:** Гайды для разработчиков ([índice de desarrollador](../developer-index.md), [referencia CLI](../developer-cli.md)) ofrece flujos de trabajo de escalamiento y banderas sensibles a la seguridad.

## Дополнения к lista de verificación de liberación

Administradores de versiones **обязаны** приложить следующие доказательства при продвижении GA кандидата:1. Revisión de seguridad de notas de hash последнего (este documento).  
2. Ссылка на тикет remediación (por ejemplo, `governance/tickets/SF6-SR-2026.md`).  
3. Salida `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` с явными аргументами audiencia/emisor.  
4. Arnés de paridad lógica (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`).  
5. Por favor, vea las notas de la versión Torii que incluyen contadores de telemetría en streaming de pruebas limitadas.

Los artefactos no utilizados están bloqueando la aprobación de GA.

**Hashes de referencia артефактов (aprobación 2026-02-20):**

- `sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## Seguimientos de Оставшиеся

- **Actualización del modelo de amenazas:** Повторять эту ревизию ежеквартально или перед крупными добавлениями флагов CLI.  
- **Cobertura de fuzzing:** Transporte de códigos de transmisión de prueba fuzz'ятся через `fuzz/proof_stream_transport`, identidad de carga útil de охватывая, gzip, deflate y zstd.  
- **Ensayo del incidente:** Запланировать операторское упражнение, симулирующее компрометацию токена and rollback manifest, чтобы документация отражала отработанные процедуры.

## Aprobación

- Previo Gremio de Ingeniería de Seguridad: @sec-eng (2026-02-20)  
- Grupo de trabajo de herramientas anterior: @tooling-wg (2026-02-20)

Храните подписанные aprobaciones вместе с lanzamiento de paquete de artefactos.