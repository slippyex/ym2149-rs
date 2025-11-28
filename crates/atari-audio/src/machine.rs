//! Atari Machine emulation - uses m68000 crate
//!
//! Integrates M68000 CPU, YM2149, MFP68901, and STE DAC
//! Original design by Arnaud Carré aka Leonard/Oxygene (@leonard_coder)

use crate::mfp68901::Mfp68901;
use crate::ste_dac::SteDac;
use crate::ym2149::Ym2149c;
use m68000::cpu_details::Mc68000;
use m68000::{M68000, MemoryAccess};

const RAM_SIZE: usize = 4 * 1024 * 1024; // 4MB
const RTE_INSTRUCTION_ADDR: u32 = 0x500;
const RESET_INSTRUCTION_ADDR: u32 = 0x502;
const SNDH_UPLOAD_ADDR: u32 = 0x10002;
const GEMDOS_MALLOC_EMUL_BUFFER: u32 = (RAM_SIZE - 0x100000) as u32;

/// Timer interrupt vectors
const IVECTOR: [u32; 5] = [0x134, 0x120, 0x114, 0x110, 0x13c];

/// Exit codes for CPU execution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    None = 0,
    Reset = 2,
}

/// Memory bus and peripherals
pub struct MemoryBus {
    pub ram: Vec<u8>,
    pub ym2149: Ym2149c,
    pub mfp: Mfp68901,
    pub ste_dac: SteDac,
    pub exit_code: ExitCode,
    pub next_gemdos_malloc_addr: u32,
}

impl MemoryBus {
    pub fn new() -> Self {
        Self {
            ram: vec![0; RAM_SIZE],
            ym2149: Ym2149c::new(),
            mfp: Mfp68901::new(),
            ste_dac: SteDac::new(),
            exit_code: ExitCode::None,
            next_gemdos_malloc_addr: GEMDOS_MALLOC_EMUL_BUFFER,
        }
    }

    // Helper read/write functions for RAM
    pub fn read_16(&self, addr: u32) -> u16 {
        let addr = addr as usize;
        if addr + 1 < self.ram.len() {
            ((self.ram[addr] as u16) << 8) | (self.ram[addr + 1] as u16)
        } else {
            0xffff
        }
    }

    pub fn read_32(&self, addr: u32) -> u32 {
        let addr = addr as usize;
        if addr + 3 < self.ram.len() {
            ((self.ram[addr] as u32) << 24)
                | ((self.ram[addr + 1] as u32) << 16)
                | ((self.ram[addr + 2] as u32) << 8)
                | (self.ram[addr + 3] as u32)
        } else {
            0xffffffff
        }
    }

    pub fn write_16(&mut self, addr: u32, value: u16) {
        let addr = addr as usize;
        if addr + 1 < self.ram.len() {
            self.ram[addr] = (value >> 8) as u8;
            self.ram[addr + 1] = value as u8;
        }
    }

    pub fn write_32(&mut self, addr: u32, value: u32) {
        let addr = addr as usize;
        if addr + 3 < self.ram.len() {
            self.ram[addr] = (value >> 24) as u8;
            self.ram[addr + 1] = (value >> 16) as u8;
            self.ram[addr + 2] = (value >> 8) as u8;
            self.ram[addr + 3] = value as u8;
        }
    }
}

impl Default for MemoryBus {
    fn default() -> Self {
        Self::new()
    }
}

/// Memory access implementation for m68000 crate
impl MemoryAccess for MemoryBus {
    fn get_byte(&mut self, address: u32) -> Option<u8> {
        let address = address & 0x00ffffff;

        if (address as usize) < RAM_SIZE {
            return Some(self.ram[address as usize]);
        }

        // YM2149 at $FF8800-$FF88FF
        if address >= 0xff8800 && address < 0xff8900 {
            return Some(self.ym2149.read_port((address & 0xff) as u8));
        }

        // Video mode at $FF8260
        if address == 0xff8260 {
            return Some(0); // Low res
        }

        // Sync mode at $FF820A
        if address == 0xff820a {
            return Some(2); // PAL 50Hz
        }

        // MFP at $FFFA00-$FFFA25
        if address >= 0xfffa00 && address < 0xfffa26 {
            return Some(self.mfp.read8((address - 0xfffa00) as usize));
        }

        // STE DAC at $FF8900-$FF8925
        if address >= 0xff8900 && address < 0xff8926 {
            return Some(self.ste_dac.read8((address - 0xff8900) as usize));
        }

        Some(0xff)
    }

