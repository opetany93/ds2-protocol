//! How `Ds2::request` reads and validates the ECU response.

mod common;

use common::{MAX_RESPONSE_DATA_LEN, MockLink, TIMEOUT, build_frame, build_ok_response};
use ds2_protocol::{Ds2, Error, ResponseStatus};

const DME_ADDRESS: u8 = 0x12;
const OTHER_ADDRESS: u8 = 0x13;
const COMMAND: [u8; 1] = [0x00];
const UNKNOWN_STATUS: u8 = 0x42;

fn request_with_incoming(incoming: &[u8]) -> Result<Vec<u8>, Error> {
    let mut ds2 = Ds2::new(MockLink::with_incoming(incoming), TIMEOUT);
    ds2.request(DME_ADDRESS, &COMMAND).map(<[u8]>::to_vec)
}

#[test]
fn returns_payload_without_header_status_and_checksum() {
    let response = build_ok_response(DME_ADDRESS, &[0x01, 0x02]);

    assert_eq!(request_with_incoming(&response), Ok(vec![0x01, 0x02]));
}

#[test]
fn returns_empty_payload_for_minimal_frame() {
    let response = build_ok_response(DME_ADDRESS, &[]);

    assert_eq!(request_with_incoming(&response), Ok(vec![]));
}

#[test]
fn returns_longest_payload() {
    let data: Vec<u8> = (0..MAX_RESPONSE_DATA_LEN).map(|i| i as u8).collect();
    let response = build_ok_response(DME_ADDRESS, &data);

    assert_eq!(request_with_incoming(&response), Ok(data));
}

#[test]
fn reassembles_response_delivered_byte_by_byte() {
    let link = MockLink::with_incoming(&build_ok_response(DME_ADDRESS, &[0x11, 0x22, 0x33]));
    link.limit_chunk(1);
    let mut ds2 = Ds2::new(link, TIMEOUT);

    assert_eq!(
        ds2.request(DME_ADDRESS, &COMMAND),
        Ok(&[0x11, 0x22, 0x33][..])
    );
}

#[test]
fn reassembles_response_delivered_in_uneven_chunks() {
    let data: Vec<u8> = (0..40).collect();
    let link = MockLink::with_incoming(&build_ok_response(DME_ADDRESS, &data));
    link.limit_chunk(7);
    let mut ds2 = Ds2::new(link, TIMEOUT);

    assert_eq!(ds2.request(DME_ADDRESS, &COMMAND), Ok(&data[..]));
}

#[test]
fn reads_exactly_one_frame() {
    let link = MockLink::with_incoming(&build_ok_response(DME_ADDRESS, &[]));
    let next_response = build_ok_response(DME_ADDRESS, &[0xB0, 0xB1]);
    link.queue_incoming(&next_response);
    let mut ds2 = Ds2::new(link.clone(), TIMEOUT);

    ds2.request(DME_ADDRESS, &COMMAND).unwrap();

    assert_eq!(link.pending_incoming(), next_response.len());
}

#[test]
fn handles_consecutive_requests() {
    let link = MockLink::with_incoming(&build_ok_response(DME_ADDRESS, &[0x01]));
    link.queue_incoming(&build_ok_response(DME_ADDRESS, &[0xB0, 0xB1]));
    let mut ds2 = Ds2::new(link, TIMEOUT);

    assert_eq!(ds2.request(DME_ADDRESS, &COMMAND), Ok(&[0x01][..]));
    assert_eq!(ds2.request(DME_ADDRESS, &COMMAND), Ok(&[0xB0, 0xB1][..]));
}

#[test]
fn passes_configured_timeout_to_every_receive() {
    let link = MockLink::with_incoming(&build_ok_response(DME_ADDRESS, &[0x01]));
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
    let response = build_ok_response(DME_ADDRESS, &[0x01, 0x02]);
    let truncated = &response[..response.len() - 1];

    assert_eq!(request_with_incoming(truncated), Err(Error::Timeout));
}

#[test]
fn times_out_when_only_header_arrives() {
    let response = build_ok_response(DME_ADDRESS, &[]);

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
fn rejects_frame_without_status() {
    let response = build_frame(DME_ADDRESS, &[]);

    assert_eq!(request_with_incoming(&response), Err(Error::InvalidLength));
}

#[test]
fn rejects_corrupted_checksum() {
    let mut response = build_ok_response(DME_ADDRESS, &[0x01]);
    *response.last_mut().unwrap() ^= 0xFF;

    assert_eq!(request_with_incoming(&response), Err(Error::Checksum));
}

#[test]
fn rejects_corrupted_payload_byte() {
    let mut response = build_ok_response(DME_ADDRESS, &[0x01]);
    response[3] ^= 0x01;

    assert_eq!(request_with_incoming(&response), Err(Error::Checksum));
}

#[test]
fn rejects_response_from_other_address() {
    let response = build_ok_response(OTHER_ADDRESS, &[]);

    assert_eq!(
        request_with_incoming(&response),
        Err(Error::UnexpectedAddress(OTHER_ADDRESS))
    );
}

#[test]
fn reports_checksum_before_address_mismatch() {
    let mut response = build_ok_response(OTHER_ADDRESS, &[]);
    *response.last_mut().unwrap() ^= 0xFF;

    assert_eq!(request_with_incoming(&response), Err(Error::Checksum));
}

#[test]
fn rejects_non_ok_status() {
    for status in [
        ResponseStatus::Busy,
        ResponseStatus::Rejected,
        ResponseStatus::Nack,
    ] {
        let response = build_frame(DME_ADDRESS, &[status as u8, 0x01]);

        assert_eq!(
            request_with_incoming(&response),
            Err(Error::WrongResponseStatus(status)),
            "status {status}"
        );
    }
}

#[test]
fn rejects_unknown_status() {
    let response = build_frame(DME_ADDRESS, &[UNKNOWN_STATUS]);

    assert_eq!(
        request_with_incoming(&response),
        Err(Error::UnknownResponseStatus(UNKNOWN_STATUS))
    );
}

#[test]
fn reports_address_mismatch_before_status() {
    let response = build_frame(OTHER_ADDRESS, &[ResponseStatus::Rejected as u8]);

    assert_eq!(
        request_with_incoming(&response),
        Err(Error::UnexpectedAddress(OTHER_ADDRESS))
    );
}

#[test]
fn status_byte_round_trips_for_every_variant() {
    for status in [
        ResponseStatus::ConditionsNotMet,
        ResponseStatus::InvalidParameters,
        ResponseStatus::FunctionNotStarted,
        ResponseStatus::FunctionDone,
        ResponseStatus::AifNotProgrammed,
        ResponseStatus::Ok,
        ResponseStatus::Busy,
        ResponseStatus::Rejected,
        ResponseStatus::Parameter,
        ResponseStatus::Function,
        ResponseStatus::Number,
        ResponseStatus::Nack,
    ] {
        assert_eq!(ResponseStatus::try_from(status as u8), Ok(status));
    }
}
