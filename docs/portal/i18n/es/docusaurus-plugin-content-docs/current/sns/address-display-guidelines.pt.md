---
lang: es
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

importar ExplorerAddressCard desde '@site/src/components/ExplorerAddressCard';

:::nota Fuente canónica
Esta página espelha `docs/source/sns/address_display_guidelines.md` y ahora sirve
como copia canónica del portal. El archivo fuente permanece para PRs de
traducao.
:::

Carteiras, exploradores e exemplos de SDK devem tratar enderecos de conta como
cargas útiles imutaveis. El ejemplo de la tarjeta minorista Android en
`examples/android/retail-wallet` ahora demuestra el padrao de UX exigido:- **Dois alvos de copia.** Envie dos botoes de copia explícita: IH58
  (preferido) e a forma comprimida somente Sora (`sora...`, segunda melhor opcao). IH58 y siempre seguro para
  compartilhar externamente y alimentar o carga útil de QR. Una variante comprimida
  debe incluir un aviso en línea porque funciona dentro de aplicaciones compatibles con
  Sora. El ejemplo de carteira retail Android liga ambos os botoes Material e seus
  información sobre herramientas
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`, y un
  demostración iOS SwiftUI desarrollada o mesmo UX a través de `AddressPreviewCard` en
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **Monoespacio, selección de texto.** Renderizar ambas como cadenas con una fuente
  monospace e `textIsSelectable="true"` para que los usuarios puedan inspeccionar
  valores sin invocar un IME. Evite campos editaveis: IMEs podem reescrever kana
  o inyectar puntos de código de longitud cero.
- **Dicas de dominio padrao implícito.** Quando o seletor aponta para o dominio
  implícito `default`, mostre uma legenda lembrando os operadores de que nenhum
  sufijo y necesario. Exploradores tambem devem destacar o etiqueta de dominio
  canónico cuando el selector codifica un resumen.
- **QR IH58.** Los códigos QR deben codificarse en una cadena IH58. Se a geracao do QR
  Falhar, muestra un error explícito en vez de una imagen en blanco.
- **Mensajería del área de transferencia.** Después de copiar en forma comprimida,emita una tostada o un snack bar lembrando os usuarios de que ela e somente Sora e
  propensa a distorsión por IME.

Seguir estas barreras evita la corrupción Unicode/IME y atiende a los criterios de
Aceitacao do roadmap ADDR-6 para UX de carteiras/exploradores.

## Capturas de tela de referencia

Utilice como referencias a seguir durante revisiones de localización para garantizar que os
rotulos de botoes, tooltips y avisos fiquem alinhados entre plataformas:

- Referencia Android: `/img/sns/address_copy_android.svg`

  ![Referencia Android de doble copia](/img/sns/address_copy_android.svg)

- Referencia iOS: `/img/sns/address_copy_ios.svg`

  ![Referencia iOS de doble copia](/img/sns/address_copy_ios.svg)

## Ayudantes de SDK

Cada SDK expone una ayuda de conveniencia que retorna como formas IH58 y comprimida
junto com a string de aviso para que as camadas UI fiquem consistentes:

- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- Inspector de JavaScript: `inspectAccountId(...)` retorna una cadena de aviso
  comprimida e a anexa a `warnings` cuando chamadores fornecem um literal
  `sora...`, para que los paneles de carteira/explorador possam exibir o aviso
  somente Sora durante fluxos de colagem/validacao em vez de apenas quando germam
  a forma comprimida por contacto propio.
- Pitón: `AccountAddress.display_formats(network_prefix: int = 753)`
- Rápido: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
-Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)Utilice estos ayudantes en vez de reimplementar la lógica de codificación de la interfaz de usuario de cada camada. oh
El asistente JavaScript también expone una carga útil `selector` en `domainSummary` (`tag`,
`digest_hex`, `registry_id`, `label`) para que las UI indiquen un selector e
Local-12 o respaldado por registro sin reparar la carga útil bruta.

## Demostración de instrumentación del explorador



Exploradores deben desarrollar o trabajar en telemetría y accesibilidad
carteira:- Aplique `data-copy-mode="ih58|compressed|qr"` aos botoes de copia para que
  front-ends possam emitir contadores de uso junto con métrica Torii
  `torii_address_format_total`. O componente demo acima dispara um evento
  `iroha:address-copy` con `{mode,timestamp}`; conectar isso ao su tubería de
  analitica/telemetria (por ejemplo, envie para Segment o un coletor NORITO)
  para que los paneles de control puedan correlacionar el uso de formatos de endereco do
  servidor con modos de copia del cliente. Replique también los contadores de
  dominio Torii (`torii_address_domain_total{domain_kind}`) sin mesmo feed para
  que revisoes de aposentadora Local-12 possam exportar uma prova de 30 dias
  `domain_kind="local12"` directamente al panel `address_ingest` a Grafana.
- Utilice cada control con pistas `aria-label`/`aria-describedby` distintas
  que expliquem se um literal e seguro para compartir (IH58) ou somente Sora
  (comprimido). Incluye la leyenda del dominio implícito en la descripción para que a
  tecnologia assistiva mostre o mesmo contexto exibido visualmente.
- Exponha uma regiao viva (por ejemplo, `<output aria-live="polite">...</output>`)
  anunciando resultados de copia y avisos, alinhando o comportamento do
  VoiceOver/TalkBack está conectado con ejemplos de Swift/Android.

Esta instrumentacao satisface ADDR-6b ao provar que los operadores pueden observar
tanto a ingestao Torii cuantos modos de copia del cliente antes que os
seletores Local sejam desativados.## Kit de herramientas de migración Local -> Global

Utilice o [kit de herramientas Local -> Global](local-to-global-toolkit.md) para automatizar un
revisao e conversao de seletores locales alternativos. O ayudante emite tanto o relatorio
JSON de auditoria cuanto a lista convertida IH58/comprimida que operadores
Anexar los tickets de preparación, cuánto o runbook asociado vincula los paneles de control.
Grafana y registra Alertmanager que controla el corte en modo estrito.

## Referencia rápida del diseño binario (ADDR-1a)

Cuando SDKs exuserem herramientas avancado de enderecos (inspectores, dicas de
validacao, builders de manifest), aponte desenvolvedores para el formato wire
canónico en `docs/account_structure.md`. O diseño sempre e
`header · selector · controller`, los bits del sistema operativo hacen el encabezado:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (bits 7-5) hoy; valores nao cero sao reservados e devem
  levantar `AccountAddressError::InvalidHeaderVersion`.
- `addr_class` distingue controladores simples (`0`) de multifirma (`1`).
- `norm_version = 1` codificado según las reglas del selector Norm v1. Normas futuras
  reutilizarao o mesmo campo de 2 bits.
- `ext_flag` y siempre `0`; bits activos indican extensos de carga útil nao
  apoyados.

El selector cambia inmediatamente al encabezado:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

Las UI y SDK deben estar próximas a mostrar el tipo de selector:- `0x00` = dominio padrao implícito (sem payload).
- `0x01` = resumen local (`blake2s_mac("SORA-LOCAL-K:v1", label)` de 12 bytes).
- `0x02` = entrada de registro global (`registry_id:u32` big-endian).

Ejemplos de hex canonicos que ferramentas de carteira podem linkar ou embutir em
 documentos/pruebas:

| Tipo de selector | Hex canónico |
|---------------|---------------|
| Implícito padrao | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| Resumen local (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| Registro mundial (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

Veja `docs/source/references/address_norm_v1.md` para una tabla completa de
selector/estado e `docs/account_structure.md` para el diagrama completo de bytes.

## Impor formas canónicas

Operadores que convierten códigos locales alternativos para IH58 canonico o
Las cadenas comprimidas deben seguir el flujo de trabajo CLI documentado en ADDR-5:

1. `iroha tools address inspect` ahora emite un resumen JSON estructurado con IH58,
   comprimido e payloads hex canonicos. El resumen también incluye un objeto
   `domain` com campos `kind`/`warning` e ecoa qualquer dominio fornecido vía o
   campo `input_domain`. Cuando `kind` e `local12`, una CLI imprime un aviso en
   stderr y resumen JSON ecoa a mesma orientacao para que pipelines CI y SDK
   possam exibi-la. Passe `legacy  suffix` siempre que quiera reproducir un
   codificación convertida como `<ih58>@<domain>`.
2. Los SDK pueden exhibirse o mesmo aviso/resumo a través del asistente JavaScript:```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.ih58.value, summary.compressed);
   ```
  O helper preserva o prefixo IH58 detectado do literal a menos que voce forneca
  explícitamente `networkPrefix`, entao resumos para redes nao padrao nao sao
  re-renderizados silenciosamente con el prefijo padrao.3. Convierta la carga útil canónica reutilizando los campos `ih58.value` o
   `compressed` do resumo (o solicita otra codificación a través de `--format`). essas
   strings ja sao seguras para compartilhamento externo.
4. Actualizar manifiestos, registros y documentos voltados al cliente con forma
   canonica e notifique as contrapartes de que seletores Local serao rejeitados
   quando o cutover for conclused.
5. Para conjuntos de dados em masa, ejecute
   `iroha tools address audit --input addresses.txt --network-prefix 753`. O comando
   le literais separados por nova linha (comentarios iniciados com `#` sao
   ignorados, e `--input -` o nenhum flag usa STDIN), emite un archivo JSON
   com resumos canonicos/IH58/comprimidos para cada entrada e conta errores de
   parse e avisos de dominio Local. Utilice `--allow-errors` para auditar volcados alternativos
   com linhas lixo, y trave a automacao com `strict CI post-check` quando os
   Los operadores estiverem prontos para bloquear selectores locales no CI.
