//! Atari ST machine emulation for SNDH playback.
//!
//! This module provides a minimal Atari ST emulation environment suitable
//! for running SNDH music drivers. It includes:
//!
//! - 4MB RAM
//! - Motorola 68000 CPU (via m68000 crate)
//! - YM2149 sound chip (via ym2149 crate)
//! - MFP 68901 timers (for SID voice and effects)
//!
//! Memory map:
//! - 0x000000 - 0x3FFFFF: RAM (4MB)
//! - 0xFF8800 - 0xFF88FF: YM2149 PSG
//! - 0xFFFA00 - 0xFFFA25: MFP 68901

use crate::error::{Result, SndhError};
use crate::mfp68901::{Mfp68901, TimerId};
use m68000::cpu_details::Mc68000;
use m68000::{M68000, MemoryAccess};
use ym2149::Ym2149;

/// RAM size (4 MB)
const RAM_SIZE: usize = 4 * 1024 * 1024;

/// Address for RTE instruction (to return from interrupts)
const RTE_INSTRUCTION_ADDR: u32 = 0x500;

/// Address for RESET instruction (to detect routine completion)
const RESET_INSTRUCTION_ADDR: u32 = 0x502;

/// SNDH upload address (must be above 64KB for some drivers)
const SNDH_UPLOAD_ADDR: u32 = 0x10002;

/// GEMDOS malloc emulation buffer start
const GEMDOS_MALLOC_START: u32 = RAM_SIZE as u32 - 0x100000;

/// Interrupt vector addresses for MFP timers
const TIMER_VECTORS: [u32; 5] = [0x134, 0x120, 0x114, 0x110, 0x13C];

/// Memory and peripheral subsystem (separate from CPU to avoid borrow issues).
struct AtariMemory {
    /// RAM (4MB)
    ram: Vec<u8>,
    /// YM2149 sound chip
    ym2149: Ym2149,
    /// MFP 68901 timer chip
    mfp: Mfp68901,
    /// YM2149 selected register
    ym_selected_reg: u8,
    /// Next GEMDOS malloc address
    next_malloc_addr: u32,
    /// Reset instruction executed flag
    reset_triggered: bool,
}

impl AtariMemory {
    fn new(sample_rate: u32) -> Self {
        Self {
            ram: vec![0; RAM_SIZE],
            ym2149: Ym2149::with_clocks(2_000_000, sample_rate),
            mfp: Mfp68901::new(sample_rate),
            ym_selected_reg: 0,
            next_malloc_addr: GEMDOS_MALLOC_START,
            reset_triggered: false,
        }
    }

    fn reset(&mut self) {
        self.ram.fill(0);
        self.ym2149.reset();
        self.mfp.reset();
        self.ym_selected_reg = 0;
        self.next_malloc_addr = GEMDOS_MALLOC_START;
        self.reset_triggered = false;

        // Setup RTE and RESET instructions in low memory
        self.write_word(RTE_INSTRUCTION_ADDR, 0x4E73); // RTE
        self.write_word(RESET_INSTRUCTION_ADDR, 0x4E70); // RESET

        // Default timer C handler to RTE
        self.write_long(0x114, RTE_INSTRUCTION_ADDR);

        // Setup cookie jar for MaxyMizer player
        self.write_long(0x900, 0x5F534E44); // '_SND'
        self.write_long(0x904, 0x03);
        self.write_long(0x908, 0x5F4D4348); // '_MCH'
        self.write_long(0x90C, 0x00010000); // STE
        self.write_long(0x910, 0);
        self.write_long(0x5A0, 0x900);
    }

    fn read_byte(&self, addr: u32) -> u8 {
        let addr = addr & 0x00FF_FFFF;

        if addr < RAM_SIZE as u32 {
            return self.ram[addr as usize];
        }

        // YM2149 PSG read
        if (0xFF8800..0xFF8900).contains(&addr) {
            if (addr & 2) == 0 {
                return self.ym2149.read_register(self.ym_selected_reg);
            }
            return 0xFF;
        }

        // MFP 68901
        if (0xFFFA00..0xFFFA26).contains(&addr) {
            return self.mfp.read8((addr - 0xFFFA00) as u8);
        }

        // Video resolution (simulate low res)
        if addr == 0xFF8260 {
            return 0;
        }

        // Video sync (simulate PAL 50Hz)
        if addr == 0xFF820A {
            return 2;
        }

        0xFF
    }

