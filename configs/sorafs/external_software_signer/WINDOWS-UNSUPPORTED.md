# Windows support policy

SoraFS external software signing V1 is unsupported on Windows. V1 requires
authenticated Unix peer credentials and fixed Unix-domain socket endpoints.
A Windows release must exclude both `sorafs_external_software_signer.exe` and
the `iroha-runtime-provider-broker-v1.exe` alias, must not report a signer
smoke test, and must not claim software-key qualification.

Adding Windows support requires a separately reviewed authenticated local
transport, new deployment evidence, and a new release-policy revision.
