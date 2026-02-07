---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: cerimônia de assinatura
título: Sustitución de la ceremonia de firma
descrição: Como o Parlamento de Sora aprova e distribui luminárias do bloco SoraFS (SF-1b).
sidebar_label: Cerimônia de firma
---

> Roteiro: **SF-1b — aprovações de luminárias do Parlamento de Sora.**
> O fluxo do Parlamento substitui a antiga “cerimônia de firma del consejo” offline.

O manual ritual de firmas usado para os acessórios do pedaço de SoraFS foi retirado. Tudo
as aprovações agora passam pelo **Parlamento de Sora**, o DAO baseado em sorte que gobierna
Nexus. Os membros do Parlamento fianzan XOR para obter ciudadania, rotan entre painéis e
emite votos on-chain que aprueban, rechazan ou revierten releases de fixtures. Este guia explica
o processo e as ferramentas para desenvolvedores.

## Resumo do Parlamento

- **Ciudadania** — Os operadores financiam o XOR necessário para se inscreverem como cidadãos e
  volverse elegíveis para sorte.
- **Painéis** — Las responsabilidades se dividem em painéis rotativos (Infraestructura,
  Moderação, Tesorería, …). O Painel de Infraestrutura é a dívida das aprovações de
  luminárias de SoraFS.
- **Sorteio e rotação** — Os asientos do painel são reatribuídos com a cadência especificada em
  a constituição do Parlamento para que nenhum grupo monopolize as aprovações.

## Fluxo de aprovação de jogos

1. **Envio de proposta**
   - El WG de Tooling sube el bundle candidato `manifest_blake3.json` mas el diff del fixture
     para registro on-chain via `sorafs.fixtureProposal`.
   - A proposta registra o resumo BLAKE3, a versão semântica e as notas de mudança.
2. **Revisão e votação**
   - El Painel de Infraestructura recebe a atribuição através da cola de tarefas do
     Parlamento.
   - Os membros do painel de inspeção de artefatos de CI, realizam testes de paridade e emitem
     votos ponderados em cadeia.
3. **Finalização**
   - Uma vez alcançado o quorum, o tempo de execução emite um evento de aprovação que inclui o
     resumo canônico do manifesto e compromisso Merkle da carga útil do dispositivo.
   - O evento é refletido no registro de SoraFS para que os clientes possam obter o
     manifesto mas recentemente aprovado pelo Parlamento.
4. **Distribuição**
   - Os auxiliares de CLI (`cargo xtask sorafs-fetch-fixture`) treinam o manifesto aprovado desde
     NexusRPC. As constantes JSON/TS/Go do repositório são mantidas sincronizadas para
     reexecutar `export_vectors` e validar o resumo contra o registro na rede.

## Fluxo de trabalho para desenvolvedores

- Jogos Regenera com:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- Use o ajudante de busca do Parlamento para baixar o envelope aprovado, verifique
  firmas y refrescar locais de luminárias. Apunta `--signatures` ao envelope publicado por el
  Parlamento; o ajudante resolve o manifesto acompanhante, recomputa o resumo BLAKE3 e
  impone o perfil canônico `sorafs.sf1@1.0.0`.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```Passe `--manifest` se o manifesto estiver ativo em outro URL. Los envelopes sin firma se rechazan
salvo que se configure `--allow-unsigned` para smoke executa locales.

- Ao validar um manifesto através de um gateway de teste, aponte para Torii em vez de cargas úteis
  localidades:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- O CI local não requer uma lista `signer.json`.
  `ci/check_sorafs_fixtures.sh` compara o estado do repo com o último compromisso
  on-chain e falha quando divergem.

## Notas de governança

- La constitucion del Parlamento gobierna quorum, rotacion y escalamiento; não é necessário
  configurando o nível da caixa.
- As reversões de emergência são feitas através do painel de moderação do Parlamento.
  O Painel de Infraestrutura apresenta uma proposta de reversão que faz referência ao resumo
  anterior do manifesto, substituindo o lançamento uma vez aprovado.
- As aprovações históricas permanecem disponíveis no registro de SoraFS para
  replay forense.

## Perguntas frequentes

- **Onde foi `signer.json`?**  
  Se eliminar. Toda a atribuição de firmas vive on-chain; `manifest_signatures.json`
  no repositório é apenas um dispositivo de desenvolvedor que deve coincidir com o último
  evento de aprovação.

- **Seguimos exigindo firmas Ed25519 localidades?**  
  As aprovações do Parlamento são armazenadas como artefatos on-chain. Los Angeles jogos
  existem locais para reprodução, mas são válidos no resumo do Parlamento.

- **Como monitorar as aprovações dos equipamentos?**  
  Inscreva-se no evento `ParliamentFixtureApproved` ou consulte o registro via RPC Nexus
  para recuperar o resumo atual do manifesto e a chamada do painel.