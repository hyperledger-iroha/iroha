---
lang: es
direction: ltr
source: docs/portal/docs/nexus/overview.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: descripción general del nexo
título: Apercu de Sora Nexus
descripción: Resume de haut niveau de l'architecture Iroha 3 (Sora Nexus) avec des pointeurs vers les docs canoniques du mono-repo.
---

Nexus (Iroha 3) incluye Iroha 2 con ejecución multicarril, espacios de cuadros de donnees para la gestión y herramientas compartidas en cada SDK. Esta página refleja el nouveau brief `docs/source/nexus_overview.md` dans le mono-repo afin que les lecteurs du portail comprennent rapidement comment les Pieces de l'architecture s'emboitent.

## Líneas de versión

- **Iroha 2** - implementaciones de auto-heberges pour consortiums ou reseaux prives.
- **Iroha 3 / Sora Nexus** - investigación pública de varios carriles o operadores que registran los espacios de donnees (DS) y heredan las herramientas de gobierno, regulación y observabilidad.
- Las dos líneas compiladas después del espacio de trabajo de memes (IVM + cadena de herramientas Kotodama), el SDK de correcciones, las actualizaciones del día ABI y los dispositivos Norito restantes en portátiles. Los operadores telecargarán el archivo `iroha3-<version>-<os>.tar.zst` para volver a unirse a Nexus; Informe a `docs/source/sora_nexus_operator_onboarding.md` para la lista de verificación en pantalla.

## Bloques de construcción| Componente | Currículum | Puntos de venta |
|-----------|---------|--------------|
| Espacio de donnees (DS) | Domaine d'execution/stockage defini par la gouvernance qui posee une ou plusieurs lanes, declara des ensembles de validadores, la clase de confidencialidad y la política de frais + DA. | Consulte [Nexus spec](./nexus-spec) para el esquema del manifiesto. |
| Carril | Fragmento determinante de ejecución; Emet des engagements que l'anneau NPoS global ordonne. Las clases de carril incluyen `default_public`, `public_custom`, `private_permissioned` y `hybrid_confidential`. | El [modelo de carril](./nexus-lane-model) captura la geometría, los prefijos de almacenamiento y la retención. |
| Plan de transición | Los identificadores de marcador de posición, las fases de ruta y un trazo de doble perfil de embalaje comentan las implementaciones mono-lane evoluent vers Nexus. | Les [notas de transición](./nexus-transition-notes) documentan cada fase de migración. |
| Directorio espacial | Contrat registre qui stocke les manifestes + versiones DS. Les operatorurs concilient les entrees du catalog avec ce repertoire avant de rejoindre. | Le suivi des diffs de manifeste vit sous `docs/source/project_tracker/nexus_config_deltas/`. || Catálogo de carriles | La sección de configuración `[nexus]` asigna los ID de carril frente a los alias, políticas de ruta y seuils DA. `irohad --sora --config ... --trace-config` imprime el catálogo resuelto para las auditorías. | Utilice `docs/source/sora_nexus_operator_onboarding.md` para la ruta CLI. |
| Ruta de control | Orquestador de transferencias XOR que conecta las líneas CBDC privadas a las líneas de liquidez pública. | `docs/source/cbdc_lane_playbook.md` detalla las reglas políticas y los controles de telemetría. |
| Telemetría/SLO | Tableaux de bord + alertes sous `dashboards/grafana/nexus_*.json` capturan la hauteur des lanes, le backlog DA, la latence de reglement et la profondeur de la file de gouvernance. | El [plan de remediación de telemetría](./nexus-telemetry-remediation) detalla los cuadros de borde, alertas y preuves de auditoría. |

## Despliegue instantáneo| Fase | Enfoque | Criterios de salida |
|-------|-------|-----------------------|
| N0 - Beta cerrada | Registrar gere par le conseil (`.sora`), manual de operador de incorporación, catálogo de carriles estadísticos. | Manifiestos DS signes + passations de gouvernance repeties. |
| N1 - Lanzamiento público | Agregue los sufijos `.nexus`, les encheres, un registrador en libre servicio, le cablage de reglement XOR. | Pruebas de sincronización resolutor/gateway, cuadros de conciliación de facturación, ejercicios de litigios. |
| N2 - Ampliación | Introducción `.dao`, API revendeurs, analytique, portail de litiges, scorecards de stewards. | Artefactos de versiones conformes, kit de herramientas del jurado de política en línea, informes de transparencia del tresor. |
| Puerta NX-13/12/14 | El motor de conformidad, los paneles de telemetría y la documentación deben clasificarse en conjunto antes de los pilotos partenaires. | [Nexus descripción general](./nexus-overview) + [Nexus operaciones](./nexus-operations) publicaciones, cables de tableros, fusión de motores políticos. |

## Responsabilidades de los operadores1. **Higiene de configuración** - gardez `config/config.toml` sincronizar con el catálogo público de líneas y espacios de datos; archivez la sortie `--trace-config` con cada ticket de liberación.
2. **Suivi des manifestes** - conciliez les entrees du catalog avec le último paquete Space Directory antes de volver a unir o de mettre a niveau les noeuds.
3. **Cobertura de telemetría**: expone los paneles `nexus_lanes.json`, `nexus_settlement.json` y estos se encuentran en el SDK; Cablee las alertas a PagerDuty y realice las revisiones trimestrales según el plan de reparación de telemetría.
4. **Signalement d'incidents** - suivez la matrice de severite dans [Nexus Operations](./nexus-operations) et deposez les RCAs sous cinq jours ouvrables.
5. **Preparación de gobierno**: asista a los votos del consejo Nexus impactando sus carriles y repita las instrucciones de reversión cada trimestre (suivi vía `docs/source/project_tracker/nexus_config_deltas/`).

## Ver también

- Apercu canonique : `docs/source/nexus_overview.md`
- Especificación detallada: [./nexus-spec](./nexus-spec)
- Geometría de los carriles: [./nexus-lane-model](./nexus-lane-model)
- Plan de transición: [./nexus-transition-notes](./nexus-transition-notes)
- Plan de remediación de telemetría: [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- Operaciones de Runbook: [./nexus-operaciones](./nexus-operations)
- Guía del operador de incorporación: `docs/source/sora_nexus_operator_onboarding.md`