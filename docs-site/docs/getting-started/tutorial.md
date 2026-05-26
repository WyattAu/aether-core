# Aether Tutorial: Building Your First Actor System

This tutorial guides you through building a complete actor-based application from scratch.

> **NOTE**: This tutorial uses SDK examples in Python, TypeScript, and Go. The Aether v2.0.0 runtime is Rust-native. For the Rust-first quick start, see the [Quick Start Guide](quickstart.md). The concepts described here apply to all SDKs. SDK-specific code examples will be updated in a future release.

## Prerequisites

- Rust 1.88+ (for native actor development) or one of the SDK languages below
- Basic understanding of async programming
- Aether CLI installed (`cargo install aether-cli`)

## Part 1: Hello World Actor

### Step 1: Create the Project

```bash
mkdir my-aether-app
cd my-aether-app
```

### Step 2: Install Dependencies

**JavaScript/TypeScript:**
```bash
npm init -y
npm install @aether/sdk typescript ts-node @types/node
```

**Python:**
```bash
pip install aether-sdk
```

**Go:**
```bash
go mod init my-aether-app
go get github.com/WyattAu/aether-core/sdks/go
```

### Step 3: Create Your First Actor

Create `hello_actor.ts` (or `.py` / `.go`):

```typescript
import { Actor, Message, MessageType } from '@aether/sdk';

class HelloActor extends Actor {
    constructor() {
        super('hello-actor');
        this.require('ACTOR_MESSAGING', 'LOG');
    }

    async onStart(): Promise<void> {
        console.log('Hello Actor is ready!');
    }

    async handleMessage(sender: string, message: Message): Promise<Message | null> {
        if (message.type === MessageType.REQUEST) {
            const name = (message.payload as any)?.name || 'World';
            return Message.response({ 
                greeting: `Hello, ${name}!` 
            });
        }
        return null;
    }
}

async function main() {
    const actor = new HelloActor();
    await actor.start();
    await actor.run();
}

main();
```

### Step 4: Run the Actor

```bash
# JavaScript
npx ts-node hello_actor.ts

# Python
python hello_actor.py

# Go
go run main.go
```

## Part 2: Stateful Actor

Let's add state persistence with a counter actor:

```typescript
import { Actor, Message, MessageType, State } from '@aether/sdk';

class CounterActor extends Actor {
    private count = 0;
    private stateKey = 'counter_state';
    private state: State;

    constructor() {
        super('counter-actor');
        this.state = new State();
        this.require('STATE_READ', 'STATE_WRITE', 'ACTOR_MESSAGING', 'LOG');
    }

    async onStart(): Promise<void> {
        // Load persisted state
        const data = await this.state.read(this.stateKey);
        if (data) {
            this.count = JSON.parse(data).count;
            console.log(`Restored count: ${this.count}`);
        }
    }

    async onStop(): Promise<void> {
        await this.saveState();
    }

    async handleMessage(sender: string, message: Message): Promise<Message | null> {
        const payload = message.payload as Record<string, any>;
        const action = payload?.action;

        switch (action) {
            case 'increment':
                this.count++;
                await this.saveState();
                return Message.response({ count: this.count });
            
            case 'decrement':
                this.count--;
                await this.saveState();
                return Message.response({ count: this.count });
            
            case 'get':
                return Message.response({ count: this.count });
        }

        return Message.response({ error: `Unknown action: ${action}` });
    }

    private async saveState(): Promise<void> {
        await this.state.write(this.stateKey, JSON.stringify({ count: this.count }));
    }
}
```

## Part 3: Actor Communication

Now let's create actors that communicate with each other:

```typescript
// Producer Actor
class ProducerActor extends Actor {
    constructor() {
        super('producer');
        this.require('ACTOR_MESSAGING', 'LOG');
    }

    async produce(item: any): Promise<void> {
        // Send to consumer
        const message = Message.request({
            action: 'process',
            item,
            timestamp: new Date().toISOString()
        });
        
        // In a real implementation, this would use actor messaging
        console.log(`Produced: ${JSON.stringify(item)}`);
    }
}

// Consumer Actor
class ConsumerActor extends Actor {
    private processedCount = 0;

    constructor() {
        super('consumer');
        this.require('ACTOR_MESSAGING', 'LOG');
    }

    async handleMessage(sender: string, message: Message): Promise<Message | null> {
        const payload = message.payload as Record<string, any>;
        
        if (payload?.action === 'process') {
            this.processedCount++;
            console.log(`Consumed item #${this.processedCount}: ${JSON.stringify(payload.item)}`);
            
            return Message.response({
                status: 'processed',
                count: this.processedCount
            });
        }
        
        return null;
    }
}
```

## Part 4: Multi-Actor Application

Let's build a complete application with multiple interacting actors:

```typescript
import { Actor, Message, MessageType, State } from '@aether/sdk';

