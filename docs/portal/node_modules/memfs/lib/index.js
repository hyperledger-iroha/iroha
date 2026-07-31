"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.memfs = exports.fs = exports.vol = exports.Volume = void 0;
exports.createFsFromVolume = createFsFromVolume;
const fs_node_1 = require("@jsonjoy.com/fs-node");
Object.defineProperty(exports, "Volume", { enumerable: true, get: function () { return fs_node_1.Volume; } });
const fs_node_utils_1 = require("@jsonjoy.com/fs-node-utils");
const { F_OK, R_OK, W_OK, X_OK } = fs_node_utils_1.constants;
// Default volume.
exports.vol = new fs_node_1.Volume();
function createFsFromVolume(vol) {
    const fs = { F_OK, R_OK, W_OK, X_OK, constants: fs_node_utils_1.constants, Stats: fs_node_1.Stats, Dirent: fs_node_1.Dirent };
    // Bind FS methods.
    for (const method of fs_node_1.fsSynchronousApiList)
        if (typeof vol[method] === 'function')
            fs[method] = vol[method].bind(vol);
    for (const method of fs_node_1.fsCallbackApiList)
        if (typeof vol[method] === 'function')
            fs[method] = vol[method].bind(vol);
    fs.StatWatcher = vol.StatWatcher;
    fs.FSWatcher = vol.FSWatcher;
    fs.WriteStream = vol.WriteStream;
    fs.ReadStream = vol.ReadStream;
    fs.promises = vol.promises;
    // Handle realpath and realpathSync with their .native properties
    if (typeof vol.realpath === 'function') {
        fs.realpath = vol.realpath.bind(vol);
        if (typeof vol.realpath.native === 'function') {
            fs.realpath.native = vol.realpath.native.bind(vol);
        }
    }
    if (typeof vol.realpathSync === 'function') {
        fs.realpathSync = vol.realpathSync.bind(vol);
        if (typeof vol.realpathSync.native === 'function') {
            fs.realpathSync.native = vol.realpathSync.native.bind(vol);
        }
    }
    fs._toUnixTimestamp = fs_node_1.toUnixTimestamp;
    fs.__vol = vol;
    return fs;
}
exports.fs = createFsFromVolume(exports.vol);
/**
 * Creates a new file system instance.
 *
 * @param json File system structure expressed as a JSON object.
 *        Use `null` for empty directories and empty string for empty files.
 * @param cwdOrOpts Current working directory (string) or options object.
 *        The JSON structure will be created relative to the cwd path.
 * @returns A `memfs` file system instance, which is a drop-in replacement for
 *          the `fs` module.
 */
const memfs = (json = {}, cwdOrOpts = '/') => {
    const opts = typeof cwdOrOpts === 'string' ? { cwd: cwdOrOpts } : cwdOrOpts;
    // When no explicit cwd is given but a custom process is provided, let the
    // Superblock use that process's cwd(). Otherwise default to '/' so the
    // convenience function keeps its opinionated virtual-root default.
    const cwd = opts.cwd ?? (opts.process ? undefined : '/');
    const vol = fs_node_1.Volume.fromNestedJSON(json, cwd, { process: opts.process });
    const fs = createFsFromVolume(vol);
    return { fs, vol };
};
exports.memfs = memfs;
module.exports = { ...module.exports, ...exports.fs };
module.exports.semantic = true;
//# sourceMappingURL=index.js.map