    fn write_byte(&mut self, addr: u32, value: u8) {
        let addr = addr & 0x00FF_FFFF;

        if addr < RAM_SIZE as u32 {
            self.ram[addr as usize] = value;
            return;
        }

        // YM2149 PSG write
        if (0xFF8800..0xFF8900).contains(&addr) {
            if (addr & 2) == 0 {
                // Select register
                self.ym_selected_reg = value & 0x0F;
            } else {
                // Write to selected register
                self.ym2149.write_register(self.ym_selected_reg, value);
            }
            return;
        }

        // MFP 68901
        if (0xFFFA00..0xFFFA26).contains(&addr) {
            self.mfp.write8((addr - 0xFFFA00) as u8, value);
        }
    }

    fn read_word(&self, addr: u32) -> u16 {
        ((self.read_byte(addr) as u16) << 8) | (self.read_byte(addr + 1) as u16)
    }

    fn write_word(&mut self, addr: u32, value: u16) {
        let addr = addr & 0x00FF_FFFF;

        // YM2149 PSG word write - special handling
        // Atari ST uses word writes: high byte is the actual data
        // Port 0 (0xFF8800/01): Select register
        // Port 2 (0xFF8802/03): Write to selected register
        if (0xFF8800..0xFF8900).contains(&addr) {
            let port = addr & 0x02; // Check bit 1 to distinguish ports
            if port == 0 {
                // Port 0: Select register
                self.ym_selected_reg = ((value >> 8) as u8) & 0x0F;
            } else {
                // Port 2: Write to selected register (using high byte)
                self.ym2149.write_register(self.ym_selected_reg, (value >> 8) as u8);
            }
            return;
        }

        // MFP 68901 word write
        if (0xFFFA00..0xFFFA26).contains(&addr) {
            self.mfp.write16((addr - 0xFFFA00) as u8, value);
            return;
        }

        // Standard word write to RAM
        if addr < (RAM_SIZE as u32) - 1 {
            self.ram[addr as usize] = (value >> 8) as u8;
            self.ram[(addr + 1) as usize] = value as u8;
        }
    }

    fn read_long(&self, addr: u32) -> u32 {
        ((self.read_word(addr) as u32) << 16) | (self.read_word(addr + 2) as u32)
    }

    fn write_long(&mut self, addr: u32, value: u32) {
        self.write_word(addr, (value >> 16) as u16);
        self.write_word(addr + 2, value as u16);
    }
}

impl MemoryAccess for AtariMemory {
    fn get_byte(&mut self, addr: u32) -> Option<u8> {
        Some(self.read_byte(addr))
    }

    fn get_word(&mut self, addr: u32) -> Option<u16> {
        Some(self.read_word(addr))
    }

    fn set_byte(&mut self, addr: u32, value: u8) -> Option<()> {
        self.write_byte(addr, value);
        Some(())
    }

    fn set_word(&mut self, addr: u32, value: u16) -> Option<()> {
        self.write_word(addr, value);
        Some(())
    }

    fn reset_instruction(&mut self) {
        self.reset_triggered = true;
    }
}

/// Atari ST machine emulation for SNDH playback.
pub struct AtariMachine {
    /// 68000 CPU
    cpu: M68000<Mc68000>,
    /// Memory and peripherals
    memory: AtariMemory,
}

impl AtariMachine {
    /// Create a new Atari ST machine.
    pub fn new(sample_rate: u32) -> Self {
        let mut machine = Self {
            cpu: M68000::new(),
            memory: AtariMemory::new(sample_rate),
        };
        machine.reset();
        machine
    }

    /// Reset the machine to initial state.
    pub fn reset(&mut self) {
        self.memory.reset();
        self.cpu = M68000::new();
    }

    /// Upload data to RAM.
    pub fn upload(&mut self, data: &[u8], addr: u32) -> Result<()> {
        let addr = addr as usize;
        if addr + data.len() > RAM_SIZE {
            return Err(SndhError::MemoryError {
                address: addr as u32,
                msg: format!("Upload would exceed RAM size ({})", data.len()),
            });
        }
        self.memory.ram[addr..addr + data.len()].copy_from_slice(data);
        Ok(())
    }

    /// Get the SNDH upload address.
    pub fn sndh_upload_addr(&self) -> u32 {
        SNDH_UPLOAD_ADDR
    }

    /// Call a subroutine (JSR) with D0 parameter.
    pub fn jsr(&mut self, addr: u32, d0: u32) -> Result<bool> {
        // Configure stack for RTS return
        self.configure_return_by_rts();
        self.cpu.regs.d[0].0 = d0;
        self.jmp_binary(addr, 50 * 10)
    }

