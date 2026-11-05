# Disposable push-signing test keys

These keys are public, disposable test fixtures, generated locally with OpenSSL.
They must never be used as provider credentials. The RSA PKCS#1 and PKCS#8 files
encode the same 2,048-bit key; the P-256 PKCS#8 and SEC1 files encode the same
curve key. P-384 and public-only inputs exercise rejection paths.

The FCM known-answer token signs the exact ASCII input
`eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJpYXQiOjE3MDAwMDAwMDAsImlzcyI6Imlzc3VlciJ9`
using `openssl dgst -sha256 -sign rsa-pkcs8.pem`. Its signature is encoded with
unpadded base64url. RSA PKCS#1 v1.5 signatures are deterministic. APNs tests use
the independent RustCrypto P-256 verifier because ECDSA signatures are randomized.

Generation commands:

```sh
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out rsa-pkcs8.pem
openssl pkey -in rsa-pkcs8.pem -traditional -out rsa-pkcs1.pem
openssl pkey -in rsa-pkcs8.pem -pubout -out rsa-public.pem
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out ec-p256-pkcs8.pem
openssl pkey -in ec-p256-pkcs8.pem -traditional -out ec-p256-sec1.pem
openssl pkey -in ec-p256-pkcs8.pem -pubout -out ec-p256-public.pem
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-384 -out ec-p384-pkcs8.pem
```
