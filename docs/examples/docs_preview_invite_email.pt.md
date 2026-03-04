---
lang: pt
direction: ltr
source: docs/examples/docs_preview_invite_email.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4e3856058310e40649d5394996b2bcbfde99effb9e706be87f284e1812d5bdbd
source_last_modified: "2025-11-15T04:49:30.881970+00:00"
translation_last_reviewed: 2026-01-01
---

# Convite de preview do portal de docs (Email de exemplo)

Use este exemplo ao redigir a mensagem de saida. Ele captura o texto exato enviado
aos revisores da comunidade W2 (`preview-2025-06-15`) para que ondas futuras possam
replicar o tom, a orientacao de verificacao e o rastro de evidencia sem precisar
reconstituir tickets antigos. Atualize os links de artefatos, hashes, IDs de
solicitacao e datas antes de enviar um novo convite.

```text
Assunto: [DOCS-SORA] convite de preview do portal de docs preview-2025-06-15 para Horizon Wallet

Oi Sam,

Obrigado novamente por oferecer a Horizon Wallet para a preview comunitaria W2. A onda
W2 ja esta aprovada, entao voce pode iniciar a revisao assim que concluir os passos
abaixo. Mantenha os artefatos e tokens de acesso privados: cada convite e registrado
em DOCS-SORA-Preview-W2 e os auditores conferirao os reconhecimentos.

1. Baixe os artefatos verificados (os mesmos bits enviados a SoraFS e CI):
   - Descriptor: https://sorafs-gateway.sora/docs-preview/preview-2025-06-15/descriptor.json (`sha256:a1f41cfb02a5f34f2a0e6535f0b079dbb645c1b5dcdbcb36f953ef5c418260ad`)
   - Archive: https://sorafs-gateway.sora/docs-preview/preview-2025-06-15/docs-portal-preview.tar.zst (`sha256:5bc30261fa3c0db032ac2b3c4b56651bebcd309d69a2634ebc9a6f0da3435399`)
2. Verifique o bundle antes de extrair:

   ./docs/portal/scripts/preview_verify.sh      --descriptor ~/Downloads/descriptor.json      --archive ~/Downloads/docs-portal-preview.tar.zst      --build-dir ~/sora-docs/preview-2025-06-15

3. Sirva a preview com enforcement de checksum:

   DOCS_RELEASE_TAG=preview-2025-06-15 npm run --prefix docs/portal serve

4. Revise os runbooks reforcados antes de testar:
   - docs/portal/docs/devportal/security-hardening.md
   - docs/portal/docs/devportal/observability.md
   - docs/portal/docs/devportal/reviewer-onboarding.md

5. Envie feedback via DOCS-SORA-Preview-REQ-C04 e marque cada achado com
   `docs-preview/w2`. Use o formulario de feedback se preferir um intake estruturado:
   docs/examples/docs_preview_feedback_form.md.

Suporte disponivel no Matrix (`#docs-preview:matrix.org`) e teremos office hours
em 2025-06-18 15:00 UTC. Para escalacoes de seguranca ou incidentes, page o alias
on-call de docs via ops@sora.org ou +1-555-0109 imediatamente; nao espere pelos
office hours.

O acesso de preview para Horizon Wallet vai de 2025-06-15 -> 2025-06-29. Avise
assim que terminar para revogar as chaves de acesso temporarias e registrar o
encerramento no tracker.

Obrigado por ajudar a levar o portal ao GA!

- Equipe DOCS-SORA
```
