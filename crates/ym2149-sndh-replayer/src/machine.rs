//! Atari ST machine emulation for SNDH playback.
//!
//! This module provides a minimal Atari ST emulation environment suitable
//! for running SNDH music drivers. It includes:
//!
//! - 4MB RAM
//! - Motorola 68000 CPU (configurable backend via features)
//! - YM2149 sound chip (via ym2149 crate)
//! - MFP 68901 timers (for SID voice and effects)
//!
//! Memory map:
//! - 0x000000 - 0x3FFFFF: RAM (4MB)
//! - 0xFF8800 - 0xFF88FF: YM2149 PSG
//! - 0xFFFA00 - 0xFFFA25: MFP 68901

use crate::cpu_backend::{Cpu68k, CpuMemory, DefaultCpu};
use crate::error::{Result, SndhError};
use crate::mfp68901::Mfp68901;
use crate::ste_dac::SteDac;
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

/// Maximum frames to wait for init/interrupt routines to return before bailing.
const INIT_TIMEOUT_FRAMES: u32 = 500;

/// Interrupt vector addresses for MFP timers
const IVECTOR: [u32; 5] = [0x134, 0x120, 0x114, 0x110, 0x13C];

struct XbiosTimerConfig {
    ctrl_port: u8,
    data_port: u8,
    enable_port: u8,
    bit: u8,
    mask: u8,
    ctrl_value: u8,
    data_value: u8,
}

/// Memory and peripheral subsystem (separate from CPU to avoid borrow issues).
pub(crate) struct AtariMemory {
    /// RAM (4MB)
    pub(crate) ram: Vec<u8>,
    /// YM2149 sound chip
    pub(crate) ym2149: Ym2149,
    /// MFP 68901 timer chip
    pub(crate) mfp: Mfp68901,
    /// STE DAC (DMA audio)
    pub(crate) ste_dac: SteDac,
    /// Next GEMDOS malloc address
    next_malloc_addr: u32,
    /// Reset instruction executed flag
    pub(crate) reset_triggered: bool,
    /// Host sample rate
    host_rate: u32,
}

impl AtariMemory {
    fn new(sample_rate: u32) -> Self {
        Self {
            ram: vec![0; RAM_SIZE],
            ym2149: Ym2149::with_clocks(2_000_000, sample_rate),
            mfp: Mfp68901::new(sample_rate),
            ste_dac: SteDac::new(sample_rate),
            next_malloc_addr: GEMDOS_MALLOC_START,
            reset_triggered: false,
            host_rate: sample_rate,
        }
    }

