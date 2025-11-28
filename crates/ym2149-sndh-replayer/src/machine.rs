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
use crate::ste_dac::SteDac;
use m68000::cpu_details::Mc68000;
use m68000::{M68000, MemoryAccess};
use std::env;
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
/// This matches the reference implementation's 50*10 = 500 frames timeout.
const INIT_TIMEOUT_FRAMES: u32 = 500;

/// Address of a tiny TRAP stub that returns a malloc-able pointer in D0.
const TRAP_STUB_ADDR: u32 = 0x600;

/// Interrupt vector addresses for MFP timers
const TIMER_VECTORS: [u32; 5] = [0x134, 0x120, 0x114, 0x110, 0x13C];

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
struct AtariMemory {
    /// RAM (4MB)
    ram: Vec<u8>,
    /// YM2149 sound chip
    ym2149: Ym2149,
    /// MFP 68901 timer chip
    mfp: Mfp68901,
    /// STE DAC (DMA audio)
    ste_dac: SteDac,
    /// YM2149 selected register
    ym_selected_reg: u8,
    /// Next GEMDOS malloc address
    next_malloc_addr: u32,
    /// Reset instruction executed flag
    reset_triggered: bool,
    /// Host sample rate
    host_rate: u32,
    /// Debug: last PC seen before a memory access (set by CPU driver)
    debug_pc: u32,
    /// Debug: last PC seen before YM write
    debug_pc_ym: u32,
}

impl AtariMemory {
    fn new(sample_rate: u32) -> Self {
        Self {
            ram: vec![0; RAM_SIZE],
            ym2149: Ym2149::with_clocks(2_000_000, sample_rate),
            mfp: Mfp68901::new(sample_rate),
            ste_dac: SteDac::new(sample_rate),
            ym_selected_reg: 0,
            next_malloc_addr: GEMDOS_MALLOC_START,
            reset_triggered: false,
            host_rate: sample_rate,
            debug_pc: 0,
            debug_pc_ym: 0,
        }
    }

