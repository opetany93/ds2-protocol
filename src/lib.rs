//! `no_std` implementation of BMW's DS2 diagnostic protocol.
//!
//! Implement [`SerialLink`] for your serial port, then send requests with [`Ds2::request`].
//! The crate is transport agnostic: on K-line, filter the echo in your [`SerialLink`].

#![no_std]

use core::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Timeout,
    Checksum,
    UnexpectedAddress(u8),
    /// Command longer than 252 bytes.
    CommandTooLong,
    InvalidLength,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Timeout => f.write_str("response timeout"),
            Self::Checksum => f.write_str("response checksum mismatch"),
            Self::UnexpectedAddress(address) => {
                write!(f, "response from unexpected address 0x{address:02X}")
            }
            Self::CommandTooLong => f.write_str("command too long"),
            Self::InvalidLength => f.write_str("invalid response length"),
        }
    }
}

impl core::error::Error for Error {}

const HEADER_LEN: usize = 2;
const CHECKSUM_LEN: usize = 1;
const MAX_FRAME_LEN: usize = 255;

pub trait SerialLink {
    /// Returns the number of bytes read, 0 on timeout.
    fn receive(&mut self, buffer: &mut [u8], timeout: Duration) -> usize;
    fn transmit(&mut self, data: &[u8]);
}

pub struct Ds2<T: SerialLink> {
    serial_link: T,
    response: [u8; MAX_FRAME_LEN],
    response_timeout: Duration,
}

impl<T: SerialLink> Ds2<T> {
    pub fn new(serial_link: T, response_timeout: Duration) -> Self {
        Self {
            serial_link,
            response: [0u8; MAX_FRAME_LEN],
            response_timeout,
        }
    }

    /// Returns the response payload without header and checksum.
    pub fn request(&mut self, address: u8, command: &[u8]) -> Result<&[u8], Error> {
        self.send_command(address, command)?;
        let len = Self::receive_response(
            &mut self.serial_link,
            &mut self.response,
            self.response_timeout,
        )?;

        if address != self.response[0] {
            return Err(Error::UnexpectedAddress(self.response[0]));
        }

        Ok(&self.response[HEADER_LEN..len - CHECKSUM_LEN])
    }

    fn send_command(&mut self, address: u8, command: &[u8]) -> Result<(), Error> {
        let frame_length = HEADER_LEN + command.len() + CHECKSUM_LEN;
        if frame_length > MAX_FRAME_LEN {
            return Err(Error::CommandTooLong);
        }

        let checksum_idx = frame_length - CHECKSUM_LEN;

        let mut frame = [0u8; MAX_FRAME_LEN];
        frame[0] = address;
        frame[1] = frame_length as u8;
        frame[HEADER_LEN..checksum_idx].copy_from_slice(command);
        frame[checksum_idx] = Self::calculate_checksum(&frame[..checksum_idx]);

        self.serial_link.transmit(&frame[..frame_length]);

        Ok(())
    }

    fn receive_response(
        link: &mut T,
        buffer: &mut [u8],
        timeout: Duration,
    ) -> Result<usize, Error> {
        Self::receive_exact(link, &mut buffer[..HEADER_LEN], timeout)?;

        let frame_length = buffer[1] as usize;
        if frame_length < HEADER_LEN + CHECKSUM_LEN {
            return Err(Error::InvalidLength);
        }

        Self::receive_exact(link, &mut buffer[HEADER_LEN..frame_length], timeout)?;

        let checksum_idx = frame_length - CHECKSUM_LEN;
        let calculated_checksum = Self::calculate_checksum(&buffer[..checksum_idx]);
        if buffer[checksum_idx] != calculated_checksum {
            return Err(Error::Checksum);
        }

        Ok(frame_length)
    }

    fn receive_exact(link: &mut T, buffer: &mut [u8], timeout: Duration) -> Result<(), Error> {
        let mut filled = 0;
        while filled < buffer.len() {
            let received = link.receive(&mut buffer[filled..], timeout);
            if received == 0 {
                return Err(Error::Timeout);
            }
            filled += received;
        }
        Ok(())
    }

    fn calculate_checksum(data: &[u8]) -> u8 {
        data.iter().fold(0u8, |acc, &byte| acc ^ byte)
    }
}
