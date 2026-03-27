"""
Aether Demo: Real-Time Order Processing System

Demonstrates actor-based distributed systems with:
- Actor registration and lifecycle
- Message passing between actors
- State management with optimistic concurrency
- Pub/sub event-driven architecture
- Event sourcing for order history
- Saga-like orchestration (simplified)

Architecture:
    Customer -> Order Service -> Inventory Service -> Payment Service
                                  |
                        Event Bus (pub/sub)
                                  |
                    Notification Service -> Analytics Service

Usage:
    python demo.py
"""

import asyncio
import json
import os
import subprocess
import sys
import time
import urllib.request
import urllib.error

_REPO_ROOT = os.path.realpath(os.path.join(os.path.dirname(__file__), "..", ".."))
_SDK_ROOT = os.path.join(_REPO_ROOT, "sdks", "python")
_SERVER_ROOT = os.path.join(_REPO_ROOT, "server")

if _SDK_ROOT not in sys.path:
    sys.path.insert(0, _SDK_ROOT)

from aether_sdk.client import AetherClient


class OrderProcessingDemo:

    def __init__(self, base_url: str = "http://localhost:8080"):
        self.base_url = base_url
        self.client: AetherClient | None = None
        self.server_process: subprocess.Popen | None = None
        self._order_counter = 0

    async def start_server(self):
        print("Starting Aether reference server...")
        self.server_process = subprocess.Popen(
            [sys.executable, "-m", "uvicorn", "server.app:app",
             "--host", "0.0.0.0", "--port", "8080"],
            cwd=_REPO_ROOT,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        for _ in range(50):
            try:
                urllib.request.urlopen(f"{self.base_url}/health")
                print("Server is ready\n")
                return
            except Exception:
                await asyncio.sleep(0.1)
        raise RuntimeError("Server failed to start within 5 seconds")

    async def stop_server(self):
        if self.server_process:
            self.server_process.terminate()
            try:
                self.server_process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.server_process.kill()
                self.server_process.wait()
            print("\nServer stopped")

    def _next_order_id(self) -> str:
        self._order_counter += 1
        return f"ORD-{int(time.time() * 1000)}-{self._order_counter:03d}"

    async def setup(self, client: AetherClient):
        print("Setting up services...")
        services = [
            ("order-service", "service", ["orders", "events"]),
            ("inventory-service", "service", ["inventory"]),
            ("payment-service", "service", ["payments"]),
            ("notification-service", "service", ["notifications"]),
            ("analytics-service", "service", ["analytics"]),
        ]
        for actor_id, actor_type, caps in services:
            await client.register_actor(actor_id, actor_type, capabilities=caps)
            print(f"  Registered: {actor_id} ({actor_type}, caps={caps})")

        subscriptions = [
            ("orders.*", "notification-service"),
            ("orders.*", "analytics-service"),
            ("inventory.*", "analytics-service"),
            ("payments.*", "notification-service"),
        ]
        for topic, subscriber in subscriptions:
            await client.subscribe(topic, subscriber)
        print("  Subscriptions configured\n")

    async def process_order(self, client: AetherClient, order: dict) -> dict:
        order_id = self._next_order_id()
        timeline = []

        # Step 1: Order Service receives order
        timeline.append(("order-service", "received", order_id))
        await client.send_message(
            "order-service",
            {"action": "create_order", "order_id": order_id, "items": order["items"]},
            source="customer",
        )
        await client.set_state("order-service", f"order:{order_id}", {
            "status": "created", "items": order["items"], "total": order["total"],
        })

        # Step 2: Check inventory
        timeline.append(("inventory-service", "checking", order_id))
        await client.send_message(
            "inventory-service",
            {"action": "check_inventory", "order_id": order_id, "items": order["items"]},
            source="order-service",
        )
        await client.set_state("inventory-service", f"holds:{order_id}", {
            "items": order["items"], "available": True,
        })

        # Step 3: Process payment
        timeline.append(("payment-service", "processing", order_id))
        await client.send_message(
            "payment-service",
            {"action": "charge", "order_id": order_id,
             "amount": order["total"], "card_last_four": "4242"},
            source="order-service",
        )
        await client.set_state("payment-service", f"payment:{order_id}", {
            "status": "completed", "amount": order["total"],
        })

        # Step 4: Update inventory (deduct)
        timeline.append(("inventory-service", "deducting", order_id))
        await client.send_message(
            "inventory-service",
            {"action": "deduct_inventory", "order_id": order_id, "items": order["items"]},
            source="order-service",
        )
        await client.set_state("inventory-service", f"holds:{order_id}", {
            "items": order["items"], "available": False, "deducted": True,
        })

        # Step 5: Mark order complete
        await client.set_state("order-service", f"order:{order_id}", {
            "status": "completed", "items": order["items"], "total": order["total"],
        })

        # Step 6: Publish events on the bus
        await client.publish("orders.completed", {
            "order_id": order_id, "total": order["total"], "items": order["items"],
        })
        await client.publish("inventory.updated", {
            "order_id": order_id, "items_deducted": order["items"],
        })
        await client.publish("payments.processed", {
            "order_id": order_id, "amount": order["total"],
        })

        # Step 7: Notification service records
        await client.set_state("notification-service", f"notif:{order_id}", {
            "type": "order_confirmation", "order_id": order_id,
            "status": "sent",
        })

        # Step 8: Analytics service aggregates
        timeline.append(("analytics-service", "recording", order_id))
        total_orders = await client.get_state("analytics-service", "total_orders")
        total_revenue = await client.get_state("analytics-service", "total_revenue")
        await client.set_state("analytics-service", "total_orders", (total_orders or 0) + 1)
        await client.set_state("analytics-service", "total_revenue",
                               (total_revenue or 0) + order["total"])

        # Step 9: Event sourcing — append the full lifecycle
        await client.append_event(order_id, "OrderCreated", order)
        await client.append_event(order_id, "InventoryChecked", {"available": True})
        await client.append_event(order_id, "PaymentProcessed", {"amount": order["total"]})
        await client.append_event(order_id, "InventoryDeducted", {"items": order["items"]})
        await client.append_event(order_id, "OrderCompleted", {"status": "completed"})

        return {"order_id": order_id, "timeline": timeline, "status": "completed"}

    def print_timeline(self, result: dict):
        print(f"\n{'=' * 60}")
        print(f"  Order {result['order_id']} — {result['status'].upper()}")
        print(f"{'=' * 60}")
        for i, (service, action, oid) in enumerate(result["timeline"], 1):
            print(f"  {i}. [{service:25s}] {action} (order: {oid})")
        print(f"\n  Final state:")
        print(f"     Order: completed")
        print(f"     Inventory: deducted")
        print(f"     Payment: processed")
        print(f"     Events: 5 recorded\n")

    async def verify_event_sourcing(self, client: AetherClient, order_id: str):
        events = await client.get_events(order_id)
        expected_types = [
            "OrderCreated", "InventoryChecked", "PaymentProcessed",
            "InventoryDeducted", "OrderCompleted",
        ]
        actual_types = [e.event_type for e in events]
        assert actual_types == expected_types, (
            f"Event mismatch for {order_id}: {actual_types} != {expected_types}"
        )
        assert events[-1].version == 5
        return events

    async def run(self):
        self.client = AetherClient(self.base_url, actor_id="demo-orchestrator")
        await self.client.connect()

        print("\n" + "=" * 62)
        print("  Aether Demo: Real-Time Order Processing System")
        print("=" * 62 + "\n")

        await self.setup(self.client)

        orders = [
            {"items": ["Widget A", "Widget B"], "total": 59.99},
            {"items": ["Gadget C"], "total": 29.99},
            {"items": ["Widget A", "Widget A", "Widget A"], "total": 89.97},
        ]

        results = []
        for i, order in enumerate(orders, 1):
            print(f"--- Order #{i} ---")
            result = await self.process_order(self.client, order)
            results.append(result)
            self.print_timeline(result)

            events = await self.verify_event_sourcing(self.client, result["order_id"])
            print(f"  Event sourcing verified: {len(events)} events\n")
            await asyncio.sleep(0.05)

        # Summary
        total_items = sum(len(o["items"]) for o in orders)
        total_revenue = sum(o["total"] for o in orders)

        print("=" * 60)
        print("  SUMMARY")
        print("=" * 60)
        print(f"  Orders processed: {len(results)}")
        print(f"  Total items:      {total_items}")
        print(f"  Total revenue:    ${total_revenue:.2f}")

        total_events = sum(
            len(await self.client.get_events(r["order_id"])) for r in results
        )
        print(f"  Events recorded:  {total_events}")

        actors = await self.client.list_actors()
        print(f"  Active actors:    {len(actors)}")
        topics = await self.client.list_topics()
        print(f"  Active topics:    {len(topics)}")

        # Verify analytics state
        analytics_state = await self.client.get_all_state("analytics-service")
        print(f"\n  Analytics state:  {json.dumps(analytics_state, default=str)}")

        print(f"\nAll {len(results)} orders processed successfully!\n")


async def main():
    demo = OrderProcessingDemo()
    try:
        await demo.start_server()
        await demo.run()
    except KeyboardInterrupt:
        print("\n\nDemo interrupted by user")
    except Exception as e:
        print(f"\nError: {e}")
        raise
    finally:
        if demo.client:
            await demo.client.close()
        await demo.stop_server()


if __name__ == "__main__":
    asyncio.run(main())
