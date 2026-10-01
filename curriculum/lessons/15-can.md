# Automotive I: CAN, Signals & ISO-TP

A modern car has dozens of ECUs talking over several networks: CAN and
CAN FD for most control traffic, LIN for cheap body electronics, FlexRay in
some chassis systems, and Automotive Ethernet for cameras, infotainment and
backbones. CAN is still the one every embedded automotive engineer must know
cold, and its frames are small, fixed-format and perfect for no_std code.

## Classical CAN in five facts

1. **Broadcast bus, no addresses.** Every node sees every frame. A frame's
   **identifier** says *what* the data is (e.g. "engine speed"), not who it is for.
2. **IDs are 11-bit (standard, 0..=0x7FF) or 29-bit (extended,
   0..=0x1FFF_FFFF).**
3. **Arbitration by ID.** Nodes transmit simultaneously; a dominant 0 bit
   overwrites a recessive 1, and a node that sends 1 but reads 0 backs off.
   So the **lowest ID wins**. Priority is the ID itself, and it's
   non-destructive (the winner's frame isn't corrupted).
4. **Payload 0-8 bytes** in classical CAN, with the length in the 4-bit **DLC**.
5. **Robust error handling**: CRC, ACK slot, bit stuffing, and error counters
   that put a misbehaving node into *error passive* and finally *bus off*.

When a standard and an extended frame start with the same 11 bits, the
**standard frame wins**: in the arbitration field, the standard frame's RTR/IDE
bits are dominant where the extended frame has recessive SRR/IDE bits.

### CAN FD

CAN FD keeps arbitration at the nominal bit rate, then switches to a faster
data phase and allows **up to 64 bytes**. The 4-bit DLC can't count to 64, so
values 9..=15 map to 12, 16, 20, 24, 32, 48, 64 bytes. A payload must be padded
up to the next valid length.

### In Rust: embedded-can

The `embedded-can` crate defines the shared vocabulary that bxcan, fdcan,
socketcan, MCP2515 drivers etc. implement:

```rust
pub enum Id { Standard(StandardId), Extended(ExtendedId) }
impl StandardId { pub const fn new(raw: u16) -> Option<Self>; /* None if > 0x7FF */ }
pub trait Frame: Sized {
    fn new(id: impl Into<Id>, data: &[u8]) -> Option<Self>;   // None if data.len() > 8
    fn new_remote(id: impl Into<Id>, dlc: usize) -> Option<Self>;
    fn is_extended(&self) -> bool;
    fn id(&self) -> Id;
    fn dlc(&self) -> usize;
    fn data(&self) -> &[u8];
}
pub trait blocking::Can { type Frame: Frame; type Error; fn transmit(..); fn receive(..); }
```

Note the validated constructors returning `Option`: an `Id` can never hold an
out-of-range value, so code receiving one never re-checks. That's the
"parse, don't validate" pattern, and it's good safety practice.

## J1939: structure inside the 29-bit ID

Heavy vehicles (trucks, buses, agricultural and construction machines) use
**SAE J1939**, which gives the extended ID a structure:

```text
 bits 28..26  priority (0 = highest)
 bit  25      EDP (extended data page)
 bit  24      DP  (data page)
 bits 23..16  PF  (PDU format)
 bits 15..8   PS  (PDU specific): destination address if PF < 240, else group extension
 bits  7..0   SA  (source address)
```

The **PGN** (parameter group number) identifies the message type:
`PGN = EDP<<17 | DP<<16 | PF<<8 | (PS if PF >= 240 else 0)`. Messages with
PF < 240 (PDU1) are addressed to one destination; PF >= 240 (PDU2) are broadcast.
Example: engine speed lives in PGN 61444 (0xF004, "EEC1").

## Signals: the DBC view of a frame

ECUs don't exchange "bytes", they exchange **signals**: engine speed, gear,
door states. A **DBC file** (the de-facto standard database format) describes
each signal: start bit, length, byte order, signedness, scaling.

