var { assert } = require('chai');
var crypto = require('crypto');

describe('CA signing', function () {
  var generate = require('../index').generate;

  it('should generate certificate signed by provided CA', async function () {
    // First generate a self-signed CA certificate
    const ca = await generate([
      { name: 'commonName', value: 'Test CA' },
      { name: 'organizationName', value: 'Test Organization' }
    ], {
      algorithm: 'sha256'
    });

    // Generate a certificate signed by the CA
    const pems = await generate([
      { name: 'commonName', value: 'localhost' }
    ], {
      algorithm: 'sha256',
      ca: {
        key: ca.private,
        cert: ca.cert
      }
    });

    assert.ok(!!pems.private, 'has a private key');
    assert.ok(!!pems.public, 'has a public key');
    assert.ok(!!pems.cert, 'has a certificate');
    assert.ok(!!pems.fingerprint, 'has fingerprint');

    const cert = new crypto.X509Certificate(pems.cert);
    const caCert = new crypto.X509Certificate(ca.cert);

    // Verify issuer is the CA, not self-signed
    assert.include(cert.issuer, 'CN=Test CA', 'issuer should be the CA');
    assert.include(cert.subject, 'CN=localhost', 'subject should be localhost');
    assert.notEqual(cert.issuer, cert.subject, 'should not be self-signed');

    // Verify the certificate is signed by the CA
    assert.isTrue(cert.verify(caCert.publicKey), 'certificate should be verified by CA public key');
  });

  it('should include Subject Alternative Name extension', async function () {
    const ca = await generate([{ name: 'commonName', value: 'Test CA' }], {
      algorithm: 'sha256'
    });

    const pems = await generate([
      { name: 'commonName', value: 'example.com' }
    ], {
      algorithm: 'sha256',
      ca: { key: ca.private, cert: ca.cert }
    });

    const cert = new crypto.X509Certificate(pems.cert);
    assert.include(cert.subjectAltName, 'DNS:example.com', 'should have DNS SAN matching CN');
  });

  it('should include IP SAN for localhost', async function () {
    const ca = await generate([{ name: 'commonName', value: 'Test CA' }], {
      algorithm: 'sha256'
    });

    const pems = await generate([
      { name: 'commonName', value: 'localhost' }
    ], {
      algorithm: 'sha256',
      ca: { key: ca.private, cert: ca.cert }
    });

    const cert = new crypto.X509Certificate(pems.cert);
    assert.include(cert.subjectAltName, 'DNS:localhost', 'should have DNS SAN');
    assert.include(cert.subjectAltName, 'IP Address:127.0.0.1', 'should have IP SAN for localhost');
  });

  it('should support different hash algorithms with CA signing', async function () {
    const ca = await generate([{ name: 'commonName', value: 'Test CA' }], {
      algorithm: 'sha256'
    });

    // Test sha384
    const pems384 = await generate([{ name: 'commonName', value: 'test384.local' }], {
      algorithm: 'sha384',
      ca: { key: ca.private, cert: ca.cert }
    });

    const cert384 = new crypto.X509Certificate(pems384.cert);
    assert.ok(cert384.publicKey, 'should generate sha384 CA-signed cert');

    // Test sha512
    const pems512 = await generate([{ name: 'commonName', value: 'test512.local' }], {
      algorithm: 'sha512',
      ca: { key: ca.private, cert: ca.cert }
    });

    const cert512 = new crypto.X509Certificate(pems512.cert);
    assert.ok(cert512.publicKey, 'should generate sha512 CA-signed cert');
  });

  it('should respect notAfterDate option with CA signing', async function () {
    const ca = await generate([{ name: 'commonName', value: 'Test CA' }], {
      algorithm: 'sha256'
    });

    const notBefore = new Date('2025-01-01T00:00:00Z');
    const notAfter = new Date('2025-01-31T00:00:00Z'); // 30 days validity
    const pems = await generate([{ name: 'commonName', value: 'short-lived.local' }], {
      algorithm: 'sha256',
      notBeforeDate: notBefore,
      notAfterDate: notAfter,
      ca: { key: ca.private, cert: ca.cert }
    });

    const cert = new crypto.X509Certificate(pems.cert);
    const validFrom = new Date(cert.validFrom);
    const validTo = new Date(cert.validTo);

    assert.approximately(validFrom.getTime(), notBefore.getTime(), 5000, 'should use custom notBeforeDate');
    assert.approximately(validTo.getTime(), notAfter.getTime(), 5000, 'should use custom notAfterDate');
  });

  it('should generate unique certificates with same CA', async function () {
    const ca = await generate([{ name: 'commonName', value: 'Test CA' }], {
      algorithm: 'sha256'
    });

    const pems1 = await generate([{ name: 'commonName', value: 'test1.local' }], {
      algorithm: 'sha256',
      ca: { key: ca.private, cert: ca.cert }
    });

    const pems2 = await generate([{ name: 'commonName', value: 'test2.local' }], {
      algorithm: 'sha256',
      ca: { key: ca.private, cert: ca.cert }
    });

    const cert1 = new crypto.X509Certificate(pems1.cert);
    const cert2 = new crypto.X509Certificate(pems2.cert);

    assert.notEqual(cert1.serialNumber, cert2.serialNumber, 'serial numbers should be unique');
    assert.notEqual(pems1.private, pems2.private, 'private keys should be different');
  });

  it('should work with custom keySize and CA signing', async function () {
    const ca = await generate([{ name: 'commonName', value: 'Test CA' }], {
      algorithm: 'sha256',
      keySize: 4096
    });

    const pems = await generate([{ name: 'commonName', value: 'bigkey.local' }], {
      algorithm: 'sha256',
      keySize: 4096,
      ca: { key: ca.private, cert: ca.cert }
    });

    const privateKey = crypto.createPrivateKey(pems.private);
    assert.strictEqual(privateKey.asymmetricKeyDetails.modulusLength, 4096, 'should use custom key size');

    const cert = new crypto.X509Certificate(pems.cert);
    const caCert = new crypto.X509Certificate(ca.cert);
    assert.isTrue(cert.verify(caCert.publicKey), 'certificate should verify with CA');
  });

  it('should support existing keyPair with CA signing', async function () {
    const ca = await generate([{ name: 'commonName', value: 'Test CA' }], {
      algorithm: 'sha256'
    });

    // Generate a key pair first
    const keyPair = await generate([{ name: 'commonName', value: 'keypair.local' }], {
      algorithm: 'sha256'
    });

    // Use existing key pair with CA signing
    const pems = await generate([{ name: 'commonName', value: 'reused.local' }], {
      algorithm: 'sha256',
      keyPair: {
        privateKey: keyPair.private,
        publicKey: keyPair.public
      },
      ca: { key: ca.private, cert: ca.cert }
    });

    assert.strictEqual(pems.private, keyPair.private, 'should use provided private key');
    assert.strictEqual(pems.public, keyPair.public, 'should use provided public key');

    const cert = new crypto.X509Certificate(pems.cert);
    const caCert = new crypto.X509Certificate(ca.cert);
    assert.isTrue(cert.verify(caCert.publicKey), 'certificate should verify with CA');
  });

  it('should include proper extended key usage extensions', async function () {
    const ca = await generate([{ name: 'commonName', value: 'Test CA' }], {
      algorithm: 'sha256'
    });

    const pems = await generate([{ name: 'commonName', value: 'server.local' }], {
      algorithm: 'sha256',
      ca: { key: ca.private, cert: ca.cert }
    });

    const cert = new crypto.X509Certificate(pems.cert);

    // Check extended key usage (OIDs)
    // 1.3.6.1.5.5.7.3.1 = serverAuth
    // 1.3.6.1.5.5.7.3.2 = clientAuth
    assert.include(cert.keyUsage, '1.3.6.1.5.5.7.3.1', 'should have serverAuth extended key usage');
    assert.include(cert.keyUsage, '1.3.6.1.5.5.7.3.2', 'should have clientAuth extended key usage');
  });

  it('should work with PKCS#1 RSA key format', async function () {
    // Generate a CA with PKCS#1 format key (like mkcert uses)
    const { privateKey, publicKey } = crypto.generateKeyPairSync('rsa', {
      modulusLength: 2048,
      privateKeyEncoding: { type: 'pkcs1', format: 'pem' },
      publicKeyEncoding: { type: 'spki', format: 'pem' }
    });

    // Create a self-signed CA cert using the PKCS#1 key
    const ca = await generate([{ name: 'commonName', value: 'PKCS1 CA' }], {
      algorithm: 'sha256'
    });

    // Now test that we can use a PKCS#1 formatted key as CA
    // Convert our generated key to PKCS#1 for testing
    const caKeyObject = crypto.createPrivateKey(ca.private);
    const pkcs1Key = caKeyObject.export({ type: 'pkcs1', format: 'pem' });

    const pems = await generate([{ name: 'commonName', value: 'pkcs1-test.local' }], {
      algorithm: 'sha256',
      ca: {
        key: pkcs1Key,
        cert: ca.cert
      }
    });

    const cert = new crypto.X509Certificate(pems.cert);
    const caCert = new crypto.X509Certificate(ca.cert);
    assert.isTrue(cert.verify(caCert.publicKey), 'should work with PKCS#1 key format');
  });
});
