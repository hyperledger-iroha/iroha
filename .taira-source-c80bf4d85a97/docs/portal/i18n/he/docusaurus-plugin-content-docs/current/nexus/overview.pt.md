---
lang: he
direction: rtl
source: docs/portal/docs/nexus/overview.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-overview
כותרת: Visao geral do Sora Nexus
תיאור: Resumo de alto nivel da arquitetura do Iroha 3 (Sora Nexus) com apontamentos para os docs canonicos do mono-repo.
---

Nexus (Iroha 3) estende Iroha 2 com execucao multi-lane, espacos de dados escopados por governanca e ferramentas compartilhadas em cada SDK. Esta pagina espelha o novo resumo `docs/source/nexus_overview.md` אין מונו-ריפו עבור que leitores do portal entendam rapidamente como as pecas da arquitetura se encaixam.

## לינה שחרור

- **Iroha 2** - implantacoes auto-hospedadas para consorcios ou redes privadas.
- **Iroha 3 / Sora Nexus** - רישום רב-נתיבי או מפעילי רישום espacos de dados (DS) e herdam ferramentas compartilhadas de governanca, liquidacao e observabilidade.
- אבטחה כמו חיבור למרחב עבודה (IVM + שרשרת כלים Kotodama), תקנות SDK, תקנות ABI ואביזרי Norito קבועים. Operadors baixam o bundle `iroha3-<version>-<os>.tar.zst` עבור כניסה לא Nexus; עיין ב-`docs/source/sora_nexus_operator_onboarding.md` עבור רשימת בדיקה עם טלה cheia.

## Blocos de construcao| רכיב | רזומה | פורטל Pontos do |
|--------|--------|----------------|
| Espaco de dados (DS) | Dominio de execucao/armazenamento definido pela governanca que possui uma ou mais lanes, declara conjuntos de validadores, classe de privacidade e politica de taxas + DA. | Veja [Nexus מפרט](./nexus-spec) למען המניפסט. |
| ליין | Shard deterministico de execucao; emite compromissos que o anel Global NPoS ordena. כמו מחלקות דה ליין כוללות `default_public`, `public_custom`, `private_permissioned` ו-`hybrid_confidential`. | O [דגם ליין](./nexus-lane-model) תפיסת גיאומטריה, קידומת ארמזנאמנטו ו-retencao. |
| Plano de transicao | מציין מיקום, שלבים של רוטממנטו e empacotamento de perfil duplo acompanham como implantacoes de lane unica evoluem para Nexus. | כמו [Notas de transicao](./nexus-transition-notes) תיעוד של שלב ה- Migracao. |
| ספריית החלל | Contrato de registro que armazena manifestos + versoes de DS. Operadores reconciliam entradas do catalogo com este diretorio antes de enrar. | O rastreador de diffs de manifesto vive em `docs/source/project_tracker/nexus_config_deltas/`. |
| קטלוג הנתיבים | A secao `[nexus]` de configuracao mapeia IDs de lane para aliases, politicas de roteamento e limiares de DA. `irohad --sora --config ... --trace-config` איפריים או פתרון קטלוגי עבור אודיטוריות. | השתמש ב-`docs/source/sora_nexus_operator_onboarding.md` para o passo a passo de CLI. |
| Roteador de liquidacao | Orquestrador de transferencia XOR que conecta lanes CBDC privadas com lanes de liquidez publicas. | `docs/source/cbdc_lane_playbook.md` ידיות מפורטות של פוליטיקה ושערים בטלמטריה. |
| Telemetria/SLOs | לוחות מחוונים + התראות ב-`dashboards/grafana/nexus_*.json` תפסו את הנתיבים, הצטברות של DA, השהייה של ליקוויד והפרופונדידדה דה פילה דה גוברננקה. | O [plano de remediacao de telemetria](./nexus-telemetry-remediation) לוחות מחוונים, התראות evidencias de auditoria. |

## תמונת מצב של השקה

| פאזה | פוקו | קריטריונים דה אמרה |
|-------|-------|-------------|
| N0 - Beta fechada | הרשם gerenciado pelo conselho (`.sora`), מדריך הפעלה למטוס, קטלוג מסלולים אסטטיים. | מניפסטים של DS assinados + מסרים של governanca ensaados. |
| N1 - Lancamento publico | Adiciona sufixos `.nexus`, leiloes, רשם שירות עצמי, cabeamento de liquidacao XOR. | Testes de sincronizacao de resolver/gateway, dashboards de reconciliacao de cobranca, exercicios de disputa em mesa. |
| N2 - Expansao | היכרות עם `.dao`, APIs de reenda, anality, portal de disputas, scorecards de stewards. | גרסאות תאימות, ערכת כלים של מושבעים פוליטית באינטרנט, relatorios de transparencia do tesouro. |
| שער NX-12/13/14 | תאימות מוטורית, לוחות מחוונים של טלמטריה ומסמכים מתקדמים. | [סקירה כללית של Nexus](./nexus-overview) + [Nexus פעולות](./nexus-operations) פרסומים, לוחות מחוונים ligados, motor de politica integrado. |

## Responsabilidades do operador1. **Higiene de configuracao** - mantenha `config/config.toml` sincronizado com o catalogo publicado de lanes e dataspaces; arquive a saida `--trace-config` עם כרטיס כניסה לשחרור.
2. **Rastreamento de Manifestos** - Reconcilie entradas do catalogo com o bundle mais recente do Space Directory antes de entradas ou atualizar nos.
3. **Cobertura de telemetria** - לוחות מחוונים של exponha os `nexus_lanes.json`, `nexus_settlement.json` e os dashboards de SDK relacionados; conecte alertas ao PagerDuty e rode revisoes trimestrais conforme o plano de remediacao de telemetria.
4. **Relato de incidentes** - siga a matriz de severidade em [Nexus operations](./nexus-operations) e entregue RCAs em ate cinco dias uteis.
5. **Prontidao de governanca** - participe de votos do conselho Nexus que impactam suas lanes e ensaie instrucoes de rollback trimestralmente (rastreadas via `docs/source/project_tracker/nexus_config_deltas/`).

## Veja tambem

- Visao canonica: `docs/source/nexus_overview.md`
- Especificacao detalhada: [./nexus-spec](./nexus-spec)
- Geometria de lanes: [./nexus-lane-model](./nexus-lane-model)
- Plano de transicao: [./nexus-transition-notes](./nexus-transition-notes)
- Plano de remediacao de telemetria: [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- ספר הפעלה: [./nexus-operations](./nexus-operations)
- Guia de onboarding de operators: `docs/source/sora_nexus_operator_onboarding.md`