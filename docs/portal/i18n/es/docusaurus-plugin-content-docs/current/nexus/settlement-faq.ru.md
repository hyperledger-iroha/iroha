---
lang: es
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preguntas frecuentes sobre nexus-settlement
título: Preguntas frecuentes sobre la liquidación
descripción: Ответы для операторов о маршрутизации asentamientos, conversiones en XOR, televisores y auditores dokazatelstvás.
---

Esta página de preguntas frecuentes sobre el acuerdo (`docs/source/nexus_settlement_faq.md`), qué portal de noticias puede encontrar aquí son nuestras recomendaciones поиска в mono-repo. Aquí está el enrutador de liquidación instalado en los archivos, las métricas disponibles y el SDK integrado en varias posiciones. Norito.

## Основные моменты

1. **Сопоставление lane** — el espacio de datos correspondiente a `settlement_handle` (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody` o `xor_dual_fund`). Mueva el carril del catálogo actual en `docs/source/project_tracker/nexus_config_deltas/`.
2. **Детерминированная конвертация** — el enrutador permanecerá en la liquidación en XOR con respecto a las conexiones históricas, утвержденные управлением. El carril privado está equipado con búferes XOR populares; cortes de pelo применяются только когда буферы выходят за пределы политики.
3. **Телеметрия** — отслеживайте `nexus_settlement_latency_seconds`, счетчики конвертации и датчики corte de pelo. Los controles están en `dashboards/grafana/nexus_settlement.json` y las alertas en `dashboards/alerts/nexus_audit_rules.yml`.
4. **Доказательства** — Configure configuraciones, registros de enrutador, televisores deportivos y dispositivos de audio.
5. **SDK activo**: un SDK exclusivo para la liquidación de códigos, carriles de ID y cargas útiles de codificadores Norito, que admiten particiones enrutador.

## Примеры потоков| Carril tip | Какие доказательства собрать | Что это подтверждает |
|-----------|--------------------|----------------|
| Privado `xor_hosted_custody` | Лог роутера + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | Los buffers CBDC están diseñados para XOR, un corte de pelo establecido en la política anterior. |
| Publicado `xor_global` | Лог роутера + ссылка на DEX/TWAP + métricas de latencia/convertencias | Общий путь ликвидности оценил перевод по опубликованному TWAP с нулевым haircut. |
| Gibridana `xor_dual_fund` | Лог роутера, показывающий разделение public vs blinded + sчетчики телеметрии | Смесь blindado/público соблюдала коэффициенты управления и зафиксировала corte de pelo для каждой части. |

## ¿Hay más detalles?

- Preguntas frecuentes en polaco: `docs/source/nexus_settlement_faq.md`
- Enrutador de liquidación de especificaciones: `docs/source/settlement_router.md`
- Política de CBDC: `docs/source/cbdc_lane_playbook.md`
- Operación del Runbook: [Operaciones Nexus](./nexus-operations)