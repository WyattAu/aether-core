/**
 * Chat Application Example
 * 
 * Demonstrates a multi-actor chat room application with room and session actors.
 */
import { Actor, Message, MessageType, State } from '@aether/sdk';

// ============================================
// Types
// ============================================

interface User {
    user_id: string;
    name: string;
    joined_at: string;
}

interface ChatMessage {
    id: string;
    user_id: string;
    user_name: string;
    content: string;
    timestamp: string;
    room_id: string;
}

interface RoomState {
    room_id: string;
    room_name: string;
    users: User[];
    messages: ChatMessage[];
}

// ============================================
// Room Actor
// ============================================

class RoomActor extends Actor {
    private roomId: string;
    private roomName: string;
    private users: Map<string, User> = new Map();
    private messages: ChatMessage[] = [];
    private stateKey: string;
    private state: State;

    constructor(roomId: string, roomName: string) {
        super(`room-${roomId}`);
        this.roomId = roomId;
        this.roomName = roomName;
        this.stateKey = `room_${roomId}_state`;
        this.state = new State();
        this.require('STATE_READ', 'STATE_WRITE', 'ACTOR_MESSAGING', 'LOG', 'TIME');
    }

    async onStart(): Promise<void> {
        console.log(`[${this.name}] Room '${this.roomName}' starting...`);

        // Load persisted state
        try {
            const data = await this.state.read(this.stateKey);
            if (data) {
                const state: RoomState = JSON.parse(data);
                
                for (const u of state.users || []) {
                    this.users.set(u.user_id, u);
                }
                
                for (const m of state.messages || []) {
                    this.messages.push(m);
                }
                
                console.log(`[${this.name}] Restored ${this.users.size} users, ${this.messages.length} messages`);
            } else {
                await this.saveState();
                console.log(`[${this.name}] Initialized new room`);
            }
        } catch (error) {
            console.error(`[${this.name}] Failed to load state:`, error);
            await this.saveState();
        }
    }

    async onStop(): Promise<void> {
        console.log(`[${this.name}] Room stopping, saving state...`);
        await this.saveState();
    }

    async handleMessage(sender: string, message: Message): Promise<Message | null> {
        if (message.type !== MessageType.REQUEST && message.type !== MessageType.RPC_REQUEST) {
            return null;
        }

        const payload = message.payload as Record<string, any> | null;
        if (!payload || typeof payload !== 'object') {
            return Message.response({ error: 'invalid payload' });
        }

        const action = payload.action || '';

        switch (action) {
            case 'join':
                return this.handleJoin(payload);
            case 'leave':
                return this.handleLeave(payload);
            case 'send':
                return this.handleSend(payload);
            case 'history':
                return this.handleHistory(payload);
            case 'users':
                return this.handleUsers();
            case 'info':
                return this.handleInfo();
            default:
                return Message.response({ error: `unknown action: ${action}` });
        }
    }

    private async handleJoin(payload: Record<string, any>): Promise<Message> {
        const userId = payload.user_id || '';
        const userName = payload.user_name || '';

        if (!userId || !userName) {
            return Message.response({ error: 'user_id and user_name required' });
        }

        const user: User = {
            user_id: userId,
            name: userName,
            joined_at: new Date().toISOString()
        };

        this.users.set(userId, user);
        await this.saveState();

        console.log(`[${this.name}] User '${userName}' joined (total: ${this.users.size})`);

        return Message.response({
            action: 'joined',
            room_id: this.roomId,
            room_name: this.roomName,
            user_id: userId,
            user_count: this.users.size
        });
    }

    private async handleLeave(payload: Record<string, any>): Promise<Message> {
        const userId = payload.user_id || '';

        const user = this.users.get(userId);
        if (user) {
            this.users.delete(userId);
            await this.saveState();
            console.log(`[${this.name}] User '${user.name}' left (remaining: ${this.users.size})`);
        }

        return Message.response({
            action: 'left',
            room_id: this.roomId,
            user_id: userId,
            user_count: this.users.size
        });
    }

