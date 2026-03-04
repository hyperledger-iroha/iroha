---
lang: he
direction: rtl
source: docs/portal/docs/nexus/lane-model.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-lane-model
כותרת: Modelo de lanes do Nexus
תיאור: Taxonomia logica de lanes, geometria de configuracao e regras de merge de world-state para Sora Nexus.
---

# Modelo de lanes do Nexus e particionamento de WSV

> **סטטוס:** entregavel NX-1 - טקסונומיה של מסלולים, גיאומטריה של הגדרות ופריסה לאחסון פרונטוס ליישום.  
> **בעלים:** Nexus Core WG, Governance WG  
> **Referencia de roadmap:** NX-1 em `roadmap.md`

Esta page do Portal espelha o brief canonico `docs/source/nexus_lanes.md` para que operators do Sora Nexus, בעלים של SDK e revisores possam ler a guia de lanes sem enrar on mono-repo. ארכיטקטורה כללית של מדינת עולמית קובעת ומאפשרת מרחבי נתונים (נתיבים) אינדיבידואלים מבצעים קשרים של אישורי ציבור או פרטיים עם עומסי עבודה בודדים.

## קונסיטוס

- **Lane:** רסיס לוגיקה לעשות חשבונות ב-Nexus com seu proprio set de validadores e backlog de execucao. Identificado por um `LaneId` estavel.
- **מרחב נתונים:** דלי גוברנאנקה que agrupa uma או mais lanes que compartilham politicas de compliance, ניתוב התיישבות.
- **מניפסט הנתיב:** מטא-נתונים בקרה על תקנות, פוליטיקה של DA, אסימון גז, תיקון התיישבות והרשאות ניתוב.
- **מחויבות גלובלית:** הוכחה ל-uma lane que resume novos roots de estado, dados de settlement e transferencias cross-lane opcionais. O anel NPoS התחייבויות סדרנה גלובליות.

## טקסונומיה דה ליין

Os tipos de lane descrevem de forma canonica sua visibilidade, superficie de governanca e hooks de settlement. גיאומטריה של קונפיגורציה (`LaneConfig`) תופסת את התכונות, SDKs e tooling possam racciocinar sobre or layout as logica bespoke.

| טיפו דה ליין | Visibilidade | Membresia de validadores | Exposicao WSV | Governanca por default | פוליטיקה דה התנחלות | Uso tipico |
|----------------------------------------------------|
| `default_public` | ציבורי | ללא רשות (הימור גלובלי) | Replica de estado completa | SORA הפרלמנט | `xor_global` | בסיס לדג'ר פובליקו |
| `public_custom` | ציבורי | ללא רשות או מוגן יתד | Replica de estado completa | Modulo ponderado por stake | `xor_lane_weighted` | Aplicacoes publicas de alto תפוקה |
| `private_permissioned` | מוגבל | סט fixo de validadores (aprovado pela governanca) | התחייבויות הוכחות | מועצה פדרציה | `xor_hosted_custody` | CBDC, עומסי עבודה de consorcio |
| `hybrid_confidential` | מוגבל | Membresia mista; לעטוף הוכחות ZK | התחייבויות + חשיפה seletiva | Modulo de dinheiro programavel | `xor_dual_fund` | Dinheiro programavel com preservacao de privacidade |

מטלות או טיפוס דה ליין להכריז:- כינוי מרחב הנתונים - agrupamento legivel por humanos que vincula politicas de compliance.
- Handle de governanca - זיהוי פתרון דרך `Nexus.governance.modules`.
- Handle de Settlement - זיהוי consumido pelo התנחלות נתב עבור מאגרים דביטרים XOR.
- מטא-נתונים אופציונליים של טלמטריה (תיאור, הודעה, דומיניום של הגנה) דרך `/status` לוחות מחוונים.

## Geometria de configuracao de lanes (`LaneConfig`)

`LaneConfig` e a geometria time runtime derivada do catalogo de lanes validado. Ele nao substitui os manifests de governanca; הם מזדהים את הגדרות האחסון והטלמטריה להגדרת נתיב.

```text
LaneConfigEntry {
    lane_id: LaneId,           // stable identifier
    alias: String,             // human-readable alias
    slug: String,              // sanitised alias for file/metric keys
    kura_segment: String,      // Kura segment directory: lane_{id:03}_{slug}
    merge_segment: String,     // Merge-ledger segment: lane_{id:03}_merge
    key_prefix: [u8; 4],       // Big-endian LaneId prefix for WSV key spaces
    shard_id: ShardId,         // WSV/Kura shard binding (defaults to lane_id)
    visibility: LaneVisibility,// public vs restricted lanes
    storage_profile: LaneStorageProfile,
    proof_scheme: DaProofScheme,// DA proof policy (merkle_sha256 default)
}
```

- `LaneConfig::from_catalog` חישוב מחדש של גיאומטריה quando a configuracao e carregada (`State::set_nexus`).
- כינויים sao sanitizados em slugs minusculos; caracteres nao alfanumericos consecutivos colapsam em `_`. Se o alias produzir um slug vazio, usamos `lane{id}`.
- `shard_id` ומפתח מטא-נתונים `da_shard_id` (ברירת מחדל `lane_id`) מדריך את יומן הסמן של הרסיסים לשמירה על הפעלה חוזרת של ה-DA לקבוע אתחול/החלפה מחדש.
- Prefixos de chave garantem que o WSV mantenha ranges de chave por lane disjuntos mesmo quando o mesmo backend e compartilhado.
- Nomes de segmentos Kura sao deterministas entre מארחים; אודיטורים פודם מאמתים את ההנחיות של הסגמנטים ומציגים כלים בהתאמה אישית.
- Segmentos de merge (`lane_{id:03}_merge`) guardam as ultimas roots de merge-hint e commitments de estado global para aquela lane.

