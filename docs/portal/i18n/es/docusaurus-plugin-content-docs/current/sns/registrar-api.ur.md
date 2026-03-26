---
lang: es
direction: ltr
source: docs/portal/docs/sns/registrar-api.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota مستند ماخذ
یہ صفحہ `docs/source/sns/registrar_api.md` کی عکاسی کرتا ہے اور اب پورٹل
کی کینونیکل کاپی ہے۔ سورس فائل ترجمہ PRs کے لئے برقرار رہتی ہے۔
:::

# API de registrador de SNS y ganchos (SN-2b)

**حالت:** 2026-03-24 Actualización - Nexus Core ریویو کے تحت  
**روڈمیپ لنک:** SN-2b "API de registrador y ganchos de gobernanza"  
**پیشگی شرائط:** اسکیمہ تعریفیں [`registry-schema.md`](./registry-schema.md) میں۔

Varios puntos finales Torii, servicios gRPC, DTO de solicitud/respuesta y gobernanza
artefactos کی وضاحت کرتا ہے جو Registrador del Servicio de nombres de Sora (SNS) چلانے کے لئے
درکار ہیں۔ یہ SDK, billeteras y automatización کے لئے مستند معاہدہ ہے جو SNS نام
رجسٹر، renovar یا administrar کرنا چاہتے ہیں۔

## 1. ٹرانسپورٹ اور توثیق| شرط | تفصیل |
|-----|-------|
| پروٹوکولز | REST `/v1/sns/*` کے تحت اور servicio gRPC `sns.v1.Registrar`۔ Nombre Norito-JSON (`application/json`) y Norito-RPC (`application/x-norito`) |
| Autenticación | Tokens `Authorization: Bearer` یا Certificados mTLS ہر administrador de sufijos کی طرف سے جاری۔ Puntos finales sensibles a la gobernanza (congelar/descongelar, asignaciones reservadas) کے لئے `scope=sns.admin` لازم ہے۔ |
| ریٹ حدود | Registradores `torii.preauth_scheme_limits` agrupa personas que llaman JSON کے ساتھ شیئر کرتے ہیں اور ہر sufijo کے لئے mayúsculas en ráfaga: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`۔ |
| ٹیلیمیٹری | Controladores de registrador Torii کے لئے `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` ظاہر کرتا ہے (`scheme="norito_rpc"` پر filter)؛ API بھی `sns_registrar_status_total{result, suffix_id}` بڑھاتی ہے۔ |

## 2. DTO خلاصہ

فیلڈز [`registry-schema.md`](./registry-schema.md) میں متعین estructuras canónicas کو consulte کرتی ہیں۔ Cargas útiles `NameSelectorV1` + `SuffixId` شامل کرتی ہیں تاکہ مبہم enrutamiento سے بچا جا سکے۔

```text
Struct RegisterNameRequestV1 {
    selector: NameSelectorV1,
    owner: AccountId,
    controllers: Vec<NameControllerV1>,
    term_years: u8,                     // 1..=max_term_years
    pricing_class_hint: Option<u8>,     // steward-advertised tier id
    payment: PaymentProofV1,
    governance: GovernanceHookV1,
    metadata: Metadata,
}

Struct RegisterNameResponseV1 {
    name_record: NameRecordV1,
    registry_event: RegistryEventV1,
    revenue_accrual: RevenueAccrualEventV1,
}

Struct PaymentProofV1 {
    asset_id: AssetId,
    gross_amount: TokenValue,
    net_amount: TokenValue,
    settlement_tx: Hash,
    payer: AccountId,
    signature: Signature,               // steward/treasury cosign
}

Struct GovernanceHookV1 {
    proposal_id: String,
    council_vote_hash: Hash,
    dao_vote_hash: Hash,
    steward_ack: Signature,
    guardian_clearance: Option<Signature>,
}

Struct RenewNameRequestV1 {
    selector: NameSelectorV1,
    term_years: u8,
    payment: PaymentProofV1,
}

Struct TransferNameRequestV1 {
    selector: NameSelectorV1,
    new_owner: AccountId,
    governance: GovernanceHookV1,
}

