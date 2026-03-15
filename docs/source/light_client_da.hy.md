---
lang: hy
direction: ltr
source: docs/source/light_client_da.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6561551b6f00fb37b8e41fc5ade61206d7bd9323ab8e089f3dd5d5cfdfc0fd53
source_last_modified: "2025-12-29T18:16:35.975661+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Light հաճախորդի տվյալների առկայության նմուշառում

Light Client Sampling API-ն թույլ է տալիս վավերացված օպերատորներին առբերել
Merkle-ի կողմից վավերացված RBC կտոր նմուշներ թռիչքի ընթացքում բլոկի համար: Թեթև հաճախորդներ
կարող է տալ պատահական նմուշառման հարցումներ, ստուգել վերադարձված ապացույցները
գովազդեց chunk root-ը և վստահություն ստեղծեք, որ տվյալները հասանելի են առանց
ամբողջ ծանրաբեռնվածությունը վերցնելը:

## Վերջնակետ

```
POST /v2/sumeragi/rbc/sample
```

Վերջնական կետը պահանջում է `X-API-Token` վերնագիր, որը համապատասխանում է կազմաձևվածներից մեկին
Torii API նշաններ: Հարցումները լրացուցիչ սահմանափակ են և ենթակա են օրական
մեկ զանգահարողի բայթ բյուջե; կամ գերազանցելը վերադարձնում է HTTP 429:

### Հարցման մարմին

```json
{
  "block_hash": "<hex-encoded block hash>",
  "height": 42,
  "view": 0,
  "count": 3,
  "seed": 12345
}
```

* `block_hash` – թիրախային բլոկի հեշը վեցանկյունով:
* `height`, `view` – նույնականացնող կրկնակի RBC նիստի համար:
* `count` – նմուշների ցանկալի քանակություն (կանխադրված է 1-ի, սահմանազերծված ըստ կոնֆիգուրացիայի):
* `seed` – կամընտիր դետերմինիստական ​​RNG սերմ՝ վերարտադրելի նմուշառման համար:

### արձագանքման մարմին

```json
{
  "block_hash": "…",
  "height": 42,
  "view": 0,
  "total_chunks": 128,
  "chunk_root": "…",
  "payload_hash": "…",
  "samples": [
    {
      "index": 7,
      "chunk_hex": "…",
      "digest_hex": "…",
      "proof": {
        "leaf_index": 7,
        "depth": 8,
        "audit_path": ["…", null, "…"]
      }
    }
  ]
}
```

Յուրաքանչյուր նմուշ մուտքագրում պարունակում է կտորի ինդեքսը, օգտակար բեռնվածքի բայթերը (վեցանկյուն), SHA-256 թերթիկը
digest և Merkle-ի ընդգրկման ապացույց (ընտրովի եղբայրների և քույրերի հետ կոդավորված են որպես վեցանկյուն
լարեր): Հաճախորդները կարող են ստուգել ապացույցները՝ օգտագործելով `chunk_root` դաշտը:

## Սահմանափակումներ և բյուջեներ

* **Առավելագույն նմուշներ յուրաքանչյուր հարցման համար** – կարգավորելի է `torii.rbc_sampling.max_samples_per_request`-ի միջոցով:
* **Առավելագույն բայթ յուրաքանչյուր հարցում** – ուժի մեջ է մտնում `torii.rbc_sampling.max_bytes_per_request`-ի միջոցով:
* **Օրական բայթ բյուջե** – հետևվում է յուրաքանչյուր զանգահարողի միջոցով `torii.rbc_sampling.daily_byte_budget`-ի միջոցով:
* **Գնահատականի սահմանափակում** – ուժի մեջ է մտնում հատուկ նշանային դույլի միջոցով (`torii.rbc_sampling.rate_per_minute`):

Ցանկացած սահմանաչափը գերազանցող հարցումները վերադարձնում են HTTP 429 (CapacityLimit): Երբ կտորը
խանութն անհասանելի է, կամ նիստին բացակայում են բեռնատար բայթերը վերջնական կետում
վերադարձնում է HTTP 404:

## SDK ինտեգրում

### JavaScript

`@iroha/iroha-js`-ը բացահայտում է `ToriiClient.sampleRbcChunks` օգնականը, որպեսզի տվյալները
Հասանելիության ստուգողները կարող են զանգահարել վերջնակետը՝ առանց իրենց սեփական բեռնափոխադրման
տրամաբանությունը։ Օգնականը վավերացնում է վեցանկյուն օգտակար բեռները, նորմալացնում է ամբողջ թվերը և վերադառնում
մուտքագրված օբյեկտներ, որոնք արտացոլում են վերը նշված պատասխանի սխեման.

```js
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.TORII_URL, {
  apiToken: process.env.TORII_API_TOKEN,
});

const sample = await torii.sampleRbcChunks({
  blockHash: "3d...ff",
  height: 42,
  view: 0,
  count: 3,
  seed: Date.now(),
});

if (!sample) {
  throw new Error("RBC session is not available yet");
}

for (const { digestHex, proof } of sample.samples) {
  verifyMerklizedChunk(sample.chunkRoot, digestHex, proof);
}
```

Օգնականը նետում է, երբ սերվերը վերադարձնում է սխալ ձևավորված տվյալներ՝ օգնելով JS-04 հավասարությանը
թեստերը հայտնաբերում են հետընթացներ Rust և Python SDK-ների կողքին: Ժանգը
(`iroha_client::ToriiClient::sample_rbc_chunks`) և Python
(`IrohaToriiClient.sample_rbc_chunks`) նավի համարժեք օգնականներ; օգտագործել ցանկացած
համապատասխանում է ձեր նմուշառման զրահին: