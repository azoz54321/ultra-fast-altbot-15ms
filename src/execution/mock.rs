use crossbeam_channel::{bounded, Receiver, Sender, TryRecvError};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Order side (Buy or Sell)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderSide {
    Buy,
    Sell,
}

/// Order intent emitted from hot path
#[derive(Debug, Clone, Copy)]
pub struct OrderIntent {
    pub symbol_id: u32,
    pub side: OrderSide,
    pub px_e8: u64,
    pub ts_unix_ms: u64,
}

impl OrderIntent {
    pub fn new(symbol_id: u32, side: OrderSide, px_e8: u64, ts_unix_ms: u64) -> Self {
        Self {
            symbol_id,
            side,
            px_e8,
            ts_unix_ms,
        }
    }
}

/// Order event kind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderEventKind {
    Submitted,
    Ack,
    Fill,
}

/// Order event from execution mock
#[derive(Debug, Clone, Copy)]
pub struct OrderEvent {
    pub kind: OrderEventKind,
    pub symbol_id: u32,
    pub px_e8: u64,
    pub ts_unix_ms: u64,
}

impl OrderEvent {
    pub fn new(kind: OrderEventKind, symbol_id: u32, px_e8: u64, ts_unix_ms: u64) -> Self {
        Self {
            kind,
            symbol_id,
            px_e8,
            ts_unix_ms,
        }
    }
}

/// Execution mock that simulates exchange responses without real I/O
/// Runs off hot path in separate thread/task
pub struct ExecutionMock {
    /// Receiver for order intents from hot path
    intent_rx: Receiver<OrderIntent>,
    /// Sender for order events back to metrics/monitoring
    event_tx: Option<Sender<OrderEvent>>,
    /// Counter for acknowledged orders
    ack_count: Arc<AtomicU64>,
    /// Counter for filled orders
    fill_count: Arc<AtomicU64>,
    /// Counter for submitted orders
    submitted_count: Arc<AtomicU64>,
    /// Deterministic delay in microseconds for Ack (RTT simulation)
    ack_delay_us: u64,
    /// Deterministic delay in microseconds for Fill after Ack
    fill_delay_us: u64,
}

impl ExecutionMock {
    /// Create a new execution mock with SPSC channel
    /// queue_capacity: bounded queue size for intents
    /// ack_delay_us: microseconds to wait before sending Ack
    /// fill_delay_us: microseconds to wait before sending Fill after Ack
    pub fn new(
        queue_capacity: usize,
        ack_delay_us: u64,
        fill_delay_us: u64,
    ) -> (Self, Sender<OrderIntent>, Receiver<OrderEvent>) {
        let (intent_tx, intent_rx) = bounded(queue_capacity);
        let (event_tx, event_rx) = bounded(queue_capacity * 2); // 2x for Ack + Fill

        let mock = ExecutionMock {
            intent_rx,
            event_tx: Some(event_tx),
            ack_count: Arc::new(AtomicU64::new(0)),
            fill_count: Arc::new(AtomicU64::new(0)),
            submitted_count: Arc::new(AtomicU64::new(0)),
            ack_delay_us,
            fill_delay_us,
        };

        (mock, intent_tx, event_rx)
    }

    /// Get counters for metrics
    pub fn get_counters(&self) -> (Arc<AtomicU64>, Arc<AtomicU64>, Arc<AtomicU64>) {
        (
            Arc::clone(&self.submitted_count),
            Arc::clone(&self.ack_count),
            Arc::clone(&self.fill_count),
        )
    }

    /// Process a single intent (deterministic delays without syscalls)
    /// Returns true if processed, false if should stop
    fn process_intent(&self, intent: OrderIntent) -> bool {
        let event_tx = match &self.event_tx {
            Some(tx) => tx,
            None => return false,
        };

        // Submit event
        self.submitted_count.fetch_add(1, Ordering::Relaxed);
        let submit_event = OrderEvent::new(
            OrderEventKind::Submitted,
            intent.symbol_id,
            intent.px_e8,
            intent.ts_unix_ms,
        );
        let _ = event_tx.try_send(submit_event);

        // Simulate deterministic delay for Ack (no actual sleep on hot path)
        // In real system, this would involve async wait or time-based processing
        // For benchmark purposes, we track timing via monotonic counter
        let ack_ts = intent.ts_unix_ms + (self.ack_delay_us / 1000);

        // Send Ack
        self.ack_count.fetch_add(1, Ordering::Relaxed);
        let ack_event = OrderEvent::new(
            OrderEventKind::Ack,
            intent.symbol_id,
            intent.px_e8,
            ack_ts,
        );
        let _ = event_tx.try_send(ack_event);

        // Simulate delay for Fill
        let fill_ts = ack_ts + (self.fill_delay_us / 1000);

        // Send Fill
        self.fill_count.fetch_add(1, Ordering::Relaxed);
        let fill_event = OrderEvent::new(
            OrderEventKind::Fill,
            intent.symbol_id,
            intent.px_e8,
            fill_ts,
        );
        let _ = event_tx.try_send(fill_event);

        true
    }

