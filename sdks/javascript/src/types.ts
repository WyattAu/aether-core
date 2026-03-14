export interface ActorConfig {
    name: string;
    capabilities?: number;
    state?: Map<string, Buffer>;
}

export interface MessageHandler {
    (sender: string, message: any): Promise<any | void>;
}

export interface RpcHandler {
    (request: any): Promise<any>;
}
