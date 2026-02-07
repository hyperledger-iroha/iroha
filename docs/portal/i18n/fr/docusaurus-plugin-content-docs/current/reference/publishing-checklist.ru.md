---
lang: fr
direction: ltr
source: docs/portal/docs/reference/publishing-checklist.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Liste de contrôle des publications du portail
description : Les preuves avant la mise à jour du portail de documentation Iroha.
---

Utilisez cette liste de contrôle pour chaque mise à jour du portail du robot. Он гарантирует, что
L'entreprise CI, en déployant des pages GitHub et en effectuant des tests de fumée, a effectué nos recherches avant la publication
ou la feuille de route de votre véhicule.

## 1. Province locale

- `npm run sync-openapi -- --version=current --latest` (pour une personne ou un autre
  (`--mirror=<label>`, ou Torii OpenAPI est disponible pour l'instantané).
- `npm run build` — убедитесь, что слоган « Construisez sur Iroha en toute confiance » всё ещё
  отображается в `build/index.html`.
- `./docs/portal/scripts/preview_verify.sh --build-dir build` — vérifier le manifeste
  checksum (utilisez `--descriptor`/`--archive` pour tester les articles CI).
- `npm run serve` : activez la fonction d'assistance avec la somme de contrôle vérifiée, qui est validée
  manifeste avant l'arrivée du `docusaurus serve`, les rapports sur le Nigéria ne sont pas affichés sur
  неподписанный snapshot (alias `serve:verified` остаётся для явных вызовов).
- Créez de nouveaux modèles de démarques à partir de `npm run start` et un serveur de développement avec live-reload.

## 2. Vérifier les pull‑requests- Veuillez noter que le travail `docs-portal-build` est prévu pour
  `.github/workflows/check-docs.yml`.
- Vérifiez que c'est le `ci/check_docs_portal.sh` (dans les journaux CI qui fournissent le héros).
- Ajoutez que l'aperçu du flux de travail a été enregistré dans le manifeste (`build/checksums.sha256`) et cela
  скрипт проверки aperçu выполнился успешно (logiciel CI содержат вывод
  `scripts/preview_verify.sh`).
- Ajoutez l'URL d'aperçu officielle à partir des pages GitHub dans la description des relations publiques.

## 3. Подписание по разделам| Rasdel | Владелец | Liste de contrôle |
|--------|----------|--------------|
| Glavna | DevRel | Hero‑копирайт отображается; les cartes de démarrage rapide ведут на валидные маршруты ; Boutons CTA. |
| Norito | Norito GT | Les informations sur les robots sont basées sur le schéma CLI réel et le schéma Norito. |
| SoraFS | Équipe de stockage | Le démarrage rapide vous permet de trouver des informations sur les manifestes de documents, des instructions pour récupérer des preuves simultanées. |
| SDK-gauches | SDK Web | Rust/Python/JS-Gays utilisent des modèles réels et sont développés pour vos dépôts. |
| Référence | Docs/DevRel | L'index concerne les spécifications spécifiques, en utilisant le codec Norito, le codec `norito.md`. |
| Aperçu‑artéfact | Docs/DevRel | L'article `docs-portal-preview` est utilisé avec PR, un anti-fumée fourni pour les rapports. |
| Sécurité et test sandbox | Docs/DevRel · Sécurité | La connexion par code de périphérique OAuth (`DOCS_OAUTH_*`), la liste de contrôle `security-hardening.md` ont été enregistrées, les types CSP/Trusted Types ont été vérifiés par `npm run build` ou `npm run probe:portal`. |

Отметьте каждую строку в ходе review PR'а ou зафиксируйте follow-up-zadachis, чтобы STATUS
оставался точным.

## 4. Notes de version- Cliquez sur `https://docs.iroha.tech/` (ou sur l'URL obtenue lors du déploiement du travail) dans les notes de version
  и STATUSные апдейты.
- Vous devez acheter de nouveaux produits ou des services supplémentaires, qui sont indiqués en aval,
  какие smoke-testы им требуется перезапустить.