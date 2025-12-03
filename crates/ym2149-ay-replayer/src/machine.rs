//! Z80 machine implementation with AY-3-8910 bridge.

use iz80::Machine;
use ym2149::Ym2149Backend;
use ym2149::ym2149::Ym2149;

use crate::format::AyBlock;

const ZX_PORT_MASK: u16 = 0xC002;
const ZX_REG_PORT: u16 = 0xC000;
const ZX_DATA_PORT: u16 = 0x8000;
const CPC_DATA_BUS_MASK: u16 = 0xFF00;
const CPC_PORT_A: u16 = 0xF400;
const CPC_PORT_C: u16 = 0xF600;

/// Memory + AY bus implementation used by the player.
pub struct AyMachine {
    memory: [u8; 65_536],
    chip: Ym2149,
    selected_register: u8,
    cpc_bus_latch: u8,
    cpc_control: u8,
    sample_rate: u32,
    cpc_clock_active: bool,
    cpc_requires_firmware: bool,
    #[cfg(feature = "trace-ports")]
    port_log: Vec<String>,
}

impl AyMachine {
    /// Create a machine with a fresh YM2149 chip.
    pub fn new(sample_rate: u32) -> Self {
        Self {
            memory: [0; 65_536],
            chip: Ym2149::with_clocks(2_000_000, sample_rate),
            selected_register: 0,
            cpc_bus_latch: 0,
            cpc_control: 0,
            sample_rate,
            cpc_clock_active: false,
            cpc_requires_firmware: false,
            #[cfg(feature = "trace-ports")]
            port_log: Vec::new(),
        }
    }

    /// Reset AY chip + memory to the ZX defaults.
    pub fn reset_layout(&mut self) {
        self.memory[..=0x00FF].fill(0xC9);
        self.memory[0x0100..=0x3FFF].fill(0xFF);
        self.memory[0x4000..].fill(0x00);
        self.memory[0x0038] = 0xFB;
        self.selected_register = 0;
        self.chip.reset();
        self.cpc_clock_active = false;
        self.cpc_requires_firmware = false;
    }

    /// Load block payload into memory (clamped to 64K).
    pub fn load_block(&mut self, block: &AyBlock) {
        let start = block.address as usize;
        let end = start
            .saturating_add(block.length as usize)
            .min(self.memory.len());
        let data_len = end - start;
        if data_len == 0 {
            return;
        }
        self.memory[start..end].copy_from_slice(&block.data[..data_len]);
    }

    /// Access the chip (immutable).
    pub fn chip(&self) -> &Ym2149 {
        &self.chip
    }

    /// Access the chip (mutable).
    pub fn chip_mut(&mut self) -> &mut Ym2149 {
        &mut self.chip
    }

    /// Whether CPC-style port accesses have been detected.
    pub fn is_cpc_mode(&self) -> bool {
        self.cpc_clock_active
    }

    /// Whether the current track requires unsupported CPC firmware.
    pub fn requires_cpc_firmware(&self) -> bool {
        self.cpc_requires_firmware
    }

    #[cfg(feature = "trace-ports")]
    pub fn take_port_log(&mut self) -> Vec<String> {
        std::mem::take(&mut self.port_log)
    }

    fn handle_cpc_control(&mut self) {
        let bdir = (self.cpc_control & 0x80) != 0;
        let bc1 = (self.cpc_control & 0x40) != 0;
        match (bc1, bdir) {
            (true, true) => {
                self.selected_register = self.cpc_bus_latch & 0x0F;
                #[cfg(feature = "trace-ports")]
                self.port_log
                    .push(format!("cpc latch {:02X}", self.selected_register));
            }
            (false, true) => {
                let reg = self.selected_register & 0x0F;
                #[cfg(feature = "trace-ports")]
                self.port_log
                    .push(format!("cpc write {:02X} {:02X}", reg, self.cpc_bus_latch));
                self.chip.write_register(reg, self.cpc_bus_latch);
            }
            _ => {}
        }
    }

    fn ensure_cpc_clock(&mut self) {
        if self.cpc_clock_active {
            return;
        }
        self.cpc_clock_active = true;
        let regs = self.chip.dump_registers();
        let mut chip = Ym2149::with_clocks(1_000_000, self.sample_rate);
        chip.load_registers(&regs);
        chip.write_register(7, 0);
        self.chip = chip;
    }
}

impl Machine for AyMachine {
    fn peek(&self, address: u16) -> u8 {
        self.memory[address as usize]
    }

    fn poke(&mut self, address: u16, value: u8) {
        self.memory[address as usize] = value;
    }

    fn port_in(&mut self, _address: u16) -> u8 {
        0xFF
    }

    fn port_out(&mut self, address: u16, value: u8) {
        let masked = address & ZX_PORT_MASK;
        if masked == ZX_REG_PORT {
            self.selected_register = value & 0x0F;
            return;
        }
        if masked == ZX_DATA_PORT {
            let reg = self.selected_register & 0x0F;
            self.chip.write_register(reg, value);
            return;
        }

        match address & CPC_DATA_BUS_MASK {
            CPC_PORT_A => {
                self.ensure_cpc_clock();
                self.cpc_requires_firmware = true;
                self.cpc_bus_latch = value;
                #[cfg(feature = "trace-ports")]
                self.port_log
                    .push(format!("port f4{:02x} {:02X}", address as u8, value));
            }
            CPC_PORT_C => {
                self.ensure_cpc_clock();
                self.cpc_requires_firmware = true;
                self.cpc_control = value;
                #[cfg(feature = "trace-ports")]
                self.port_log
                    .push(format!("port f6{:02x} {:02X}", address as u8, value));
                self.handle_cpc_control();
            }
            _ => {}
        }
    }
}
