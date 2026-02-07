---
lang: es
direction: ltr
source: docs/portal/docs/nexus/overview.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: descripción general del nexo
título: Visao general do Sora Nexus
descripción: Resumen de alto nivel de arquitectura de Iroha 3 (Sora Nexus) con apontamentos para los documentos canónicos del mono-repo.
---

Nexus (Iroha 3) estende Iroha 2 com ejecucao multi-lane, espacios de dados escondidos por gobierno y herramientas compartiladas en cada SDK. Esta página espelha o novo resumo `docs/source/nexus_overview.md` no mono-repo para que los lectores del portal comprendan rápidamente como as pecas da arquitetura se encaixam.

## Líneas de liberación

- **Iroha 2** - implantacoes auto-hospedadas para consorcios ou redes privadas.
- **Iroha 3 / Sora Nexus** - una rede publica multi-lane onde operadores registram espacos de dados (DS) e herdam ferramentas compartilhadas degobernanza, liquidacao y observabilidade.
- Ambas líneas compiladas del mismo espacio de trabajo (IVM + cadena de herramientas Kotodama), todas las correcciones de SDK, actualizaciones de ABI y accesorios Norito permanecen portátiles. Operadores bajan el paquete `iroha3-<version>-<os>.tar.zst` para entrar no Nexus; consulte `docs/source/sora_nexus_operator_onboarding.md` para o lista de verificación en tela cheia.

## Bloques de construcción| Componente | Resumen | Puntos del portal |
|-----------|---------|--------------|
| Espacio de dados (DS) | Dominio de ejecución/armazenamento definido para la gobernanza que possui uma ou mais lanes, declara conjuntos de validadores, clase de privacidade y política de taxas + DA. | Veja [Nexus spec](./nexus-spec) para el esquema del manifiesto. |
| Carril | Fragmento determinístico de execucao; emite compromisos que o anel global NPoS ordena. Como clases de carril se incluyen `default_public`, `public_custom`, `private_permissioned` e `hybrid_confidential`. | O [modelo de carril](./nexus-lane-model) captura geométrica, prefijos de armazenamento y retencao. |
| Plano de transicao | Identificadores de marcador de posición, fases de rotación y empacado de perfil doble que acompañan como implantacoes de lane unica evoluem para Nexus. | As [notas de transicao](./nexus-transition-notes) documentam cada fase de migracao. |
| Directorio espacial | Contrato de registro que armazena manifestos + versos de DS. Los operadores reconcilian las entradas del catálogo con este directorio antes de entrar. | El rastreador de diferencias de manifiesto vive en `docs/source/project_tracker/nexus_config_deltas/`. |
| Catálogo de carriles | A secao `[nexus]` de configuración mapeia IDs de lane para alias, políticas de roteamento y limiares de DA. `irohad --sora --config ... --trace-config` imprime o catálogo resuelto para auditorios. | Utilice `docs/source/sora_nexus_operator_onboarding.md` para pasar a CLI. || Roteador de liquidacao | Orquestrador de transferencia XOR que conecta líneas CBDC privadas con líneas de liquidez públicas. | `docs/source/cbdc_lane_playbook.md` Detalles de perillas políticas y puertas de telemetría. |
| Telemetría/SLO | Dashboards + alertas em `dashboards/grafana/nexus_*.json` capturan altura de carriles, backlog de DA, latencia de liquidacao y profundidade da fila de gobernanza. | O [plano de reparación de telemetría](./nexus-telemetry-remediation) paneles de control detallados, alertas y evidencias de auditoría. |

## Instantánea del lanzamiento| Fase | Foco | Criterios de dicha |
|-------|-------|-----------------------|
| N0 - Beta fechada | Registrar gerenciado pelo conselho (`.sora`), manual de incorporación de operadores, catálogo de carriles estáticos. | Manifiestos de DS assinados + traspasos degobernanza ensaiados. |
| N1 - Lanzamiento público | Adiciona sufixos `.nexus`, leiloes, registrador de autoservicio, cabeamento de liquidacao XOR. | Pruebas de sincronización de resolutor/gateway, paneles de reconciliación de cobranca, ejercicios de disputa en mesa. |
| N2 - Expansão | Introduz `.dao`, APIs de revenda, analitica, portal de disputas, scorecards de stewards. | Artefatos de cumplimiento versionados, kit de herramientas de jurado de política en línea, relatorios de transparencia do tesouro. |
| Puerta NX-12/13/14 | Motor de cumplimiento, paneles de telemetría y documentación deben sair juntos antes de pilotos con parceiros. | [Nexus resumen](./nexus-overview) + [Nexus operaciones](./nexus-operations) publicados, tableros ligados, motor de política integrada. |

## Responsabilidades del operador1. **Higiene de configuracao** - mantenha `config/config.toml` sincronizado con el catálogo publicado de carriles y espacios de datos; Archive a Saida `--trace-config` en cada boleto de liberación.
2. **Rastreamento de manifestos** - reconcilia las entradas del catálogo con el paquete más reciente de Space Directory antes de entrar o actualizarnos.
3. **Cobertura de telemetría** - muestra los paneles de control `nexus_lanes.json`, `nexus_settlement.json` y los paneles de SDK relacionados; conecte alertas a PagerDuty y realice revisiones trimestrales conforme al plano de reparación de telemetría.
4. **Relato de incidentes** - siga a matriz de severidade em [Nexus operaciones](./nexus-operations) e entregue RCAs em ate cinco dias uteis.
5. **Prontidao degobernanza** - participe de votos do conselho Nexus que impactam sus carriles e ensaie instrucoes de rollback trimestralmente (rastreadas vía `docs/source/project_tracker/nexus_config_deltas/`).

## Veja también

- Visao canónica: `docs/source/nexus_overview.md`
- Detalle específico: [./nexus-spec](./nexus-spec)
- Geometría de carriles: [./nexus-lane-model](./nexus-lane-model)
- Plano de transición: [./nexus-transition-notes](./nexus-transition-notes)
- Plano de remediacao de telemetria: [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- Runbook de operaciones: [./nexus-operatives](./nexus-operations)
- Guía de incorporación de operadores: `docs/source/sora_nexus_operator_onboarding.md`