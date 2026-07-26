---
lang: ka
direction: ltr
source: docs/source/bridge_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1cbd248fe14e63d00f002f09e1663181f3ab9bd99124ffeb89c56763b784046b
source_last_modified: "2026-07-12"
translation_last_reviewed: 2026-07-12
translator: machine-google-reviewed
---

<!--
SPDX-License-Identifier: Apache-2.0
-->

# ხიდის ფინალურობის მტკიცებულებები

ეს დოკუმენტი განსაზღვრავს პირველი გამოშვების ხიდის ფინალურობის ფორმატს. მტკიცებულებას
გადააქვს Sumeragi v2-ის მიერ შექმნილი და მუდმივად შენახული ზუსტი ფინალურობის მონაცემები.
მტკიცებულების გარსის schema version არის `1`, ხოლო შიგნით consensus protocol version —
`2`. Sumeragi v1 certificate-ის projection, decoder ან fallback გზა არ არსებობს.

## მტკიცებულების ზუსტი ფორმატი

Norito ან Norito JSON-ით კოდირებულ `BridgeFinalityProof`-ს მხოლოდ სამი ველი აქვს:

```text
{ version, block_header, finality_artifact }
```

- `version` აუცილებლად `1` უნდა იყოს;
- `block_header` მოთხოვნილი სიმაღლის კანონიკური `BlockHeader`-ია;
- `finality_artifact` ამ ბლოკისთვის შენახული ზუსტი `V2FinalityArtifact`-ია. ის
  height-context roster-ის რიგით მუდმივად შეიცავს თითოეული validator-ის BLS-normal
  PoP-ს (`validator_set_pops`).

Artifact შეიცავს სრულ და უცვლელ `HeightContext`-ს, ზუსტ `BlockSubject`-ს, block hash-ს,
CommitQC-ს და roster-ს შესაბამის PoP-ებს. Height context აფიქსირებს chain-ს, epoch-ს,
roster-ს, `DualQuorum`-ს, DA layout-ს, leader seed-ს და სხვა consensus მონაცემებს.
Epoch-ის დამამთავრებელი parent block-ის context ასევე შეიცავს optional
`next_epoch_snapshot`-ს; რადგან ეს ველი context id-ის ნაწილია, parent CommitQC მას
ამოწმებს მანამდე, სანამ ის child roster-ს უფლებას მისცემს. Finalized snapshot ასევე
ამოწმებს `epoch_end_height`-ს და შემდეგ roster-ს მორგებულ `validator_set_pops`-ს,
შემდეგი epoch-ის პარამეტრებთან ერთად.

## მუდმივი შენახვა და შემოწმება

Finality-ის გამოქვეყნებამდე ან block body eviction-მდე Kura ზუსტ canonical header-ს და
root-authenticated SCCP archive-ს წერს immutable retained-block record-ში, შემდეგ exact
V2 artifact-ს ცალკე immutable finality record-ში ინახავს. ორივე ჩანაწერი idempotent-ია
და იმავე height-ის კონფლიქტს უარყოფს. `build_finality_proof` მხოლოდ retained header-სა
და verified finality record-ს კითხულობს; historical block body-ს ან mutable world state-ის
PoP-ს არასოდეს იყენებს. Restart-ზე header/archive/artifact/hash association ხელახლა
მოწმდება. Body eviction სწორ proof-ს ხელმისაწვდომობას არ უკარგავს; დაკარგული,
დაზიანებული, კონფლიქტური ან შეუმოწმებელი record fail closed რეჟიმში უარყოფილია.

Stateless verifier ზუსტად ადარებს version, chain, height, header hash, header-ის canonical
predecessor-სა და view-ს, context, subject და CommitQC-ს და ამოწმებს artifact-ში არსებულ ყველა PoP-ს.
Signer index-ები მკაცრად მზარდი
და დიაპაზონში უნდა იყოს. CommitQC-მა უნდა დააკმაყოფილოს როგორც validator count, ისე
voting power quorum, ხოლო ზუსტი Sumeragi v2 vote preimage-ის BLS aggregate signature
ვალიდური უნდა იყოს.

## ნდობის anchor და successor-ის შემოწმება

ცალკეული მტკიცებულება მხოლოდ მის მიერ მოტანილი roster-ის ქვეშ შიდა თანმიმდევრულობას
აჩვენებს. `BridgeFinalityVerifier` პირველი მტკიცებულების მიღებამდე აშკარად სანდო
`HeightContextId`-ს ითხოვს. შემდეგ ის მხოლოდ უშუალოდ მომდევნო სიმაღლეს იღებს და child
context-ის parent CommitQC-ს წინა გაყინული roster-ითა და PoP-ით ამოწმებს. Epoch-ის
შიგნით child artifact წინა artifact-ის PoP-ებს აკოპირებს; საზღვარზე epoch, roster,
quorum, seed და PoP უნდა ემთხვეოდეს parent CommitQC-ით დამოწმებულ
`next_epoch_snapshot`-ს, მათ შორის `epoch_end_height`-ს. ძველი, გამოტოვებული და
დაუკავშირებელი სიმაღლეები უარყოფილია.

SCCP იმავე `BridgeFinalityProof`-ს იყენებს. Message-ის მიერ მოცემული roster-ის ქვეშ
ხელმოწერის ნდობა საკმარისი არ არის; governance-ით დაფიქსირებული checkpoint
context/artifact-იდან message artifact-მდე ყოველი უშუალო successor უნდა შემოწმდეს.

## Bundle და API

`BridgeFinalityBundle` ზუსტად `{ commitment, finality_proof }`-ია. Commitment არის
`{ chain_id, height_context_id, block_height, block_hash }`.

- `GET /v1/bridge/finality/{height}` აბრუნებს `BridgeFinalityProof`-ს;
- `GET /v1/bridge/finality/bundle/{height}` აბრუნებს `BridgeFinalityBundle`-ს.

თუ retained canonical header ან ზუსტი მუდმივი v2 artifact არ არის ან არასწორია, ორივე
endpoint fail closed რეჟიმშია. Historical block body eviction სწორ proof-ს
ხელმისაწვდომობას არ უკარგავს. უცნობი ველები, მხარდაუჭერელი version-ები და მოძველებული
proof shape-ები უნდა უარყოფილ იქნეს.
