import { both } from 'ramda';
import isNumber from '../../isNumber.js';

// eslint-disable-next-line no-restricted-globals
var isFinitePonyfill = both(isNumber, isFinite);
export default isFinitePonyfill;