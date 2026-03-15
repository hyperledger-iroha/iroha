---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/node-storage.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: node-storage
כותרת: Design de storage do nodo SoraFS
sidebar_label: Design de storage do nodo
תיאור: ארכיון אחסון, מכסות e hooks de ciclo de vida para nodes Torii que hospedam dados SoraFS.
---

:::שים לב Fonte canonica
Esta pagina espelha `docs/source/sorafs/sorafs_node_storage.md`. Mantenha ambas as copias sincronizadas.
:::

## עיצוב אחסון do nodo SoraFS (טיוטה)

Esta not refina como um nodo Iroha (Torii) pode optar pela camada de
זמינות de dados do SoraFS e dedicar um pedaco do disco local para
נתחי armazenar e servir. אלה משלימים לגילוי ספציפי
`sorafs_node_client_protocol.md` e o trabalho de fixtures SF-1b ao detalhar a
arquitetura do lado do אחסון, controls de recursos e plumbing de
configuracao que devem aterrissar no nodo e nos caminhos do gateway.
Os drills operacionais ficam no
[Runbook de operacoes do nodo](./node-operations).

### אובייקטים

- מתיר que qualquer validador ou processo auxiliar do Iroha exponha disco
  ocioso como provedor SoraFS סאם אפטר כאחראיות לעשות ספר חשבונות.
- Manter o modulo de storage deterministico e guiado por Norito: מניפסטים,
  planos de chunk, מעלה הוכחת אחזור (PoR) e adverts de provedor sao
  a fonte de verdade.
- הכניסו מכסות definidas pelo operador para que um nodo nao esgote seus
  recursos ao aceitar demasiadas requisicoes de pin ou להביא.
- Expor saude/telemetria (amostragem PoR, latencia de fetch de chunk, pressao de
  disco) de volta para governanca e clientes.

### ארכיטטורה דה אלטו ניבל

```
+--------------------------------------------------------------------+
|                         Iroha/Torii Node                           |
|                                                                    |
|  +----------+      +----------------------+                        |
|  | Torii APIs|<---->|    SoraFS Gateway   |<---------------+       |
|  +----------+      |  (Norito endpoints)  |                |       |
|                    +----------+-----------+                |       |
|                               |                            |       |
|                    +----------v-----------+                |       |
|                    |     Pin Registry     |<---- manifests |       |
|                    |     (State / DB)     |                |       |
|                    +----------+-----------+                |       |
|                               |                            |       |
|                    +----------v-----------+                |       |
|                    |     Chunk Storage    |<---- chunk plans|       |
|                    |      (ChunkStore)    |                |       |
|                    +----------+-----------+                |       |
|                               |                            |       |
|                    +----------v-----------+                |       |
|                    |    Disk Quota/IO     |--pin/serve----->| Fetch |
|                    |      Scheduler       |                | Clients|
|                    +----------------------+                |       |
|                                                                    |
+--------------------------------------------------------------------+
```

Modulos chave:

- **שער**: נקודות קצה של חשיפה HTTP Norito להצעת ה-Pin, Requisicoes de
  להביא את הנתחים, amostragem PoR e telemetria. מטענים של Valida Norito e
  encaminha requisicoes para o chunk store. שימוש ב-Stack HTTP do Torii para
  evitar um novo daemon.
- **רישום סיכות**: estado de pin de manifest rastreado em `iroha_data_model::sorafs`
  e `iroha_core`. בצע את המניפסט או את רישום הרישום או לעכל
  manifest, digest do plano de chunk, raiz PoR e flags de capacidade do provedor.
- **אחסון נתחים**: implementacao `ChunkStore` em disco que ingere manifests
  assinados, materializa planos de chunk usando `ChunkProfile::DEFAULT`, e
  מתמשכים נתחים עם פריסה דטרמיניסטית. Cada chunk e associado a um
  טביעת אצבע de conteudo e metadados PoR para que a amostragem possa revalidar
  sem reler o arquivo inteiro.
- **מכסה/מתזמן**: impoe limites configurados pelo operador (bytes max de
  דיסקו, pins pendentes max, fetches paralelos max, TTL de chunk) e coordena IO
  para que as tarefas do Ledger nao fiquem sem recursos. O מתזמן tambem ה
  תשובות לשירותים בדוקים PoR e requisicoes de amostragem com CPU limitada.

### Configuracao

