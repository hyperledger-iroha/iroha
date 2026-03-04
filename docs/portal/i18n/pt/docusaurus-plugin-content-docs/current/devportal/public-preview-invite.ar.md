---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# دليل دعوات المعاينة العامة

## اهداف البرنامج

Não há nada que você possa fazer e que você possa usar para obter mais informações. يحافظ على
O produto DOCS-SORA é um produto de alta qualidade que pode ser usado para evitar problemas de segurança. امنية, ومسارا
واضحا للتغذية الراجعة.

- **الجمهور:** قائمة منقحة من اعضاء المجتمع والشركاء والـ mantenedores الذين وقعوا سياسة الاستخدام المقبول Não.
- **الحدود:** حجم الموجة الافتراضي <= 25 meses, نافذة وصول 14 يوما, استجابة للحوادث خلال 24h.

## قائمة فحص بوابة الاطلاق

اكملهذه المهام قبل ارسال اي دعوة:

1. Verifique o valor do arquivo no CI (`docs-portal-preview`,
   soma de verificação do manifesto, descritor, pacote SoraFS).
2. `npm run --prefix docs/portal serve` (soma de verificação) para definir sua tag.
3. Verifique o valor do produto e o valor do produto.
4. مستندات الامن والمراقبة والحوادث مؤكدة
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. O feedback e o problema do problema (يتضمن حقول الشدة, خطوات اعادة الانتاج, لقطات شاشة, ومعلومات البيئة).
6. نسخة الاعلان تمت مراجعتها من Docs/DevRel + Governance.

## حزمة الدعوة

Isso é algo que você pode fazer:

1. **Como criar um arquivo** — Crie um manifesto/plano para SoraFS e artefato no GitHub
   Use a soma de verificação e o descritor do manifesto. اذكر امر التحقق صراحة حتى يتمكن المراجعون
   Isso é tudo que você precisa saber.
2. **تعليمات التشغيل** — ادرج امر المعاينة المقيد بالchecksum:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **تذكيرات امنية** — اوضح ان الرموز تنتهي تلقائيا, والروابط لا يجب مشاركتها,
   ويجب الابلاغ عن الحوادث فورا.
4. **feedback de comentários** — Problema de اربط نموذج/قالب ووضح توقعات زمن الاستجابة.
5. **تواريخ البرنامج** — قدم تواريخ البداية/النهاية, ساعات المكتب او جلسات sincronização,
   ونافذة التحديث التالية.

البريد النموذجي
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
Não é possível. حدّث العناصر النائبة (URLs, e URLs)
Então.

## اظهار مضيف المعاينة

Não se esqueça de usar o cabo de alimentação e descarregar o produto. راجع
[دليل اظهار مضيف المعاينة](./preview-host-exposure.md) para construir/publicar/verificar com o nome de usuário
Isso é o que acontece.

1. **البناء والتعبئة:** ضع release tag وانتج اثارا حتمية.

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

   Pino de pino `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   O `portal.dns-cutover.json` é o `artifacts/sorafs/`. ارفق هذه الملفات بموجة الدعوة
   Verifique se o dinheiro está no lugar certo.

2. **نشر alias المعاينة:** اعد تشغيل الامر بدون `--skip-submit`
   (`TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`, e também conhecido como الصادر عن الحوكمة).
   سيقوم السكربت بربط manifesto بـ `docs-preview.sora` ويصدر
   `portal.manifest.submit.summary.json` e `portal.pin.report.json` são necessários.

3. **فحص النشر:** تاكد من ان alias يحل وانه checksum يطابق tag قبل ارسال الدعوات.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   Use o `npm run serve` (`scripts/serve-verified-preview.mjs`) para obter um substituto
   Você pode usar o recurso de pré-visualização na borda.

## جدول الاتصالات

| اليوم | الاجراء | Proprietário |
| --- | --- | --- |
| D-3 | Máquinas de teste de funcionamento em seco | Documentos/DevRel |
| D-2 | موافقة الحوكمة + تذكرة تغيير | Documentos/DevRel + Governança |
| D-1 | ارسال الدعوات باستخدام القالب, تحديث rastreador بقائمة المستلمين | Documentos/DevRel |
| D | مكالمة kickoff / horário comercial, مراقبة لوحات القياس | Documentos/DevRel + plantão |
| D+7 | digest للfeedback في منتصف الموجة, triagem للقضايا المانعة | Documentos/DevRel |
| D+14 | Faça o download do arquivo em `status.md` | Documentos/DevRel |

## تتبع الوصول والقياس عن بعد

1. سجل كل مستلم وطابع وقت الدعوة وتاريخ الالغاء باستخدام visualizar o registrador de feedback
   (انظر [`preview-feedback-log`](./preview-feedback-log)) حتى تشترك كل موجة
   Aqui está o seguinte:

   ```bash
   # اضافة حدث دعوة جديد الى artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   O código de barras é `invite-sent`, `acknowledged`, `feedback-submitted`,
   `issue-opened`, e`access-revoked`. يعيش السجل افتراضيا في
   `artifacts/docs_portal_preview/feedback_log.json`; ارفقه بتذكرة موجة الدعوة
   مع نماذج الموافقة. استخدم مساعد summary لانتاج roll-up قابل للتدقيق قبل
   O que fazer:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   resumo JSON يعد الدعوات لكل موجة, المستلمين المفتوحين, feedback, e feedback
   Você não pode. المساعد يعتمد على
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   Não há fluxo de trabalho do tipo CI. Como fazer um resumo em
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   Esta é uma recapitulação do seguinte.
2. Use o método `DOCS_RELEASE_TAG` para obter informações sobre o produto.
   Os grupos são de coorte.
3. Verifique `npm run probe:portal -- --expect-release=<tag>` para obter mais informações
   Não há metadados em metadados.
4. Coloque-o no runbook e execute-o.

## feedback1. Obtenha feedback sobre quaisquer problemas e problemas. O modelo `docs-preview/<wave>` está pronto
   Crie um roteiro para você.
2. استخدم مخرجات resumo do registrador de visualização
   `status.md` (não disponível), mas também `roadmap.md`
   Use o produto DOCS-SORA.
3. Como desligar o embarque aqui
   [`reviewer-onboarding`](./reviewer-onboarding.md)
4. جهز الموجة التالية عبر تحديث الاثار, اعادة تشغيل gates الخاصة بالchecksum,
   وتحديث قالب الدعوة بتواريخ جديدة.

Docs/DevRel
طريقة قابلة للتكرار لتوسيع الدعوات مع اقتراب البوابة من GA.