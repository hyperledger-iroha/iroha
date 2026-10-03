# KAGEMUSHA evidence gate — tests

Companion to [`kagemusha_single_design_proposal.md`](kagemusha_single_design_proposal.md),
revision 6. This file holds the parts of §10.4 of the proposal that are test
procedure: the conditions for every test (10.4.4), the tests (10.4.5), what
exists in the repository and what must be written (10.4.7), the tests the gate
does not replace (10.4.8), and the sources for the platform statements
(10.4.10). The proposal keeps what the gate is (10.4.1), what it takes as
given (10.4.2), the claims that need evidence (10.4.3), how results are read
(10.4.6) and what the gate cannot show (10.4.9). Section numbers such as §5.10
refer to the proposal.

Nothing in this file has been run. It lists tests and holds no results. Where
it says how a platform behaves, the statement comes from documentation or
source code unless it says measured.

Step numbers in the tests are the gate app's, which has no prover: 1 the
wallet is ready; 2 sign in memory; 3 create the new marker, which carries the
checkpoint; 4 commit to the journal, durably; 5 delete the previous marker and
confirm it absent; 6 release. The proposal's §5.2 has seven steps, because its
step 3 is the proof. Steps 3 to 6 here are steps 4 to 7 of §5.2.

#### 10.4.4 Conditions for every test

**Tuples.** The owner names the models and fixes the list (§10.1). The
minimum per family is one model per key-store implementation the family ships
and one per build family. Each operating-system major version a listed model
may run is its own tuple. Two phones already have records in the repository:
a Pixel 6 on Android 17, build
`google/oriole/oriole:17/CP2A.260705.006/15641320`, and an iPhone 17 Pro Max
on iOS 26.7 (`specs/kagemusha_v1_production_readiness.md:327-415`). No
HarmonyOS NEXT phone can enter the gate until a wallet design for it exists
(§10.2).

**What every record holds.** Android: manufacturer, model, build fingerprint,
operating-system version, security patch level, bootloader lock state,
verified-boot state, the KeyMint version and security levels read from an
attestation, whether Google Play services are present. iPhone: model
identifier, iOS build, whether a passcode is set, how the gate app was
distributed. Both: the hash of the gate app build, the date, the tester, the
steps taken, and the observations. Raw material (attestation chains, logs,
key-store listings, screen recordings) is kept outside the repository; the
record holds its SHA-256.

**The gate app.** It contains what can be built before Q0 fixes the relation:

- a hardware device key, on Android attested through an app attestation key
  (§10.3);
- a journal with the durable commit of §5.2;
- the marker with its checkpoint and its terms entry, and the recovery and
  resume rules, as §5.10 gives them;
- the Request, Payment and Outcome exchange with stand-in objects of the
  estimated sizes (0.85 KB, 1.1 KB and 0.2 KB), under a test scheme id, and a
  second Payment form padded to 7.6 KB to stand in for the proof;
- a stand-in issuer and ledger that issue numbered vouchers, countersign a
  Migrate, and accept a resume record at a sync;
- the carriers of §5.6.

It needs no final byte format. It shows, on a diagnostic screen, the wallet
state, the head, the checkpoint's balance, the key-store listing and the
result code of every key-store call. Its builds:

- Release build. Not debuggable and signed as a release. On Android its
  manifest carries the settings of §10.3. On iPhone it is distributed through
  the App Store, or through TestFlight under a test policy entry that admits
  that launch category, because §10.3 requires "App Store" on iOS 27. It
  uses the production App Attest environment and is opted out of Mac. Group
  a uses this build, because what a user can reach differs for a debuggable
  app.
- Permissive variant, Android only. The same code with `allowBackup="true"`,
  no extraction rules, the default rollback policy and no
  `hasFragileUserData`. It runs once with no backup agent and once with the
  agent that saves and restores nothing. It shows whether the marker holds by
  itself when a tool ignores the manifest settings, and whether the agent
  keeps a restore from clearing the app's key-store entries. Google's own
  documentation says that on some manufacturers' devices
  `allowBackup="false"` does not disable device-to-device transfer.
- Instrumented build. The same wallet code with these additions: a pause
  after each of steps 2 to 7; a pause inside the marker-creation call, after
  the request has left the app's process; a command that kills the process at
  a pause; a signal at a pause for the power-cut rig; export and import of
  the wallet's files; removal of the files; injection of a key-store error or
  an unreadable marker. It has its own package name and bundle identifier.
  Groups b, c and d use it. Its file export stands in for a restore tool. It
  tests the wallet's recovery logic. It does not show that an ordinary user
  can obtain such a copy; group a shows that.

**Forcing a power cut.** Two methods. The record says which was used.

- Forced restart by the key combination. It resets the processor. The storage
  chip stays powered and may still write out its own cache. It tests what the
  operating system had not yet handed to storage. An ordinary user can do it.
- True power cut. A spare unit is opened and its battery line is put on a
  switch; the USB cable is unplugged. It tests the storage chip's cache as
  well. An ordinary user cannot do it without opening the phone.
- Trigger. The instrumented build signals the chosen pause (a screen flash
  read by a photodiode, or an audio tone). The rig cuts power after a set
  delay from the signal.

**Number of trials.** A count of clean trials bounds a failure rate; it does
not show that the rate is zero. With n clean trials the rate is below about
3/n at 95% confidence: 100 trials bound it below 3%, 1,000 below 0.3%. For
the P2 tests of groups b and c the pass condition is zero failures, and one
failure is a fail. The reason is that a failed attempt costs an ordinary user
nothing: the wallet is simply at its true state, and the user pays an
accomplice again and tries again.

**Before any test.** The owner fixes, in writing:

- the list of tuples;
- the time target for the exchange without a proof, its percentile, its end
  points and whether it is a gate (group g);
- the number of trials for the durability tests;
- whether a true power cut on an opened phone is inside what an ordinary user
  can do;
- the patch floor that test h1 applies;
- whether P2 must hold against a compromised phone (10.4.6).

The pass conditions of 10.4.5 are fixed with them. A condition changed later
is an owner decision, recorded with the result that prompted it.

#### 10.4.5 The tests

Each test gives the property it supports, the setup, the steps, what to
record, and pass and fail. A test with no pass condition is a measurement;
10.4.6 says how its result is used. Three tests run first, because a fail in
any of them changes the design before the rest is worth running: c1 on
iPhone, d1 on Android, and h1 with h2 on each Android tuple.

**Group a — no payment from a restored or rolled-back state**

Supports P2. Its results also bear on P1: they show which user actions the
wallet survives and which end it.

Common procedure (the restore script). Every path below runs it on Android
with the release build and with both runs of the permissive variant, and once
on iPhone.

1. Install the gate app, enroll, load, and make one payment. The marker now
   holds state S0 with balance b0. Record the key-store listing, the marker's
   name and bytes, and that the device key signs a test nonce.
2. Capture. Take the backup, snapshot, clone or copy that the path offers.
3. Pay amount p to a second phone and see complete on both. The marker now
   holds S1. Record the listing again. The S0 marker must be absent.
4. Return. Apply the restore, rollback or transfer that the path offers.
5. Start the app. Record the wallet state, the head, the balance shown,
   whether the app reports a resume, the listing, the marker's bytes, whether
   the device key signs a test nonce, and which files are on the phone (those
   of S0, those of S1, or none).
6. Ask the app to pay p to a third phone. Record whether a signed SendSplit is
   produced and, if so, its sequence number, its previous digest and the
   balance it leaves.

Results.

- Kept. The wallet is ready at S1 and its files are unchanged.
- Resumed. The wallet is ready at S1 after a resume. The files were older or
  gone, the marker was untouched, and step 6 pays from S1's balance.
- Destroyed. The device key is gone, or no valid marker exists while the key
  lives. Step 6 signs nothing.
- Reset. The wallet is ready at S0 or at any state before S1, and step 6
  produces a signature. Also reset: an older marker is in the listing after
  step 4, whether or not the app starts.

Pass for P2: anything but reset, in every build. A reset on any path makes
the tuple unsupported.

Pass for P1: kept or resumed, in the release build. Destroyed is the expected
result on the paths T6 names: an erase, an uninstall that does not keep the
app's data, clearing the app's storage, and on iPhone removing or resetting
the passcode. Destroyed on any other path means that a platform or vendor
path ends the wallet although the holder kept the phone, the app and the
passcode. 10.4.6 says what follows from that. After a resume, record also
what the wallet lost with its files (§5.10 lists it).

Receiver forms. Run once per platform on one path that returns older files
(a1 on Android, a15 on iPhone).

- Second credit. The phone under test receives a payment at step 3 and pays
  it onward. Step 4 follows. The first payer presents the same Payment again.
  Pass: no second credit. The wallet returns the stored `Credited` Outcome or
  signs nothing.
- Credit after a refusal. The phone under test refuses a payment and the
  payer refunds. Step 4 returns the files from before the refusal. The payer
  presents the Payment again. Pass: no credit.
- Second refund. The phone under test pays, is refused, and folds the refund.
  Step 4 returns the files from before the refund. The phone scans the same
  `Refused` Outcome again. Pass: no second RefundFold.

