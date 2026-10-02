//! Shared test helpers: an in-memory serial link and a DS2 frame builder.

// Each test crate uses a different subset of these helpers.
#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Duration;

use ds2_protocol::SerialLink;

pub const TIMEOUT: Duration = Duration::from_millis(100);

/// Smallest valid frame: address + length + checksum.
pub const MIN_FRAME_LEN: usize = 3;
pub const MAX_FRAME_LEN: usize = 255;
pub const MAX_PAYLOAD_LEN: usize = MAX_FRAME_LEN - MIN_FRAME_LEN;

#[derive(Default)]
struct LinkState {
    incoming: VecDeque<u8>,
    max_chunk: Option<usize>,
    transmitted: Vec<Vec<u8>>,
    receive_timeouts: Vec<Duration>,
}

/// Cloneable handle, so the test can still inspect traffic after `Ds2` takes ownership of the link.
#[derive(Clone, Default)]
pub struct MockLink {
    state: Rc<RefCell<LinkState>>,
}

impl MockLink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_incoming(bytes: &[u8]) -> Self {
        let link = Self::new();
        link.queue_incoming(bytes);
        link
    }

    pub fn queue_incoming(&self, bytes: &[u8]) {
        self.state.borrow_mut().incoming.extend(bytes);
    }

    /// Simulates a UART that delivers data in small pieces.
    pub fn limit_chunk(&self, max_chunk: usize) {
        self.state.borrow_mut().max_chunk = Some(max_chunk);
    }

    pub fn transmitted(&self) -> Vec<Vec<u8>> {
        self.state.borrow().transmitted.clone()
    }

    pub fn receive_timeouts(&self) -> Vec<Duration> {
        self.state.borrow().receive_timeouts.clone()
    }

    pub fn pending_incoming(&self) -> usize {
        self.state.borrow().incoming.len()
    }
}

impl SerialLink for MockLink {
    fn receive(&mut self, buffer: &mut [u8], timeout: Duration) -> usize {
        let mut state = self.state.borrow_mut();
        state.receive_timeouts.push(timeout);

        let available = state.incoming.len().min(buffer.len());
        let count = state.max_chunk.map_or(available, |max| available.min(max));
        for slot in &mut buffer[..count] {
            *slot = state
                .incoming
                .pop_front()
                .expect("count never exceeds queued bytes");
        }
        count
    }

    fn transmit(&mut self, data: &[u8]) {
        self.state.borrow_mut().transmitted.push(data.to_vec());
    }
}

pub fn xor_checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0, |acc, byte| acc ^ byte)
}

/// Builds a well-formed frame independently of the library, so tests don't trust its encoder.
pub fn build_frame(address: u8, payload: &[u8]) -> Vec<u8> {
    let frame_len = u8::try_from(payload.len() + MIN_FRAME_LEN).expect("payload too long for DS2");
    let mut frame = vec![address, frame_len];
    frame.extend_from_slice(payload);
    frame.push(xor_checksum(&frame));
    frame
}
