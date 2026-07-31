"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.MountEncoder = void 0;
const Writer_1 = require("@jsonjoy.com/buffers/lib/Writer");
const XdrEncoder_1 = require("../../../xdr/XdrEncoder");
const errors_1 = require("../errors");
class MountEncoder {
    constructor(writer = new Writer_1.Writer()) {
        this.writer = writer;
        this.xdr = new XdrEncoder_1.XdrEncoder(writer);
    }
    encodeMessage(message, proc, isRequest) {
        if (isRequest)
            this.writeRequest(message, proc);
        else
            this.writeResponse(message, proc);
        return this.writer.flush();
    }
    writeMessage(message, proc, isRequest) {
        if (isRequest)
            this.writeRequest(message, proc);
        else
            this.writeResponse(message, proc);
    }
    writeRequest(request, proc) {
        switch (proc) {
            case 0 /* MountProc.NULL */:
                return;
            case 1 /* MountProc.MNT */:
                return this.writeMntRequest(request);
            case 2 /* MountProc.DUMP */:
                return;
            case 3 /* MountProc.UMNT */:
                return this.writeUmntRequest(request);
            case 4 /* MountProc.UMNTALL */:
                return;
            case 5 /* MountProc.EXPORT */:
                return;
            default:
                throw new errors_1.Nfsv3EncodingError(`Unknown MOUNT procedure: ${proc}`);
        }
    }
    writeResponse(response, proc) {
        switch (proc) {
            case 0 /* MountProc.NULL */:
                return;
            case 1 /* MountProc.MNT */:
                return this.writeMntResponse(response);
            case 2 /* MountProc.DUMP */:
                return this.writeDumpResponse(response);
            case 3 /* MountProc.UMNT */:
                return;
            case 4 /* MountProc.UMNTALL */:
                return;
            case 5 /* MountProc.EXPORT */:
                return this.writeExportResponse(response);
            default:
                throw new errors_1.Nfsv3EncodingError(`Unknown MOUNT procedure: ${proc}`);
        }
    }
    writeFhandle3(fh) {
        const data = fh.data.uint8;
        this.xdr.writeVarlenOpaque(data);
    }
    writeDirpath(path) {
        this.xdr.writeStr(path);
    }
    writeMountBody(body) {
        const xdr = this.xdr;
        if (!body) {
            xdr.writeBoolean(false);
            return;
        }
        xdr.writeBoolean(true);
        xdr.writeStr(body.hostname);
        this.writeDirpath(body.directory);
        this.writeMountBody(body.next);
    }
    writeGroupNode(group) {
        const xdr = this.xdr;
        if (!group) {
            xdr.writeBoolean(false);
            return;
        }
        xdr.writeBoolean(true);
        xdr.writeStr(group.name);
        this.writeGroupNode(group.next);
    }
    writeExportNode(exportNode) {
        const xdr = this.xdr;
        if (!exportNode) {
            xdr.writeBoolean(false);
            return;
        }
        xdr.writeBoolean(true);
        this.writeDirpath(exportNode.dir);
        this.writeGroupNode(exportNode.groups);
        this.writeExportNode(exportNode.next);
    }
    writeMntRequest(req) {
        this.writeDirpath(req.dirpath);
    }
    writeMntResponse(res) {
        const xdr = this.xdr;
        xdr.writeUnsignedInt(res.status);
        if (res.status === 0 && res.mountinfo) {
            this.writeFhandle3(res.mountinfo.fhandle);
            xdr.writeUnsignedInt(res.mountinfo.authFlavors.length);
            for (const flavor of res.mountinfo.authFlavors) {
                xdr.writeUnsignedInt(flavor);
            }
        }
    }
    writeDumpResponse(res) {
        this.writeMountBody(res.mountlist);
    }
    writeUmntRequest(req) {
        this.writeDirpath(req.dirpath);
    }
    writeExportResponse(res) {
        this.writeExportNode(res.exports);
    }
}
exports.MountEncoder = MountEncoder;
//# sourceMappingURL=MountEncoder.js.map