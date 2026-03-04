---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-integrity-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Plano de controle de pré-visualização por checksum

Este plano descreve o trabalho restante necessário para tornar cada artefato de pré-visualização do portal verificável antes da publicação. O objetivo é garantir que os leitores carreguem exatamente o instantâneo construído em CI, que o manifesto da soma de verificação seja imuável e que a pré-visualização seja decupável via SoraFS com os metadonnes Norito.

## Objetivos

- **Builds deterministas:** Garanta que `npm run build` produza uma saída reproduzível e continue sempre `build/checksums.sha256`.
- **Pré-visualizações verificadas:** Exigir que cada artefato de pré-visualização forneça um manifesto de soma de verificação e recuse a publicação quando a verificação for repetida.
- **Metadonnees publicados via Norito:** Persistir os descritores de pré-visualização (metadonnees de commit, digest de checksum, CID SoraFS) em JSON Norito para que as ferramentas de governança possam auditar os lançamentos.
- **Operador de inicialização:** Fornece um script de verificação em uma etapa em que os usuários podem executar a localização do executado (`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`); o script encapsula o fluxo de verificação de validação + descritor de luta em luta. O comando de pré-visualização padrão (`npm run serve`) invoca a manutenção deste assistente automaticamente antes de `docusaurus serve` para que os instantâneos localizados permaneçam sob o controle de soma de verificação (com `npm run serve:verified` conservam como alias explícito).

## Fase 1 - CI de aplicação

1. Coloque um dia `.github/workflows/docs-portal-preview.yml` para:
   - O executor `node docs/portal/scripts/write-checksums.mjs` após a compilação Docusaurus (deixa de chamar a localização).
   - Execute `cd build && sha256sum -c checksums.sha256` e reproduza o trabalho em caso de divergência.
   - Empaque o repertório construído em `artifacts/preview-site.tar.gz`, copie o manifesto de checksum, o executor `scripts/generate-preview-descriptor.mjs` e o executor `scripts/sorafs-package-preview.sh` com uma configuração JSON (veja `docs/examples/sorafs_preview_publish.json`) para que o fluxo de trabalho seja emitido para os metadonnees e um pacote Determinador SoraFS.
   - Televerse le site statique, les artefatos de metadonnes (`docs-portal-preview`, `docs-portal-preview-metadata`) et le bundle SoraFS (`docs-portal-preview-sorafs`) afin que le manifeste, le resume CAR et le plan puissent etre inspeciona sans relancer le build.
2. Adicione um comentário de crachá CI para retomar o resultado da soma de verificação de verificação nas solicitações pull (implementado por meio da fita de comentário GitHub Script de `docs-portal-preview.yml`).
3. Documente o fluxo de trabalho em `docs/portal/README.md` (seção CI) e coloque as etapas de verificação na lista de verificação de publicação.

## Script de verificação

`docs/portal/scripts/preview_verify.sh` valida os artefatos de pré-visualização telecarregados sem a necessidade de invocações manuais de `sha256sum`. Use `npm run serve` (ou o alias explícito `npm run serve:verified`) para executar o script e lancer `docusaurus serve` em uma única etapa ao compartilhar snapshots locais. A lógica de verificação:

1. Execute o utilitário SHA apropriado (`sha256sum` ou `shasum -a 256`) contra `build/checksums.sha256`.
2. Compare as opções do resumo/nome do arquivo do descritor de pré-visualização `checksums_manifest` e, quando for o caso, o resumo/nome do arquivo do arquivo de pré-visualização.
3. Classifique com um código não nulo quando uma divergência for detectada para que os reletores possam bloquear as pré-visualizações alteradas.

Exemplo de utilização (após extração de artefatos CI):

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

Os engenheiros de CI e lançamento devem chamar o script cada vez que eles baixam um pacote de pré-visualização ou anexam os artefatos a um bilhete de lançamento.

## Fase 2 - Publicação SoraFS

1. Crie o fluxo de trabalho de pré-visualização com um trabalho aqui:
   - Televerse le site construit ver la passerelle de staging SoraFS en utilisant `sorafs_cli car pack` et `manifest submit`.
   - Capture o resumo do manifesto enviado e o CID SoraFS.
   - Serialize `{ commit, branch, checksum_manifest, cid }` em JSON Norito (`docs/portal/preview/preview_descriptor.json`).
2. Armazene o descritor com o artefato de construção e exponha o CID no comentário da solicitação pull.
3. Adicionar testes de integração que exercem `sorafs_cli` em modo de simulação para garantir que as evoluções futuras conservam a coerência do esquema de metadonnees.

## Fase 3 - Governança e auditoria1. Publique um esquema Norito (`PreviewDescriptorV1`) que descreve a estrutura do descritor sob `docs/portal/schemas/`.
2. Insira hoje a lista de verificação da publicação DOCS-SORA para exigir:
   - Lancer `sorafs_cli manifest verify` na carga CID.
   - Registre o resumo do manifesto da soma de verificação e o CID na descrição do PR de lançamento.
3. Conecte a automação de governança para cruzar o descritor com o manifesto da soma de verificação pendente dos votos de liberação.

## Livrables e responsabilidades

| Jalão | Proprietário(s) | Cível | Notas |
|-------|-----------------|-------|-------|
| Aplicação de somas de verificação em CI grátis | Documentos de infraestrutura | Semana 1 | Adicione uma porta de validação e upload de artefatos. |
| Publicação de prévias SoraFS | Documentos de Infraestrutura / Equipe Storage | Semana 2 | Necessário acesso a identificadores de teste e erros no dia do esquema Norito. |
| Governança da integração | Documentos Responsáveis/DevRel / WG Governança | Semana 3 | Publique o esquema e apresente recentemente as listas de verificação e as entradas do roteiro. |

## Perguntas abertas

- Qual ambiente SoraFS possui artefatos de pré-visualização (staging vs. lane de previsualization dediee)?
- Avons-nous besoin de assinaturas duplas (Ed25519 + ML-DSA) no descritor de pré-visualização antes da publicação?
- O fluxo de trabalho CI deve copiar a configuração do orquestrador (`orchestrator_tuning.json`) durante a execução de `sorafs_cli` para armazenar manifestos reproduzíveis?

Transmita as decisões em `docs/portal/docs/reference/publishing-checklist.md` e faça um plano diário para as resoluções inconnuas.