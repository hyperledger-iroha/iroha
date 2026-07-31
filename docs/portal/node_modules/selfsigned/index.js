const { X509CertificateGenerator, X509Certificate, cryptoProvider,  X509ChainBuilder, BasicConstraintsExtension, KeyUsagesExtension, KeyUsageFlags, ExtendedKeyUsageExtension, ExtendedKeyUsage, SubjectAlternativeNameExtension, GeneralName } = require("@peculiar/x509");
const nodeCrypto = require("crypto");

// Use Node.js native webcrypto
const crypto = nodeCrypto.webcrypto;

// Patch global CryptoProvider to use Node.js crypto
cryptoProvider.set(crypto);

// a hexString is considered negative if it's most significant bit is 1
// because serial numbers use ones' complement notation
// this RFC in section 4.1.2.2 requires serial numbers to be positive
// http://www.ietf.org/rfc/rfc5280.txt
function toPositiveHex(hexString) {
  var mostSiginficativeHexAsInt = parseInt(hexString[0], 16);
  if (mostSiginficativeHexAsInt < 8) {
    return hexString;
  }

  mostSiginficativeHexAsInt -= 8;
  return mostSiginficativeHexAsInt.toString() + hexString.substring(1);
}

function getAlgorithmName(key) {
  switch (key) {
    case "sha256":
      return "SHA-256";
    case 'sha384':
      return "SHA-384";
    case 'sha512':
      return "SHA-512";
    default:
      return "SHA-1";
  }
}

function getSigningAlgorithm(hashKey, keyType) {
  const hashAlg = getAlgorithmName(hashKey);
  if (keyType === 'ec') {
    return {
      name: "ECDSA",
      hash: hashAlg
    };
  }
  return {
    name: "RSASSA-PKCS1-v1_5",
    hash: hashAlg
  };
}

function getKeyAlgorithm(options) {
  const keyType = options.keyType || 'rsa';
  const hashAlg = getAlgorithmName(options.algorithm || 'sha1');

  if (keyType === 'ec') {
    const curve = options.curve || 'P-256';
    return {
      name: "ECDSA",
      namedCurve: curve
    };
  }

  return {
    name: "RSASSA-PKCS1-v1_5",
    modulusLength: options.keySize || 2048,
    publicExponent: new Uint8Array([1, 0, 1]),
    hash: hashAlg
  };
}

