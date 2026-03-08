# Nota de despliegue de IH58 para responsables de SDK y códecs

Equipos: SDK de Rust, SDK de TypeScript/JavaScript, SDK de Python, SDK de Kotlin, tooling de códecs

Contexto: `docs/account_structure.md` ahora refleja la implementación de AccountId IH58 en producción.
Alineen el comportamiento y las pruebas de los SDK con la especificación canónica.

Referencias clave:
- Códec de direcciones + layout del encabezado — `docs/account_structure.md` §2
- Registro de curvas — `docs/source/references/address_curve_registry.md`
- Manejo de dominio Norm v1 — `docs/source/references/address_norm_v1.md`
- Vectores de fixtures — `fixtures/account/address_vectors.json`

Acciones:
1. **Salida canónica:** `AccountId::to_string()`/Display DEBE emitir solo IH58
   (sin sufijo `@domain`). El hex canónico es solo para depuración (`0x...`).
2. **Entradas aceptadas:** los parsers DEBEN aceptar IH58 (preferido), `sora`
   comprimido y hex canónico (solo `0x...`; el hex sin prefijo se rechaza).
   Las entradas PUEDEN incluir un sufijo `@<domain>` para hints de ruteo;
   los alias `<label>@<domain>` (rejected legacy form) requieren un resolver. 
   (hex multihash) sigue siendo compatible.
3. **Resolvers:** el parseo IH58/sora sin dominio requiere un resolver de
   selección de dominio, salvo que el selector sea el default implícito
   (usar la etiqueta de dominio por defecto configurada). Los literales UAID
   (`uaid:...`) y opaque (`opaque:...`) requieren resolvers.
4. **Checksum IH58:** usar Blake2b-512 sobre `IH58PRE || prefix || payload` y
   tomar los primeros 2 bytes. La base del alfabeto comprimido es **105**.
5. **Habilitación de curvas:** los SDK usan Ed25519 por defecto. Proveer opt-in
   explícito para ML‑DSA/GOST/SM (flags de build en Swift; `configureCurveSupport`
   en JS/Android). No asumir secp256k1 habilitado por defecto fuera de Rust.
6. **Sin CAIP-10:** aún no hay mapeo CAIP‑10 entregado; no exponer ni depender
   de conversiones CAIP‑10.

Por favor confirmen una vez que los códecs/pruebas estén actualizados; las dudas
pueden seguirse en el hilo del RFC de direccionamiento de cuentas.
