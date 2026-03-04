import {createPrivateKey, createPublicKey, sign as signNode} from 'node:crypto';

const ED25519_PRIVATE_KEY_DER_PREFIX = Buffer.from(
  '302e020100300506032b657004220420',
  'hex',
);
const ED25519_PUBLIC_KEY_DER_PREFIX = Buffer.from(
  '302a300506032b6570032100',
  'hex',
);

export const TEST_ED25519_PRIVATE_KEY_HEX =
  '8f2195b4c53a6d7e1f0cbd93a4e8f7650a1b2c3d4e5f60718293a4b5c6d7e8f1';

export function signPayload(payloadInput, {privateKeyHex = TEST_ED25519_PRIVATE_KEY_HEX} = {}) {
  const payload = Buffer.isBuffer(payloadInput)
    ? payloadInput
    : Buffer.from(payloadInput, 'utf8');
  const keyBytes = Buffer.from(privateKeyHex, 'hex');
  if (keyBytes.length !== 32) {
    throw new Error('ed25519 private key must be 32 bytes');
  }
  const privateKey = createPrivateKey({
    key: Buffer.concat([ED25519_PRIVATE_KEY_DER_PREFIX, keyBytes]),
    format: 'der',
    type: 'pkcs8',
  });
  const signature = signNode(null, payload, privateKey);
  const publicKeyDer = createPublicKey(privateKey).export({format: 'der', type: 'spki'});
  const publicKeyHex = publicKeyDer
    .slice(ED25519_PUBLIC_KEY_DER_PREFIX.length)
    .toString('hex');
  return {
    signatureHex: signature.toString('hex'),
    publicKeyHex,
  };
}
