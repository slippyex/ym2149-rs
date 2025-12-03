//! Shared snapshot of the YM2149 register state.
//!
//! This resource is populated by the playback diagnostics system so that
//! visualization crates can read the most recent register dump without
//! locking the player directly. It also carries the derived
//! [`ChannelStates`](ym2149::ChannelStates) for convenience.

use bevy::prelude::Resource;
use ym2149::ChannelStates;

/// Resource containing the latest YM2149 register dump and derived state.
#[derive(Resource, Debug, Clone, Default)]
pub struct ChipStateSnapshot {
    /// Raw 16-byte YM2149 register dump.
    pub registers: [u8; 16],
    /// Derived channel/envelope/noise state computed from the registers.
    pub channel_states: ChannelStates,
}

impl ChipStateSnapshot {
    /// Replace the stored registers and recompute the derived state.
    pub fn update_from_registers(&mut self, registers: [u8; 16]) {
        self.channel_states = ChannelStates::from_registers(&registers);
        self.registers = registers;
    }
}
