---
lang: es
direction: ltr
source: docs/source/finance/settlement_iso_mapping.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d1f1005d6a273ab732a7c7a7adca349c17569fe2e2755b8daccf2186724044f8
source_last_modified: "2026-01-22T15:57:09.830555+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Asentamiento ↔ Mapeo de Campo ISO 20022

Esta nota captura el mapeo canónico entre las instrucciones de liquidación Iroha.
(`DvpIsi`, `PvpIsi`, flujos de garantía repo) y los mensajes ISO 20022 ejercidos
por el puente. Refleja el andamiaje de mensajes implementado en
`crates/ivm/src/iso20022.rs` y sirve como referencia a la hora de producir o
validando las cargas útiles Norito.

### Política de Datos de Referencia (Identificadores y Validación)

Esta política incluye las preferencias de identificador, las reglas de validación y los datos de referencia.
obligaciones que el puente Norito ↔ ISO 20022 debe hacer cumplir antes de emitir mensajes.

**Puntos de anclaje dentro del mensaje ISO:**
- **Identificadores de instrumentos** → `delivery_leg.asset_definition_id` ↔ `SctiesLeg/FinInstrmId`
  (o el campo de instrumento equivalente).
- **Partes/agentes** → `DlvrgSttlmPties/Pty` y `RcvgSttlmPties/Pty` para `sese.*`,
  o las estructuras del agente en `pacs.009`.
- **Cuentas** → `…/Acct` elementos para custodia/cuentas de efectivo; reflejar el libro mayor
  `AccountId` en `SupplementaryData`.
- **Identificadores propietarios** → `…/OthrId` con `Tp/Prtry` y reflejado en
  `SupplementaryData`. Nunca reemplace los identificadores regulados por identificadores propietarios.

#### Preferencia de identificador por familia de mensajes

##### `sese.023` / `.024` / `.025` (liquidación de valores)

- **Instrumento (`FinInstrmId`)**
  - Preferido: **ISIN** bajo `…/ISIN`. Es el identificador canónico para CSD/T2S.[^anna]
  - Opciones alternativas:
    - **CUSIP** u otro NSIN bajo `…/OthrId/Id` con `Tp/Cd` configurado desde el ISO externo
      lista de códigos (por ejemplo, `CUSP`); incluya el emisor en `Issr` cuando sea obligatorio.[^iso_mdr]
    - **Norito ID de activo** como propietario: `…/OthrId/Id`, `Tp/Prtry="NORITO_ASSET_ID"` y
      registre el mismo valor en `SupplementaryData`.
  - Descriptores opcionales: **CFI** (`ClssfctnTp`) y **FISN** donde se admiten para facilitar
    reconciliación.[^iso_cfi][^iso_fisn]
- **Partes (`DlvrgSttlmPties`, `RcvgSttlmPties`)**
  - Preferido: **BIC** (`AnyBIC/BICFI`, ISO 9362).[^swift_bic]
  - Respaldo: **LEI** donde la versión del mensaje expone un campo LEI dedicado; si
    ausente, lleve identificaciones de propiedad con etiquetas `Prtry` claras e incluya BIC en los metadatos.[^iso_cr]
- **Lugar de liquidación/sede** → **MIC** para la sede y **BIC** para el CSD.[^iso_mic]

##### `colr.010` / `.011` / `.012` y `colr.007` (gestión de garantías)

- Siga las mismas reglas del instrumento que `sese.*` (se prefiere ISIN).
- Las partes utilizan **BIC** de forma predeterminada; **LEI** es aceptable donde el esquema lo expone.[^swift_bic]
- Los montos en efectivo deben utilizar códigos de moneda **ISO 4217** con unidades menores correctas.[^iso_4217]

##### `pacs.009` / `camt.054` (financiación y declaraciones PvP)- **Agentes (`InstgAgt`, `InstdAgt`, agentes deudores/acreedores)** → **BIC** con opcional
  LEI donde esté permitido.[^swift_bic]
- **Cuentas**
  - Interbancario: identificar por **BIC** y referencias internas de cuentas.
  - Extractos de cara al cliente (`camt.054`): incluya **IBAN** cuando esté presente y valídelo
    (longitud, reglas del país, suma de comprobación mod-97).[^swift_iban]
- **Moneda** → **ISO 4217** Código de 3 letras, respete el redondeo de unidades menores.[^iso_4217]
- **Ingestión de Torii** → Enviar tramos de financiación PvP a través de `POST /v2/iso20022/pacs009`; el puente
  requiere `Purp=SECU` y ahora aplica cruces peatonales BIC cuando se configuran datos de referencia.

#### Reglas de validación (se aplican antes de la emisión)

