interface ParseResult {
  readonly result: {
    readonly success: boolean;
  };
  readonly ast: {
    readonly translate: (parts: any[]) => Array<[string, string]>;
    readonly toXml: () => string;
  };
}

interface TestOptions {
  readonly strict?: boolean;
}

interface ServerVariables {
  [key: string]: any;
}

interface SubstituteOptions {
  encoder: (serverVariableValue: string, serverVariableName?: string) => string;
}

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

export function parse(serverURLTemplate: string): ParseResult;
export function test(serverURLTemplate: string, options?: TestOptions): boolean;
export function substitute(
  serverURLTemplate: string,
  serverVariables: ServerVariables,
  options?: SubstituteOptions,
): string;
export function encodeServerVariable(serverVariableValue: string): string;
export function Grammar(): Grammar;