// Order Actor - Manages orders
class OrderActor extends Actor {
    private orders: Map<string, any> = new Map();
    private state: State;

    constructor() {
        super('order-actor');
        this.state = new State();
        this.require('STATE_READ', 'STATE_WRITE', 'ACTOR_MESSAGING', 'LOG');
    }

    async handleMessage(sender: string, message: Message): Promise<Message | null> {
        const payload = message.payload as Record<string, any>;
        const action = payload?.action;

        switch (action) {
            case 'create':
                return this.createOrder(payload);
            case 'get':
                return this.getOrder(payload);
            case 'list':
                return this.listOrders();
            case 'update_status':
                return this.updateStatus(payload);
        }

        return Message.response({ error: `Unknown action: ${action}` });
    }

    private createOrder(payload: any): Message {
        const orderId = `order-${Date.now()}`;
        const order = {
            id: orderId,
            customer: payload.customer,
            items: payload.items || [],
            status: 'pending',
            createdAt: new Date().toISOString()
        };

        this.orders.set(orderId, order);
        this.saveState();

        console.log(`Created order ${orderId} for ${payload.customer}`);

        return Message.response({
            action: 'created',
            order_id: orderId,
            order
        });
    }

    private getOrder(payload: any): Message {
        const order = this.orders.get(payload.order_id);
        
        if (!order) {
            return Message.response({ error: 'Order not found' });
        }

        return Message.response({ order });
    }

    private listOrders(): Message {
        const orders = Array.from(this.orders.values());
        return Message.response({ orders, count: orders.length });
    }

    private updateStatus(payload: any): Message {
        const order = this.orders.get(payload.order_id);
        
        if (!order) {
            return Message.response({ error: 'Order not found' });
        }

        order.status = payload.status;
        order.updatedAt = new Date().toISOString();
        this.saveState();

        console.log(`Updated order ${payload.order_id} to ${payload.status}`);

        return Message.response({ order });
    }

    private async saveState(): Promise<void> {
        const data = Object.fromEntries(this.orders);
        await this.state.write('orders_state', JSON.stringify(data));
    }
}

// Inventory Actor - Manages stock
class InventoryActor extends Actor {
    private inventory: Map<string, number> = new Map();
    private state: State;

    constructor() {
        super('inventory-actor');
        this.state = new State();
        this.require('STATE_READ', 'STATE_WRITE', 'ACTOR_MESSAGING', 'LOG');
    }

    async onStart(): Promise<void> {
        // Initialize with some stock
        this.inventory.set('widget', 100);
        this.inventory.set('gadget', 50);
        this.inventory.set('gizmo', 25);
    }

    async handleMessage(sender: string, message: Message): Promise<Message | null> {
        const payload = message.payload as Record<string, any>;
        const action = payload?.action;

        switch (action) {
            case 'check':
                return this.checkStock(payload);
            case 'reserve':
                return this.reserveStock(payload);
            case 'release':
                return this.releaseStock(payload);
            case 'restock':
                return this.restock(payload);
        }

        return Message.response({ error: `Unknown action: ${action}` });
    }

    private checkStock(payload: any): Message {
        const item = payload.item;
        const quantity = this.inventory.get(item) || 0;

        return Message.response({
            item,
            available: quantity,
            in_stock: quantity > 0
        });
    }

    private reserveStock(payload: any): Message {
        const { item, quantity } = payload;
        const available = this.inventory.get(item) || 0;

        if (available < quantity) {
            return Message.response({
                error: 'Insufficient stock',
                item,
                requested: quantity,
                available
            });
        }

        this.inventory.set(item, available - quantity);
        this.saveState();

        return Message.response({
            action: 'reserved',
            item,
            quantity,
            remaining: this.inventory.get(item)
        });
    }

    private releaseStock(payload: any): Message {
        const { item, quantity } = payload;
        const current = this.inventory.get(item) || 0;
        
        this.inventory.set(item, current + quantity);
        this.saveState();

        return Message.response({
            action: 'released',
            item,
            quantity,
            total: this.inventory.get(item)
        });
    }