Negative control, a0. Before the paths: with the release build on a locked,
unrooted phone, try to copy the app's data directly. Android: `adb shell
run-as <package>`, `adb pull` of the data directory, `adb backup`. iPhone:
download the app container with Xcode's device window or with `devicectl`.
Record each result. Expected: every attempt is refused for a release build.
Not tested. If one succeeds, it is one more path and runs the restore script.

Paths on stock Android. Run on each Pixel tuple and again on every other
Android tuple, because every vendor build inherits them.

- **a1. Package rollback.** Needs developer options and USB debugging.
  Capture: `adb install -r --enable-rollback <the same apk>`. Return: `adb
  shell pm rollback-app <package>`. Expected from AOSP source: with the
  default policy the app's data is put back and the key store is not touched,
  so the wallet resumes at S1; with `rollbackDataPolicy="retain"` the data is
  left alone and the wallet is kept. Not run on a production build.
- **a2. Backup manager.** Three runs. (i) Local transport: `adb shell bmgr
  enable true`, `adb shell bmgr transport
  com.android.localtransport/.LocalTransport`, `adb shell settings put secure
  backup_local_transport_parameters 'is_encrypted=true'`, `adb shell bmgr
  backupnow <package>` as capture; `adb shell bmgr restore <token> <package>`
  onto the installed app as return. (ii) Device-to-device test mode: `adb
  shell settings put secure backup_enable_d2d_test_mode 1` and the transport
  `com.google.android.gms/.backup.migrate.service.D2dTransport`, then the same
  capture and return. (iii) Cloud backup by Google, restored after the phone
  is reset. Expected from AOSP source: with the release build the backup is
  refused; record the tool's answer. In the permissive variant with no backup
  agent, a restore onto the installed app first clears the app's data through
  the path that also clears its key-store entries, so the result is
  destroyed. With the agent that saves and restores nothing the clear does
  not happen, and the wallet is kept. A system setting can force the clear
  for listed packages; record whether the vendor build sets it. The backup
  and test-mode commands are from Google's backup-testing page as read on
  2026-10-02; the restore command's exact form is to be confirmed from `bmgr`
  on the phone.
- **a3. Remove and reinstall.** Five runs, with step 2 empty: clear storage in
  Settings; uninstall and install, answering "do not keep" at the system's
  prompt; the same answering "keep"; uninstall keeping data with `adb shell
  pm uninstall -k <package>` and install; archive and unarchive (Android 15
  and later). Expected from AOSP source: clear storage and an uninstall that
  does not keep data remove the files and the key-store entries, so the
  result is destroyed, on a path T6 names; the three others keep both. Record
  also whether the vendor's settings app opens the wallet's own screen where
  it would offer to clear storage, and whether the prompt at uninstall
  appears. One more run: cut power during an uninstall, between the removal
  of the files and the clearing of the key store, then install again.
  Expected: resumed at S1, or destroyed; never reset.
- **a4. Second user, work profile, private space, cloned app.** Enroll in the
  main user. Create each of the others in turn and install the gate app
  there. Record whether the second instance sees the first instance's files
  or key-store entries, and whether any platform control copies an app's data
  from one to the other. Pass: the second instance has no key, no marker and
  no files of the first; it can only enroll as a new device. A second
  instance on one phone is then a second enrollment, which the caps of §8.1
  count. It is not a reset.
- **a5. Transfer to a second phone, and back.** Capture: start the setup-time
  transfer ("copy apps and data") from the phone under test to a second
  phone, by cable and by Wi-Fi. Record what arrives on the second phone. Then
  reset the first phone and transfer back. Pass: no phone ends with the
  device key of the first enrollment and an earlier state. The first phone is
  kept until it is reset.
- **a6. Operating-system update.** Install an update between step 1 and step
  3, and one between step 3 and step 5. At the first start of the app after
  each update, record `adb shell getprop vold.checkpoint_committed`. Pass:
  the wallet is kept, and the update's checkpoint was committed before the
  app could run. The second point matters because an update that fails its
  first boot rolls the files and the key-store database back together. A
  payment made before the commit would be undone with them, and the receiver
  would still hold it. AOSP commits before apps run; vendor builds were not
  read.

Paths on Samsung. Record the Smart Switch and One UI versions.

- **a7. Smart Switch.** Three runs: backup to a computer and restore to the
  same phone; backup to an SD card or USB storage and restore to the same
  phone; phone-to-phone transfer to a second Galaxy and back. Samsung's page
  says that app data moves from Galaxy devices only and that data saved in
  private storage cannot be backed up. The two statements are not reconciled,
  and nothing says what happens to key-store entries. Not tested.
- **a8. Samsung Cloud, Secure Folder, Dual Messenger, maintenance mode.**
  Samsung Cloud: back up, pay, restore. Secure Folder: add the gate app to
  it; back up and restore the folder. Dual Messenger: record whether the gate
  app can be duplicated at all. Maintenance mode: enter it, leave it, start
  the app. Pass as in a4 for the copies; kept for maintenance mode.

Paths on Xiaomi, Redmi and POCO. Run on a China build and on a global build.

- **a9. Local backup, Mi Mover, Xiaomi Cloud, dual apps, second space.** Local
  backup: Settings, back up to the phone's storage, pay, restore on the same
  phone. Xiaomi's support page lists third-party apps and their data as items
  of this backup. A third-party report says China builds back up an app
  regardless of `allowBackup`; that is a lead, not a result. Mi Mover: to a
  second phone and back. Dual apps and second space: as a4.

Paths on Huawei with EMUI or HarmonyOS up to 4.3, and on Honor, if the owner
lists such a tuple.

- **a10. HiSuite, Phone Clone, external-storage backup, Huawei or Honor cloud
  backup, App Twin, PrivateSpace.** HiSuite and external-storage backup: back
  up, pay, restore to the same phone. Phone Clone or Device Clone: to a
  second phone and back. Record whether the tool lists the gate app at all:
  Huawei's pages exclude what they call financial application data and do
  not say how an app is classed. Honor documents a restore of third-party app
  data that overwrites local data. App Twin and PrivateSpace: as a4.

Paths on Meizu, if the owner lists such a tuple.

- **a11. Flyme local backup, the Meizu transfer tool, app clone.** Local
  backup: back up, pay, restore on the same phone. Meizu's page says the
  local backup packages the current application data. Transfer tool: to a
  second Meizu phone and back. App clone, if the build has it: as a4.

For every vendor tool the three results that matter are these. The entries
are untouched: the wallet resumes. The tool clears them: destroyed, on a path
T6 does not name. The tool puts older entries back: reset. No vendor
documents which it is.

Paths on iPhone. For each, also record what happened to four things: the
files, the Secure Enclave payment key (present, and does it sign), the marker
item and the terms item, and the App Attest key (does it still produce an
assertion, and with what counter). §7.2 uses the last when it re-attests.

- **a12. Computer backup restored without erasing.** Capture: an encrypted
  backup with Finder or the Apple Devices app. Return: "Restore Backup" to
  the same phone without erasing it first. Run again with an unencrypted
  backup. Expected from Apple's documentation: the marker's class is never in
  a backup, so an older marker cannot return; other device-only keychain
  items do return on the same phone. What the restore does to the marker that
  is on the phone, and whether the payment key still signs, is not
  documented. Not tested. Pass: kept or resumed. If the marker is removed
  while the key signs, or the key no longer signs, the result is destroyed.
  One more observation in the same run: create a second Secure Enclave key
  before the capture, delete it after, and record whether the restore brings
  it back and whether it signs.
- **a13. Erase, then restore.** Capture as a12, and separately an iCloud
  backup. Return: erase all content and settings, then restore each.
  Expected: no marker; developers report that a Secure Enclave key is gone or
  no longer signs. Destroyed, on a path T6 names. Not tested.
- **a14. A second iPhone.** Capture and return: device-to-device migration at
  setup, and separately a restore of the first phone's encrypted backup onto
  the second phone. Pass: the second phone has no marker and no payment key
  that signs; the first phone is kept.
- **a15. One app's data, with a desktop tool.** Capture: back up the gate
  app's data with iMazing. Return: restore that app's data. The tool's guide
  says it does not restore the app's keychain data. Expected: the S0 files
  return, the marker still holds S1, and the wallet resumes at S1. Not
  tested.
- **a16. Offload, delete, update.** Three runs with step 2 empty: offload the
  app and reinstall it; delete the app and install it again; install a newer
  build over it. Expected: offload and update keep everything. After delete
  and reinstall the files are gone and the keychain items remain, which Apple
  staff call an implementation detail, so the wallet resumes at S1. Pass:
  kept or resumed, and never ready at an earlier state. Run on each iOS
  release on the list. If a release removes the keychain items with the app,
  the result is destroyed, and deleting the app is then an uninstall in the
  sense of T6.
- **a17. Operating-system update.** A minor and a major update between
  payments. Pass: kept.
- **a18. Passcode.** Capture a12's backup at S0, pay, remove the passcode,
  restore the backup. Pass: the wallet does not offer the old balance; it is
  stopped. Also record, with no backup involved, what each of these does to
  the marker item, the terms item and the payment key: a passcode change; a
  passcode removal; the forgotten-passcode reset; "Reset All Settings"; a
  remote passcode clear on a managed phone. Then set a passcode again and
  record whether anything returns. Apple's security guide says that if the
  passcode is "removed or reset" the items of this class become useless. No
  source read says that a plain change keeps them; the pass condition for a
  change is kept.
- **a19. Mac.** Try to install and run the release build on an Apple-silicon
  Mac. Pass: it does not install, or App Attest reports that it is not
  supported and the app refuses to enroll.

**Group b — crashes and power cuts at every step of the marker order**

Supports P1, P2 and PC. Uses the instrumented build. "Copy A" is the wallet's
files at head k, taken before object k+1. "Older or none" is the files from
any earlier head, or no files. The crash table and the restore table are
those of §5.10.

- **b1. Process kill at each step.** Setup: head k, ready. Steps: pause after
  step 2, 4, 5, 6 and 7 in turn; kill the process; start. Record state, head,
  balance, listing, and whether `open` returns an unreleased object. Pass:
  the "process killed" column of the crash table, in every row. Fail: any
  other state, a stopped wallet, or a second signature at one sequence
  number.
- **b2. Power cut at each step.** As b1, with a forced restart, and on the
  opened spare unit with a true power cut, 20 times per step. Pass: the
  "power cut" column. In addition the wallet is never stopped after a power
  cut alone. Fail: stopped, or a state earlier than the one a released object
  came from.
- **b3. Interruption, then other files.** As b1 and b2, and before the start
  import copy A; separately import an older copy; separately remove the
  files. Pass: the last two columns of the crash table. In words: once the
  new marker exists the wallet ends at k+1 whatever the files are, and it
  never ends at k after object k+1 was released. Then kill the process during
  the recovery and during the resume itself, and start again. Pass: the same
  end state. Fail: ready at k, or at any older head, after a release.
- **b4. The first three of the six sequences of §5.10.** The fourth is the
  second-credit receiver form of group a, the fifth is b9 and the sixth is
  b8. (i) Export at S0; commit an object;
  kill before the deletion; start and make a later payment; import the S0
  copy. (ii) Commit an object and kill before the deletion; export; import
  the copy from before the object and try to pay a second phone; import the
  exported copy. (iii) Receive a payment and refuse it; the payer refunds;
  import the files from before the refusal; present the same Payment. Record
  the listing and the balance after every start. Pass: after every import the
  wallet is at its true latest state, no second object exists at any
  sequence number, and in (iii) nothing is credited.
- **b5. Two signatures at one sequence number.** Steps: pay; cut power the
  moment the Payment is shown; start; pay again. 100 times with a forced
  restart, and 100 on the opened unit. Then the same with the files removed
  before the start. Record the sequence number and digest of every SendSplit
  the two receiving phones hold. Pass: no two different digests at one
  sequence number. A fail means an honest phone can pay twice from one state
  after a power cut. That breaks P2 with no attacker, and it puts a hold on
  an honest holder's row (§5.3).
- **b6. The credit survives.** Steps: A pays B; cut B's power the moment B
  shows complete; start B. Then the same on A the moment A shows complete.
  100 times each. Then repeat with B's files removed before the start. Record
  B's head and balance, whether B returns the stored Outcome, and whether B
  is ready. Pass: the credit is there every time, B is ready, and the Outcome
  can be shown again. Fail: a credit that was reported complete is missing,
  or B is stopped.
- **b7. Complete is reported last.** Steps: log, with one clock, when the new
  marker is confirmed, when the commit returns, when the previous marker is
  confirmed absent, and when the screen shows complete, over 1,000 payments
  on each phone; and kill the process at the first frame that shows complete,
  100 times. On the paying phone log also when the `Credited` Outcome is
  stored and what the screen shows before that. Pass: the receiving phone's
  report never precedes the other three; after every kill the marker holds
  the credit and the previous marker is absent; the paying phone shows "sent,
  not confirmed" until the Outcome is stored. Fail: one counter-example.
- **b8. No second object from a copy of the files.** Steps: export the files
  at each pause; search each export for a valid device signature over an
  object that had not been released when the export was taken; then import
  that export, start, and try to pay a different receiver. Record what is
  found and what the wallet signs. Expected: an export taken after step 5
  holds the signed object, and the wallet started from it is at k+1 with that
  same object, because the marker that holds the object exists. Pass: the
  wallet never signs a different object at the sequence number of an object
  found in an export. Showing the found object with other software is then
  the same payment released early, not a second one.
- **b9. A voucher is folded once.** Setup: the stand-in issuer numbers
  vouchers per device id and returns the same voucher for a repeated request
  (§7.1). Steps: load; fold the voucher; pay the amount away; remove the
  files (on iPhone, delete the app and install it again); start, so that the
  wallet resumes; go online and ask for the voucher of the same load id.
  Record what the wallet does with the voucher and the balance. Pass: it does
  not fold it; the balance is unchanged. Second form, the honest case: commit
  a load on the stand-in ledger, remove the files before the voucher is
  folded, resume, fetch the voucher. Pass: it is folded exactly once. Third
  form: present voucher n+2 before voucher n+1. Pass: refused until n+1 is
  folded. Fail in the first form means an ordinary user doubles a load.
- **b10. A countersigned Migrate is folded once.** Steps: on a successor
  phone, fold the countersigned Migrate; pay part away; remove the files;
  resume; fetch the countersigned Migrate again from the stand-in issuer.
  Pass: it is not folded again.
- **b11. Kill during marker creation.** Steps: pause inside the
  marker-creation call, after the request has left the app's process; kill
  the process; start at once; sign a different object. 1,000 times, with the
  restart delayed by 0, 10, 100 and 1,000 ms. Record, at every start, the
  markers listed, their commit counters, and the wallet state. Pass: the
  wallet is ready after every start and never stopped; recovery's own write
  and read-back precedes its listing (§5.10); where two markers with one
  commit counter are seen, the one with no journal commit is deleted and
  nothing was released from it. Fail: a stopped wallet, or two different
  released objects at one sequence number.
- **b12. Refunds beyond the checkpoint's list.** Setup: nine payments with no
  stored Outcome, so that the oldest is beyond the list of unresolved
  SendSplits the checkpoint holds in full or in short form (§5.10); the
  oldest was refused. Steps: (i) scan its `Refused` Outcome with the files
  current. Pass: one RefundFold. (ii) Export the files, delete the stored
  Outcome from the export, import it, scan the Outcome again. Pass: no
  second RefundFold. (iii) Remove the files, resume, scan the Outcome.
  Record the result: the refund is expected to be impossible, which is a
  cost §5.10 states.
- **b13. Edited files.** Steps: export the files; in the export change the
  balance, and separately remove a stored Outcome, truncate the last commit,
  and replace the block list with an older signed list; import each; start;
  pay. Then, after a resume, import the true current files again. Pass: after
  each import the wallet is at its true state and uses no edited record; with
  R6 on and an older list it neither pays nor creates a Request until it
  holds the list version its checkpoint names; with the true files back it is
  ready at the same state with its history.

**Group c — key-store writes and commits across power loss**

Supports P2 (a marker write that returned stays done) and P1 (a created
marker and a committed credit are not lost). Uses the instrumented build and
both power-cut methods.

- **c1. iPhone: pay, forced restart, delete the app, install it again.** This
  is the first iPhone test. Apple's published keychain source opens its
  database in write-ahead-log mode and sets no synchronous or full-sync
  option, and Apple's SQLite build as read on macOS then does not sync each
  transaction. The iOS build was not read. If a returned keychain write can
  be lost, an ordinary user needs no tool: the old marker is back after the
  restart, deleting the app removes the files, and the wallet resumes one
  state back. Steps: at head k pay a second phone and see complete on both;
  force a restart d seconds after the Payment was released, with d = 0, 2, 5,
  10 and 30; before opening the wallet delete the app; install it again;
  start; ask it to pay a third phone. 100 times per delay. Record which
  markers the keychain holds after the restart, the state the wallet resumes
  at, and whether a SendSplit is signed at the sequence number already used.
  Pass: the wallet resumes at k+1 every time, at every delay. Fail: one
  resume at k. Then repeat the whole test with a wait before release (step
  7), of each length at which the first run failed and of the next longer
  one. That wait is the candidate barrier of §5.10; group g charges it to the
  time of a payment. Also record one run with an app-side sync call in place
  of the wait; nothing read says it reaches the keychain's database. If no
  wait gives a pass, P2 against an ordinary user is not shown on iPhone
  (10.4.6). Receiver form, for P1: B receives and shows complete; force a
  restart on B after the same delays; delete the app on B; install it again;
  start. Pass: the credit is in B's balance after the resume, every time.
- **c2. A returned creation.** Steps: create a marker; read it back with the
  same bytes; cut power after a delay d; start; list. Delays: under 100 ms,
  1 s, 5 s, 30 s. 100 times per delay and method. Record whether the marker
  is present. Pass: present every time. While the files are intact recovery
  repairs a lost creation (§5.10), and b2 checks that it does. With the files
  removed it does not; c1 is that case.
- **c3. A returned deletion.** Steps: delete a marker; confirm it absent; cut
  power after delay d; start; list. Same delays and counts. Record whether
  the marker is back. Pass: absent every time, at every delay, with both
  methods. If only the deletion is lost, two markers exist and the one with
  the higher commit counter wins, which is safe; record that the wallet ends
  at the later state.
- **c4. Order of loss.** Steps: three payments in quick succession; cut power
  under 100 ms after the last deletion returns; start; list. 100 times.
  Record which creations and deletions survived. Pass: the surviving writes
  are always a prefix of the writes made. Fail: a later write is kept while
  an earlier one is lost. Recovery assumes this never happens (§5.10).
- **c5. A returned journal commit.** Steps: commit; cut power under 100 ms
  after the transaction returns; start. 100 times per method. Pass: the
  commit is present every time. Settings under test: SQLite
  `synchronous=FULL`, and on iPhone `fullfsync` and `checkpoint_fullfsync`.
- **c6. Endurance.** Steps: 10,000 marker creations and deletions on one
  phone per tuple. Record the time of each, the size of the key-store
  database where it can be read, and every failure. Pass: no failure and no
  step that ends the wallet. This is a P1 test: a key store that degrades
  would stop an honest wallet.
- **c7. The marker entry holds its bytes.** Steps: write, read back, list and
  delete a marker of 0.6 KB, 3 KB and 4 KB, and an entry of 7 KB and of 14 KB
  as stand-ins for the proof entry of §5.10, whose bound Q0 fixes (§2.3); on
  iPhone also 16 KB. Repeat after a reboot and after an operating-system
  update. Pass: the bytes read
  equal the bytes written, every time.

What the sources say, as a reading and not a result. AOSP's key-store
service opens its database with SQLite's default settings, which sync each
transaction before it returns. So c2 and c3 are expected to pass on a Pixel.
Vendor builds of Android were not read. On iPhone the reading is unfavourable
and c1 decides.

**Group d — what the key store returns, and what survives a settings change**

Supports P1 and P5: a wallet that reads "cannot be read now" as "gone" would
stop for good or send its holder online. It also supports P2: the delete step
relies on "definitely absent" being true.

- **d1. Android: the marker survives removal of the screen lock.** This is
  the first Android test, and it decides the marker's storage form. By AOSP
  source on the Android 12, 13 and 14 release branches, when the screen lock
  is removed the key store deletes every entry of the user except one that
  has a KeyMint key blob with no authentication requirement. An entry that
  holds only a certificate is deleted; the device key is kept. On the Android
  15 branch and later only authentication-bound entries are deleted. Not
  tested on a device. Steps: on each Android release of the list, create each
  candidate form of §5.10 beside a device key: a certificate-only entry; an
  entry that has a KeyMint blob with no authentication requirement and holds
  the checkpoint in its certificate; a key whose alias carries the
  checkpoint. Set a PIN. Then, in turn: change it; set the screen lock to
  "none"; set it to "swipe"; have a device administrator clear it; enroll and
  remove a biometric. After each, list the entries, read each back and sign
  with the device key. Pass, per release: at least one form survives every
  change with its bytes intact, together with the device key. The marker form
  for that release is one that passed. If none passes on a release, a tuple
  on that release is unsupported; the alternative, that the holder keeps a
  screen lock set, would be one more condition in T6 and is the owner's to
  rule on.
- **d2. iPhone read mapping.** Setup: an enrolled wallet with one marker.
  Steps: read, list, add and delete in the marker's keychain class, and read
  the payment key item, in four conditions: unlocked; locked, from a
  background task; after a reboot and before the first unlock; during an app
  pre-warm launch. Record every status code. Pass: an item that exists is
  never reported as `errSecItemNotFound`, and a listing never returns success
  with an item missing.
- **d3. Android read mapping.** Setup: as d2, on each Android release of the
  list, for the marker form d1 chose. Steps: the read, create, delete and
  list calls of §5.10 for a marker that exists and for one that was deleted;
  each also after a reboot before the first unlock, in a second user, and
  with the key-store service stopped or restarting (needs a debug build or a
  rooted spare unit). On Android 13 and later record `isTransientFailure` for
  each failure. Record what `containsAlias` returns in the same conditions,
  to confirm that the wallet must not use it. Pass: absence is reported only
  for an entry that does not exist; a listing that lacks the device key's
  alias is never treated as the list.
- **d4. Full storage, and a write that fails after the marker exists.**
  Steps: fill the phone's storage; ask for a payment. Then, with the
  instrumented build, make the journal write of step 5 fail after step 4
  succeeded. Record what each call returns, the state, and what is released.
  Pass: before the marker is created a failure leaves the wallet unchanged
  and nothing is signed. After the marker is created nothing is discarded:
  the wallet waits at k+1, releases nothing, and completes the commit and the
  release once space is freed (§5.2, §9).
- **d5. Lock or failure between commit and deletion.** Steps: on iPhone, lock
  the phone at the pause after step 5; on Android, inject a key-store error
  there. Record the state and whether the Payment is shown. Pass: the Payment
  is not released, nothing is deleted, and the payment completes after the
  unlock.
- **d6. Settings changes and updates.** Steps: with a funded wallet, apply
  each change and then pay. Android: the changes of d1; an operating-system
  update; an app update; the settings action that clears stored credentials.
  iPhone: change the passcode; enroll and remove Face ID or Touch ID; an
  operating-system update; an app update. Pass: ready at the same head after
  each. Separate case, iPhone: remove the passcode, and separately reset it.
  Record what the wallet reports and whether the payment key still signs.
  Expected: stopped, with nothing deleted by the wallet. This case cannot
  pass with the marker of §5.10. It is a finding about the design on every
  iPhone tuple, not about one phone (10.4.6).
- **d7. Android: the certificate parser stops accepting the marker.** Applies
  where the marker's bytes sit in a certificate. The platform parses the
  stored bytes when the app reads them and returns nothing if they do not
  parse; the parser can change with a system module update. Steps: with a
  funded wallet at head k, make every read of the current marker return
  nothing while the listing still contains its name (the instrumented build
  inserts a security provider that rejects the wrapper); start; pay.
  Separately, corrupt the bytes so that the checkpoint's checksum fails.
  Record the state. Pass: the wallet finds that the highest marker's name
  carries the commit counter and the digest prefix of its journal's head,
  continues from the files, writes a fresh marker and is ready at k (§5.10).
  With the files also removed the wallet waits where the marker is unreadable
  and is stopped where its bytes are damaged (§5.10, V4); record both. Then
  install a system module update on a spare unit and read every existing
  marker again.
  Pass: every marker still reads.
- **d8. The key store answers again.** Steps: over 30 days of ordinary use on
  one phone per tuple, with reboots, locks, low battery and low storage, log
  every key-store call that returns "unknown" and how long the wallet waits
  before the same call succeeds. Pass: every wait ends at an unlock or a
  restart. Fail: a wait that does not end. Such a wallet can neither pay nor
  unload, with nothing lost and no control switched on.

**Group e — the one-use key on TEE and on StrongBox, per vendor**

Supports P2 against a compromised phone. It is the only Android candidate
found for a limit that the secure hardware, and not the operating system,
enforces. Tests e1 to e3 and e7 run on a production phone. Tests e4 to e6
need a spare unit of the same model with root or a userdebug build, calling
the KeyMint interface below the key-store service.

- **e1. What the phone declares.** Record `FEATURE_KEYSTORE_SINGLE_USE_KEY`,
  `FEATURE_KEYSTORE_LIMITED_USE_KEY`, `FEATURE_KEYSTORE_APP_ATTEST_KEY` and
  `FEATURE_STRONGBOX_KEYSTORE`.
- **e2. TEE key limited to one use.** Steps: generate a P-256 signing key
  without the StrongBox flag, with an attestation challenge and a use limit
  of one; save the chain; sign once; sign again; reboot; sign again. Record
  whether tag 405 (the use limit) and tag 303 (rollback resistance) are in
  the hardware-enforced or the software-enforced list, and each signing
  result. The condition for hardware enforcement: tag 405 equals one in the
  hardware-enforced list at the key's own security level. This test has never
  been run on any phone; the repository's probe returns before it generates
  the key.
- **e3. StrongBox key limited to one use.** As e2 with the StrongBox flag.
  Already measured on the Pixel 6 under Android 16 and 17: tag 405 was in the
  software-enforced list
  (`specs/kagemusha_v1_production_readiness.md:342-379`). It has to be run on
  every other vendor's StrongBox.
- **e4. Two operations begun before either finishes.** Only where e2 or e3
  shows hardware enforcement. Steps: begin two signing operations on the one
  key blob; finish both. Record the number of valid signatures. Pass: one.
- **e5. Key upgrade.** Only where e2 or e3 shows hardware enforcement. Steps:
  generate the key; install a security-patch update; call the key-upgrade
  operation on the old blob twice; sign with each result and with the old
  blob. Record the number of valid signatures. Pass: one.
- **e6. Blob put back.** Only where e2 or e3 shows hardware enforcement.
  Steps: save the key blob and its key-store database row; use the key; put
  both back; sign. Pass: no signature.
- **e7. Use in a payment.** Only where e4 to e6 pass. Steps: with the phone
  in airplane mode for at least seven days, generate one-use keys in a loop,
  each attested by the app attestation key; 1,000 cycles. Record whether
  generation succeeds without a network, the time per key, every failure, and
  how many such keys the phone can hold at once and what happens to other
  keys when that number is reached. Also cut power between the end of a
  signing operation and its return, 100 times, and record whether the key is
  used up with no signature delivered. That last result is a P1 hazard: a
  signature lost that way would leave a balance out of reach.

Reaching the spare-unit tests. On a Pixel a spare unit can be unlocked and
rooted, or run a userdebug build made from AOSP. Unlocking changes the root
of trust, and keys made before it become unusable, so the test makes its keys
afterwards. Whether a vendor's secure hardware behaves the same on an
unlocked phone is an open point to record per vendor. For Samsung, whether
StrongBox still works after the bootloader is unlocked is not known here. For
Huawei and Meizu no route to a rooted current phone is known here. These
three statements were not checked against a primary source. Where e4 to e6
cannot be run, the finding is "not shown", and 10.4.6 treats it as no
hardware enforcement.

**Group f — the iPhone assertion counter**

Supports P2 against a compromised phone, only if payments were co-signed
with the App Attest key, which is an option outside the base design (§2.1).
It also records what the second anchor that §5.10 considers for a removed
passcode would need. Apple documents the counter as the number of assertions
the key has signed and asks a server to check that it grows. Apple does not
say where it is kept or that it steps by one.

- **f1. The step.** Setup: a production-environment build with one attested
  key. Steps: 1,000 assertions in a row; then 2, 4 and 8 calls at once; then
  calls that are cancelled or whose process is killed before the result
  returns. Record every counter value, the time per call and the size of each
  assertion. Pass: each returned assertion carries the previous value plus
  one, simultaneous calls get distinct consecutive values, and the record
  states whether an unreturned call uses up a value. The repository's one
  record shows the values 0, 1 and 2 on one iPhone, and that a discarded
  assertion still advanced the counter.
- **f2. Persistence.** Steps: read the counter by one assertion before and
  after each of: app relaunch; reboot; a minor and a major operating-system
  update; an app update; seven days in airplane mode; storage nearly full; a
  passcode change; a passcode removal. Record the two values, whether the key
  still exists, and whether the call worked offline. Pass: plus one each
  time, and the call needs no network.
- **f3. Across restore paths.** Steps: in each iPhone path of group a, read
  the counter before the capture and after the return. Pass: no path leaves
  the key valid with a lower or a repeated counter. Apple documents that the
  key does not survive reinstall or restore; Apple engineers have said that
  this loss is to be fixed, so the result is recorded per iOS release.
- **f4. Forgery with operating-system privilege.** (i) On a jailbroken device
  of the class that a boot-ROM exploit reaches, at the newest operating
  system it runs: hook the system service that builds the assertion, as the
  public `aaoracled` code does, and try to produce two assertions with the
  same counter, and one with a chosen counter, that both verify under the
  attested key by Apple's documented steps. (ii) The same on a current iPhone
  through Apple's Security Research Device, if the project can obtain one.
  Record whether each forged assertion verifies. A result on the old device
  shows how that operating-system version builds the assertion. It does not
  show what a current iPhone does. Only (ii) can show that the counter cannot
  be forged on current hardware. Apple's page for the research device, as
  read through a fetch tool on 2026-10-02, requires a proven record of
  finding security issues and says the device is for security research only;
  whether the project qualifies, and whether its terms allow this test, is
  not established. One limit of (i): search results say the public jailbreak
  for the newest chip of that class needs the passcode off. The marker's
  keychain class cannot exist on such a device, so (i) covers App Attest
  only.

**Group g — the time of one complete exchange, without a proof**

Supports PC, and bears on the owner's wish that "1-2 s should be good ux".
PC sets no time limit by itself; it says that everything needed for P1 to P4
finishes before the report. A proof-carrying payment cannot be timed: no
prover exists (§2.2). What this group measures is the time every payment
spends with no proof at all. A proof-carrying payment takes that time, plus
the payer's proving, the transfer of the larger Payment, the receiver's
verification and the receiver's proving (§2.3). So a pass here is necessary
for the timing wish and far from sufficient.

- **g1. Parts.** On each phone, at least 100 times each, as median, 95th
  percentile and maximum: one device-key signature (TEE key, StrongBox key,
  Secure Enclave key); marker creation, in each form that passed d1, at
  0.6 KB and 3 KB; marker deletion; a confirming read; a listing with 1, 2
  and 10 entries; one durable commit; one signature verification; the
  start-time check with 100, 10,000 and 100,000 Requests on record. Record
  cold (first use after start) and warm values. Also one device-key signature
  over a caller-supplied 32-byte value that is not a SHA-2 output: on Android
  with a key authorized to sign without a digest, in the TEE and in
  StrongBox; on iPhone with the Secure Enclave's digest call. Record whether
  it works and its time. Q0 needs this to fix the digest authorization of the
  device key (§2.3).
- **g2. Carriers.** For every ordered pair of phones and each carrier of
  §5.6: the time from first frame or tap to complete decode for a 0.85 KB
  Request, a 1.1 KB Payment, a 7.6 KB Payment and a 0.2 KB Outcome. QR at 5,
  8 and 12 frames per second, counting second passes and the time to aim.
  Also the largest still code a phone reads reliably from another phone's
  screen at arm's length.
- **g3. The whole exchange.** Setup: two phones, radios off except the
  carrier, both wallets ready, the Request already shown. The receiver
  interval: from the payer's confirmation to the receiving wallet showing
  complete. The payer interval: from the payer's confirmation to the paying
  wallet showing complete, which includes the Outcome crossing back. Every
  ordered pair of phones, every carrier, at least 100 exchanges each, once
  with the 1.1 KB Payment and once with the 7.6 KB one; then again in
  low-power mode, and with a warm phone after ten minutes of continuous
  payments. Record median, 95th percentile and maximum of both intervals,
  and the share of each part from g1.
  Time the platform's authentication prompt of §5.9 separately; it comes
  before the payer's confirmation.
  Pass: the owner's target on the receiver interval, and on the payer
  interval if the owner includes it. By
  arithmetic, not measurement: at the QR widget's default of 5 frames per
  second the 1.1 KB Payment alone takes about 1.8 s to cross, and the 7.6 KB
  one about 10 s (§5.6).
- **g4. With added steps.** Only where the design adopts one: the wait before
  release that c1 found for iPhone; a fresh attested leaf per payment (h2);
  an assertion per payment (h8); a one-use key per payment (e7). Repeat g3
  with each. Pass: the same target.
- **g5. Computing in the background.** A measurement, used only by the
  candidate shapes that §2.3 lists for shortening a proof-carrying payment.
  Steps: start a stand-in computation sized to the proving and memory targets
  after a payment; record whether it completes with the app in the
  background, with the screen locked, in low-power mode and under memory
  pressure, and after how long the platform suspends or ends it.

**Group h — what attestation shows, and what an enrollment statement can say**

Supports P2, and gives Q0 the facts that the relation of §2.1 is fixed from.
It records the evidence the issuer and the validators hold for T1 to T3 at
enrollment and at renewal, what E can state for the tuple, and what a
receiver can check on the last hop.

- **h1. Android: the enrollment chain.** Steps: generate the app attestation
  key and then the device key as §10.3 describes, once in the TEE and once in
  StrongBox, with device-properties attestation requested; save every
  certificate; decode and verify them off the phone with the repository's
  verifier and against Google's revocation list. Record:
  - the root the chain ends at, and whether the chain is factory-provisioned
    or remotely provisioned; the number of certificates, the key type and
    signature algorithm at each level, the validity dates, and the sizes;
  - for the app attestation key: the purpose set and the list it is in; the
    origin; both security levels;
  - for the device key's leaf: its size; who signed it; the attestation
    version; purposes and digests; that no use limit, no
    user-authentication requirement and no unlocked-device requirement is
    present;
  - the root-of-trust fields (boot key, lock state, boot state, boot hash),
    whether each is in the hardware-enforced list, and whether the boot key
    is non-zero and the same after a reboot and after an update;
  - the OS version and the system, vendor and boot patch levels, and the list
    each is in;
  - the app identity (package, version, signing-certificate digest) and the
    list it is in;
  - brand and model, if attested.

  Pass: the chain ends at a pinned Google root; both security levels are
  hardware; the phone is locked and verified; the patch levels meet the
  floor; the app identity is the release build's; the origin is generated;
  the app attestation key's hardware-enforced purpose is "attest key" alone;
  the root of trust and the patch levels are in the hardware-enforced list.
  The record states, per tuple, which fields of E exist and which the secure
  hardware enforces. The repository's Pixel 6 record already shows a factory
  chain under Google's first root for StrongBox, which the current verifier
  does not admit (§10.3).
- **h2. Android: a leaf under the app attestation key, with no network.**
  This is what a renewal carries (§7.1). Steps: put the phone in airplane
  mode; generate a fresh key attested by the app attestation key, with a
  chosen challenge; save and decode the leaf. Repeat after 7, 30 and 90 days
  without a network, which is past the life of remotely provisioned
  certificates in Google's test chains (9 to 29 days; about 70 for the
  intermediate). In the same state try a key attested by the system's key and
  record the error. Then install an operating-system update, reboot, and
  generate again. Record: whether each generation succeeds; the leaf's size
  (estimate 0.6 to 0.8 KB); whether it carries the root of trust and all
  four version fields with current values; whether the values change after
  the update; the time per generation as median and 95th percentile, in the
  TEE and in StrongBox, cold and warm. Pass: the app attestation key signs a
  full leaf at every interval, and the leaf after the update shows the new
  patch levels. A tuple that fails has no fresh hardware statement at
  renewal; §10.3 gives the fallback and what it does not show. Also confirm
  that a work profile, a second user and a cloned app each need their own
  app attestation key.
- **h3. Android negatives.** On spare units: an unlocked bootloader; a
  relocked phone with a user-installed boot key; an emulator; the app rebuilt
  and signed with another key; a key imported and not generated; a device key
  attested by a key whose purposes include signing. Pass: the verifier
  rejects each from a field of the chain. Record which field.
- **h4. Android: a takeover after boot changes nothing.** A record, with no
  pass condition. Steps: on a locked spare unit with an old build and a
  public, already-patched privilege-escalation exploit, take over the
  operating system; from the attacker's code generate a leaf under the app
  attestation key and sign with the device key. Record every field of the
  leaf. Expected from the documentation: locked, verified, the same patch
  levels, the genuine app identity. The record shows, on a device, what §2.1
  says E and the proof do not show.
- **h5. Android: key binding to the boot state.** A record, with no pass
  condition. The KeyMint interface text says a key must be unusable when the
  boot key or the lock state changes, and after a return to an older release.
  Steps: enroll on a locked spare unit; unlock the bootloader; relock; at
  each stage read the device key and try to sign. Unlocking erases the phone
  on the phones known here, so on a production unit the binding cannot be
  seen; record whether anything of the wallet survives. The binding is not
  an assumption of the design. Where it holds it is a bound on a compromised
  phone, and §3.2 uses the record.
- **h6. iPhone: the attestation at enrollment.** Setup: the release build,
  production environment. Steps: attest; save the attestation object and the
  receipt; verify with the repository's verifier. Record every field of the
  authenticator data, the iOS 27 extension fields (launch category and bundle
  version), and every extension of the leaf certificate, documented or not.
  Repeat for an App Store build, a TestFlight build, an ad hoc build and a
  development build. Pass: the documented fields verify, and the verifier
  tells the four builds apart on iOS 27. Expected and already documented: the
  attestation carries no operating-system version, patch level, boot state or
  device model, and nothing attests the payment key. So on iPhone E states
  the App Attest facts and nothing about the operating system. No test
  changes that.
- **h7. iPhone negatives.** (i) A development-environment attestation and a
  Mac are rejected. (ii) On the jailbroken device of f4: obtain a production
  attestation for the gate app's App ID whose nonce covers a software key,
  and submit it to the repository's enrollment verifier. Record whether it is
  accepted, and what the extension fields hold. Apple describes those fields
  as collected on the device. If the verifier accepts it, the issuer cannot
  tell an old jailbroken device from a current iPhone, and iPhone enrollment
  gives no evidence against a compromised phone at all.
- **h8. iPhone: an assertion with no network.** This is what a renewal
  carries on iPhone, and what an optional per-hop assertion would be. Steps:
  in airplane mode, after 7 and 30 days offline, after a reboot and before
  the first unlock, generate an assertion with the enrolled App Attest key.
  Record whether it succeeds, its size with and without the iOS 27 fields
  (about 140 B on iOS 26.7 in the repository's record; estimate 185 to 205 B
  with the fields), and its time. What a verifier learns from it: the holder
  of the attested key signed these bytes, and a counter. It shows no
  operating-system state.
- **h9. Other platform classes, if the owner lists such a tuple.** A Huawei
  phone after 2019 on EMUI or HarmonyOS 4: what the Android attestation call
  returns, and under which root. A mainland build of Xiaomi, OPPO, vivo,
  Honor or Meizu, one from a generation with factory keys and one launched
  with Android 16: h1 and h2 with Google's services app disabled and
  enabled. A HarmonyOS NEXT phone: capture an anonymous attestation chain;
  record its algorithms and sizes, every field present, who fills each, and
  how its offline form behaves after a month without a network.
- **h10. Reach.** One enrollment on each platform from a mainland-China
  network. Record whether Apple's attestation call succeeds, and whether an
  Android phone can generate a key under the system's attestation key.

**Group i — onward spending and running with no network**

Supports P1, P3, P5 and PC. Uses the release build with the stand-in issuer
and ledger.

- **i1. Onward at once.** Steps: A pays B; the moment B shows complete, B
  pays C the whole amount, radios off. 100 times; again after B is power-cut
  and restarted; again after B's files are removed and B resumes; again after
  B has been switched off for 30 days. Pass: C credits every time. Fail: B
  needs any step before it can pay.
- **i2. No network.** Steps: run the exchange with both phones in airplane
  mode. Then with the network on, record the app's traffic during an exchange
  and for 24 hours after it (per-app counters on Android; a packet capture on
  iPhone). Pass: the exchange completes in airplane mode, and with the
  network on the app sends nothing until the user starts a sync.
- **i3. Many hops.** Setup: at least four phones of mixed platforms; one
  loads; none syncs again. Steps: move the value around the ring for at least
  50 hops; the last holder then unloads at the stand-in ledger. Pass: every
  hop completes and the unload is recorded for the full amount. Record the
  Payment's size at hop 1 and hop 50; it must not grow.
- **i4. With the proof.** Cannot be run. Conditions for Q2, fixed here so
  that they stand before any prover is measured: the receiving wallet reports
  complete only after its own proof is made; i1 passes with that rule on
  every supported tuple; no wallet commits a transition it has not proven;
  and after a resume the wallet can still make its next proof, which needs
  the proof entry that §5.10 describes to hold what the prover needs.
- **i5. Every control off.** Setup: a certificate with no lease, no limits
  and no receive freshness, and no block list. Steps: with no network, pay
  and receive after each of: ten reboots; the clock set back a day and a
  year, and forward; a time-zone change; 30 and 90 days idle; an
  operating-system update installed and the network removed again; an app
  update; nearly full storage; the SIM removed; the battery run down to
  shutdown; a resume after the files were removed; on iPhone, the App Attest
  key made invalid; and after receiving from a peer each notice of §5.11: a
  planned key rotation, a key revocation, a new rules version, a tightened
  row for another tier, a scheme closure. Pass: the wallet pays and receives
  after each and never asks for the network. Fail: any event after which the
  wallet needs the network. Each fail names a rule that breaks P5.
- **i6. One control on.** Steps: switch on the lease, the limits, receive
  freshness and the block list, one at a time, and reach each one's limit.
  With the block list on, also resume after the files were removed. Pass:
  only what that control governs stops, and the wallet names the control.

**Group j — later misconduct by the payer**

Supports P4. Uses the release build with the stand-in issuer and ledger.

- **j1. The receiver learns.** Steps: A pays B. B's wallet then receives,
  once from a peer and once at a sync: a block entry for A's device; two
  conflicting transitions signed by A; a revocation of the issuer key that
  certified A. After each, read B's balance and have B pay C the whole amount
  offline. Pass: the balance is unchanged and C credits. Fail: B's wallet
  reduces, holds or flags the credit.
- **j2. The next receiver already knows.** Steps: give C all three items
  first; then B pays C value that came from A. Pass: C credits. C's checks on
  the last hop see nothing of A. With the proof, the history contains A's
  key; the test cannot be run until a prover exists, and the condition for
  Q0 is that such a proof stays accepted (§2.1, "Policy inputs").

#### 10.4.7 What exists in the repository and what must be written

Read on 2026-10-02 at commit `b2a3cd05bc` with uncommitted edits in the tree.
Nothing was built or run.

Reusable as it is or with a small change.

- `kotlin/client-android/src/main/java/org/hyperledger/iroha/sdk/offline/probe/AndroidKeyMintSingleUseProbeV1.kt`.
  It generates a P-256 signing key with a challenge and a use limit of one and
  no StrongBox flag, signs twice and returns the chain. That is test e2. It
  returns at lines 79-81 when neither feature flag is declared, so it has never
  generated the key on the Pixel 6. Change: record the flags and continue. It
  also uses `containsAlias` (lines 166 and 208), which the wallet's marker code
  must not copy.
- `kotlin/client-android/src/androidTest/java/org/hyperledger/iroha/sdk/offline/probe/AndroidKeyMintSingleUseDeviceTest.kt`.
  Its `strongBoxOneUseWithoutFeatureFlagDiagnostic` is test e3 and carries no
  Pixel check, so it runs on any vendor. Its `attestationFacts` parser reads
  tags 303 and 405 from both lists; e1 to e3 and h1 reuse it. Its three restart
  stages are tied to the Pixel 6 (`requirePixel6RestartProbe`) and must be
  freed from that check for other vendors.
- `KeyMintRestartDiagnosticV1.kt` in the same probe directory: the order of
  checks for a used-up key after a reboot. Reused by e2 and e3.
- `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/crypto/keystore/attestation/`
  and the `iroha-attestation` command in `kotlin/tools`: verification of a
  saved Android chain off the phone. Reused by h1 to h3 with the changes of
  §10.3. The chain through an app attestation key is new to it.
- `kotlin/client-android/src/main/java/org/hyperledger/iroha/sdk/crypto/keystore/KagemushaAndroidHardwareAppKeyStoreV1.kt`:
  generation of the hardware device key (SHA-256 only, StrongBox preferred, no
  user-authentication requirement). Reused by the gate app for the device key,
  with the app attestation key added. Not reused: its alias lookups at lines
  61, 81 and 164.
- `examples/ios/KagemushaAppAttestProbe/`: one attestation and two
  assertions, with the raw objects saved and the counter parsed. It is the
  start of f1, h6 and h8. Changes: a loop of 1,000 assertions, simultaneous
  calls, a production-environment entitlement, and a run mode that reads the
  counter once for f2 and f3.
- `python/iroha_app_attestation/src/iroha_app_attestation/verify_physical_apple.py`
  and `attestation.py`: verification off the phone of a raw Apple attestation,
  its receipt and two assertions with consecutive counters; and of an Android
  chain. Reused by f1, h6 and h7. Change: any number of assertions.
- `IrohaSwift/Sources/IrohaSwift/KagemushaAttested/KagemushaAttestedDatabase.swift`:
  a SQLite connection with `synchronous=FULL`, `fullfsync` and
  `checkpoint_fullfsync` (lines 53-54). Reused for the journal on iPhone; c5
  tests it.
- `IrohaSwift/Sources/IrohaSwift/KagemushaAttested/KagemushaAttestedHardware.swift`:
  creation and use of the Secure Enclave payment key. Reused for the key. Not
  reused: its reading of every keychain error as "no key" (lines 67-78).
- The carriers: `IrohaPeerQRV1`, `IrohaPeerNfcV1` and `IrohaPeerNearbyV1` in
  Swift and Kotlin, and the QR widget. Reused by g2 and g3.
- The device records in `specs/kagemusha_v1_production_readiness.md:327-415`.
  They already answer e1 and e3 for the Pixel 6 (flags false, tag 405
  software-enforced, on Android 16 and 17), record that the Pixel 6 StrongBox
  chain is a factory chain under Google's first root, and give a two-step
  counter sample for one iPhone on iOS 26.7 in the development environment.
  They are entered in the gate's record as prior results with their dates.

Useful as a pattern only.

- `AndroidKeyMintOneUseSelectionCandidateV1.kt` and
  `AndroidPixel6TestnetStrongBoxObservationV1.kt`. They prepare an attested
  one-use key, sign once and keep a durable record so that a lost result is
  never retried. They are bound to Recursive V1's selection frame and, the
  second, to the Pixel 6. Test e7 reuses the pattern.
- `specs/kagemusha_v1_physical_evidence.md` and
  `scripts/verify_kagemusha_v1_physical_device.py`. They define a signed,
  chained transcript with restart, power-loss and clock-rollback cycles, each
  ending on a boot identifier not seen before. That idea is reused for the
  gate's records. The checker itself is not: it requires an OEM attestation
  report, a hardware profile and the sender-admission cases of a qualified
  hardware service, none of which a stock phone has.
- The other probes in the directory (`KagemushaTestnet*.kt`,
  `Pixel6Testnet*.kt`) are testnet diagnostics and have no use in the gate.

Must be written.

- The gate app for Android and iPhone, in its release, permissive and
  instrumented builds (10.4.4). This is the largest item. Its marker,
  checkpoint, commit and recovery code is the code §9 specifies for the
  wallet core, so writing it in the core's crate avoids writing it twice.
- The Android enrollment through an app attestation key, and a decoder that
  lists every field of a key description with the list it sits in, for h1 and
  h2. No code in the repository creates such a key.
- A key-store status probe for d2 and d3 that records the exact result of
  every call in every lock state, and the three marker forms for d1.
- A caller for the KeyMint interface below the key-store service, for e4 to
  e6 on a rooted or userdebug spare unit.
- The hook for f4 and h7 on a jailbroken device. Public third-party code
  exists (`aaoracled`); it was read, not run.
- The power-cut rig and its trigger.
- The timing harness for group g.
- A written checklist per vendor path of group a, with the tool versions.
- The record format and a checker for it.
- The gate app's manifests. The Android example manifests set
  `allowBackup="true"` and no rollback policy (§10.3).

No effort estimate exists for the gate.

#### 10.4.8 Tests the gate does not replace

The gate tests the wallet's own state, the key store, timing without a proof
and attestation. The tests below cover the rest of the design: the exchange
rules, time, limits, the block list, versions, fees, the ledger, the sync
flows, Migrate and renewal. None has been run. They run on the wallet core
and its shells, not on the gate app, and each must pass before a support
claim for a tuple (§11).

Exchange (§5.2).

- Two phones: present a Payment to a receiver after its Request has expired,
  after the receiver has rebooted, and after a long idle period; the receiver
  must answer `Refused` each time and the payer must be able to fold the
  refund. Present a credited Payment again after the same three events; the
  answer must be `Credited` each time. Then the same after the receiver's
  files were removed and it resumed: for a Request or a decision that its
  marker still holds the answer must be the same; for any other the receiver
  must sign nothing (§5.10).
- The paying wallet's display: "sent, not confirmed" from the release until a
  `Credited` Outcome is stored; "returned" after the RefundFold commit; never
  "complete" on a timeout.

Ledger and fees (§5.8, §6, §8).

- Per platform: time a renewal end to end, from the sync request to the
  committed Recertify, including the fresh leaf under the app attestation key
  on Android or the assertion on iPhone, the issuer's check and the
  validators' check of it (§7.1), and the wait for the serial anchor to be
  final on the ledger. Run it in
  the foreground and in the background. Record how often it does not finish
  in one attempt.
- Per platform: with a fee policy in the certificate, measure the Request and
  Payment sizes and the QR frame count and time per pass (estimates:
  certificate +70 B, SendSplit +65 B).
- Per platform: a refused payment with a fee. Kill the app, and separately
  force a power-off, between scanning the `Refused` Outcome and the RefundFold
  commit. After restart the balance must be back by the amount plus the fee
  exactly once.
- Fee formula agreement: run the same boundary vectors (amount 0, 1, the
  amount at which the ceiling binds, the largest amount, a rate that gives a
  fractional result) through the iPhone build, the Android build and the
  node. All three must give the same fee, and each SendSplit must carry the
  `fee_policy_id` of the record in the payer's certificate.
- Per platform: sign a RedeemSplit and kill the app, and separately force a
  power-off, before the ledger accepts it. On restart the wallet must list it
  and present it again, and the ledger must pay the increase once. The same
  after the files were removed and the wallet resumed.
- Per platform: unload with an expired lease, in the clock-reset state, after
  a key-revocation notice, and with a stale block list. The wallet must sign
  the RedeemSplit and the ledger must record it in each case.
- Per platform: before an unload is signed, the wallet shows what §7.1 says
  it shows, read from the registry row. With the ledger unreachable it must
  sign nothing.
- Registration: the validators reject each negative of h3 and h7 (i) from the
  raw attestation, with the same result on every validator, and seal a row
  only for a chain that the issuer's verifier also accepts.
- Later misconduct, on the ledger and in the issuer service: with evidence
  against A accepted, A's row under a hold, A blocked, and the key that
  certified A revoked, an unload by B and a renewal for B proceed exactly as
  they would otherwise. A hold is placed only on two conflicting signatures
  by one device key, verified on-chain, and only on that key's row or the
  row that took over its balance by Migrate.
- If the head anchor uses a separate device-signed sync statement (§6): time
  that extra hardware signature per sync on each platform.

Sync, resume, Migrate and renewal (§5.9, §5.10, §7).

- A sync after a resume. The issuer accepts the resume record when the wallet
  made transitions since its last sync, and when it made none. A renewal and
  a Migrate after a resume are accepted on the same record.
- iPhone: delete the app and install it again with (i) nothing signed since
  the last sync and (ii) several payments since the last sync. In both the
  wallet must resume offline at its current balance and pay. Record whether
  the payment key item, the marker item and the terms item survive app
  deletion on each iOS release on the list.
- iPhone: remove the passcode, and separately reset it, with a funded wallet.
  The wallet must report that it is stopped, keep the key and the files, and
  delete nothing. Set a passcode again and confirm that nothing changes.
- Per platform: read the device key, the marker and the journal with the
  phone locked, after a reboot before the first unlock, and on Android with
  the key-store service failing or restarted. The wallet must report that it
  is waiting, sign nothing, delete nothing, and continue unchanged after the
  unlock.
- Enrollment on a phone that still holds an earlier wallet's key and marker:
  the app offers to resume that wallet, and enrolls a new one only after a
  declared loss. Markers of the earlier wallet are left in place while its
  key is on the phone.
- Prompted use: time the platform prompt (BiometricPrompt, LAContext) from
  display to success on each tuple; confirm that a background renewal and a
  receive complete with no prompt; confirm the behaviour with no screen lock
  set under both tier settings (plain confirmation, or refuse to pay until a
  lock is set).
- Migrate interrupted at every step (kill and forced power-off): after the
  pre-check; after the Migrate is committed and before upload; after upload
  and before the countersignature; after the countersignature and before the
  old key is deleted; before MigrateFold. The balance must never be in two
  places: it is in the old wallet until the Migrate commits, in neither
  wallet from then until the MigrateFold commits (§7.2), and in the new
  wallet after that. Nothing may be destroyed, and each case must resume.
  Also: the issuer refuses at the pre-check and the old wallet must remain
  fully usable.
- Migrate and open Requests: the Migrate carries only the Requests the old
  wallet can show are undecided, and the successor answers `Refused` for
  those only. A Payment that the old wallet credited, presented to the
  successor, must not be refused.
- Migrate and limits: spend part of the day and month limit, Migrate, and
  confirm the new wallet's remaining allowance equals the old one's; repeat
  with the new phone's clock set one day back and one month back.
- Re-enrollment and limits: declare a loss with limits and a lease on;
  confirm the new certificate carries none of the old share until the end of
  the UTC day and month in which the old lease plus grace ends, and the full
  share afterwards.
- Declared loss: confirm the old key and files are untouched until the
  retirement is final on-chain and the user confirms a second time; then
  bring back the phone that was declared lost and confirm that it can unload
  its whole balance, and that it still pays and is paid offline unless the
  scheme switched on blocking at the holder's request (§7.2).
- Renewal with a lost response: repeat the request and confirm the same
  certificate (same serial) is returned; lose the final acknowledgment of the
  Recertify and confirm the next sync brings the acknowledged head up to
  date. A renewal is refused only for an enabled regulatory control or
  because the device's own key signed two successors; confirm each other
  cause is not a refusal.
- iPhone App Attest: provoke `DCError.invalidKey` and `serverUnavailable`
  separately; the wallet must ask for re-attestation only on the former, and
  only while a valid marker is present.

Time, limits, block list, versions (§5.4, §5.5, §5.11).

- Every test that §5.4 lists under "Required tests, per platform" and that
  is not repeated below: the clock, reboot and anchor cases; the opening
  counters after a renewal that carried a resume record; and a receiver's
  files put back inside one window, with the tally recorded under each of
  the two forms of §5.4.
- Each row of the worked-cases table in §5.4 on Android and iPhone: record
  the signed time, the window used and the floor after each step.
- A Request dated just under and just over `window_future_tolerance` ahead of
  the payer's clock: paid and refused respectively, with the payer's floor
  checked after each.
- Twelve payments in a row to receivers each dated `W` ahead of the payer's
  time: the payer's floor never passes its clock reading plus `W`.
- A payment just before and just after UTC midnight and a month end: the
  payer's counters and the receiver's tally are in the window of the signed
  time; the receiver refuses above one limit per window.
- A wallet in the clock-reset state under a certificate with no
  time-dependent control pays and creates marked Requests; a payer with
  limits refuses the marked Request with nothing signed; a payer with none
  pays it.
- Date set ahead, one Request created, date corrected: the wallet enters the
  clock-reset state; record when sending resumes without a sync and that a
  sync with Recertify resumes it at once.
- The date check: a wallet unanchored with the clock more than 30 days ahead
  of its floor prompts once, then signs.
- Opening counters: after a renewal, after a Recertify with the floor ahead
  of issuer time, after a Migrate, and after re-enrollment following a
  declared loss, the first payment starts from the certificate's opening
  values.
- The time records after a resume: a wallet whose files were put back to an
  older copy must not open an earlier limit window.
- Under `require_anchor` on iPhone: how often the shell reports a reboot when
  none happened, over a week of normal use.
- Clock error after long power-off and after months without network time,
  per tuple: the measured disagreement between phones sets
  `window_future_tolerance`.
- Versions: the oldest released build and the newest pay each other in both
  directions, because every release accepts every rules version ever allowed;
  a SendSplit under a version outside the Request's range is answered with a
  signed `Refused` and refunded; an object with an unknown critical extension
  is not paid.
- Key revocation by peer relay: a wallet that holds a revocation of the key
  that signed its own certificate still pays and requests; a receiver that
  holds the revocation accepts that certificate until its own expiry, judged
  after the stricter-of rule of §5.11.
- Tier-row notice by peer relay: a receiver holding a stricter row refuses a
  payer whose counters exceed it, attaches the notice to the Outcome, and the
  payer refunds and applies the row to its next payment.
- Root succession by peer relay: a phone holding only the old root receives
  the succession notice and issuer key certificate from the other phone,
  verifies the chain and completes a payment in the same meeting. Record the
  extra scans and time.
- Request with key notices attached: frame count and scan time against the
  plain Request.
- Block list: an entry covering a certificate serial; renewal above the
  serial after the ledger clears the flags; a wallet holding an entry on
  itself creates no Request or does not pay; lookup time and storage with
  10,000 entries; a Payment carrying as many entries as the size bound
  allows.

One design (§11).

- Profile isolation, on each platform, over QR and over NFC where the pair
  supports it: show a wallet build that holds value of this design a V1
  profile-1 Request, Payment and Acknowledgement, and an attested-suite
  message under profile code 2 and under the text prefix `kga1:`. Each must
  be rejected before the payload is decoded, with nothing committed and
  nothing signed. Repeat in reverse with a V1 build shown this design's
  messages.
- Before any removal of the device probes (§11.1 row 7): record the results
  they produced, per tuple, with the build fingerprint, so the measurement
  outlives the code.

Carriers (§5.6).

- NFC: time to move 1.1 KB and 7.6 KB over ISO 7816 commands Android to
  Android and iPhone (reader) to Android (card); confirm an iPhone app reads
  an Android phone's card emulation; if tap from Android to iPhone outside
  the EEA is wanted, test the reversed flow (Android payer as card, iPhone
  receiver as reader).
- Bluetooth LE, only if the owner wants the carrier: on iPhone to iPhone,
  iPhone and Android in both role assignments, Android to Android, and one
  Android phone without Google Play services, confirm a store app can
  advertise a service, connect and move 1.1 KB, 7.6 KB and 10 KB; record
  connection setup time and transfer time; record on each Android tuple
  whether `getBluetoothLeAdvertiser` returns an advertiser; iPhone app in the
  foreground.
- Google Nearby at the pinned revision: whether an iPhone and an Android
  phone connect with no common network (the README says Wi-Fi LAN only on
  Apple platforms; the checkout contains Apple Bluetooth code).

#### 10.4.10 Sources for the platform statements in this section

Read on 2026-10-02 by automated passes, most through a fetch tool that
returns a summary with quotations. Each should be read again in full before
it is relied on. The evidence appendix (§13) has the older sources.

- https://developer.android.com/identity/data/testingbackup: the `bmgr` and
  `settings` command lines quoted in a2. The `bmgr` reference page returned
  no body, so the restore command is not confirmed from it.
- https://developer.android.com/identity/data/autobackup: that on some
  manufacturers' devices `allowBackup="false"` does not disable
  device-to-device transfer; the three sections of the extraction rules.
- AOSP `keystore2` on the android12, android13 and android14 release
  branches (`database.rs`, `super_key.rs`): what removal of the screen lock
  deletes (d1). On the android15 branch, `maintenance.rs`.
- AOSP `frameworks/base`: `AndroidKeyStoreSpi.java` (a stored certificate is
  parsed on read, and nothing is returned if it does not parse);
  `FullRestoreEngine.java` (an app with no backup agent is cleared before a
  full restore); `RemovePackageHelper.java` (an uninstall that keeps data
  returns before the key store is cleared); `attrs_manifest.xml`
  (`rollbackDataPolicy`, `hasFragileUserData`, `manageSpaceActivity`).
- AOSP `IKeyMintDevice.aidl`, "Root of Trust Binding" and "Version Binding";
  `KeyCreationResult.aidl` (one certificate when the caller names an
  attestation key); `keystore2/src/attestation_key_utils.rs` (no remote
  provisioning call when an attestation key is named).
- Google's published attestation test chains,
  https://github.com/android/keyattestation: sizes, algorithms and validity
  periods; the zero boot key in two Pixel 9 Pro chains.
- Apple, `kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly`: items of the
  class never migrate, cannot be stored without a passcode, and are deleted
  when the passcode is disabled. Apple Platform Security, keychain data
  protection: the items become useless if the passcode is "removed or reset",
  and are not backed up.
- Apple's published keychain source (`SecDb.c`, `SecItemServer.c`):
  write-ahead-log mode and no synchronous or full-sync setting. Apple's
  SQLite build on macOS 27.0.1: a write-ahead-log database defaults to a
  synchronous level that SQLite documents as able to lose a committed
  transaction on power loss. The iOS build was not read.
- Apple, "Validating apps that connect to your server" and "Assessing fraud
  risk": the counter, the two iOS 27 extension fields, and that an attacker
  who modifies the operating system "might bypass restrictions".
- https://security.apple.com/research-device/: eligibility and use of the
  research device. Summary only.
- https://www.samsung.com/us/support/answer/ANS10002458/: the two sentences
  on app data in Smart Switch.
- The public boot-ROM jailbreak for Apple devices with A8 to A11 chips
  (https://github.com/palera1n/palera1n): result summaries only.

