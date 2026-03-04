---
lang: pt
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

importar ExplorerAddressCard de '@site/src/components/ExplorerAddressCard';

:::nota Fonte canônica
Esta página reflete `docs/source/sns/address_display_guidelines.md` e agora sirve
como a cópia canônica do portal. O arquivo fonte é mantido para PRs de
tradução.
:::

Os boletos, exploradores e exemplos de SDK devem tratar as direções de
conta como cargas úteis imutáveis. O exemplo de fatura de varejo do Android em
`examples/android/retail-wallet` agora mostre o patrono de UX necessário:

- **Dos objetivos de cópia.** Envia dos botões de cópia explicitos: IH58
  (preferido) e a forma comprimida apenas Sora (`sora...`, segunda melhor opção).
  IH58 é sempre seguro para compartilhar externamente e alimentar a carga útil do QR. A variante
  obrigatória deve incluir uma advertência em linha porque só funciona dentro
  de aplicativos com suporte de Sora. O exemplo de fatura de varejo do Android
  conecte ambos os botões Material e suas dicas de ferramentas em
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`, e la
  demo iOS SwiftUI reflete o mesmo UX via `AddressPreviewCard` dentro de
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **Monoespaçado, texto selecionável.** Renderiza ambas as cadeias com uma fonte
  monospace e `textIsSelectable="true"` para que os usuários possam inspecionar
  valores sem invocar um IME. Evita campos editáveis: o IME pode ser reescrito
  kana o inyectar pontos de código de ancho zero.
- **Pistas de domínio por defeito implícito.** Quando o seletor é colocado em
  domínio implícito `default`, mostra uma legenda gravada pelos operadores
  que não se requer sufijo. Os exploradores também devem realçar a etiqueta
  de domínio canônico quando o seletor codifica um resumo.
- **QR IH58.** Os códigos QR devem codificar a cadeia IH58. Si la geração
  do QR falha, mostra um erro explícito no lugar de uma imagem em branco.
- **Mensajeria del portapapeles.** Depois de copiar o formato comprado, emite
  un brinde o snackbar registrando aos usuários que estão sozinhos Sora e propens a la
  distorção por IME.

Seguir estas diretrizes evita a corrupção Unicode/IME e satisfaz os critérios de
aceitação do roteiro ADDR-6 para UX de boletos/exploradores.

## Capturas de tela de referência

Use as seguintes referências durante as revisões de localização para garantir
que as etiquetas de botões, dicas de ferramentas e advertências se mantêm alinhadas
entre plataformas:

- Referência Android: `/img/sns/address_copy_android.svg`

  ![Referência Android de cópia dupla](/img/sns/address_copy_android.svg)

- Referência iOS: `/img/sns/address_copy_ios.svg`

  ![Referência iOS de cópia dupla](/img/sns/address_copy_ios.svg)

## Ajudantes do SDK

Cada SDK apresenta um auxiliar de conveniência que desenvolve as formas IH58 e
comprometido junto com a cadência de advertência para que as capas UI sejam mantidas
consistentes:- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- Inspetor JavaScript: `inspectAccountId(...)` retorna a cadeia de advertência
  compactada e adicionada a `warnings` quando as chamadas fornecem um
  literal `sora...`, de modo que os exploradores/dashboards de boletos podem
  mostrar o aviso solo Sora durante os fluxos de pegado/validação em lugar de
  Faça-o sozinho quando gerar a forma comprometida por sua conta.
-Python: `AccountAddress.display_formats(network_prefix: int = 753)`
- Rápido: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
-Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

Use esses ajudantes em vez de reimplementar a lógica de codificação nas capas da UI.
O auxiliar de JavaScript também expõe uma carga útil `selector` e `domainSummary`
(`tag`, `digest_hex`, `registry_id`, `label`) para que as UIs possam indicar se
um seletor é Local-12 ou respaldado por registro sem retornar para analisar a carga útil
em bruto.

## Demonstração de instrumentação do explorador



Os exploradores devem refletir sobre o trabalho de telemetria e a acessibilidade do
boleto:

- Aplique `data-copy-mode="ih58|compressed|qr"` aos botões de cópia para que
  os front-ends podem emitir contadores de uso junto com a métrica Torii
  `torii_address_format_total`. El componente demo anterior despacha un evento
  `iroha:address-copy` com `{mode,timestamp}`: conecte este ao seu pipeline de
  analitica/telemetria (por exemplo, envie um Segmento a um coletor respaldado
  por NORITO) para que os dashboards possam correlacionar o uso de formatos de
  direção do servidor com os modos de cópia do cliente. Também reflita sobre eles
  contadores de domínio de Torii (`torii_address_domain_total{domain_kind}`) en
  o mesmo feed para que as revisões de retiro do Local-12 possam exportar
  uma teste de 30 dias `domain_kind="local12"` diretamente da mesa
  `address_ingest` de Grafana.
- Compare cada controle com pistas `aria-label`/`aria-describedby` distintas que
  explique se é literalmente seguro para compartilhar (IH58) ou apenas Sora
  (comprimido). Inclui a legenda de domínio implícito na descrição para
  que a tecnologia assistiva mostra o mesmo contexto visual.
- Expor uma região viva (por exemplo, `<output aria-live="polite">...</output>`)
  anunciando resultados de cópia e advertências, igualando o comportamento de
  VoiceOver/TalkBack está conectado aos exemplos Swift/Android.

Esta instrumentação satisfaça ADDR-6b ao demonstrar que os operadores podem
observe tanto a ingestão Torii como os modos de cópia do cliente antes de
que desativa os seletores locais.

## Toolkit de migração Local -> Global

Use o [kit de ferramentas Local -> Global](local-to-global-toolkit.md) para automatizar
revisão e conversão de seletores herdados locais. El ajudante emite tanto el
relatório de auditoria JSON como a lista convertida IH58/comprimida que los
operadores auxiliares aos tickets de prontidão, enquanto o runbook
acompanhando os painéis de Grafana e as regras do Alertmanager que
controla a transferência em modo restrito.## Referência rápida do layout binário (ADDR-1a)

Quando os SDKs apresentam ferramentas avançadas de direções (inspetores, pistas de
validação, construtores de manifesto), dirijan a los desarrolladores no formato
fio canônico capturado em `docs/account_structure.md`. O layout sempre está
`header · selector · controller`, onde estão os bits do cabeçalho:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (bits 7-5) hoje; valores no cero estão reservados e deben
  lançador `AccountAddressError::InvalidHeaderVersion`.
- `addr_class` distingue entre drivers simples (`0`) e multisig (`1`).
- `norm_version = 1` codifica as regras do seletor Norm v1. Normas futuras
  reutilizara o mesmo campo de 2 bits.
- `ext_flag` sempre é `0`; bits ativos indicam extensões de carga útil não
  suportados.

O seletor segue imediatamente para o cabeçalho:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

As UIs e SDKs devem estar listas para mostrar o tipo de seletor:

- `0x00` = domínio por defeito implícito (sem carga útil).
- `0x01` = resumo local (`blake2s_mac("SORA-LOCAL-K:v1", label)` de 12 bytes).
- `0x02` = entrada de registro global (`registry_id:u32` big-endian).

Exemplos hexadecimais canônicos de que as ferramentas de fatura podem enlaçar o
inserir em docs/tests:

| Tipo de seletor | Hex canônico |
|---------------|---------------|
| Implícito por defeito | `0x02000001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| Resumo local (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| Registro global (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

Consulte `docs/source/references/address_norm_v1.md` para a tabela completa de
selector/estado e `docs/account_structure.md` para o diagrama de bytes completo.

## Forzar formas canônicas

Os operadores que convertem codificações locais herdadas para IH58 canônico ou
Cadenas comprimidas devem seguir o fluxo CLI documentado em ADDR-5:

1. `iroha tools address inspect` agora emite um currículo JSON estruturado com IH58,
   compactado e payloads hexadecimais canônicos. O currículo também inclui um objeto
   `domain` com campos `kind`/`warning` e reflexo de qualquer domínio fornecido
   através do campo `input_domain`. Quando `kind` é `local12`, o CLI imprime um
   advertencia a stderr e o resumo JSON reflete o mesmo guia para que los
   pipelines de CI e SDKs podem ser exibidos. Pasa `--append-domain` quando
   quero que a codificação convertida seja reproduzida como `<ih58>@<domain>`.
2. Os SDKs podem mostrar a mesma advertência/resumo por meio do ajudante de
   JavaScript:

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.ih58.value, summary.compressed);
   ```
  O ajudante conserva o prefijo IH58 detectado do literal a menos que
  proporciones explicitamente `networkPrefix`, por isso que os currículos para
  redes no default não se re-renderizan silenciosamente com o prefijo por
  defeito.3. Converta o payload canônico reutilizando os campos `ih58.value` ou
   `compressed` do currículo (ou solicitação de outra codificação via `--format`). Estas
   Cadenas são seguras para compartilhar externamente.
4. Atualize manifestos, registros e documentos de rosto do cliente com o
   forma canônica e notificar as contrapartes que os seletores locais serão
   rechazados uma vez completado a transição.
5. Para conjuntos de dados massivos, executados
   `iroha tools address audit --input addresses.txt --network-prefix 753`. O comando
   lee literais separados por nova linha (comentários que são compilados com `#` se
   ignorante, e `--input -` ou ningun flag usa STDIN), emite um relatório JSON com
   resumos canônicos/IH58/comprimidos para cada entrada, e possíveis erros de
   analisar e anúncios de domínio local. Usa `--allow-errors` al auditar dumps
   herdados que contêm filas basura, e bloqueiam a automatização com
   `--fail-on-warning` quando os operadores estão listados para bloquear
   seletores locais em CI.
