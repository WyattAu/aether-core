// Package streaming provides stream processing capabilities for Aether actors.
//
// The package includes:
//   - Windowing: Tumbling, sliding, and session windows
//   - Backpressure: Flow control with multiple strategies
//   - Stream Actors: Specialized actors for stream processing
//   - State Management: Persistent state for streaming operators
//
// # Windowing Example
//
//	// Create a tumbling window with 1-minute size
//	window, err := streaming.NewTumblingWindow[string, int](streaming.FromMinutes(1))
//	if err != nil {
//	    log.Fatal(err)
//	}
//
//	// Process events
//	event := streaming.NewStreamEvent("user-123", 42)
//	window.Process("user-123", event)
//
//	// Trigger windows based on watermark
//	watermark := streaming.Now()
//	results := window.Trigger(watermark)
//
// # Backpressure Example
//
//	// Create a backpressure controller
//	config := streaming.BackpressureConfig{
//	    Strategy:      streaming.BackpressureStrategyBuffer,
//	    BufferSize:    10000,
//	    HighWatermark: 0.9,
//	    LowWatermark:  0.5,
//	}
//	controller := streaming.NewBackpressureController[int](config)
//
//	// Offer events
//	if err := controller.Offer(42); err != nil {
//	    // Handle backpressure
//	}
//
//	// Poll for processing
//	if value, ok := controller.Poll(); ok {
//	    process(value)
//	}
//
// # Stream Actor Example
//
//	// Create a keyed stream actor
//	config := streaming.DefaultStreamConfig()
//	actor := streaming.NewKeyedStreamActor[string, string](
//	    "processor",
//	    config,
//	    func(ctx context.Context, key string, event streaming.StreamEvent[string]) error {
//	        // Process the event
//	        return nil
//	    },
//	)
//
//	// Run the actor
//	go actor.Run(context.Background())
//
// # Windowed Processing Example
//
//	// Create a windowed stream actor for aggregations
//	config := streaming.DefaultStreamConfig()
//	spec := streaming.NewTumblingWindowSpec(streaming.FromMinutes(5))
//
//	actor, err := streaming.NewWindowedStreamActor[string, int, int](
//	    "aggregator",
//	    config,
//	    spec,
//	    func(key string, values []int) int {
//	        sum := 0
//	        for _, v := range values {
//	            sum += v
//	        }
//	        return sum
//	    },
//	)
//	if err != nil {
//	    log.Fatal(err)
//	}
//
// # Multi-Level Backpressure Example
//
//	// Create backpressure with priority levels
//	levels := map[int]streaming.BackpressureConfig{
//	    0: {Strategy: streaming.BackpressureStrategyBuffer, BufferSize: 10000}, // High priority
//	    1: {Strategy: streaming.BackpressureStrategyDrop, BufferSize: 5000},    // Low priority
//	}
//	mlb := streaming.NewMultiLevelBackpressure[int](levels)
//
//	// Offer with priority
//	mlb.Offer(42, 0) // High priority
//	mlb.Offer(99, 1) // Low priority
//
// # Rate-Based Backpressure Example
//
//	// Create rate limiter (1000 events per second)
//	rateLimiter := streaming.NewRateBasedBackpressure(1000)
//
//	// Check before processing
//	if rateLimiter.Allow() {
//	    processEvent()
//	} else {
//	    // Wait for token
//	    rateLimiter.WaitForToken(ctx)
//	}
//
// # Adaptive Backpressure Example
//
//	// Create adaptive backpressure that scales buffer size
//	adaptive := streaming.NewAdaptiveBackpressure[int](
//	    1000,  // initial size
//	    500,   // minimum size
//	    10000, // maximum size
//	)
//
//	// Use like regular backpressure controller
//	adaptive.Offer(event)
//	value, ok := adaptive.Poll()
package streaming
