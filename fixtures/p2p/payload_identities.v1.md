# P2P test-payload identity vectors

`payload_identities.v1.json` contains one reference identity for each typed
test payload checked by `iroha_p2p::frame_identity_tests`. Each hash is the
first 16 bytes of SHA-256 over `b"norito:v1:type-name\0"` followed by the UTF-8
nominal name, calculated independently with Python's `hashlib`.

These are current identity vectors, not historical compiler observations.
The previously referenced `test_payload_identity_observations.v1.json` was
absent from the checkout. The filename avoids the repository's `test_*`
ignore rule. Original production frame and signature captures remain in
their separate fixture files.