    fn get_word(&mut self, address: u32) -> Option<u16> {
        let address = address & 0x00ffffff;

        if (address as usize) < RAM_SIZE - 1 {
            return Some(
                ((self.ram[address as usize] as u16) << 8)
                    | (self.ram[(address + 1) as usize] as u16),
            );
        }

        // YM2149
        if address >= 0xff8800 && address < 0xff8900 {
            return Some((self.ym2149.read_port((address & 0xfe) as u8) as u16) << 8);
        }

        // MFP
        if address >= 0xfffa00 && address < 0xfffa26 {
            return Some(self.mfp.read16((address - 0xfffa00) as usize));
        }

        // STE DAC
        if address >= 0xff8900 && address < 0xff8926 {
            return Some(self.ste_dac.read16((address - 0xff8900) as usize));
        }

        Some(0xffff)
    }

    fn set_byte(&mut self, address: u32, value: u8) -> Option<()> {
        let address = address & 0x00ffffff;

        if (address as usize) < RAM_SIZE {
            self.ram[address as usize] = value;
            return Some(());
        }

        // YM2149
        if address >= 0xff8800 && address < 0xff8900 {
            self.ym2149.write_port((address & 0xfe) as u8, value);
            return Some(());
        }

        // MFP
        if address >= 0xfffa00 && address < 0xfffa26 {
            self.mfp.write8((address - 0xfffa00) as usize, value);
            return Some(());
        }

        // STE DAC
        if address >= 0xff8900 && address < 0xff8926 {
            self.ste_dac.write8((address - 0xff8900) as usize, value);
            return Some(());
        }

        Some(())
    }

    fn set_word(&mut self, address: u32, value: u16) -> Option<()> {
        let address = address & 0x00ffffff;

        if (address as usize) < RAM_SIZE - 1 {
            self.ram[address as usize] = (value >> 8) as u8;
            self.ram[(address + 1) as usize] = value as u8;
            return Some(());
        }

        // YM2149
        if address >= 0xff8800 && address < 0xff8900 {
            self.ym2149.write_port((address & 0xfe) as u8, (value >> 8) as u8);
            return Some(());
        }

        // MFP
        if address >= 0xfffa00 && address < 0xfffa26 {
            self.mfp.write16((address - 0xfffa00) as usize, value);
            return Some(());
        }

        // STE DAC
        if address >= 0xff8900 && address < 0xff8926 {
            self.ste_dac.write16((address - 0xff8900) as usize, value);
            return Some(());
        }

        Some(())
    }

    fn reset_instruction(&mut self) {
        self.exit_code = ExitCode::Reset;
    }
}

/// Atari ST machine emulator
pub struct AtariMachine {
    pub bus: MemoryBus,
    cpu: M68000<Mc68000>,
}

impl AtariMachine {
    pub fn new() -> Self {
        Self {
            bus: MemoryBus::new(),
            cpu: M68000::new(),
        }
    }

    /// Initialize the machine with a specific replay rate
    pub fn startup(&mut self, host_replay_rate: u32) {
        // Clear RAM
        self.bus.ram.fill(0);

        // Reset audio chips
        self.bus.ym2149.reset(host_replay_rate, 2_000_000);
        self.bus.mfp.reset(host_replay_rate);
        self.bus.ste_dac.reset(host_replay_rate);
        self.bus.next_gemdos_malloc_addr = GEMDOS_MALLOC_EMUL_BUFFER;

        // Reset CPU
        self.cpu = M68000::new();

        // Setup cookie jar for MaxyMizer player
        self.bus.write_32(0x900, u32::from_be_bytes(*b"_SND"));
        self.bus.write_32(0x904, 0x3); // soundchip+STE DMA
        self.bus.write_32(0x908, u32::from_be_bytes(*b"_MCH"));
        self.bus.write_32(0x90c, 0x00010000); // STE
        self.bus.write_32(0x910, 0); // end
        self.bus.write_32(0x5a0, 0x900); // cookie jar start

        // Setup special instruction handlers
        self.bus.write_16(RESET_INSTRUCTION_ADDR, 0x4e70); // RESET instruction
        self.bus.write_16(RTE_INSTRUCTION_ADDR, 0x4e73); // RTE instruction

        // Setup default Timer C handler to RTE
        self.bus.write_32(0x114, RTE_INSTRUCTION_ADDR);
    }

