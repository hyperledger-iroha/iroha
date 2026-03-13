---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/node-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: תוכנית צמתים
כותרת: Plano de implementacao do nodo SoraFS
sidebar_label: Plano de implementacao do nodo
תיאור: המרת מפת דרכים של אחסון SF-3 ל-trabalho de engenharia acionavel com marcos, tarefas e cobertura de testes.
---

:::שים לב Fonte canonica
Esta pagina espelha `docs/source/sorafs/sorafs_node_plan.md`. Mantenha ambas as copias sincronizadas ate que a documentacao Sphinx alternativa seja retirada.
:::

SF-3 entrega או primeiro executavel `sorafs-node` que transforma um processo Iroha/Torii עם מוכיח אחסון SoraFS. השתמש ב-este plano junto com o [guia de storage do nodo](node-storage.md), a [politica de admissao de provedores](provider-admission-policy.md) e o [מפת הדרכים של ה-marketplace de capacidade de storage](Norito) entregas.

## Escopo alvo (Marco M1)

1. **Integracao do chunk store.** Envolver `sorafs_car::ChunkStore` com um backend persistente que armazena bytes de chunk, manifests e arvores PoR no diretorio de dados configurado.
2. **נקודות קצה של שער.** יציאת נקודות קצה HTTP Norito להגשת סיכה, אחזור נתחים, אסטרטגיית PoR וטלמטריה של אחסון דנטרו לעיבוד Torii.
3. ** אינסטלציה של קונפיגורציה.** Adicionar uma struct de config `SoraFsStorage` (דגל תקין, capacidade, diretorios, limites de concorrencia) conectada via `iroha_config`, Prometheus, Prometheus.
4. **מכסה/אג'נדה.** איפור מגבלות דיסקו/פראלליסמו definidos pelo operator e enfileirar requisicoes com back-pressure.
5. **Telemetria.** Emitir מדדים/יומנים עבור סיכת סיכה, זמן אחזור של נתחים, שימוש ב-capacidade ו-Resultados de amostragem PoR.

## Quebra de trabalho

### A. Estrutura de crate e modulos

| טארפה | דונו(ים) | Notas |
|------|--------|------|
| Criar `crates/sorafs_node` com modulos: `config`, `store`, `gateway`, `scheduler`, `telemetry`. | צוות אחסון | Reexporta tipos reutilizaveis para integracao com Torii. |
| Implementar `StorageConfig` mapeado de `SoraFsStorage` (משתמש -> בפועל -> ברירות מחדל). | צוות אחסון / Config WG | Garante que as caadas Norito/`iroha_config` permanecam deterministicas. |
| Fornecer uma facade `NodeHandle` que Torii ארה"ב עבור סיכות/שליפות תת-מדיות. | צוות אחסון | Encapsula internos de storage e אינסטלציה אסינכרון. |

### ב. חנות נתחים מתמשכת

| טארפה | דונו(ים) | Notas |
|------|--------|------|
| Construir um backend em disco envolvendo `sorafs_car::ChunkStore` com indice de manifest em disco (`sled`/`sqlite`). | צוות אחסון | דטרמיניסטי פריסה: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| Manter metadados PoR (arvores 64 KiB/4 KiB) usando `ChunkStore::sample_leaves`. | צוות אחסון | תמיכה בשידור חוזר אפוס הפעלה מחדש; falha rapido em corrupcao. |
| שידור חוזר מיושם של אינטגריד ללא הפעלה (rehash de manifests, podar pins uncompletos). | צוות אחסון | Bloqueia o start de Torii אכל או terminar הפעלה חוזרת. |

### ג. נקודות קצה של שער| נקודת קצה | Comportamento | טארפס |
|--------|-------------|--------|
| `POST /sorafs/pin` | Aceita `PinProposalV1`, valida manifests, enfileira ingestao, response com o CID do manifest. | תקף פרפיל של צ'אנקר, מכסות ייבוא, זרם תקליטורים דרך חנות נתחים. |
| `GET /sorafs/chunks/{cid}` + שאילתת טווח | Servir bytes de chunk com headers `Content-Chunker`; Respeita a especificacao de capacidade de range. | מתזמן Usar + orcamentos de stream (ליגר ויכולת טווח SF-2d). |
| `POST /sorafs/por/sample` | Rodar amostragem PoR para um manifest e retornar bundle de prova. | Reutilizar amostragem do chunk store, responder com מטענים Norito JSON. |
| `GET /sorafs/telemetry` | קורות חיים: capacidade, sucesso PoR, contadores de erro de fech. | Fornecer dados עבור לוחות מחוונים/אופרדורים. |