| Identificador | Regla de validación | Notas |
|------------|-----------------|-------|
| **ISIN** | Regex `^[A-Z]{2}[A-Z0-9]{9}[0-9]$` y dígito de control Luhn (mod-10) según ISO 6166 Anexo C | Rechazar antes de la emisión del puente; Prefiero el enriquecimiento ascendente.[^anna_luhn] |
| **CUSIP** | Regex `^[A-Z0-9]{9}$` y ​​módulo-10 con 2 ponderaciones (asignación de caracteres a dígitos) | Sólo cuando ISIN no esté disponible; mapa a través del paso de peatones ANNA/CUSIP una vez obtenido.[^cusip] |
| **LEI** | Regex `^[A-Z0-9]{18}[0-9]{2}$` y dígito de control mod-97 (ISO 17442) | Valide con los archivos delta diarios de la GLEIF antes de la aceptación.[^gleif] |
| **BIC** | Expresión regular `^[A-Z]{4}[A-Z]{2}[A-Z0-9]{2}([A-Z0-9]{3})?$` | Código de sucursal opcional (últimos tres caracteres). Confirmar el estado activo en archivos RA.[^swift_bic] |
| **MIC** | Mantener desde el archivo RA ISO 10383; asegúrese de que los lugares estén activos (sin indicador de terminación `!`) | Marcar los MIC desmantelados antes de su emisión.[^iso_mic] |
| **IBAN** | Longitud específica del país, alfanumérica en mayúsculas, mod-97 = 1 | Utilice el registro mantenido por SWIFT; rechazar IBAN estructuralmente no válidos.[^swift_iban] |
| **Cuenta propietaria/ID de parte** | `Max35Text` (UTF-8, ≤35 caracteres) con espacios en blanco recortados | Se aplica a los campos `GenericAccountIdentification1.Id` e `PartyIdentification135.Othr/Id`. Rechace las entradas que superen los 35 caracteres para que las cargas útiles del puente se ajusten a los esquemas ISO. |
| **Identificadores de cuenta proxy** | `Max2048Text` no vacío en `…/Prxy/Id` con códigos de tipo opcionales en `…/Prxy/Tp/{Cd,Prtry}` | Almacenado junto al IBAN principal; la validación aún requiere IBAN y acepta identificadores de proxy (con códigos de tipo opcionales) para reflejar los rieles PvP. |
| **CFI** | Código de seis caracteres, letras mayúsculas según la taxonomía ISO 10962 | Enriquecimiento opcional; asegúrese de que los caracteres coincidan con la clase del instrumento.[^iso_cfi] |
| **FISN** | Hasta 35 caracteres, alfanuméricos en mayúsculas más puntuación limitada | Opcional; truncar/normalizar según la guía ISO 18774.[^iso_fisn] |
| **Moneda** | Código ISO 4217 de 3 letras, escala determinada por unidades menores | Los montos deben redondearse a los decimales permitidos; aplicar en el lado Norito.[^iso_4217] |

#### Obligaciones de paso de peatones y mantenimiento de datos- Mantener los cruces peatonales **ISIN ↔ Norito ID de activo** y **CUSIP ↔ ISIN**. Actualizar todas las noches desde
  Los feeds de ANNA/DSB y la versión controlan las instantáneas utilizadas por CI.[^anna_crosswalk]
- Actualizar las asignaciones **BIC ↔ LEI** de los archivos de relaciones públicas de la GLEIF para que el puente pueda
  emitir ambos cuando sea necesario.[^bic_lei]
- Almacene **definiciones de MIC** junto con los metadatos del puente para que la validación del lugar sea
  determinista incluso cuando los archivos RA cambian al mediodía.[^iso_mic]
- Registrar la procedencia de los datos (marca de tiempo + fuente) en metadatos del puente para auditoría. persistir el
  identificador de instantánea junto con las instrucciones emitidas.
- Configure `iso_bridge.reference_data.cache_dir` para conservar una copia de cada conjunto de datos cargado
  junto con metadatos de procedencia (versión, fuente, marca de tiempo, suma de verificación). Esto permite a los auditores
  y operadores para diferenciar las fuentes históricas incluso después de que las instantáneas ascendentes roten.
- `iroha_core::iso_bridge::reference_data` ingiere instantáneas ISO de cruces peatonales utilizando
  el bloque de configuración `iso_bridge.reference_data` (rutas + intervalo de actualización). medidores
  `iso_reference_status`, `iso_reference_age_seconds`, `iso_reference_records` y
  `iso_reference_refresh_interval_secs` expone el estado del tiempo de ejecución para generar alertas. El Torii
  El puente rechaza los envíos `pacs.008` cuyos BIC de agente están ausentes del configurado
  cruce de peatones, lo que muestra errores deterministas `InvalidIdentifier` cuando una contraparte está
  desconocido.【crates/iroha_torii/src/iso20022_bridge.rs#L1078】
- Los enlaces IBAN e ISO 4217 se aplican en la misma capa: pacs.008/pacs.009 fluye ahora
  emite errores `InvalidIdentifier` cuando los IBAN de deudor/acreedor carecen de alias configurados o cuando
  falta la moneda de liquidación en `currency_assets`, lo que evita que el puente tenga formato incorrecto
  instrucciones para llegar al libro mayor. La validación del IBAN también se aplica en función del país.
  longitudes y dígitos de verificación numéricos antes de que la norma ISO 7064 mod-97 pase de manera estructuralmente inválida
  los valores se rechazan antes de tiempo.
- Los ayudantes de liquidación CLI heredan las mismas barandillas: pasar
  `--iso-reference-crosswalk <path>` junto con `--delivery-instrument-id` para tener el DvP
  Obtenga una vista previa de las ID de los instrumentos de validación antes de emitir la instantánea XML `sese.023`. 【crates/iroha_cli/src/main.rs#L3752】
