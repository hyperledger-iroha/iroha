---
lang: es
direction: ltr
source: docs/source/examples/iroha_js_ci.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d21afd33da5ee459b1f6ffb6ac7c42adc0852ed7929e69993f81914637b5e6b5
source_last_modified: "2026-01-03T18:08:00.440949+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
  This document provides guidance for running @iroha/iroha-js in CI systems.
-->

# Iroha Referencia JS CI

El paquete `@iroha/iroha-js` incluye enlaces nativos a través de `iroha_js_host`. Cualquiera
La canalización de CI que ejecuta pruebas o compilaciones debe proporcionar un tiempo de ejecución de Node.js
y la cadena de herramientas Rust para que el paquete nativo pueda compilarse antes de las pruebas.
correr.

## Pasos recomendados

1. Utilice una versión de Node LTS (18 o 20) a través de `actions/setup-node` o su CI
   equivalente.
2. Instale la cadena de herramientas de Rust que figura en `rust-toolchain.toml`. recomendamos
   `dtolnay/rust-toolchain@v1` en Acciones de GitHub.
3. Almacene en caché los índices de registro de carga/git y el directorio `target/` para evitar
   reconstruir el complemento nativo en cada trabajo.
4. Ejecute `npm install`, luego `npm run lint:test`. El guión combinado impone
   ESLint sin advertencias, crea el complemento nativo y ejecuta la prueba de Nodo
   suite para que CI coincida con el flujo de trabajo de activación de versiones.
5. Opcionalmente, ejecute `node --test` como paso de humo rápido una vez `npm run build:native`
   ha producido el complemento (por ejemplo, enviar previamente líneas de verificación rápida que reutilizan
   artefactos almacenados en caché).
6. Superponga cualquier verificación de formato o pelusa adicional de su consumidor
   proyecto además de `npm run lint:test` cuando se requieren políticas más estrictas.
7. Al compartir la configuración entre servicios, cargue `iroha_config` y pase el
   documento analizado en `resolveToriiClientConfig({ config })` para que los clientes de Node
   reutilizar la misma política de tiempo de espera/reintento/token que el resto de la implementación (consulte
   `docs/source/sdk/js/quickstart.md` para ver un ejemplo completo).

## Plantilla de acciones de GitHub

```yaml
name: iroha-js-ci

on:
  push:
    branches: [ main ]
  pull_request:

jobs:
  build:
    runs-on: ubuntu-latest
    strategy:
      fail-fast: false
      matrix:
        node-version: [18, 20]
    steps:
      - uses: actions/checkout@v4

      - name: Set up Node.js
        uses: actions/setup-node@v4
        with:
          node-version: ${{ matrix.node-version }}
          cache: npm

      - name: Set up Rust toolchain
        uses: dtolnay/rust-toolchain@v1
        with:
          toolchain: stable

      - name: Cache cargo artifacts
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - run: npm install
      - run: npm run lint:test
```

## Trabajo de humo rápido (opcional)

Para solicitudes de extracción que solo tocan documentación o definiciones de TypeScript, un
Un trabajo mínimo puede reutilizar artefactos almacenados en caché, reconstruir el módulo nativo y ejecutar el
Ejecutor de prueba de nodo directamente:

```yaml
jobs:
  smoke:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 20
      - uses: dtolnay/rust-toolchain@v1
        with:
          toolchain: stable
      - run: npm ci
      - run: npm run build:native
      - run: node --test
```

Este trabajo se completa rápidamente mientras se verifica que el complemento nativo se compila
y que pase el conjunto de pruebas de Node.

> **Implementación de referencia:** el repositorio incluye
> `.github/workflows/javascript-sdk.yml`, que conecta los pasos anteriores a un
> Matriz de nodos 18/20 con almacenamiento en caché de carga.