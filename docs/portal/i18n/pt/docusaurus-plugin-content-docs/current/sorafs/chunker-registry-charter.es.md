---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: chunker-registry-charter
título: Carta de registro de chunker de SoraFS
sidebar_label: Carta de registro de chunker
descrição: Carta de governo para apresentações e aprovações de perfis de chunker.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/chunker_registry_charter.md`. Mantenha ambas as cópias sincronizadas até que o conjunto de documentação Sphinx herdado seja retirado.
:::

# Carta de governança do registro de chunker de SoraFS

> **Ratificado:** 2025-10-29 por el Sora Parliament Infrastructure Panel (ver
> `docs/source/sorafs/council_minutes_2025-10-29.md`). Qualquer amizade requer um
> voto de governança formal; as equipes de implementação devem tratar este documento como
> normativo até que se aprove uma carta que a substitua.

Esta carta define o processo e as funções para evoluir o registro de chunker de SoraFS.
Complementa o [Guia de autoria de perfis de chunker](./chunker-profile-authoring.md) ao descrever como novos
perfis são propostos, revisados, ratificados e eventualmente depreciados.

## Alcance

A carta se aplica a cada entrada em `sorafs_manifest::chunker_registry` e
todas as ferramentas que consomem o registro (CLI de manifesto, CLI de provedor-anúncio,
SDK). Impone as invariantes de alias e identificadores verificados por
`chunker_registry::ensure_charter_compliance()`:

- IDs de perfil são valores positivos que aumentam de forma monótona.
- El handle canónico `namespace.name@semver` **debe** aparecer como primeira
  entrada em `profile_aliases`. Siga o pseudônimo heredados.
- As cadeias de alias são gravadas, são únicas e não colidem com alças canônicas
  de outras entradas.

## Funções

- **Autor(es)** – prepara a proposta, regenera os fixtures e recopila a
  evidência de determinismo.
- **Grupo de Trabalho de Ferramentas (TWG)** – valida a proposta usando as listas de verificação
  publicadas e certifique-se de que as invariantes do registro sejam cumpridas.
- **Conselho de Governança (GC)** – revisa o relatório do TWG, confirma a proposta
  e verifique os prazos de publicação/depreciação.
- **Equipe de armazenamento** – mantém a implementação do registro e publicação
  atualizações de documentação.

## Fluxo do ciclo de vida

1. **Apresentação da proposta**
   - O autor executa a lista de verificação de validação do guia de autoria e criação
     um JSON `ChunkerProfileProposalV1` pt
     `docs/source/sorafs/proposals/`.
   - Inclui a saída CLI de:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Envie um PR que contenha jogos, propuesta, relatório de determinismo e
     atualizações do registro.

2. **Revisão de ferramentas (TWG)**
   - Reproduzir o checklist de validação (fixtures, fuzz, pipeline de manifesto/PoR).
   - Execute `cargo test -p sorafs_car --chunker-registry` e certifique-se de que
     `ensure_charter_compliance()` passe com a nova entrada.
   - Verifique o comportamento do CLI (`--list-profiles`, `--promote-profile`, streaming
     `--json-out=-`) reflete o alias e os identificadores atualizados.
   - Produzir um relatório breve que resuma os hallazgos e o estado de aprovação/rechazo.3. **Aprovação do conselho (GC)**
   - Revise o relatório do TWG e os metadados da proposta.
   - Firma o resumo da proposta (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     e adicione as firmas ao conselho mantido junto com os equipamentos.
   - Registrar o resultado da votação nas leis de governo.

4. **Publicação**
   - Fusiona el PR, atualizando:
     -`sorafs_manifest::chunker_registry_data`.
     - Documentação (`chunker_registry.md`, guias de autoria/conformidade).
     - Luminárias e relatórios de determinismo.
   - Notificação aos operadores e equipamentos SDK do novo perfil e do lançamento planejado.

5. **Depreciação/Retirada**
   - As propostas que substituem um perfil existente devem incluir uma janela de publicação
     dual (períodos de graça) e um plano de atualização.
     no registro e atualiza o registro de migração.

6. **Câmbios de emergência**
   - As eliminações ou hotfixes exigem um voto de conselho com aprovação por prefeitura.
   - O TWG deve documentar as etapas de mitigação de riscos e atualizar o registro de incidentes.

## Expectativas de ferramentas

- `sorafs_manifest_chunk_store` e `sorafs_manifest_stub` exposição:
  - `--list-profiles` para inspeção de registro.
  - `--promote-profile=<handle>` para gerar o bloco de metadados canônicos usados
    para promover um perfil.
  - `--json-out=-` para transmitir relatórios para stdout, habilitando logs de revisão
    reprodutíveis.
- `ensure_charter_compliance()` invoca o início nos binários relevantes
  (`manifest_chunk_store`, `provider_advert_stub`). As tentativas de CI devem falhar
  novas entradas violam a carta.

## Registro

- Guarda todos os relatórios de determinismo em `docs/source/sorafs/reports/`.
- Las actas del consejo que referenciam decisões de chunker viven en
  `docs/source/sorafs/migration_ledger.md`.
- Atualiza `roadmap.md` e `status.md` após cada mudança maior no registro.

## Referências

- Guia de autoria: [Guia de autoria de perfis de chunker](./chunker-profile-authoring.md)
- Checklist de conformidade: `docs/source/sorafs/chunker_conformance.md`
- Referência de registro: [Registro de perfis de chunker](./chunker-registry.md)