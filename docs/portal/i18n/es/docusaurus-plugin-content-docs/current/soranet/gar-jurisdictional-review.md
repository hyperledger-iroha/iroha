---
lang: es
direction: ltr
source: docs/portal/docs/soranet/gar-jurisdictional-review.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Revision jurisdiccional GAR (SNNet-9)

El track de compliance SNNet-9 ya esta completo. Esta pagina lista las decisiones
jurisdiccionales firmadas, los digests Blake2b-256 que los operadores deben copiar en sus
bloques `compliance.attestations`, y las proximas fechas de revision. Conserva los PDFs
firmados en tu archivo de gobernanza; estos digests son las huellas canonicas para
automatizacion y auditorias.

| Jurisdiccion | Decision | Memo | Digest Blake2b-256 (hex en mayusculas) | Proxima revision |
|--------------|----------|------|---------------------------------------|-----------------|
| Estados Unidos | Se requiere transporte direct-only (sin circuitos SoraNet) | `governance/compliance/attestations/us-2027-q2.md` | `1636B0B52286896C4894FA0333CD691D9B3DB7F2B73548EA2EA622B90A09BCF7` | 2027-09-30 |
| Canada | Se requiere transporte direct-only | `governance/compliance/attestations/ca-2027-q2.md` | `52D9D9EE1E43DA0526D8C659AC61C1844858F9A6A74650EA5C04CBD8F8614063` | 2027-09-30 |
| UE/EEE | Se permite transporte anonimo SoraNet con presupuestos de privacidad SNNet-8 aplicados | `governance/compliance/attestations/eu-2027-q2.md` | `30FDAF718095E87FDFADA6BE3EC1EF9D56DFFDEE97BF4BBEAB9013F7A0963B15` | 2027-09-30 |

## Fragmento de despliegue

```jsonc
{
  "compliance": {
    "operator_jurisdictions": ["US", "CA", "DE"],
    "jurisdiction_opt_outs": ["US", "CA"],
    "blinded_cid_opt_outs": [
      "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828",
      "7F8B1E9D04878F1AEAB553C1DB0A3E3A2AB689F75FE6BE17469F85A4D201B4AC"
    ],
    "attestations": [
      {
        "jurisdiction": "US",
        "document_uri": "norito://gar/attestations/us-2027-q2.pdf",
        "digest_hex": "1636B0B52286896C4894FA0333CD691D9B3DB7F2B73548EA2EA622B90A09BCF7",
        "issued_at_ms": 1805313600000,
        "expires_at_ms": 1822248000000
      },
      {
        "jurisdiction": "CA",
        "document_uri": "norito://gar/attestations/ca-2027-q2.pdf",
        "digest_hex": "52D9D9EE1E43DA0526D8C659AC61C1844858F9A6A74650EA5C04CBD8F8614063",
        "issued_at_ms": 1805313600000,
        "expires_at_ms": 1822248000000
      },
      {
        "jurisdiction": "EU",
        "document_uri": "norito://gar/attestations/eu-2027-q2.pdf",
        "digest_hex": "30FDAF718095E87FDFADA6BE3EC1EF9D56DFFDEE97BF4BBEAB9013F7A0963B15",
        "issued_at_ms": 1805313600000
      }
    ]
  }
}
```

## Checklist de auditoria

- Digests de atestacion copiados exactamente en configs de produccion.
- `jurisdiction_opt_outs` coincide con el catalogo canonico.
- PDFs firmados retenidos en el archivo de gobernanza con digests coincidentes.
- Ventana de activacion y aprobadores capturados en el logbook GAR.
- Recordatorios de proxima revision programados a partir de la tabla anterior.

## Ver tambien

- [Resumen de onboarding para operadores GAR](gar-operator-onboarding)
- [Playbook de compliance GAR (fuente)](../../../source/soranet/gar_compliance_playbook.md)
