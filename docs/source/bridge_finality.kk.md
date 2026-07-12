---
lang: kk
direction: ltr
source: docs/source/bridge_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5e28e5c38283ad6be40a0fc48e0312797f490542a143f4cefdd209aaf8099ac5
source_last_modified: "2026-07-11T20:38:35.470900+00:00"
translation_last_reviewed: 2026-07-12
translator: machine-google-reviewed
---

<!--
SPDX-License-Identifier: Apache-2.0
-->

# Көпір финалдылығының дәлелдері

Бұл құжат бірінші шығарылымдағы көпір финалдылығының пішімін анықтайды. Дәлел
Sumeragi v2 жасап, тұрақты сақтаған нақты финалдылық дерегін тасымалдайды. Дәлел
қабының схема нұсқасы — `1`, ал ішіндегі консенсус протоколының нұсқасы — `2`.
Sumeragi v1 сертификатына проекция, декодер немесе fallback жолы жоқ.

## Дәлелдің нақты пішімі

Norito немесе Norito JSON арқылы кодталған `BridgeFinalityProof` тек үш өрістен тұрады:

```text
{ version, block_header, finality_artifact }
```

- `version` міндетті түрде `1` болуы керек;
- `block_header` — сұралған биіктіктің канондық `BlockHeader`-і;
- `finality_artifact` — осы блок үшін сақталған нақты `V2FinalityArtifact`. Ол height-context
  roster ретімен әр validator-дың BLS-normal PoP-ын (`validator_set_pops`) тұрақты түрде
  өзіне енгізеді.

Артефакт толық әрі өзгермейтін `HeightContext`, нақты `BlockSubject`, блок hash-і,
CommitQC және roster-ге сәйкестендірілген PoP-тарды қамтиды. Height context chain,
epoch, roster, `DualQuorum`, DA орналасуы, leader seed және басқа консенсус деректерін
бекітеді. Epoch-ты аяқтайтын ата-блок context-і optional `next_epoch_snapshot`-ты да
қамтиды; бұл өріс context id бөлігі болғандықтан, бала roster-ге рұқсат берілмей тұрып
ата CommitQC оны аутентификациялайды. Finalized snapshot келесі epoch параметрлерімен
бірге `epoch_end_height` пен келесі roster-ге сәйкестендірілген `validator_set_pops`-ты да бекітеді.

## Тұрақты сақтау және тексеру

Sumeragi v2 apply жолы артефактты тексеріп, өзгермейтін Kura sidecar ретінде сақтайды.
Дәлел құрастырушысы канондық блок пен оның sidecar-ын оқиды; тарихи PoP не сертификатты
өзгермелі ағымдағы world state-тен қайта жасамайды. Жоқ, бүлінген, қайшы немесе
тексерілмейтін sidecar жабық түрде қабылданбайды; қолжетімділік соңғы in-memory тарих
терезесімен шектелмейді.

Stateless тексергіш version, chain, height, header hash, header-дің canonical predecessor-ы
және view-ы, context, subject және CommitQC-ді дәл сәйкестендіріп, артефакттағы барлық PoP-ты
тексереді. Қол қоюшы индекстері қатаң
өспелі және диапазонда болуы керек. CommitQC validator саны мен дауыс қуатының екі
quorum талабын да орындап, нақты Sumeragi v2 vote preimage үшін BLS aggregate signature
жарамды болуы тиіс.

## Сенім зәкірі және мұрагерлерді тексеру

Жеке дәлел тек өзі алып келген roster астындағы ішкі сәйкестікті көрсетеді.
`BridgeFinalityVerifier` алғашқы дәлелге дейін анық сенімді `HeightContextId` талап етеді.
Одан кейін тек тікелей келесі биіктікті қабылдап, бала context-тегі parent CommitQC-ді
алдыңғы бекітілген roster және PoP арқылы тексереді. Epoch ішінде бала artifact алдыңғы
artifact PoP-тарын көшіреді; шекарада epoch, roster, quorum, seed және PoP ата CommitQC
аутентификациялаған `next_epoch_snapshot`-қа, соның ішінде `epoch_end_height`-қа, сай
болуы керек. Ескі, аттап өтілген және байланыспаған биіктіктер қабылданбайды.

SCCP сол `BridgeFinalityProof` түрін қолданады. Хабар берген roster астындағы қолтаңбаға
ғана сенуге болмайды; governance бекіткен checkpoint context/артефактынан хабар
артефактына дейінгі әрбір тікелей successor тексерілуі тиіс.

## Bundle және API

`BridgeFinalityBundle` дәл `{ commitment, finality_proof }` пішімінде. Commitment:
`{ chain_id, height_context_id, block_height, block_hash }`.

- `GET /v1/bridge/finality/{height}` `BridgeFinalityProof` қайтарады;
- `GET /v1/bridge/finality/bundle/{height}` `BridgeFinalityBundle` қайтарады.

Блок немесе нақты тұрақты v2 артефакты жоқ не жарамсыз болса, екі endpoint те жабық
түрде сәтсіз аяқталады. Белгісіз өрістер, қолдаусыз нұсқалар және ескірген дәлел
пішімдері қабылданбауы керек.
