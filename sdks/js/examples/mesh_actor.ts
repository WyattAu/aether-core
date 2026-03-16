/**
 * Mesh Communication Actor Example
 * 
 * Demonstrates distributed messaging across the mesh network.
 */
import { Actor, Message, MessageType } from '@aether/sdk';
import * as crypto from 'crypto';

interface MeshNode {
    nodeId: string;
    region: string;
    endpoint: string;
    status: string;
    metadata: Record<string, string>;
}

interface MeshMessage {
    sourceNode: string;
    targetNode?: string;
    content: string;
    timestamp: string;
    hopCount: number;
}

class MeshActor extends Actor {
    private nodeId: string;
    private region: string;
    private knownNodes: Map<string, MeshNode> = new Map();
    private messageLog: MeshMessage[] = [];
    private isLeader: boolean = false;
    private leaderId?: string;

    constructor(region: string = 'local') {
        const nodeId = `node-${region}-${Math.floor(Math.random() * 9000) + 1000}`;
        super(`mesh-${nodeId}`);
        this.nodeId = nodeId;
        this.region = region;
        this.require('NETWORK_OUTBOUND', 'ACTOR_MESSAGING', 'LOG', 'TIME');
    }

    async onStart(): Promise<void> {
        console.log(`[${this.nodeId}] Starting mesh actor in region: ${this.region}`);
        
        // Register self
        this.knownNodes.set(this.nodeId, {
            nodeId: this.nodeId,
            region: this.region,
            endpoint: `localhost:${Math.floor(Math.random() * 1000) + 4000}`,
            status: 'active',
            metadata: { joined_at: new Date().toISOString() }
        });
    }

    async onStop(): Promise<void> {
        console.log(`[${this.nodeId}] Mesh actor stopping`);
        console.log(`[${this.nodeId}] Known nodes: ${this.knownNodes.size}, Messages: ${this.messageLog.length}`);
    }

    async handleMessage(sender: string, message: Message): Promise<Message | null> {
        if (message.type === MessageType.REQUEST) {
            return this.handleRequest(sender, message);
        } else if (message.type === MessageType.EVENT) {
            await this.handleEvent(sender, message);
            return null;
        } else if (message.type === MessageType.RESPONSE) {
            console.log(`[${this.nodeId}] Received response from ${sender}`);
            return null;
        }
        
        return Message.response({ error: 'unknown message type' });
    }

    private async handleRequest(sender: string, message: Message): Promise<Message> {
        const payload = message.payload as Record<string, any> | null;
        if (!payload || typeof payload !== 'object') {
            return Message.response({ error: 'invalid payload format' });
        }

        const action = payload.action as string || '';

        switch (action) {
            case 'ping':
                return Message.response({
                    action: 'pong',
                    node_id: this.nodeId,
                    region: this.region,
                    timestamp: new Date().toISOString(),
                    status: 'healthy'
                });

            case 'discover': {
                const nodes = Array.from(this.knownNodes.values()).map(n => ({
                    id: n.nodeId,
                    region: n.region,
                    status: n.status
                }));
                return Message.response({
                    action: 'discover_response',
                    node_id: this.nodeId,
                    known_nodes: nodes,
                    count: nodes.length
                });
            }

            case 'broadcast': {
                const content = payload.content as string || '';
                const source = payload.source_node as string || sender;
                const hopCount = (payload.hop_count as number) || 0;

                this.messageLog.push({
                    sourceNode: source,
                    content,
                    timestamp: new Date().toISOString(),
                    hopCount
                });

                console.log(`[${this.nodeId}] Broadcast from ${source}: ${content.substring(0, 30)}...`);

                return Message.response({
                    action: 'broadcast_ack',
                    node_id: this.nodeId,
                    received: true
                });
            }

            case 'direct_message': {
                const content = payload.content as string || '';
                const source = payload.source_node as string || sender;

                this.messageLog.push({
                    sourceNode: source,
                    targetNode: this.nodeId,
                    content,
                    timestamp: new Date().toISOString(),
                    hopCount: 0
                });

                console.log(`[${this.nodeId}] Direct message from ${source}: ${content}`);

                return Message.response({
                    action: 'direct_message_ack',
                    node_id: this.nodeId,
                    received: true,
                    timestamp: new Date().toISOString()
                });
            }

            case 'get_status':
                return Message.response({
                    action: 'status',
                    node_id: this.nodeId,
                    region: this.region,
                    status: 'active',
                    known_nodes: this.knownNodes.size,
                    messages_handled: this.messageLog.length,
                    is_leader: this.isLeader,
                    leader_id: this.leaderId
                });

            case 'elect_leader': {
                const candidateId = payload.candidate_id as string || '';
                
                // Simple election: highest node ID wins
                if (candidateId > this.nodeId) {
                    this.leaderId = candidateId;
                    this.isLeader = false;
                    console.log(`[${this.nodeId}] Acknowledging ${candidateId} as leader`);
                } else {
                    this.leaderId = this.nodeId;
                    this.isLeader = true;
                    console.log(`[${this.nodeId}] Claiming leadership`);
                }

                return Message.response({
                    action: 'election_vote',
                    voter_id: this.nodeId,
                    leader_id: this.leaderId,
                    is_leader: this.isLeader,
                    timestamp: new Date().toISOString()
                });
            }

            default:
                return Message.response({
                    error: `unknown action: ${action}`,
                    node_id: this.nodeId
                });
        }
    }

    private async handleEvent(sender: string, message: Message): Promise<void> {
        const payload = message.payload as Record<string, any> | null;
        if (!payload) return;

        const eventType = payload.type as string || '';

        switch (eventType) {
            case 'node_join': {
                const nodeData = payload.node as Record<string, any>;
                if (nodeData?.id) {
                    this.knownNodes.set(nodeData.id, {
                        nodeId: nodeData.id,
                        region: nodeData.region || '',
                        endpoint: nodeData.endpoint || '',
                        status: 'active',
                        metadata: {}
                    });
                    console.log(`[${this.nodeId}] Node joined: ${nodeData.id}`);
                }
                break;
            }

            case 'node_leave': {
                const nodeId = payload.node_id as string;
                if (nodeId && this.knownNodes.has(nodeId)) {
                    this.knownNodes.delete(nodeId);
                    console.log(`[${this.nodeId}] Node left: ${nodeId}`);
                    
                    // Trigger re-election if leader left
                    if (nodeId === this.leaderId) {
                        this.leaderId = undefined;
                        this.isLeader = false;
                        console.log(`[${this.nodeId}] Leader left, re-election needed`);
                    }
                }
                break;
            }
        }
    }
}

async function main(): Promise<void> {
    const region = process.env.AETHER_REGION || 'us-east-1';
    const actor = new MeshActor(region);

    console.log('Starting mesh actor...');
    console.log(`Node ID: ${actor['nodeId']}`);
    console.log(`Region: ${actor['region']}`);
    console.log('Supported actions: ping, discover, broadcast, direct_message, get_status, elect_leader');

    // Handle shutdown
    process.on('SIGINT', async () => {
        console.log('Shutting down...');
        await actor.stop();
        process.exit(0);
    });

    try {
        await actor.start();
        await actor.run();
    } catch (error) {
        console.error('Actor error:', error);
        process.exit(1);
    }
}

main();