// Build extensions array from options or use defaults
// Supports the old node-forge extension format for backwards compatibility
function buildExtensions(userExtensions, commonName) {
  if (!userExtensions || userExtensions.length === 0) {
    // Default extensions
    return [
      new BasicConstraintsExtension(false, undefined, true),
      new KeyUsagesExtension(KeyUsageFlags.digitalSignature | KeyUsageFlags.keyEncipherment, true),
      new ExtendedKeyUsageExtension([ExtendedKeyUsage.serverAuth, ExtendedKeyUsage.clientAuth], false),
      new SubjectAlternativeNameExtension([
        { type: 'dns', value: commonName },
        ...(commonName === 'localhost' ? [{ type: 'ip', value: '127.0.0.1' }] : [])
      ], false)
    ];
  }

  // Convert user extensions from node-forge format to @peculiar/x509 format
  const extensions = [];

  for (const ext of userExtensions) {
    const critical = ext.critical || false;

    switch (ext.name) {
      case 'basicConstraints':
        extensions.push(new BasicConstraintsExtension(
          ext.cA || false,
          ext.pathLenConstraint,
          critical
        ));
        break;

      case 'keyUsage':
        let flags = 0;
        if (ext.digitalSignature) flags |= KeyUsageFlags.digitalSignature;
        if (ext.nonRepudiation || ext.contentCommitment) flags |= KeyUsageFlags.nonRepudiation;
        if (ext.keyEncipherment) flags |= KeyUsageFlags.keyEncipherment;
        if (ext.dataEncipherment) flags |= KeyUsageFlags.dataEncipherment;
        if (ext.keyAgreement) flags |= KeyUsageFlags.keyAgreement;
        if (ext.keyCertSign) flags |= KeyUsageFlags.keyCertSign;
        if (ext.cRLSign) flags |= KeyUsageFlags.cRLSign;
        if (ext.encipherOnly) flags |= KeyUsageFlags.encipherOnly;
        if (ext.decipherOnly) flags |= KeyUsageFlags.decipherOnly;
        extensions.push(new KeyUsagesExtension(flags, critical));
        break;

      case 'extKeyUsage':
        const usages = [];
        if (ext.serverAuth) usages.push(ExtendedKeyUsage.serverAuth);
        if (ext.clientAuth) usages.push(ExtendedKeyUsage.clientAuth);
        if (ext.codeSigning) usages.push(ExtendedKeyUsage.codeSigning);
        if (ext.emailProtection) usages.push(ExtendedKeyUsage.emailProtection);
        if (ext.timeStamping) usages.push(ExtendedKeyUsage.timeStamping);
        extensions.push(new ExtendedKeyUsageExtension(usages, critical));
        break;

      case 'subjectAltName':
        const altNames = (ext.altNames || []).map(alt => {
          // node-forge type values:
          // 1 = email (rfc822Name)
          // 2 = DNS
          // 6 = URI
          // 7 = IP
          switch (alt.type) {
            case 1: // email
              return { type: 'email', value: alt.value };
            case 2: // DNS
              return { type: 'dns', value: alt.value };
            case 6: // URI
              return { type: 'url', value: alt.value };
            case 7: // IP
              return { type: 'ip', value: alt.ip || alt.value };
            default:
              // Try to infer type from properties
              if (alt.ip) return { type: 'ip', value: alt.ip };
              if (alt.dns) return { type: 'dns', value: alt.dns };
              if (alt.email) return { type: 'email', value: alt.email };
              if (alt.uri || alt.url) return { type: 'url', value: alt.uri || alt.url };
              return { type: 'dns', value: alt.value };
          }
        });
        extensions.push(new SubjectAlternativeNameExtension(altNames, critical));
        break;

      default:
        // Skip unknown extensions with a warning
        console.warn(`Unknown extension "${ext.name}" ignored`);
    }
  }

  return extensions;
}

// Convert attributes from node-forge format to X509 name format
function convertAttributes(attrs) {
  const nameMap = {
    'commonName': 'CN',
    'countryName': 'C',
    'ST': 'ST',
    'localityName': 'L',
    'organizationName': 'O',
    'OU': 'OU'
  };

  return attrs.map(attr => {
    const key = attr.name || attr.shortName;
    const oid = nameMap[key] || key;
    return `${oid}=${attr.value}`;
  }).join(', ');
}

// Detect key type from PEM key using Node.js crypto
function detectKeyType(pemKey) {
  const keyObject = nodeCrypto.createPrivateKey(pemKey);
  return keyObject.asymmetricKeyType; // 'rsa' or 'ec'
}

// Map Node.js curve names to Web Crypto curve names
function normalizeECCurve(curveName) {
  const curveMap = {
    'prime256v1': 'P-256',
    'secp384r1': 'P-384',
    'secp521r1': 'P-521',
    'P-256': 'P-256',
    'P-384': 'P-384',
    'P-521': 'P-521'
  };
  return curveMap[curveName] || curveName;
}

// Get EC curve from key object
function getECCurve(keyObject) {
  const details = keyObject.asymmetricKeyDetails;
  if (details && details.namedCurve) {
    return normalizeECCurve(details.namedCurve);
  }
  return 'P-256'; // default
}

// Convert PEM key to CryptoKey
async function importPrivateKey(pemKey, algorithm, keyType) {
  // Auto-detect key type if not provided
  const keyObject = nodeCrypto.createPrivateKey(pemKey);
  const detectedKeyType = keyObject.asymmetricKeyType;
  const actualKeyType = keyType || detectedKeyType;

  // Convert to PKCS#8 format
  const pkcs8Pem = keyObject.export({ type: 'pkcs8', format: 'pem' });
  const pemContents = pkcs8Pem
    .replace(/-----BEGIN PRIVATE KEY-----/, '')
    .replace(/-----END PRIVATE KEY-----/, '')
    .replace(/\s/g, '');
  const binaryDer = Buffer.from(pemContents, 'base64');

  let importAlgorithm;
  if (actualKeyType === 'ec') {
    const curve = getECCurve(keyObject);
    importAlgorithm = {
      name: 'ECDSA',
      namedCurve: curve
    };
  } else {
    importAlgorithm = {
      name: 'RSASSA-PKCS1-v1_5',
      hash: getAlgorithmName(algorithm)
    };
  }

  return await crypto.subtle.importKey(
    'pkcs8',
    binaryDer,
    importAlgorithm,
    true,
    ['sign']
  );
}

