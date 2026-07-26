---
lang: pt
direction: ltr
source: docs/portal/docs/reference/publishing-checklist.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# قائمة التحقق للنشر

Verifique se o dispositivo está funcionando corretamente. تضمن ان بناء CI e GitHub Pages واختبارات smoke اليدوية تغطي كل قسم قبل وصول اصدار او محطة طريق.

## 1. التحقق المحلي

- `npm run sync-openapi -- --version=current --latest` (o modelo `--mirror=<label>` é compatível com Torii OpenAPI).
- `npm run build` – تحقق من ان نص الـ hero `Build on Iroha with confidence` não está disponível em `build/index.html`.
- `./docs/portal/scripts/preview_verify.sh --build-dir build` – gera somas de verificação (não `--descriptor`/`--archive` para artefatos de CI).
- `npm run serve` – A pré-visualização da soma de verificação é feita através do `docusaurus serve`. يتصفح المراجعون لقطة غير موقعة (يبقى alias `serve:verified` متاحا للاستدعاءات الصريحة).
- راجع الـ markdown الذي لمسته عبر `npm run start` e recarregar ao vivo.

## 2. فحوصات طلب السحب

- Use o `docs-portal-build` para `.github/workflows/check-docs.yml`.
- تاكد من تشغيل `ci/check_docs_portal.sh` (سجلات CI تظهر hero smoke check).
- تاكد ان سير عمل preview رفع بيانا (`build/checksums.sha256`) وان سكربت التحقق من preview نجح (تظهر السجلات خرج `scripts/preview_verify.sh`).
- Este é o URL do site do GitHub Pages e do PR.

## 3. اعتماد الاقسام

| القسم | المالك | قائمة التحقق |
|--------|-------|-----------|
| Página inicial | DevRel | يظهر نص الـ hero, وبطاقات quickstart تربط بمسارات صالحة, وازرار CTA تعمل. |
| Norito | Norito WG | Veja a visão geral e os primeiros passos. Use flags para CLI e use Norito. |
| SoraFS | Equipe de armazenamento | يعمل quickstart حتى الاكتمال, وحقول تقرير manifest موثقة, وتعليمات محاكاة fetch متحققة. |
| Guias do SDK | SDK do SDK | Usar Rust/Python/JS é uma tarefa que pode ser executada sem problemas. |
| Referência | Documentos/DevRel | Confira as especificações, e o codec Norito é o `norito.md`. |
| Artefato de visualização | Documentos/DevRel | Use o artefato `docs-portal-preview` para usar PR, e para remover fumaça, e para remover a fumaça do cigarro. |
| Segurança e experimente sandbox | Documentos/DevRel · Segurança | Para usar o login de código de dispositivo OAuth (`DOCS_OAUTH_*`), use o `security-hardening.md` e use CSP/Trusted Types `npm run build` e `npm run probe:portal`. |

Você pode fazer isso em um PR, e uma pessoa que não pode pagar por isso.

## 4. ملاحظات الاصدار

- Use `https://docs.iroha.tech/` (você pode usar o código de barras) para obter mais informações e informações.
- Evite fumaça e fumaça Não.