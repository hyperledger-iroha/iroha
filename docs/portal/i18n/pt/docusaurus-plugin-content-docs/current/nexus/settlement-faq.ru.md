---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-settlement-faq
título: FAQ sobre liquidação
description: Ответы для операторов о маршрутизации assentamento, конвертации в XOR, телеметрии и аудиторских доказательствах.
---

Esta página contém perguntas freqüentes sobre liquidação (`docs/source/nexus_settlement_faq.md`), este portal pode ser usado para você A recomendação não é feita em mono-repo. Aqui está como o Settlement Router está sendo instalado, como as métricas estão sendo instaladas e como o SDK está integrado полезные нагрузки Norito.

## Momentos incríveis

1. **Сопоставление lane** — onde o espaço de dados contém `settlement_handle` (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody` ou `xor_dual_fund`). Verifique a faixa de catálogo atual em `docs/source/project_tracker/nexus_config_deltas/`.
2. **Determinar conversão** — o roteador irá transferir sua liquidação no XOR para obter uma situação de licenciamento, утвержденные atualizado. Приватные lane заранее пополняют XOR-буферы; cortes de cabelo применяются только когда буферы выходят за пределы политики.
3. **Телеметрия** — отслеживайте `nexus_settlement_latency_seconds`, счетчики конвертации e датчики corte de cabelo. Os dados foram encontrados em `dashboards/grafana/nexus_settlement.json` e os alertas em `dashboards/alerts/nexus_audit_rules.yml`.
4. **Configurar** — архивируйте конфиги, логи роутера, эксport телеметрии и отчеты по сверке для аудитов.
5. **SDK de SDK** — o SDK do SDK fornece liquidação de liquidação, faixa de IDs e cargas úteis de codificação Norito, чтобы сохранить паритет с роутером.

## Primeiros passos

| Pista tipo | Какие доказательства собрать | O que isso pode ser feito |
|-----------|--------------------|----------------|
| Privado `xor_hosted_custody` | Roteador + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | CBDC-буферы списывают детерминированный XOR, um corte de cabelo остаются в пределах политики. |
| Publicação `xor_global` | Roteador + pesquisa em DEX/TWAP + métricas de latência/conversão | Общий путь ликвидности оценил перевод по опубликованному TWAP com novo corte de cabelo. |
| Girador `xor_dual_fund` | Лог роутера, показывающий разделение public vs blindado + счетчики телеметрии | Смесь blindado/público соблюдала коэффициенты управления и зафиксировала corte de cabelo para каждой части. |

## Quais são os detalhes?

- Perguntas freqüentes: `docs/source/nexus_settlement_faq.md`
- Roteador de liquidação específico: `docs/source/settlement_router.md`
- Política de CBDC: `docs/source/cbdc_lane_playbook.md`
- Operação do runbook: [Exibição Nexus](./nexus-operations)