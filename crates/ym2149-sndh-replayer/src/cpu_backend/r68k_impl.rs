//! r68k emulator backend implementation.

use super::{Cpu68k, CpuMemory};
use r68k::cpu::{ConfiguredCore, ProcessingState};
use r68k::interrupts::AutoInterruptController;
use r68k::ram::AddressBus;
use std::cell::UnsafeCell;

// Thread-local storage for the current memory context.
// This allows the ProxyBus to access the CpuMemory during CPU execution.
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
        MEMORY_CONTEXT.with(|ctx| unsafe {
            let ptr = (*ctx.get()).expect("Memory context not set during CPU execution");
            f(&mut *ptr)
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
        MEMORY_CONTEXT.with(|ctx| unsafe {
            let ptr = (*ctx.get()).expect("Memory context not set during CPU execution");
            (*ptr).set_byte(address, value as u8);
        });
    }

    fn write_word(&mut self, _address_space: r68k::ram::AddressSpace, address: u32, value: u32) {
        MEMORY_CONTEXT.with(|ctx| unsafe {
            let ptr = (*ctx.get()).expect("Memory context not set during CPU execution");
            (*ptr).set_word(address, value as u16);
        });
    }

    fn write_long(&mut self, address_space: r68k::ram::AddressSpace, address: u32, value: u32) {
        self.write_word(address_space, address, value >> 16);
        self.write_word(address_space, address.wrapping_add(2), value & 0xFFFF);
    }

    fn reset_instruction(&mut self) {
        MEMORY_CONTEXT.with(|ctx| unsafe {
            if let Some(ptr) = *ctx.get() {
                (*ptr).reset_instruction();
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
        // Set memory context for this execution
        let ptr = memory as *mut M as *mut dyn CpuMemoryDyn;
        MEMORY_CONTEXT.with(|ctx| unsafe {
            *ctx.get() = Some(ptr);
        });

        // Execute one instruction
        let cycles = self.cpu.execute1();

        // Clear memory context
        MEMORY_CONTEXT.with(|ctx| unsafe {
            *ctx.get() = None;
        });

        cycles.0 as usize
    }

    fn is_stopped(&self) -> bool {
        matches!(self.cpu.processing_state, ProcessingState::Stopped | ProcessingState::Halted)
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