    fn reset(&mut self) {
        self.ram.fill(0);
        self.ym2149.reset();
        self.mfp.reset();
        self.ste_dac.reset(self.host_rate);
        self.ym_selected_reg = 0;
        self.next_malloc_addr = GEMDOS_MALLOC_START;
        self.reset_triggered = false;
        self.debug_pc = 0;
        self.debug_pc_ym = 0;

        // Setup RTE and RESET instructions in low memory
        self.write_word(RTE_INSTRUCTION_ADDR, 0x4E73); // RTE
        self.write_word(RESET_INSTRUCTION_ADDR, 0x4E70); // RESET

        // Install a minimal trap stub that hands out memory instead of hanging on GEMDOS/XBIOS
        self.install_trap_stub(TRAP_STUB_ADDR);
        self.write_long((32 + 1) * 4, TRAP_STUB_ADDR); // TRAP #1  (GEMDOS)
        self.write_long((32 + 14) * 4, TRAP_STUB_ADDR); // TRAP #14 (XBIOS)

        // Default unhandled vectors to a safe RTE stub so OS/GEMDOS traps or
        // autovector interrupts don't jump into zeroed memory.
        for vector in 24..32 {
            // Auto-vectors 1-7
            self.write_long((vector * 4) as u32, RTE_INSTRUCTION_ADDR);
        }
        for vector in 32..48 {
            // TRAP #0-15
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

    fn install_trap_stub(&mut self, addr: u32) {
        // move.l #GEMDOS_MALLOC_START, d0
        self.write_word(addr, 0x203C);
        self.write_long(addr + 2, GEMDOS_MALLOC_START);
        // rte
        self.write_word(addr + 6, 0x4E73);
    }

    fn read_byte(&mut self, addr: u32) -> u8 {
        let addr = addr & 0x00FF_FFFF;

        if addr < RAM_SIZE as u32 {
            return self.ram[addr as usize];
        }

        // Treat ROM space (0xE00000-0xEFFFFF) as an endless RTS stub to satisfy
        // occasional TOS calls without full ROM emulation.
        if (0x00E0_0000..0x0100_0000).contains(&addr) {
            return if (addr & 1) == 0 { 0x4E } else { 0x75 };
        }

        // YM2149 PSG read
        if (0xFF8800..0xFF8900).contains(&addr) {
            if (addr & 2) == 0 {
                return self.ym2149.read_register(self.ym_selected_reg);
            }
            return 0xFF;
        }

        // STE DAC read
        if (0xFF8900..0xFF8926).contains(&addr) {
            if (addr & 1) == 0 {
                return (self.ste_dac.read16((addr - 0xFF8900) as u8) >> 8) as u8;
            }
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
            #[cfg(debug_assertions)]
            {
                if env::var_os("YM2149_VECTOR_DEBUG").is_some() && (0x100..=0x140).contains(&addr) {
                    eprintln!("VEC write byte ${:06X} = ${:02X}", addr, value);
                }
            }
            return;
        }

        // YM2149 PSG write
        if (0xFF8800..0xFF8900).contains(&addr) {
            #[cfg(debug_assertions)]
            {
                if env::var_os("YM2149_BYTE_DEBUG").is_some() {
                    eprintln!(
                        "YM byte write: addr=${:06X} value=${:02X} (pc=${:06X})",
                        addr,
                        value,
                        self.debug_pc_ym & 0x00FF_FFFF
                    );
                }
            }
            if (addr & 2) == 0 {
                // Select register
                self.ym_selected_reg = value & 0x0F;
            } else {
                // Write to selected register
                #[cfg(debug_assertions)]
                {
                    if env::var_os("YM2149_REG_DEBUG").is_some() {
                        if self.ym_selected_reg == 13 {
                            eprintln!(
                                "YM R13=${:02X} (ENVELOPE SHAPE!) pc=${:06X}",
                                value,
                                self.debug_pc_ym & 0x00FF_FFFF
                            );
                        } else {
                            eprintln!(
                                "YM R{:02}=${:02X} pc=${:06X}",
                                self.ym_selected_reg,
                                value,
                                self.debug_pc_ym & 0x00FF_FFFF
                            );
                        }
                    }
                }
                self.ym2149.write_register(self.ym_selected_reg, value);
            }
            return;
        }

        // STE DAC write
        if (0xFF8900..0xFF8926).contains(&addr) {
            if (addr & 1) == 0 {
                // word writes handled in write_word
                return;
            }
            self.ste_dac.write8((addr - 0xFF8900) as u8, value);
            return;
        }

        // MFP 68901
        if (0xFFFA00..0xFFFB00).contains(&addr) {
            #[cfg(debug_assertions)]
            {
                if env::var_os("YM2149_MFP_DEBUG").is_some()
                    || env::var_os("YM2149_MFP_TRACE").is_some()
                {
                    let port = addr - 0xFFFA00;
                    let port_name = match port {
                        0x07 => "IERA",
                        0x09 => "IERB",
                        0x13 => "IMRA",
                        0x15 => "IMRB",
                        0x19 => "TACR",
                        0x1B => "TBCR",
                        0x1D => "TCDCR",
                        0x1F => "TADR",
                        0x21 => "TBDR",
                        0x23 => "TCDR",
                        0x25 => "TDDR",
                        _ => "???",
                    };
                    eprintln!(
                        "MFP write ${:02X} ({}) = ${:02X} (pc=${:06X})",
                        port,
                        port_name,
                        value,
                        self.debug_pc & 0x00FF_FFFF
                    );
                }
            }
            self.mfp.write8((addr - 0xFFFA00) as u8, value);
        }
    }

    fn read_word(&mut self, addr: u32) -> u16 {
        let addr = addr & 0x00FF_FFFF;

        // MFP word read
        if (0xFFFA00..0xFFFA26).contains(&addr) {
            return self.mfp.read16((addr - 0xFFFA00) as u8);
        }

        // STE DAC word read - must be handled specially for microwire registers
        if (0xFF8900..0xFF8926).contains(&addr) && (addr & 1) == 0 {
            return self.ste_dac.read16((addr - 0xFF8900) as u8);
        }

        // Standard word read from two bytes
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
                let data = (value >> 8) as u8;
                #[cfg(debug_assertions)]
                {
                    if env::var_os("YM2149_REG_DEBUG").is_some() {
                        if self.ym_selected_reg == 13 {
                            eprintln!(
                                "YM R13=${:02X} (word) (ENVELOPE SHAPE!) pc=${:06X}",
                                data,
                                self.debug_pc_ym & 0x00FF_FFFF
                            );
                        } else {
                            eprintln!(
                                "YM R{:02}=${:02X} (word) pc=${:06X}",
                                self.ym_selected_reg,
                                data,
                                self.debug_pc_ym & 0x00FF_FFFF
                            );
                        }
                    }
                }
                self.ym2149.write_register(self.ym_selected_reg, data);
            }
            return;
        }

        // STE DAC word write
        if (0xFF8900..0xFF8926).contains(&addr) {
            self.ste_dac.write16((addr - 0xFF8900) as u8, value);
            return;
        }

        // MFP 68901 word write
        if (0xFFFA00..0xFFFB00).contains(&addr) {
            #[cfg(debug_assertions)]
            {
                if env::var_os("YM2149_MFP_DEBUG").is_some()
                    || env::var_os("YM2149_MFP_TRACE").is_some()
                {
                    eprintln!(
                        "MFP write16: ${:06X} = ${:04X} (pc=${:06X})",
                        addr,
                        value,
                        self.debug_pc & 0x00FF_FFFF
                    );
                }
            }
            self.mfp.write16((addr - 0xFFFA00) as u8, value);
            return;
        }

        // Standard word write to RAM
        if addr < (RAM_SIZE as u32) - 1 {
            self.ram[addr as usize] = (value >> 8) as u8;
            self.ram[(addr + 1) as usize] = value as u8;
            #[cfg(debug_assertions)]
            {
                if env::var_os("YM2149_VECTOR_DEBUG").is_some() && (0x100..=0x140).contains(&addr) {
                    eprintln!("VEC write word ${:06X} = ${:04X}", addr, value);
                }
            }
        }
    }