6. Quando você precisa de uma reescritura linha a linha, EUA
  Para horas de cálculo de remediação de seletores Local, EUA
  para exportar um CSV `input,status,format,...` que realça codificações
  canônicas, advertências e falhas de análise em uma única passagem.
   El helper omite filas no Local por defeito, converta cada entrada restante
   na codificação solicitada (IH58/comprimido/hex/JSON), e preserva o domínio
   original quando se usa `--append-domain`. Combinação com `--allow-errors` para
   seguir escaneando até mesmo quando um dump contém literais mal formados.
7. A automação de CI/lint pode executar `ci/check_address_normalize.sh`,
   que extrae os seletores locais de `fixtures/account/address_vectors.json`,
   convierte via `iroha tools address normalize`, e vuelve a ejecutar
   `iroha tools address audit --fail-on-warning` para demonstrar que os releases ya no
   emiten digests Local.

`torii_address_local8_total{endpoint}` junto com
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}`, e a tabela Grafana
`dashboards/grafana/address_ingest.json` fornece o sinal de cumprimento:
quando os painéis de produção mostram zero envios locais legítimos e zero
colisões Local-12 durante 30 dias consecutivos, Torii alterou o portão Local-8
para cair duro na mainnet, seguido por Local-12 quando os domínios globais
cuenten con entradas de registro correspondentes. Considere a saída do CLI
como o aviso para operadores deste congelamento: la misma cadena de
a advertência é usada nas dicas de ferramentas do SDK e na automação para manter a paridade com
os critérios de saída do roteiro. Torii agora usa por defeito
quando são feitas regressões diagnósticas. Siga refletindo
`torii_address_domain_total{domain_kind}` e Grafana
(`dashboards/grafana/address_ingest.json`) para que o pacote de evidências
ADDR-7 mostra que `domain_kind="local12"` permanece em zero durante
ventana requerida de 30 dias antes de que mainnet desabilite os seletores
(`dashboards/alerts/address_ingest_rules.yml`) agrega três guarda-corpos:- `AddressLocal8Resurgence` página quando um contexto reporta um incremento
  Afresco local-8. Deten los rollouts de modo estricto, localiza el SDK responsable
  no painel e, se necessário, configure temporariamente
  o padrão (`true`).
- `AddressLocal12Collision` é diferente quando as etiquetas Local-12 hacen hash
  al mismo resumo. Pausar as promoções do manifesto, executar o kit de ferramentas
  Local -> Global para auditar o mapa de resumos e coordenar com o governo
  de Nexus antes de reemitir a entrada de registro ou reativar rollouts aguas
  abaixo.
- `AddressInvalidRatioSlo` avisa quando a proporção de inválidos em toda a
  flota (excluindo rechazos Local-8/strict-mode) excede o SLO de 0,1% durante
  dez minutos. Usa `torii_address_invalid_total` para identificar o
  contexto/razão responsável e coordenado com o equipamento SDK proprietário antes de
  reativar o modo restrito.

### Fragmento para notas de lançamento (billetera y explorador)

Inclui o próximo marcador nas notas de lançamento do billetera/explorador
ao publicar o corte:

> **Instruções:** Adicione o ajudante `iroha tools address normalize --only-local --append-domain`
> e se conectou em CI (`ci/check_address_normalize.sh`) para que os pipelines de
> billetera/explorador pode converter seletores locais herdados em formas
> canonicas IH58/comprimidas antes de Local-8/Local-12 ser bloqueado na rede principal.
> Atualizar qualquer exportação personalizada para executar o comando e
> adjunta a lista normalizada ao pacote de evidências de lançamento.