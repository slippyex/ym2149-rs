//! Abstraction layer for 68000 CPU backend.
//!
//! This module provides a unified interface for the 68000 CPU emulator,
//! using the local `r68k` emulator for SNDH playback.

mod r68k_impl;

pub use r68k_impl::R68kBackend;

/// Default CPU backend type alias.
pub type DefaultCpu = R68kBackend;

/// Memory access trait for CPU backends.
///
/// This trait abstracts the memory interface used by the CPU during execution.
/// Implementations handle RAM access and memory-mapped I/O (YM2149, MFP, etc.).
pub trait CpuMemory {
    /// Read a byte from memory.
    fn get_byte(&mut self, addr: u32) -> u8;

    /// Read a 16-bit word from memory (big-endian).
    fn get_word(&mut self, addr: u32) -> u16;

    /// Write a byte to memory.
    fn set_byte(&mut self, addr: u32, value: u8);

    /// Write a 16-bit word to memory (big-endian).
    fn set_word(&mut self, addr: u32, value: u16);

    /// Called when the CPU executes a RESET instruction.
    fn reset_instruction(&mut self);
}

/// Unified 68000 CPU interface.
///
/// This trait provides a common API for 68000 emulator backends.
pub trait Cpu68k {
    /// Create a new CPU instance in reset state.
    fn new() -> Self;

    /// Execute a single instruction and return the number of cycles consumed.
    fn step<M: CpuMemory>(&mut self, memory: &mut M) -> usize;

    /// Check if the CPU is in stopped state (STOP instruction executed).
    fn is_stopped(&self) -> bool;

    /// Set the stopped state.
    fn set_stopped(&mut self, stop: bool);

    /// Get the program counter.
    fn pc(&self) -> u32;

    /// Set the program counter.
    fn set_pc(&mut self, pc: u32);

    /// Set a data register (D0-D7).
    fn set_d(&mut self, n: usize, value: u32);

    /// Get an address register (A0-A7).
    fn a(&self, n: usize) -> u32;

    /// Set an address register (A0-A7).
    fn set_a(&mut self, n: usize, value: u32);

    /// Get the status register.
    fn sr(&self) -> u16;

    /// Get the interrupt priority level (IPL) from SR bits 8-10.
    /// Range: 0-7 (0 = all interrupts enabled, 7 = only NMI)
    fn ipl(&self) -> u8;

    /// Set the interrupt priority level (IPL) in SR bits 8-10.
    /// Used when entering/exiting interrupt handlers.
    fn set_ipl(&mut self, level: u8);

    /// Get the total number of CPU cycles executed since reset.
    fn total_cycles(&self) -> u64;

    /// Add cycles to the total cycle count (for exception processing overhead).
    /// Used when emulating exception entry that bypasses normal instruction execution.
    fn add_cycles(&mut self, cycles: u64);
}
