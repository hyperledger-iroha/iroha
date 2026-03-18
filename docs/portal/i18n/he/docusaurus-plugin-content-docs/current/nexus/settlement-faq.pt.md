---
lang: he
direction: rtl
source: docs/portal/docs/nexus/settlement-faq.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-settlement-faq
כותרת: שאלות נפוצות על הסדר
תיאור: Repostas para operations cobrindo roteamento de settlement, conversao XOR, telemetria evidencia de auditoria.
---

Esta page espelha o FAQ interno de settlement (`docs/source/nexus_settlement_faq.md`) עבור que os leitores do revisem Portal a mesma orientacao Sem vasculhar o mono-repo. הסבר כמו או נתב התיישבות מעבד עמודים, מעקב מדדי דומה ו-SDKs מפתחים מטענים משולבים של מערכת הפעלה Norito.

## Destaques

1. **Mapeamento de lanes** - cada dataspace declara um `settlement_handle` (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody` או `xor_dual_fund`). התייעץ עם קטלוג הנתיבים החדשים ב-`docs/source/project_tracker/nexus_config_deltas/`.
2. **Conversao deterministica** - או נתב להמיר todos os settlements para XOR por meio das fontes de liquidez aprovadas pela governanca. מאגרים פרטיים פרטיים של נתיבים XOR; תספורות אז se aplicam quando os buffers desviam da politica.
3. **טלמטריה** - מוניטור `nexus_settlement_latency_seconds`, תספורת ותספורת. לוחות מחוונים ficam em `dashboards/grafana/nexus_settlement.json` e alertas em `dashboards/alerts/nexus_audit_rules.yml`.
4. **Evidencia** - ארכיון תצורות, יומנים לנתב, Exportacoes de telemetria e relatorios de reconciliacao para auditorias.
5. **Responsabilidades do SDK** - cada SDK deve expor helpers de settlement, IDs de lane e codificadores de payloads Norito para manter paridade com o router.

## Fluxos de exemplo

| טיפו דה ליין | Evidencia a capturar | O que comprova |
|-----------|------------------------|----------------|
| Privada `xor_hosted_custody` | לוג לעשות נתב + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | Buffers CBDC debitam XOR deterministica e haircuts ficam dentro da politica. |
| Publica `xor_global` | התחבר לנתב + הפניה ל-DEX/TWAP + מדדי עכשווי/התקשרות | O caminho de liquidez compartilhado fixou o preco da transferencia no TWAP publicado com a sero hair. |
| Hibrida `xor_dual_fund` | התחבר לעשות נתב mostrando a divisao publico vs shielded + contadores de telemetria | A Mistura shielded/publica respeitou os ratios de governanca e registrou o תספורת aplicado a cada perna. |

## פרטים נוספים?

- שאלות נפוצות מלאות: `docs/source/nexus_settlement_faq.md`
- נתב יישוב Especificacao do: `docs/source/settlement_router.md`
- Playbook de politica CBDC: `docs/source/cbdc_lane_playbook.md`
- ספר אופרות: [Operacoes do Nexus](./nexus-operations)