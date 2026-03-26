/**
 * Performance benchmarks for the Aether JavaScript SDK.
 *
 * Run with:
 *   npx jest tests/performance/benchmark.test.ts --verbose
 *
 * Run only performance tests:
 *   npx jest --testPathPattern=benchmark --verbose
 */

// @jest/tag:performance

import { Timestamp, Duration, WindowType, StreamEvent, createStreamEvent, createWindowSpec, BackpressureStrategy } from '../../src/streaming/types';
import { WindowAssigner, TumblingWindow } from '../../src/streaming/window';
import { BackpressureController, MultiLevelBackpressure } from '../../src/streaming/backpressure';
import { CircuitBreaker } from '../../src/resilience/circuit_breaker';
import { RetryPolicy } from '../../src/resilience/retry';
import { validateEmail, validateUUID } from '../../src/validation/validators';
import { Message, MessageType, Priority } from '../../src/messaging';

const N_STREAM = 100_000;
const N_RESILIENCE = 10_000;
const N_VALIDATION = 10_000;
const N_MESSAGES = 10_000;

function fmtOps(n: number, elapsedMs: number): string {
  const ops = (n / elapsedMs) * 1000;
  return `${ops.toLocaleString('en-US', { maximumFractionDigits: 0 })} ops/sec (${elapsedMs.toFixed(1)}ms for ${n.toLocaleString()} ops)`;
}

function fmtLatency(totalMs: number, n: number): string {
  const avgUs = (totalMs / n) * 1000;
  return `${avgUs.toFixed(2)} us/call (${totalMs.toFixed(2)}ms for ${n.toLocaleString()} calls)`;
}

