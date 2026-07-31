var _isString = /*#__PURE__*/require("./_isString.js");
function _nth(offset, list) {
  var idx = offset < 0 ? list.length + offset : offset;
  return _isString(list) ? list.charAt(idx) : list[idx];
}
module.exports = _nth;