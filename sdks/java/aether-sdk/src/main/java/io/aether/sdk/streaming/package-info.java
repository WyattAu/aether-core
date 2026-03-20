/**
 * Aether SDK Streaming Module
 *
 * Provides stream processing capabilities for building event-driven applications:
 * - Event-time processing with watermarks
 * - Windowed aggregations (tumbling, sliding, session)
 * - Backpressure handling
 * - Stream actors
 *
 * @see StreamActor
 * @see BackpressureController
 * @see WindowAssigner
 */
package io.aether.sdk.streaming;

import java.util.*;
import java.time.Instant;
import java.util.function.*;

/**
 * Core types for stream processing.
 *
 * <p>This package provides:
 * <ul>
 *   <li>{@link Types.Timestamp} - Event timestamp with millisecond precision</li>
 *   <li>{@link Types.Duration} - Time duration with millisecond precision</li>
 *   <li>{@link StreamEvent} - Individual event in a stream</li>
 *   <li>{@link Types.Watermark} - Time marker for event progress</li>
 *   <li>{@link Types.WindowSpec} - Window configuration</li>
 *   <li>{@link Types.StreamConfig} - Stream actor configuration</li>
 * </ul>
 *
 * <p>Backpressure handling:
 * <ul>
 *   <li>{@link BackpressureController} - Main controller with strategies</li>
 *   <li>{@link MultiLevelBackpressure} - Priority-based queues</li>
 *   <li>{@link RateBasedBackpressure} - Rate limiting</li>
 * </ul>
 *
 * <p>Windowing functions:
 * <ul>
 *   <li>{@link WindowAssigner} - Assigns events to windows</li>
 *   <li>{@link WindowTrigger} - Triggers window firing</li>
 *   <li>{@link TumblingWindow} - Fixed-size non-overlapping windows</li>
 *   <li>{@link SlidingWindow} - Fixed-size overlapping windows</li>
 *   <li>{@link SessionWindow} - Dynamic size based on activity gaps</li>
 * </ul>
 *
 * <p>Example usage:
 * <pre>{@code
 * // Create a stream processor
 * public class OrderProcessor extends StreamActor<String, Order> {
 *     @Override
 *     protected void processEvent(StreamEvent<Order> event) {
 *         Order order = event.getValue();
 *         // Process the order
 *         emit("processed", transform(order));
 *     }
 * }
 *
 * // Create backpressure controller
 * BackpressureController<Order> controller = BackpressureController.<Order>builder()
 *     .strategy(BackpressureStrategy.BUFFER)
 *     .bufferSize(10000)
 *     .build();
 *
 * // Create tumbling window
 * TumblingWindow<String, Order> window = TumblingWindow.<String, Order>builder()
 *     .size(Duration.fromMinutes(5))
 *     .handler((events, info) -> {
 *         // Aggregate events
 *         return events.size();
 *     })
 *     .build();
 * }</pre>
 *
 * @since 0.3.0
 */
package io.aether.sdk.streaming;