O אינסטלציה עם זמן ריצה לעבור כאינטראקואים PoR דרך `sorafs_node::por`: o Tracker registra cada `PorChallengeV1`, `PorProofV1` e `AuditVerdictV1` para que as metricas Prometheus sem metricas Prometheus Torii בהזמנה אישית. [crates/sorafs_node/src/scheduler.rs:147]

Notas de implementacao:

- השתמש ב-o stack Axum de Torii com loads `norito::json`.
- Adicone schemas Norito לתשובות (`PinResultV1`, `FetchErrorV1`, structs de telemetria).

- `/v2/sorafs/por/ingestion/{manifest_digest_hex}` agora expoe a profundidade do backlog mais a epoca/deadline mais antiga e os timestamps mais recentes de sucesso/falha por provedor, via `sorafs_node::NodeHandle::por_ingestion_status`, e Torii gauges `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` עבור לוחות מחוונים. [crates/sorafs_node/src/lib.rs:510] [crates/iroha_torii/src/sorafs/api.rs:1883] [crates/iroha_torii/src/routing.rs:7244] [crates/iroha_telemetry/src:/3metrics.0]

### ד. מתזמן e cumprimento de quotas

| טארפה | פרטים |
|------|--------|
| מכסה דה דיסקו | Rastrear bytes em disco; rejeitar novos pins ao exceder `max_capacity_bytes`. Fornecer hooks de eviccao para politicas futuras. |
| Concorrencia de fetch | Semaforo העולמית (`max_parallel_fetches`) מאיה אורקמנטוס למקורי קפס SF-2d. |
| פילה דה סיכות | משרות מוגבלות de ingestao pendentes; Expor Points Norito de status para profundidade da fila. |
| Cadencia PoR | Worker de background dirigido por `por_sample_interval_secs`. |

### E. רישום אלקטרוני של Telemetria

מדדים (Prometheus):

- `sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (תוויות histograma com `result`)
- `torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
- `torii_sorafs_storage_fetch_bytes_per_sec`
- `torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

יומנים / אירועים:

- Telemetria Norito estruturada para ingestao de governanca (`StorageTelemetryV1`).
- אזעקות quando utilizacao > 90% ou streak de falhas PoR מעבר לסף.

### F. Estrategia de testes1. **Testes unitarios.** Persistencia do chunk store, calculos de quata, invariantes do scheduler (ver `crates/sorafs_node/src/scheduler.rs`).
2. **Testes de integracao** (`crates/sorafs_node/tests`). Pin -> אחזר הלוך ושוב, התאוששות אpos restart, rejeicao por quota, verificacao de provas de amostragem PoR.
3. **Testes de integracao Torii.** Rodar Torii com אחסון זמין, הפעלת נקודות קצה HTTP באמצעות `assert_cmd`.
4. **מפת הדרכים של קאוס.** תרגילי עתיד סימולאם אקסוסטאו דה דיסקו, IO lento, remocao de provedores.

## Dependencias

- Politica de admissao SF-2b - garantir que nodes verifiquem envelopes de admissao antes de anunciar.
- Marketplace de capacidade SF-2c - ליגר telemetria de volta a declaracoes de capacidade.
- הרחבות של פרסומות SF-2d - צריכת נפח טווח + מקורות זרמים.

## קריטריונים דה אמרו למרקו

- `cargo run -p sorafs_node --example pin_fetch` funciona קונטרה fixtures locais.
- Torii קומפילה com `--features sorafs-storage` e passa testes de integracao.
- Documentacao ([guia de storage do nodo](node-storage.md)) atualizada com defaults de configuracao + exemplos de CLI; runbook de operador disponivel.
- Telemetria visivel em dashboards de staging; alertas configurados para saturacao de capacidade e falhas PoR.

## Entregaveis de documentacao e ops

- אטואליסר ל-[referencia de storage do nodo](node-storage.md) com ברירות המחדל של הגדרות, שימוש ב-CLI ומעבר לפתרון בעיות.
- Manter o [runbook de operacoes de nodo](node-operations.md) alinhado com a implementacao conforme SF-3 evolui.
- הפניות פומביות של ה-API לנקודות קצה `/sorafs/*` מבצעות את הפורטל ליצירת קשרים או מניפסט OpenAPI.