    /// Upload data to RAM
    pub fn upload(&mut self, data: &[u8], addr: u32) -> bool {
        let addr = addr as usize;
        if addr + data.len() > RAM_SIZE {
            return false;
        }
        if data.is_empty() {
            return false;
        }
        self.bus.ram[addr..addr + data.len()].copy_from_slice(data);
        true
    }

    /// Get the recommended SNDH upload address
    pub fn sndh_upload_addr() -> u32 {
        SNDH_UPLOAD_ADDR
    }

    /// Execute a JSR to an address with D0 parameter
    pub fn jsr(&mut self, addr: u32, d0: u32) -> bool {
        self.configure_return_by_rts();
        self.cpu.regs.d[0].0 = d0;
        self.jmp_binary(addr, 50 * 10) // 1 second timeout
    }

    /// Compute next audio sample
    pub fn compute_next_sample(&mut self) -> i16 {
        // Get YM2149 sample
        let mut level = self.bus.ym2149.compute_next_sample() as i32;

        // Get STE DAC sample
        let ste_level = self
            .bus
            .ste_dac
            .compute_next_sample(&self.bus.ram, &mut self.bus.mfp) as i32;
        level += ste_level;

        // Clamp to i16 range
        level = level.clamp(-32768, 32767);

        // Tick MFP timers
        for t in 0..5 {
            if self.bus.mfp.tick(t) {
                let pc = self.bus.read_32(IVECTOR[t]);
                self.configure_return_by_rte();
                self.bus.ym2149.inside_timer_irq(true);
                self.jmp_binary(pc, 1);
                self.bus.ym2149.inside_timer_irq(false);
            }
        }

        level as i16
    }

    /// Configure stack for RTS return
    fn configure_return_by_rts(&mut self) {
        let ram_top = RAM_SIZE as u32;
        self.bus.write_32(ram_top - 4, RESET_INSTRUCTION_ADDR);
        self.bus.write_32(0, ram_top - 4); // Stack pointer
    }

    /// Configure stack for RTE return
    fn configure_return_by_rte(&mut self) {
        let ram_top = RAM_SIZE as u32;
        self.bus.write_32(ram_top - 4, RESET_INSTRUCTION_ADDR);
        self.bus.write_16(ram_top - 6, 0x2300); // SR=2300
        self.bus.write_32(0, ram_top - 6); // Stack pointer
    }

    /// Jump to a binary and execute until reset or timeout
    fn jmp_binary(&mut self, pc: u32, timeout_50hz: i32) -> bool {
        self.bus.write_32(0x14, RTE_INSTRUCTION_ADDR); // DIV by zero exception
        self.bus.write_32(4, pc); // PC at next location

        // Set PC and SP
        self.cpu.regs.pc.0 = pc;
        self.cpu.regs.a_mut(7).0 = self.bus.read_32(0);
        self.cpu.stop = false;

        self.bus.exit_code = ExitCode::None;

        let cycles_per_frame = 512 * 313;
        let total_cycles = (timeout_50hz as usize) * cycles_per_frame;
        let mut executed = 0;

        while executed < total_cycles && self.bus.exit_code == ExitCode::None && !self.cpu.stop {
            let current_pc = self.cpu.regs.pc.0 & 0x00ffffff;

            // Check for TRAP instructions before execution
            let opcode = self.bus.read_16(current_pc);
            if opcode & 0xfff0 == 0x4e40 {
                let vector = (opcode & 0x000f) as u8;
                if self.handle_trap(vector) {
                    executed += 40;
                    continue;
                }
            }

            // Execute one instruction
            executed += self.cpu.interpreter(&mut self.bus);
        }

        self.bus.exit_code == ExitCode::Reset
    }

