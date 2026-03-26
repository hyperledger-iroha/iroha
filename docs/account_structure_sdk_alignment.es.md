# Nota de despliegue de I105 para responsables de SDK y códecs

Equipos: SDK de Rust, SDK de TypeScript/JavaScript, SDK de Python, SDK de Kotlin, tooling de códecs

Contexto: `docs/account_structure.md` ahora refleja la implementación de AccountId I105 en producción.
Alineen el comportamiento y las pruebas de los SDK con la especificación canónica.

Referencias clave:
- Códec de direcciones + layout del encabezado — `docs/account_structure.md` §2
- Registro de curvas — `docs/source/references/address_curve_registry.md`
- Manejo de dominio Norm v1 — `docs/source/references/address_norm_v1.md`
- Vectores de fixtures — `fixtures/account/address_vectors.json`

Acciones:
1. **Salida canónica:** `AccountId::to_string()`/Display DEBE emitir solo I105
   (sin sufijo `@domain`). El hex canónico es solo para depuración (`0x...`).
2. **Accepted inputs:** parsers MUST accept only canonical Katakana i105 account literals. Reject non-canonical Katakana i105 literals, canonical hex (`0x...`), any `@<domain>` suffix, alias literals, legacy `norito:<hex>`, and `uaid:` / `opaque:` parser forms.
3. **Resolvers:** canonical account parsing has no default-domain binding, scoped inference, or fallback resolver path. Use `ScopedAccountId` only on interfaces that explicitly require `<account>@<domain>`.
4. **Checksum I105:** usar Blake2b-512 sobre `I105PRE || prefix || payload` y
   tomar los primeros 2 bytes. La base del alfabeto comprimido es **105**.
5. **Habilitación de curvas:** los SDK usan Ed25519 por defecto. Proveer opt-in
   explícito para ML‑DSA/GOST/SM (flags de build en Swift; `configureCurveSupport`
   en JS/Android). No asumir secp256k1 habilitado por defecto fuera de Rust.
6. **Sin CAIP-10:** aún no hay mapeo CAIP‑10 entregado; no exponer ni depender
   de conversiones CAIP‑10.

Por favor confirmen una vez que los códecs/pruebas estén actualizados; las dudas
pueden seguirse en el hilo del RFC de direccionamiento de cuentas.
