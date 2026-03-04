---
lang: pt
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fe68de8593c0eac177dbf727e4f47adc98021c9a6eceb9f851abcabfa549fd23
source_last_modified: "2025-12-14T10:43:49.840406+00:00"
translation_last_reviewed: 2026-01-01
---

import ExplorerAddressCard from '@site/src/components/ExplorerAddressCard';

:::note Fonte canonica
Esta pagina espelha `docs/source/sns/address_display_guidelines.md` e agora serve
como a copia canonica do portal. O arquivo fonte permanece para PRs de
traducao.
:::

Carteiras, exploradores e exemplos de SDK devem tratar enderecos de conta como
payloads imutaveis. O exemplo da carteira retail Android em
`examples/android/retail-wallet` agora demonstra o padrao de UX exigido:

- **Dois alvos de copia.** Envie dois botoes de copia explicitos: IH58
  (preferido) e a forma comprimida somente Sora (`sora...`, segunda melhor opcao). IH58 e sempre seguro para
  compartilhar externamente e alimenta o payload do QR. A variante comprimida
  deve incluir um aviso inline porque so funciona dentro de apps compatveis com
  Sora. O exemplo de carteira retail Android liga ambos os botoes Material e seus
  tooltips em
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`, e a
  demo iOS SwiftUI espelha o mesmo UX via `AddressPreviewCard` em
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **Monospace, texto selecionavel.** Renderize ambas as strings com uma fonte
  monospace e `textIsSelectable="true"` para que os usuarios possam inspecionar
  valores sem invocar um IME. Evite campos editaveis: IMEs podem reescrever kana
  ou injetar pontos de codigo de largura zero.
- **Dicas de dominio padrao implicito.** Quando o seletor aponta para o dominio
  implicito `default`, mostre uma legenda lembrando os operadores de que nenhum
  sufixo e necessario. Exploradores tambem devem destacar o label de dominio
  canonico quando o seletor codifica um digest.
- **QR IH58.** Codigos QR devem codificar a string IH58. Se a geracao do QR
  falhar, mostre um erro explicito em vez de uma imagem em branco.
- **Mensageria da area de transferencia.** Depois de copiar a forma comprimida,
  emita um toast ou snackbar lembrando os usuarios de que ela e somente Sora e
  propensa a distorcao por IME.

Seguir essas guardrails evita corrupcao Unicode/IME e atende aos criterios de
aceitacao do roadmap ADDR-6 para UX de carteiras/exploradores.

## Capturas de tela de referencia

Use as referencias a seguir durante revisoes de localizacao para garantir que os
rotulos de botoes, tooltips e avisos fiquem alinhados entre plataformas:

- Referencia Android: `/img/sns/address_copy_android.svg`

  ![Referencia Android de dupla copia](/img/sns/address_copy_android.svg)

- Referencia iOS: `/img/sns/address_copy_ios.svg`

  ![Referencia iOS de dupla copia](/img/sns/address_copy_ios.svg)

## Helpers de SDK

Cada SDK expoe um helper de conveniencia que retorna as formas IH58 e comprimida
junto com a string de aviso para que as camadas UI fiquem consistentes:

- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- JavaScript inspector: `inspectAccountId(...)` retorna a string de aviso
  comprimida e a anexa a `warnings` quando chamadores fornecem um literal
  `sora...`, para que dashboards de carteira/explorador possam exibir o aviso
  somente Sora durante fluxos de colagem/validacao em vez de apenas quando geram
  a forma comprimida por conta propria.
- Python: `AccountAddress.display_formats(network_prefix: int = 753)`
- Swift: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
- Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

Use estes helpers em vez de reimplementar a logica de encode nas camadas UI. O
helper JavaScript tambem expoe um payload `selector` em `domainSummary` (`tag`,
`digest_hex`, `registry_id`, `label`) para que UIs indiquem se um seletor e
Local-12 ou respaldado por registro sem reparsear o payload bruto.

## Demo de instrumentacao do explorador

<ExplorerAddressCard />

Exploradores devem espelhar o trabalho de telemetria e acessibilidade da
carteira:

- Aplique `data-copy-mode="ih58|compressed|qr"` aos botoes de copia para que
  front-ends possam emitir contadores de uso junto com a metrica Torii
  `torii_address_format_total`. O componente demo acima dispara um evento
  `iroha:address-copy` com `{mode,timestamp}`; conecte isso ao seu pipeline de
  analitica/telemetria (por exemplo, envie para Segment ou um coletor NORITO)
  para que dashboards possam correlacionar o uso de formatos de endereco do
  servidor com modos de copia do cliente. Replique tambem os contadores de
  dominio Torii (`torii_address_domain_total{domain_kind}`) no mesmo feed para
  que revisoes de aposentadoria Local-12 possam exportar uma prova de 30 dias
  `domain_kind="local12"` diretamente do painel `address_ingest` do Grafana.
- Emparelhe cada controle com pistas `aria-label`/`aria-describedby` distintas
  que expliquem se um literal e seguro para compartilhar (IH58) ou somente Sora
  (comprimido). Inclua a legenda do dominio implicito na descricao para que a
  tecnologia assistiva mostre o mesmo contexto exibido visualmente.
- Exponha uma regiao viva (por exemplo, `<output aria-live="polite">...</output>`)
  anunciando resultados de copia e avisos, alinhando o comportamento do
  VoiceOver/TalkBack ja conectado nos exemplos Swift/Android.

Esta instrumentacao satisfaz ADDR-6b ao provar que operadores podem observar
tanto a ingestao Torii quanto os modos de copia do cliente antes que os
seletores Local sejam desativados.

## Toolkit de migracao Local -> Global

Use o [toolkit Local -> Global](local-to-global-toolkit.md) para automatizar a
revisao e conversao de seletores Local alternativos. O helper emite tanto o relatorio
JSON de auditoria quanto a lista convertida IH58/comprimida que operadores
anexam a tickets de readiness, enquanto o runbook associado vincula dashboards
Grafana e regras Alertmanager que controlam o cutover em modo estrito.

## Referencia rapida do layout binario (ADDR-1a)

Quando SDKs expuserem tooling avancado de enderecos (inspectores, dicas de
validacao, builders de manifest), aponte desenvolvedores para o formato wire
canonico em `docs/account_structure.md`. O layout sempre e
`header · selector · controller`, onde os bits do header sao:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (bits 7-5) hoje; valores nao zero sao reservados e devem
  levantar `AccountAddressError::InvalidHeaderVersion`.
- `addr_class` distingue controladores simples (`0`) de multisig (`1`).
- `norm_version = 1` codifica as regras do seletor Norm v1. Norms futuras
  reutilizarao o mesmo campo de 2 bits.
- `ext_flag` e sempre `0`; bits ativos indicam extensoes de payload nao
  suportadas.

O seletor segue imediatamente o header:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

UIs e SDKs devem estar prontas para exibir o tipo de seletor:

- `0x00` = dominio padrao implicito (sem payload).
- `0x01` = digest local (12-byte `blake2s_mac("SORA-LOCAL-K:v1", label)`).
- `0x02` = entrada de registro global (`registry_id:u32` big-endian).

Exemplos hex canonicos que ferramentas de carteira podem linkar ou embutir em
 docs/tests:

| Tipo de seletor | Hex canonico |
|---------------|---------------|
| Implicito padrao | `0x02000001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| Digest local (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| Registro global (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

Veja `docs/source/references/address_norm_v1.md` para a tabela completa de
seletor/estado e `docs/account_structure.md` para o diagrama completo de bytes.

## Impor formas canonicas

Operadores que convertem codificacoes Local alternativas para IH58 canonico ou
strings comprimidas devem seguir o workflow CLI documentado em ADDR-5:

1. `iroha tools address inspect` agora emite um resumo JSON estruturado com IH58,
   comprimido e payloads hex canonicos. O resumo tambem inclui um objeto
   `domain` com campos `kind`/`warning` e ecoa qualquer dominio fornecido via o
   campo `input_domain`. Quando `kind` e `local12`, a CLI imprime um aviso em
   stderr e o resumo JSON ecoa a mesma orientacao para que pipelines CI e SDKs
   possam exibi-la. Passe `--append-domain` sempre que quiser reproduzir a
   codificacao convertida como `<ih58>@<domain>`.
2. SDKs podem exibir o mesmo aviso/resumo via o helper JavaScript:

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.ih58.value, summary.compressed);
   ```
  O helper preserva o prefixo IH58 detectado do literal a menos que voce forneca
  explicitamente `networkPrefix`, entao resumos para redes nao padrao nao sao
  re-renderizados silenciosamente com o prefixo padrao.

3. Converta o payload canonico reutilizando os campos `ih58.value` ou
   `compressed` do resumo (ou solicite outra codificacao via `--format`). Essas
   strings ja sao seguras para compartilhamento externo.
4. Atualize manifests, registros e documentos voltados ao cliente com a forma
   canonica e notifique as contrapartes de que seletores Local serao rejeitados
   quando o cutover for concluido.
5. Para conjuntos de dados em massa, execute
   `iroha tools address audit --input addresses.txt --network-prefix 753`. O comando
   le literais separados por nova linha (comentarios iniciados com `#` sao
   ignorados, e `--input -` ou nenhum flag usa STDIN), emite um relatorio JSON
   com resumos canonicos/IH58/comprimidos para cada entrada e conta erros de
   parse e avisos de dominio Local. Use `--allow-errors` ao auditar dumps alternativos
   com linhas lixo, e trave a automacao com `--fail-on-warning` quando os
   operadores estiverem prontos para bloquear seletores Local no CI.
