---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: chunker-registry-charter
title: Carta do registro de chunker da SoraFS
sidebar_label: Carta do registro de chunker
description: Carta de governanca para submissao e aprovacao de perfis de chunker.
---

:::note Fonte canonica
Esta pagina espelha `docs/source/sorafs/chunker_registry_charter.md`. Mantenha ambas as copias sincronizadas.
:::

# Carta de governanca do registro de chunker da SoraFS

> **Ratificado:** 2025-10-29 pelo Sora Parliament Infrastructure Panel (veja
> `docs/source/sorafs/council_minutes_2025-10-29.md`). Qualquer emenda requer um
> voto formal de governanca; equipes de implementacao devem tratar este documento como
> normativo ate que uma carta substituta seja aprovada.

Esta carta define o processo e os papeis para evoluir o registro de chunker da SoraFS.
Ela complementa o [Guia de autoria de perfis de chunker](./chunker-profile-authoring.md) ao descrever como novos
perfis sao propostos, revisados, ratificados e eventualmente descontinuados.

## Escopo

A carta se aplica a cada entrada em `sorafs_manifest::chunker_registry` e
a qualquer tooling que consome o registro (manifest CLI, provider-advert CLI,
SDKs). Ela impoe os invariantes de alias e handle verificados por
`chunker_registry::ensure_charter_compliance()`:

- IDs de perfil sao inteiros positivos que aumentam de forma monotona.
- O handle canonico `namespace.name@semver` **deve** aparecer como a primeira
  entrada em `profile_aliases`. Aliases alternativos vem em seguida.
- As strings de alias sao aparadas, unicas e nao colidem com handles canonicos
  de outras entradas.

## Papeis

- **Autor(es)** - preparam a proposta, regeneram fixtures e coletam a
  evidencia de determinismo.
- **Tooling Working Group (TWG)** - valida a proposta usando as checklists
  publicadas e assegura que os invariantes do registro sejam atendidos.
- **Governance Council (GC)** - revisa o relatorio do TWG, assina o envelope da proposta
  e aprova os prazos de publicacao/deprecacao.
- **Storage Team** - mantem a implementacao do registro e publica
  atualizacoes de documentacao.

## Fluxo do ciclo de vida

1. **Submissao de proposta**
   - O autor executa a checklist de validacao do guia de autoria e cria
     um JSON `ChunkerProfileProposalV1` sob
     `docs/source/sorafs/proposals/`.
   - Inclua a saida do CLI de:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Envie um PR contendo fixtures, proposta, relatorio de determinismo e
     atualizacoes do registro.

2. **Revisao de tooling (TWG)**
   - Repita a checklist de validacao (fixtures, fuzz, pipeline de manifest/PoR).
   - Execute `cargo test -p sorafs_car --chunker-registry` e garanta que
     `ensure_charter_compliance()` passe com a nova entrada.
   - Verifique que o comportamento do CLI (`--list-profiles`, `--promote-profile`, streaming
     `--json-out=-`) reflita os aliases e handles atualizados.
   - Produza um relatorio curto resumindo achados e status de aprovacao/reprovacao.

3. **Aprovacao do conselho (GC)**
   - Revise o relatorio do TWG e os metadados da proposta.
   - Assine o digest da proposta (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     e anexe as assinaturas ao envelope do conselho mantido junto aos fixtures.
   - Registre o resultado da votacao nas atas de governanca.

4. **Publicacao**
   - Faca merge do PR, atualizando:
     - `sorafs_manifest::chunker_registry_data`.
     - Documentacao (`chunker_registry.md`, guias de autoria/conformidade).
     - Fixtures e relatorios de determinismo.
   - Notifique operadores e equipes SDK sobre o novo perfil e o rollout planejado.

5. **Deprecacao / Encerramento**
   - Propostas que substituem um perfil existente devem incluir uma janela de publicacao
     dupla (periodos de carencia) e um plano de upgrade.
     no registro e atualize o ledger de migracao.

6. **Mudancas de emergencia**
   - Remocoes ou hotfixes exigem voto do conselho com aprovacao por maioria.
   - O TWG deve documentar as etapas de mitigacao de risco e atualizar o log de incidentes.

## Expectativas de tooling

- `sorafs_manifest_chunk_store` e `sorafs_manifest_stub` expoem:
  - `--list-profiles` para inspecao do registro.
  - `--promote-profile=<handle>` para gerar o bloco de metadados canonico usado
    ao promover um perfil.
  - `--json-out=-` para transmitir relatorios para stdout, habilitando logs de revisao
    reproduziveis.
- `ensure_charter_compliance()` e invocado na inicializacao dos binarios relevantes
  (`manifest_chunk_store`, `provider_advert_stub`). Os testes de CI devem falhar se
  novas entradas violarem a carta.

## Registro

- Armazene todos os relatorios de determinismo em `docs/source/sorafs/reports/`.
- As atas do conselho que referenciam decisoes de chunker ficam em
  `docs/source/sorafs/migration_ledger.md`.
- Atualize `roadmap.md` e `status.md` apos cada mudanca maior no registro.

## Referencias

- Guia de autoria: [Guia de autoria de perfis de chunker](./chunker-profile-authoring.md)
- Checklist de conformidade: `docs/source/sorafs/chunker_conformance.md`
- Referencia do registro: [Registro de perfis de chunker](./chunker-registry.md)
