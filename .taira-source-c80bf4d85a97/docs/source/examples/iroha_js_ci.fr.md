---
lang: fr
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

# Iroha Référence JS CI

Le package `@iroha/iroha-js` regroupe les liaisons natives via `iroha_js_host`. N'importe lequel
Le pipeline CI qui exécute des tests ou des builds doit fournir à la fois un environnement d'exécution Node.js
et la chaîne d'outils Rust pour que le bundle natif puisse être compilé avant les tests
courir.

## Étapes recommandées

1. Utilisez une version Node LTS (18 ou 20) via `actions/setup-node` ou votre CI
   équivalent.
2. Installez la chaîne d'outils Rust répertoriée dans `rust-toolchain.toml`. Nous recommandons
   `dtolnay/rust-toolchain@v1` dans les actions GitHub.
3. Mettez en cache les index cargo/git et le répertoire `target/` pour éviter
   reconstruire l'addon natif dans chaque tâche.
4. Exécutez `npm install`, puis `npm run lint:test`. Le script combiné applique
   ESLint sans avertissement, construit le module complémentaire natif et exécute le test Node
   suite afin que CI corresponde au flux de travail de contrôle des versions.
5. Exécutez éventuellement `node --test` comme étape de fumée rapide une fois `npm run build:native`.
   a produit l'addon (par exemple, pré-soumettez des voies de contrôle rapide qui réutilisent
   artefacts mis en cache).
6. Superposez tous les contrôles de peluchage ou de formatage supplémentaires de votre consommateur
   projet au-dessus de `npm run lint:test` lorsque des politiques plus strictes sont requises.
7. Lors du partage de configuration entre services, chargez `iroha_config` et transmettez le
   document analysé en `resolveToriiClientConfig({ config })` afin que les clients Node
   réutiliser la même politique de délai d'attente/nouvelle tentative/jeton que le reste du déploiement (voir
   `docs/source/sdk/js/quickstart.md` pour un exemple complet).

## Modèle d'actions GitHub

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

## Travail de fumée rapide (facultatif)

Pour les demandes d'extraction qui touchent uniquement la documentation ou les définitions TypeScript, un
un travail minimal peut réutiliser les artefacts mis en cache, reconstruire le module natif et exécuter le
Exécuteur de test de nœud directement :

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

Ce travail se termine rapidement tout en vérifiant que le module complémentaire natif se compile
et que la suite de tests Node réussit.

> **Implémentation de référence :** le référentiel comprend
> `.github/workflows/javascript-sdk.yml`, qui câble les étapes ci-dessus dans un
> Matrice de nœuds 18/20 avec mise en cache du fret.