---
lang: fr
direction: ltr
source: docs/examples/docs_preview_invite_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6c819c8d2a9517f1235a66a4661efd061a166ea89c953fd599e102b3cfd9157b
source_last_modified: "2025-11-10T18:08:48.050596+00:00"
translation_last_reviewed: 2026-01-01
---

# Invitation preview du portail docs (Modele)

Utilisez ce modele pour envoyer les instructions d'acces preview aux relecteurs. Remplacez
les placeholders (`<...>`) par les valeurs pertinentes, joignez les artefacts descriptor +
archive mentionnes dans le message, et stockez le texte final dans le ticket d'intake
correspondant.

```text
Objet: [DOCS-SORA] invitation preview du portail docs <preview_tag> pour <reviewer/org>

Bonjour <name>,

Merci d'avoir propose de relire le portail docs avant GA. Vous etes autorise
pour la vague <wave_id>. Suivez les etapes ci-dessous avant de parcourir la preview:

1. Telechargez les artefacts verifies depuis CI ou SoraFS:
   - Descriptor: <descriptor_url> (`sha256:<descriptor_sha256>`)
   - Archive: <archive_url> (`sha256:<archive_sha256>`)
2. Executez le gate de checksum:

   ./docs/portal/scripts/preview_verify.sh      --descriptor <path-to-descriptor>      --archive <path-to-archive>      --build-dir <path-to-extracted-build>

3. Servez la preview avec enforcement de checksum active:

   DOCS_RELEASE_TAG=<preview_tag> npm run --prefix docs/portal serve

4. Lisez les notes d'usage acceptable, securite et observabilite:
   - docs/portal/docs/devportal/security-hardening.md
   - docs/portal/docs/devportal/observability.md
   - docs/portal/docs/devportal/reviewer-onboarding.md

5. Deposez le feedback via <request_ticket> et taguez chaque constat avec `<preview_tag>`.

Support disponible sur <contact_channel>. Les incidents ou sujets de securite doivent etre
signales immediatement via <incident_channel>. Si vous avez besoin de tokens API Torii,
demandez-les via le ticket; ne reutilisez jamais les credentials de production.

L'acces preview expire le <end_date> sauf extension ecrite. Nous journalisons
les checksums et metadonnees d'invitation pour la governance; dites-nous quand vous avez termine
afin que nous puissions vous retirer proprement.

Merci encore pour votre aide a stabiliser le portail!

- Equipe DOCS-SORA
```
