/**
 * Real-Time Chat Application using the Aether Actor Model.
 *
 * This example demonstrates:
 *   - Actor-based chat rooms (each room is an actor)
 *   - Message routing between users
 *   - State management (room members, message history)
 *   - Capabilities (messaging capability required)
 *   - Error handling (unknown rooms, full rooms)
 *
 * Each ChatRoom is an Actor that manages a list of members and a message
 * history. Users interact with rooms by sending Message objects. Because
 * the JS SDK's Actor.send() is a placeholder, we route messages directly
 * via handle() to keep the example self-contained and runnable.
 *
 * Usage:
 *   npx ts-node examples/chat-application.ts
 */

import { Actor, Message, MessageType, Capability, StateHandle } from '../src';

// -------------------------------------------------------------------
// Types
// -------------------------------------------------------------------

interface ChatMember {
    username: string;
}

interface ChatMsg {
    sender: string;
    text: string;
}

interface RoomState {
    members: string[];
    messages: ChatMsg[];
}

const MAX_ROOM_MEMBERS = 5;

// -------------------------------------------------------------------
// ChatRoom Actor
// -------------------------------------------------------------------

/**
 * An actor that represents a single chat room.
 *
 * Capabilities:
 *   - ACTOR_MESSAGING: required to send/receive messages
 *   - STATE_READ / STATE_WRITE: required to persist room state
 */
class ChatRoom extends Actor {
    private roomName: string;

    constructor(config: { name: string; roomName: string }) {
        super(config);
        this.roomName = config.roomName;
        this.require(
            Capability.ACTOR_MESSAGING,
            Capability.STATE_READ,
            Capability.STATE_WRITE,
        );
    }

    static override get name(): string {
        return 'chat_room';
    }

    protected override async onStart(): Promise<void> {
        await this.state.setJSON<RoomState>('room', { members: [], messages: [] });
        console.log(`[Room '${this.roomName}'] Started.`);
    }

    /**
     * Handle an incoming message.  Dispatches by the "action" field in
     * the payload and returns a response Message.
     */
    override async handle(sender: string, message: Message): Promise<Message | void> {
        const action = message.payload?.action;

        switch (action) {
            case 'join':
                return this.handleJoin(sender);
            case 'leave':
                return this.handleLeave(sender);
            case 'say':
                return this.handleSay(sender, message.payload?.text);
            case 'history':
                return this.handleHistory();
            case 'members':
                return this.handleMembers();
            default:
                return Message.custom({ error: `Unknown action: ${action}` });
        }
    }

    // -------------------------------------------------------------------
    // Action handlers
    // -------------------------------------------------------------------

    private async handleJoin(sender: string): Promise<Message> {
        const room = await this.getRoomState();

        if (room.members.includes(sender)) {
            return Message.custom({ info: `'${sender}' is already in the room.` });
        }

        if (room.members.length >= MAX_ROOM_MEMBERS) {
            return Message.custom({
                error: `Room is full (${MAX_ROOM_MEMBERS} members max).`,
            });
        }

        room.members.push(sender);
        await this.state.setJSON('room', room);
        await this.appendMessage('system', `'${sender}' joined the room.`);

        console.log(`[Room '${this.roomName}'] ${sender} joined. (${room.members.length} members)`);
        return Message.custom({ info: `'${sender}' joined '${this.roomName}'.` });
    }

    private async handleLeave(sender: string): Promise<Message> {
        const room = await this.getRoomState();

        if (!room.members.includes(sender)) {
            return Message.custom({ error: `'${sender}' is not in the room.` });
        }

        room.members = room.members.filter(m => m !== sender);
        await this.state.setJSON('room', room);
        await this.appendMessage('system', `'${sender}' left the room.`);

        console.log(`[Room '${this.roomName}'] ${sender} left. (${room.members.length} members)`);
        return Message.custom({ info: `'${sender}' left '${this.roomName}'.` });
    }

    private async handleSay(sender: string, text: string): Promise<Message> {
        const room = await this.getRoomState();

        if (!room.members.includes(sender)) {
            return Message.custom({ error: `'${sender}' is not a member of the room.` });
        }

        if (!text) {
            return Message.custom({ error: 'Message text cannot be empty.' });
        }

        await this.appendMessage(sender, text);
        console.log(`[Room '${this.roomName}'] ${sender}: ${text}`);

        return Message.custom({ info: 'Message delivered.' });
    }

    private async handleHistory(): Promise<Message> {
        const room = await this.getRoomState();
        return Message.custom({ history: room.messages });
    }

    private async handleMembers(): Promise<Message> {
        const room = await this.getRoomState();
        return Message.custom({ members: [...room.members] });
    }