```text
SG_ EngineSpeed : 24|16@1+ (0.125,0) [0|8031.875] "rpm" Vector__XXX
                  │  │  │└ unsigned      │      └ range       └ unit
                  │  │  └ @1 = Intel (little-endian), @0 = Motorola (big-endian)
                  │  └ length in bits
                  └ start bit
                 physical = raw × 0.125 + 0
```

Bits are numbered `byte * 8 + bit_in_byte`, with bit 0 the least
significant bit of byte 0.

- **Intel / little-endian (`@1`)**: the start bit is the signal's **LSB**; the
  signal occupies start, start+1, start+2, … continuing into higher bytes.
  A 16-bit signal at start bit 0 is `data[0] | data[1] << 8`.
- **Motorola / big-endian (`@0`)**: the start bit is the signal's **MSB**.
  Moving towards less significant bits, go *down* within a byte (bit 7 → 0),
  then jump to bit 7 of the **next** byte. A 16-bit signal with start bit 7 is
  `data[0] << 8 | data[1]`. The bit-walk rule: from position `p`, the next
  less significant bit is `p - 1`, unless `p % 8 == 0`, in which case it's
  `p + 15`.

Signed signals are two's complement in `length` bits, so sign-extend after
extracting. Scaling is often decimal (0.1, 0.125, 0.01): in firmware, keep it
as an integer ratio (`× 1/8`) or fixed-point to avoid floats.

## ISO-TP: messages longer than one frame

Diagnostics (UDS, ISO 14229) and flashing exchange messages up to 4095 bytes
(more with CAN FD extensions). **ISO-TP (ISO 15765-2)** segments them. The first
nibble of byte 0 is the *protocol control information* (PCI) type:

| PCI | Frame | Layout (classical CAN, 8 bytes) |
|---|---|---|
| 0 | Single Frame (SF) | `0L` + up to 7 data bytes, L = length 1..=7 |
| 1 | First Frame (FF) | `1L LL` (12-bit total length, ≥ 8) + 6 data bytes |
| 2 | Consecutive Frame (CF) | `2N` + up to 7 data bytes, N = sequence number 1, 2, … 15, 0, 1, … |
| 3 | Flow Control (FC) | `3S BS ST`: status (0 = continue), block size, separation time |

After a First Frame the **receiver** sends a Flow Control frame, then the
sender streams Consecutive Frames. The receiver must check sequence numbers
(a wrong one aborts reception), enforce timeouts, and refuse messages larger
than its buffer, all with a fixed-size buffer and no allocation. A small,
classic state machine, and a classic place for security bugs (length
handling!).

## On Linux: SocketCAN

Linux exposes CAN interfaces as network sockets (`can0`, `vcan0`). The
`socketcan` crate implements the `embedded-can` traits on top, so the same
frame-handling code can run on a Linux gateway and on a microcontroller. With
`can-utils` you can create a virtual bus for testing:

```console
$ sudo ip link add dev vcan0 type vcan && sudo ip link set up vcan0
$ candump vcan0 &           # watch traffic
$ cansend vcan0 123#DEADBEEF
```

> **Toyota lens:** CAN/CAN FD signals, DBC databases, ISO-TP and UDS
> diagnostics are daily bread in ECU development, and their parsing code is
> both safety- and security-relevant (ISO/SAE 21434 treats the in-vehicle
> network as an attack surface). Type-safe IDs, panic-free decoders over fixed
> buffers, explicit handling of every malformed frame, and exhaustive tests are
> exactly what a Rust engineer is expected to bring. Many interview take-home
> tasks are "parse these CAN frames".

> **Interview:** "Why does the lowest CAN ID win arbitration?", "What's the
> difference between Intel and Motorola signal layout?", "How does ISO-TP
> send 100 bytes over 8-byte frames?", "What changes with CAN FD?"
