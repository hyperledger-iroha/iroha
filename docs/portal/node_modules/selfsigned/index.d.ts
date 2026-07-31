declare enum ASN1Class {
    UNIVERSAL = 0x00,
    APPLICATION = 0x40,
    CONTEXT_SPECIFIC = 0x80,
    PRIVATE = 0xc0,
}

interface CertificateFieldOptions {
    name?: string | undefined;
    type?: string | undefined;
    shortName?: string | undefined;
}

interface CertificateField extends CertificateFieldOptions {
    valueConstructed?: boolean | undefined;
    valueTagClass?: ASN1Class | undefined;
    value?: any[] | string | undefined;
    extensions?: any[] | undefined;
}

/**
 * Subject Alternative Name entry types:
 * - 1: email (rfc822Name)
 * - 2: DNS name
 * - 6: URI
 * - 7: IP address
 */
declare interface SubjectAltNameEntry {
  /**
   * Type of the alternative name:
   * - 1: email (rfc822Name)
   * - 2: DNS name
   * - 6: URI
   * - 7: IP address
   */
  type: 1 | 2 | 6 | 7;
  /** Value for types 1, 2, 6 (email, DNS, URI) */
  value?: string;
  /** IP address for type 7 (IPv4 or IPv6) */
  ip?: string;
}

declare interface BasicConstraintsExtension {
  name: 'basicConstraints';
  /** Is this a CA certificate? */
  cA?: boolean;
  /** Maximum depth of valid certificate chain */
  pathLenConstraint?: number;
  /** Mark extension as critical */
  critical?: boolean;
}

declare interface KeyUsageExtension {
  name: 'keyUsage';
  digitalSignature?: boolean;
  nonRepudiation?: boolean;
  /** Also known as contentCommitment */
  contentCommitment?: boolean;
  keyEncipherment?: boolean;
  dataEncipherment?: boolean;
  keyAgreement?: boolean;
  /** For CA certificates */
  keyCertSign?: boolean;
  /** For CA certificates */
  cRLSign?: boolean;
  encipherOnly?: boolean;
  decipherOnly?: boolean;
  /** Mark extension as critical */
  critical?: boolean;
}

declare interface ExtKeyUsageExtension {
  name: 'extKeyUsage';
  /** TLS server authentication */
  serverAuth?: boolean;
  /** TLS client authentication */
  clientAuth?: boolean;
  codeSigning?: boolean;
  emailProtection?: boolean;
  timeStamping?: boolean;
  /** Mark extension as critical */
  critical?: boolean;
}

declare interface SubjectAltNameExtension {
  name: 'subjectAltName';
  altNames: SubjectAltNameEntry[];
  /** Mark extension as critical */
  critical?: boolean;
}

declare type CertificateExtension =
  | BasicConstraintsExtension
  | KeyUsageExtension
  | ExtKeyUsageExtension
  | SubjectAltNameExtension;

declare interface ClientCertificateOptions {
  /**
   * Key size for the client certificate in bits (RSA only)
   * @default 2048
   */
  keySize?: number
  /**
   * Key type for client certificate
   * @default inherits from main keyType
   */
  keyType?: 'rsa' | 'ec'
  /**
   * Elliptic curve for client certificate (EC only)
   * @default "P-256"
   */
  curve?: 'P-256' | 'P-384' | 'P-521'
  /**
   * Signature algorithm for client certificate
   * @default inherits from main algorithm or "sha1"
   */
  algorithm?: string
  /**
   * Client certificate's common name
   * @default "John Doe jdoe123"
   */
  cn?: string
  /**
   * The date before which the client certificate should not be valid
   * @default now
   */
  notBeforeDate?: Date
  /**
   * The date after which the client certificate should not be valid
   * @default notBeforeDate + 1 year
   */
  notAfterDate?: Date
}

declare interface SelfsignedOptions {
  /**
   * The date before which the certificate should not be valid
   *
   * @default now */
  notBeforeDate?: Date

  /**
   * The date after which the certificate should not be valid
   *
   * @default notBeforeDate + 365 days */
  notAfterDate?: Date

  /**
   * Key type: "rsa" or "ec" (elliptic curve)
   * @default "rsa"
   */
  keyType?: 'rsa' | 'ec'
  /**
   * the size for the private key in bits (RSA only)
   * @default 2048
   */
  keySize?: number
  /**
   * The elliptic curve to use (EC only): "P-256", "P-384", or "P-521"
   * @default "P-256"
   */
  curve?: 'P-256' | 'P-384' | 'P-521'
  /**
   * Certificate extensions. Supports basicConstraints, keyUsage, extKeyUsage, and subjectAltName.
   * If not provided, defaults are used including DNS SAN matching commonName.
   * @example
   * ```typescript
   * extensions: [
   *   { name: 'basicConstraints', cA: false },
   *   { name: 'keyUsage', digitalSignature: true, keyEncipherment: true },
   *   { name: 'subjectAltName', altNames: [
   *     { type: 2, value: 'localhost' },
   *     { type: 7, ip: '127.0.0.1' },
   *     { type: 7, ip: '::1' }
   *   ]}
   * ]
   * ```
   */
  extensions?: CertificateExtension[];
  /**
   * The signature algorithm: sha256, sha384, sha512 or sha1
   * @default "sha1"
   */
  algorithm?: string
  /**
   * include PKCS#7 as part of the output
   * @default false
   */
  pkcs7?: boolean
  /**
   * generate client cert signed by the original key
   * Can be `true` for defaults or an options object
   * @default false
   */
  clientCertificate?: boolean | ClientCertificateOptions
  /**
   * client certificate's common name
   * @default "John Doe jdoe123"
   * @deprecated Use clientCertificate.cn instead
   */
  clientCertificateCN?: string
  /**
   * the size for the client private key in bits
   * @default 2048
   * @deprecated Use clientCertificate.keySize instead
   */
   clientCertificateKeySize?: number
  /**
   * existing key pair to use instead of generating new keys
   */
   keyPair?: {
     privateKey: string
     publicKey: string
   }
  /**
   * CA certificate and key for signing (if not provided, generates self-signed)
   */
   ca?: {
     /** CA private key in PEM format */
     key: string
     /** CA certificate in PEM format */
     cert: string
   }
  /**
   * Passphrase to encrypt the private key (PKCS#8 encrypted format)
   * When provided, the private key will be encrypted using AES-256-CBC
   */
   passphrase?: string
}

declare interface GenerateResult {
  private: string
  public: string
  cert: string
  fingerprint: string
  pkcs7?: string
  clientprivate?: string
  clientpublic?: string
  clientcert?: string
  clientpkcs7?: string
}

/**
 * Generate a certificate (async only)
 *
 * @param attrs Certificate attributes
 * @param opts Generation options
 * @returns Promise that resolves with certificate data
 *
 * @example
 * ```typescript
 * // Self-signed certificate
 * const pems = await generate();
 *
 * const pems = await generate([{ name: 'commonName', value: 'example.com' }]);
 *
 * const pems = await generate(null, {
 *   keySize: 2048,
 *   algorithm: 'sha256'
 * });
 *
 * // CA-signed certificate
 * const pems = await generate([{ name: 'commonName', value: 'localhost' }], {
 *   algorithm: 'sha256',
 *   ca: {
 *     key: fs.readFileSync('/path/to/ca.key', 'utf8'),
 *     cert: fs.readFileSync('/path/to/ca.crt', 'utf8')
 *   }
 * });
 * ```
 */
export declare function generate(
  attrs?: CertificateField[],
  opts?: SelfsignedOptions
): Promise<GenerateResult>
