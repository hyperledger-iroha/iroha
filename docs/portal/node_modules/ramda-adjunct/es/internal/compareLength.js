import { curry, compose, length } from 'ramda';
var compareLength = curry(function (comparator, value, list) {
  return compose(comparator(value), length)(list);
});
export default compareLength;