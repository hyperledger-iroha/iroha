---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-default-lane-quickstart
כותרת: Guia rapida do lane padrao (NX-5)
sidebar_label: Guia rapida do lane padrao
תיאור: קבע את התצורה של אימות e-fallback do lane padrao do Nexus עבור Torii e SDKs possam omitir lane_id em lanes publicas.
---

:::שים לב Fonte canonica
Esta pagina espelha `docs/source/quickstart/default_lane.md`. Mantenha ambas as copias alinhadas ate que a revisao de localizacao chegue ao פורטל.
:::

# Guia rapida do lane padrao (NX-5)

> **הקשר של מפת הדרכים:** NX-5 - אינטגרה למסלול ציבורי. O runtime agora expoe um fallback `nexus.routing_policy.default_lane` para que e endpoints REST/gRPC do Torii e cada SDK possam omitir com seguranca um `lane_id` quando o trafego pertence canonico lane. Este guia Leva מפעילה קטלוג, אימות או fallback ב-`/status` eercitar o comportamento do cliente de ponta a ponta.

## דרישות מוקדמות

- אום build Sora/Nexus de `irohad` (ביצוע `irohad --sora --config ...`).
- Acesso ao repositorio de configuracao para editar secoes `nexus.*`.
- `iroha_cli` configurado para falar com o cluster alvo.
- `curl`/`jq` (ou equivalente) לבחירת מטען או מטען `/status` ל-Torii.

## 1. הסר את קטלוג הנתיבים ומרחבי הנתונים

הכרזה על נתיבים של מערכת הפעלה ומרחבי נתונים. O trecho abaixo (recortado de `defaults/nexus/config.toml`) registra tres lanes publicas mais os alias de dataspace correspondentes:

```toml
[nexus]
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"
dataspace = "universal"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance & parliament traffic"
dataspace = "governance"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
description = "Zero-knowledge attachments"
dataspace = "zk"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance proposals & manifests"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "zk"
id = 2
description = "Zero-knowledge proofs and attachments"
fault_tolerance = 1
```

Cada `index` deve ser unico e contiguo. מערכת הזיהויים של מרחב הנתונים בגודל 64 סיביות; os exemplos acima usam os mesmos valores numericos que os indices de lane para maior clareza.

## 2. Defina os padroes de roteamento e as sobreposicoes opcionais

A secao `nexus.routing_policy` controla o lane de fallback e permite sobrescrever o roteamento para instrucoes especificas ou prefixos de conta. ראה nenhuma regra corresponder, או מתזמן רותיה a transacao para `default_lane` e `default_dataspace` configurados. A logica do router vive em `crates/iroha_core/src/queue/router.rs` e aplica a politica de forma transparente as superficies REST/gRPC do Torii.

```toml
[nexus.routing_policy]
default_lane = 0                # use the "core" lane when no rules match
default_dataspace = "universal"    # reuse the public dataspace for the fallback

[[nexus.routing_policy.rules]]
lane = 1
dataspace = "governance"
[nexus.routing_policy.rules.matcher]
instruction = "governance"
description = "Route governance instructions to the governance lane"

[[nexus.routing_policy.rules]]
lane = 2
dataspace = "zk"
[nexus.routing_policy.rules.matcher]
instruction = "smartcontract::deploy"
description = "Route contract deployments to the zk lane for proof tracking"
```

Quando voce adicionar novas lanes no futuro, להגדיר את פרימיירו או קטלוג e depois estenda כמו regras de roteamento. O lane de fallback deve continuar apontando para o lane publico que concentra a maior parte do trafego de usuarios para que SDKs alternatives permanecam compativeis.

## 3. פתח את הצומת עם אפליקציית פוליטיקה

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

או רישום צומת ופוליטיקה של רוטאמנטו נגזרת משך ההפעלה. Quaisquer erros de validacao (מדדים ausentes, alias duplicados, ids de dataspace invalidos) aparecem antes de o gossip iniciar.

## 4. אשר את estado de governanca do lane

Assim que o node estiver באינטרנט, השתמש o helper do CLI para verificar se o lane padrao esta selado (manifest carregado) e pronto para trafego. תצהיר רזומה איפריים אומה לינה פור ליין:

```bash
iroha_cli app nexus lane-report --summary
```

פלט לדוגמה:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```Se o lane padrao mostrar `sealed`, סיג או runbook de governanca de lanes antes de permitir trafego externo. דגל `--fail-on-sealed` e util para CI.

## 5. Inspecione OS deadloads de status do Torii

תשובה `/status` מציגה את הפוליטיקה של הרוטאמנטו או תמונת מצב לעשות מתזמן לפי ליין. השתמש ב-`curl`/`jq` עבור קונפיגורציית קונפיגורציות של מערכת הפעלה ובדיקה של מסלול ה-fallback esta produzindo telemetria:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

פלט לדוגמה:

```json
{
  "default_lane": 0,
  "default_dataspace": "universal",
  "rules": [
    {"lane": 1, "dataspace": "governance", "matcher": {"instruction": "governance"}},
    {"lane": 2, "dataspace": "zk", "matcher": {"instruction": "smartcontract::deploy"}}
  ]
}
```

לבחירתך של מתזמן למסלול `0`:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

איסו מאשרת que o תמונת מצב של TEU, os metadados de alias e os flags de manifest alinham com a configuracao. O mesmo payload e usado pelos paineis do Grafana עבור לוח מחוונים לכניסת נתיב.

## 6. תרגיל OS padroes do cliente

- **Rust/CLI.** `iroha_cli` e o crate cliente Rust omitem o campo `lane_id` quando voce nao passa `--lane-id` / `LaneSelector`. או נתב de filas, portanto, cai em `default_lane`. השתמש בדגלים מפורשים `--lane-id`/`--dataspace-id` אפנים או מיראר um lane nao padrao.
- **JS/Swift/Android.** כפי שיוצא מהדורות האולטימטיביות של SDK tratam `laneId`/`lane_id` כמו אופציונליים ו-fazem fallback para o valor anunciado por `/status`. Mantenha a politica de roteamento sincronizada entre staging e producao para que apps moveis nao precisem de reconfiguracoes de emergencia.
- **בדיקות צינור/SSE.** מסנני אירועים של טרנסאקאו aceitam predicados `tx_lane_id == <u32>` (veja `docs/source/pipeline.md`). Assine `/v2/pipeline/events/transactions` com esse filtro para provar que escritas enviadas Sem lane explicito chegam sob o id de lane de fallback.

## 7. Observabilidade e ganchos de governanca

- `/status` tambem publica `nexus_lane_governance_sealed_total` e `nexus_lane_governance_sealed_aliases` para que o Alertmanager avise quando um lane perder seu manifest. Mantenha esses alertas habilitados mesmo em devnets.
- O mapa de telemetria do scheduler e o לוח המחוונים של governanca de lanes (`dashboards/grafana/nexus_lanes.json`) esperam os campos alias/slug do catalogo. ראה מחדש את הכינוי, כתוב מחדש את המדריך.
- Aprovacoes parlamentares para lanes padrao devem כוללות את הפלנו של החזרה לאחור. הירשם ל-hash do manifest e a Evidencia de governanca Junto com este quickstart no seu runbook de operador para que futuras rotacoes nao adivinhem o estado requerido.

Depois que essas verificacoes passarem, voce pode tratar `nexus.routing_policy.default_lane` como a fonte de verdade para a configuracao dos SDKs e comecar and disabilitar os caminhos de codigo alternativos de lane unico na red.