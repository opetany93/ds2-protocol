# ds2-protocol

`no_std` implementation of BMW's DS2 diagnostic protocol, so it runs on bare-metal microcontrollers as well as on a PC. It builds request frames, reads the response and validates it. The serial port is up to you: implement `SerialLink` for your hardware.

> **Note:** This crate implements only the DS2 protocol. It does not filter K-line echo; handle that in your `SerialLink` implementation if your interface produces it.

## Frame format

```
| address | length | data ... | checksum |
```

- `length` covers the whole frame, including address and checksum (3 to 255 bytes).
- `checksum` is the XOR of all preceding bytes.

Example: command `00` to address `0x12` is sent as `12 04 00 16`.

## Usage

```rust
use core::time::Duration;
use ds2_protocol::{Ds2, SerialLink};

struct Uart { /* ... */ }

impl SerialLink for Uart {
    fn receive(&mut self, buffer: &mut [u8], timeout: Duration) -> usize {
        // Read up to buffer.len() bytes. Return 0 on timeout.
    }

    fn transmit(&mut self, data: &[u8]) {
        // Write all bytes.
    }
}

let mut ds2 = Ds2::new(Uart { /* ... */ }, Duration::from_millis(100));
let payload = ds2.request(0x12, &[0x00])?;
```

`request` returns the response payload, without address, length and checksum.

`receive` may return fewer bytes than requested; the library keeps reading until the frame is complete. A return value of `0` is treated as a timeout.

## Errors

| Error | Cause |
|---|---|
| `Timeout` | `receive` returned 0 before the full frame arrived |
| `Checksum` | Response checksum does not match |
| `UnexpectedAddress(u8)` | Response came from a different address than requested |
| `InvalidLength` | Response length byte is below 3 |
| `CommandTooLong` | Command exceeds 252 bytes; nothing is sent |

## Tests

```
cargo test
```

Tests live in `tests/` and run against an in-memory `SerialLink` mock (`tests/common/mod.rs`).

Not affiliated with or endorsed by BMW AG.
