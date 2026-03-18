---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-invite-flow.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Fluxo de convites לעשות תצוגה מקדימה

## אובייקטיבו

O פריט לעשות מפת הדרכים **DOCS-SORA** destaca o onboarding de revisores e o programa de convites לעשות תצוגה מקדימה ציבורית como os ultimos bloqueadores antes de o portal sair de beta. Esta pagina descreve como abrir cada onda de convites, quais artefatos devem ser enviados antes de mandar convites e como provar que o fluxo e auditavel. השתמש ב-junto com:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) para o manejo por revisor.
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) para garantias de checksum.
- [`devportal/observability`](./observability.md) עבור ייצוא של טלמטריה ו-hooks de alertas.

## פלאנו דה אונדס

| אונדה | Audiencia | קריטריונים דה אנטרדה | קריטריונים דה אמרה | Notas |
| --- | --- | --- | --- | --- |
| **W0 - ליבת תחזוקה** | Maintainers de Docs/SDK validando conteudo do dia um. | זמן GitHub `docs-portal-preview` populado, gate de checksum `npm run serve` verde, Alertmanager silencioso por 7 dias. | Todos os docs P0 revisados, backlog tagueado, sem incidentes bloqueadores. | Usado para validar o fluxo; sem email de convite, apenas compartilhar os artefatos de preview. |
| **W1 - שותפים** | Operadores SoraFS, אינטגרדורים Torii, בוחנים של הממשל בוחנים NDA. | W0 encerrado, termos legais aprovados, proxy Try-it em staging. | חתימת שותפים בשיתוף פעולה (בעיה או בנוסחא), טלמטריה <=10 בוחנים בהתאמה, סיום נסיגות של 14 ימים. | אפליקציית תבנית de convite + tickets de solicitacao. |
| **W2 - קומונידה** | תורמים לבחירה בקהילה. | התקנת W1, תרגילי תקריות, שאלות נפוצות. | משוב דיגרידו, >=2 מהדורות מסמכים דרך צינור תצוגה מקדימה סם החזרה לאחור. | Limitar convites concorrentes (<=25) e agrupar semanalmente. |

Documente qual onda esta ativa em `status.md` e no tracker de solicitacoes de preview para que a governanca veja o estado rapidamente.

## רשימת בדיקה מוקדמת

Conclua estas acoes **antes** de agendar convites para uma onda:

1. **Artefatos de CI disponiveis**
   - Ultimo `docs-portal-preview` + מתאר enviado por `.github/workflows/docs-portal-preview.yml`.
   - Pin de SoraFS anotado em `docs/portal/docs/devportal/deploy-guide.md` (מתאר de cutover presente).
2. **אכיפה של סכום ביקורת**
   - `docs/portal/scripts/serve-verified-preview.mjs` אינבוקדו דרך `npm run serve`.
   - הוראות `scripts/preview_verify.sh` ב-macOS + Linux.
3. **בסיס טלמטריה**
   - `dashboards/grafana/docs_portal.json` mostra trafego נסה את זה saudavel e o alerta `docs.preview.integrity` esta verde.
   - Ultimo apendice de `docs/portal/docs/devportal/observability.md` atualizado com קישורים לעשות Grafana.
4. **Artefatos de governanca**
   - בעיה עם הזמנת tracker pronta (uma issue por onda).
   - Template de registro de revisores copiado (ver [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - Aprovacoes legais e de SRE מחייב בעיה.

הירשם למסקנה לעשות טיסה מוקדמת ללא הזמנה מעקב לפני קביעת דוא"ל.

## Etapas do fluxo1. **מועמדים נבחרים**
   - Puxar da planilha de espera ou fila de partners.
   - Garantir que cada candidato tenha o template de solicitacao completo.
2. **Aprovar acesso**
   - Atribuir um aprovador בעיה לעשות מעקב אחר הזמנת.
   - תנאים מוקדמים לאמת (CLA/contrato, uso aceitavel, brief de seguranca).
3. **Enviar מזמין**
   - Preencher os placeholders de [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`, `<request_ticket>`, contatos).
   - Anexar o descriptor + hash do archive, URL de staging do נסה את זה, e canais de supporte.
   - שומר או דוא"ל סופי (או תמלול למטריקס/Slack) בבעיה.
4. **עלייה למטוס Acompanhar**
   - Atualizar או invite tracker com `invite_sent_at`, `expected_exit_at`, e status (`pending`, `active`, `complete`, SORA).
   - Linkar a solicitacao de entrada do revisor para auditabilidade.
5. **טלמטריה מוניטורית**
   - Observar `docs.preview.session_active` e alertas `TryItProxyErrors`.
   - Abrir um incidente se a telemetria desviar do baseline e registrar o resultado ao lado da entrada de convite.
6. **משוב קולטר eencerrar**
   - Encerrar מזמינה שוב ושוב משוב או `expected_exit_at` תפוגה.
   - Atualizar a issue da onda com um resumo curto (achados, incidentes, proximas acoes) antes de passar para a proxima coorte.

## Evidencia e reporting

| ארטפטו | Onde armazenar | Cadencia de atualizacao |
| --- | --- | --- |
| בעיה לעשות מעקב אחר הזמנות | Projeto GitHub `docs-portal-preview` | Atualizar apos cada convite. |
| ייצוא לרשימת רוסטרים | Registro vinculado em `docs/portal/docs/devportal/reviewer-onboarding.md` | סמנאל. |
| תמונות Snapshots de telemetria | `docs/source/sdk/android/readiness/dashboards/<date>/` (צרור מחדש של טלמטריה) | Por onda + תקריות אפוס. |
| תקציר משוב | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (Criar pasta por onda) | Dentro de 5 dias apos a saya da onda. |
| Nota de reuniao de governanca | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | Preencher antes de cada sync DOCS-SORA. |

בצע את `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
apos cada lote para produzir um digest legivel por maquina. Anexe o JSON renderizado a issue da onda para que revisores de governanca confirmem as contagens de convite sem reproduzir todo o log.

נספח לרשימת הוכחות ל-`status.md` המהווה את הקצה הסופי של מפת הדרכים לביצוע מהיר.

## קריטריונים להחזרה והפסקה

הפסקה o fluxo de convites (e notifique a governanca) quando qualquer um dos itens abaixo ocorrer:

- Incidente de proxy נסה את זה לאחר החזרה לאחור (`npm run manage:tryit-proxy`).
- Fadiga de alertas: >3 דפי התראה לנקודות קצה של תצוגה מקדימה בשבעה ימים.
- Gap de compliance: הזמינו את ה-enviado sem termos assinados או את הרשם או התבנית של solicitacao.
- Risco de integridade: אי התאמה של checksum detectado por `scripts/preview_verify.sh`.

Retome somente apos documentar a remediacao no order tracker e confirmar que o לוח המחוונים de telemetria esta estavel por pelo menos 48 horas.