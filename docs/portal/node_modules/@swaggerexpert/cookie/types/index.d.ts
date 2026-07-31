export type CookieName = string;
export type CookieValue = string;
export type CookieString = string;
export type CookiePair = [CookieName, CookieValue];

/**
 * Cookie - parse
 */
export function parseCookie(cookieString: CookieString, options?: ParseOptions): ParseResult;

export interface ParseResult {
  readonly result: {
    readonly success: boolean;
  }
  readonly ast: {
    readonly translate: (parts: CookiePair[]) => CookiePair[];
    readonly toXml: () => string;
  };
}

export interface ParseOptions {
  readonly strict?: boolean;
}

/**
 * Cookie - serialize
 */
export function serializeCookie(cookiePairs: CookiePair[], options?: CookieSerializeOptions): CookieString;
export function serializeCookie(cookies: Record<CookieName, CookieValue>, options?: CookieSerializeOptions): CookieString;

export interface CookieSerializeOptions {
  encoders?: {
    name?: CookieNameEncoder;
    value?: CookieValueEncoder;
  },
  validators?: {
    name?: CookieNameValidator;
    value?: CookieValueValidator;
  }
}

/**
 * Encoders
 */
export declare const base64Encoder: Encoder;
export declare const base64urlEncoder: Encoder;
export declare const cookieNameStrictPercentEncoder: CookieNameEncoder;
export declare const cookieNameLenientPercentEncoder: CookieNameEncoder;
export declare const cookieValueStrictPercentEncoder: CookieValueEncoder;
export declare const cookieValueStrictBase64Encoder: CookieValueEncoder;
export declare const cookieValueStrictBase64urlEncoder: CookieValueEncoder;
export declare const cookieValueLenientPercentEncoder: CookieValueEncoder;
export declare const cookieValueLenientBase64Encoder: CookieValueEncoder;
export declare const cookieValueLenientBase64urlEncoder: CookieValueEncoder;
export interface Encoder {
  (value: string): string
}
export interface CookieNameEncoder extends Encoder {
  (value: CookieName): string
}
export interface CookieValueEncoder extends Encoder {
  (value: CookieValue): string
}

/**
 * Validators
 */
export declare const cookieNameStrictValidator: CookieNameValidator;
export declare const cookieNameLenientValidator: CookieNameValidator;
export declare const cookieValueStrictValidator: CookieValueValidator;
export declare const cookieValueLenientValidator: CookieValueValidator;
export interface Validator {
  (value: string): void;
}
export interface CookieNameValidator extends Validator {
  (value: CookieName): void;
}
export interface CookieValueValidator extends Validator {
  (value: CookieValue): void;
}

/**
 * Utils
 */
export function identity<T>(value: T): T;
export function noop(...args: any[]): void;

/**
 * Grammar
 */
export declare const Grammar: {
  new (): Grammar;
};
export interface Grammar {
  grammarObject: string; // Internal identifier
  rules: Rule[]; // List of grammar rules
  udts: UDT[]; // User-defined terminals (empty in this grammar)
  toString(): string; // Method to return the grammar in ABNF format
}

export interface Rule {
  name: string; // Rule name
  lower: string; // Lowercased rule name
  index: number; // Rule index
  isBkr: boolean; // Is this a back-reference?
  opcodes?: Opcode[]; // List of opcodes for the rule
}

export type Opcode =
  | { type: 1; children: number[] } // ALT (alternation)
  | { type: 2; children: number[] } // CAT (concatenation)
  | { type: 3; min: number; max: number } // REP (repetition)
  | { type: 4; index: number } // RNM (rule reference)
  | { type: 5; min: number; max: number } // TRG (terminal range)
  | { type: 6 | 7; string: number[] }; // TBS or TLS (byte sequence or literal string)

export type UDT = {}; // User-defined terminals (empty in this grammar)
