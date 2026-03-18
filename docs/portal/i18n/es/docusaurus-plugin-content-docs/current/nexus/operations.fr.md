---
lang: es
direction: ltr
source: docs/portal/docs/nexus/operations.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operaciones-nexus
título: Operaciones de Runbook Nexus
descripción: Resumen operativo previo al terreno del operador de flujo de trabajo Nexus, reflétant `docs/source/nexus_operations.md`.
---

Utilice esta página como compañero de referencia rápida de `docs/source/nexus_operations.md`. Esto resume la lista de verificación operativa, los puntos de control de gestión de cambios y las exigencias de cobertura telefónica que los operadores Nexus deben seguir.

## Lista de verificación del ciclo de vida| Étape | Acciones | Preuves |
|-------|--------|----------|
| Pre-vol | Verifique los hash/firmas de liberación, confirme `profile = "iroha3"` y prepare los modelos de configuración. | Sortie de `scripts/select_release_profile.py`, diario de suma de comprobación, paquete de manifiestos firmado. |
| Alineación del catálogo | Mettre à jour le catalog `[nexus]`, la politique de routage et les seuils DA selon le manifeste émis par le conseil, puis capturer `--trace-config`. | Salida `irohad --sora --config ... --trace-config` almacenada con el billete de embarque. |
| Humo y corte | Lancer `irohad --sora --config ... --trace-config`, ejecuta el humo del CLI (`FindNetworkStatus`), valida las exportaciones de télémétrie y exige la admisión. | Registro de prueba de humo + confirmación Alertmanager. |
| Régimen estable | Paneles de vigilancia/alertas, giran las claves según la cadencia de gobierno y configuraciones/runbooks del sincronizador cuando los manifiestos cambian. | Minutos de revista trimestral, capturas de paneles, ID de tickets de rotación. |

Los detalles de incorporación (reemplazo de claves, modelos de ruta, etapas de perfil de liberación) se encuentran en `docs/source/sora_nexus_operator_onboarding.md`.

## Gestión del cambio1. **Mises à jour de release** - suivre les annonces dans `status.md`/`roadmap.md` ; Únase a la lista de verificación de incorporación a cada PR de liberación.
2. **Cambios de manifiestos de carril**: verifique los paquetes firmados del Directorio espacial y el archivador en `docs/source/project_tracker/nexus_config_deltas/`.
3. **Deltas de configuración**: todo cambio de `config/config.toml` requiere un boleto relacionado con el carril/espacio de datos. Guarde una copia eliminada de la configuración efectiva durante las uniones/actualización de noeuds.
4. **Ejercicios de reversión** - Repita trimestralmente los procedimientos para detener/restaurar/ahumar; consignar los resultados en `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **Aprobaciones conforme**: las líneas privadas/CBDC deben obtener un fuego verde conforme antes de modificar la política DA o las perillas de redacción de télémétrie (ver `docs/source/cbdc_lane_playbook.md`).

## Telemétrie y SLO- Paneles de control: `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, además de vistas específicas del SDK (por ejemplo, `android_operator_console.json`).
- Alertas: `dashboards/alerts/nexus_audit_rules.yml` y reglas de transporte Torii/Norito (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- Métricas de vigilancia :
  - `nexus_lane_height{lane_id}` - alerta de ausencia de progresión en tres ranuras.
  - `nexus_da_backlog_chunks{lane_id}` - alerta au-dessus des seuils par lane (por defecto 64 públicos / 8 privados).
  - `nexus_settlement_latency_seconds{lane_id}` - alerta cuando el P99 pasa de 900 ms (público) o 1200 ms (privado).
  - `torii_request_failures_total{scheme="norito_rpc"}` - alerta si la relación de error a 5 minutos supera el 2%.
  - `telemetry_redaction_override_total` - Sev 2 inmediatamente; Asegure los boletos conformes para las anulaciones.
- Ejecute la lista de verificación de recuperación télémétrie en el [plan de recuperación télémétrie Nexus](./nexus-telemetry-remediation) au moins trimestriellement et joindre le formulaire rempli aux notes de revue opérations.

## Matriz de incidente| Gravitación | Definición | Respuesta |
|----------|------------|----------|
| Septiembre 1 | Brecha de aislamiento del espacio de datos, arresto de asentamiento >15 min, o corrupción del voto de gobierno. | Alerter Nexus Primario + Ingeniería de lanzamiento + Cumplimiento, geler l'admission, recopilador de artefactos, comunicaciones del editor 30 min, lanzamiento del manifiesto échoué. | Alerter Nexus Primary + SRE, atténuer <=4 h, déposer des suivis sous 2 jours ouvrés. |
| Septiembre 3 | Dérive non bloquante (docs, alertas). | Regístrelo en el rastreador y planifique el corrector en el sprint. |

Los tickets de incidente deben registrar los ID de carril/espacio de datos afectados, los hashes de manifiesto, la cronología, las métricas/registros de soporte y las tareas/propietarios de seguimiento.

## Archivo de anteriores

- Stocker bundles/manifestes/exports de télémétrie sous `artifacts/nexus/<lane>/<date>/`.
- Configuraciones del conservador eliminadas + salida `--trace-config` para cada liberación.
- Actas unidas del consejo + decisiones firmadas en caso de cambios de configuración o manifiestos aplicados.
- Conservar las instantáneas Prometheus hebdomadaires des métriques Nexus durante 12 meses.
- Registre las modificaciones del runbook en `docs/source/project_tracker/nexus_config_deltas/README.md` para que los auditores comprendan las responsabilidades sobre el cambio.

## Material lié- Vista del conjunto: [descripción general de Nexus](./nexus-overview)
- Especificación: [Especificación Nexus] (./nexus-spec)
- Géométrie des lanes: [Nexus modelo de carril](./nexus-lane-model)
- Transición y calzas de ruta: [Notas de transición Nexus](./nexus-transition-notes)
- Incorporación de operador: [Incorporación de operador Sora Nexus](./nexus-operator-onboarding)
- Remédiation télémétrie : [Nexus plan de remediación de telemetría](./nexus-telemetry-remediation)