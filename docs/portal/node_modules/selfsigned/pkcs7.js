const pkijs = require("pkijs");
const nodeCrypto = require("crypto");

// Use Node.js native webcrypto
const crypto = nodeCrypto.webcrypto;

// Set up pkijs to use native crypto
// Note: This modifies global pkijs state. If the consumer also uses pkijs,
// they should set their own engine or use a version that supports per-instance engines.
let pkijsInitialized = false;

function ensurePkijsInitialized() {
  if (!pkijsInitialized) {
    pkijs.setEngine("nodeEngine", crypto, new pkijs.CryptoEngine({
      name: "",
      crypto: crypto,
      subtle: crypto.subtle
    }));
    pkijsInitialized = true;
  }
}

/**
 * Create PKCS#7 formatted certificate from PEM certificate
 *
 * @param {string} certPem - PEM formatted certificate
 * @returns {string} PKCS#7 PEM formatted certificate
 */
function createPkcs7(certPem) {
  ensurePkijsInitialized();

  // Parse the PEM certificate to get raw data
  const certLines = certPem.split('\n').filter(line =>
    !line.includes('BEGIN CERTIFICATE') &&
    !line.includes('END CERTIFICATE') &&
    line.trim()
  );
  const certBase64 = certLines.join('');
  const certBuffer = Buffer.from(certBase64, 'base64');

  // Parse certificate using pkijs
  const asn1Cert = pkijs.Certificate.fromBER(certBuffer);

  // Create PKCS#7 SignedData structure
  const cmsSigned = new pkijs.SignedData({
    version: 1,
    encapContentInfo: new pkijs.EncapsulatedContentInfo({
      eContentType: "1.2.840.113549.1.7.1" // data
    }),
    certificates: [asn1Cert]
  });

  // Wrap in ContentInfo
  const cmsSignedSchema = cmsSigned.toSchema();
  const cmsContentInfo = new pkijs.ContentInfo({
    contentType: "1.2.840.113549.1.7.2", // signedData
    content: cmsSignedSchema
  });

  // Convert to DER and then PEM
  const cmsSignedDer = cmsContentInfo.toSchema().toBER(false);
  const pkcs7Pem =
    '-----BEGIN PKCS7-----\n' +
    Buffer.from(cmsSignedDer).toString('base64').match(/.{1,64}/g).join('\n') +
    '\n-----END PKCS7-----\n';

  return pkcs7Pem;
}

module.exports = { createPkcs7 };