    /// Call a subroutine (JSR) with D0 parameter and limited cycles.
    /// Used for the play routine which should complete quickly.
    /// Uses single-instruction stepping to check for RESET after each instruction.
    pub fn jsr_limited(&mut self, addr: u32, d0: u32, max_cycles: usize) -> Result<bool> {
        self.configure_return_by_rts();
        self.cpu.regs.d[0].0 = d0;

        self.memory.write_long(0x14, RTE_INSTRUCTION_ADDR);
        self.memory.write_long(4, addr);

        self.cpu.regs.pc.0 = addr;
        self.cpu.regs.a_mut(7).0 = self.memory.read_long(0);
        self.cpu.stop = false;
        self.memory.reset_triggered = false;

        let mut executed = 0;
        // Execute one instruction at a time and check reset flag after each
        while executed < max_cycles && !self.memory.reset_triggered && !self.cpu.stop {
            executed += self.cpu.interpreter(&mut self.memory);
        }

        Ok(self.memory.reset_triggered)
    }

    /// Compute the next audio sample.
    pub fn compute_sample(&mut self) -> i16 {
        self.memory.ym2149.clock();
        let level = (self.memory.ym2149.get_sample() * 32767.0) as i32;

        // Tick all timers
        for timer_id in 0..5 {
            let timer = match timer_id {
                0 => TimerId::TimerA,
                1 => TimerId::TimerB,
                2 => TimerId::TimerC,
                3 => TimerId::TimerD,
                4 => TimerId::Gpi7,
                _ => continue,
            };

            if self.memory.mfp.tick(timer) {
                let vector_addr = TIMER_VECTORS[timer_id];
                let pc = self.memory.read_long(vector_addr);

                if pc != 0 && pc != RTE_INSTRUCTION_ADDR {
                    self.configure_return_by_rte();
                    let _ = self.jmp_binary(pc, 1);
                }
            }
        }

        level.clamp(-32768, 32767) as i16
    }

    /// Get mutable reference to YM2149.
    #[allow(dead_code)]
    pub fn ym2149_mut(&mut self) -> &mut Ym2149 {
        &mut self.memory.ym2149
    }

    /// Get reference to YM2149.
    pub fn ym2149(&self) -> &Ym2149 {
        &self.memory.ym2149
    }

    fn configure_return_by_rts(&mut self) {
        self.memory.write_long(RAM_SIZE as u32 - 4, RESET_INSTRUCTION_ADDR);
        self.memory.write_long(0, RAM_SIZE as u32 - 4);
    }

    fn configure_return_by_rte(&mut self) {
        self.memory.write_long(RAM_SIZE as u32 - 4, RESET_INSTRUCTION_ADDR);
        self.memory.write_word(RAM_SIZE as u32 - 6, 0x2300);
        self.memory.write_long(0, RAM_SIZE as u32 - 6);
    }

    fn jmp_binary(&mut self, pc: u32, timeout_frames: u32) -> Result<bool> {
        self.memory.write_long(0x14, RTE_INSTRUCTION_ADDR);
        self.memory.write_long(4, pc);

        self.cpu.regs.pc.0 = pc;
        self.cpu.regs.a_mut(7).0 = self.memory.read_long(0);
        self.cpu.stop = false;
        self.memory.reset_triggered = false;

        let cycles_per_frame = 512 * 313;
        let total_cycles = (timeout_frames as usize) * cycles_per_frame;
        let mut executed = 0;

        // Execute one instruction at a time for accurate RESET detection
        while executed < total_cycles && !self.memory.reset_triggered && !self.cpu.stop {
            executed += self.cpu.interpreter(&mut self.memory);
        }

        if !self.memory.reset_triggered && !self.cpu.stop {
            return Err(SndhError::InitTimeout {
                frames: timeout_frames,
            });
        }

        Ok(self.memory.reset_triggered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_machine_creation() {
        let machine = AtariMachine::new(44100);
        assert_eq!(machine.memory.ram.len(), RAM_SIZE);
    }

    #[test]
    fn test_upload() {
        let mut machine = AtariMachine::new(44100);
        let data = [1, 2, 3, 4];
        machine.upload(&data, 0x1000).unwrap();
        assert_eq!(machine.memory.ram[0x1000], 1);
        assert_eq!(machine.memory.ram[0x1003], 4);
    }

    #[test]
    fn test_memory_read_write() {
        let mut machine = AtariMachine::new(44100);
        machine.memory.write_long(0x100, 0x12345678);
        assert_eq!(machine.memory.read_long(0x100), 0x12345678);
        assert_eq!(machine.memory.read_word(0x100), 0x1234);
        assert_eq!(machine.memory.read_byte(0x100), 0x12);
    }
}