## Particionamento de state world

- הו לוגיקה של המדינה העולמית לעשות Nexus e a uniao de espacos de estado por lane. Lanes publicas persistem estado completo; נתיבים פרטיים/שורשי ייצוא סודיים Merkle/מחויבות עבור ספר מיזוג.
- O armazenamento MV prefixa cada chave com o prefixo de 4 bytes de `LaneConfigEntry::key_prefix`, gerando chaves como `[00 00 00 01] ++ PackedKey`.
- Tabelas compartilhadas (חשבונות, נכסים, טריגרים, registros de governanca) armazenam entradas agrupadas por prefixo de lane, mantendo os range scans deterministas.
- מטא-נתונים לעשות מיזוג-ספר חשבונות מחדש או פריסת mesmo: cada lane escreve roots de merge-hint e roots de estado global reduzido em `lane_{id:03}_merge`, permitindo retencao או eviction direction quando uma lane se retira.
- מדדים חוצי נתיב (כינויים של חשבונות, רישום נכסים, מניפסטים של ממשל) Armazenam prefixos de lane explicitos para que operadores reconciliem entradas rapidamente.
- **Politica de retencao** - lanes publicas retencem corpos de bloco completos; lanes apenas de commitments podem compactar corpos antigos apos checkpoints porque התחייבויות sao autoritativos. כתבי העת החסויים של ה-Lanes cifrados em segmentos dedicados para nao bloquear outros עומסי עבודה.
- **כלים** - שימושי עזר (`kagami`, מנהלי מערכת ב-CLI) מפתחים רפרנסים של מרחב שמות com slug או מדדי אקספור, תוויות Prometheus או או קטעי קבצים מקוונים.

## ניתוב ממשקי API- נקודות קצה Torii REST/gRPC aceitam um `lane_id` אופציונליים; ausencia implica `lane_default`.
- SDKs expoem seletores de lane e mapeiam aliases amigaveis para `LaneId` usando o catalogo de lanes.
- Regras de routing operam sobre o catalogo validado e podem escolher lane e dataspace. `LaneConfig` כינויים מתאימים לטלמטריה עם לוחות מחוונים ויומנים.

## עמלות הסדר e

- Cada lane paga fees XOR ao set global de validadores. נתיבי קודם קולטר אסימונים de gas nativos, mas devem fazer escrow de equivalentes XOR junto com התחייבויות.
- Provas de settlement incluem montante, metadata de conversao e prova de escrow (לדוגמה, העברה לכספת אגרות גלובליות).
- נתב התיישבות יחודי (NX-3) debita buffers usando os mesmos prefixos de lane, assim a telemetria de settlement se alinha com a geometria de storage.

## ממשל

- Lanes declaram seu modulo de governanca via catalogo. `LaneConfigEntry` carrega o alias e o slug originais para manter telemetria e audit trails legiveis.
- Registry O do Nexus distribui lane manifests assinados que incluem `LaneId`, מחייב את מרחב הנתונים, ניהול הממשל, ניהול ההתיישבות ומטא נתונים.
- Hooks de runtime-upgrade continuam aplicando politicas de governanca (`gov_upgrade_id` לברירת מחדל) והרישום משתנים באמצעות גשר טלמטריה (אירועים `nexus.config.diff`).

## סטטוס Telemetria e

- `/status` כינויי חשיפה דה ליין, bindings de dataspace, מטפל ב-governanca e perfis de settlement, derivados do catalogo e do `LaneConfig`.
- מדדי מתזמן (`nexus_scheduler_lane_teu_*`) מציגים כינויים/שבלולים עבור צבר מאפיין מפעיל ו-pressao de TEU rapidamente.
- `nexus_lane_configured_total` כולל מספרים מרוכזים של נתיבי כביש וחשבון מחדש. פליטת טלמטריה מבדילה בין המסלולים לגיאומטריה.
- מדדי צבר ה-dataspace כוללים מטא-נתונים כינוי/תיאור עבור מפעילי עזר שותפים.

## Configuracao e tipos Norito

- `LaneCatalog`, `LaneConfig`, e `DataSpaceCatalog` vivem em `iroha_data_model::nexus` e fornecem estruturas compativeis com Norito para manifests e SDK.
- `LaneConfig` vive em `iroha_config::parameters::actual::Nexus` e e derivado automaticamente do catalogo; nao requer קידוד Norito porque e um helper interno runtime.
- Configuracao voltada ao usuario (`iroha_config::parameters::user::Nexus`) ממשיך aceitando decritores declarativos de lane e dataspace; o ניתוח agora deriva a geometria e rejeita aliases invalidos או IDs de lane duplicados.

## Trabalho pendente- עדכונים אינטגרליים עושים את נתב ההתיישבות (NX-3) com a nova geometria para que debitos e recibos de buffer XOR sejam etiquetados por slug de lane.
- Admin של Estender Tooling עבור משפחות עמודות רשימות, חיבורי נתיבים קומפקטיים ויומנים מיוחדים של קוביות לפי נתיב USando או Namespace com Slug.
- סיום אלרגומי מיזוג (הזמנה, גיזום, זיהוי קונפליקטים) ואביזרים נלווים לרגרסיה עבור שידור חוזר חוצה נתיב.
- ווים נוספים לציות לרשימות הלבנות/רשימות שחורות ופוליטיקה של תכניות (acompanhado sob NX-12).

---

*עקוב אחרי NX-1 מתמשך ל-NX-2 תואם את NX-18. Por favor traga perguntas em aberto para `roadmap.md` ou o tracker de governanca para que o portal fique alinhado com os docs canonicos.*