    fn read_long(&mut self, addr: u32) -> u32 {
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
    /// 68000 CPU (m68000 crate)
    cpu: M68000<Mc68000>,
    /// Memory and peripherals
    memory: AtariMemory,
    /// Prevent nested interrupt execution
    in_interrupt: bool,
    /// Incremental GEMDOS malloc pointer
    next_gemdos_malloc: u32,
    /// Host sample counter (for debug/timing traces)
    sample_counter: u64,
    /// Optional simulated VBL dispatcher (enabled via env)
    vbl_enabled: bool,
    /// Samples between VBL interrupts
    vbl_period_samples: u32,
    /// Sample counter for VBL
    vbl_counter: u32,
}

impl AtariMachine {
    /// Create a new Atari ST machine.
    pub fn new(sample_rate: u32) -> Self {
        let mut machine = Self {
            cpu: M68000::new(),
            memory: AtariMemory::new(sample_rate),
            in_interrupt: false,
            next_gemdos_malloc: GEMDOS_MALLOC_START,
            sample_counter: 0,
            vbl_enabled: std::env::var_os("YM2149_SIM_VBL").is_some(),
            vbl_period_samples: sample_rate / 50,
            vbl_counter: 0,
        };
        machine.reset();
        machine
    }

    /// Reset the machine to initial state.
    pub fn reset(&mut self) {
        self.memory.reset();
        self.cpu = M68000::new();
        self.in_interrupt = false;
        self.next_gemdos_malloc = GEMDOS_MALLOC_START;
        self.sample_counter = 0;
        self.vbl_counter = 0;

        // Force a Timer D event right after reset to align with drivers that expect
        // an immediate first tick (avoids silent first tens of milliseconds).
        self.memory.mfp.trigger_external_event(TimerId::TimerD);
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
        self.jmp_binary(addr, INIT_TIMEOUT_FRAMES)
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
            let pc = self.cpu.regs.pc.0;
            self.memory.debug_pc = pc;
            self.memory.debug_pc_ym = pc;
            executed += self.cpu.interpreter(&mut self.memory);
        }

