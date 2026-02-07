---
lang: es
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preguntas frecuentes sobre nexus-settlement
título: Preguntas frecuentes sobre liquidación
descripción: Respostas para operadores cobrindo roteamento de asentamiento, conversación XOR, telemetría e evidencia de auditoría.
---

Esta página espelha o FAQ interno de asentamiento (`docs/source/nexus_settlement_faq.md`) para que los lectores del portal revisen a mesma orientacao sem vasculhar o mono-repo. Explica cómo los procesos de pago de Settlement Router, qué métricas monitorean y cómo los SDK deben integrar las cargas útiles Norito.

## Destaques

1. **Mapeamento de carriles** - cada espacio de datos declara un `settlement_handle` (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody` o `xor_dual_fund`). Consulte el catálogo de carriles más recientes en `docs/source/project_tracker/nexus_config_deltas/`.
2. **Conversao determinística** - El enrutador convierte todos los asentamientos para XOR por meio das fontes de liquidez aprovadas pelagobernanza. Lanes privadas prefinanciam buffers XOR; Los cortes de pelo se aplican cuando los amortiguadores se desvían de la política.
3. **Telemetría** - monitore `nexus_settlement_latency_seconds`, contadores de conversación y medidores de corte de pelo. Paneles ficam en `dashboards/grafana/nexus_settlement.json` y alertas en `dashboards/alerts/nexus_audit_rules.yml`.
4. **Evidencia** - archivar configuraciones, registros del enrutador, exportación de telemetría y relatorios de reconciliación para auditorías.
5. **Responsabilidades del SDK** - cada SDK debe exportar ayudantes de liquidación, ID de carril y codificadores de cargas útiles Norito para mantener la paridad con el enrutador.## Flujos de ejemplo

| Tipo de carril | Pruebas de captura | O que comprova |
|-----------|--------------------|----------------|
| Privada `xor_hosted_custody` | Registro del enrutador + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | Los buffers CBDC deben ser deterministas XOR y los recortes se realizan dentro de la política. |
| Publica `xor_global` | Registro del router + referencia DEX/TWAP + métricas de latencia/conversao | El camino de liquidez compartilhado fixou o preco da transferencia no TWAP publicado con zero haircut. |
| Híbrida `xor_dual_fund` | Iniciar sesión en el enrutador mostrando una divisa pública vs blindada + contadores de telemetría | A mistura blinded/publica respeitou os ratios degobernanza e registrou o haircut aplicado a cada perna. |

## ¿Precisa de más detalles?

- Preguntas frecuentes completas: `docs/source/nexus_settlement_faq.md`
- Específica del enrutador de liquidación: `docs/source/settlement_router.md`
- Manual de estrategia política CBDC: `docs/source/cbdc_lane_playbook.md`
- Runbook de operaciones: [operaciones de Nexus](./nexus-operations)