"use strict";
/**
 * Attribute encoding utilities for NFSv4 server operations.
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.encodeAttrs = void 0;
const tslib_1 = require("tslib");
const Writer_1 = require("@jsonjoy.com/buffers/lib/Writer");
const XdrEncoder_1 = require("../../../../../xdr/XdrEncoder");
const struct = tslib_1.__importStar(require("../../../structs"));
const attributes_1 = require("../../../attributes");
/**
 * Encodes file attributes based on the requested bitmap.
 * Returns the attributes as a Nfsv4Fattr structure.
 * @param requestedAttrs Bitmap of requested attributes
 * @param stats Optional file stats (required only if stat-based attributes are requested)
 * @param path File path (for context)
 * @param fh Optional file handle (required only if FATTR4_FILEHANDLE is requested)
 * @param leaseTime Optional lease time in seconds (required only if FATTR4_LEASE_TIME is requested)
 * @param fsStats Optional filesystem statistics (required for space/files attributes)
 */
const encodeAttrs = (requestedAttrs, stats, path, fh, leaseTime, fsStats) => {
    const writer = new Writer_1.Writer(512);
    const xdr = new XdrEncoder_1.XdrEncoder(writer);
    const supportedMask = [];
    const requested = requestedAttrs.mask;
    for (let i = 0; i < requested.length; i++) {
        const word = requested[i];
        if (!word)
            continue;
        const wordIndex = i;
        for (let bit = 0; bit < 32; bit++) {
            if (!(word & (1 << bit)))
                continue;
            const attrNum = wordIndex * 32 + bit;
            switch (attrNum) {
                case 0 /* Nfsv4Attr.FATTR4_SUPPORTED_ATTRS */: {
                    const implementedAttrs = [];
                    (0, attributes_1.setBit)(implementedAttrs, 0 /* Nfsv4Attr.FATTR4_SUPPORTED_ATTRS */);
                    (0, attributes_1.setBit)(implementedAttrs, 1 /* Nfsv4Attr.FATTR4_TYPE */);
                    (0, attributes_1.setBit)(implementedAttrs, 2 /* Nfsv4Attr.FATTR4_FH_EXPIRE_TYPE */);
                    (0, attributes_1.setBit)(implementedAttrs, 3 /* Nfsv4Attr.FATTR4_CHANGE */);
                    (0, attributes_1.setBit)(implementedAttrs, 4 /* Nfsv4Attr.FATTR4_SIZE */);
                    (0, attributes_1.setBit)(implementedAttrs, 5 /* Nfsv4Attr.FATTR4_LINK_SUPPORT */);
                    (0, attributes_1.setBit)(implementedAttrs, 6 /* Nfsv4Attr.FATTR4_SYMLINK_SUPPORT */);
                    (0, attributes_1.setBit)(implementedAttrs, 7 /* Nfsv4Attr.FATTR4_NAMED_ATTR */);
                    (0, attributes_1.setBit)(implementedAttrs, 8 /* Nfsv4Attr.FATTR4_FSID */);
                    (0, attributes_1.setBit)(implementedAttrs, 9 /* Nfsv4Attr.FATTR4_UNIQUE_HANDLES */);
                    (0, attributes_1.setBit)(implementedAttrs, 10 /* Nfsv4Attr.FATTR4_LEASE_TIME */);
                    (0, attributes_1.setBit)(implementedAttrs, 11 /* Nfsv4Attr.FATTR4_RDATTR_ERROR */);
                    (0, attributes_1.setBit)(implementedAttrs, 19 /* Nfsv4Attr.FATTR4_FILEHANDLE */);
                    (0, attributes_1.setBit)(implementedAttrs, 20 /* Nfsv4Attr.FATTR4_FILEID */);
                    (0, attributes_1.setBit)(implementedAttrs, 33 /* Nfsv4Attr.FATTR4_MODE */);
                    (0, attributes_1.setBit)(implementedAttrs, 35 /* Nfsv4Attr.FATTR4_NUMLINKS */);
                    (0, attributes_1.setBit)(implementedAttrs, 45 /* Nfsv4Attr.FATTR4_SPACE_USED */);
                    (0, attributes_1.setBit)(implementedAttrs, 42 /* Nfsv4Attr.FATTR4_SPACE_AVAIL */);
                    (0, attributes_1.setBit)(implementedAttrs, 43 /* Nfsv4Attr.FATTR4_SPACE_FREE */);
                    (0, attributes_1.setBit)(implementedAttrs, 44 /* Nfsv4Attr.FATTR4_SPACE_TOTAL */);
                    (0, attributes_1.setBit)(implementedAttrs, 21 /* Nfsv4Attr.FATTR4_FILES_AVAIL */);
                    (0, attributes_1.setBit)(implementedAttrs, 22 /* Nfsv4Attr.FATTR4_FILES_FREE */);
                    (0, attributes_1.setBit)(implementedAttrs, 23 /* Nfsv4Attr.FATTR4_FILES_TOTAL */);
                    (0, attributes_1.setBit)(implementedAttrs, 47 /* Nfsv4Attr.FATTR4_TIME_ACCESS */);
                    (0, attributes_1.setBit)(implementedAttrs, 52 /* Nfsv4Attr.FATTR4_TIME_METADATA */);
                    (0, attributes_1.setBit)(implementedAttrs, 53 /* Nfsv4Attr.FATTR4_TIME_MODIFY */);
                    xdr.writeUnsignedInt(implementedAttrs.length);
                    for (let j = 0; j < implementedAttrs.length; j++) {
                        xdr.writeUnsignedInt(implementedAttrs[j]);
                    }
                    (0, attributes_1.setBit)(supportedMask, attrNum);
                    break;
                }
                case 1 /* Nfsv4Attr.FATTR4_TYPE */: {
                    if (!stats)
                        break;
                    let type;
                    if (stats.isFile())
                        type = 1 /* Nfsv4FType.NF4REG */;
                    else if (stats.isDirectory())
                        type = 2 /* Nfsv4FType.NF4DIR */;
                    else if (stats.isSymbolicLink())
                        type = 5 /* Nfsv4FType.NF4LNK */;
                    else if (stats.isBlockDevice())
                        type = 3 /* Nfsv4FType.NF4BLK */;
                    else if (stats.isCharacterDevice())
                        type = 4 /* Nfsv4FType.NF4CHR */;
                    else if (stats.isFIFO())
                        type = 7 /* Nfsv4FType.NF4FIFO */;
                    else if (stats.isSocket())
                        type = 6 /* Nfsv4FType.NF4SOCK */;
                    else
                        type = 1 /* Nfsv4FType.NF4REG */;
                    xdr.writeUnsignedInt(type);
                    (0, attributes_1.setBit)(supportedMask, attrNum);
                    break;
                }
                case 4 /* Nfsv4Attr.FATTR4_SIZE */: {
                    if (!stats)
                        break;
                    xdr.writeUnsignedHyper(BigInt(stats.size));
                    (0, attributes_1.setBit)(supportedMask, attrNum);
                    break;
                }
                case 20 /* Nfsv4Attr.FATTR4_FILEID */: {
                    if (!stats)
                        break;
                    xdr.writeUnsignedHyper(BigInt(stats.ino));
                    (0, attributes_1.setBit)(supportedMask, attrNum);
                    break;
                }
                case 33 /* Nfsv4Attr.FATTR4_MODE */: {
                    if (!stats)
                        break;
                    xdr.writeUnsignedInt(stats.mode & 0o7777);
                    (0, attributes_1.setBit)(supportedMask, attrNum);
                    break;
                }
                case 35 /* Nfsv4Attr.FATTR4_NUMLINKS */: {
                    if (!stats)
                        break;
                    xdr.writeUnsignedInt(stats.nlink);
                    (0, attributes_1.setBit)(supportedMask, attrNum);
                    break;
                }
                case 45 /* Nfsv4Attr.FATTR4_SPACE_USED */: {
                    if (!stats)
                        break;
                    xdr.writeUnsignedHyper(BigInt(stats.blocks * 512));
                    (0, attributes_1.setBit)(supportedMask, attrNum);
                    break;
                }
                case 42 /* Nfsv4Attr.FATTR4_SPACE_AVAIL */: {
                    if (!fsStats)
                        break;
                    xdr.writeUnsignedHyper(fsStats.spaceAvail);
                    (0, attributes_1.setBit)(supportedMask, attrNum);
                    break;
                }
                case 43 /* Nfsv4Attr.FATTR4_SPACE_FREE */: {
                    if (!fsStats)
                        break;
                    xdr.writeUnsignedHyper(fsStats.spaceFree);
                    (0, attributes_1.setBit)(supportedMask, attrNum);
                    break;
                }
                case 44 /* Nfsv4Attr.FATTR4_SPACE_TOTAL */: {
                    if (!fsStats)
                        break;
                    xdr.writeUnsignedHyper(fsStats.spaceTotal);
                    (0, attributes_1.setBit)(supportedMask, attrNum);
                    break;
                }
                case 21 /* Nfsv4Attr.FATTR4_FILES_AVAIL */: {
                    if (!fsStats)
                        break;
                    xdr.writeUnsignedHyper(fsStats.filesAvail);
                    (0, attributes_1.setBit)(supportedMask, attrNum);
                    break;
                }
                case 22 /* Nfsv4Attr.FATTR4_FILES_FREE */: {
                    if (!fsStats)
                        break;
                    xdr.writeUnsignedHyper(fsStats.filesFree);
                    (0, attributes_1.setBit)(supportedMask, attrNum);
                    break;
                }
                case 23 /* Nfsv4Attr.FATTR4_FILES_TOTAL */: {
                    if (!fsStats)
                        break;
                    xdr.writeUnsignedHyper(fsStats.filesTotal);
                    (0, attributes_1.setBit)(supportedMask, attrNum);
                    break;
                }
                case 47 /* Nfsv4Attr.FATTR4_TIME_ACCESS */: {
                    if (!stats)
                        break;
                    const atime = stats.atimeMs;
                    const seconds = Math.floor(atime / 1000);
                    const nseconds = Math.floor((atime % 1000) * 1000000);
                    xdr.writeHyper(BigInt(seconds));
                    xdr.writeUnsignedInt(nseconds);
                    (0, attributes_1.setBit)(supportedMask, attrNum);
                    break;
                }
                case 53 /* Nfsv4Attr.FATTR4_TIME_MODIFY */: {
                    if (!stats)
                        break;
                    const mtime = stats.mtimeMs;
                    const seconds = Math.floor(mtime / 1000);
                    const nseconds = Math.floor((mtime % 1000) * 1000000);
                    xdr.writeHyper(BigInt(seconds));
                    xdr.writeUnsignedInt(nseconds);
                    (0, attributes_1.setBit)(supportedMask, attrNum);
                    break;
                }
                case 52 /* Nfsv4Attr.FATTR4_TIME_METADATA */: {
                    if (!stats)
                        break;
                    const ctime = stats.ctimeMs;
                    const seconds = Math.floor(ctime / 1000);
                    const nseconds = Math.floor((ctime % 1000) * 1000000);
                    xdr.writeHyper(BigInt(seconds));
                    xdr.writeUnsignedInt(nseconds);
                    (0, attributes_1.setBit)(supportedMask, attrNum);
                    break;
                }
                case 3 /* Nfsv4Attr.FATTR4_CHANGE */: {
                    if (!stats)
                        break;
                    const changeTime = BigInt(Math.floor(stats.mtimeMs * 1000000));
                    xdr.writeUnsignedHyper(changeTime);
                    (0, attributes_1.setBit)(supportedMask, attrNum);
                    break;
                }
                case 10 /* Nfsv4Attr.FATTR4_LEASE_TIME */: {
                    if (leaseTime !== undefined) {
                        xdr.writeUnsignedInt(leaseTime);
                        (0, attributes_1.setBit)(supportedMask, attrNum);
                    }
                    break;
                }
                case 2 /* Nfsv4Attr.FATTR4_FH_EXPIRE_TYPE */: {
                    xdr.writeUnsignedInt(2 /* Nfsv4FhExpireType.FH4_VOLATILE_ANY */);
                    (0, attributes_1.setBit)(supportedMask, attrNum);
                    break;
                }
                case 5 /* Nfsv4Attr.FATTR4_LINK_SUPPORT */: {
                    xdr.writeUnsignedInt(1);
                    (0, attributes_1.setBit)(supportedMask, attrNum);
                    break;
                }
                case 6 /* Nfsv4Attr.FATTR4_SYMLINK_SUPPORT */: {
                    xdr.writeUnsignedInt(1);
                    (0, attributes_1.setBit)(supportedMask, attrNum);
                    break;
                }
                case 7 /* Nfsv4Attr.FATTR4_NAMED_ATTR */: {
                    xdr.writeUnsignedInt(0);
                    (0, attributes_1.setBit)(supportedMask, attrNum);
                    break;
                }
                case 8 /* Nfsv4Attr.FATTR4_FSID */: {
                    xdr.writeUnsignedHyper(BigInt(0));
                    xdr.writeUnsignedHyper(BigInt(0));
                    (0, attributes_1.setBit)(supportedMask, attrNum);
                    break;
                }
                case 9 /* Nfsv4Attr.FATTR4_UNIQUE_HANDLES */: {
                    xdr.writeUnsignedInt(1);
                    (0, attributes_1.setBit)(supportedMask, attrNum);
                    break;
                }
                case 11 /* Nfsv4Attr.FATTR4_RDATTR_ERROR */: {
                    xdr.writeUnsignedInt(0);
                    (0, attributes_1.setBit)(supportedMask, attrNum);
                    break;
                }
                case 19 /* Nfsv4Attr.FATTR4_FILEHANDLE */: {
                    if (fh) {
                        xdr.writeVarlenOpaque(fh);
                        (0, attributes_1.setBit)(supportedMask, attrNum);
                    }
                    break;
                }
                default: {
                    if (attributes_1.SET_ONLY_ATTRS.has(attrNum))
                        throw 22 /* Nfsv4Stat.NFS4ERR_INVAL */;
                }
            }
        }
    }
    const attrVals = writer.flush();
    return new struct.Nfsv4Fattr(new struct.Nfsv4Bitmap(supportedMask), attrVals);
};
exports.encodeAttrs = encodeAttrs;
//# sourceMappingURL=attrs.js.map