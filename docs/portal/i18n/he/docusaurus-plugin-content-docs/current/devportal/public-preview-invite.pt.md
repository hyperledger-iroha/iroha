---
lang: he
direction: rtl
source: docs/portal/docs/devportal/public-preview-invite.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Playbook de convites לעשות תצוגה מקדימה של publico

## אובייקטיבוס לעשות תוכניות

Este playbook explica como ununciar e executar o תצוגה מקדימה publico assim que o
זרימת עבודה של onboarding de revisores estiver ativo. חלק מפת הדרכים DOCS-SORA honesto ao
garantir que cada convite saia com artefatos verificavis, orientacao de seguranca e um
caminho claro de feedback.

- **Audiencia:** רשימה של ממברס da comunidade, שותפים ומנהלים que assinaram a
  politica de uso aceitavel לעשות תצוגה מקדימה.
- **מגבלות:** tamanho de onda padrao <= 25 revisores, janela de acesso de 14 dias, resposta
  מקרים בהם 24 שעות.

## רשימת רשימות לשער דה לנקמנטו

השלם estas tarefas ante de enviar qualquer convite:

1. Ultimos artefatos de preview enviados na CI (`docs-portal-preview`,
   Manifest de Checksum, Descriptor, Bundle SoraFS).
2. `npm run --prefix docs/portal serve` (gateado por checksum) testado no mesmo tag.
3. כרטיסים de onboarding de revisores aprovados e vinculados a onda de convites.
4. Docs de seguranca, observabilidade e incidentes validados
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. נוסחת משוב או תבנית הכנה בנושא (כולל campos de severidade,
   passos de reproducao, צילומי מסך e info de ambiente).
6. העתק do anuncio revisada por Docs/DevRel + Governance.

## Pacote de convite

Cada convite deve כולל:

1. **Artefatos verificados** - Forneca links para o manifest/plan SoraFS או para os artefatos
   GitHub, mais o manifest de checksum e o descriptor. Referencecie explicitamente o comando
   de verificacao para que os revisores possam executa-lo antes de subir o site.
2. **הוראות לשרת** - כולל הוראות תצוגה מקדימה לפי סיכום:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Lembretes de seguranca** - Informe que tokens expiram automaticamente, links nao devem ser
   התקדמות ותקריות התפתחו לדיווחים מיידיים.
4. **Canal de feedback** - Linke o template/formulario e esclareca expectativas de tempo de resposta.
5. **Datas do programa** - מידע על תחילת העבודה, שעות המשרד או סנכרון, וסמוך
   ג'אנלה דה רענן.

O email de exemplo em
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
cobre esses requisitos. להטמיע את מצייני המיקום של מערכת ההפעלה (נתונים, כתובות אתרים, הגדרות)
antes de enviar.

## Expor או מארח התצוגה המקדימה

אז קידום או מארח תצוגה מקדימה quando o onboarding estiver completo e o ticket de mudanca estiver
אפרודו. Veja o [guia de exposicao do host de preview](./preview-host-exposure.md) para os passos
מקצה לקצה de build/publish/verify usados nesta secao.