Adicione uma nova secao em `iroha_config`:

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # tag opcional legivel
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```- `enabled`: החלפת מצב. Quando false o gateway retorna 503 para
  נקודות קצה de storage e o nodo nao anuncia em discovery.
- `data_dir`: diretorio raiz para dados de chunk, arvores PoR e telemetria de
  להביא. ברירת מחדל `<iroha.data_dir>/sorafs`.
- `max_capacity_bytes`: limite rigido para dados de chunk pinados. אומה טארפה דה
  fundo rejeita novos pins quando o limite e atingido.
- `max_parallel_fetches`: limite de concorrencia imposto pelo scheduler para
  balancear banda/IO de disco contra a carga do validador.
- `max_pins`: maximo de pins de manifest que o nodo aceita antes de aplicar
  eviccao/לחץ גב.
- `por_sample_interval_secs`: cadencia para jobs automaticos de amostragem PoR.
  Cada job amostra `N` folhas (configuravel por manifest) e emite eventos de
  טלמטריה. A governanca pode escalar `N` de forma deterministica definindo a
  chave de metadata `profile.sample_multiplier` (inteiro `1-4`). הו גבורה
  ser um numero/string unico ou um objeto com עוקף por perfil, por exemplo
  `{"default":2,"sorafs.sf2@1.0.0":3}`.
- `adverts`: estrutura usada pelo gerador de advert para preencher campos
  `ProviderAdvertV1` (מצביע על הימור, רמזים ל-QoS, נושאים). Se omitido o nodo
  ברירת המחדל של ארה"ב עושה את הרישום דה גוברננקה.

אינסטלציה דה קונפיגוראקאו:

- `[sorafs.storage]` e definido em `iroha_config` como `SorafsStorage` e e
  carregado do arquivo de config do nodo.
- `iroha_core` ו-`iroha_torii` מעבר לתצורת אחסון עבור בונה
  עשה gateway e o chunk store ללא הפעלה.
- עוקף את קיומי הפיתוח/בדיקה (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`), mas
  פורס את de producao devem confiar ללא arquivo de config.

### שימושי עזר של CLI