    fn reset(&mut self) {
        self.ram.fill(0);
        self.ym2149.reset();
        self.mfp.reset();
        self.ste_dac.reset(self.host_rate);
        self.next_malloc_addr = GEMDOS_MALLOC_START;
        self.reset_triggered = false;

        // Setup RTE and RESET instructions in low memory
        self.write_word(RTE_INSTRUCTION_ADDR, 0x4E73); // RTE
        self.write_word(RESET_INSTRUCTION_ADDR, 0x4E70); // RESET

        // Default unhandled vectors to a safe RTE stub
        for vector in 24..32 {
            self.write_long((vector * 4) as u32, RTE_INSTRUCTION_ADDR);
        }
        for vector in 32..48 {
            self.write_long((vector * 4) as u32, RTE_INSTRUCTION_ADDR);
        }
        // A-line / F-line illegal instructions
        self.write_long(0x28, RTE_INSTRUCTION_ADDR);
        self.write_long(0x2C, RTE_INSTRUCTION_ADDR);

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

    fn read_byte(&mut self, addr: u32) -> u8 {
        let addr = addr & 0x00FF_FFFF;

        if addr < RAM_SIZE as u32 {
            return self.ram[addr as usize];
        }

        // YM2149 PSG read
        if (0xFF8800..0xFF8900).contains(&addr) {
            return self.ym2149.read_port((addr & 0xff) as u8);
        }

        // STE DAC read
        if (0xFF8900..0xFF8926).contains(&addr) {
            return self.ste_dac.read8((addr - 0xFF8900) as u8);
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
            self.ym2149.write_port((addr & 0xfe) as u8, value);
            return;
        }

        // STE DAC write
        if (0xFF8900..0xFF8926).contains(&addr) {
            self.ste_dac.write8((addr - 0xFF8900) as u8, value);
            return;
        }

        // MFP 68901
        if (0xFFFA00..0xFFFB00).contains(&addr) {
            self.mfp.write8((addr - 0xFFFA00) as u8, value);
        }
    }

    pub(crate) fn read_word(&mut self, addr: u32) -> u16 {
        let addr = addr & 0x00FF_FFFF;

        // MFP word read
        if (0xFFFA00..0xFFFA26).contains(&addr) {
            return self.mfp.read16((addr - 0xFFFA00) as u8);
        }

        // STE DAC word read
        if (0xFF8900..0xFF8926).contains(&addr) && (addr & 1) == 0 {
            return self.ste_dac.read16((addr - 0xFF8900) as u8);
        }

        // YM2149
        if (0xFF8800..0xFF8900).contains(&addr) {
            return (self.ym2149.read_port((addr & 0xfe) as u8) as u16) << 8;
        }

        // Standard word read from two bytes
        ((self.read_byte(addr) as u16) << 8) | (self.read_byte(addr + 1) as u16)
    }

    pub(crate) fn write_word(&mut self, addr: u32, value: u16) {
        let addr = addr & 0x00FF_FFFF;

        // YM2149 PSG word write
        if (0xFF8800..0xFF8900).contains(&addr) {
            self.ym2149
                .write_port((addr & 0xfe) as u8, (value >> 8) as u8);
            return;
        }

        // STE DAC word write
        if (0xFF8900..0xFF8926).contains(&addr) {
            self.ste_dac.write16((addr - 0xFF8900) as u8, value);
            return;
        }

        // MFP 68901 word write
        if (0xFFFA00..0xFFFB00).contains(&addr) {
            self.mfp.write16((addr - 0xFFFA00) as u8, value);
            return;
        }

        // Standard word write to RAM
        if addr < (RAM_SIZE as u32) - 1 {
            self.ram[addr as usize] = (value >> 8) as u8;
            self.ram[(addr + 1) as usize] = value as u8;
        }
    }

    pub(crate) fn read_long(&mut self, addr: u32) -> u32 {
        ((self.read_word(addr) as u32) << 16) | (self.read_word(addr + 2) as u32)
    }

    pub(crate) fn write_long(&mut self, addr: u32, value: u32) {
        self.write_word(addr, (value >> 16) as u16);
        self.write_word(addr + 2, value as u16);
    }
}

// Implement our CpuMemory trait for AtariMemory
impl CpuMemory for AtariMemory {
    fn get_byte(&mut self, addr: u32) -> u8 {
        self.read_byte(addr)
    }

    fn get_word(&mut self, addr: u32) -> u16 {
        self.read_word(addr)
    }

    fn set_byte(&mut self, addr: u32, value: u8) {
        self.write_byte(addr, value);
    }

    fn set_word(&mut self, addr: u32, value: u16) {
        self.write_word(addr, value);
    }

    fn reset_instruction(&mut self) {
        self.reset_triggered = true;
    }
}

/// Atari ST machine emulation for SNDH playback.
pub struct AtariMachine {
    /// 68000 CPU (backend selected via features)
    cpu: DefaultCpu,
    /// Memory and peripherals
    memory: AtariMemory,
    /// Prevent nested interrupt execution
    in_interrupt: bool,
    /// Incremental GEMDOS malloc pointer
    next_gemdos_malloc: u32,
}

impl AtariMachine {
    /// Create a new Atari ST machine.
    pub fn new(sample_rate: u32) -> Self {
        let mut machine = Self {
            cpu: DefaultCpu::new(),
            memory: AtariMemory::new(sample_rate),
            in_interrupt: false,
            next_gemdos_malloc: GEMDOS_MALLOC_START,
        };
        machine.reset();
        machine
    }

    /// Reset the machine to initial state.
    pub fn reset(&mut self) {
        self.memory.reset();
        self.cpu = DefaultCpu::new();
        self.in_interrupt = false;
        self.next_gemdos_malloc = GEMDOS_MALLOC_START;
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
        self.configure_return_by_rts();
        self.cpu.set_d(0, d0);
        self.jmp_binary(addr, INIT_TIMEOUT_FRAMES)
    }

    /// Call a subroutine (JSR) with D0 parameter and limited cycles.
    pub fn jsr_limited(&mut self, addr: u32, d0: u32, max_cycles: usize) -> Result<bool> {
        self.configure_return_by_rts();
        self.cpu.set_d(0, d0);

        self.memory.write_long(0x14, RTE_INSTRUCTION_ADDR);
        self.memory.write_long(4, addr);

        self.cpu.set_pc(addr);
        self.cpu.set_a(7, self.memory.read_long(0));
        self.cpu.set_stopped(false);
        self.memory.reset_triggered = false;

        let mut executed = 0;
        while executed < max_cycles && !self.memory.reset_triggered && !self.cpu.is_stopped() {
            executed += self.cpu.step(&mut self.memory);
        }

        Ok(self.memory.reset_triggered)
    }

