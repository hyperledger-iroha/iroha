# NoritoBridge XCFramework Artifacts

Current source ABI: 21. ABI 14 added
`connect_norito_encode_transfer_instruction_box` for native multisig proposal
instruction boxes; later additive revisions include the native Kagemusha V2
surfaces. The archive checksums below are historical and do not establish an
ABI-21 artifact. Regenerate, verify, and republish the bridge artifacts before
cutting an SDK release that depends on the current source surface.

- `NoritoBridge.xcframework.zip`
  - SHA-256: 9bdd96f97f2eccc9e901c0500bd8f2b046c600080ebbe1213a4febba13c44efd
- `NoritoBridge-xcframework.tar.gz`
  - SHA-256: 316fe22a83f217180700e1aa8b98c00d001d8009dfbdf465717794878c75441c

Instructions:
1. Upload one of the archives to the release host.
2. Update `Package.swift.template` `<URL>` + `<CHECKSUM>` and `NoritoBridge.podspec.template` `<ZIP_URL>`.
3. Commit/publish templates for consumers.
