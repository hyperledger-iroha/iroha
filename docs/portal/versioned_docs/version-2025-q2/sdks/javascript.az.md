---
lang: az
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/javascript.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c19e80d060b1ecd57524e7398420990bd9159e7c4ac431ee5b85cfbf3b3df07
source_last_modified: "2026-01-22T16:26:46.562559+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# JavaScript SDK Quickstart

`@iroha2/torii-client` Torii ətrafında brauzer və Node.js dostu paketi təqdim edir.
Bu sürətli başlanğıc SDK reseptlərindən əsas axınları əks etdirir, beləliklə siz əldə edə bilərsiniz
müştəri bir neçə dəqiqə ərzində işləyir. Daha dolğun nümunələr üçün baxın
Repozitoriyada `javascript/iroha_js/recipes/`.

## 1. Quraşdırın

```sh
npm install @iroha2/torii-client
# or
yarn add @iroha2/torii-client
```

Lokal olaraq əməliyyatlar imzalamağı planlaşdırırsınızsa, kripto köməkçilərini də quraşdırın:

```sh
npm install @iroha2/crypto-target-node  # Node18+/Bun/Deno
```

## 2. Torii müştəri yaradın

```ts title="client.ts"
import {ToriiClient} from '@iroha2/torii-client';

const client = ToriiClient.create({
  apiUrl: 'https://localhost:8080',
  telemetryUrl: 'https://localhost:8080', // optional
});
```

Konfiqurasiya reseptlərdə istifadə olunan konstruktoru əks etdirir. Əgər qovşağınız
əsas auth istifadə edir, `basicAuth` seçimi vasitəsilə `{username, password}` keçir.

## 3. Düyün statusunu əldə edin

```ts
const status = await client.getStatus();
console.log(status.irohaVersion, status.latestBlock.height);
```

Bütün oxu əməliyyatları Norito dəstəkli JSON obyektlərini qaytarır. Yaradılmış növlərə baxın
Sahə təfərrüatları üçün `index.d.ts`.

## 4. Əməliyyat təqdim edin

İmzalayanlar köməkçi API ilə əməliyyatlar qura bilərlər:

```ts
import {createKeyPairFromHex} from '@iroha2/crypto-target-node';
import {ToriiClient, buildTransaction} from '@iroha2/torii-client';

const {publicKey, privateKey} = createKeyPairFromHex(process.env.IROHA_PRIVATE_KEY!);

const tx = buildTransaction({
  chain: '00000000-0000-0000-0000-000000000000', // ChainId
  authority: 'ih58...',
  instructions: [
    {Register: {domain: {name: 'research', logo: null}}},
  ],
});

tx.sign({publicKey, privateKey});
const hash = await client.submitTransaction(tx);
console.log('Submitted tx', hash);
```

Köməkçi əməliyyatı avtomatik olaraq gözlənilən Norito zərfinə yığır
Torii tərəfindən. Daha zəngin bir nümunə üçün (sonluğu gözləmək daxil olmaqla) baxın
`javascript/iroha_js/recipes/registration.mjs`.

## 5. Yüksək səviyyəli köməkçilərdən istifadə edin

SDK CLI-ni əks etdirən ixtisaslaşdırılmış axınları birləşdirir:

- **İdarəetmə köməkçiləri** – `recipes/governance.mjs` səhnələşdirməni nümayiş etdirir
  `governance` təlimat qurucuları ilə təkliflər və bülletenlər.
- **ISO körpüsü** – `recipes/iso_bridge.mjs` `pacs.008` və necə təqdim olunacağını göstərir
  `/v1/iso20022` son nöqtələrindən istifadə edərək sorğu köçürmə statusu.
- **SoraFS və tetikler** – `src/toriiClient.js` altında səhifələmə köməkçiləri ifşa olunur
  müqavilələr, aktivlər, triggerlər və SoraFS provayderləri üçün yazılmış iteratorlar.

Həmin axınlardan təkrar istifadə etmək üçün müvafiq qurucu funksiyalarını `@iroha2/torii-client`-dən idxal edin.

## 6. Səhvlərin idarə edilməsi

Bütün SDK zəngləri nəqliyyat metadatası ilə zəngin `ToriiClientError` nümunələri atır
və Norito səhv yükü. Zəngləri `try/catch`-ə sarın və ya `.catch()` istifadə edin
istifadəçilər üçün səthi kontekst:

```ts
try {
  await client.submitTransaction(tx);
} catch (error) {
  console.error('Torii submission failed', error);
}
```

## Növbəti addımlar

- `javascript/iroha_js/recipes/`-də başdan sona axınlar üçün reseptləri araşdırın.
- Ətraflı məlumat üçün `javascript/iroha_js/index.d.ts`-də yaradılan növləri oxuyun
  metod imzaları.
- Yükləri yoxlamaq və sazlamaq üçün bu SDK-nı Norito sürətli başlanğıc ilə birləşdirin
  Torii nömrəsinə göndərirsiniz.