    /// Tick all MFP timers and dispatch their interrupts.
    fn tick_timers(&mut self) {
        let fired = self.memory.mfp.tick();
        for (timer_id, active) in fired.into_iter().enumerate() {
            if !active {
                continue;
            }
            let vector_addr = IVECTOR[timer_id];
            let pc = self.memory.read_long(vector_addr);
            let pc24 = pc & 0x00FF_FFFF;

            if pc != 0 && pc != RTE_INSTRUCTION_ADDR && (pc24 as usize) < RAM_SIZE {
                if self.in_interrupt {
                    continue;
                }
                self.in_interrupt = true;
                self.configure_return_by_rte();
                self.memory.ym2149.inside_timer_irq(true);
                let _ = self.jmp_binary(pc, 1);
                self.memory.ym2149.inside_timer_irq(false);
                self.in_interrupt = false;
            }
        }
    }

    fn xbios_timer_set(&mut self, config: XbiosTimerConfig) {
        let back = self.memory.mfp.read8(config.ctrl_port) & config.mask;
        self.memory.mfp.write8(config.ctrl_port, back);
        self.memory.mfp.write8(config.data_port, config.data_value);
        self.memory
            .mfp
            .write8(config.ctrl_port, back | config.ctrl_value);
        // Enable timer
        self.memory.mfp.write8(
            config.enable_port,
            self.memory.mfp.read8(config.enable_port) | (1 << config.bit),
        );
        self.memory.mfp.write8(
            config.enable_port + 12,
            self.memory.mfp.read8(config.enable_port + 12) | (1 << config.bit),
        );
    }

    fn handle_gemdos(&mut self, func: u16, sp: u32) {
        match func {
            0x01 => {
                // Supervisor
                let sr_raw: u16 = self.cpu.sr();
                self.cpu.set_d(0, sr_raw as u32);
            }
            0x48 => {
                // Malloc
                let size = self.memory.read_long(sp.wrapping_add(2));
                let addr = self.next_gemdos_malloc;
                self.next_gemdos_malloc =
                    self.next_gemdos_malloc.saturating_add(size + 1) & (!1u32);
                if (self.next_gemdos_malloc as usize) > RAM_SIZE {
                    self.cpu.set_d(0, 0);
                } else {
                    self.cpu.set_d(0, addr);
                }
            }
            0x30 => {
                // System version
                self.cpu.set_d(0, 0x0000);
            }
            _ => {
                self.cpu.set_d(0, 0);
            }
        }
    }

    fn handle_xbios(&mut self, func: u16, sp: u32) {
        match func {
            31 => {
                // Xbtimer
                let timer = self.memory.read_word(sp.wrapping_add(2));
                let ctrl_word = self.memory.read_word(sp.wrapping_add(4));
                let data_word = self.memory.read_word(sp.wrapping_add(6));
                let vector = self.memory.read_long(sp.wrapping_add(8)) & 0x00FF_FFFF;

                if (timer as usize) < IVECTOR.len() - 1 {
                    if (vector as usize) < RAM_SIZE {
                        self.memory.write_long(IVECTOR[timer as usize], vector);
                    } else {
                        self.memory
                            .write_long(IVECTOR[timer as usize], RTE_INSTRUCTION_ADDR);
                    }
                    match timer {
                        0 => self.xbios_timer_set(XbiosTimerConfig {
                            ctrl_port: 0x19,
                            data_port: 0x1F,
                            enable_port: 0x07,
                            bit: 5,
                            mask: 0x00,
                            ctrl_value: ctrl_word as u8,
                            data_value: data_word as u8,
                        }),
                        1 => self.xbios_timer_set(XbiosTimerConfig {
                            ctrl_port: 0x1B,
                            data_port: 0x21,
                            enable_port: 0x07,
                            bit: 0,
                            mask: 0x00,
                            ctrl_value: ctrl_word as u8,
                            data_value: data_word as u8,
                        }),
                        2 => self.xbios_timer_set(XbiosTimerConfig {
                            ctrl_port: 0x1D,
                            data_port: 0x23,
                            enable_port: 0x09,
                            bit: 5,
                            mask: 0x0F,
                            ctrl_value: ((ctrl_word & 0x0F) << 4) as u8,
                            data_value: data_word as u8,
                        }),
                        3 => self.xbios_timer_set(XbiosTimerConfig {
                            ctrl_port: 0x1D,
                            data_port: 0x25,
                            enable_port: 0x09,
                            bit: 4,
                            mask: 0xF0,
                            ctrl_value: (ctrl_word & 0x0F) as u8,
                            data_value: data_word as u8,
                        }),
                        _ => {}
                    }
                }
            }
            38 => {
                // Supexec
                let callback = self.memory.read_long(sp.wrapping_add(2));
                let mut a7 = self.cpu.a(7);
                a7 = a7.wrapping_sub(4);
                self.memory
                    .write_long(a7, self.cpu.pc().wrapping_add(2));
                self.cpu.set_a(7, a7);
                self.cpu.set_pc(callback & 0x00FF_FFFF);
            }
            _ => {}
        }
    }

