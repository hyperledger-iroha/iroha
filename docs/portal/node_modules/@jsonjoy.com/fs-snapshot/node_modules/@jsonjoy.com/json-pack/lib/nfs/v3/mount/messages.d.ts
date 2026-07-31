import type { MountStat } from './constants';
import type * as stucts from './structs';
/**
 * MOUNT protocol messages (Appendix I)
 */
export type MountMessage = MountRequest | MountResponse;
export type MountRequest = MountMntRequest | MountUmntRequest | MountDumpRequest | MountUmntallRequest | MountExportRequest;
export type MountResponse = MountMntResponse | MountDumpResponse | MountExportResponse;
/**
 * MNT request
 */
export declare class MountMntRequest {
    readonly dirpath: string;
    constructor(dirpath: string);
}
/**
 * MNT response - success case
 */
export declare class MountMntResOk {
    readonly fhandle: stucts.MountFhandle3;
    readonly authFlavors: number[];
    constructor(fhandle: stucts.MountFhandle3, authFlavors: number[]);
}
/**
 * MNT response
 */
export declare class MountMntResponse {
    readonly status: MountStat;
    readonly mountinfo?: MountMntResOk | undefined;
    constructor(status: MountStat, mountinfo?: MountMntResOk | undefined);
}
/**
 * DUMP request (void - no arguments)
 */
export declare class MountDumpRequest {
}
/**
 * DUMP response
 */
export declare class MountDumpResponse {
    readonly mountlist?: stucts.MountBody | undefined;
    constructor(mountlist?: stucts.MountBody | undefined);
}
/**
 * UMNT request
 */
export declare class MountUmntRequest {
    readonly dirpath: string;
    constructor(dirpath: string);
}
/**
 * UMNTALL request (void - no arguments)
 */
export declare class MountUmntallRequest {
}
/**
 * EXPORT request (void - no arguments)
 */
export declare class MountExportRequest {
}
/**
 * EXPORT response
 */
export declare class MountExportResponse {
    readonly exports?: stucts.MountExportNode | undefined;
    constructor(exports?: stucts.MountExportNode | undefined);
}
