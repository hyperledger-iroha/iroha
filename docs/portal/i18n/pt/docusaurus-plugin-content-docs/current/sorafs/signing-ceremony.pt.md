---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: cerimônia de assinatura
título: Substituição da cerimonia de assinatura
descrição: Como o Parlamento Sora aprova e distribui fixtures do chunker SoraFS (SF-1b).
sidebar_label: Cerimônia de assinatura
---

> Roteiro: **SF-1b - aprovações de luminárias do Parlamento Sora.**
> O fluxo do Parlamento substitui a antiga "cerimonia de assinatura do conselho" offline.

O ritual manual de assinatura usado para os fixtures do chunker SoraFS foi aposentado.
Todas as aprovações agora passam pelo **Parlamento Sora**, a DAO baseado em sorteio que
governa o Nexus. Membros do Parlamento bloquearam XOR para obter cidadania, rotacionaram
entre paineis e votam on-chain para aprovar, rejeitar ou reverter lançamentos de fixtures.
Este guia explica o processo e as ferramentas para desenvolvedores.

## Visão geral do Parlamento

- **Cidadania** - Operadores bloqueiam o XOR necessário para se inscrever como cidadãos e
  se tornar elegível ao sorteio.
- **Paineis** - As responsabilidades são divididas entre paineis rotativos (Infraestrutura,
  Moderação, Tesouraria, ...). O Painel de Infraestrutura e o dono das aprovações de
  luminárias fazem SoraFS.
- **Sorteio e rotação** - As cadeiras de painel são redesenhadas na cadência definida na
  constituição do Parlamento para que nenhum grupo monopolize as aprovações.

## Fluxo de aprovação de fixtures

1. **Envio de proposta**
   - O Tooling WG envia o pacote candidato `manifest_blake3.json` mais o diff do fixture
     para o registro on-chain via `sorafs.fixtureProposal`.
   - A proposta registra o resumo BLAKE3, a versão semântica e as notas de mudança.
2. **Revisão e votação**
   - O Painel de Infraestrutura recebe atribuição pela fila de tarefas do Parlamento.
   - Membros do painel de operação de artistas de CI, rodaram testes de paridade e
     registram votos ponderados on-chain.
3. **Finalização**
   - Quando o quorum é atingido, o runtime emite um evento de aprovação que inclui o
     digest canonico do manifest e o compromisso Merkle do payload do fixture.
   - O evento é espelhado no registro SoraFS para que os clientes possam buscar o
     manifesto mais recente aprovado pelo Parlamento.
4. **Distribuição**
   - Helpers de CLI (`cargo xtask sorafs-fetch-fixture`) puxam o manifesto aprovado via
     NexusRPC. As constantes JSON/TS/Go do repositório ficam sincronizadas ao
     reexecutar `export_vectors` e validar o resumo contra o registro on-chain.

## Fluxo de trabalho do desenvolvedor

- Regenere fixtures com:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- Use o helper de fetch do Parlamento para baixar o envelope aprovado, verifique
  assinaturas e atualizações de luminárias locais. Ponte `--signatures` para envelope
  publicado pelo Parlamento; o helper resolve o manifesto associado, recomputa o
  digerir BLAKE3 e impor o perfil canônico `sorafs.sf1@1.0.0`.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

Passe `--manifest` se o manifesto estiver em outra URL. Envelopes sem assinatura
são recusados, a menos que `--allow-unsigned` seja definido para fumo em locais.- Ao validar um manifesto via gateway de staging, aponte para Torii em vez de
  cargas úteis locais:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- O CI local não exige mais um roster `signer.json`.
  `ci/check_sorafs_fixtures.sh` compara o estado do repo com o último compromisso
  on-chain e falha quando divergem.

## Notas de governança

- A constituição do Parlamento governa quorum, rotação e escalonamento - nao e
  necessaria configuração no nível da caixa.
- Reversões de emergência são tratadas pelo painel de moderação do Parlamento. Ó
  Painel de Infraestrutura abre uma proposta de reversão que referencia o resumo
  anterior do manifesto, substituindo a liberação quando aprovado.
- Aprovações históricas permanecem disponíveis no registro SoraFS para reprodução
  forense.

## Perguntas frequentes

- **Para onde foi `signer.json`?**  
  Foi removido. Toda a atribuição de assinaturas vive on-chain; `manifest_signatures.json`
  no repositório e apenas um fixture de desenvolvedor que deve ser responsável ao final
  evento de aprovação.

- **Ainda exigimos assinaturas Ed25519 locais?**  
  Não. As aprovações do Parlamento são armazenadas como artefatos on-chain. Jogos
  existem locais para reprodutibilidade, mas são validados contra o digest do Parlamento.

- **Como as equipes monitoram aprovações?**  
  Assine o evento `ParliamentFixtureApproved` ou consulte o registro via Nexus RPC
  para recuperar o resumo atual do manifesto e a chamada do painel.