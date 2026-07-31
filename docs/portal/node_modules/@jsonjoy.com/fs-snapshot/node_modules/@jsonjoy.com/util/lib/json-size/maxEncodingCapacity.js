"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.maxEncodingCapacity = void 0;
const maxEncodingCapacity = (value) => {
    switch (typeof value) {
        case 'number':
            return 22 /* MaxEncodingOverhead.Number */;
        case 'string':
            return 5 /* MaxEncodingOverhead.String */ + value.length * 5 /* MaxEncodingOverhead.StringLengthMultiplier */;
        case 'boolean':
            return 5 /* MaxEncodingOverhead.Boolean */;
        case 'object': {
            if (!value)
                return 4 /* MaxEncodingOverhead.Null */;
            // biome-ignore lint: fine name
            const constructor = value.constructor;
            switch (constructor) {
                case Array: {
                    const arr = value;
                    const length = arr.length;
                    let size = 5 /* MaxEncodingOverhead.Array */ + length * 1 /* MaxEncodingOverhead.ArrayElement */;
                    for (let i = arr.length - 1; i >= 0; i--)
                        size += (0, exports.maxEncodingCapacity)(arr[i]);
                    return size;
                }
                case Uint8Array: {
                    return 41 /* MaxEncodingOverhead.Binary */ + value.length * 2 /* MaxEncodingOverhead.BinaryLengthMultiplier */;
                }
                case Object: {
                    let size = 5 /* MaxEncodingOverhead.Object */;
                    const obj = value;
                    for (const key in obj)
                        if (
                        // biome-ignore lint: .hasOwnProperty access is intentional
                        obj.hasOwnProperty(key))
                            size += 2 /* MaxEncodingOverhead.ObjectElement */ + (0, exports.maxEncodingCapacity)(key) + (0, exports.maxEncodingCapacity)(obj[key]);
                    return size;
                }
                default:
                    return 45 /* MaxEncodingOverhead.Undefined */;
            }
        }
        case 'bigint':
            return 22 /* MaxEncodingOverhead.Number */;
        default:
            return 45 /* MaxEncodingOverhead.Undefined */;
    }
};
exports.maxEncodingCapacity = maxEncodingCapacity;
//# sourceMappingURL=maxEncodingCapacity.js.map