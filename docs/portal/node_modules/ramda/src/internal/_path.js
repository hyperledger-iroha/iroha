var _isInteger = /*#__PURE__*/require("./_isInteger.js");
var _nth = /*#__PURE__*/require("./_nth.js");
function _path(pathAr, obj) {
  var val = obj;
  for (var i = 0; i < pathAr.length; i += 1) {
    if (val == null) {
      return undefined;
    }
    var p = pathAr[i];
    if (_isInteger(p)) {
      val = _nth(p, val);
    } else {
      val = val[p];
    }
  }
  return val;
}
module.exports = _path;