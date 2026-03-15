---
lang: kk
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1ad407370f375e45f0143f082b33a5ea61698825c8cd92dac402f656fb0f61a2
source_last_modified: "2026-01-22T16:26:46.524254+00:00"
translation_last_reviewed: 2026-02-07
id: dispute-revocation-runbook
title: SoraFS Dispute & Revocation Runbook
sidebar_label: Dispute & Revocation Runbook
description: Governance workflow for filing SoraFS capacity disputes, coordinating revocations, and evacuating data deterministically.
translator: machine-google-reviewed
---

:::ескерту Канондық дереккөз
:::

## Мақсат

Бұл runbook басқару операторларына SoraFS сыйымдылық дауларын беру, қайтарып алуды үйлестіру және деректерді эвакуациялауды анықтауды қамтамасыз ету арқылы басшылыққа алады.

## 1. Оқиғаны бағалаңыз

- **Триггер шарттары:** SLA бұзылуын анықтау (жұмыс уақыты/PoR қатесі), репликацияның жетіспеушілігі немесе есеп айырысу бойынша келіспеушілік.
- **Телеметрияны растау:** провайдер үшін `/v1/sorafs/capacity/state` және `/v1/sorafs/capacity/telemetry` суретін түсіріңіз.
- **Мүдделі тараптарды хабардар ету:** Сақтау тобы (провайдер операциялары), Басқару кеңесі (шешім қабылдайтын орган), Бақылау мүмкіндігі (бақылау тақтасының жаңартулары).

## 2. Дәлелдемелер жинағын дайындаңыз

1. Шикі артефактілерді жинаңыз (телметриялық JSON, CLI журналдары, аудитор жазбалары).
2. Детерминирленген мұрағатқа қалыпқа келтіру (мысалы, тарбол); жазба:
   - BLAKE3-256 дайджест (`evidence_digest`)
   - Тасымалдаушы түрі (`application/zip`, `application/jsonl` және т.б.)
   - Хостинг URI (нысанды сақтау, SoraFS PIN немесе Torii қол жетімді соңғы нүкте)
3. Буманы бір рет жазу рұқсаты бар басқару дәлелдерін жинау шелегінде сақтаңыз.

## 3. Дауды жіберіңіз

1. `sorafs_manifest_stub capacity dispute` үшін арнайы JSON жасаңыз:

   ```json
   {
     "provider_id_hex": "<hex>",
     "complainant_id_hex": "<hex>",
     "replication_order_id_hex": "<hex or omit>",
     "kind": "replication_shortfall",
     "submitted_epoch": 1700100000,
     "description": "Provider failed to ingest order within SLA.",
     "requested_remedy": "Slash 10% stake and suspend adverts",
     "evidence": {
       "digest_hex": "<blake3-256>",
       "media_type": "application/zip",
       "uri": "https://evidence.sora.net/bundles/<id>.zip",
       "size_bytes": 1024
     }
   }
   ```

2. CLI іске қосыңыз:

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=i105... \
     --private-key=ed25519:<key>
   ```

3. `dispute_summary.json` шолу (түрін, дәлел дайджестін, уақыт белгілерін растау).
4. JSON сұрауын Torii `/v1/sorafs/capacity/dispute` нөміріне басқару транзакция кезегі арқылы жіберіңіз. `dispute_id_hex` жауап мәнін түсіру; ол кері қайтарып алу әрекеттерін және аудит есептерін бекітеді.

## 4. Эвакуация және күшін жою

1. **Жеңілдік терезе:** провайдерге алдағы қайтарып алу туралы хабарлау; саясат рұқсат еткенде бекітілген деректерді эвакуациялауға мүмкіндік береді.
2. **`ProviderAdmissionRevocationV1` жасау:**
   - Бекітілген себеппен `sorafs_manifest_stub provider-admission revoke` пайдаланыңыз.
   - Қолтаңбаларды және кері қайтарып алу дайджестін тексеріңіз.
3. **Бас тартуды жариялау:**
   - Torii нөміріне қайтарып алу туралы өтінішті жіберіңіз.
   - Провайдер жарнамаларының бұғатталғанына көз жеткізіңіз (`torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` көтеріледі деп күтіңіз).
4. **Жаңарту бақылау тақталары:** провайдерді қайтарылған деп белгілеңіз, дау идентификаторына сілтеме жасаңыз және дәлелдер бумасын байланыстырыңыз.

## 5. Өлгеннен кейінгі және бақылау

- Басқару оқиғасын бақылау құралында уақыт кестесін, негізгі себебін және түзету әрекеттерін жазып алыңыз.
- Реституцияны анықтаңыз (үлестерді кесу, алымдарды қайтару, тұтынушыны қайтару).
- оқуды құжаттау; қажет болса, SLA шектерін немесе бақылау ескертулерін жаңартыңыз.

№# 6. Анықтамалық материалдар

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (даулы бөлім)
- `docs/source/sorafs/provider_admission_policy.md` (қайтарып алу жұмыс процесі)
- Бақылаудың бақылау тақтасы: `SoraFS / Capacity Providers`

## Бақылау тізімі

- [ ] Дәлелдер жинағы түсіріліп, хэштелген.
- [ ] Даулы пайдалы жүктеме жергілікті түрде расталды.
- [ ] Torii даулы транзакция қабылданды.
- [ ] Қайтару орындалды (егер мақұлданса).
- [ ] Бақылау тақталары/runbooks жаңартылды.
- [ ] Өлімнен кейін басқару кеңесіне жіберілді.