        if executed >= max_cycles
            && !self.memory.reset_triggered
            && !self.cpu.stop
            && std::env::var_os("YM2149_PLAY_OVERRUN").is_some()
        {
            let pc = self.cpu.regs.pc.0 & 0x00FF_FFFF;
            eprintln!(
                "PLAY overrun: PC=0x{:06X}, executed {} cycles (limit {}), A7=0x{:08X}",
                pc,
                executed,
                max_cycles,
                self.cpu.regs.a(7)
            );
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
            #[cfg(debug_assertions)]
            {
                if env::var_os("YM2149_TIMER_DEBUG").is_some() {
                    let timer_name = match timer_id {
                        0 => "A",
                        1 => "B",
                        2 => "C",
                        3 => "D",
                        4 => "GPI7",
                        _ => "?",
                    };
                    let t = &self.memory.mfp.debug_timer(timer_id);
                    eprintln!(
                        "TIMER {} FIRED @sample {} (CR={:02X} DR={:02X} EN={} MASK={} FIRES={})",
                        timer_name,
                        self.sample_counter,
                        t.control,
                        t.data,
                        t.enable as u8,
                        t.mask as u8,
                        t.fire_count
                    );
                }
            }
            let vector_addr = TIMER_VECTORS[timer_id];
            let pc = self.memory.read_long(vector_addr);
            let pc24 = pc & 0x00FF_FFFF;

            if pc != 0 && pc != RTE_INSTRUCTION_ADDR && (pc24 as usize) < RAM_SIZE {
                if self.in_interrupt {
                    continue;
                }
                self.in_interrupt = true;
                self.configure_return_by_rte();
                // Signal YM2149 that we're inside a timer IRQ for square-sync buzzer effects
                self.memory.ym2149.set_inside_timer_irq(true);
                let _ = self.jmp_binary(pc, 1);
                self.memory.ym2149.set_inside_timer_irq(false);
                self.in_interrupt = false;
            }
        }
    }

    /// Advance time by a number of samples without calling the play routine.
    /// Useful to let timers/IRQs run between init and first play call.
    pub fn warmup_samples(&mut self, samples: u32) {
        for _ in 0..samples {
            // Advance YM clock (keeps noise/envelope in sync)
            self.memory.ym2149.clock();
            self.tick_timers();
            if self.vbl_enabled {
                self.tick_vbl();
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
        // Atari BIOS always enables the timer
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
                // Supervisor: return old SR in D0 (ignore USP switch)
                let sr_raw: u16 = self.cpu.regs.sr.into();
                self.cpu.regs.d[0].0 = sr_raw as u32;
            }
            0x48 => {
                // Malloc: read requested size (long at sp+2), return ptr in D0
                let size = self.memory.read_long(sp.wrapping_add(2));
                let addr = self.next_gemdos_malloc;
                self.next_gemdos_malloc =
                    self.next_gemdos_malloc.saturating_add(size + 1) & (!1u32);
                if (self.next_gemdos_malloc as usize) > RAM_SIZE {
                    self.cpu.regs.d[0].0 = 0; // fail
                } else {
                    self.cpu.regs.d[0].0 = addr;
                }
            }
            0x30 => {
                // System version
                self.cpu.regs.d[0].0 = 0x0000;
            }
            _ => {
                // Unknown -> return zero
                self.cpu.regs.d[0].0 = 0;
            }
        }
    }

