---
lang: es
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/javascript.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c19e80d060b1ecd57524e7398420990bd9159e7c4ac431ee5b85cfbf3b3df07
source_last_modified: "2026-01-22T06:58:48.951155+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Inicio rápido del SDK de JavaScript

`@iroha2/torii-client` proporciona un navegador y un contenedor compatible con Node.js alrededor de Torii.
Este inicio rápido refleja los flujos principales de las recetas del SDK para que pueda obtener una
cliente ejecutándose en unos minutos. Para ejemplos más completos, consulte
`javascript/iroha_js/recipes/` en el repositorio.

## 1. Instalar

```sh
npm install @iroha2/torii-client
# or
yarn add @iroha2/torii-client
```

Si planea firmar transacciones localmente, instale también los asistentes criptográficos:

```sh
npm install @iroha2/crypto-target-node  # Node18+/Bun/Deno
```

## 2. Cree un cliente Torii

```ts title="client.ts"
import {ToriiClient} from '@iroha2/torii-client';

const client = ToriiClient.create({
  apiUrl: 'https://localhost:8080',
  telemetryUrl: 'https://localhost:8080', // optional
});
```

La configuración refleja el constructor utilizado en las recetas. Si tu nodo
utiliza autenticación básica, pase `{username, password}` a través de la opción `basicAuth`.

## 3. Obtener el estado del nodo

```ts
const status = await client.getStatus();
console.log(status.irohaVersion, status.latestBlock.height);
```

Todas las operaciones de lectura devuelven objetos JSON respaldados por Norito. Ver los tipos generados en
`index.d.ts` para obtener detalles del campo.

## 4. Enviar una transacción

Los firmantes pueden crear transacciones con la API auxiliar:

```ts
import {createKeyPairFromHex} from '@iroha2/crypto-target-node';
import {ToriiClient, buildTransaction} from '@iroha2/torii-client';

const {publicKey, privateKey} = createKeyPairFromHex(process.env.IROHA_PRIVATE_KEY!);

const tx = buildTransaction({
  chain: '00000000-0000-0000-0000-000000000000', // ChainId
  authority: 'soraカタカナ...',
  instructions: [
    {Register: {domain: {name: 'research', logo: null}}},
  ],
});

tx.sign({publicKey, privateKey});
const hash = await client.submitTransaction(tx);
console.log('Submitted tx', hash);
```

El asistente envuelve automáticamente la transacción en el sobre Norito esperado.
por Torii. Para ver un ejemplo más completo (incluidas las esperas hasta la finalización), consulte
`javascript/iroha_js/recipes/registration.mjs`.

## 5. Utilice ayudantes de alto nivel

El SDK incluye flujos especializados que reflejan la CLI:

- **Ayudantes de gobernanza** – `recipes/governance.mjs` demuestra la puesta en escena
  propuestas y votaciones con los constructores de instrucciones `governance`.
- **Puente ISO** – `recipes/iso_bridge.mjs` muestra cómo enviar `pacs.008` y
  estado de transferencia de encuesta utilizando los puntos finales `/v1/iso20022`.
- **SoraFS y activadores**: se exponen los ayudantes de paginación en `src/toriiClient.js`
  iteradores escritos para contratos, activos, activadores y proveedores SoraFS.

Importe las funciones de creación relevantes de `@iroha2/torii-client` para reutilizar esos flujos.

## 6. Manejo de errores

Todas las llamadas al SDK generan instancias `ToriiClientError` enriquecidas con metadatos de transporte
y la carga útil del error Norito. Envuelva llamadas en `try/catch` o use `.catch()` para
contexto superficial para los usuarios:

```ts
try {
  await client.submitTransaction(tx);
} catch (error) {
  console.error('Torii submission failed', error);
}
```

## Próximos pasos

- Explore las recetas en `javascript/iroha_js/recipes/` para flujos de un extremo a otro.
- Lea los tipos generados en `javascript/iroha_js/index.d.ts` para obtener información detallada.
  firmas de métodos.
- Empareje este SDK con el inicio rápido Norito para inspeccionar y depurar las cargas útiles.
  lo envías a Torii.