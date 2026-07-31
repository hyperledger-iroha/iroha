import { both } from 'ramda';
import isNumber from '../../isNumber.js';

// eslint-disable-next-line no-restricted-globals
var isNaNPonyfill = both(isNumber, isNaN);
export default isNaNPonyfill;