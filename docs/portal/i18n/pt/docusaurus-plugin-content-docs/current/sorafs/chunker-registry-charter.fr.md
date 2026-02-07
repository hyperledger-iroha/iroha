---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: chunker-registry-charter
título: Charte du registre chunker SoraFS
sidebar_label: Gráfico de registro chunker
descrição: Carta de governança para submissões e aprovações de perfis chunker.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/chunker_registry_charter.md`. Gardez as duas cópias sincronizadas junto com o retorno completo do conjunto Sphinx herité.
:::

# Carta de governança do bloco de registro SoraFS

> **Ratifiée:** 2025-10-29 pelo Sora Parliament Infrastructure Panel (ver
> `docs/source/sorafs/council_minutes_2025-10-29.md`). Toda alteração exige um
> vote de gouvernance formel; as equipes de implementação devem trair este documento como
> normatif jusqu'à l'approbation d'une charte de remplacement.

Este gráfico define o processo e as funções para fazer evoluir o bloco de registro SoraFS.
Ela completou o [Guia de criação de perfis chunker](./chunker-profile-authoring.md) e descritivo comentário de novos
perfis são propostos, revisados, ratificados e finalizados.

## Portée

O gráfico é aplicado a cada entrada de `sorafs_manifest::chunker_registry` e
em todas as ferramentas que usam o registro (CLI manifesto, CLI de anúncio de provedor,
SDK). Ela impõe invariantes de apelido e identificador verificados por
`chunker_registry::ensure_charter_compliance()`:

- Les ID de perfil são todos os pontos positivos que aumentam de forma monótona.
- Le handle canonique `namespace.name@semver` **doit** apparaître en première
- As cadeias de alias são trimadas, únicas e não colisões com as alças
  canônicas de outras entradas.

## Funções

- **Autor(es)** – prepara a proposta, administra os equipamentos e coleta os
  testes de determinismo.
- **Grupo de Trabalho de Ferramentas (TWG)** – valida a proposta com auxílio das listas de verificação
  publiquei e certifique-se de que as invariantes do registro sejam respeitadas.
- **Conselho de Governança (CG)** – examinar o relatório do TWG, assinar o envelope da proposta
  e aprove os calendários de publicação/depreciação.
- **Equipe de armazenamento** – mantém a implementação do registro e publicação
  as mises à jour de documentação.

## Fluxo do ciclo de vida

1. **Envio da proposta**
   - O autor executa a lista de verificação de validação do guia do autor e da criação
     um JSON `ChunkerProfileProposalV1` baseado
     `docs/source/sorafs/proposals/`.
   - Incluir a sortie CLI de:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Soumettre une PR contendo luminárias, proposição, relacionamento de determinismo et
     mises à jour du registre.

2. **Ferramentas de revisão (TWG)**
   - Refazer o checklist de validação (fixtures, fuzz, pipeline manifest/PoR).
   - Execute `cargo test -p sorafs_car --chunker-registry` e verifique se
     `ensure_charter_compliance()` passe com a nova entrada.
   - Verifique o comportamento do CLI (`--list-profiles`, `--promote-profile`, streaming
     `--json-out=-`) reflete o alias e os identificadores atuais.
   - Produire un tribunal rapport resumo les constats et le statut pass/fail.3. **Aprovação do Conselho (GC)**
   - Examinar o relacionamento do TWG e os objetivos da proposta.
   - Assine o resumo da proposta (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     e adicione as assinaturas ao envelope do conselho de manutenção com os equipamentos.
   - Consignar o resultado da votação nas atas de governo.

4. **Publicação**
   - Fusionar o PR no momento:
     -`sorafs_manifest::chunker_registry_data`.
     - Documentação (`chunker_registry.md`, guias de autor/conformidade).
     - Luminárias e relatórios de determinismo.
   - Notificar operadores e equipes SDK do novo perfil e da implementação anterior.

5. **Depreciação/Retração**
   - As propostas que substituem um perfil existente devem incluir uma janela de publicação
     double (períodos de graça) e um plano de atualização.
   - Após a expiração da janela de graça, marque o perfil substituído quando depreciado
     no registro e no registro do registro de migração.

6. **Alterações de urgência**
   - As supressões ou hotfixes exigem um voto do conselho da maioria.
   - O TWG documenta as etapas de mitigação de riscos e publica diariamente o diário de incidentes.

## Ferramentas Atentes

- `sorafs_manifest_chunk_store` e `sorafs_manifest_stub` exposto:
  - `--list-profiles` para inspeção de registro.
  - `--promote-profile=<handle>` para gerar o bloco de metadonées canônico utilizado
    durante a promoção de um perfil.
  - `--json-out=-` para streamer de relatórios em stdout, permitindo registros de revisão
    reprodutíveis.
- `ensure_charter_compliance()` é invocado durante o descarte nos binários em questão
  (`manifest_chunk_store`, `provider_advert_stub`). Os testes CI devem ser ouvidos
  de nouvelles entrées violenta la charte.

## Registrar

- Armazene todos os relatórios de determinação em `docs/source/sorafs/reports/`.
- Os minutos do conselho referente às decisões chunker vivent sous
  `docs/source/sorafs/migration_ledger.md`.
- Envie hoje `roadmap.md` e `status.md` após cada alteração maior do registro.

## Referências

- Guia de criação: [Guia de criação de perfis chunker] (./chunker-profile-authoring.md)
- Checklist de conformidade: `docs/source/sorafs/chunker_conformance.md`
- Referência do registro: [Registro do bloco de perfis] (./chunker-registry.md)