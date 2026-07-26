---
lang: he
direction: rtl
source: docs/portal/docs/devportal/reviewer-onboarding.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Onboarding de revisores לעשות תצוגה מקדימה

## Visao Geral

DOCS-SORA מלווה את השלבים בפורטל הפיתוח. בונה com gate de checksum
(`npm run serve`) e fluxos נסה את זה reforcados destravam o proximo marco:
onboarding de revisores validados antes de o תצוגה מקדימה publico se abrir amplamente. אסטה גויה
descreve como coletar solicitacoes, verificar elegibilidade, provisor acesso e fazer offboard
de participantes com seguranca. להתייעץ עם o
[תצוגה מקדימה של זרימת הזמנה](./preview-invite-flow.md) para o planejamento de coortes, a
cadencia de convites e exports de telemetria; os passos abaixo focam nas acoes
a tomar quando um revisor ja foi selecionado.

- **Escopo:** revisores que precisam de acesso ao preview de docs (`docs-preview.sora`,
  בונה לעשות GitHub Pages או חבילות של SoraFS) antes de GA.
- **Fora do escopo:** operadores de Torii או SoraFS (cobertos por seus proprios kits de onboarding)
  e implantacoes do portal em producao (ver
  [`devportal/deploy-guide`](./deploy-guide.md)).

## דרישות מוקדמות

| פאפל | Objetivos tipicos | Artefatos requeridos | Notas |
| --- | --- | --- | --- |
| מתחזק ליבה | אימות חדש, מבצע בדיקות עשן. | ידית GitHub, משולבת במטריקס, CLA מבוססת על ארכיון. | Geralmente ja esta no time GitHub `docs-preview`; ainda assim registre uma solicitacao para que o acesso seja auditavel. |
| סוקר שותף | קטעי קוד תקפים של SDK או קודמות הממשל לפני השחרור. | דוא"ל corporativo, POC משפטי, מונחי תצוגה מקדימה של מתאבדים. | פיתח מחדש את דרישות הטלמטריה + טרטאמנטו דה דאדוס. |
| מתנדב קהילתי | Fornecer feedback de usabilidade sobre guias. | ידית GitHub, contato preferido, fuso horario, aceitacao do CoC. | Mantenha coortes pequenas; עדיפויות revisores que assinaram o acordo de contribuicao. |

טיפים למנהלי חשבונות:

1. בדוק מחדש את המדיניות המקדימה.
2. Ler os apendices de seguranca/observabilidade
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. Concordar em executar `docs/portal/scripts/preview_verify.sh` antes de servir qualquer
   תמונת מצב מקומית.

## צריכת זרימה1. Pedir ao solicitante que preencha o
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   פורמולאריו (בעיית עותק/קולר אם אומה). Capturar ao Menos: Identidade, Methodo de Contato,
   ידית GitHub, נתונים מקדימים של revisao e confirmacao de que os docs de seguranca foram lidos.
2. רשם א solicitacao no tracker `docs-preview` (הנפק GitHub או ticket de governanca)
   e atribuir um aprovador.
3. דרישות קדם תקפות:
   - CLA / acordo de contribuicao em arquivo (ou referencia de contrato partner).
   - Reconhecimento de uso aceitavel armazenado na solicitacao.
   - Avaliacao de risco completa (לדוגמה, שותף revisores aprovados pelo Legal).
4. O aprovador faz o sign-off na solicitacao e vincula a issue de tracking a qualquer entrada de
   ניהול שינויים (דוגמה: `DOCS-SORA-Preview-####`).

## Provisionamento e ferramentas

1. **Compartilhar artefatos** - Fornecer o descriptor + arquivo de preview mais recente do workflow
   de CI ou do pin SoraFS (artefato `docs-portal-preview`). Lembrar os revisores de executar:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **Servir com enforcement de checksum** - Apontar os revisores para o comando com gate de checksum:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   Isso reutiliza `scripts/serve-verified-preview.mjs` para que nenhum build nao verificado
   seja iniciado por acidente.

3. **Conceder acesso GitHub (אופציונלי)** - Se revisores precisarem de branches nao publicadas,
   adicona-los ao זמן GitHub `docs-preview` לאחר בדיקה ורשם וחברות
   na solicitacao.

4. **Comunicar canais de suporte** - שיתוף פעולה בטלפון כוננות (מטריקס/Slack) ופרוצדורה
   de incidentes de [`incident-runbooks`](./incident-runbooks.md).

5. **טלמטריה + משוב** - Lembrar os revisores que analytics anonimizada e coletada
   (ver [`observability`](./observability.md)). Fornecer או נוסחת משוב או תבנית לבעיה
   citado no convite e registrar o evento com o helper
   [`preview-feedback-log`](./preview-feedback-log) למען חידושים.

## רשימת רשימת מבקר

הוכחות או תצוגה מקדימה, מבקרים התפתחו:

1. Verificar os artefatos baixados (`preview_verify.sh`).
2. התחלת הפורטל דרך `npm run serve` (ou `serve:verified`) למען הבטחת השמירה על הבדיקה.
3. Ler as notas de seguranca e observabilidade vinculadas acima.
4. בדוק את המסוף OAuth/נסה זאת בשימוש בכניסה לקוד התקן (לפי יישום) evitar reusal tokens de producao.
5. הרשם achados no tracker acordado (הנפקה, doc compartilhado ou formulario) e taguea-los com
   o tag de release לעשות תצוגה מקדימה.

## אחריות לתחזוקה ויציאה מהמטוס| פאזה | Acoes |
| --- | --- |
| בעיטה | Confirmar que a checklist de intake esta anexada a solicitacao, compartilhar artefatos + instrucoes, adicionar uma entrada `invite-sent` via [`preview-feedback-log`](./preview-feedback-log), e mei de maio de uma umar period sync סמנה. |
| ניטור | צג טלמטריה מקדימה (לשיג תקלות נסה את זה, בדוק את הבדיקה) ובדוק את ספר התקריות. הרשם אירועי `feedback-submitted`/`issue-opened` תואם את שאר הפעולות כמטרות מדויקות. |
| יציאה למטוס | Revogar acesso temporario a GitHub ou SoraFS, רשם `access-revoked`, arquivar a solicitacao (כולל רזומה של משוב + אקוות תלויות), eatualizer או registro de revisores. Solicitar ao revisor que remova builds locais e anexar o digest gerado a partir de [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md). |

השתמש ב-Mesmo Processo או Rotacionar revisores Entre Ondas. Manter או Rastro no repo (הנפקה + תבניות)
ajuda o DOCS-SORA a permanecer auditavel e permite que a governanca confirme que o acesso de preview
seguiu os controls documentados.

## תבניות de convite e מעקב

- תחילת העבודה עם הסברה
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md).
  Ele captura o minimo de linguagem legal, instrucoes de checksum de preview e a expectativa de que
  revisores reconhecam a politica de uso aceitavel.
- Ao editar o template, replacea os placeholders de `<preview_tag>`, `<request_ticket>` e canais de contato.
  Guarde uma copia da mensagem final no ticket de intake para que revisores, aprovadores e auditores possam
  התייחסות או טקסטו אקסטו.
- Depois de enviar או להזמין, להגדיר תוכנית מעקב או בעיה עם חותמת זמן `invite_sent_at` e a data
  de encerramento esperada para que o relatorio
  [תצוגה מקדימה של הזמנת זרימת](./preview-invite-flow.md) possa identificar a coorte automaticamente.