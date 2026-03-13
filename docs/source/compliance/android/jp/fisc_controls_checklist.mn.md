---
lang: mn
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

# FISC Аюулгүй байдлын хяналтын хяналтын хуудас — Android SDK

| Талбай | Үнэ цэнэ |
|-------|-------|
| Хувилбар | 0.1 (2026-02-12) |
| Хамрах хүрээ | Японы санхүүгийн хэрэглээнд ашигладаг Android SDK + оператор хэрэгсэл |
| Эзэмшигчид | Дагаж мөрдөх ба хууль эрх зүй (Даниел Парк), Android хөтөлбөрийн удирдагч |

## Хяналтын матриц

| FISC хяналт | Хэрэгжилтийн дэлгэрэнгүй | Нотлох баримт / Лавлагаа | Статус |
|-------------|-----------------------|-----------------------|--------|
| **Системийн тохиргооны бүрэн бүтэн байдал** | `ClientConfig` нь манифест хэшлэх, схем баталгаажуулалт, зөвхөн унших боломжтой ажиллах цагийн хандалтыг хэрэгжүүлдэг. Тохиргооны дахин ачааллын алдаа нь runbook-д бичигдсэн `android.telemetry.config.reload` үйл явдлуудыг гаргадаг. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`; `docs/source/android_runbook.md` §1–2. | ✅ Хэрэгжүүлсэн |
| **Хандалтын хяналт ба баталгаажуулалт** | SDK нь Torii TLS бодлого болон `/v2/pipeline` гарын үсэгтэй хүсэлтийг хүндэтгэдэг; Операторын ажлын урсгалын лавлагаа Дэмжих Playbook §4–5-д гарын үсэг зурсан Norito олдворуудаар дамжуулж хаалтыг хүчингүй болгох. | `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_redaction.md` (ажлын урсгалыг хүчингүй болгох). | ✅ Хэрэгжүүлсэн |
| **Криптографийн түлхүүрийн удирдлага** | StrongBox-ийг илүүд үздэг үйлчилгээ үзүүлэгчид, баталгаажуулалтын баталгаажуулалт, төхөөрөмжийн матрицын хамрах хүрээ нь KMS-ийн нийцлийг баталгаажуулдаг. Баталгаажуулалтын гаралтын гаралтыг `artifacts/android/attestation/` доор архивлаж, бэлэн байдлын матрицад хянасан. | `docs/source/sdk/android/key_management.md`; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`; `scripts/android_strongbox_attestation_ci.sh`. | ✅ Хэрэгжүүлсэн |
| **Бүртгүүлэх, хянах, хадгалах** | Телеметрийн редакцийн бодлого нь эмзэг өгөгдлийг хэш болгож, төхөөрөмжийн атрибутуудыг хувааж, хадгалалтыг хэрэгжүүлдэг (7/30/90/365 өдрийн цонх). Дэмжлэгийн тоглоомын дэвтэр §8-д хяналтын самбарын босгыг тодорхойлсон; `telemetry_override_log.md`-д бүртгэгдсэн дарж бичих. | `docs/source/sdk/android/telemetry_redaction.md`; `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_override_log.md`. | ✅ Хэрэгжүүлсэн |
| **Үйл ажиллагаа ба өөрчлөлтийн удирдлага** | GA таслах процедур (Дэмжих Playbook §7.2) дээр нэмэх нь `status.md` нь хувилбарын бэлэн байдлыг хянах боломжийг олгодог. `docs/source/compliance/android/eu/sbom_attestation.md`-ээр холбогдсон нотлох баримтыг (SBOM, Sigstore багцууд) гарга. | `docs/source/android_support_playbook.md`; `status.md`; `docs/source/compliance/android/eu/sbom_attestation.md`. | ✅ Хэрэгжүүлсэн |
| **Осолд хариу өгөх, мэдээлэх** | Playbook нь ноцтой байдлын матриц, SLA хариултын цонх, дагаж мөрдөх мэдэгдлийн алхмуудыг тодорхойлдог; телеметрийг дарах + эмх замбараагүй байдлын бэлтгэл нь нисгэгчдийн өмнө давтагдах чадварыг баталгаажуулдаг. | `docs/source/android_support_playbook.md` §§4–9; `docs/source/sdk/android/telemetry_chaos_checklist.md`. | ✅ Хэрэгжүүлсэн |
| **Өгөгдлийн оршин суух / нутагшуулах** | JP-ийн байршуулалтад зориулсан телеметрийн цуглуулагч нь батлагдсан Токио бүсэд ажилладаг; StrongBox баталгаажуулалтын багцуудыг бүс нутагт хадгалж, түншийн тасалбараас иш татсан. Локалчлалын төлөвлөгөө нь бета хувилбараас өмнө (AND5) Япон хэл дээр байгаа баримт бичгүүдийг баталгаажуулдаг. | `docs/source/android_support_playbook.md` §9; `docs/source/sdk/android/developer_experience_plan.md` §5; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`. | 🈺 Явж байна ( нутагшуулах ажил үргэлжилж байна) |

## Шүүмжлэгчийн тэмдэглэл

- Зохицуулалттай түншийг элсүүлэхийн өмнө Galaxy S23/S24-ийн төхөөрөмжийн матрицын оруулгуудыг шалгана уу (`s23-strongbox-a`, `s24-strongbox-a` бэлэн байдлын баримтын мөрүүдийг үзнэ үү).
- JP байршуулалт дахь телеметрийн цуглуулагчид DPIA (`docs/source/compliance/android/eu/gdpr_dpia_summary.md`)-д тодорхойлсон ижил хадгалах/давчлах логикийг мөрдүүлж байгаа эсэхийг шалгаарай.
- Банкны түншүүд энэхүү хяналтын хуудсыг хянасны дараа хөндлөнгийн аудиторуудын баталгаажуулалтыг авах.