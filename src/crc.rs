use crate::stm32::crc::cr::POLYSIZE_A;
use alloc::vec::Vec;
use stm32f0xx_hal::stm32::{self, crc::cr::REV_OUT_A};
use strum::{Display, EnumIter};

#[derive(Clone)]
pub struct CrcConfig {
    pub reflect_input: BitReversal,
    pub reflect_output: bool,
    pub initial_value: u32,
    pub polynomial: Polynomial,
}

#[derive(Copy, Clone, Display, EnumIter, PartialEq)]
pub enum BitReversal {
    Disabled,
    By8Bits,
    By16Bits,
    By32Bits,
}

impl From<BitReversal> for stm32::crc::cr::REV_IN_A {
    fn from(bit_reversal: BitReversal) -> Self {
        match bit_reversal {
            BitReversal::Disabled => Self::NORMAL,
            BitReversal::By8Bits => Self::BYTE,
            BitReversal::By16Bits => Self::HALFWORD,
            BitReversal::By32Bits => Self::WORD,
        }
    }
}

#[derive(Copy, Clone, Display)]
pub enum Polynomial {
    Crc7(u8),
    Crc8(u8),
    Crc16(u16),
    Crc32(u32),
}

impl Polynomial {
    pub fn poly_size(&self) -> POLYSIZE_A {
        match self {
            Polynomial::Crc7(_) => POLYSIZE_A::POLYSIZE7,
            Polynomial::Crc8(_) => POLYSIZE_A::POLYSIZE8,
            Polynomial::Crc16(_) => POLYSIZE_A::POLYSIZE16,
            Polynomial::Crc32(_) => POLYSIZE_A::POLYSIZE32,
        }
    }

    pub fn bits(&self) -> u8 {
        match self {
            Polynomial::Crc7(_) => 7,
            Polynomial::Crc8(_) => 8,
            Polynomial::Crc16(_) => 16,
            Polynomial::Crc32(_) => 32,
        }
    }

    pub fn value(&self) -> u32 {
        match self {
            Polynomial::Crc7(value) |
            Polynomial::Crc8(value) => *value as u32,
            Polynomial::Crc16(value) => *value as u32,
            Polynomial::Crc32(value) => *value as u32,
        }
    }
}

pub struct CrcCalculation {
    pub config: CrcConfig,
    pub steps: Vec<Step>,
}

pub enum Step {
    Data8(u8),
    Data16(u16),
    Data32(u32),
}

mod hardware {
    use super::*;

    impl CrcCalculation {
        pub fn run_hardware(&self, crc: &mut stm32::CRC) -> u32 {
            crc.init.write(|w| w.init().bits(self.config.initial_value));

            // Current version of stm32f0 crate doesn't yet support the `pol` register, so we write the
            // register manually.
            unsafe { core::ptr::write_volatile(0x40023014 as *mut u32, self.config.polynomial.value()); }
            //crc.pol.write(|w| w.pol().bits(self.config.polynomial.value()));

            crc.cr.write(|w| w
                .rev_in().variant(self.config.reflect_input.into())
                .rev_out().variant(if self.config.reflect_output { REV_OUT_A::REVERSED } else { REV_OUT_A::NORMAL })
                .polysize().variant(self.config.polynomial.poly_size())
                .reset().set_bit()
            );

            for step in &self.steps {
                match step {
                    Step::Data8(value) => crc.dr8().write(|w| w.dr8().bits(*value)),
                    Step::Data16(value) => crc.dr16().write(|w| w.dr16().bits(*value)),
                    Step::Data32(value) => crc.dr().write(|w| w.dr().bits(*value)),
                }
            }
            crc.dr().read().bits()
        }
    }
}

mod software {
    use super::*;

    impl CrcCalculation {
        pub fn run_software(&self) -> u32 {
            let mut crc = crc_any::CRCu32::create_crc(
                self.config.polynomial.value(),
                self.config.polynomial.bits(),
                self.config.initial_value,
                0,
                false,
            );

            let input_reversal = self.config.reflect_input;
            for step in &self.steps {
                match step {
                    Step::Data8(value) => crc.digest(&input_reversal.reflect8(*value).to_be_bytes()),
                    Step::Data16(value) => crc.digest(&input_reversal.reflect16(*value).to_be_bytes()),
                    Step::Data32(value) => crc.digest(&input_reversal.reflect32(*value).to_be_bytes()),
                }
            }

            let result = crc.get_crc();
            if self.config.reflect_output {
                self.config.polynomial.reflect_output(result)
            } else {
                result
            }
        }
    }

    impl BitReversal {
        fn reflect32(&self, v: u32) -> u32 {
            match self {
                Self::Disabled => v,
                Self::By8Bits => reflect8(v),
                Self::By16Bits => reflect16(v),
                Self::By32Bits => reflect32(v),
            }
        }

        fn reflect16(&self, v: u16) -> u16 {
            match self {
                Self::Disabled => v,
                Self::By8Bits => reflect8(v as u32) as u16,
                Self::By16Bits |
                Self::By32Bits => reflect16(v as u32) as u16,
            }
        }

        fn reflect8(&self, v: u8) -> u8 {
            match self {
                Self::Disabled => v,
                Self::By8Bits |
                Self::By16Bits |
                Self::By32Bits => reflect8(v as u32) as u8,
            }
        }
    }

    impl Polynomial {
        pub fn reflect_output(&self, output: u32) -> u32 {
            match self {
                Polynomial::Crc7(_) => reflect8(output) >> 1,
                Polynomial::Crc8(_) => reflect8(output),
                Polynomial::Crc16(_) => reflect16(output),
                Polynomial::Crc32(_) => reflect32(output),
            }
        }
    }

    fn reflect8(mut v: u32) -> u32 {
        v = ((v >> 1) & 0x55555555) | ((v & 0x55555555) << 1); // swap odd and even bits
        v = ((v >> 2) & 0x33333333) | ((v & 0x33333333) << 2); // swap consecutive pairs of bits
        v = ((v >> 4) & 0x0F0F0F0F) | ((v & 0x0F0F0F0F) << 4); // swap nibbles ...
        v // bits of each byte have now been reversed
    }

    fn reflect16(mut v: u32) -> u32 {
        v = reflect8(v);
        v = ((v >> 8) & 0x00FF00FF) | ((v & 0x00FF00FF) << 8); // swap bytes
        v // bits of each 16-bit chunk have now been reversed
    }

    fn reflect32(mut v: u32) -> u32 {
        v = reflect16(v);
        v = (v >> 16) | (v << 16); // swap 16-bit pairs
        v // all bits have now been reversed
    }
}
