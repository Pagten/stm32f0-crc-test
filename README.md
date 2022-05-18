# stm32f0-crc-test
Validates the output of the STM32F0 CRC peripheral against a software CRC implementation.

## Description
This application is intended to test the functionality of the STM32F0-CRC peripheral contributed to [Renode](https://github.com/renode/renode). It runs a large number of CRC calculations, with a wide variety of configuration parameters. Each calculation is run twice: once using the hardware CRC peripheral (emulated by Renode when running in simulation) and once using the software CRC implementation provided by the [crc-any](https://crates.io/crates/crc-any) crate. The output of both calculations should be identical, otherwise this indicates a bug in the CRC peripheral emulation. The application reports on the CRC calculations it performs via USART1 (configured on pins PA9 and PA10).

## Target
The application is written for STM32F072 microprocessors. It can run on actual hardware such as the STM32F072B Discovery board, or on a compatible board emulated by [Renode](https://github.com/renode/renode). Of course, when running on actual hardware, all hardware and software CRC calculations should match. This has been verified using an STM32F072B Discovery board.

## Toolchain
This application has been verified to compile correctly on the `nightly-2022-01-11` Rust toolchain.