    fn handle_xbios(&mut self, func: u16, sp: u32) {
        match func {
            31 => {
                // Set timer vector & control
                let timer = self.memory.read_word(sp.wrapping_add(2));
                let ctrl_word = self.memory.read_word(sp.wrapping_add(4));
                let data_word = self.memory.read_word(sp.wrapping_add(6));
                let raw_vector = self.memory.read_long(sp.wrapping_add(8));
                #[cfg(debug_assertions)]
                {
                    if env::var_os("YM2149_TIMER_DEBUG").is_some() {
                        let timer_name = match timer {
                            0 => "A",
                            1 => "B",
                            2 => "C",
                            3 => "D",
                            _ => "?",
                        };
                        eprintln!(
                            "XBIOS 31: Setup Timer {} ctrl=${:02X} data=${:02X} vector=${:06X}",
                            timer_name, ctrl_word, data_word, raw_vector
                        );
                    }
                }
                let vector = raw_vector & 0x00FF_FFFF;
                const IVECTOR: [u32; 4] = [0x134, 0x120, 0x114, 0x110];
                if (timer as usize) < IVECTOR.len() {
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
                // Supervisor callback: push PC and jump
                let callback = self.memory.read_long(sp.wrapping_add(2));
                let mut a7 = self.cpu.regs.a_mut(7).0;
                a7 = a7.wrapping_sub(4);
                // Return to next instruction after TRAP
                self.memory
                    .write_long(a7, self.cpu.regs.pc.0.wrapping_add(2));
                self.cpu.regs.a_mut(7).0 = a7;
                self.cpu.regs.pc.0 = callback & 0x00FF_FFFF;
            }
            _ => {
                // Ignore other XBIOS calls
            }
        }
    }

    fn handle_trap(&mut self, vector: u8) -> bool {
        // Match the reference implementation (modified Musashi): bypass exception frame entirely.
        // The callback is called INSTEAD of building a TRAP exception frame.
        // Stack remains unchanged; we just read function code from current SP and advance PC.
        //
        // Stack layout at TRAP: [func.W] [params...]
        // After handling: PC advances by 2 to skip TRAP instruction.
        let sp = self.cpu.regs.a(7);
        let func = self.memory.read_word(sp);

        #[cfg(debug_assertions)]
        {
            if env::var_os("YM2149_SNDH_DEBUG").is_some() {
                eprintln!(
                    "TRAP #{} func=0x{:04x} pc=0x{:06x} sp=0x{:08x}",
                    vector,
                    func,
                    self.cpu.regs.pc.0 & 0x00FF_FFFF,
                    sp
                );
            }
        }

        match vector {
            1 => {
                self.handle_gemdos(func, sp);
                // Skip past the TRAP instruction
                self.cpu.regs.pc.0 = self.cpu.regs.pc.0.wrapping_add(2);
                true
            }
            14 => {
                let old_pc = self.cpu.regs.pc.0;
                self.handle_xbios(func, sp);
                // Only advance PC if XBIOS didn't change it (e.g., supervisor callback changes PC)
                if self.cpu.regs.pc.0 == old_pc {
                    self.cpu.regs.pc.0 = self.cpu.regs.pc.0.wrapping_add(2);
                }
                true
            }
            _ => false,
        }
    }

    /// Compute the next audio sample.
    pub fn compute_sample(&mut self) -> i16 {
        self.memory.ym2149.clock();
        let mut level = (self.memory.ym2149.get_sample() * 32767.0) as i32;

        let ste_level = self
            .memory
            .ste_dac
            .compute_sample(&self.memory.ram, &mut self.memory.mfp) as i32;
        level += ste_level;

        self.sample_counter = self.sample_counter.wrapping_add(1);

        let out = level.clamp(-32768, 32767) as i16;

        // Tick timers after mixing (match C++ reference order)
        self.tick_timers();
        if std::env::var_os("YM2149_TIMERB_EVENT").is_some() {
            self.memory.mfp.trigger_external_event(TimerId::TimerB);
        }
        if self.vbl_enabled {
            self.tick_vbl();
        }

        out
    }

    /// Simulate VBL (IRQ4) at 50 Hz if enabled via env `YM2149_SIM_VBL`.
    fn tick_vbl(&mut self) {
        self.vbl_counter = self.vbl_counter.wrapping_add(1);
        if self.vbl_counter < self.vbl_period_samples {
            return;
        }
        self.vbl_counter = 0;

        // IRQ4 autovector lives at 0x70
        let vector_addr = 0x70u32;
        let pc = self.memory.read_long(vector_addr);
        let pc24 = pc & 0x00FF_FFFF;
        if pc == 0 || pc == RTE_INSTRUCTION_ADDR || (pc24 as usize) >= RAM_SIZE {
            return;
        }
        if self.in_interrupt {
            return;
        }
        self.in_interrupt = true;
        self.configure_return_by_rte();
        let _ = self.jmp_binary(pc, 1);
        self.in_interrupt = false;
    }

    /// Debug: disassemble a small slice of memory (word-wise) to find R10 writes.
    #[cfg(debug_assertions)]
    pub fn debug_dump_slice(&mut self, start: u32, words: usize) {
        if std::env::var_os("YM2149_DUMP_SLICE").is_none() {
            return;
        }
        eprintln!("Dump @${start:06X}:");
        for i in 0..words {
            let addr = start + (i as u32) * 2;
            let w = self.memory.read_word(addr);
            eprintln!(" ${addr:06X}: {:04X}", w);
        }
    }

    /// Get reference to YM2149.
    pub fn ym2149(&self) -> &Ym2149 {
        &self.memory.ym2149
    }

    /// Debug helper: dump vector table regions for timer/autovector analysis.
    pub fn dump_vectors(&mut self) {
        if env::var_os("YM2149_VECTOR_DUMP").is_none() {
            return;
        }
        let ranges = &[
            (0x100u32, 0x140u32),
            (0x200u32, 0x240u32),
            (0x240u32, 0x280u32),
        ];
        for (start, end) in ranges {
            eprintln!("Vector dump ${start:04X}-${end:04X}:");
            let mut addr = *start;
            while addr < *end {
                let word = self.memory.read_word(addr);
                eprintln!(" ${addr:04X}: {:04X}", word);
                addr += 2;
            }
        }
    }

    /// Debug helper: force Timer B config (for quick A/B tests).
    pub fn force_timer_b(&mut self, ctrl: u8, data: u8) {
        self.memory.mfp.write8(0x1B, ctrl); // TBCR
        self.memory.mfp.write8(0x21, data); // TBDR
        // Enable + mask timer B
        self.memory
            .mfp
            .write8(0x07, self.memory.mfp.read8(0x07) | 0x01);
        self.memory
            .mfp
            .write8(0x13, self.memory.mfp.read8(0x13) | 0x01);
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

        self.cpu.regs.pc.0 = pc;
        self.cpu.regs.a_mut(7).0 = self.memory.read_long(0);
        self.cpu.stop = false;
        self.memory.reset_triggered = false;

        let cycles_per_frame = 512 * 313;
        let total_cycles = (timeout_frames as usize) * cycles_per_frame;
        let mut executed = 0;

        // Execute instructions until we hit RESET or timeout.
        // Unlike the playback loop, we do NOT tick MFP timers or trigger VBL here.
        // This matches the reference implementation behavior where init only executes
        // CPU cycles without peripheral interrupts.
        while executed < total_cycles && !self.memory.reset_triggered && !self.cpu.stop {
            let pc = self.cpu.regs.pc.0;
            let pc24 = pc & 0x00FF_FFFF;
            self.memory.debug_pc = pc;
            self.memory.debug_pc_ym = pc;

            // Intercept TRAP #1/#14 before interpretation to simulate GEMDOS/XBIOS services.
            let opcode = self.memory.read_word(pc24);
            if opcode & 0xFFF0 == 0x4E40 {
                let vector = (opcode & 0x000F) as u8;
                if self.handle_trap(vector) {
                    // Account a small cost for the trap
                    executed += 40;
                    continue;
                }
            }

            // If code jumps into TOS ROM space, treat it as a stubbed RTS.
            if (pc24 >= 0x00E0_0000) && (pc24 as usize) < 0x0100_0000 {
                self.memory.reset_triggered = true;
                break;
            }

            let step_cycles = self.cpu.interpreter(&mut self.memory);
            executed += step_cycles;
        }

        if !self.memory.reset_triggered && !self.cpu.stop {
            if env::var_os("YM2149_SNDH_DEBUG").is_some() {
                let pc = self.cpu.regs.pc.0 & 0x00FF_FFFF;
                let opcode = self.memory.read_word(pc);
                eprintln!(
                    "SNDH init timeout: PC=0x{:06x}, opcode=0x{:04x}, executed {} cycles, limit {} frames",
                    pc, opcode, executed, timeout_frames
                );
                // Dump memory around PC
                eprintln!("Memory at PC:");
                for i in 0..8u32 {
                    eprint!("{:02x} ", self.memory.read_byte(pc + i));
                }
                eprintln!();
                // Dump some registers
                eprintln!(
                    "D0=0x{:08x} D1=0x{:08x} A0=0x{:08x} A7=0x{:08x}",
                    self.cpu.regs.d[0].0,
                    self.cpu.regs.d[1].0,
                    self.cpu.regs.a(0),
                    self.cpu.regs.a(7)
                );
            }
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
