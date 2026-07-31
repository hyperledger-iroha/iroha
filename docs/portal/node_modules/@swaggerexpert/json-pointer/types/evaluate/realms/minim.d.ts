import type { EvaluationRealm } from '../../index.ts';

declare class MinimEvaluationRealm extends EvaluationRealm {
  public readonly name: 'minim';

  public isArray(node: unknown): boolean;
  public isObject(node: unknown): boolean;
  public sizeOf(node: unknown): number;
  public has(node: unknown, referenceToken: string): boolean;
  public evaluate<T = unknown>(node: unknown, referenceToken: string): T;
}

export default MinimEvaluationRealm;