- Pelusa `cargo xtask iso-bridge-lint` (y el contenedor CI `ci/check_iso_reference_data.sh`)
  instantáneas y accesorios del cruce de peatones. El comando acepta `--isin`, `--bic-lei`, `--mic` y
  `--fixtures` marca y recurre a los conjuntos de datos de muestra en `fixtures/iso_bridge/` cuando se ejecuta
  sin argumentos.【xtask/src/main.rs#L146】【ci/check_iso_reference_data.sh#L1】
- El asistente IVM ahora ingiere sobres XML ISO 20022 reales (head.001 + `DataPDU` + `Document`)
  y valida el encabezado de la aplicación empresarial a través del esquema `head.001` para que `BizMsgIdr`,
  Los agentes `MsgDefIdr`, `CreDt` y BIC/ClrSysMmbId se conservan de forma determinista; XMLDSig/XAdES
  Los bloques permanecen omitidos intencionalmente. Las pruebas de regresión consumen las muestras y los nuevosAccesorio del sobre del encabezado para proteger las asignaciones.

#### Consideraciones regulatorias y de estructura de mercado

- **Liquidación T+1**: los mercados de valores de EE. UU. y Canadá pasaron a T+1 en 2024; ajustar Norito
  programación y alertas de SLA en consecuencia.[^sec_t1][^csa_t1]
- **sanciones CSDR**: las reglas disciplinarias de los acuerdos imponen sanciones en efectivo; asegurar Norito
  Los metadatos capturan referencias de penalización para la conciliación.[^csdr]
- **Pilotos de liquidación el mismo día**: el regulador de la India está implementando gradualmente la liquidación T0/T+0; mantener
  Los calendarios del puente se actualizan a medida que los pilotos se expanden.[^india_t0]
- **Compras/retenciones de garantía**: supervise las actualizaciones de la ESMA sobre los plazos de compra y las retenciones opcionales
  por lo que la entrega condicional (`HldInd`) se alinea con la guía más reciente.[^csdr]

[^anna]: ANNA ISIN Guidelines, December 2023. https://anna-web.org/wp-content/uploads/2024/01/ISIN-Guidelines-Version-22-Dec-2023.pdf
[^iso_mdr]: ISO 20022 external code list (CUSIP `CUSP`) and MDR Part 2. https://www.iso20022.org/milestone/22048/download
[^iso_cfi]: ISO 10962 (CFI) taxonomy. https://www.iso.org/standard/81140.html
[^iso_fisn]: ISO 18774 (FISN) format guidance. https://www.iso.org/standard/66153.html
[^swift_bic]: SWIFT business identifier code (ISO 9362) guidance. https://www.swift.com/standards/data-standards/bic-business-identifier-code
[^iso_cr]: ISO 20022 change request introducing LEI options for party identification. https://www.iso20022.org/milestone/16116/download
[^iso_mic]: ISO 10383 Market Identifier Code maintenance agency. https://www.iso20022.org/market-identifier-codes
[^iso_4217]: ISO 4217 currency and minor-units table (SIX). https://www.six-group.com/en/products-services/financial-information/market-reference-data/data-standards.html
[^swift_iban]: IBAN registry and validation rules. https://www.swift.com/swift-resource/22851/download
[^anna_luhn]: ISIN checksum algorithm (Annex C). https://www.anna-dsb.com/isin/
[^cusip]: CUSIP format and checksum rules. https://www.iso20022.org/milestone/22048/download
[^gleif]: GLEIF LEI structure and validation details. https://www.gleif.org/en/organizational-identity/introducing-the-legal-entity-identifier-lei/iso-17442-the-lei-code-structure
[^anna_crosswalk]: ISIN cross-reference (ANNA DSB) feeds for derivatives and debt instruments. https://www.anna-dsb.com/isin/
[^bic_lei]: GLEIF BIC-to-LEI relationship files. https://www.gleif.org/en/lei-data/lei-mapping/download-bic-to-lei-relationship-files
[^sec_t1]: SEC release on US T+1 transition (2023). https://www.sec.gov/newsroom/press-releases/2023-29
[^csa_t1]: CSA amendments for Canadian institutional trade matching (T+1). https://www.osc.ca/en/securities-law/instruments-rules-policies/2/24-101/csa-notice-amendments-national-instrument-24-101-institutional-trade-matching-and-settlement-and
[^csdr]: ESMA CSDR settlement discipline / penalty mechanism updates. https://www.esma.europa.eu/sites/default/files/2024-11/ESMA74-2119945925-2059_Final_Report_on_Technical_Advice_on_CSDR_Penalty_Mechanism.pdf
[^india_t0]: SEBI circular on same-day settlement pilot. https://www.reuters.com/sustainability/boards-policy-regulation/india-markets-regulator-extends-deadline-same-day-settlement-plan-brokers-2025-04-29/