async function importPublicKey(pemKey, algorithm, keyType, curve) {
  const pemContents = pemKey
    .replace(/-----BEGIN PUBLIC KEY-----/, '')
    .replace(/-----END PUBLIC KEY-----/, '')
    .replace(/\s/g, '');

  const binaryDer = Buffer.from(pemContents, 'base64');

  let importAlgorithm;
  if (keyType === 'ec') {
    importAlgorithm = {
      name: 'ECDSA',
      namedCurve: curve || 'P-256'
    };
  } else {
    importAlgorithm = {
      name: 'RSASSA-PKCS1-v1_5',
      hash: getAlgorithmName(algorithm)
    };
  }

  return await crypto.subtle.importKey(
    'spki',
    binaryDer,
    importAlgorithm,
    true,
    ['verify']
  );
}

async function generatePemAsync(keyPair, attrs, options, ca) {
  const { privateKey, publicKey } = keyPair;

  // Generate serial number
  const serialBytes = crypto.getRandomValues(new Uint8Array(9));
  const serialHex = toPositiveHex(Buffer.from(serialBytes).toString('hex'));

  // Set up dates
  const notBefore = options.notBeforeDate || new Date();
  let notAfter;
  if (options.notAfterDate) {
    notAfter = options.notAfterDate;
  } else {
    notAfter = new Date(notBefore);
    notAfter.setDate(notAfter.getDate() + 365);
  }

  // Default attributes
  attrs = attrs || [
    {
      name: "commonName",
      value: "example.org",
    },
    {
      name: "countryName",
      value: "US",
    },
    {
      shortName: "ST",
      value: "Virginia",
    },
    {
      name: "localityName",
      value: "Blacksburg",
    },
    {
      name: "organizationName",
      value: "Test",
    },
    {
      shortName: "OU",
      value: "Test",
    },
  ];

  const subjectName = convertAttributes(attrs);
  const keyType = options.keyType || 'rsa';
  const signingAlg = getSigningAlgorithm(options.algorithm, keyType);

  // Extract common name for SAN extension
  const commonNameAttr = attrs.find(attr => attr.name === 'commonName' || attr.shortName === 'CN');
  const commonName = commonNameAttr ? commonNameAttr.value : 'localhost';

  // Build extensions array
  const extensions = buildExtensions(options.extensions, commonName);

  let cert;

  if (ca) {
    // Generate certificate signed by CA
    const caCert = new X509Certificate(ca.cert);
    const caPrivateKey = await importPrivateKey(ca.key, options.algorithm || "sha256", keyType);

    cert = await X509CertificateGenerator.create({
      serialNumber: serialHex,
      subject: subjectName,
      issuer: caCert.subject,
      notBefore: notBefore,
      notAfter: notAfter,
      signingAlgorithm: signingAlg,
      publicKey: publicKey,
      signingKey: caPrivateKey,
      extensions: extensions
    });
  } else {
    // Generate self-signed certificate
    cert = await X509CertificateGenerator.createSelfSigned({
      serialNumber: serialHex,
      name: subjectName,
      notBefore: notBefore,
      notAfter: notAfter,
      signingAlgorithm: signingAlg,
      keys: {
        privateKey: privateKey,
        publicKey: publicKey
      },
      extensions: extensions
    });
  }

  // Calculate fingerprint (SHA-1 hash of the certificate)
  const certRaw = cert.rawData;
  const fingerprintBuffer = await crypto.subtle.digest('SHA-1', certRaw);
  const fingerprint = Buffer.from(fingerprintBuffer)
    .toString('hex')
    .match(/.{2}/g)
    .join(':');

  // Export keys to PEM
  const privateKeyDer = await crypto.subtle.exportKey('pkcs8', privateKey);
  const publicKeyDer = await crypto.subtle.exportKey('spki', publicKey);

  let privatePem;
  if (options.passphrase) {
    // Encrypt the private key with the passphrase using Node.js crypto
    const keyObject = nodeCrypto.createPrivateKey({
      key: Buffer.from(privateKeyDer),
      format: 'der',
      type: 'pkcs8'
    });
    privatePem = keyObject.export({
      type: 'pkcs8',
      format: 'pem',
      cipher: 'aes-256-cbc',
      passphrase: options.passphrase
    });
  } else {
    privatePem =
      '-----BEGIN PRIVATE KEY-----\n' +
      Buffer.from(privateKeyDer).toString('base64').match(/.{1,64}/g).join('\n') +
      '\n-----END PRIVATE KEY-----\n';
  }

  const publicPem =
    '-----BEGIN PUBLIC KEY-----\n' +
    Buffer.from(publicKeyDer).toString('base64').match(/.{1,64}/g).join('\n') +
    '\n-----END PUBLIC KEY-----\n';

  const certPem = cert.toString('pem');

  const pem = {
    private: privatePem,
    public: publicPem,
    cert: certPem,
    fingerprint: fingerprint,
  };

  // Client certificate support
  if (options && options.clientCertificate) {
    // Parse clientCertificate options - can be boolean or object
    const clientOpts = typeof options.clientCertificate === 'object' ? options.clientCertificate : {};

    // Resolve client certificate options with fallbacks to deprecated options
    const clientKeySize = clientOpts.keySize || options.clientCertificateKeySize || 2048;
    const clientAlgorithm = clientOpts.algorithm || options.algorithm || "sha1";
    const clientCN = clientOpts.cn || options.clientCertificateCN || "John Doe jdoe123";
    // Client cert uses same key type and curve as main cert by default
    const clientKeyType = clientOpts.keyType || keyType;
    const clientCurve = clientOpts.curve || options.curve || 'P-256';

    const clientKeyAlg = getKeyAlgorithm({
      keyType: clientKeyType,
      keySize: clientKeySize,
      algorithm: clientAlgorithm,
      curve: clientCurve
    });

    const clientKeyPair = await crypto.subtle.generateKey(
      clientKeyAlg,
      true,
      ["sign", "verify"]
    );

    const clientSerialBytes = crypto.getRandomValues(new Uint8Array(9));
    const clientSerialHex = toPositiveHex(Buffer.from(clientSerialBytes).toString('hex'));

    // Resolve client certificate validity dates
    const clientNotBefore = clientOpts.notBeforeDate || new Date();
    let clientNotAfter;
    if (clientOpts.notAfterDate) {
      clientNotAfter = clientOpts.notAfterDate;
    } else {
      clientNotAfter = new Date(clientNotBefore);
      clientNotAfter.setFullYear(clientNotBefore.getFullYear() + 1);
    }

    const clientAttrs = JSON.parse(JSON.stringify(attrs));
    for (let i = 0; i < clientAttrs.length; i++) {
      if (clientAttrs[i].name === "commonName") {
        clientAttrs[i] = {
          name: "commonName",
          value: clientCN
        };
      }
    }

    const clientSubjectName = convertAttributes(clientAttrs);
    const issuerName = convertAttributes(attrs);

    // Signing algorithm for client cert - uses main key type since signed by root
    const clientSigningAlg = getSigningAlgorithm(clientAlgorithm, keyType);

    // Create client cert signed by root key
    const clientCertRaw = await X509CertificateGenerator.create({
      serialNumber: clientSerialHex,
      subject: clientSubjectName,
      issuer: issuerName,
      notBefore: clientNotBefore,
      notAfter: clientNotAfter,
      signingAlgorithm: clientSigningAlg,
      publicKey: clientKeyPair.publicKey,
      signingKey: privateKey // Sign with root private key
    });

    // Export client keys
    const clientPrivateKeyDer = await crypto.subtle.exportKey('pkcs8', clientKeyPair.privateKey);
    const clientPublicKeyDer = await crypto.subtle.exportKey('spki', clientKeyPair.publicKey);

    pem.clientprivate =
      '-----BEGIN PRIVATE KEY-----\n' +
      Buffer.from(clientPrivateKeyDer).toString('base64').match(/.{1,64}/g).join('\n') +
      '\n-----END PRIVATE KEY-----\n';

    pem.clientpublic =
      '-----BEGIN PUBLIC KEY-----\n' +
      Buffer.from(clientPublicKeyDer).toString('base64').match(/.{1,64}/g).join('\n') +
      '\n-----END PUBLIC KEY-----\n';

    pem.clientcert = clientCertRaw.toString('pem');
  }

  // Verify certificate chain
  const x509Cert = new X509Certificate(cert.rawData);
  const certificates = [x509Cert];

  // If CA-signed, include CA cert in the chain for verification
  if (ca) {
    const caCert = new X509Certificate(ca.cert);
    certificates.push(caCert);
  }

  const chainBuilder = new X509ChainBuilder({
    certificates: certificates
  });

  const chain = await chainBuilder.build(x509Cert);
  if (chain.length === 0) {
    throw new Error("Certificate could not be verified.");
  }

  return pem;
}

