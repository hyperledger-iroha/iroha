---
lang: fr
direction: ltr
source: docs/examples/docs_preview_invite_email.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4e3856058310e40649d5394996b2bcbfde99effb9e706be87f284e1812d5bdbd
source_last_modified: "2025-11-15T04:49:30.881970+00:00"
translation_last_reviewed: 2026-01-01
---

# Invitation preview du portail docs (Email exemple)

Utilisez cet exemple pour rediger le message sortant. Il capture le texte exact
envoye aux relecteurs de la communaute W2 (`preview-2025-06-15`) afin que les
prochaines vagues puissent reproduire le ton, la guidance de verification et la
trace de preuves sans reconstituer d'anciens tickets. Mettez a jour les liens
d'artefacts, les hashes, les IDs de demande et les dates avant d'envoyer une
nouvelle invitation.

```text
Objet: [DOCS-SORA] invitation preview du portail docs preview-2025-06-15 pour Horizon Wallet

Bonjour Sam,

Merci encore d'avoir propose Horizon Wallet pour la preview communautaire W2. La vague
W2 est maintenant validee, donc vous pouvez commencer votre revue des que vous terminez
les etapes ci-dessous. Gardez les artefacts et tokens d'acces prives: chaque invitation
est tracee dans DOCS-SORA-Preview-W2 et les auditeurs verifieront les accus.

1. Telechargez les artefacts verifies (les memes bits que nous avons livres a SoraFS et CI):
   - Descriptor: https://sorafs-gateway.sora/docs-preview/preview-2025-06-15/descriptor.json (`sha256:a1f41cfb02a5f34f2a0e6535f0b079dbb645c1b5dcdbcb36f953ef5c418260ad`)
   - Archive: https://sorafs-gateway.sora/docs-preview/preview-2025-06-15/docs-portal-preview.tar.zst (`sha256:5bc30261fa3c0db032ac2b3c4b56651bebcd309d69a2634ebc9a6f0da3435399`)
2. Verifiez le bundle avant extraction:

   ./docs/portal/scripts/preview_verify.sh      --descriptor ~/Downloads/descriptor.json      --archive ~/Downloads/docs-portal-preview.tar.zst      --build-dir ~/sora-docs/preview-2025-06-15

3. Servez la preview avec enforcement de checksum:

   DOCS_RELEASE_TAG=preview-2025-06-15 npm run --prefix docs/portal serve

4. Revoyez les runbooks renforces avant les tests:
   - docs/portal/docs/devportal/security-hardening.md
   - docs/portal/docs/devportal/observability.md
   - docs/portal/docs/devportal/reviewer-onboarding.md

5. Deposez le feedback via DOCS-SORA-Preview-REQ-C04 et taguez chaque constat avec
   `docs-preview/w2`. Utilisez le formulaire de feedback si vous preferez un intake structure:
   docs/examples/docs_preview_feedback_form.md.

Support disponible sur Matrix (`#docs-preview:matrix.org`) et nous tenons des office hours
le 2025-06-18 15:00 UTC. Pour les escalades securite ou incident, pagez l'alias on-call
docs via ops@sora.org ou +1-555-0109 immediatement; ne pas attendre les office hours.

L'acces preview pour Horizon Wallet court 2025-06-15 -> 2025-06-29. Dites-nous des que
vous avez termine afin que nous puissions revoquer les cles d'acces temporaires et
consigner la cloture dans le tracker.

Merci pour votre aide pour amener le portail au GA!

- Equipe DOCS-SORA
```
