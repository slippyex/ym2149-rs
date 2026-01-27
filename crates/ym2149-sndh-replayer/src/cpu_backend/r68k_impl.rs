//! r68k emulator backend implementation.
//!
//! # Safety Architecture
//!
//! This module uses thread-local storage to pass memory context to the r68k CPU emulator.
//! The r68k crate requires an `AddressBus` implementation that is stored inside the CPU
//! struct, but we need to access external memory that changes per-step. To solve this,
//! we use a `ProxyBus` that delegates to a thread-local `CpuMemoryDyn` pointer.
//!
//! ## Safety Invariants
//!
//! 1. **Single-threaded execution**: The `MEMORY_CONTEXT` is thread-local, so each thread
//!    has its own context. However, a single `R68kBackend` instance must not be used from
//!    multiple threads simultaneously (it is `!Send` and `!Sync` by design via the raw pointer).
//!
//! 2. **Scoped lifetime**: The memory pointer is only valid during a single `step()` call.
//!    It is set before `execute1()` and cleared immediately after. No references escape.
//!
//! 3. **No re-entrancy**: `step()` must not be called recursively. The context is cleared
//!    after each step, so nested calls would see `None` and panic.
//!
//! ## Why Unsafe?
//!
//! The r68k crate's `AddressBus` trait is designed for the bus to be owned by the CPU.
//! Since we need external memory that varies per-step, we use a raw pointer in thread-local
//! storage. The alternative would be to fork r68k or use a different 68k emulator.

use super::{Cpu68k, CpuMemory};
use r68k::cpu::{ConfiguredCore, ProcessingState};
use r68k::interrupts::AutoInterruptController;
use r68k::ram::AddressBus;
use std::cell::UnsafeCell;

// Thread-local storage for the current memory context.
// This allows the ProxyBus to access the CpuMemory during CPU execution.
//
// SAFETY: The pointer is only valid during a single `step()` call and is cleared
// immediately after. The UnsafeCell is needed because we modify the pointer from
// within the AddressBus methods which only have `&self`.
thread_local! {
    static MEMORY_CONTEXT: UnsafeCell<Option<*mut dyn CpuMemoryDyn>> = const { UnsafeCell::new(None) };
}

/// Type-erased CpuMemory trait for thread-local storage.
trait CpuMemoryDyn {
    fn get_byte(&mut self, addr: u32) -> u8;
    fn get_word(&mut self, addr: u32) -> u16;
    fn set_byte(&mut self, addr: u32, value: u8);
    fn set_word(&mut self, addr: u32, value: u16);
    fn reset_instruction(&mut self);
}

impl<M: CpuMemory> CpuMemoryDyn for M {
    fn get_byte(&mut self, addr: u32) -> u8 {
        CpuMemory::get_byte(self, addr)
    }
    fn get_word(&mut self, addr: u32) -> u16 {
        CpuMemory::get_word(self, addr)
    }
    fn set_byte(&mut self, addr: u32, value: u8) {
        CpuMemory::set_byte(self, addr, value);
    }
    fn set_word(&mut self, addr: u32, value: u16) {
        CpuMemory::set_word(self, addr, value);
    }
    fn reset_instruction(&mut self) {
        CpuMemory::reset_instruction(self);
    }
}

/// Proxy AddressBus that delegates to the thread-local memory context.
#[derive(Clone)]
pub struct ProxyBus;

impl ProxyBus {
    fn new() -> Self {
        ProxyBus
    }

    fn with_memory<R, F: FnOnce(&mut dyn CpuMemoryDyn) -> R>(&self, f: F) -> R {
        MEMORY_CONTEXT.with(|ctx| {
            // SAFETY: The pointer is valid because:
            // 1. It was set by `step()` before calling `execute1()`
            // 2. It points to the `memory` parameter which is borrowed for the duration of `step()`
            // 3. No other code can modify MEMORY_CONTEXT on this thread during execution
            // The expect() is a programming error if triggered (step() not called correctly).
            unsafe {
                let ptr = (*ctx.get()).expect("Memory context not set during CPU execution");
                f(&mut *ptr)
            }
        })
    }
}

impl AddressBus for ProxyBus {
    fn copy_from(&mut self, _other: &Self) {
        // No-op: state is external
    }

    fn read_byte(&self, _address_space: r68k::ram::AddressSpace, address: u32) -> u32 {
        self.with_memory(|mem| mem.get_byte(address) as u32)
    }

    fn read_word(&self, _address_space: r68k::ram::AddressSpace, address: u32) -> u32 {
        self.with_memory(|mem| mem.get_word(address) as u32)
    }