Enquanto a superficie HTTP do Torii ainda esta sendo ligada, או ארגז
`sorafs_node` envia uma CLI leve para que operadores possam automatizar drills de
ingestao/exportacao contra o backend persistente. [crates/sorafs_node/src/bin/sorafs-node.rs:1]

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` espera um manifest `.to` codificado em Norito mais os bytes de payload
  כתבים. Ele reconstroi o plano de chunk a partir do perfil de
  chunking do manifest, impoe paridade de digest, persiste arquivos de chunk e
  optionalmente emite um blob JSON `chunk_fetch_specs` עבור כלי עבודה במורד הזרם
  פריסה חוקית.
- `export` aceita um ID de manifest e grava o manifest/loadload armazenado em
  דיסקו (com plan JSON אופציונלי) עבור אביזרי ההמשך.

Ambos os comandos imprimem um resumo Norito JSON em stdout, facilitando o uso em
תסריטים. A CLI e coberta por um teste de integracao para garanter que manifests e
מטענים מותאמים הלוך ושוב לפני ה-APIs Torii entrarem. [crates/sorafs_node/tests/cli.rs:1]> Paridade HTTP
>
> O gateway Torii agora expoe helpers לקריאה בלבד apoiados pelo mesmo
> `NodeHandle`:
>
> - `GET /v1/sorafs/storage/manifest/{manifest_id_hex}` - retorna o manifest
> Norito armazenado (base64) junto com digest/metadata. [crates/iroha_torii/src/sorafs/api.rs:1207]
> - `GET /v1/sorafs/storage/plan/{manifest_id_hex}` - retorna o plano de chunk
> JSON (`chunk_fetch_specs`) מוגדר עבור כלי עבודה במורד הזרם. [crates/iroha_torii/src/sorafs/api.rs:1259]
>
> נקודות הקצה של Esses espelham a saida do CLI para que pipelines possam trocar
> סקריפטים מקוונים לבדיקת HTTP סמים מנתחים. [crates/iroha_torii/src/sorafs/api.rs:1207] [crates/iroha_torii/src/sorafs/api.rs:1259]

### Ciclo de vida do nodo

1. **אתחול**:
   - Se storage estiver habilitado o nodo inicializa o chunk store com o
     הגדרות מדריך ויכולות. זה כולל אימות או בסיס
     de dados de manifest PoR e fazer replay de manifests pinados para aquecer
     מטמונים.
   - רשם כ-rotas do gateway SoraFS (נקודות קצה Norito JSON POST/GET עבור פין,
     להביא, amostragem PoR, טלמטריה).
   - Iniciar o worker de amostragem PoR e o לפקח על המכסות.
2. **גילוי / פרסומות**:
   - Gerar documentos `ProviderAdvertV1` usando capacidade/saude atual, assinar
     com a chave aprovada pelo conselho e publicar via Discovery. השתמש ברשימה
3. **Fluxo de pin**:
   - O gateway recebe um manifest assinado (כולל plano de chunk, raiz PoR,
     assinaturas do conselho). תוקף רשימה של כינויים (`sorafs.sf1@1.0.0`
     requerido) e garante que o plano de chunk corresponde a metadata do manifest.
   - מכסות Verifica. ראה capacidade/limites de pin excederem, תגיב com erro
     de politica (Norito estruturado).
   - Stream de dados de chunk para o `ChunkStore`, verificando digests durante
     אינגסטאו. Atualiza arvores PoR e armazena metadata לא מראה שום רישום.
4. **Fluxo de fetch**:
   - מגישים רצונות דה רנד דה נתח למנות דיסקו. הו מתזמן
     `max_parallel_fetches` e retorna `429` quando saturado.
   - Emite telemetria estruturada (Norito JSON) com latencia, bytes servidos e
     Contadores de Erro Para Monitoramento במורד הזרם.
5. **Amostragem PoR**:
   - O worker seleciona manifestes proporcionalmente ao peso (por exemplo, bytes
     armazenados) e roda amostragem deterministica usando a arvore PoR לעשות חנות נתחים.
   - Persiste resultados para auditorias de governanca e inclui resumos em adverts
     de provedor / נקודות קצה של טלמטריה.
6. **Eviccao / cumprimento de quotas**:
   - Quando a capacidade e atingida o nodo rejeita novos pins por padrao. אופציונלי,
     מפעילי פודם קונפיגורר פוליטיקה של eviccao (לדוגמה: TTL, LRU)
     de governanca estiver definido; por ora o design assume quotas estritas e
     operacoes de unpin iniciadas pelo operador.

### Declaracao de capacidade e integracao de Scheduling- Torii עדכוני agora repassa de `CapacityDeclarationRecord` de `/v1/sorafs/capacity/declare`
  para o `CapacityManager` embutido, de modo que cada nodo constroi uma visao em memoria
  de suas alocacoes comprometidas de chunker e lane. O מנהל חשיפת תמונות לקריאה בלבד
  para telemetria (`GET /v1/sorafs/capacity/state`) e impoe reservas por perfil ou lane
  אנטס דה נובה אורנסס סרם aceitas. [crates/sorafs_node/src/capacity.rs:1] [crates/sorafs_node/src/lib.rs:60]
- O נקודת קצה `/v1/sorafs/capacity/schedule` aceita מטענים `ReplicationOrderV1`
  emitidos pela governanca. בדוק או מוכיח או מוכיח את המנהל המקומי
  תזמון דופליקדו, valida capacidade de chunker/lane, reserva o slice e retorna
  um `ReplicationPlan` descrevendo capacidade restante para que ferramentas de
  orquestracao possam seguir com a ingestao. Ordens para outros provedores sao
  Reconhecidas com Reposta `ignored` עבור זרימות עבודה מרובות מפעילים נוחים. [crates/iroha_torii/src/routing.rs:4845]
- Hooks de conclusao (לדוגמה, disparados apos ingestao bem sucedida) chamam
  `POST /v1/sorafs/capacity/complete` para liberar reservas via
  `CapacityManager::complete_order`. תשובה כוללת תמונת מצב
  `ReplicationRelease` (totais restantes, residuais de chunker/lane) para que tooling
  de orquestracao possa enfileirar a proxima ordem sem polling. Trabalho futuro
  vai conectar isso ao pipeline do chunk store assim que a logica de ingestao chegar.
  [crates/iroha_torii/src/routing.rs:4885] [crates/sorafs_node/src/capacity.rs:90]
- O `TelemetryAccumulator` embutido pode ser mutado via
  `NodeHandle::update_telemetry`, מותר לרשום עובדים ברקע
  תכונות של PoR/uptime ובסופו של דבר נגזרים עומסים קנוניים
  `CapacityTelemetryV1` sem tocar nos internos do מתזמן. [crates/sorafs_node/src/lib.rs:142] [crates/sorafs_node/src/telemetry.rs:1]

### אינטגרציות ופיתוח עתידיים

- **Governanca**: estender `sorafs_pin_registry_tracker.md` com telemetria de
  אחסון (taxa de sucesso PoR, utilizacao de disco). Politicas de admissao podem
  exigir capacidade minima או taxa minima de sucesso PoR antes de aceitar adverts.
- **SDKs de cliente**: גלה תצורת אחסון חדשה (מגבלות דיסקו, כינוי)
  para que ferramentas de gestao possam bootstrapar nodes programaticamente.
- **טלמטריה**: אינטגרר com o stack de metricas existente (Prometheus /
  OpenTelemetry) למטרות אחסון במכשירים עם לוחות מחוונים של תצפית.
- **Seguranca**: rodar o modulo de storage em um pool dedicado de tarefas async com
  לחץ אחורי וקליעה לארגזי חול מרוכזים דרך בריכות
  Tokio limitados para evitar que clientes maliciosos esgotem recursos.

Este design mantem o modulo de storage optional e deterministico enquanto fornece
כפתורי הפעלה נחוצים עבור מפעילים משתתף בארגון הזמינות
SoraFS. Implementa-lo exigira mudancas em `iroha_config`, `iroha_core`, `iroha_torii`
ללא שער Norito.