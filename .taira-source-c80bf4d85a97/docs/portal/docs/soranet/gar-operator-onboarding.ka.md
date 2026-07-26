---
lang: ka
direction: ltr
source: docs/portal/docs/soranet/gar-operator-onboarding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 565d4e8bf0a043b2c83a03ec87a8c71a30da34f56d94a28cad03677963b3e69a
source_last_modified: "2025-12-29T18:16:35.206864+00:00"
translation_last_reviewed: 2026-02-07
title: GAR Operator Onboarding
sidebar_label: GAR Operator Onboarding
description: Checklist to activate SNNet-9 compliance policies with attestation digests and evidence capture.
translator: machine-google-reviewed
---

გამოიყენეთ ეს მოკლე SNNet-9 შესაბამისობის კონფიგურაციის გასახსნელად განმეორებადი,
აუდიტის მოსახერხებელი პროცესი. დააწყვილეთ იგი იურისდიქციის მიმოხილვასთან ისე, რომ ყველა ოპერატორი
იყენებს იგივე დაიჯესტს და მტკიცებულების განლაგებას.

## ნაბიჯები

1. **კონფიგურაციის აწყობა**
   - იმპორტი `governance/compliance/soranet_opt_outs.json`.
   - შეაერთეთ თქვენი `operator_jurisdictions` გამოქვეყნებულ ატესტაციის დიჯესტებთან
     [იურისდიქციის განხილვაში] (gar-jurisdictional-review).
2. **გადამოწმება**
   - `cargo test -p sorafs_orchestrator -- compliance_policy_parses_from_json`
   - `cargo test -p sorafs_orchestrator -- compliance_example_config_parses`
   - სურვილისამებრ: `cargo xtask soranet-privacy-report --max-suppression-ratio 0.2 --ndjson <privacy-log.ndjson>`
3. ** მტკიცებულებების აღება **
   - შეინახეთ `artifacts/soranet/compliance/<YYYYMMDD>/` ქვეშ:
     - `config.json` (საბოლოო შესაბამისობის ბლოკი)
     - `attestations.json` (URIs + დაიჯესტები)
     - ვალიდაციის ჟურნალები
     - მითითებები ხელმოწერილი PDF/Norito კონვერტებზე
4. **გააქტიურება**
   - მონიშნეთ გამოშვება (`gar-opt-out-<date>`), განაახლეთ ორკესტრის/SDK კონფიგურაციები,
     და დაადასტურეთ `compliance_*` მოვლენების გამოშვება ჟურნალებში, სადაც მოსალოდნელია.
5. ** დახურეთ **
   - წარადგინეთ მტკიცებულებათა ნაკრები მმართველობის საბჭოსთან.
   - ჩაწერეთ აქტივაციის ფანჯარა + დამმტკიცებლები GAR ჟურნალში.
   - დაგეგმეთ შემდეგი განხილვის თარიღები იურისდიქციის განხილვის ცხრილიდან.

## სწრაფი საკონტროლო სია

- [ ] `jurisdiction_opt_outs` შეესაბამება კანონიკურ კატალოგს.
- [ ] საატესტაციო დაიჯესტები ზუსტად დაკოპირებულია.
- [ ] ვალიდაციის ბრძანებები გაშვებული და დაარქივებულია.
- [ ] მტკიცებულებათა ნაკრები ინახება `artifacts/soranet/compliance/<date>/`-ში.
- [ ] Rollout tag + GAR logbook განახლდა.
- [ ] შემდეგი მიმოხილვის შეხსენებების ნაკრები.

## აგრეთვე იხილეთ

- [GAR იურისდიქციის მიმოხილვა] (gar-jurisdictional-review)
- [GAR Compliance Playbook (წყარო)] (../../../source/soranet/gar_compliance_playbook.md)