Struct UpdateControllersRequestV1 {
    selector: NameSelectorV1,
    controllers: Vec<NameControllerV1>,
}

Struct FreezeNameRequestV1 {
    selector: NameSelectorV1,
    reason: String,
    until: Timestamp,
    guardian_ticket: Signature,
}

Struct ReservedAssignmentRequestV1 {
    selector: NameSelectorV1,
    reserved_label: ReservedNameV1,
    governance: GovernanceHookV1,
}
```

## 3. Puntos finales REST| Punto final | طریقہ | Carga útil | تفصیل |
|----------|-------|---------|-------|
| `/v1/sns/names` | PUBLICAR | `RegisterNameRequestV1` | نام رجسٹر یا دوبارہ کھولنا۔ nivel de precios حل کرتا ہے، pruebas de pago/gobernanza کی توثیق کرتا ہے، los eventos de registro emiten کرتا ہے۔ |
| `/v1/sns/names/{namespace}/{literal}/renew` | PUBLICAR | `RenewNameRequestV1` | مدت بڑھاتا ہے۔ پالیسی سے ventanas de gracia/redención نافذ کرتا ہے۔ |
| `/v1/sns/names/{namespace}/{literal}/transfer` | PUBLICAR | `TransferNameRequestV1` | حکمرانی aprobaciones لگنے کے بعد propiedad منتقل کرتا ہے۔ |
| `/v1/sns/names/{namespace}/{literal}/controllers` | PONER | `UpdateControllersRequestV1` | controladores کا سیٹ بدلتا ہے؛ direcciones de cuenta firmadas کی توثیق کرتا ہے۔ |
| `/v1/sns/names/{namespace}/{literal}/freeze` | PUBLICAR | `FreezeNameRequestV1` | congelación de tutor/consejo۔ billete de guardián اور expediente de gobernanza کا حوالہ درکار۔ |
| `/v1/sns/names/{namespace}/{literal}/freeze` | BORRAR | `GovernanceHookV1` | remediación کے بعد descongelar؛ anulación del consejo ریکارڈ ہونے کو یقینی بناتا ہے۔ |
| `/v1/sns/reserved/{selector}` | PUBLICAR | `ReservedAssignmentRequestV1` | nombres reservados کی mayordomo/consejo کی طرف سے asignación۔ |
| `/v1/sns/policies/{suffix_id}` | OBTENER | -- | `SuffixPolicyV1` Memoria caché ہے (almacenable en caché) ۔ |
| `/v1/sns/names/{namespace}/{literal}` | OBTENER | -- | موجودہ `NameRecordV1` + موثر حالت (Active, Grace وغیرہ) واپس کرتا ہے۔ |

**Codificación del selector:** `{selector}` segmento de ruta i105, comprimido (`sora`) یا canónico hexadecimal ADDR-5 کے مطابق قبول کرتا ہے؛ Torii `NameSelectorV1` سے normalizar کرتا ہے۔**Modelo de error:** Puntos finales de actualización Norito JSON `code`, `message`, `details` y کرتے ہیں۔ Códigos میں `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing` شامل ہیں۔

### 3.1 CLI helpers (N0 registrador دستی ضرورت)

Administradores de beta cerrada اب CLI کے ذریعے registrador استعمال کر سکتے ہیں بغیر ہاتھ سے JSON بنانے کے:

```bash
iroha sns register \
  --label makoto \
  --suffix-id 1 \
  --term-years 2 \
  --payment-asset-id 61CtjvNd9T3THAR65GsMVHr82Bjc \
  --payment-gross 240 \
  --payment-settlement '"settlement-tx-hash"' \
  --payment-signature '"steward-signature"'
