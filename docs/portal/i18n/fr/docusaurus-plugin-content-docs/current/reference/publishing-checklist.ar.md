---
lang: fr
direction: ltr
source: docs/portal/docs/reference/publishing-checklist.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# قائمة التحقق للنشر

استخدم قائمة التحقق هذه كلما قمت بتحديث بوابة المطورين. Il existe des pages CI et GitHub qui vous permettent de fumer facilement et facilement.

## 1. التحقق المحلي

- `npm run sync-openapi -- --version=current --latest` (`npm run sync-openapi -- --version=current --latest` (`--mirror=<label>` ou Torii OpenAPI est utilisé).
- `npm run build` – Vous êtes le héros `Build on Iroha with confidence` et vous êtes `build/index.html`.
- `./docs/portal/scripts/preview_verify.sh --build-dir build` – Utiliser les sommes de contrôle (`--descriptor`/`--archive` pour les artefacts de CI).
- `npm run serve` – permet d'obtenir un aperçu de la somme de contrôle pour la vérification de la somme de contrôle `docusaurus serve`, Il s'agit d'un alias `serve:verified` qui correspond à votre numéro de téléphone.
- La démarque est disponible sur `npm run start` et le rechargement en direct.

## 2. فحوصات طلب السحب

- Utilisez le lien `docs-portal-build` vers `.github/workflows/check-docs.yml`.
- تاكد من تشغيل `ci/check_docs_portal.sh` (سجلات CI تظهر hero smoke check).
- Vous pouvez utiliser l'aperçu de la version (`build/checksums.sha256`) et vous connecter à l'aperçu de la version (`scripts/preview_verify.sh`).
- L'URL de votre site Web est également compatible avec les pages GitHub et les relations publiques.

## 3. اعتماد الاقسام| القسم | المالك | قائمة التحقق |
|---------|-------|---------------|
| Page d'accueil | DevRel | Il s'agit d'un héros et d'un guide de démarrage rapide ainsi que d'un CTA. |
| Norito | Norito GT | Voici une présentation et un démarrage rapide des drapeaux de la CLI et du Norito. |
| SoraFS | Équipe de stockage | يعمل quickstart حتى الاكتمال، وحقول تقرير manifest موثقة، وتعليمات محاكاة fetch متحققة. |
| Guides SDK | Voir SDK | Rust/Python/JS est un outil de développement rapide et efficace. |
| Référence | Docs/DevRel | Voici les spécifications du codec Norito et `norito.md`. |
| Aperçu de l'artefact | Docs/DevRel | Il s'agit de l'artefact `docs-portal-preview` pour PR, et de la fumée, ainsi que de la fumée. |
| Sécurité et test sandbox | Docs/DevRel · Sécurité | Vous pouvez également utiliser la connexion par code de périphérique OAuth (`DOCS_OAUTH_*`) et la connexion `security-hardening.md` pour les types CSP/Trusted Types. `npm run build` et `npm run probe:portal`. |

علّم كل صف كجزء من مراجعة PR، او دوّن اي مهام متابعة حتى يبقى تتبع الحالة دقيقا.

## 4. ملاحظات الاصدار

- ادرج `https://docs.iroha.tech/` (او عنوان بيئة النشر من مهمة النشر) pour les applications et les réseaux sociaux.
- اذكر اي اقسام جديدة او معدلة بوضوح حتى تعرف الفرق التابعة اين تعيد تشغيل اختبارات smoke الخاصة بها.