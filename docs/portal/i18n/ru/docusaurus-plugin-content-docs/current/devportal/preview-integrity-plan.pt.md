---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/preview-integrity-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Plano de pre-visualizacao com checksum

Este plano descreve o trabalho restante necessario para tornar cada artefato de pre-visualizacao do portal verificavel antes da publicacao. O objetivo e garantir que os revisores baixem exatamente o snapshot construido em CI, que o manifesto de checksum seja imutavel e que a pre-visualizacao seja descobrivel via SoraFS com metadados Norito.

## Objetivos

- **Builds deterministicas:** Garantir que `npm run build` produza saida reproduzivel e sempre gere `build/checksums.sha256`.
- **Pre-visualizacoes verificadas:** Exigir que cada artefato de pre-visualizacao inclua um manifesto de checksum e recusar a publicacao quando a verificacao falhar.
- **Metadados publicados via Norito:** Persistir descritores de pre-visualizacao (metadados de commit, digest de checksum, CID SoraFS) como JSON Norito para que as ferramentas de governanca possam auditar releases.
- **Ferramentas para operadores:** Fornecer um script de verificacao de um passo que consumidores possam executar localmente (`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`); o script agora envolve o fluxo de validacao de checksum + descritor de ponta a ponta. O comando padrao de pre-visualizacao (`npm run serve`) agora invoca esse helper automaticamente antes de `docusaurus serve` para que snapshots locais permanecam protegidos por checksum (com `npm run serve:verified` mantido como alias explicito).

## Fase 1 - Aplicacao em CI

1. Atualizar `.github/workflows/docs-portal-preview.yml` para:
   - Executar `node docs/portal/scripts/write-checksums.mjs` apos o build do Docusaurus (ja invocado localmente).
   - Executar `cd build && sha256sum -c checksums.sha256` e falhar o job em caso de divergencia.
   - Empacotar o diretorio build como `artifacts/preview-site.tar.gz`, copiar o manifesto de checksum, executar `scripts/generate-preview-descriptor.mjs` e executar `scripts/sorafs-package-preview.sh` com uma configuracao JSON (ver `docs/examples/sorafs_preview_publish.json`) para que o workflow emita tanto metadados quanto um bundle SoraFS deterministico.
   - Enviar o site estatico, os artefatos de metadados (`docs-portal-preview`, `docs-portal-preview-metadata`) e o bundle SoraFS (`docs-portal-preview-sorafs`) para que o manifesto, o resumo CAR e o plano possam ser inspecionados sem refazer o build.
2. Adicionar um comentario de badge CI resumindo o resultado da verificacao de checksum nos pull requests (implementado via o passo de comentario GitHub Script de `docs-portal-preview.yml`).
3. Documentar o workflow em `docs/portal/README.md` (secao CI) e linkar as etapas de verificacao na checklist de publicacao.

## Script de verificacao

`docs/portal/scripts/preview_verify.sh` valida os artefatos de pre-visualizacao baixados sem exigir invocacoes manuais de `sha256sum`. Use `npm run serve` (ou o alias explicito `npm run serve:verified`) para executar o script e iniciar `docusaurus serve` em um unico passo ao compartilhar snapshots locais. A logica de verificacao:

1. Executa a ferramenta SHA apropriada (`sha256sum` ou `shasum -a 256`) contra `build/checksums.sha256`.
2. Opcionalmente compara o digest/nome de arquivo do descritor de pre-visualizacao `checksums_manifest` e, quando fornecido, o digest/nome de arquivo do arquivo de pre-visualizacao.
3. Sai com codigo nao zero quando alguma divergencia e detectada para que revisores possam bloquear pre-visualizacoes adulteradas.

Exemplo de uso (apos extrair os artefatos de CI):

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

Engenheiros de CI e release devem executar o script sempre que baixarem um bundle de pre-visualizacao ou anexarem artefatos a um ticket de release.

## Fase 2 - Publicacao em SoraFS

1. Estender o workflow de pre-visualizacao com um job que:
   - Envie o site construido para o gateway de staging do SoraFS usando `sorafs_cli car pack` e `manifest submit`.
   - Capture o digest do manifesto retornado e o CID do SoraFS.
   - Serializa `{ commit, branch, checksum_manifest, cid }` em JSON Norito (`docs/portal/preview/preview_descriptor.json`).
2. Armazenar o descritor junto ao artefato de build e expor o CID no comentario do pull request.
3. Adicionar testes de integracao que exercitem `sorafs_cli` em modo dry-run para garantir que mudancas futuras mantenham a consistencia do schema de metadados.

## Fase 3 - Governanca e auditoria

1. Publicar um schema Norito (`PreviewDescriptorV1`) descrevendo a estrutura do descritor em `docs/portal/schemas/`.
2. Atualizar a checklist de publicacao DOCS-SORA para exigir:
   - Rodar `sorafs_cli manifest verify` contra o CID enviado.
   - Registrar o digest do manifesto de checksum e o CID na descricao do PR de release.
3. Ligar a automacao de governanca para cruzar o descritor com o manifesto de checksum durante votos de release.

## Entregaveis e responsabilidade

| Marco | Proprietario(s) | Alvo | Notas |
|-------|-----------------|------|-------|
| Aplicacao de checksum em CI concluida | Infraestrutura de Docs | Semana 1 | Adiciona gate de falha e uploads de artefatos. |
| Publicacao de pre-visualizacao no SoraFS | Infraestrutura de Docs / Equipe de Storage | Semana 2 | Requer acesso a credenciais de staging e atualizacoes do schema Norito. |
| Integracao de governanca | Lider de Docs/DevRel / WG de Governanca | Semana 3 | Publica o schema e atualiza checklists e entradas do roadmap. |

## Perguntas em aberto

- Qual ambiente do SoraFS deve hospedar artefatos de pre-visualizacao (staging vs. lane de pre-visualizacao dedicada)?
- Precisamos de assinaturas duplas (Ed25519 + ML-DSA) no descritor de pre-visualizacao antes da publicacao?
- O workflow de CI deve fixar a configuracao do orquestrador (`orchestrator_tuning.json`) ao executar `sorafs_cli` para manter manifestos reproduziveis?

Registre as decisoes em `docs/portal/docs/reference/publishing-checklist.md` e atualize este plano quando as duvidas forem resolvidas.
