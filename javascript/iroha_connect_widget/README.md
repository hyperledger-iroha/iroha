# @iroha/connect-widget

The IrohaConnect button for SORA Nexus web apps. One element gives an app the
whole IrohaConnect experience, identical in every app:

- the branded **IrohaConnect** button (Iroha flower mark, Iroha red);
- the pairing dialog with the wallet QR code, a same-device "Open wallet"
  link, and the approval moment;
- the connected-account chip and account dialog;
- the **sakura layer**: the Iroha wallet's falling-petal background, added to
  the page automatically and shared by every widget on it.

Pairing runs the current app-side Connect flow from
[`@iroha/iroha-js/connect-browser`](../iroha_js/README.md): the session is
bound to the app's pinned `NetworkId`, registered with Torii, approved by the
wallet with a signature the SDK verifies against Torii's native account
inspection, and encrypted end to end. Private keys never enter the app; every
signature is approved in the wallet.

## Quick start

With a bundler:

```bash
npm install @iroha/connect-widget @iroha/iroha-js
```

```js
import "@iroha/connect-widget";
```

```html
<iroha-connect
  torii-url="https://taira.sora.org"
  network-id="hash:82531CE8EAE8BFF6BEECA4698BFD13A3BC8BEC5F0EE0D23D428C97FC17AB0F3B#3E94"
  app-name="Hanami Market"
></iroha-connect>
```

Without a bundler, load the self-contained build from `dist/` (`npm run build`
produces it; the Connect SDK chunk downloads only when someone starts pairing):

```html
<script type="module" src="/vendor/iroha-connect/iroha-connect.js"></script>
```

`network-id` must be the deployed network's exact `NetworkId` literal. The
widget never guesses it from a chain label or Torii response; a Taira reset
changes it, so take it from the network's published profile.

### Allow your origin on Torii

Browsers call Torii cross-origin (`POST` and `DELETE /v1/connect/session`,
`POST /v1/accounts/inspect`, and `GET /v1/accounts/{account_id}/signing-key`
before raw signing), so the Torii you point at must list your app's origin in
`torii.cors.allowed_origins` with `torii.cors.enabled = true`.
Without it the dialog reports that Torii could not be reached. The relay
WebSocket (`/v1/connect/ws`) also needs upgrade headers forwarded by any
reverse proxy.

## React, Vue, Svelte

The widget is a standard custom element, so it works in every framework.

```jsx
// React 19
import { useEffect, useRef } from "react";
import "@iroha/connect-widget";

const TORII = "https://taira.sora.org";
const NETWORK_ID = "hash:…"; // the deployed network's exact NetworkId

export function ConnectButton({ onAccount }) {
  const ref = useRef(null);
  useEffect(() => {
    const element = ref.current;
    const approved = (event) => onAccount(event.detail.accountId);
    const disconnected = () => onAccount(null);
    element.addEventListener("iroha-connect-approved", approved);
    element.addEventListener("iroha-connect-disconnected", disconnected);
    return () => {
      element.removeEventListener("iroha-connect-approved", approved);
      element.removeEventListener("iroha-connect-disconnected", disconnected);
    };
  }, [onAccount]);
  return <iroha-connect ref={ref} torii-url={TORII} network-id={NETWORK_ID} app-name="Hanami Market" />;
}
```

```vue
<!-- Vue 3: mark iroha-* as custom elements (compilerOptions.isCustomElement) -->
<iroha-connect
  :torii-url="torii"
  :network-id="networkId"
  app-name="Hanami Market"
  @iroha-connect-approved="account = $event.detail.accountId"
  @iroha-connect-disconnected="account = null"
/>
```

```svelte
<iroha-connect
  torii-url={torii}
  network-id={networkId}
  app-name="Hanami Market"
  on:iroha-connect-approved={(event) => (account = event.detail.accountId)}
/>
```

Importing the package during server-side rendering is safe: nothing is
registered until it runs in a browser.

## Attributes

| Attribute | Default | Meaning |
| --- | --- | --- |
| `torii-url` | required | Torii origin, for example `https://taira.sora.org` (no path). |
| `network-id` | required | Exact deployed `NetworkId` literal the session is bound to. |
| `app-name` | page title | Name the wallet shows in its approval prompt. |
| `app-url` | page origin | URL the wallet shows in its approval prompt. |
| `launch-protocol` | `irohaconnect` | Scheme of the same-device "Open wallet" link. |
| `theme` | `auto` | `auto` follows `data-theme` on `<html>`, then a `dark`/`light` class, then the OS. |
| `sakura` | `background` | `background` (behind content), `overlay` (above content, click-through) or `off`. |
| `backdrop` | off | Also paint the SORA Nexus illuminated background behind the page. |
| `lang` | page language | Built-in English and Japanese copy. |
| `allow-insecure` | off | Permit `http://` Torii and `ws://` relay for local development only. |