6. Cuando necesites reescrita línea a línea, utiliza
  Para planilhas de remediacao de seletores Local, use
  para exportar un CSV `input,status,format,...` que destaca codificados
  canónicas, avisos y falhas de parse em uma unica passada.
   O helper ignora linhas nao Local por padrao, converte cada entrada restante
   para una codificación solicitada (IH58/comprimido/hex/JSON), y preserva o dominiooriginal cuando `legacy  suffix` y definido. Combinar com `--allow-errors`
   para continuar a varredura mesmo quando um dump contem literais malformados.
7. Un CI/lint automático puede ejecutar `ci/check_address_normalize.sh`, que extrai
   selectores Local de `fixtures/account/address_vectors.json`, convertir vía
   `iroha tools address normalize`, y reejecuta
   `iroha tools address audit` para probar que libera nao emitem
   mais digiere local.

`torii_address_local8_total{endpoint}` junto con
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}`, y el panel Grafana
`dashboards/grafana/address_ingest.json` fornecem o señal de cumplimiento: cuando
os paneles de producción mostram zero envios Local legitimos e zero colisoes
Local-12 por 30 días consecutivos, Torii vai virar o gate Local-8 para hard-fail
en mainnet, seguido por Local-12 cuando dominios globales tienen entradas de
registro correspondientes. Considere a dicho CLI como aviso al operador para
esse congelamento - a mesma string de aviso e used em tooltips de SDK e
Automacao para mantener la paridade con los criterios de dicha hoja de ruta. Torii ágora
clusters dev/test y regresiones de diagnóstico. Continuar espelhando
`torii_address_domain_total{domain_kind}` sin Grafana
(`dashboards/grafana/address_ingest.json`) para que o paquete de evidencia ADDR-7
pruebe que `domain_kind="local12"` permanece en cero en janela requerida de 30
 Los días antes de una mainnet desactivan selecciones alternativas. O pacote Alertmanager
(`dashboards/alerts/address_ingest_rules.yml`) adiciona tres barandillas:- `AddressLocal8Resurgence` página siempre que un contexto reporta un incremento
  Local-8 nuevo. Pare rollouts de modo estrito, localice a superficie SDK
  responsavel no Dashboard e, se necesita, define temporalmente
  padrao (`true`).
- `AddressLocal12Collision` dispara cuando dos etiquetas Local-12 fazem hash para
  o mesmo resumen. Pausar promociones de manifiesto, ejecutar el kit de herramientas Local -> Global
  para auditar o mapeamento de digests e coordene com agobernanca Nexus antes
  de remitir una entrada de registro o reactivar implementaciones posteriores.
- `AddressInvalidRatioSlo` avisa quando a proporcao invalida em toda a frota
  (excluidos los rechazos Local-8/modo estricto) excede el SLO de 0.1% por dez minutos.
  Utilice `torii_address_invalid_total` para localizar o contexto/razao responsavel
  Se coordina con el SDK propietario del equipo antes de reactivar el modo estrito.

### Trecho de nota de liberación (carteira e explorador)

Incluye las siguientes viñetas en las notas de lanzamiento de carteira/explorador ao enviar o
transición:

> **Enderecos:** Adicionado o helper `iroha tools address normalize`
> e conectado no CI (`ci/check_address_normalize.sh`) para que tuberías de
> carteira/explorador possam convertidor selectores Local alternativas para formas
> canónicas IH58/comprimidas antes de Local-8/Local-12 serem bloqueadas na
> red principal. Atualize quaisquer exports personalizados para rodar o comando e
> anexar una lista normalizada al paquete de evidencia de liberación.