```

- `--owner` Configuración de la cuenta de configuración CLI Estas cuentas de controlador son `--controller` y دہرائيں (predeterminado `[owner]`).
- Banderas de pago en línea براہ راست `PaymentProofV1` سے map ہوتے ہیں؛ جب recibo estructurado ہو تو `--payment-json PATH` دیں۔ Metadatos (`--metadata-json`) اور ganchos de gobernanza (`--governance-json`) بھی اسی انداز میں ہیں۔

Ensayos de ayudantes de solo lectura کو مکمل کرتے ہیں:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Implementación کے لئے `crates/iroha_cli/src/commands/sns.rs` دیکھیں؛ comandos اس دستاویز میں بیان کردہ Norito DTO دوبارہ استعمال کرتے ہیں تاکہ Salida CLI Torii respuestas کے ساتھ byte por byte میل کھائے۔

Renovaciones de ayudantes adicionales, transferencias y acciones de tutor کو کور کرتے ہیں:

```bash
# Renew an expiring name
iroha sns renew \
  --selector makoto.sora \
  --term-years 1 \
  --payment-asset-id 61CtjvNd9T3THAR65GsMVHr82Bjc \
  --payment-gross 120 \
  --payment-settlement '"renewal-settlement"' \
  --payment-signature '"steward-signature"'

# Transfer ownership once governance approves
iroha sns transfer \
  --selector makoto.sora \
  --new-owner soraカタカナ... \
  --governance-json /path/to/hook.json

# Freeze/unfreeze flows
iroha sns freeze \
  --selector makoto.sora \
  --reason "guardian investigation" \
  --until-ms 1750000000000 \
  --guardian-ticket '{"sig":"guardian"}'

iroha sns unfreeze \
  --selector makoto.sora \
  --governance-json /path/to/unfreeze_hook.json
```

`--governance-json` میں درست `GovernanceHookV1` ریکارڈ ہونا چاہیے (identificación de propuesta, hashes de voto, firmas de administrador/tutor)۔ ہر کمانڈ متعلقہ `/v1/sns/names/{namespace}/{literal}/...` endpoint کی عکاسی کرتی ہے تاکہ beta operadores بالکل وہی Torii ensayo de superficies کر سکیں جو SDKs کال کریں گے۔

## 4. Servicio gRPC

```text
service Registrar {
    rpc Register(RegisterNameRequestV1) returns (RegisterNameResponseV1);
    rpc Renew(RenewNameRequestV1) returns (NameRecordV1);
    rpc Transfer(TransferNameRequestV1) returns (NameRecordV1);
    rpc UpdateControllers(UpdateControllersRequestV1) returns (NameRecordV1);
    rpc Freeze(FreezeNameRequestV1) returns (NameRecordV1);
    rpc Unfreeze(GovernanceHookV1) returns (NameRecordV1);
    rpc AssignReserved(ReservedAssignmentRequestV1) returns (NameRecordV1);
    rpc GetRegistration(NameSelectorV1) returns (NameRecordV1);
    rpc GetPolicy(SuffixId) returns (SuffixPolicyV1);
}
```Formato de cable: hash de esquema Norito en tiempo de compilación
`fixtures/norito_rpc/schema_hashes.json` میں درج ہے (filas `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1`, etc.).

## 5. Ganchos de gobernanza y evidencia

ہر llamada mutante کو repetición کے لئے موزوں evidencia منسلک کرنا ہوتا ہے:

| Acción | Datos de gobernanza requeridos |
|--------|--------------------------|
| Registro/renovación estándar | Instrucción de liquidación کو consulte کرنے والی comprobante de pago؛ votación del consejo کی ضرورت نہیں جب تک nivel کو aprobación del delegado نہ چاہئے۔ |
| Registro de nivel premium/asignación reservada | `GovernanceHookV1` ID de propuesta + acuse de recibo del administrador consulte کرتا ہے۔ |
| Traslado | Hash de voto del consejo + hash de señal DAO؛ autorización del tutor جب resolución de disputas de transferencia سے desencadenante ہو۔ |
| Congelar/Descongelar | Firma del boleto del tutor کے ساتھ anulación del consejo (descongelación) ۔ |

Pruebas Torii کو جانچتے ہوئے چیک کرتا ہے:

1. Libro mayor de gobernanza de identificación de propuesta (`/v1/governance/proposals/{id}`) میں موجود ہے اور status `Approved` ہے۔
2. hashes ریکارڈ شدہ artefactos de voto سے coincidencia کرتے ہیں۔
3. firmas de administrador/tutor `SuffixPolicyV1` سے متوقع claves públicas کو consulte کرتے ہیں۔

Comprobaciones fallidas `sns_err_governance_missing` واپس کرتے ہیں۔

## 6. Ejemplos de flujo de trabajo

### 6.1 Registro estándar1. Cliente `/v1/sns/policies/{suffix_id}` کو consulta کرتا ہے تاکہ precios, gracia اور دستیاب niveles حاصل کرے۔
2. Cliente `RegisterNameRequestV1` بناتا ہے:
   - `selector` ترجیحی i105 یا segunda mejor etiqueta comprimida (`sora`) سے derivada ہے۔
   - `term_years` پالیسی حدود میں۔
   - Transferencia del divisor de tesorería/administrador `payment` کو consulte کرتا ہے۔
3. Torii valida کرتا ہے:
   - Normalización de etiquetas + lista reservada۔
   - Plazo/precio bruto vs `PriceTierV1`.
   - Monto del comprobante de pago >= precio calculado + tarifas۔
4. کامیابی پر Torii:
   - `NameRecordV1` محفوظ کرتا ہے۔
   - `RegistryEventV1::NameRegistered` emite کرتا ہے۔
   - `RevenueAccrualEventV1` emite کرتا ہے۔
   - نیا registro + eventos واپس کرتا ہے۔

### 6.2 Renovación Durante la Gracia

Renovaciones de gracia میں solicitud estándar کے ساتھ detección de penalización شامل ہے:

- Torii `now` vs `grace_expires_at` چیک کرتا ہے اور `SuffixPolicyV1` سے tablas de recargos شامل کرتا ہے۔
- Comprobante de pago کو cobertura de recargo کرنا ہوگا۔ Fallo => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` نیا `expires_at` ریکارڈ کرتا ہے۔

