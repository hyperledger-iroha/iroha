import type { EvaluationRealm, JSONPointer, EvaluationOptions  } from '../../index.ts';

/**
 * Realm
 */
declare class ApiDOMEvaluationRealm extends EvaluationRealm {
  public readonly name: 'apidom';

  public isArray(node: unknown): boolean;
  public isObject(node: unknown): boolean;
  public sizeOf(node: unknown): number;
  public has(node: unknown, referenceToken: string): boolean;
  public evaluate<T = unknown>(node: unknown, referenceToken: string): T;
}

/**
 * Evaluating
 */
export function evaluate<T = unknown>(value: unknown, jsonPointer: JSONPointer, options?: ApiDOMRealmEvaluationOptions): T;

export type ApiDOMRealmEvaluationOptions = Omit<EvaluationOptions, 'realm'>;

export default ApiDOMEvaluationRealm;
