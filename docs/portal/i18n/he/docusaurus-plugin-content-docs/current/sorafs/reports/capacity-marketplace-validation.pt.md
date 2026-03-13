---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: Validacao do mercado de capacidade SoraFS
תגיות: [SF-2c, קבלה, רשימת תיוג]
תקציר: רשימת רשימות של aceitacao cobrindo onboarding של ספקים, fluxos de disputa e reconciliacao do tesouro que liberam a disponibilidade geral do mercado de capacidade SoraFS.
---

# רשימת רשימות של validacao do mercado de capacidade SoraFS

**Janela de revisao:** 2026-03-18 -> 2026-03-24  
**Responsaveis do programa:** צוות אחסון (`@storage-wg`), מועצת ממשל (`@council`), גילדת האוצר (`@treasury`)  
**Escopo:** צינורות של ספקים, זרימות של ניהול מחלוקת ותהליכים של ריקונסיציה לעשות דרישות חיפוש עבור GA SF-2c.

רשימת בדיקות אבאיקסו deve ser revisado antes de habilitar o Mercado para Operatores externos. Cada linha conecta evidencias deterministicas (בדיקות, מתקנים או דוקומנטרים) que auditores podem reproduzir.

## רשימת רשימות של אסיטקאו

### כניסה לספקים

| Checagem | Validacao | Evidencia |
|-------|----------------|--------|
| O registry aceita declaracoes canonicas de capacidade | בדיקה אינטגרלית של תרגול `/v2/sorafs/capacity/declare` באמצעות API של אפליקציה, אימות תיקון של אסינטוראס, קלטת מטא נתונים והעברה לצומת רישום. | `crates/iroha_torii/src/routing.rs:7654` |
| O חוזה חכם דחה מטענים שונים | O teste unitario garante que IDs de provider e campos GiB comprometidos correspondem a declaracao assinada antes de persistir. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| O CLI emite artefatos canonicos de onboarding | הרתמה של CLI escreve saidas Norito/JSON/Base64 קובעת את תקינות נסיעות הלוך ושוב עבור מפעילי אפשרות הכנה לא מקוונת. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| O guia do operator cobre o workflow de admissao e guardrails de governanca | תקציר של סכימת הצהרות, ברירת מחדל של מדיניות e passos de revisao para o Council. | `../storage-capacity-marketplace.md` |

### Resolucao de disputas

| Checagem | Validacao | Evidencia |
|-------|----------------|--------|
| Registros de disputa persistem com digest canonico do payload | או בדיקה יחידה רישום אומה מחלוקת, decodifica או מטען ארמזנאדו efirma status ממתין להכרעת קביעת החשבונות. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| O gerador de disputas do CLI corresponde ao schema canonico | O teste do CLI cobre saidas Base64/Norito e resumos JSON para `CapacityDisputeV1`, garantindo que queds bunles tenham hash deterministico. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| O teste de replay prova determinismo de disputa/penalidade | טלמטריה של הוכחה-כישלון חוזרת על תצלומי מצב זהים לפנקס החשבונות, קרדיט ודיון כדי לחתוך את התוצאות הקובעות בין עמיתים. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| O runbook documenta o fluxo de escalonamento e revogacao | O guia de operacoes captura o workflow do Council, requisitos de Evidencia e Procedimentos de rollback. | `../dispute-revocation-runbook.md` |

### Reconciliacao do tesouro| Checagem | Validacao | Evidencia |
|-------|----------------|--------|
| O צבירת פנקס חשבונות מתכתב עם פרויקט ספיגה של 30 ימים | O teste de soak abrange cinco ספקים em 30 Janelas de Settlement, comparando entradas do Ledger com a referencia de payout esperada. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| A reconciliacao de exportes do book and registrada a cada noite | `capacity_reconcile.py` השוואת ציפיות לעשות עמלת חשבונות com מייצאת XOR executados, emite metricas Prometheus e faz gate da aprovacao do tesouro via Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| לוחות מחוונים של תשלומים לחיובים וטלמטריה לצבירה | O ייבוא ​​לעשות Grafana תוכנית או צבירת GiB-שעת, אישורי שביתות ובטחונות מלוכדים לראייה ב-call. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| O relatorio publicado arquiva a metodologia de soak e comandos de replay | O relatorio detalha o escopo do soap, comandos de execucao e hooks de observabilidade para auditores. | `./sf2c-capacity-soak.md` |

## Notas de execucao

בצע מחדש את Suite de validacao ante do sign-off:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

מערכת ההפעלה מפתחת מטענים מחודשים של בקשה ל- onboarding/disputa com `sorafs_manifest_stub capacity {declaration,dispute}` e arquivar os bytes JSON/Norito resultantes Junto ao ticket de governanca.

## Artefatos de aprovacao

| ארטפטו | קמיניו | blake2b-256 |
|--------|------|--------|
| Pacote de aprovacao de onboarding de ספקים | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Pacote de aprovacao de resolucao de disputas | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Pacote de aprovacao de reconciliacao do tesouro | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

Guarde as copias assinadas desses artefatos com o bundle de release e vincule-as no registro de mudancas de governanca.

## אפרובאקו

- ראש צוות אחסון - @storage-tl (2026-03-24)  
- מזכיר מועצת הממשל - @council-sec (2026-03-24)  
- מוביל תפעול משרד האוצר - @treasury-ops (2026-03-24)