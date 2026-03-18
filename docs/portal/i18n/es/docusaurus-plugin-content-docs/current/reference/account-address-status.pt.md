---
lang: es
direction: ltr
source: docs/portal/docs/reference/account-address-status.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: dirección-cuenta-estado
título: Conformidad de enderecos de conta
descripción: Resumen del flujo del dispositivo ADDR-2 y como equipos de SDK ficam sincronizadas.
---

El paquete canónico ADDR-2 (`fixtures/account/address_vectors.json`) accesorios de captura I105 (preferido), comprimido (`sora`, segundo mejor; ancho medio/completo), firma múltiple y negativo. Cada superficie de SDK + Torii utiliza un mismo JSON para detectar cualquier deriva de códec antes de cargar la producción. Esta página espelha o brief interno de status (`docs/source/account_address_status.md` no repositorio raiz) para que los lectores del portal consulten o fluxo sem vasculhar o mono-repo.

## Regenerar o verificar el pacote

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Banderas:

- `--stdout`: emite JSON en salida estándar para inspección ad-hoc.
- `--out <path>` - grava en un camino diferente (ej.: ao comparar mudancas localmente).
- `--verify` - compara una copia de trabajo con el contenido recibido (nao pode ser combinado con `--stdout`).

El flujo de trabajo de CI **Address Vector Drift** roda `cargo xtask address-vectors --verify`
siempre que el dispositivo, el generador o los documentos se muden para alertar a los revisores inmediatamente.

## ¿Quem consome o accesorio?

| Superficie | Validación |
|---------|------------|
| Modelo de datos de Rust | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (servidor) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| SDK de JavaScript | `javascript/iroha_js/test/address.test.js` |
| SDK rápido | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| SDK de Android | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |Cada arnés faz round-trip de bytes canónicos + I105 + codificaciones comprimidos y verifica se os códigos de error no estilo Norito batem com o accesorio para casos negativos.

## ¿Precisa de automacao?

Ferramentas de liberación podem automatizar actualizaciones de accesorio con o ayudante
`scripts/account_fixture_helper.py`, que busca o verifica el paquete canónico sin copiar/pegar:

```bash
# Download to a custom path (defaults to fixtures/account/address_vectors.json)
python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

# Verify that a local copy matches the canonical source (HTTPS or file://)
python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

# Emit Prometheus textfile metrics for dashboards/alerts
python3 scripts/account_fixture_helper.py check \
  --target path/to/sdk/address_vectors.json \
  --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
  --metrics-label android
```

El asistente reemplaza `--source` o una variación de ambiente `IROHA_ACCOUNT_FIXTURE_URL` para que los trabajos de CI de SDK apontem para su espejo preferido. Cuando `--metrics-out` y fornecido, el archivo auxiliar `account_address_fixture_check_status{target=\"...\"}` junto con el resumen SHA-256 canónico (`account_address_fixture_remote_info`) para que los recopiladores de archivos de texto hagan Prometheus y el tablero Grafana `account_address_fixture_status` compruebe que cada superficie permanece en sincronización. Gere alerta quando um target reportar `0`. Para el uso automático de múltiples superficies, el contenedor `ci/account_fixture_metrics.sh` (aceita `--target label=path[::source]` repetidos) para que equipe on-call publiquem un único archivo `.prom` consolidado para el recopilador de archivos de texto del nodo-exportador.

## ¿Precisa el breve completo?

O status completo de conformidade ADDR-2 (owners, plano de monitoramento, itens de acao em aberto)
fica em `docs/source/account_address_status.md` dentro del repositorio junto con el Address Structure RFC (`docs/account_structure.md`). Utilice esta página como lembrete operacional rápido; Para orientación detallada, consulte los documentos del repositorio.