    /// Handle TRAP instruction
    fn handle_trap(&mut self, vector: u8) -> bool {
        let a7 = self.cpu.regs.a(7);
        let func = self.bus.read_16(a7);

        match vector {
            1 => {
                // GEMDOS
                self.gemdos(func as u32, a7);
                self.cpu.regs.pc.0 = self.cpu.regs.pc.0.wrapping_add(2);
                true
            }
            14 => {
                // XBIOS
                let old_pc = self.cpu.regs.pc.0;
                self.xbios(func as u32, a7);
                if self.cpu.regs.pc.0 == old_pc {
                    self.cpu.regs.pc.0 = self.cpu.regs.pc.0.wrapping_add(2);
                }
                true
            }
            _ => false,
        }
    }

    /// Handle GEMDOS trap
    fn gemdos(&mut self, func: u32, a7: u32) {
        match func {
            0x48 => {
                // MALLOC
                let size = self.bus.read_32(a7 + 2);
                self.cpu.regs.d[0].0 = self.bus.next_gemdos_malloc_addr;
                self.bus.next_gemdos_malloc_addr =
                    (self.bus.next_gemdos_malloc_addr + size + 1) & !1;
            }
            0x30 => {
                // System version
                self.cpu.regs.d[0].0 = 0x0000;
            }
            _ => {}
        }
    }

    /// Handle XBIOS trap
    fn xbios(&mut self, func: u32, a7: u32) {
        match func {
            31 => {
                // Xbtimer
                let timer = self.bus.read_16(a7 + 2) as usize;
                let ctrl_word = self.bus.read_16(a7 + 4) as u8;
                let data_word = self.bus.read_16(a7 + 6) as u8;
                let vector = self.bus.read_32(a7 + 8);

                if timer < 4 {
                    self.bus.write_32(IVECTOR[timer], vector);
                    match timer {
                        0 => {
                            self.xbios_timer_set(0x19, 0x1f, 0x07, 5, 0x00, ctrl_word, data_word)
                        }
                        1 => {
                            self.xbios_timer_set(0x1b, 0x21, 0x07, 0, 0x00, ctrl_word, data_word)
                        }
                        2 => self.xbios_timer_set(
                            0x1d,
                            0x23,
                            0x09,
                            5,
                            0x0f,
                            (ctrl_word & 0xf) << 4,
                            data_word,
                        ),
                        3 => self.xbios_timer_set(
                            0x1d,
                            0x25,
                            0x09,
                            4,
                            0xf0,
                            ctrl_word & 0xf,
                            data_word,
                        ),
                        _ => {}
                    }
                }
            }
            38 => {
                // Supexec - execute callback in supervisor mode
                let callback_addr = self.bus.read_32(a7 + 2);
                let pc = self.cpu.regs.pc.0;
                let new_a7 = a7 - 4;
                self.bus.write_32(new_a7, pc);
                self.cpu.regs.a_mut(7).0 = new_a7;
                self.cpu.regs.pc.0 = callback_addr;
            }
            _ => {}
        }
    }

    /// Helper for XBIOS timer setup
    fn xbios_timer_set(
        &mut self,
        ctrl_port: usize,
        data_port: usize,
        enable_port: usize,
        bit: u8,
        mask: u8,
        ctrl_value: u8,
        data_value: u8,
    ) {
        let back = self.bus.mfp.read8(ctrl_port) & mask;
        self.bus.mfp.write8(ctrl_port, back);
        self.bus.mfp.write8(data_port, data_value);
        self.bus.mfp.write8(ctrl_port, back | ctrl_value);

        // Enable timer
        let enable_val = self.bus.mfp.read8(enable_port) | (1 << bit);
        self.bus.mfp.write8(enable_port, enable_val);
        let mask_val = self.bus.mfp.read8(enable_port + 12) | (1 << bit);
        self.bus.mfp.write8(enable_port + 12, mask_val);
    }
}

impl Default for AtariMachine {
    fn default() -> Self {
        Self::new()
    }
}
