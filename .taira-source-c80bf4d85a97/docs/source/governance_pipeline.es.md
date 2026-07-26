---
lang: es
direction: ltr
source: docs/source/governance_pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f9f765fbe3170f654a9c44c3cd1afc5d82a72ff49137f32b98cf9d310faf114e
source_last_modified: "2026-01-03T18:08:00.913452+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Gobernanza en trámite (Iroha 2 y SORA Parlamento)

# Estado actual (v1)
- Las propuestas de gobernanza se ejecutan como: proponente → referéndum → recuento → promulgación. Los períodos de referéndum y los umbrales de participación/aprobación se aplican como se describe en `gov.md`; los bloqueos son de solo extensión y se desbloquean al vencimiento.
- La selección del Parlamento utiliza sorteos basados ​​en VRF con ordenamiento y límites de mandatos deterministas; cuando no existe una lista persistente, Torii deriva un respaldo usando la configuración `gov.parliament_*`. Las verificaciones de quórum y control del consejo se realizan en las pruebas `gov_parliament_bodies` / `gov_pipeline_sla`.
- Modos de votación: ZK (predeterminado, requiere `Active` VK con bytes en línea) y Simple (peso cuadrático). Se rechazan las discrepancias de modo; La creación/extensión de bloqueo es monótona en ambos modos con pruebas de regresión para ZK y nuevas votaciones simples.
- La mala conducta del validador se actúa a través del canal de evidencia (`/v1/sumeragi/evidence*`, ayudantes de CLI) con transferencias de consenso conjunto aplicadas por `NextMode` + `ModeActivationHeight`.
- Los espacios de nombres protegidos, los enlaces de actualización del tiempo de ejecución y la admisión del manifiesto de gobernanza están documentados en `governance_api.md` y cubiertos por telemetría (`governance_manifest_*`, `governance_protected_namespace_total`).

# En vuelo/atrasos
- Publicar artefactos del sorteo de VRF (semillas, pruebas, lista ordenada, suplentes) y codificar reglas de reemplazo para personas que no se presentan; Suma los apliques dorados para el sorteo y los reemplazos.
- La aplicación del Stage-SLA para los órganos del Parlamento (reglas → agenda → estudio → revisión → jurado → promulgar) necesita temporizadores explícitos, rutas de escalada y contadores de telemetría.
- La votación de secreto de jurado/compromiso-revelación de políticas y las auditorías de resistencia al soborno asociadas aún deben implementarse.
- Los multiplicadores de vínculo de roles, la reducción de mala conducta para cuerpos de alto riesgo y los tiempos de reutilización entre espacios de servicio requieren plomería de configuración y pruebas.
- El sellado de carriles de gobernanza y las ventanas/puertas de participación para referendos se rastrean en `gov.md`/`status.md`; mantenga actualizadas las entradas de la hoja de ruta a medida que se realicen las pruebas de aceptación restantes.