import * as _ from 'ts-toolbelt';

export type _ZipObj<K extends readonly PropertyKey[], V extends {[key in keyof K]: unknown} | readonly unknown[]> =
    number extends V['length'] ?
      { [T in K[number]]: V[number] } :
      number extends K['length'] ?
        { [T in K[number]]: V[number] } : _.L.ZipObj<K, V>;