    /// Run the execution mock (call from off hot-path thread)
    /// Processes intents in a loop until channel is closed
    pub fn run(&self) {
        loop {
            match self.intent_rx.try_recv() {
                Ok(intent) => {
                    if !self.process_intent(intent) {
                        break;
                    }
                }
                Err(TryRecvError::Empty) => {
                    // No intents available, continue
                    // In real implementation, might use blocking recv or async
                    continue;
                }
                Err(TryRecvError::Disconnected) => {
                    // Channel closed, exit
                    break;
                }
            }
        }
    }

    /// Run with a maximum number of intents to process (for testing/benchmarks)
    pub fn run_with_limit(&self, max_intents: usize) {
        let mut processed = 0;
        loop {
            if processed >= max_intents {
                break;
            }

            match self.intent_rx.try_recv() {
                Ok(intent) => {
                    if !self.process_intent(intent) {
                        break;
                    }
                    processed += 1;
                }
                Err(TryRecvError::Empty) => {
                    // No intents available, continue
                    continue;
                }
                Err(TryRecvError::Disconnected) => {
                    // Channel closed, exit
                    break;
                }
            }
        }
    }
}

/// Metrics for execution mock
pub struct ExecutionMetrics {
    pub submitted: Arc<AtomicU64>,
    pub acks: Arc<AtomicU64>,
    pub fills: Arc<AtomicU64>,
}

impl ExecutionMetrics {
    pub fn new(
        submitted: Arc<AtomicU64>,
        acks: Arc<AtomicU64>,
        fills: Arc<AtomicU64>,
    ) -> Self {
        Self {
            submitted,
            acks,
            fills,
        }
    }

    pub fn get_submitted(&self) -> u64 {
        self.submitted.load(Ordering::Relaxed)
    }

    pub fn get_acks(&self) -> u64 {
        self.acks.load(Ordering::Relaxed)
    }

    pub fn get_fills(&self) -> u64 {
        self.fills.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_intent_creation() {
        let intent = OrderIntent::new(123, OrderSide::Buy, 5000_0000_0000, 1700000000000);
        assert_eq!(intent.symbol_id, 123);
        assert_eq!(intent.side, OrderSide::Buy);
        assert_eq!(intent.px_e8, 5000_0000_0000);
    }

    #[test]
    fn test_order_event_creation() {
        let event = OrderEvent::new(OrderEventKind::Ack, 123, 5000_0000_0000, 1700000000000);
        assert_eq!(event.kind, OrderEventKind::Ack);
        assert_eq!(event.symbol_id, 123);
    }

    #[test]
    fn test_execution_mock_creation() {
        let (_mock, _intent_tx, _event_rx) = ExecutionMock::new(100, 50, 100);
        // Successfully created
    }

    #[test]
    fn test_execution_mock_process_intent() {
        let (mock, intent_tx, event_rx) = ExecutionMock::new(100, 50, 100);

        // Send an intent
        let intent = OrderIntent::new(42, OrderSide::Buy, 1000_0000_0000, 1700000000000);
        intent_tx.send(intent).unwrap();

        // Process intents
        mock.run_with_limit(1);

        // Should receive 3 events: Submitted, Ack, Fill
        let mut events = Vec::new();
        while let Ok(event) = event_rx.try_recv() {
            events.push(event);
        }

        assert_eq!(events.len(), 3);
        assert_eq!(events[0].kind, OrderEventKind::Submitted);
        assert_eq!(events[1].kind, OrderEventKind::Ack);
        assert_eq!(events[2].kind, OrderEventKind::Fill);

        // Verify counters
        let (submitted, acks, fills) = mock.get_counters();
        assert_eq!(submitted.load(Ordering::Relaxed), 1);
        assert_eq!(acks.load(Ordering::Relaxed), 1);
        assert_eq!(fills.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_execution_mock_multiple_intents() {
        let (mock, intent_tx, event_rx) = ExecutionMock::new(100, 50, 100);

        // Send multiple intents
        for i in 0..5 {
            let intent = OrderIntent::new(i, OrderSide::Buy, 1000_0000_0000, 1700000000000 + i as u64);
            intent_tx.send(intent).unwrap();
        }

        // Process all intents
        mock.run_with_limit(5);

        // Should receive 15 events: 5 * (Submitted, Ack, Fill)
        let mut event_count = 0;
        while event_rx.try_recv().is_ok() {
            event_count += 1;
        }

        assert_eq!(event_count, 15);

        // Verify counters
        let (submitted, acks, fills) = mock.get_counters();
        assert_eq!(submitted.load(Ordering::Relaxed), 5);
        assert_eq!(acks.load(Ordering::Relaxed), 5);
        assert_eq!(fills.load(Ordering::Relaxed), 5);
    }
}
