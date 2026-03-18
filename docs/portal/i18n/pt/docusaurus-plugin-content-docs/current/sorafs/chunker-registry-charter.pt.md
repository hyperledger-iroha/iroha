---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: chunker-registry-charter
título: Carta de registro de chunker da SoraFS
sidebar_label: Carta de registro do chunker
descrição: Carta de governança para submissão e aprovação de perfis de chunker.
---

:::nota Fonte canônica
Esta página espelha `docs/source/sorafs/chunker_registry_charter.md`. Mantenha ambas as cópias sincronizadas.
:::

# Carta de governança do registro de chunker da SoraFS

> **Ratificado:** 2025-10-29 pelo Sora Parliament Infrastructure Panel (veja
> `docs/source/sorafs/council_minutes_2025-10-29.md`). Qualquer emenda requer um
> voto formal de governança; equipes de implementação deverão tratar este documento como
> normativo até que uma carta substituta seja aprovada.

Esta carta define o processo e os papéis para evoluir o registro de chunker da SoraFS.
Ela complementa o [Guia de autoria de perfis de chunker](./chunker-profile-authoring.md) ao descrever como novos
perfis são propostos, revisados, ratificados e eventualmente descontinuados.

## Escopo

A carta é aplicada a cada entrada em `sorafs_manifest::chunker_registry` e
a qualquer ferramenta que consome o registro (CLI manifesto, CLI fornecedor-anúncio,
SDK). Ela impõe invariantes de alias e handle selecionados por
`chunker_registry::ensure_charter_compliance()`:

- IDs de perfil são inteiros positivos que aumentam de forma monótona.
- O identificador canônico `namespace.name@semver` **deve** aparecer como a primeira
  entrada em `profile_aliases`. Aliases alternativos vêm em seguida.
- As strings de alias são aparadas, unicas e não colidem com handles canonicos
  de outras entradas.

## Papeis

- **Autor(es)** - preparam uma proposta, regeneram luminárias e coletam a
  evidência de determinismo.
- **Grupo de Trabalho de Ferramentas (TWG)** - valida a proposta usando as listas de verificação
  publicadas e garantimos que as invariantes do registro sejam atendidas.
- **Conselho de Governança (CG)** - revisão do relatorio do TWG, assinatura do envelope da proposta
  e aprova os prazos de publicação/depreciação.
- **Storage Team** - mantem a implementação do registro e publicação
  atualizações de documentação.

## Fluxo do ciclo de vida

1. **Envio de proposta**
   - O autor executa um checklist de validação do guia de autoria e criação
     um JSON `ChunkerProfileProposalV1` sob
     `docs/source/sorafs/proposals/`.
   - Inclui a frase do CLI de:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Envie um PR contendo jogos, proposta, relatorio de determinismo e
     atualizações do registro.

2. **Revisão de ferramentas (TWG)**
   - Repita um checklist de validação (fixtures, fuzz, pipeline de manifest/PoR).
   - Execute `cargo test -p sorafs_car --chunker-registry` e garanta que
     `ensure_charter_compliance()` passe com a nova entrada.
   - Verifique que o comportamento do CLI (`--list-profiles`, `--promote-profile`, streaming
     `--json-out=-`) reflita os aliases e identificadores atualizados.
   - Produza um relatorio curto resumindo achados e status de aprovação/reprovação.3. **Aprovação do conselho (GC)**
   - Revisar o relatorio do TWG e os metadados da proposta.
   - Assinatura do resumo da proposta (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     e anexo as assinaturas ao envelope do conselho fechado junto aos jogos.
   - Registrar o resultado da votação nas atas de governança.

4. **Publicação**
   - Faca merge do PR, atualizando:
     -`sorafs_manifest::chunker_registry_data`.
     - Documentação (`chunker_registry.md`, guias de autoria/conformidade).
     - Fixtures e relatorios de determinismo.
   - Notifique operadores e equipes SDK sobre o novo perfil e o rollout planejado.

5. **Depreciação/Encerramento**
   - Propostas que substituam um perfil existente deverão incluir uma janela de publicação
     dupla (períodos de carência) e um plano de atualização.
     no registro e atualizar o ledger de migração.

6. **Mudanças de emergência**
   - Remoções ou hotfixes desativar voto do conselho com aprovação por maioria.
   - O TWG deve documentar as etapas de mitigação de risco e atualização do log de incidentes.

## Expectativas de ferramentas

- Exposição `sorafs_manifest_chunk_store` e `sorafs_manifest_stub`:
  - `--list-profiles` para inspeção do registro.
  - `--promote-profile=<handle>` para gerar o bloco de metadados canônicos usados
    ao promover um perfil.
  - `--json-out=-` para transmitir relatórios para stdout, habilitando logs de revisão
    reproduziveis.
- `ensure_charter_compliance()` e invocado na inicialização dos binários relevantes
  (`manifest_chunk_store`, `provider_advert_stub`). Os testes de CI devem falhar
  novas entradas violarem a carta.

## Registro

- Armazene todos os relatos de determinismo em `docs/source/sorafs/reports/`.
- As atas do conselho que referenciam decisões de chunker em ficam
  `docs/source/sorafs/migration_ledger.md`.
- Atualizar `roadmap.md` e `status.md` após cada mudança maior no registro.

## Referências

- Guia de autoria: [Guia de autoria de perfis de chunker](./chunker-profile-authoring.md)
- Checklist de conformidade: `docs/source/sorafs/chunker_conformance.md`
- Referência do registro: [Registro de perfis de chunker](./chunker-registry.md)