### Entrega contra pago → `sese.023`| Campo DvP | Ruta ISO 20022 | Notas |
|--------------------------------------------------------|----------------------------------------|-------|
| `settlement_id` | `TxId` | Identificador de ciclo de vida estable |
| `delivery_leg.asset_definition_id` (seguridad) | `SctiesLeg/FinInstrmId` | Identificador canónico (ISIN, CUSIP,…) |
| `delivery_leg.quantity` | `SctiesLeg/Qty` | Cadena decimal; honra la precisión de los activos |
| `payment_leg.asset_definition_id` (moneda) | `CashLeg/Ccy` | Código de moneda ISO |
| `payment_leg.quantity` | `CashLeg/Amt` | Cadena decimal; redondeado según especificación numérica |
| `delivery_leg.from` (vendedor/entregador) | `DlvrgSttlmPties/Pty/Bic` | BIC del participante que realizó la entrega *(la identificación canónica de la cuenta se exporta actualmente en metadatos)* |
| `delivery_leg.from` identificador de cuenta | `DlvrgSttlmPties/Acct` | Forma libre; Los metadatos Norito llevan el ID de cuenta exacto |
| `delivery_leg.to` (comprador/parte receptora) | `RcvgSttlmPties/Pty/Bic` | BIC del participante receptor |
| Identificador de cuenta `delivery_leg.to` | `RcvgSttlmPties/Acct` | Forma libre; coincide con el ID de la cuenta receptora |
| `plan.order` | `Plan/ExecutionOrder` | Enum: `DELIVERY_THEN_PAYMENT` o `PAYMENT_THEN_DELIVERY` |
| `plan.atomicity` | `Plan/Atomicity` | Enumeración: `ALL_OR_NOTHING`, `COMMIT_FIRST_LEG`, `COMMIT_SECOND_LEG` |
| **Propósito del mensaje** | `SttlmTpAndAddtlParams/SctiesMvmntTp` | `DELI` (entregar) o `RECE` (recibir); refleja qué etapa ejecuta la parte que la presenta. |
|                                                        | `SttlmTpAndAddtlParams/Pmt` | `APMT` (contra pago) o `FREE` (libre de pago). |
| `delivery_leg.metadata`, `payment_leg.metadata` | `SctiesLeg/Metadata`, `CashLeg/Metadata` | Opcional Norito JSON codificado como UTF‑8 |

> **Calificadores de liquidación**: el puente refleja la práctica del mercado al copiar códigos de condición de liquidación (`SttlmTxCond`), indicadores de liquidación parcial (`PrtlSttlmInd`) y otros calificadores opcionales de los metadatos Norito en `sese.023/025` cuando estén presentes. Aplique las enumeraciones publicadas en las listas de códigos externos ISO para que el CSD de destino reconozca los valores.

### Financiamiento de pago contra pago → `pacs.009`

Los tramos de efectivo por efectivo que financian una instrucción PvP se emiten como crédito de FI a FI.
transferencias. El puente anota estos pagos para que los sistemas posteriores los reconozcan.
financian una liquidación de valores.| Campo de financiación PvP | Ruta ISO 20022 | Notas |
|------------------------------------------------|-----------------------------------------------------|-------|
| `primary_leg.quantity` / {monto, moneda} | `IntrBkSttlmAmt` + `IntrBkSttlmCcy` | Importe/moneda debitado del iniciador. |
| Identificadores de agentes de contraparte | `InstgAgt`, `InstdAgt` | BIC/LEI de agentes emisores y receptores. |
| Objeto de la liquidación | `CdtTrfTxInf/PmtTpInf/CtgyPurp/Cd` | Establecido en `SECU` para financiación PvP relacionada con valores. |
| Metadatos Norito (ID de cuenta, datos FX) | `CdtTrfTxInf/SplmtryData` | Lleva ID de cuenta completo, marcas de tiempo FX y sugerencias del plan de ejecución. |
| Identificador de instrucción/vinculación del ciclo de vida | `CdtTrfTxInf/PmtId/InstrId`, `CdtTrfTxInf/RmtInf` | Coincide con Norito `settlement_id` para que el lado de efectivo se concilie con el lado de valores. |

El puente ISO del SDK de JavaScript se alinea con este requisito al establecer de forma predeterminada el
Propósito de categoría `pacs.009` a `SECU`; las personas que llaman pueden anularlo con otro
Código ISO válido al emitir transferencias de crédito no relacionadas con valores, pero no válido
Los valores se rechazan por adelantado.

Si una infraestructura requiere una confirmación explícita de valores, el puente
continúa emitiendo `sese.025`, pero esa confirmación refleja la pierna de valores
estado (por ejemplo, `ConfSts = ACCP`) en lugar del "propósito" de PvP.

### Confirmación de pago contra pago → `sese.025`

| Campo PvP | Ruta ISO 20022 | Notas |
|-----------------------------------------------|---------------------|-------|
| `settlement_id` | `TxId` | Identificador de ciclo de vida estable |
| `primary_leg.asset_definition_id` | `SttlmCcy` | Código de moneda para el tramo primario |
| `primary_leg.quantity` | `SttlmAmt` | Importe entregado por iniciador |
| `counter_leg.asset_definition_id` | `AddtlInf` (carga útil JSON) | Código de moneda contador integrado en información complementaria |
| `counter_leg.quantity` | `SttlmQty` | Importe del contador |
| `plan.order` | `Plan/ExecutionOrder` | Misma enumeración configurada como DvP |
| `plan.atomicity` | `Plan/Atomicity` | Misma enumeración configurada como DvP |
| Estado `plan.atomicity` (`ConfSts`) | `ConfSts` | `ACCP` cuando coincide; puente emite códigos de falla al ser rechazado |
| Identificadores de contraparte | `AddtlInf` JSON | El puente actual serializa tuplas AccountId/BIC completas en metadatos |

