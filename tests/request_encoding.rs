//! What `Ds2::request` puts on the wire.

mod common;

use common::{MAX_PAYLOAD_LEN, MockLink, TIMEOUT, build_frame, xor_checksum};
use ds2_protocol::{Ds2, Error};

const DME_ADDRESS: u8 = 0x12;

fn link_answering_with_empty_payload() -> MockLink {
    MockLink::with_incoming(&build_frame(DME_ADDRESS, &[]))
}

#[test]
fn encodes_known_ds2_frame() {
    let link = link_answering_with_empty_payload();
    let mut ds2 = Ds2::new(link.clone(), TIMEOUT);

    ds2.request(DME_ADDRESS, &[0x00]).unwrap();

    assert_eq!(link.transmitted(), vec![vec![0x12, 0x04, 0x00, 0x16]]);
}

#[test]
fn encodes_empty_command() {
    let link = link_answering_with_empty_payload();
    let mut ds2 = Ds2::new(link.clone(), TIMEOUT);

    ds2.request(DME_ADDRESS, &[]).unwrap();

    assert_eq!(link.transmitted(), vec![vec![0x12, 0x03, 0x11]]);
}

#[test]
fn length_byte_counts_whole_frame() {
    let link = link_answering_with_empty_payload();
    let mut ds2 = Ds2::new(link.clone(), TIMEOUT);
    let command = [0xAA; 10];

    ds2.request(DME_ADDRESS, &command).unwrap();

    let frame = &link.transmitted()[0];
    assert_eq!(frame.len(), usize::from(frame[1]));
    assert_eq!(frame.len(), command.len() + 3);
}

#[test]
fn checksum_makes_whole_frame_xor_to_zero() {
    let link = link_answering_with_empty_payload();
    let mut ds2 = Ds2::new(link.clone(), TIMEOUT);

    ds2.request(DME_ADDRESS, &[0x00, 0x7F, 0x80, 0xFF, 0x55])
        .unwrap();

    assert_eq!(xor_checksum(&link.transmitted()[0]), 0);
}

#[test]
fn transmitted_frame_matches_reference_encoder() {
    let link = link_answering_with_empty_payload();
    let mut ds2 = Ds2::new(link.clone(), TIMEOUT);
    let command = [0x06, 0x00, 0x00, 0x10];

    ds2.request(DME_ADDRESS, &command).unwrap();

    assert_eq!(link.transmitted(), vec![build_frame(DME_ADDRESS, &command)]);
}

#[test]
fn accepts_longest_command() {
    let link = link_answering_with_empty_payload();
    let mut ds2 = Ds2::new(link.clone(), TIMEOUT);
    let command = [0x5A; MAX_PAYLOAD_LEN];

    ds2.request(DME_ADDRESS, &command).unwrap();

    assert_eq!(link.transmitted(), vec![build_frame(DME_ADDRESS, &command)]);
}

#[test]
fn rejects_command_one_byte_too_long() {
    let link = link_answering_with_empty_payload();
    let mut ds2 = Ds2::new(link.clone(), TIMEOUT);

    let result = ds2.request(DME_ADDRESS, &[0x5A; MAX_PAYLOAD_LEN + 1]);

    assert_eq!(result, Err(Error::CommandTooLong));
}

#[test]
fn transmits_nothing_when_command_too_long() {
    let link = link_answering_with_empty_payload();
    let mut ds2 = Ds2::new(link.clone(), TIMEOUT);

    let _ = ds2.request(DME_ADDRESS, &[0x5A; MAX_PAYLOAD_LEN + 1]);

    assert!(link.transmitted().is_empty());
    assert!(
        link.receive_timeouts().is_empty(),
        "must not wait for a response either"
    );
}

#[test]
fn transmits_once_per_request() {
    let link = MockLink::new();
    link.queue_incoming(&build_frame(DME_ADDRESS, &[]));
    link.queue_incoming(&build_frame(DME_ADDRESS, &[]));
    let mut ds2 = Ds2::new(link.clone(), TIMEOUT);

    ds2.request(DME_ADDRESS, &[0x01]).unwrap();
    ds2.request(DME_ADDRESS, &[0x02]).unwrap();

    assert_eq!(
        link.transmitted(),
        vec![
            build_frame(DME_ADDRESS, &[0x01]),
            build_frame(DME_ADDRESS, &[0x02])
        ]
    );
}

#[test]
fn transmits_before_waiting_for_response() {
    let link = MockLink::new();
    let mut ds2 = Ds2::new(link.clone(), TIMEOUT);

    let result = ds2.request(DME_ADDRESS, &[0x00]);

    assert_eq!(result, Err(Error::Timeout));
    assert_eq!(link.transmitted(), vec![build_frame(DME_ADDRESS, &[0x00])]);
}