## Events

All events bubble and cross shadow roots.

| Event | `detail` |
| --- | --- |
| `iroha-connect-state` | `{ status }` plus, while `awaiting`, `walletUri` and `launchUri`; while `connected`, `accountId`. |
| `iroha-connect-approved` | `{ accountId, signingPublicKey, walletPublicKey, sid, toriiUrl }` |
| `iroha-connect-disconnected` | `{ reason, initiator }` where `initiator` is `app` or `remote`. |
| `iroha-connect-error` | `{ code, message, error }`; `code` is `rejected`, `closed`, `network`, `config` or `failed`. |

`status` is `idle`, `preparing`, `awaiting`, `connected` or `error`.

## JavaScript API

```js
const connect = document.querySelector("iroha-connect");

await connect.connect();                       // opens the dialog; resolves with the approval
connect.accountId;                             // approved I105 account id, or null
await connect.signTransaction(unsignedTxBytes); // the wallet asks the user, then signs
await connect.disconnect();

// Torii canonical-request authentication backed by the wallet:
connect.permissions = { methods: ["sign_raw"], resources: ["iroha:torii:canonical-request:v1"] };
const auth = await connect.createCanonicalRequestAuth();
```

Raw signatures (`signRaw`, and the canonical-request auth built on it) come
from the account's separate off-chain signing key, over the SDK's off-chain
envelope (`buildOffchainSigningMessageV1`), never from its transaction key or
over the bare bytes. `connect.accountSigningKey()` returns that key as
`{ publicKey, generation }` for verifying them; accounts without a registered
signing key reject with `SIGNING_KEY_REQUIRED`.

`permissions` must be set before pairing; the wallet enforces it. Widgets
with the same configuration share one session, so a header button and an
in-page call to action stay in sync, and an approved session survives a
single-page-app route change that re-mounts the button. Removing the last
widget while pairing cancels the pairing.

For a custom UI that keeps the same protocol, use the headless controller:

```js
import { IrohaConnectController } from "@iroha/connect-widget/controller";

const controller = new IrohaConnectController({ toriiUrl, networkId, appMeta: { name: "Hanami Market" } });
controller.addEventListener("statechange", (event) => render(event.detail));
await controller.connect();
```

## The sakura layer on its own

Pages without a connect button (a landing page, a wallet, an explorer) use the
same animation directly:

```js
import "@iroha/connect-widget/sakura";
```

```html
<body>
  <iroha-sakura backdrop></iroha-sakura>
  …
</body>
```

The layer is fixed, click-through and `aria-hidden`. It keeps the wallet's
petal shape, colors, density (one petal per 26,000 px², 16–55 petals), pointer
wind and parallax (written to `--iroha-parallax-x` / `--iroha-parallax-y`). It
pauses in hidden tabs, drops to 30 fps and 36 petals on low-power devices, and
shows a still frame when the user prefers reduced motion.

In `background` mode the layer sits behind page content: petals show through
transparent or frosted surfaces and hide behind opaque ones. Give the page
background to `<body>` (or `<html>`), not to both.

An app that already owns its theme and stacking can drive its own canvas with
the same engine instead of copying it; the Iroha wallet does this:

```js
import { SakuraField } from "@iroha/connect-widget/sakura";

// `canvas`: fixed, full-viewport, click-through and aria-hidden.
const field = new SakuraField(canvas, { theme: "dark" }).start();
field.setTheme("light"); // follow the app's theme
field.destroy(); // when the canvas leaves the page
```

## Brand rules

The widget renders the canonical IrohaConnect look; keep it recognizable.
Layout may be adjusted through `::part(trigger)`, `::part(dialog)` and
`::part(panel)` (for example margins), and copy through
`element.strings = { … }`, but keep the Iroha mark, colors and the sakura
layer so users recognize IrohaConnect in every app.

## Development

```bash
npm install            # links ../iroha_js; build its dist first: (cd ../iroha_js && npm run build:dist)
npm run lint
npm run typecheck
npm test
npm run build          # dist/iroha-connect.js and dist/iroha-sakura.js
```

`examples/playground.html` walks through every state with a simulated wallet
(serve the package directory and open it after `npm run build`);
`examples/index.html` pairs with a real wallet through a Torii that allows the
page's origin.