Ejemplo (vista previa de CLI ISO con vínculos, retención y MIC de mercado):

```sh
iroha app settlement dvp \
  --settlement-id DVP-FIXTURE-1 \
  --delivery-asset security#equities \
  --delivery-quantity 500 \
  --delivery-from i105... \
  --delivery-to i105... \
  --payment-asset usd#fi \
  --payment-quantity 1050000 \
  --payment-from i105... \
  --payment-to i105... \
  --delivery-instrument-id US0378331005 \
  --place-of-settlement-mic XNAS \
  --partial-indicator npar \
  --hold-indicator \
  --settlement-condition NOMC \
  --linkage WITH:PACS009-CLS \
  --linkage BEFO:SUBST-PAIR-B \
  --iso-xml-out sese023_preview.xml
```

### Sustitución de garantía de repositorio → `colr.007`| Campo/contexto de repositorio | Ruta ISO 20022 | Notas |
|-------------------------------------------------|-----------------------------------|-------|
| `agreement_id` (`RepoIsi` / `ReverseRepoIsi`) | `OblgtnId` | Identificador de contrato de repositorio |
| Identificador Tx de sustitución de garantía | `TxId` | Generado por sustitución |
| Cantidad de garantía original | `Substitution/OriginalAmt` | Partidos prometidos como garantía antes de la sustitución |
| Moneda de garantía original | `Substitution/OriginalCcy` | Código de moneda |
| Cantidad de garantía sustitutiva | `Substitution/SubstituteAmt` | Importe de reposición |
| Moneda de garantía sustitutiva | `Substitution/SubstituteCcy` | Código de moneda |
| Fecha de entrada en vigor (cronograma de márgenes de gobernanza) | `Substitution/EffectiveDt` | Fecha ISO (AAAA-MM-DD) |
| Clasificación de corte de pelo | `Substitution/Type` | Actualmente `FULL` o `PARTIAL` según la política de gobernanza |
| Razón de gobernanza / nota de recorte | `Substitution/ReasonCd` | Opcional, conlleva fundamentos de gobernanza |
| Tamaño del corte de pelo | `Substitution/Haircut` | Numérico; mapea el corte de pelo aplicado durante la sustitución |
| ID de instrumentos originales/sustitutos | `Substitution/OriginalFinInstrmId`, `Substitution/SubstituteFinInstrmId` | ISIN/CUSIP opcional para cada tramo |

### Financiamiento y declaraciones

| Iroha contexto | Mensaje ISO 20022 | Ubicación del mapa |
|----------------------------------|-------------------|------------------|
| Encendido/desconexión del tramo de efectivo del repositorio | `pacs.009` | `IntrBkSttlmAmt`, `IntrBkSttlmCcy`, `IntrBkSttlmDt`, `InstgAgt`, `InstdAgt` poblados a partir de tramos DvP/PvP |
| Declaraciones posteriores al acuerdo | `camt.054` | Movimientos de tramo de pago registrados bajo `Ntfctn/Ntry[*]`; puente inyecta metadatos del libro mayor/cuenta en `SplmtryData` |

### Notas de uso* Todos los importes se serializan utilizando los ayudantes numéricos Norito (`NumericSpec`)
  para garantizar la conformidad de escala entre las definiciones de activos.
* Los valores `TxId` son `Max35Text`: aplica una longitud UTF‑8 ≤35 caracteres antes
  exportar a mensajes ISO 20022.
* Los BIC deben tener 8 u 11 caracteres alfanuméricos en mayúsculas (ISO9362); rechazar
  Metadatos Norito que no pasan esta verificación antes de emitir pagos o liquidaciones
  confirmaciones.
* Los identificadores de cuenta (AccountId / ChainId) se exportan a archivos complementarios.
  metadatos para que los participantes receptores puedan conciliarlos con sus libros de contabilidad locales.
* `SupplementaryData` debe ser JSON canónico (UTF-8, claves ordenadas, JSON nativo)
  escapar). Los asistentes del SDK aplican esto para que las firmas, los hashes de telemetría y la ISO
  Los archivos de carga útil siguen siendo deterministas en todas las reconstrucciones.
* Los montos de moneda siguen los dígitos de fracción ISO4217 (por ejemplo, JPY tiene 0
  decimales, USD tiene 2); el puente sujeta la precisión numérica Norito en consecuencia.
* Los ayudantes de liquidación CLI (`iroha app settlement ... --atomicity ...`) ahora emiten
  Instrucciones Norito cuyos planes de ejecución se asignan 1:1 a `Plan/ExecutionOrder` y
  `Plan/Atomicity` arriba.
* El asistente ISO (`ivm::iso20022`) valida los campos enumerados anteriormente y rechaza
  mensajes en los que los tramos DvP/PvP violan las especificaciones numéricas o la reciprocidad de la contraparte.

### Ayudantes del generador de SDK

- El SDK de JavaScript ahora expone `buildPacs008Message` /
  `buildPacs009Message` (ver `javascript/iroha_js/src/isoBridge.js`) para que el cliente
  La automatización puede convertir metadatos de liquidación estructurados (BIC/LEI, IBAN,
  códigos de propósito, campos suplementarios Norito) en pacs XML deterministas
  sin volver a implementar las reglas de mapeo de esta guía.