1. **בנה ותג:** תג שחרור והמוצרים המוגדרים.

   ```bash
   cd docs/portal
   export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
   npm ci
   npm run build
   ./scripts/sorafs-pin-release.sh \
     --alias docs-preview.sora \
     --alias-namespace docs \
     --alias-name preview \
     --pin-label docs-preview \
     --skip-submit
   node scripts/generate-preview-descriptor.mjs \
     --manifest artifacts/checksums.sha256 \
     --archive artifacts/sorafs/portal.tar.gz \
     --out artifacts/sorafs/preview-descriptor.json
   ```

   O script de pin grava `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   e `portal.dns-cutover.json` em `artifacts/sorafs/`. Anexe esses arquivos a onda de convites
   para que cada revisor possa verificar os mesmos bits.2. **Publicar o alias de preview:** Rode o comando sem `--skip-submit`
   (forneca `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`, e a prova de alias emitida
   Pela governanca). O script vai amarrar o manifest a `docs-preview.sora` e emitir
   `portal.manifest.submit.summary.json` mais `portal.pin.report.json` para o bundle de evidencias.

3. **בוחן פריסה:** אשר את פתרון הכינוי או תג הבדיקה
   antes de enviar convites.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   Mantenha `npm run serve` (`scripts/serve-verified-preview.mjs`) a mao como fallback para
   que revisores possam subir uma copia local se o edge de preview falhar.

## קו זמן של comunicacao

| דיא | Acao | בעלים |
| --- | --- | --- |
| D-3 | גמר עותק לעשות זימון, אטואליזר artefatos, הפעלה יבשה de verificacao | Docs/DevRel |
| D-2 | Sign-off de governanca + ticket de mudanca | Docs/DevRel + ממשל |
| D-1 | Enviar מזמינה את Usando או תבנית, אטואליסר מעקב com List destinatarios | Docs/DevRel |
| ד | שיחת בעיטה / שעות עבודה, לוחות מחוונים לניטור של טלמטריה | Docs/DevRel + כוננות |
| D+7 | תקציר משוב לא מאיו דה אונדה, טריאז' דה בעיות bloqueantes | Docs/DevRel |
| D+14 | Fechar a onda, revogar acesso temporario, resumo public em `status.md` | Docs/DevRel |

## מעקב אחר טלמטריה

1. הרשמו את cada destinatario, timestamp de convite e data de revogacao com o
   לוגר משוב לתצוגה מקדימה (veja
   [`preview-feedback-log`](./preview-feedback-log)) para que cada onda compartilhe o mesmo
   ראסטרו דה evidencias:

   ```bash
   # Adicione um novo evento de convite a artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   תמיכה באירועים של `invite-sent`, `acknowledged`,
   `feedback-submitted`, `issue-opened`, ו-`access-revoked`. O log fica em
   `artifacts/docs_portal_preview/feedback_log.json` por padrao; anexe ao ticket da
   onda de convites junto com os formularios de consentimento. השתמש ב-o helper de
   תקציר עבור פרודוזיר אום רול-אפ auditavel ante da not de encerramento:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   O סיכום JSON enumera מזמין por onda, destinatarios abertos, contagens de
   משוב וחותמת זמן לעשות את האירוע האחרון. הו עוזר e apoiado por
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   portanto o mesmo workflow pode rodar localmente ou em CI. השתמש ב-o template de digest em
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   ao publicar o recap da onda.
2. לוחות המחוונים של Tague OS de telemetria com o `DOCS_RELEASE_TAG` בארה"ב
   picos possam ser correlacionados com as coortes de convite.
3. Rode `npm run probe:portal -- --expect-release=<tag>` apos o deploy para confirmar que
   o ambiente de preview a uncia a metadata correta de release.
4. הרשמה qualquer incidente אין תבנית לעשות runbook e vincule a coorte.

## משוב e fechamento1. הסכימו למשוב על מסמך התייחסות ללוח הנושאים. Marque os itens com
   `docs-preview/<wave>` עבור הבעלים של מערכות הפעלה מבצעים מפת דרכים סיוע.
2. השתמש בסיכום של saida do preview logger para preencher o relatorio da onda, depois
   resuma a coorte em `status.md` (משתתפים, principais achados, fixes planejados) e
   לממש את `roadmap.md` se o marco DOCS-SORA mudou.
3. Siga os passos de offboarding de
   [`reviewer-onboarding`](./reviewer-onboarding.md): revogue acesso, arquive solicitacoes e
   משתתפי agradeca os.
4. הכן proxima onda atualizando artefatos, reexecutando os gates de checksum e
   אטואליזנדו או תבנית הזמנת com novas נתונים.

אפליקציית ספר הפעלה של פורמה עקבית או תוכנית תצוגה מקדימה לביקורת ה
oferece ao Docs/DevRel um caminho repetivel para escalar convites a medida que o portal
se aproxima de GA.