6. Quando precisar de reescrita linha a linha, use
  Para planilhas de remediacao de seletores Local, use
  para exportar um CSV `input,status,format,...` que destaca codificacoes
  canonicas, avisos e falhas de parse em uma unica passada.
   O helper ignora linhas nao Local por padrao, converte cada entrada restante
   para a codificacao solicitada (IH58/comprimido/hex/JSON), e preserva o dominio
   original quando `--append-domain` e definido. Combine com `--allow-errors`
   para continuar a varredura mesmo quando um dump contem literais malformados.
7. A automacao CI/lint pode executar `ci/check_address_normalize.sh`, que extrai
   seletores Local de `fixtures/account/address_vectors.json`, converte via
   `iroha tools address normalize`, e reexecuta
   `iroha tools address audit --fail-on-warning` para provar que releases nao emitem
   mais digests Local.

`torii_address_local8_total{endpoint}` junto com
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}`, e o painel Grafana
`dashboards/grafana/address_ingest.json` fornecem o sinal de enforcement: quando
os dashboards de producao mostram zero envios Local legitimos e zero colisoes
Local-12 por 30 dias consecutivos, Torii vai virar o gate Local-8 para hard-fail
na mainnet, seguido por Local-12 quando dominios globais tiverem entradas de
registro correspondentes. Considere a saida do CLI como aviso ao operador para
esse congelamento - a mesma string de aviso e usada em tooltips de SDK e
automacao para manter paridade com os criterios de saida do roadmap. Torii agora
clusters dev/test ao diagnosticar regressions. Continue espelhando
`torii_address_domain_total{domain_kind}` no Grafana
(`dashboards/grafana/address_ingest.json`) para que o pacote de evidencia ADDR-7
prove que `domain_kind="local12"` permaneceu em zero na janela requerida de 30
 dias antes de a mainnet desativar seletores alternativos. O pacote Alertmanager
(`dashboards/alerts/address_ingest_rules.yml`) adiciona tres guardrails:

- `AddressLocal8Resurgence` pagina sempre que um contexto reporta um incremento
  Local-8 novo. Pare rollouts de modo estrito, localize a superficie SDK
  responsavel no dashboard e, se necessario, defina temporariamente
  padrao (`true`).
- `AddressLocal12Collision` dispara quando dois labels Local-12 fazem hash para
  o mesmo digest. Pause promocoes de manifest, execute o toolkit Local -> Global
  para auditar o mapeamento de digests e coordene com a governanca Nexus antes
  de reemitir a entrada de registro ou reativar rollouts downstream.
- `AddressInvalidRatioSlo` avisa quando a proporcao invalida em toda a frota
  (excluindo rejeicoes Local-8/strict-mode) excede o SLO de 0.1% por dez minutos.
  Use `torii_address_invalid_total` para localizar o contexto/razao responsavel
  e coordene com a equipe SDK proprietaria antes de reativar o modo estrito.

### Trecho de nota de release (carteira e explorador)

Inclua o seguinte bullet nas notas de release de carteira/explorador ao enviar o
cutover:

> **Enderecos:** Adicionado o helper `iroha tools address normalize --only-local --append-domain`
> e conectado no CI (`ci/check_address_normalize.sh`) para que pipelines de
> carteira/explorador possam converter seletores Local alternativos para formas
> canonicas IH58/comprimidas antes de Local-8/Local-12 serem bloqueados na
> mainnet. Atualize quaisquer exports personalizados para rodar o comando e
> anexar a lista normalizada ao bundle de evidencia de release.