    private restock(payload: any): Message {
        const { item, quantity } = payload;
        const current = this.inventory.get(item) || 0;
        
        this.inventory.set(item, current + quantity);
        this.saveState();

        console.log(`Restocked ${item}: +${quantity} (total: ${this.inventory.get(item)})`);

        return Message.response({
            action: 'restocked',
            item,
            quantity,
            total: this.inventory.get(item)
        });
    }

    private async saveState(): Promise<void> {
        const data = Object.fromEntries(this.inventory);
        await this.state.write('inventory_state', JSON.stringify(data));
    }
}

// Application Coordinator
class ShopApp {
    private orderActor: OrderActor;
    private inventoryActor: InventoryActor;

    constructor() {
        this.orderActor = new OrderActor();
        this.inventoryActor = new InventoryActor();
    }

    async start(): Promise<void> {
        await this.orderActor.start();
        await this.inventoryActor.start();
        
        console.log('=== Shop Application Started ===');
    }

    async stop(): Promise<void> {
        await this.orderActor.stop();
        await this.inventoryActor.stop();
    }

    async createOrder(customer: string, items: Array<{item: string, quantity: number}>): Promise<any> {
        // Check and reserve inventory
        for (const { item, quantity } of items) {
            const checkResult = await this.inventoryActor.handleMessage('shop', 
                Message.request({ action: 'check', item }));
            
            const checkPayload = checkResult?.payload as Record<string, any>;
            if (!checkPayload?.in_stock || checkPayload.available < quantity) {
                return { error: `Insufficient stock for ${item}` };
            }
        }

        // Reserve all items
        for (const { item, quantity } of items) {
            await this.inventoryActor.handleMessage('shop',
                Message.request({ action: 'reserve', item, quantity }));
        }

        // Create order
        const orderResult = await this.orderActor.handleMessage('shop',
            Message.request({ action: 'create', customer, items }));

        return orderResult?.payload;
    }

    async demo(): Promise<void> {
        // Create an order
        const result = await this.createOrder('Alice', [
            { item: 'widget', quantity: 2 },
            { item: 'gadget', quantity: 1 }
        ]);
        
        console.log('Order result:', result);

        // Check inventory
        const widgetStock = await this.inventoryActor.handleMessage('shop',
            Message.request({ action: 'check', item: 'widget' }));
        console.log('Widget stock:', widgetStock?.payload);

        // List orders
        const orders = await this.orderActor.handleMessage('shop',
            Message.request({ action: 'list' }));
        console.log('All orders:', orders?.payload);
    }
}

// Main entry point
async function main(): Promise<void> {
    const app = new ShopApp();

    process.on('SIGINT', async () => {
        console.log('\nShutting down...');
        await app.stop();
        process.exit(0);
    });

    await app.start();
    await app.demo();

    // Keep running
    console.log('\nPress Ctrl+C to stop...');
    await new Promise(() => {}); // Run forever
}

main();
```

## Part 5: Running Your Application

### Local Development

```bash
# Run the application
npx ts-node shop_app.ts

# Expected output:
# === Shop Application Started ===
# Created order order-XXX for Alice
# Order result: { action: 'created', order_id: 'order-XXX', ... }
# Widget stock: { item: 'widget', available: 98, in_stock: true }
# All orders: { orders: [...], count: 1 }
```

### Testing Your Actors

Create a test file `test_actors.ts`:

```typescript
import { Actor, Message, MessageType } from '@aether/sdk';
import { assert } from 'console';

async function testCounterActor() {
    const actor = new CounterActor();
    await actor.start();

    // Test increment
    let result = await actor.handleMessage('test', 
        Message.request({ action: 'increment' }));
    assert((result?.payload as any).count === 1);

    // Test get
    result = await actor.handleMessage('test', 
        Message.request({ action: 'get' }));
    assert((result?.payload as any).count === 1);

    // Test decrement
    result = await actor.handleMessage('test', 
        Message.request({ action: 'decrement' }));
    assert((result?.payload as any).count === 0);

    await actor.stop();
    console.log('[PASS] Counter actor tests passed');
}

async function runTests() {
    await testCounterActor();
    console.log('All tests passed!');
}

runTests();
```

## Summary

In this tutorial, you learned:

1. **Actor Basics**: Creating actors with lifecycle hooks
2. **State Management**: Persisting actor state
3. **Message Handling**: Processing different message types
4. **Multi-Actor Systems**: Building applications with multiple actors
5. **Testing**: Writing tests for your actors

## Next Steps

- Read the [Best Practices Guide](./best-practices.md)
- Explore the [API Reference](../api-reference.md)
- Check out more [Examples](../examples/overview.md)
