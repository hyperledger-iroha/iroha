---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: signing-ceremony
title: Substituicao da cerimonia de assinatura
description: Como o Parlamento Sora aprova e distribui fixtures do chunker SoraFS (SF-1b).
sidebar_label: Cerimonia de assinatura
---

> Roadmap: **SF-1b - aprovacoes de fixtures do Parlamento Sora.**
> O fluxo do Parlamento substitui a antiga "cerimonia de assinatura do conselho" offline.

O ritual manual de assinatura usado para os fixtures do chunker SoraFS foi aposentado.
Todas as aprovacoes agora passam pelo **Parlamento Sora**, a DAO baseada em sorteio que
governa o Nexus. Membros do Parlamento bloqueiam XOR para obter cidadania, rotacionam
entre paineis e votam on-chain para aprovar, rejeitar ou reverter releases de fixtures.
Este guia explica o processo e o tooling para developers.

## Visao geral do Parlamento

- **Cidadania** - Operadores bloqueiam o XOR necessario para se inscrever como cidadaos e
  se tornar elegiveis ao sorteio.
- **Paineis** - As responsabilidades sao divididas entre paineis rotativos (Infraestrutura,
  Moderacao, Tesouraria, ...). O Painel de Infraestrutura e o dono das aprovacoes de
  fixtures do SoraFS.
- **Sorteio e rotacao** - As cadeiras de painel sao redesenhadas na cadencia definida na
  constituicao do Parlamento para que nenhum grupo monopolize as aprovacoes.

## Fluxo de aprovacao de fixtures

1. **Submissao da proposta**
   - O Tooling WG envia o bundle candidato `manifest_blake3.json` mais o diff do fixture
     para o registry on-chain via `sorafs.fixtureProposal`.
   - A proposta registra o digest BLAKE3, a versao semantica e as notas de mudanca.
2. **Revisao e votacao**
   - O Painel de Infraestrutura recebe a atribuicao pela fila de tarefas do Parlamento.
   - Membros do painel inspecionam artefatos de CI, rodam testes de paridade e
     registram votos ponderados on-chain.
3. **Finalizacao**
   - Quando o quorum e atingido, o runtime emite um evento de aprovacao que inclui o
     digest canonico do manifest e o compromisso Merkle do payload do fixture.
   - O evento e espelhado no registry SoraFS para que clientes possam buscar o
     manifest mais recente aprovado pelo Parlamento.
4. **Distribuicao**
   - Helpers de CLI (`cargo xtask sorafs-fetch-fixture`) puxam o manifest aprovado via
     Nexus RPC. As constantes JSON/TS/Go do repositorio ficam sincronizadas ao
     reexecutar `export_vectors` e validar o digest contra o registro on-chain.

## Fluxo de trabalho de developer

- Regenere fixtures com:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- Use o helper de fetch do Parlamento para baixar o envelope aprovado, verificar
  assinaturas e atualizar fixtures locais. Aponte `--signatures` para o envelope
  publicado pelo Parlamento; o helper resolve o manifest associado, recomputa o
  digest BLAKE3 e impoe o perfil canonico `sorafs.sf1@1.0.0`.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

Passe `--manifest` se o manifest estiver em outra URL. Envelopes sem assinatura
sao recusados, a menos que `--allow-unsigned` seja definido para smoke runs locais.

- Ao validar um manifest via gateway de staging, aponte para Torii em vez de
  payloads locais:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- O CI local nao exige mais um roster `signer.json`.
  `ci/check_sorafs_fixtures.sh` compara o estado do repo com o ultimo compromisso
  on-chain e falha quando divergem.

## Notas de governanca

- A constituicao do Parlamento governa quorum, rotacao e escalonamento - nao e
  necessaria configuracao no nivel do crate.
- Rollbacks de emergencia sao tratados pelo painel de moderacao do Parlamento. O
  Painel de Infraestrutura abre uma proposta de revert que referencia o digest
  anterior do manifest, substituindo a release quando aprovada.
- Aprovacoes historicas permanecem disponiveis no registry SoraFS para replay
  forense.

## FAQ

- **Para onde foi `signer.json`?**  
  Foi removido. Toda a atribuicao de assinaturas vive on-chain; `manifest_signatures.json`
  no repositorio e apenas um fixture de developer que deve corresponder ao ultimo
  evento de aprovacao.

- **Ainda exigimos assinaturas Ed25519 locais?**  
  Nao. As aprovacoes do Parlamento sao armazenadas como artefatos on-chain. Fixtures
  locais existem para reprodutibilidade, mas sao validados contra o digest do Parlamento.

- **Como as equipes monitoram aprovacoes?**  
  Assinem o evento `ParliamentFixtureApproved` ou consultem o registry via Nexus RPC
  para recuperar o digest atual do manifest e a chamada do painel.
