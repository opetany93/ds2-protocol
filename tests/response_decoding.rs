//! How `Ds2::request` reads and validates the ECU response.

mod common;

use common::{MAX_PAYLOAD_LEN, MockLink, TIMEOUT, build_frame};
use ds2_protocol::{Ds2, Error};

const DME_ADDRESS: u8 = 0x12;
const OTHER_ADDRESS: u8 = 0x13;
const COMMAND: [u8; 1] = [0x00];

fn request_with_incoming(incoming: &[u8]) -> Result<Vec<u8>, Error> {
    let mut ds2 = Ds2::new(MockLink::with_incoming(incoming), TIMEOUT);
    ds2.request(DME_ADDRESS, &COMMAND).map(<[u8]>::to_vec)
}

#[test]
fn returns_payload_without_header_and_checksum() {
    let response = build_frame(DME_ADDRESS, &[0xA0, 0x01, 0x02]);

    assert_eq!(request_with_incoming(&response), Ok(vec![0xA0, 0x01, 0x02]));
}

#[test]
fn returns_empty_payload_for_minimal_frame() {
    let response = build_frame(DME_ADDRESS, &[]);

    assert_eq!(request_with_incoming(&response), Ok(vec![]));
}

#[test]
fn returns_longest_payload() {
    let payload: Vec<u8> = (0..MAX_PAYLOAD_LEN).map(|i| i as u8).collect();
    let response = build_frame(DME_ADDRESS, &payload);

    assert_eq!(request_with_incoming(&response), Ok(payload));
}

#[test]
fn reassembles_response_delivered_byte_by_byte() {
    let link = MockLink::with_incoming(&build_frame(DME_ADDRESS, &[0xA0, 0x11, 0x22, 0x33]));
    link.limit_chunk(1);
    let mut ds2 = Ds2::new(link, TIMEOUT);

    assert_eq!(
        ds2.request(DME_ADDRESS, &COMMAND),
        Ok(&[0xA0, 0x11, 0x22, 0x33][..])
    );
}

#[test]
fn reassembles_response_delivered_in_uneven_chunks() {
    let payload: Vec<u8> = (0..40).collect();
    let link = MockLink::with_incoming(&build_frame(DME_ADDRESS, &payload));
    link.limit_chunk(7);
    let mut ds2 = Ds2::new(link, TIMEOUT);

    assert_eq!(ds2.request(DME_ADDRESS, &COMMAND), Ok(&payload[..]));
}

#[test]
fn reads_exactly_one_frame() {
    let link = MockLink::with_incoming(&build_frame(DME_ADDRESS, &[0xA0]));
    let next_response = build_frame(DME_ADDRESS, &[0xB0, 0xB1]);
    link.queue_incoming(&next_response);
    let mut ds2 = Ds2::new(link.clone(), TIMEOUT);

    ds2.request(DME_ADDRESS, &COMMAND).unwrap();

    assert_eq!(link.pending_incoming(), next_response.len());
}

#[test]
fn handles_consecutive_requests() {
    let link = MockLink::with_incoming(&build_frame(DME_ADDRESS, &[0xA0]));
    link.queue_incoming(&build_frame(DME_ADDRESS, &[0xB0, 0xB1]));
    let mut ds2 = Ds2::new(link, TIMEOUT);

    assert_eq!(ds2.request(DME_ADDRESS, &COMMAND), Ok(&[0xA0][..]));
    assert_eq!(ds2.request(DME_ADDRESS, &COMMAND), Ok(&[0xB0, 0xB1][..]));
}

#[test]
fn passes_configured_timeout_to_every_receive() {
    let link = MockLink::with_incoming(&build_frame(DME_ADDRESS, &[0xA0, 0x01]));
    link.limit_chunk(1);
    let mut ds2 = Ds2::new(link.clone(), TIMEOUT);

    ds2.request(DME_ADDRESS, &COMMAND).unwrap();

    let timeouts = link.receive_timeouts();
    assert!(!timeouts.is_empty());
    assert!(timeouts.iter().all(|&timeout| TIMEOUT == timeout));
}

#[test]
fn times_out_when_ecu_is_silent() {
    assert_eq!(request_with_incoming(&[]), Err(Error::Timeout));
}

#[test]
fn times_out_on_truncated_header() {
    assert_eq!(request_with_incoming(&[DME_ADDRESS]), Err(Error::Timeout));
}

#[test]
fn times_out_on_truncated_body() {
    let response = build_frame(DME_ADDRESS, &[0xA0, 0x01, 0x02]);
    let truncated = &response[..response.len() - 1];

    assert_eq!(request_with_incoming(truncated), Err(Error::Timeout));
}

#[test]
fn times_out_when_only_header_arrives() {
    let response = build_frame(DME_ADDRESS, &[0xA0]);

    assert_eq!(request_with_incoming(&response[..2]), Err(Error::Timeout));
}

#[test]
fn rejects_length_below_minimum_frame() {
    for length in 0..3 {
        assert_eq!(
            request_with_incoming(&[DME_ADDRESS, length]),
            Err(Error::InvalidLength),
            "length byte {length}"
        );
    }
}

#[test]
fn rejects_corrupted_checksum() {
    let mut response = build_frame(DME_ADDRESS, &[0xA0, 0x01]);
    *response.last_mut().unwrap() ^= 0xFF;

    assert_eq!(request_with_incoming(&response), Err(Error::Checksum));
}

#[test]
fn rejects_corrupted_payload_byte() {
    let mut response = build_frame(DME_ADDRESS, &[0xA0, 0x01]);
    response[2] ^= 0x01;

    assert_eq!(request_with_incoming(&response), Err(Error::Checksum));
}

#[test]
fn rejects_response_from_other_address() {
    let response = build_frame(OTHER_ADDRESS, &[0xA0]);

    assert_eq!(
        request_with_incoming(&response),
        Err(Error::UnexpectedAddress(OTHER_ADDRESS))
    );
}

#[test]
fn reports_checksum_before_address_mismatch() {
    let mut response = build_frame(OTHER_ADDRESS, &[0xA0]);
    *response.last_mut().unwrap() ^= 0xFF;

    assert_eq!(request_with_incoming(&response), Err(Error::Checksum));
}
