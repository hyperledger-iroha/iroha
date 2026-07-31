import type { Nfsv4Connection } from './Nfsv4Connection';
import * as msg from '../messages';
/**
 * NFS v4 COMPOUND Procedure Context, holds state for a single COMPOUND procedure
 * call. This state is injected into each operation handler called as part of
 * the COMPOUND procedure.
 */
export declare class Nfsv4CompoundProcCtx {
    readonly connection: Nfsv4Connection;
    readonly req: msg.Nfsv4CompoundRequest;
    /** Current file handle */
    cfh: Uint8Array | null;
    /** Saved file handle */
    sfh: Uint8Array | null;
    constructor(connection: Nfsv4Connection, req: msg.Nfsv4CompoundRequest);
    /**
     * Returns the principal associated with the current RPC call. For now,
     * this is a stub returning "none" since we don't have real authentication.
     * In a real implementation, this would extract the principal from the RPC
     * credentials in the connection's current RPC call context.
     *
     * - AUTH_NONE -> `none`
     * - AUTH_SYS -> `sys:machinename:uid`
     * - GSS -> `gss:client@REALM`
     *
     * @returns The principal associated with the current RPC call.
     */
    getPrincipal(): string;
    exec(): Promise<msg.Nfsv4CompoundResponse>;
}
