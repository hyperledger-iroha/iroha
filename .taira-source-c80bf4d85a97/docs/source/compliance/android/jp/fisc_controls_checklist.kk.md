---
lang: kk
direction: ltr
source: docs/source/compliance/android/jp/fisc_controls_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2d8b4c90c94dddd8118fcb9c55f07c25000c6dab1f8d239570402023ab89e844
source_last_modified: "2025-12-29T18:16:35.928660+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# FISC қауіпсіздік бақылауларының бақылау тізімі — Android SDK

| Өріс | Мән |
|-------|-------|
| Нұсқа | 0,1 (2026-02-12) |
| Қолдану аясы | Жапондық қаржылық орналастыруларда қолданылатын Android SDK + оператор құралы |
| Меншік иелері | Сәйкестік және заң (Даниэл Парк), Android бағдарламасының жетекшісі |

## Басқару матрицасы

| FISC бақылау | Іске асыру мәліметтері | Дәлелдер / Анықтамалар | Күй |
|-------------|-----------------------|-----------------------|--------|
| **Жүйе конфигурациясының тұтастығы** | `ClientConfig` манифест хэштеуді, схеманы тексеруді және тек оқуға арналған орындалу уақытына қатынасуды қамтамасыз етеді. Конфигурацияны қайта жүктеу қателері runbook ішінде құжатталған `android.telemetry.config.reload` оқиғаларын шығарады. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`; `docs/source/android_runbook.md` §1–2. | ✅ Орындалды |
| **Қатынасты басқару және аутентификация** | SDK Torii TLS саясаттарын және `/v1/pipeline` қол қойылған сұрауларды құрметтейді; оператордың жұмыс ағындарының анықтамасы Қол қойылған Norito артефактілері арқылы ұлғайтуға арналған §4–5 Қолдау көрсету Playbook плюс шлюзді қайта анықтау. | `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_redaction.md` (жұмыс процесін қайта анықтау). | ✅ Орындалды |
| **Криптографиялық кілттерді басқару** | StrongBox таңдаулы провайдерлер, аттестацияны тексеру және құрылғы матрицасын қамту KMS сәйкестігін қамтамасыз етеді. `artifacts/android/attestation/` астында мұрағатталған және дайындық матрицасында қадағаланатын аттестаттау қондырғысының шығыстары. | `docs/source/sdk/android/key_management.md`; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`; `scripts/android_strongbox_attestation_ci.sh`. | ✅ Орындалды |
| **Тіркеу, бақылау және сақтау** | Телеметрияны редакциялау саясаты құпия деректерді хэштейді, құрылғы атрибуттарын шелекке бөледі және сақтауды күшейтеді (30/7/90/365 күндік терезелер). Қолдау ойнату кітабы §8 бақылау тақтасының шектерін сипаттайды; `telemetry_override_log.md` ішінде жазылған қайта анықтау. | `docs/source/sdk/android/telemetry_redaction.md`; `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_override_log.md`. | ✅ Орындалды |
| **Операциялар мен өзгерістерді басқару** | GA кесу процедурасы (Қолдау көрсету Playbook §7.2) плюс `status.md` жаңартулары шығару дайындығын қадағалайды. `docs/source/compliance/android/eu/sbom_attestation.md` арқылы байланыстырылған дәлелдемелерді (SBOM, Sigstore бумалары). | `docs/source/android_support_playbook.md`; `status.md`; `docs/source/compliance/android/eu/sbom_attestation.md`. | ✅ Орындалды |
| **Оқиғаға ден қою және есеп беру** | Playbook маңыздылық матрицасын, SLA жауап терезелерін және сәйкестік туралы хабарландыру қадамдарын анықтайды; телеметрияны қайта анықтау + хаос жаттығулары ұшқыштар алдында қайталануды қамтамасыз етеді. | `docs/source/android_support_playbook.md` §§4–9; `docs/source/sdk/android/telemetry_chaos_checklist.md`. | ✅ Орындалды |
| **Деректердің резиденттігі/локализациясы** | JP орналастыруларына арналған телеметриялық коллекторлар бекітілген Токио аймағында жұмыс істейді; StrongBox аттестаттау жинақтары аймақта сақталады және серіктес билеттерінен сілтеме жасалады. Локализация жоспары бета нұсқасына дейін жапон тілінде қолжетімді құжаттарды қамтамасыз етеді (AND5). | `docs/source/android_support_playbook.md` §9; `docs/source/sdk/android/developer_experience_plan.md` §5; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`. | 🈺 Орындалуда (локализация жалғасуда) |

## Шолушының жазбалары

- Реттелетін серіктесті қосу алдында Galaxy S23/S24 құрылғысының матрицалық жазбаларын тексеріңіз (`s23-strongbox-a`, `s24-strongbox-a` дайындығы туралы құжат жолдарын қараңыз).
- JP орналастыруларындағы телеметриялық коллекторлардың DPIA (`docs/source/compliance/android/eu/gdpr_dpia_summary.md`) ішінде анықталған бірдей сақтау/қайта анықтау логикасын орындауын қамтамасыз етіңіз.
- Банк серіктестері осы бақылау тізімін қарап шыққаннан кейін сыртқы аудиторлардан растауды алыңыз.