    fn handle_trap(&mut self, vector: u8) -> bool {
        let sp = self.cpu.a(7);
        let func = self.memory.read_word(sp);

        match vector {
            1 => {
                self.handle_gemdos(func, sp);
                self.cpu.set_pc(self.cpu.pc().wrapping_add(2));
                true
            }
            14 => {
                let old_pc = self.cpu.pc();
                self.handle_xbios(func, sp);
                if self.cpu.pc() == old_pc {
                    self.cpu.set_pc(self.cpu.pc().wrapping_add(2));
                }
                true
            }
            _ => false,
        }
    }

    /// Compute the next audio sample.
    pub fn compute_sample(&mut self) -> i16 {
        let sample_i16 = self.memory.ym2149.compute_next_sample();
        let mut level = sample_i16 as i32;

        let ste_level = self
            .memory
            .ste_dac
            .compute_sample(&self.memory.ram, &mut self.memory.mfp) as i32;
        level += ste_level;

        // Tick timers after mixing
        self.tick_timers();

        level.clamp(-32768, 32767) as i16
    }

    /// Get reference to YM2149.
    pub fn ym2149(&self) -> &Ym2149 {
        &self.memory.ym2149
    }

    /// Get mutable reference to YM2149 (for channel muting).
    pub fn ym2149_mut(&mut self) -> &mut Ym2149 {
        &mut self.memory.ym2149
    }

    fn configure_return_by_rts(&mut self) {
        self.memory
            .write_long(RAM_SIZE as u32 - 4, RESET_INSTRUCTION_ADDR);
        self.memory.write_long(0, RAM_SIZE as u32 - 4);
    }

    fn configure_return_by_rte(&mut self) {
        self.memory
            .write_long(RAM_SIZE as u32 - 4, RESET_INSTRUCTION_ADDR);
        self.memory.write_word(RAM_SIZE as u32 - 6, 0x2300);
        self.memory.write_long(0, RAM_SIZE as u32 - 6);
    }

    fn jmp_binary(&mut self, pc: u32, timeout_frames: u32) -> Result<bool> {
        self.memory.write_long(0x14, RTE_INSTRUCTION_ADDR);
        self.memory.write_long(4, pc);

        self.cpu.set_pc(pc);
        self.cpu.set_a(7, self.memory.read_long(0));
        self.cpu.set_stopped(false);
        self.memory.reset_triggered = false;

        let cycles_per_frame = 512 * 313;
        let total_cycles = (timeout_frames as usize) * cycles_per_frame;
        let mut executed = 0;

        while executed < total_cycles && !self.memory.reset_triggered && !self.cpu.is_stopped() {
            let current_pc = self.cpu.pc();
            let pc24 = current_pc & 0x00FF_FFFF;

            // Intercept TRAP #1/#14
            let opcode = self.memory.read_word(pc24);
            if opcode & 0xFFF0 == 0x4E40 {
                let vector = (opcode & 0x000F) as u8;
                if self.handle_trap(vector) {
                    executed += 40;
                    continue;
                }
            }

            // If code jumps into TOS ROM space, treat it as a stubbed RTS.
            if (pc24 >= 0x00E0_0000) && (pc24 as usize) < 0x0100_0000 {
                self.memory.reset_triggered = true;
                break;
            }

            let step_cycles = self.cpu.step(&mut self.memory);
            executed += step_cycles;
        }

        if !self.memory.reset_triggered && !self.cpu.is_stopped() {
            return Err(SndhError::InitTimeout {
                frames: timeout_frames,
            });
        }

        Ok(self.memory.reset_triggered)
    }
}