    fn read_long(&self, address_space: r68k::ram::AddressSpace, address: u32) -> u32 {
        let hi = self.read_word(address_space, address);
        let lo = self.read_word(address_space, address.wrapping_add(2));
        (hi << 16) | lo
    }

    fn write_byte(&mut self, _address_space: r68k::ram::AddressSpace, address: u32, value: u32) {
        MEMORY_CONTEXT.with(|ctx| {
            // SAFETY: See with_memory() - same invariants apply
            unsafe {
                let ptr = (*ctx.get()).expect("Memory context not set during CPU execution");
                (*ptr).set_byte(address, value as u8);
            }
        });
    }

    fn write_word(&mut self, _address_space: r68k::ram::AddressSpace, address: u32, value: u32) {
        MEMORY_CONTEXT.with(|ctx| {
            // SAFETY: See with_memory() - same invariants apply
            unsafe {
                let ptr = (*ctx.get()).expect("Memory context not set during CPU execution");
                (*ptr).set_word(address, value as u16);
            }
        });
    }

    fn write_long(&mut self, address_space: r68k::ram::AddressSpace, address: u32, value: u32) {
        self.write_word(address_space, address, value >> 16);
        self.write_word(address_space, address.wrapping_add(2), value & 0xFFFF);
    }

    fn reset_instruction(&mut self) {
        MEMORY_CONTEXT.with(|ctx| {
            // SAFETY: See with_memory() - same invariants apply.
            // We check for Some because reset_instruction may be called during error recovery.
            unsafe {
                if let Some(ptr) = *ctx.get() {
                    (*ptr).reset_instruction();
                }
            }
        });
    }
}

/// r68k CPU backend.
pub struct R68kBackend {
    cpu: ConfiguredCore<AutoInterruptController, ProxyBus>,
}

impl Cpu68k for R68kBackend {
    fn new() -> Self {
        // Create CPU with proxy bus - memory is set per-step via thread-local
        let int_ctrl = AutoInterruptController::new();
        let bus = ProxyBus::new();
        let mut cpu = ConfiguredCore::new_with(0, int_ctrl, bus);
        // Set to Normal state so it's ready to execute instructions
        // (new_with starts in Group0Exception state)
        cpu.processing_state = ProcessingState::Normal;
        // Atari ST bus timing: 4-cycle boundary alignment due to GLUE/MMU wait states
        // (r68k's Musashi tables provide base cycles, granularity models ST bus)
        cpu.set_cycle_granularity(4);
        Self { cpu }
    }

    fn step<M: CpuMemory>(&mut self, memory: &mut M) -> usize {
        // SAFETY: We create a raw pointer to `memory` which is borrowed mutably for
        // the duration of this function. The pointer is stored in thread-local storage
        // and is only accessed by the ProxyBus during `execute1()`. We clear it before
        // returning, ensuring the pointer never outlives the borrow.
        let ptr = memory as *mut M as *mut dyn CpuMemoryDyn;

        MEMORY_CONTEXT.with(|ctx| {
            // SAFETY: We have exclusive access to the thread-local cell.
            // Setting the pointer before execute1() ensures ProxyBus can access memory.
            unsafe { *ctx.get() = Some(ptr) };
        });

        // Execute one instruction - ProxyBus will access memory via MEMORY_CONTEXT
        let cycles = self.cpu.execute1();

        MEMORY_CONTEXT.with(|ctx| {
            // SAFETY: Clearing the pointer ensures it cannot be used after `memory` is dropped.
            // This is the critical step that maintains memory safety.
            unsafe { *ctx.get() = None };
        });

        cycles.0 as usize
    }

    fn is_stopped(&self) -> bool {
        matches!(
            self.cpu.processing_state,
            ProcessingState::Stopped | ProcessingState::Halted
        )
    }

    fn set_stopped(&mut self, stop: bool) {
        if stop {
            self.cpu.processing_state = ProcessingState::Stopped;
        } else {
            self.cpu.processing_state = ProcessingState::Normal;
        }
    }

    fn pc(&self) -> u32 {
        self.cpu.pc
    }

    fn set_pc(&mut self, pc: u32) {
        self.cpu.pc = pc;
        // Force prefetch by setting prefetch_addr to invalid value
        // (r68k needs prefetch_addr != pc&!3 to trigger a fetch)
        self.cpu.prefetch_addr = !0;
    }

    fn set_d(&mut self, n: usize, value: u32) {
        self.cpu.dar[n] = value;
    }

    fn a(&self, n: usize) -> u32 {
        self.cpu.dar[8 + n]
    }

    fn set_a(&mut self, n: usize, value: u32) {
        self.cpu.dar[8 + n] = value;
    }

    fn sr(&self) -> u16 {
        self.cpu.status_register()
    }
}