/**
 * Generate a certificate (async)
 *
 * @param {CertificateField[]} attrs Attributes used for subject.
 * @param {object} options
 * @param {string} [options.keyType="rsa"] Key type: "rsa" or "ec" (elliptic curve)
 * @param {number} [options.keySize=2048] the size for the private key in bits (RSA only)
 * @param {string} [options.curve="P-256"] The elliptic curve to use: "P-256", "P-384", or "P-521" (EC only)
 * @param {object} [options.extensions] additional extensions for the certificate
 * @param {string} [options.algorithm="sha1"] The signature algorithm sha256, sha384, sha512 or sha1
 * @param {Date} [options.notBeforeDate=new Date()] The date before which the certificate should not be valid
 * @param {Date} [options.notAfterDate] The date after which the certificate should not be valid (default: notBeforeDate + 365 days)
 * @param {boolean|object} [options.clientCertificate=false] Generate client cert signed by the original key. Can be `true` for defaults or an options object.
 * @param {number} [options.clientCertificate.keySize=2048] Key size for the client certificate in bits (RSA only)
 * @param {string} [options.clientCertificate.keyType] Key type for client cert (defaults to main keyType)
 * @param {string} [options.clientCertificate.curve] Elliptic curve for client cert (EC only)
 * @param {string} [options.clientCertificate.algorithm] Signature algorithm for client cert (defaults to options.algorithm or "sha1")
 * @param {string} [options.clientCertificate.cn="John Doe jdoe123"] Client certificate's common name
 * @param {Date} [options.clientCertificate.notBeforeDate=new Date()] The date before which the client certificate should not be valid
 * @param {Date} [options.clientCertificate.notAfterDate] The date after which the client certificate should not be valid (default: notBeforeDate + 1 year)
 * @param {string} [options.clientCertificateCN="John Doe jdoe123"] @deprecated Use options.clientCertificate.cn instead
 * @param {number} [options.clientCertificateKeySize] @deprecated Use options.clientCertificate.keySize instead
 * @param {object} [options.ca] CA certificate and key for signing (if not provided, generates self-signed)
 * @param {string} [options.ca.key] CA private key in PEM format
 * @param {string} [options.ca.cert] CA certificate in PEM format
 * @param {string} [options.passphrase] Passphrase to encrypt the private key (uses AES-256-CBC)
 * @returns {Promise<object>} Promise that resolves with certificate data
 */
exports.generate = async function generate(attrs, options) {
  attrs = attrs || undefined;
  options = options || {};

  const keyType = options.keyType || 'rsa';
  const curve = options.curve || 'P-256';

  let keyPair;

  if (options.keyPair) {
    // Import existing key pair
    keyPair = {
      privateKey: await importPrivateKey(options.keyPair.privateKey, options.algorithm || "sha1", keyType),
      publicKey: await importPublicKey(options.keyPair.publicKey, options.algorithm || "sha1", keyType, curve)
    };
  } else {
    // Generate new key pair using appropriate algorithm
    const keyAlg = getKeyAlgorithm(options);
    keyPair = await crypto.subtle.generateKey(
      keyAlg,
      true,
      ["sign", "verify"]
    );
  }

  return await generatePemAsync(keyPair, attrs, options, options.ca);
};