    // -------------------------------------------------------------------
    // Helpers
    // -------------------------------------------------------------------

    private async getRoomState(): Promise<RoomState> {
        const room = await this.state.getJSON<RoomState>('room');
        return room ?? { members: [], messages: [] };
    }

    private async appendMessage(sender: string, text: string): Promise<void> {
        const room = await this.getRoomState();
        room.messages.push({ sender, text });
        await this.state.setJSON('room', room);
    }
}

// -------------------------------------------------------------------
// Chat Server (in-process router)
// -------------------------------------------------------------------

/**
 * A simple in-process router that simulates the actor system.
 * In a real deployment the actor system handles routing automatically.
 */
class ChatServer {
    private rooms: Map<string, ChatRoom> = new Map();

    async createRoom(roomName: string): Promise<ChatRoom> {
        const room = new ChatRoom({ name: 'chat_room', roomName });
        this.rooms.set(roomName, room);
        await room.start();
        return room;
    }

    async send(roomName: string, sender: string, message: Message): Promise<Message> {
        const room = this.rooms.get(roomName);
        if (!room) {
            return Message.custom({ error: `Room '${roomName}' does not exist.` });
        }
        const resp = await room.handle(sender, message);
        return resp || Message.custom({ info: 'No response.' });
    }
}

// -------------------------------------------------------------------
// Main demo
// -------------------------------------------------------------------

async function main(): Promise<void> {
    console.log('='.repeat(60));
    console.log('  Aether SDK - Real-Time Chat Application Example');
    console.log('='.repeat(60));
    console.log();

    const server = new ChatServer();

    // Create a chat room
    await server.createRoom('general');
    console.log();

    // --- Scenario 1: Users join the room ---
    console.log('--- Users Joining ---');
    for (const user of ['alice', 'bob', 'charlie']) {
        const resp = await server.send(
            'general',
            user,
            Message.custom({ action: 'join' }),
        );
        console.log(`  -> ${user}:`, resp.payload);
    }
    console.log();

    // --- Scenario 2: Chat messages ---
    console.log('--- Chat Messages ---');
    const chatMessages: [string, string][] = [
        ['alice', 'Hey everyone! Welcome to the room.'],
        ['bob', 'Thanks Alice! Happy to be here.'],
        ['charlie', 'This actor model is pretty cool.'],
        ['alice', 'Agreed! Each room is its own actor with isolated state.'],
    ];
    for (const [sender, text] of chatMessages) {
        const resp = await server.send(
            'general',
            sender,
            Message.custom({ action: 'say', text }),
        );
        console.log(`  -> ${sender}:`, resp.payload);
    }
    console.log();

    // --- Scenario 3: Retrieve message history ---
    console.log('--- Message History ---');
    const historyResp = await server.send(
        'general',
        'alice',
        Message.custom({ action: 'history' }),
    );
    for (const msg of historyResp.payload.history as ChatMsg[]) {
        console.log(`  [${msg.sender}] ${msg.text}`);
    }
    console.log();

    // --- Scenario 4: List members ---
    console.log('--- Room Members ---');
    const membersResp = await server.send(
        'general',
        'alice',
        Message.custom({ action: 'members' }),
    );
    console.log('  Members:', membersResp.payload.members);
    console.log();

    // --- Scenario 5: User leaves ---
    console.log('--- User Leaves ---');
    const leaveResp = await server.send(
        'general',
        'bob',
        Message.custom({ action: 'leave' }),
    );
    console.log('  -> bob:', leaveResp.payload);
    console.log();

    // --- Scenario 6: Error - non-member tries to speak ---
    console.log('--- Error: Non-member speaks ---');
    const errResp = await server.send(
        'general',
        'bob',
        Message.custom({ action: 'say', text: 'Can I still talk?' }),
    );
    console.log('  -> bob:', errResp.payload);
    console.log();

    // --- Scenario 7: Error - room does not exist ---
    console.log('--- Error: Unknown room ---');
    const unknownResp = await server.send(
        'random',
        'alice',
        Message.custom({ action: 'join' }),
    );
    console.log('  -> alice:', unknownResp.payload);
    console.log();

    // --- Scenario 8: Error - room is full ---
    console.log('--- Error: Room full ---');
    await server.createRoom('small');
    for (let i = 1; i <= MAX_ROOM_MEMBERS; i++) {
        await server.send('small', `u${i}`, Message.custom({ action: 'join' }));
    }
    const fullResp = await server.send('small', 'u6', Message.custom({ action: 'join' }));
    console.log('  -> u6:', fullResp.payload);
    console.log();

    console.log('='.repeat(60));
    console.log('  Chat application demo complete!');
    console.log('='.repeat(60));
}

// Run the demo
main().catch(console.error);
