# Drivers & the embedded-hal Model

A temperature sensor driver shouldn't care whether it runs on an STM32, an
nRF52, an ESP32 or a Linux board. In C that portability usually comes from
a hand-written abstraction layer per project. In Rust it comes from a shared
set of traits, **embedded-hal**, which HALs implement and drivers consume.

```text
   application
       │  uses
   driver crate (bme280, ssd1306, mcp2515, your code)      generic over traits
       │  embedded-hal traits: OutputPin, I2c, SpiDevice, DelayNs
   HAL crate (stm32f4xx-hal, embassy-stm32, nrf-hal, linux-embedded-hal)
       │  implements the traits using
   PAC (registers)  →  hardware
```

## embedded-hal 1.0

Version 1.0 (January 2024) stabilised the core blocking traits:

```rust
// digital
pub trait OutputPin: ErrorType {
    fn set_low(&mut self) -> Result<(), Self::Error>;
    fn set_high(&mut self) -> Result<(), Self::Error>;
    fn set_state(&mut self, state: PinState) -> Result<(), Self::Error> { ... }
}
pub trait InputPin: ErrorType {
    fn is_high(&mut self) -> Result<bool, Self::Error>;
    fn is_low(&mut self) -> Result<bool, Self::Error>;
}

// delay: only delay_ns is required; delay_us/delay_ms are provided
pub trait DelayNs { fn delay_ns(&mut self, ns: u32); ... }

// I2C: one required method; read/write/write_read are provided via transaction
pub trait I2c<A: AddressMode = SevenBitAddress>: ErrorType {
    fn transaction(&mut self, address: A, operations: &mut [Operation<'_>]) -> Result<(), Self::Error>;
    fn write_read(&mut self, address: A, write: &[u8], read: &mut [u8]) -> Result<(), Self::Error> { ... }
    ...
}

// SPI, two levels:
pub trait SpiBus: ErrorType { fn transfer(...); fn write(...); fn read(...); fn flush(...); ... }
pub trait SpiDevice: ErrorType {
    fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), Self::Error>;
    ...
}
```

Design points worth knowing (they come up in interviews):

- **Everything is fallible.** Even `set_high` returns a `Result`: pins on an I/O
  expander *can* fail. On-chip pins use `Infallible`.
- **Errors are associated types** with a common `Error` trait exposing
  `kind()` (e.g. `i2c::ErrorKind::NoAcknowledge`), so drivers can react to error
  categories without knowing the HAL.
- **`SpiDevice` vs `SpiBus`.** A *bus* is the shared SCK/MOSI/MISO lines. A
  *device* is the bus plus one chip-select, and its `transaction` asserts CS,
  runs all operations, then deasserts CS, with exclusive access to the bus
  meanwhile. **Drivers take `SpiDevice`.** Sharing one bus between several
  devices is handled by `embedded-hal-bus` (`ExclusiveDevice`,
  `RefCellDevice`, `CriticalSectionDevice`).
- **`&mut self` everywhere**: drivers own (or exclusively borrow) their bus.
- **Companion crates**: `embedded-hal-async` (async versions, used by
  Embassy), `embedded-hal-nb` (non-blocking serial), `embedded-io` (byte
  streams), `embedded-can` (CAN frames, module 15).

## Anatomy of a driver

```rust
pub struct Tmp<I2C> {
    i2c: I2C,
    address: u8,
}

#[derive(Debug, PartialEq)]
pub enum Error<E> {
    Bus(E),               // the HAL's error, wrapped
    WrongDevice(u8),      // driver-level errors
}

impl<I2C: I2c> Tmp<I2C> {
    pub fn new(i2c: I2C, address: u8) -> Self { Tmp { i2c, address } }

    fn read_reg(&mut self, reg: u8) -> Result<u8, Error<I2C::Error>> {
        let mut buf = [0u8; 1];
        self.i2c.write_read(self.address, &[reg], &mut buf).map_err(Error::Bus)?;
        Ok(buf[0])
    }

    pub fn release(self) -> I2C { self.i2c }   // give the bus back
}
```

Conventions:

- Take the bus **by value** (or `&mut`) and offer `release()`.
- Generic error `Error<E>` wrapping the bus error, plus driver-specific
  variants. No `unwrap`, ever: a sensor that NACKs is a normal event.
- Register addresses and bit masks as named constants, with read-modify-write
  helpers that only touch the bits they own.
- Physical conversions in **integer/fixed-point** arithmetic (milli-degrees,
  millivolts) to avoid pulling in float code.
- Timeouts instead of `loop { if ready { break } }`: hardware can hang.

## Testing drivers without hardware

Because the driver is generic, tests can plug in **mock** implementations:

- *recording mocks* log every call (pin set high, delay 500 ms, I2C write
  [0x0F]) and the test asserts on the log;
- *simulating mocks* model the device's behaviour (a register file that
  responds to reads and writes), which is great for protocol logic;
- `embedded-hal-mock` provides expectation-based mocks for all the traits.

This is how real driver crates reach high test coverage, and how automotive
teams run most driver unit tests in CI without boards (software-in-the-loop).

> **Toyota lens:** the layering here mirrors AUTOSAR Classic: MCAL drivers
> (≈ HAL), ECU abstraction (≈ device drivers like a CAN-controller or sensor
> driver), and services on top. Safety-relevant drivers add **plausibility
> checks** (is the value physically possible? is it stuck? did it change too
> fast?) and often CRCs on sensor frames. The structure you learn here, a
> generic driver with typed errors and mocks, is what makes those checks
> testable.

> **Interview:** "How would you write a driver that works on any MCU?"
> (embedded-hal traits, generic over the bus, owned bus + `release`, typed
> errors). "SpiBus or SpiDevice?" (SpiDevice: CS management and bus
> sharing). "How do you test it?" (mocks on the host).