describe('Performance Benchmarks', () => {
  describe('Stream Processing', () => {
    test('WindowAssigner throughput (100K events)', () => {
      const spec = createWindowSpec(WindowType.Tumbling, Duration.fromMinutes(5));
      const assigner = new WindowAssigner<string, number>(spec);

      const events: StreamEvent<number>[] = [];
      for (let i = 0; i < N_STREAM; i++) {
        events.push(createStreamEvent(`key-${i % 100}`, i, new Timestamp(i * 1000)));
      }

      const t0 = performance.now();
      for (const ev of events) {
        assigner.assign(ev, ev.key);
      }
      const elapsed = performance.now() - t0;

      console.log(`\n  [WindowAssigner.assign] ${fmtOps(N_STREAM, elapsed)}`);
      expect(elapsed).toBeLessThan(10000);
    });

    test('TumblingWindow.process throughput (100K events)', () => {
      let firedCount = 0;
      const tw = new TumblingWindow<string, number, number>(
        Duration.fromMinutes(5),
        (events, info) => {
          firedCount++;
          return events.length;
        }
      );

      const events: StreamEvent<number>[] = [];
      for (let i = 0; i < N_STREAM; i++) {
        events.push(createStreamEvent('k1', i, new Timestamp(i * 1000)));
      }

      const t0 = performance.now();
      for (const ev of events) {
        tw.process(ev, 'k1');
      }
      const elapsed = performance.now() - t0;

      console.log(`\n  [TumblingWindow.process] ${fmtOps(N_STREAM, elapsed)}`);
      expect(elapsed).toBeLessThan(10000);
    });
  });

  describe('Backpressure', () => {
    test('BackpressureController.tryPush throughput (100K events)', () => {
      const ctrl = new BackpressureController({
        strategy: BackpressureStrategy.Buffer,
        bufferSize: 200_000,
        highWatermark: 0.9,
        lowWatermark: 0.5,
      });

      const events: StreamEvent<number>[] = [];
      for (let i = 0; i < N_STREAM; i++) {
        events.push(createStreamEvent('k', i, new Timestamp(i)));
      }

      const t0 = performance.now();
      for (const ev of events) {
        ctrl.tryPush(ev);
      }
      const elapsed = performance.now() - t0;

      console.log(`\n  [BackpressureController.tryPush] ${fmtOps(N_STREAM, elapsed)}`);
      expect(elapsed).toBeLessThan(10000);
    });

    test('MultiLevelBackpressure.push throughput (100K events)', () => {
      const bp = new MultiLevelBackpressure<number>(200_000);

      const events: StreamEvent<number>[] = [];
      for (let i = 0; i < N_STREAM; i++) {
        events.push(createStreamEvent('k', i, new Timestamp(i)));
      }

      const t0 = performance.now();
      for (const ev of events) {
        bp.push(ev, MultiLevelBackpressure.NORMAL);
      }
      const elapsed = performance.now() - t0;

      console.log(`\n  [MultiLevelBackpressure.push] ${fmtOps(N_STREAM, elapsed)}`);
      expect(elapsed).toBeLessThan(10000);
    });
  });

  describe('Circuit Breaker', () => {
    test('CircuitBreaker.execute overhead (10K calls)', async () => {
      const breaker = new CircuitBreaker({ failureThreshold: 1000 });

      const ok = async () => 42;

      const t0 = performance.now();
      for (let i = 0; i < N_RESILIENCE; i++) {
        await breaker.execute(ok);
      }
      const elapsed = performance.now() - t0;

      console.log(`\n  [CircuitBreaker.execute] ${fmtLatency(elapsed, N_RESILIENCE)}`);
      expect(elapsed).toBeLessThan(10000);
    });

    test('CircuitBreaker overhead vs direct call (10K calls)', async () => {
      const breaker = new CircuitBreaker({ failureThreshold: 1000 });
      const ok = async () => 42;

      const t0 = performance.now();
      for (let i = 0; i < N_RESILIENCE; i++) {
        await breaker.execute(ok);
      }
      const withCb = performance.now() - t0;

      const t1 = performance.now();
      for (let i = 0; i < N_RESILIENCE; i++) {
        await ok();
      }
      const withoutCb = performance.now() - t1;

      const overheadUs = ((withCb - withoutCb) / N_RESILIENCE) * 1000;
      console.log(`\n  [CircuitBreaker overhead vs direct] ${overheadUs.toFixed(2)} us/call`);
      console.log(`    with_cb=${withCb.toFixed(2)}ms  without_cb=${withoutCb.toFixed(2)}ms`);
    });
  });

  describe('Retry', () => {
    test('RetryPolicy.execute overhead (10K calls, success first attempt)', async () => {
      const policy = new RetryPolicy({ maxAttempts: 1 });

      const ok = async () => 42;

      const t0 = performance.now();
      for (let i = 0; i < N_RESILIENCE; i++) {
        await policy.execute(ok);
      }
      const elapsed = performance.now() - t0;

      console.log(`\n  [RetryPolicy.execute (maxAttempts=1)] ${fmtLatency(elapsed, N_RESILIENCE)}`);
      expect(elapsed).toBeLessThan(10000);
    });
  });

  describe('Validation', () => {
    test('validateEmail throughput (10K emails)', () => {
      const emails: string[] = [];
      for (let i = 0; i < N_VALIDATION; i++) {
        emails.push(`user${i}@example.com`);
      }

      const t0 = performance.now();
      for (const email of emails) {
        validateEmail(email);
      }
      const elapsed = performance.now() - t0;

      console.log(`\n  [validateEmail] ${fmtOps(N_VALIDATION, elapsed)}`);
      expect(elapsed).toBeLessThan(5000);
    });

    test('validateUUID throughput (10K UUIDs)', () => {
      const uuids: string[] = [];
      for (let i = 0; i < N_VALIDATION; i++) {
        uuids.push(
          `${i.toString(16).padStart(8, '0')}-1234-5678-1234-${i.toString(16).padStart(12, '0')}`
        );
      }

      const t0 = performance.now();
      for (const u of uuids) {
        validateUUID(u);
      }
      const elapsed = performance.now() - t0;

      console.log(`\n  [validateUUID] ${fmtOps(N_VALIDATION, elapsed)}`);
      expect(elapsed).toBeLessThan(5000);
    });
  });

  describe('Message Serialization', () => {
    test('Message.custom().toJSON() throughput (10K messages)', () => {
      const messages: Message[] = [];
      for (let i = 0; i < N_MESSAGES; i++) {
        messages.push(Message.custom({ action: 'test', id: i }, Priority.NORMAL));
      }

      const t0 = performance.now();
      for (const msg of messages) {
        msg.toJSON();
      }
      const elapsed = performance.now() - t0;

      console.log(`\n  [Message.toJSON] ${fmtOps(N_MESSAGES, elapsed)}`);
      expect(elapsed).toBeLessThan(5000);
    });

    test('Message.fromJSON() throughput (10K messages)', () => {
      const payloads: object[] = [];
      for (let i = 0; i < N_MESSAGES; i++) {
        payloads.push({
          type: MessageType.CUSTOM,
          payload: { action: 'test', id: i },
          sender: undefined,
          correlationId: undefined,
          priority: Priority.NORMAL,
        });
      }

      const t0 = performance.now();
      for (const data of payloads) {
        Message.fromJSON(data);
      }
      const elapsed = performance.now() - t0;

      console.log(`\n  [Message.fromJSON] ${fmtOps(N_MESSAGES, elapsed)}`);
      expect(elapsed).toBeLessThan(5000);
    });

    test('Message round-trip toJSON/fromJSON (10K messages)', () => {
      const messages: Message[] = [];
      for (let i = 0; i < N_MESSAGES; i++) {
        messages.push(Message.custom({ action: 'test', id: i }));
      }

      const t0 = performance.now();
      for (const msg of messages) {
        const json = msg.toJSON();
        Message.fromJSON(json);
      }
      const elapsed = performance.now() - t0;

      console.log(`\n  [Message round-trip] ${fmtOps(N_MESSAGES, elapsed)}`);
      expect(elapsed).toBeLessThan(5000);
    });
  });
});
