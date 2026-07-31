var { assert } = require('chai');
var crypto = require('crypto');

describe('EC keys', function () {
  var generate = require('../index').generate;

  it('should generate EC certificate with P-256 curve (default)', async function () {
    var pems = await generate(null, { keyType: 'ec' });

    assert.ok(!!pems.private, 'has a private key');
    assert.ok(!!pems.public, 'has a public key');
    assert.ok(!!pems.cert, 'has a certificate');
    assert.ok(!!pems.fingerprint, 'has fingerprint');

    const privateKey = crypto.createPrivateKey(pems.private);
    assert.strictEqual(privateKey.asymmetricKeyType, 'ec', 'should be EC key');
    assert.strictEqual(privateKey.asymmetricKeyDetails.namedCurve, 'prime256v1', 'should use P-256 curve');

    const cert = new crypto.X509Certificate(pems.cert);
    assert.ok(cert.subject, 'cert has a subject');
  });

  it('should generate EC certificate with P-384 curve', async function () {
    var pems = await generate(null, { keyType: 'ec', curve: 'P-384' });

    const privateKey = crypto.createPrivateKey(pems.private);
    assert.strictEqual(privateKey.asymmetricKeyType, 'ec', 'should be EC key');
    assert.strictEqual(privateKey.asymmetricKeyDetails.namedCurve, 'secp384r1', 'should use P-384 curve');

    const cert = new crypto.X509Certificate(pems.cert);
    assert.ok(cert.publicKey, 'can generate P-384 EC certs');
  });

  it('should generate EC certificate with P-521 curve', async function () {
    var pems = await generate(null, { keyType: 'ec', curve: 'P-521' });

    const privateKey = crypto.createPrivateKey(pems.private);
    assert.strictEqual(privateKey.asymmetricKeyType, 'ec', 'should be EC key');
    assert.strictEqual(privateKey.asymmetricKeyDetails.namedCurve, 'secp521r1', 'should use P-521 curve');

    const cert = new crypto.X509Certificate(pems.cert);
    assert.ok(cert.publicKey, 'can generate P-521 EC certs');
  });

  it('should generate valid EC key pair that work together', async function () {
    var pems = await generate(null, { keyType: 'ec', curve: 'P-256' });

    const testData = 'Hello, World!';

    const privateKey = crypto.createPrivateKey(pems.private);
    const publicKey = crypto.createPublicKey(pems.public);

    // Sign with private key
    const sign = crypto.createSign('SHA256');
    sign.update(testData);
    sign.end();
    const signature = sign.sign(privateKey);

    // Verify with public key
    const verify = crypto.createVerify('SHA256');
    verify.update(testData);
    verify.end();
    const isValid = verify.verify(publicKey, signature);

    assert.isTrue(isValid, 'EC public key should verify signature from EC private key');
  });

  it('should support EC with sha256 algorithm', async function () {
    var pems = await generate(null, { keyType: 'ec', algorithm: 'sha256' });

    const cert = new crypto.X509Certificate(pems.cert);
    assert.ok(cert.publicKey, 'can generate EC cert with sha256');
  });

  it('should support EC with sha384 algorithm', async function () {
    var pems = await generate(null, { keyType: 'ec', curve: 'P-384', algorithm: 'sha384' });

    const cert = new crypto.X509Certificate(pems.cert);
    assert.ok(cert.publicKey, 'can generate EC cert with sha384');
  });

  it('should support EC with sha512 algorithm', async function () {
    var pems = await generate(null, { keyType: 'ec', curve: 'P-521', algorithm: 'sha512' });

    const cert = new crypto.X509Certificate(pems.cert);
    assert.ok(cert.publicKey, 'can generate EC cert with sha512');
  });

  it('should generate EC client certificate', async function () {
    var pems = await generate(null, { keyType: 'ec', clientCertificate: true });

    assert.ok(!!pems.clientcert, 'should include a client cert');
    assert.ok(!!pems.clientprivate, 'should include a client private key');
    assert.ok(!!pems.clientpublic, 'should include a client public key');

    const clientPrivateKey = crypto.createPrivateKey(pems.clientprivate);
    assert.strictEqual(clientPrivateKey.asymmetricKeyType, 'ec', 'client key should be EC');
  });

  it('should support passphrase with EC keys', async function () {
    const passphrase = 'ec-secret-passphrase';
    var pems = await generate(null, { keyType: 'ec', passphrase: passphrase });

    assert.include(pems.private, 'ENCRYPTED', 'EC private key should be encrypted');

    const privateKey = crypto.createPrivateKey({
      key: pems.private,
      passphrase: passphrase
    });
    assert.strictEqual(privateKey.asymmetricKeyType, 'ec', 'decrypted key should be EC');
  });

  it('should support using existing EC keyPair', async function () {
    const firstPems = await generate(null, { keyType: 'ec', curve: 'P-256' });

    const secondPems = await generate(null, {
      keyType: 'ec',
      curve: 'P-256',
      keyPair: {
        privateKey: firstPems.private,
        publicKey: firstPems.public
      }
    });

    assert.strictEqual(firstPems.private, secondPems.private, 'should use provided EC private key');
    assert.strictEqual(firstPems.public, secondPems.public, 'should use provided EC public key');
  });

  it('should support custom attributes with EC', async function () {
    const attrs = [
      { name: 'commonName', value: 'ec-test.example.com' },
      { name: 'countryName', value: 'US' },
      { name: 'organizationName', value: 'EC Test Corp' }
    ];

    var pems = await generate(attrs, { keyType: 'ec' });
    const cert = new crypto.X509Certificate(pems.cert);

    assert.include(cert.subject, 'CN=ec-test.example.com', 'should include custom CN');
    assert.include(cert.subject, 'O=EC Test Corp', 'should include custom organization');
  });
});
