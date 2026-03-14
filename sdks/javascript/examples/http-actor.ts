import { Actor, Message, MessageType, Capability, HttpClient } from '../src';

class HttpActor extends Actor {
    static get name() { return 'http-client'; }
    private httpClient!: HttpClient;

    async onStart(): Promise<void> {
        this.require(Capability.NETWORK_OUTBOUND, Capability.HTTP_CLIENT);
        this.httpClient = new HttpClient(this.capabilities, 5000);
    }

    async handle(sender: string, message: Message): Promise<Message | void> {
        if (message.type === MessageType.CUSTOM && message.payload.url) {
            try {
                const response = await this.httpClient.get(message.payload.url);
                return Message.custom({ status: response.status, body: response.body });
            } catch (error) {
                return Message.custom({ error: String(error) });
            }
        }
    }
}

const actor = new HttpActor({ name: 'http-client' });
actor.start().catch(console.error);
