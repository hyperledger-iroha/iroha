import { replace } from 'ramda';
var trimStart = replace(/^[\s\uFEFF\xA0]+/, '');
export default trimStart;