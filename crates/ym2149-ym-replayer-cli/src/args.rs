//! Command-line argument parsing for the YM2149 replayer CLI.
//!
//! This module handles parsing and validation of CLI arguments including:
//! - File path specification
//! - Chip backend selection (currently only ym2149)
//! - Color filter settings
//! - Help text generation

use std::env;
use std::fmt;

/// Available chip emulation backends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChipChoice {
    /// YM2149 hardware emulation backend
    Ym2149,
    /// Tiny 250kHz-step backend inspired by AtariAudio
    TinyYm2149,
}

impl ChipChoice {
    /// Parse chip choice from string argument.
    pub fn from_str(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "ym2149" => Some(ChipChoice::Ym2149),
            "tiny" | "ym2149-tiny" => Some(ChipChoice::TinyYm2149),
            _ => None,
        }
    }

    /// Get string representation of chip choice.
    pub fn as_str(&self) -> &'static str {
        match self {
            ChipChoice::Ym2149 => "ym2149",
            ChipChoice::TinyYm2149 => "ym2149-tiny",
        }
    }
}

impl fmt::Display for ChipChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Parsed command-line arguments.
#[derive(Debug)]
pub struct CliArgs {
    /// YM file path to play
    pub file_path: Option<String>,
    /// Whether to enable/disable color filter (None = use default)
    pub color_filter_override: Option<bool>,
    /// Selected chip backend
    pub chip_choice: ChipChoice,
    /// Whether help was requested
    pub show_help: bool,
}

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            file_path: None,
            color_filter_override: None,
            chip_choice: ChipChoice::Ym2149,
            show_help: false,
        }
    }
}

impl CliArgs {
    /// Parse arguments from command line.
    pub fn parse() -> Self {
        let mut args = Self::default();
        let mut iter = env::args().skip(1);

        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--no-color-filter" => {
                    args.color_filter_override = Some(false);
                }
                "--help" | "-h" => {
                    args.show_help = true;
                }
                "--chip" => {
                    if let Some(value) = iter.next() {
                        if let Some(choice) = ChipChoice::from_str(&value) {
                            args.chip_choice = choice;
                        } else {
                            eprintln!("Unknown chip type: {}", value);
                            args.show_help = true;
                        }
                    } else {
                        eprintln!("--chip requires an argument (ym2149)");
                        args.show_help = true;
                    }
                }
                _ if arg.starts_with("--chip=") => {
                    let value = &arg[7..];
                    if let Some(choice) = ChipChoice::from_str(value) {
                        args.chip_choice = choice;
                    } else {
                        eprintln!("Unknown chip type: {}", value);
                        args.show_help = true;
                    }
                }
                _ if arg.starts_with('-') => {
                    eprintln!("Unknown flag: {}", arg);
                    args.show_help = true;
                }
                _ => {
                    args.file_path = Some(arg);
                }
            }
        }

        args
    }

    /// Print help text to stderr.
    pub fn print_help() {
        eprintln!(
            "Usage:\n  ym2149 [--no-color-filter] [--chip <mode>] <file.ym>\n\n\
             Flags:\n\
             \x20 --no-color-filter    Disable ST-style color filter globally (default enabled)\n\
             \x20 --chip <mode>        Select synthesis engine:\n\
             \x20                        - ym2149 (default)\n\
             \x20                        - ym2149-tiny | tiny (250kHz step, DC filter)\n\
             \x20 -h, --help           Show this help\n\n\
             Examples:\n\
             \x20 ym2149 examples/Scaven6.ym\n"
        );
    }
}