- Ambos asistentes requieren un `creationDateTime` explícito (ISO‑8601 con zona horaria)
  por lo que los operadores deben incluir una marca de tiempo determinista en su flujo de trabajo.
  de dejar que el SDK utilice de forma predeterminada la hora del reloj de pared.
- `recipes/iso_bridge_builder.mjs` demuestra cómo conectar esos ayudantes a
  una CLI que fusiona variables de entorno o archivos de configuración JSON, imprime el
  XML generado y, opcionalmente, lo envía a Torii (`ISO_SUBMIT=1`), reutilizando
  la misma cadencia de espera que la receta del puente ISO.


### Referencias

- Ejemplos de liquidación LuxCSD/Clearstream ISO 20022 que muestran `SttlmTpAndAddtlParams/SctiesMvmntTp` (`DELI`/`RECE`) e `Pmt` (`APMT`/`FREE`).[1](https://www.luxcsd.com/resource/blob/3434074/6f8add4708407a4701055be4dd04846b/c23005-eis-examples-cbf-data.pdf)
- Especificaciones de Clearstream DCP que cubren los calificadores de liquidación (`SttlmTxCond`, `PrtlSttlmInd`).[2](https://www.clearstream.com/clearstream-en/res-library/market-coverage/instruction-specifications-swift-iso-20022-dcp-mode-ceu-spain-2357008)
- Guía de SWIFT PMPG que recomienda `pacs.009` con `CtgyPurp/Cd = SECU` para financiación PvP relacionada con valores.[3](https://www.swift.com/swift-resource/251897/download)
- Informes de definición de mensajes ISO 20022 para restricciones de longitud de identificadores (BIC, Max35Text).[4](https://www.iso20022.org/sites/default/files/2020-12/ISO20022_MDRPart2_ChangeOrVerifyAccountIdentification_2020_2021_v1_ForSEGReview.pdf)
- Guía de ANNA DSB sobre el formato ISIN y las reglas de suma de comprobación.[5](https://www.anna-dsb.com/isin/)

### Consejos de uso- Pegue siempre el fragmento Norito o el comando CLI correspondiente para que LLM pueda inspeccionar
  nombres de campo exactos y escalas numéricas.
- Solicitar citas (`provide clause references`) para mantener un rastro en papel de
  cumplimiento y revisión del auditor.
- Capture el resumen de la respuesta en `docs/source/finance/settlement_iso_mapping.md`
  (o apéndices vinculados) para que los futuros ingenieros no necesiten repetir la consulta.

## Guías de pedido de eventos (ISO 20022 ↔ Puente Norito)

### Escenario A: sustitución de garantías (recompra/promesa)

**Participantes:** donante/receptor de garantía (y/o agentes), custodio(s), CSD/T2S  
**Tiempo:** por cortes de mercado y ciclos día/noche T2S; Orqueste las dos etapas para que se completen dentro de la misma ventana de liquidación.

#### Coreografía de mensajes
1. `colr.010` Solicitud de sustitución de garantía → dador/receptor de garantía o agente.  
2. `colr.011` Respuesta de sustitución de garantía → aceptar/rechazar (motivo de rechazo opcional).  
3. `colr.012` Confirmación de sustitución de garantía → confirma el acuerdo de sustitución.  
4. Instrucciones `sese.023` (dos patas):  
   - Devolver garantía original (`SctiesMvmntTp=DELI`, `Pmt=FREE`, `SctiesTxTp=COLO`).  
   - Entregar garantía sustituta (`SctiesMvmntTp=RECE`, `Pmt=FREE`, `SctiesTxTp=COLI`).  
   Vincula el par (ver más abajo).  
5. Avisos de estado `sese.024` (aceptado, coincidente, pendiente, fallido, rechazado).  
6. Confirmaciones `sese.025` una vez reservada.  
7. Delta de efectivo opcional (tarifas/recorte) → `pacs.009` Transferencia de crédito de FI a FI con `CtgyPurp/Cd = SECU`; estado a través de `pacs.002`, regresa a través de `pacs.004`.

#### Agradecimientos/estados requeridos
- Nivel de transporte: las puertas de enlace pueden emitir `admi.007` o rechazarlos antes del procesamiento comercial.  
- Ciclo de vida de la liquidación: `sese.024` (estados de procesamiento + códigos de motivo), `sese.025` (final).  
- Lado de efectivo: `pacs.002` (`PDNG`, `ACSC`, `RJCT` etc.), `pacs.004` para devoluciones.

#### Condicionalidad / campos de desenredado
- `SctiesSttlmTxInstr/Lnkgs` (`WITH`/`BEFO`/`AFTE`) para encadenar las dos instrucciones.  
- `SttlmParams/HldInd` se mantendrá hasta que se cumplan los criterios; liberación a través de `sese.030` (estado `sese.031`).  
- `SttlmParams/PrtlSttlmInd` para controlar la liquidación parcial (`NPAR`, `PART`, `PARC`, `PARQ`).  
- `SttlmParams/SttlmTxCond/Cd` para condiciones específicas del mercado (`NOMC`, etc.).  
- Reglas opcionales de entrega de valores condicionales (CoSD) de T2S cuando sean compatibles.

#### Referencias
- Gestión de garantías SWIFT MDR (`colr.010/011/012`).  
- Guías de uso de CSD/T2S (por ejemplo, DNB, ECB Insights) para vinculación y estados.  
- Práctica de liquidación SMPG, manuales de Clearstream DCP, talleres ASX ISO.

### Escenario B: Incumplimiento de la ventana FX (fallo en la financiación PvP)

**Participantes:** contrapartes y agentes de efectivo, custodio de valores, CSD/T2S  
**Tiempo:** Ventanas FX PvP (CLS/bilateral) y límites de CSD; mantener los tramos de valores en espera a la espera de la confirmación del efectivo.#### Coreografía de mensajes
1. `pacs.009` Transferencia de crédito de FI a FI por moneda con `CtgyPurp/Cd = SECU`; estado a través de `pacs.002`; recuperar/cancelar a través de `camt.056`/`camt.029`; si ya está liquidado, regresa `pacs.004`.  
2. Instrucciones DvP `sese.023` con `HldInd=true` para que el tramo de valores espere la confirmación de efectivo.  
3. Avisos del ciclo de vida `sese.024` (aceptados/coincidentes/pendientes).  
4. Si ambos tramos `pacs.009` llegan a `ACSC` antes de que expire la ventana → libere con `sese.030` → `sese.031` (estado de modificación) → `sese.025` (confirmación).  
5. Si se infringe la ventana FX → cancelar/retirar efectivo (`camt.056/029` o `pacs.004`) y cancelar valores (`sese.020` + `sese.027`, o reversión `sese.026` si ya está confirmado según la regla del mercado).

#### Agradecimientos/estados requeridos
- Efectivo: `pacs.002` (`PDNG`, `ACSC`, `RJCT`), `pacs.004` para devoluciones.  
- Valores: `sese.024` (motivos pendientes/fallidos como `NORE`, `ADEA`), `sese.025`.  
- Transporte: `admi.007` / gateway rechaza antes del procesamiento comercial.

#### Condicionalidad / campos de desenredado
- `SttlmParams/HldInd` + `sese.030` liberación/cancelación en caso de éxito/fracaso.  
- `Lnkgs` para vincular instrucciones de valores al tramo de efectivo.  
- Regla T2S CoSD si se utiliza entrega condicional.  
- `PrtlSttlmInd` para evitar parciales no deseados.  
- En `pacs.009`, `CtgyPurp/Cd = SECU` señala financiación relacionada con valores.

#### Referencias
- Orientación PMPG/CBPR+ para pagos en procesos de valores.  
- Prácticas de liquidación de SMPG, insights de T2S sobre vinculaciones/retenciones.  
- Manuales de Clearstream DCP, documentación ECMS para mensajes de mantenimiento.

### pacs.004 devolver notas de mapeo

- Los accesorios de devolución ahora normalizan `ChrgBr` (`DEBT`/`CRED`/`SHAR`/`SLEV`) y los motivos de devolución de propiedad expuestos como `TxInf[*]/RtrdRsn/Prtry`, por lo que los consumidores del puente pueden reproducir los códigos de operador y atribución de tarifas sin volver a analizar el Sobre XML.
- Los bloques de firma AppHdr dentro de los sobres `DataPDU` permanecen ignorados durante la ingesta; las auditorías deben basarse en la procedencia del canal en lugar de en campos XMLDSIG integrados.

### Lista de verificación operativa para el puente
- Hacer cumplir la coreografía anterior (colateral: `colr.010/011/012 → sese.023/024/025`; incumplimiento de FX: `pacs.009 (+pacs.002) → sese.023 held → release/cancel`).  
- Trate los estados `sese.024`/`sese.025` y los resultados `pacs.002` como señales de activación; `ACSC` activa la liberación, `RJCT` fuerza el desenrollado.  
- Codifique la entrega condicional a través de `HldInd`, `Lnkgs`, `PrtlSttlmInd`, `SttlmTxCond` y reglas CoSD opcionales.  
- Utilice `SupplementaryData` para correlacionar ID externos (por ejemplo, UETR para `pacs.009`) cuando sea necesario.  
- Parametrizar el tiempo de retención/desconexión según el calendario/límites del mercado; emita `sese.030`/`camt.056` antes de los plazos de cancelación, recurra a devoluciones cuando sea necesario.

### Muestra de cargas útiles ISO 20022 (anotadas)

#### Par de sustitución de garantía (`sese.023`) con enlace de instrucciones

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.023.001.11">
  <sese:SctiesSttlmTxInstr>
    <sese:TxId>SUBST-2025-04-001-A</sese:TxId>
    <sese:SttlmTpAndAddtlParams>
      <sese:SctiesMvmntTp>DELI</sese:SctiesMvmntTp>
      <sese:Pmt>FREE</sese:Pmt>
    </sese:SttlmTpAndAddtlParams>
    <sese:SttlmParams>
      <sese:HldInd>true</sese:HldInd>
      <sese:PrtlSttlmInd>NPAR</sese:PrtlSttlmInd>
      <sese:SttlmTxCond>
        <sese:Cd>NOMC</sese:Cd>
      </sese:SttlmTxCond>
    </sese:SttlmParams>
    <sese:Lnkgs>
      <sese:Lnkg>
        <sese:Tp>
          <sese:Cd>WITH</sese:Cd>
        </sese:Tp>
        <sese:Ref>
          <sese:Prtry>SUBST-2025-04-001-B</sese:Prtry>
        </sese:Ref>
      </sese:Lnkg>
    </sese:Lnkgs>
    <!-- Original collateral FoP back to giver -->
    <sese:FctvSttlmDt>2025-04-03</sese:FctvSttlmDt>
    <sese:SctiesMvmntDtls>
      <sese:SctiesId>
        <sese:ISIN>XS1234567890</sese:ISIN>
      </sese:SctiesId>
      <sese:Qty>
        <sese:QtyChc>
          <sese:Unit>1000</sese:Unit>
        </sese:QtyChc>
      </sese:Qty>
    </sese:SctiesMvmntDtls>
  </sese:SctiesSttlmTxInstr>
</sese:Document>
```Envíe la instrucción vinculada `SUBST-2025-04-001-B` (recepción FoP de garantía sustituta) con `SctiesMvmntTp=RECE`, `Pmt=FREE` y el vínculo `WITH` apuntando a `SUBST-2025-04-001-A`. Suelte ambas patas con un `sese.030` coincidente una vez que se apruebe la sustitución.

#### Tramo de valores en espera pendiente de confirmación de FX (`sese.023` + `sese.030`)

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.023.001.11">
  <sese:SctiesSttlmTxInstr>
    <sese:TxId>DVP-2025-05-CLS01</sese:TxId>
    <sese:SttlmTpAndAddtlParams>
      <sese:SctiesMvmntTp>DELI</sese:SctiesMvmntTp>
      <sese:Pmt>APMT</sese:Pmt>
    </sese:SttlmTpAndAddtlParams>
    <sese:SttlmParams>
      <sese:HldInd>true</sese:HldInd>
      <sese:PrtlSttlmInd>NPAR</sese:PrtlSttlmInd>
    </sese:SttlmParams>
    <sese:Lnkgs>
      <sese:Lnkg>
        <sese:Tp>
          <sese:Cd>WITH</sese:Cd>
        </sese:Tp>
        <sese:Ref>
          <sese:Prtry>PACS009-USD-CLS01</sese:Prtry>
        </sese:Ref>
      </sese:Lnkg>
    </sese:Lnkgs>
    <!-- Remaining settlement details omitted for brevity -->
  </sese:SctiesSttlmTxInstr>
</sese:Document>
```

Suelte una vez que ambas patas `pacs.009` lleguen a `ACSC`:

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.030.001.04">
  <sese:SctiesSttlmCondModReq>
    <sese:ReqDtls>
      <sese:TxId>DVP-2025-05-CLS01</sese:TxId>
      <sese:ChngTp>
        <sese:Cd>RELE</sese:Cd>
      </sese:ChngTp>
    </sese:ReqDtls>
  </sese:SctiesSttlmCondModReq>
</sese:Document>
```

`sese.031` confirma la liberación de la retención, seguido de `sese.025` una vez que se registra el tramo de valores.

#### tramo de financiación PvP (`pacs.009` con finalidad de valores)

```xml
<pacs:Document xmlns:pacs="urn:iso:std:iso:20022:tech:xsd:pacs.009.001.08">
  <pacs:FinInstnCdtTrf>
    <pacs:GrpHdr>
      <pacs:MsgId>PACS009-USD-CLS01</pacs:MsgId>
      <pacs:IntrBkSttlmDt>2025-05-07</pacs:IntrBkSttlmDt>
    </pacs:GrpHdr>
    <pacs:CdtTrfTxInf>
      <pacs:PmtId>
        <pacs:InstrId>DVP-2025-05-CLS01-USD</pacs:InstrId>
        <pacs:EndToEndId>SETTLEMENT-CLS01</pacs:EndToEndId>
      </pacs:PmtId>
      <pacs:PmtTpInf>
        <pacs:CtgyPurp>
          <pacs:Cd>SECU</pacs:Cd>
        </pacs:CtgyPurp>
      </pacs:PmtTpInf>
      <pacs:IntrBkSttlmAmt Ccy="USD">5000000.00</pacs:IntrBkSttlmAmt>
      <pacs:InstgAgt>
        <pacs:FinInstnId>
          <pacs:BICFI>BANKUS33XXX</pacs:BICFI>
        </pacs:FinInstnId>
      </pacs:InstgAgt>
      <pacs:InstdAgt>
        <pacs:FinInstnId>
          <pacs:BICFI>BANKGB22XXX</pacs:BICFI>
        </pacs:FinInstnId>
      </pacs:InstdAgt>
      <pacs:SplmtryData>
        <pacs:Envlp>
          <nor:NoritoBridge xmlns:nor="urn:norito:settlement">
            <nor:SettlementId>DVP-2025-05-CLS01</nor:SettlementId>
            <nor:Atomicity>ALL_OR_NOTHING</nor:Atomicity>
          </nor:NoritoBridge>
        </pacs:Envlp>
      </pacs:SplmtryData>
    </pacs:CdtTrfTxInf>
  </pacs:FinInstnCdtTrf>
</pacs:Document>
```

`pacs.002` rastrea el estado del pago (`ACSC` = confirmado, `RJCT` = rechazado). Si se infringe la ventana, recupere a través de `camt.056`/`camt.029` o envíe `pacs.004` para devolver los fondos liquidados.