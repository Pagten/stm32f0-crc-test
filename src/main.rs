#![no_std]
#![no_main]
#![deny(warnings)]
#![allow(dead_code)]
#![feature(alloc_error_handler)]

extern crate alloc;

use alloc::{format, vec, vec::Vec};
use alloc_cortex_m::CortexMHeap;
use core::alloc::Layout;
use core::fmt::Write;
use cortex_m_rt::entry;
use panic_halt as _;
use stm32f0xx_hal::{stm32, prelude::*};
use stm32f0xx_hal::serial::Serial;
use strum::IntoEnumIterator;

use crate::crc::{BitReversal, CrcCalculation, CrcConfig, Polynomial, Step};

mod crc;

const HEAP_SIZE: usize = 2048;

#[global_allocator]
static ALLOCATOR: CortexMHeap = CortexMHeap::empty();


// ***************************************** Test values *****************************************
static POLYNOMIALS: [Polynomial; 5] = [
    Polynomial::Crc7(0x9),
    Polynomial::Crc8(0x7),
    Polynomial::Crc16(0x8005),
    Polynomial::Crc32(0x1EDC6F41),
    Polynomial::Crc32(0x04C11DB7),
];
static INITIAL_VALUES: [u32; 3] = [
    0x00000000,
    0xFFFFFFFF,
    0x000000FF,
];

fn steps() -> Vec<Vec<Step>> {
    vec![
        vec![],
        vec![Step::Data8(0x42)],
        vec![Step::Data16(0x4232)],
        vec![Step::Data8(0x42), Step::Data8(0x32), Step::Data8(0x68), Step::Data8(0xA4)],
        vec![Step::Data16(0x4232), Step::Data16(0x68A4)],
        vec![Step::Data32(0x423268A4)],
        vec![Step::Data32(0x423268A4), Step::Data32(0xAD91FE38)],
    ]
}
// ***********************************************************************************************

#[entry]
fn main() -> ! {
    cortex_m::interrupt::free(|cs| {
        init_allocator();

        let mut dp = stm32::Peripherals::take().unwrap();

        // Enable CRC clock
        dp.RCC.ahbenr.modify(|_, w| w.crcen().enabled());
        dp.RCC.apb2enr.modify(|_, w| w.usart1en().enabled());
        let mut rcc = dp.RCC.configure().freeze(&mut dp.FLASH);

        let gpioa = dp.GPIOA.split(&mut rcc);
        let tx = gpioa.pa9.into_alternate_af1(cs);
        let rx = gpioa.pa10.into_alternate_af1(cs);
        let mut serial = Serial::usart1(dp.USART1, (tx, rx), 115_200.bps(), &mut rcc);
        let mut crc = dp.CRC;

        run_tests(&mut serial, &mut crc);
        loop {
            // NOP
        }
    })
}

fn run_tests<S: Write>(serial: &mut S, crc: &mut stm32::CRC) {
    let mut passed = 0u32;
    let mut failed= 0u32;
    serial.write_str(
    "\r\n\
     Type  | Polynomial | Input refl | Output refl |   Init val | Test |     Output | Result\r\n\
     ---------------------------------------------------------------------------------------\r\n"
    ).unwrap();
    for polynomial in POLYNOMIALS {
        for reflect_input in BitReversal::iter() {
            for reflect_output in [false, true] {
                for initial_value in INITIAL_VALUES {
                    for (i, steps) in steps().into_iter().enumerate() {
                        let calculation = CrcCalculation {
                            config: CrcConfig {
                                reflect_input,
                                reflect_output,
                                initial_value,
                                polynomial
                            },
                            steps,
                        };

                        let name = format!("{:>5} | 0x{:08x} | {:>10} | {:>11} | 0x{:08x} | {:>4}",
                           polynomial, polynomial.value(), reflect_input, to_enabled_disabled(reflect_output), initial_value, i);
                        let pass = crc_test(serial, crc, &calculation, &name);
                        if pass { passed += 1; } else { failed += 1; }
                    }
                }
            }
        }
    }
    let result = if failed == 0 { "ok" } else { "FAILED" };
    serial.write_fmt(format_args!("test result: {}. {} passed; {} failed\r\n", result, passed, failed)).unwrap();
}

fn to_enabled_disabled(v: bool) -> &'static str {
    if v {
        "Enabled"
    } else {
        "Disabled"
    }
}

fn crc_test<S: Write>(serial: &mut S, crc: &mut stm32::CRC, calculation: &CrcCalculation, name: &str) -> bool {
    let expected_output = calculation.run_software();
    let output = calculation.run_hardware(crc);
    serial.write_fmt(format_args!("{} | 0x{:08x} | ", name, output)).unwrap();
    if output == expected_output {
        serial.write_str("    OK\r\n").unwrap();
        true
    } else {
        serial.write_fmt(format_args!("failed - expected 0x{:08x})\r\n", expected_output)).unwrap();
        false
    }
}

fn init_allocator() {
    use core::mem::MaybeUninit;
    static mut HEAP: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
    unsafe { ALLOCATOR.init((&mut HEAP).as_ptr() as usize, HEAP_SIZE) }
}

#[alloc_error_handler]
fn oom(_: Layout) -> ! {
    loop {}
}