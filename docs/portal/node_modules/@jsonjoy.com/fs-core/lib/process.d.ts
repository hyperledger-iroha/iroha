export interface IProcess {
    getuid?(): number;
    getgid?(): number;
    cwd(): string;
    platform: string;
    emitWarning: (message: string, type: string) => void;
    env: Record<string, string | undefined>;
}
export declare function createProcess(): IProcess;
declare const _default: IProcess;
export default _default;