### 6.3 Congelación del guardián y anulación del consejo1. Guardian `FreezeNameRequestV1` envía کرتا ہے جس میں ID de incidente کا حوالہ دینے والا ticket ہوتا ہے۔
2. Torii registra کو `NameStatus::Frozen` میں منتقل کرتا ہے، `NameFrozen` emite کرتا ہے۔
3. Remediación کے بعد anulación del consejo جاری کرتا ہے؛ operador BORRAR `/v1/sns/names/{namespace}/{literal}/freeze` کو `GovernanceHookV1` کے ساتھ بھیجتا ہے۔
4. Torii anula valida کرتا ہے، `NameUnfrozen` emite کرتا ہے۔

## 7. Códigos de validación y error

| Código | Descripción | HTTP |
|------|-------------|------|
| `sns_err_reserved` | Etiqueta reservada یا bloqueada ہے۔ | 409 |
| `sns_err_policy_violation` | Término, controladores de nivel یا کا سیٹ پالیسی کی خلاف ورزی کرتا ہے۔ | 422 |
| `sns_err_payment_mismatch` | Comprobante de pago میں valor یا desajuste de activos۔ | 402 |
| `sns_err_governance_missing` | Artefactos de gobernanza requeridos غائب/invalid ہیں۔ | 403 |
| `sns_err_state_conflict` | موجودہ estado del ciclo de vida میں operación permitida نہیں۔ | 409 |

Códigos de acceso `X-Iroha-Error-Code` y estructurados Norito Sobres JSON/NRPC کے ذریعے superficie ہوتے ہیں۔

## 8. Notas de implementación

- Torii subastas pendientes کو `NameRecordV1.auction` میں رکھتا ہے اور `PendingAuction` کے دوران intentos de registro directo کو rechazar کرتا ہے۔
- Comprobantes de pago Norito Recibos del libro mayor دوبارہ استعمال کرتے ہیں؛ API auxiliares de servicios de tesorería (`/v1/finance/sns/payments`) فراہم کرتی ہیں۔
- SDK, puntos finales, ayudantes fuertemente tipados, envoltura, billeteras y motivos de error (`ERR_SNS_RESERVED`, etc.)## 9. Próximos pasos

- Subastas SN-3 کے بعد Torii manejadores کو اصل contrato de registro سے cable کریں۔
- Guías específicas de SDK (Rust/JS/Swift) شائع کریں جو اس API کو consulte کریں۔
- [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) کو campos de evidencia de gancho de gobernanza کے enlaces cruzados کے ساتھ extender کریں۔