    private async handleSend(payload: Record<string, any>): Promise<Message> {
        const userId = payload.user_id || '';
        const content = payload.content || '';

        if (!userId || !content) {
            return Message.response({ error: 'user_id and content required' });
        }

        const user = this.users.get(userId);
        if (!user) {
            return Message.response({ error: 'user not in room, join first' });
        }

        const msg: ChatMessage = {
            id: `msg-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
            user_id: userId,
            user_name: user.name,
            content: content,
            timestamp: new Date().toISOString(),
            room_id: this.roomId
        };

        this.messages.push(msg);
        await this.saveState();

        console.log(`[${this.name}] [${msg.id.substring(0, 12)}] ${user.name}: ${content.substring(0, 30)}...`);

        return Message.response({
            action: 'sent',
            message_id: msg.id,
            message_count: this.messages.length
        });
    }

    private handleHistory(payload: Record<string, any>): Message {
        const limit = payload.limit || 50;
        const start = Math.max(0, this.messages.length - limit);
        const messages = this.messages.slice(start);

        return Message.response({
            action: 'history',
            room_id: this.roomId,
            messages: messages,
            count: messages.length
        });
    }

    private handleUsers(): Message {
        const users = Array.from(this.users.values());
        return Message.response({
            action: 'users',
            room_id: this.roomId,
            users: users,
            count: users.length
        });
    }

    private handleInfo(): Message {
        return Message.response({
            action: 'info',
            room_id: this.roomId,
            room_name: this.roomName,
            user_count: this.users.size,
            message_count: this.messages.length
        });
    }

    private async saveState(): Promise<void> {
        const state: RoomState = {
            room_id: this.roomId,
            room_name: this.roomName,
            users: Array.from(this.users.values()),
            messages: this.messages
        };
        await this.state.write(this.stateKey, JSON.stringify(state));
    }
}

// ============================================
// Session Actor
// ============================================

class SessionActor extends Actor {
    private userId: string;
    private userName: string;
    private rooms: Set<string> = new Set();

    constructor(userId: string, userName: string) {
        super(`session-${userId}`);
        this.userId = userId;
        this.userName = userName;
        this.require('ACTOR_MESSAGING', 'LOG');
    }

    async onStart(): Promise<void> {
        console.log(`[${this.name}] Session started for user '${this.userName}'`);
    }

    async onStop(): Promise<void> {
        console.log(`[${this.name}] Session ended for user '${this.userName}'`);
    }

    async handleMessage(sender: string, message: Message): Promise<Message | null> {
        if (message.type !== MessageType.REQUEST && message.type !== MessageType.RPC_REQUEST) {
            return null;
        }

        const payload = message.payload as Record<string, any> | null;
        if (!payload || typeof payload !== 'object') {
            return Message.response({ error: 'invalid payload' });
        }

        const action = payload.action || '';

        switch (action) {
            case 'status':
                return Message.response({
                    action: 'status',
                    user_id: this.userId,
                    user_name: this.userName,
                    rooms: Array.from(this.rooms)
                });
            case 'join_room':
                return this.handleJoinRoom(payload);
            case 'leave_room':
                return this.handleLeaveRoom(payload);
            default:
                return Message.response({ error: `unknown action: ${action}` });
        }
    }

    private handleJoinRoom(payload: Record<string, any>): Message {
        const roomId = payload.room_id || '';
        this.rooms.add(roomId);
        console.log(`[${this.name}] Joined room '${roomId}' (total: ${this.rooms.size})`);
        return Message.response({
            action: 'joined_room',
            room_id: roomId,
            room_count: this.rooms.size
        });
    }

    private handleLeaveRoom(payload: Record<string, any>): Message {
        const roomId = payload.room_id || '';
        this.rooms.delete(roomId);
        console.log(`[${this.name}] Left room '${roomId}' (remaining: ${this.rooms.size})`);
        return Message.response({
            action: 'left_room',
            room_id: roomId,
            room_count: this.rooms.size
        });
    }
}

// ============================================
// Chat Application
// ============================================

class ChatApp {
    private roomActor: RoomActor | null = null;
    private sessionActor: SessionActor | null = null;
    private running: boolean = false;

    async start(): Promise<void> {
        // Create actors
        this.roomActor = new RoomActor('general', 'General Chat');
        this.sessionActor = new SessionActor('demo-user', 'Demo User');

        // Start actors
        await this.roomActor.start();
        await this.sessionActor.start();

        // Demo: Auto-join the room
        await this.sessionActor.handleMessage('system', Message.request({
            action: 'join_room',
            room_id: 'general'
        }));

        await this.roomActor.handleMessage('system', Message.request({
            action: 'join',
            user_id: 'demo-user',
            user_name: 'Demo User'
        }));

        this.running = true;
        console.log('=== Aether Chat Application ===');
        console.log("Room: 'General Chat', User: 'Demo User'");
        console.log('Commands: join, leave, send, history, users, info, status, quit');
    }

    async stop(): Promise<void> {
        this.running = false;
        if (this.roomActor) {
            await this.roomActor.stop();
        }
        if (this.sessionActor) {
            await this.sessionActor.stop();
        }
    }

    async demo(): Promise<void> {
        // Wait for startup
        await this.delay(500);

        // Send a welcome message
        if (this.roomActor) {
            await this.roomActor.handleMessage('demo-user', Message.request({
                action: 'send',
                user_id: 'demo-user',
                content: 'Hello, Aether Chat!'
            }));
        }

        await this.delay(300);

        // Get room info
        if (this.roomActor) {
            const info = await this.roomActor.handleMessage('demo-user', Message.request({
                action: 'info'
            }));
            console.log('Room info:', info?.payload);
        }

        await this.delay(300);

        // Get message history
        if (this.roomActor) {
            const history = await this.roomActor.handleMessage('demo-user', Message.request({
                action: 'history',
                limit: 10
            }));
            const payload = history?.payload as Record<string, any> | null;
            console.log('Message count:', payload?.count || 0);
        }

        // Keep running
        while (this.running) {
            await this.delay(1000);
        }
    }

    private delay(ms: number): Promise<void> {
        return new Promise(resolve => setTimeout(resolve, ms));
    }
}

// ============================================
// Main Entry Point
// ============================================

async function main(): Promise<void> {
    const app = new ChatApp();

    // Handle shutdown
    process.on('SIGINT', async () => {
        console.log('\nShutting down...');
        await app.stop();
        process.exit(0);
    });

    try {
        await app.start();
        await app.demo();
    } catch (error) {
        console.error('Application error:', error);
        await app.stop();
        process.exit(1);
    }
}

main();
