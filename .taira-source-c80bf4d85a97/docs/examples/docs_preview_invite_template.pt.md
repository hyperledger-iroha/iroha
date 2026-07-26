---
lang: pt
direction: ltr
source: docs/examples/docs_preview_invite_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6c819c8d2a9517f1235a66a4661efd061a166ea89c953fd599e102b3cfd9157b
source_last_modified: "2025-11-10T18:08:48.050596+00:00"
translation_last_reviewed: 2026-01-01
---

# Convite de preview do portal de docs (Modelo)

Use este modelo quando enviar instrucoes de acesso de preview aos revisores. Substitua
os placeholders (`<...>`) pelos valores relevantes, anexe os artefatos de descriptor +
archive citados na mensagem e guarde o texto final no ticket de intake correspondente.

```text
Assunto: [DOCS-SORA] convite de preview do portal de docs <preview_tag> para <reviewer/org>

Oi <name>,

Obrigado por se voluntariar para revisar o portal de docs antes do GA. Voce esta liberado
para a onda <wave_id>. Siga os passos abaixo antes de navegar na preview:

1. Baixe os artefatos verificados do CI ou SoraFS:
   - Descriptor: <descriptor_url> (`sha256:<descriptor_sha256>`)
   - Archive: <archive_url> (`sha256:<archive_sha256>`)
2. Execute o gate de checksum:

   ./docs/portal/scripts/preview_verify.sh      --descriptor <path-to-descriptor>      --archive <path-to-archive>      --build-dir <path-to-extracted-build>

3. Sirva a preview com enforcement de checksum ativado:

   DOCS_RELEASE_TAG=<preview_tag> npm run --prefix docs/portal serve

4. Leia as notas de uso aceitavel, seguranca e observabilidade:
   - docs/portal/docs/devportal/security-hardening.md
   - docs/portal/docs/devportal/observability.md
   - docs/portal/docs/devportal/reviewer-onboarding.md

5. Envie feedback via <request_ticket> e marque cada achado com `<preview_tag>`.

Suporte disponivel em <contact_channel>. Incidentes ou questoes de seguranca devem ser
reportados imediatamente via <incident_channel>. Se precisar de tokens da API Torii,
solicite-os pelo ticket; nunca reutilize credenciais de producao.

O acesso de preview expira em <end_date> salvo extensao por escrito. Registramos
checksums e metadados do convite para governance; avise quando terminar
para que possamos fazer o offboard de forma limpa.

Obrigado novamente por ajudar a estabilizar o portal!

- Equipe DOCS-SORA
```
