#[cfg(not(feature = "streaming"))]
fn main() {
    eprintln!(
        "The ym2149 CLI requires the \"streaming\" feature. Rebuild with `--features streaming` to enable playback."
    );
}

#[cfg(feature = "streaming")]
mod cli {
    use std::env;
    use std::fmt;
    use std::fs;
    use std::io::{self, Read, Write};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Instant;

    use ym2149::replayer::{
        load_song, PlaybackController, Player, Ym6Info, Ym6Player, YmFileFormat,
    };
    #[cfg(feature = "softsynth")]
    use ym2149::softsynth::SoftPlayer;
    use ym2149::streaming::{
        StreamConfig, BUFFER_BACKOFF_MICROS, DEFAULT_SAMPLE_RATE, VISUALIZATION_UPDATE_MS,
    };
    use ym2149::visualization::{create_channel_status, create_volume_bar};
    use ym2149::{AudioDevice, RealtimePlayer};

    const PSG_MASTER_CLOCK_HZ: f32 = 2_000_000.0;
    const NOTE_NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];

    fn get_envelope_shape_name(shape_val: u8) -> &'static str {
        match shape_val & 0x0F {
            0x00..=0x03 => "AD",
            0x04 => "ADR",
            0x05 => "ASR",
            0x06 => "TRI",
            0x07 => "TRISUS",
            0x08 => "SAWDN",
            0x09 => "ASAWDN",
            0x0A => "SUSSAWDN",
            0x0B => "ASSAWDN",
            0x0C => "SAWUP",
            0x0D | 0x0F => "AH",
            0x0E => "SAWDN1x",
            _ => "",
        }
    }

    fn channel_period(lo: u8, hi: u8) -> Option<u16> {
        let period = (((hi as u16) & 0x0F) << 8) | (lo as u16);
        if period == 0 {
            None
        } else {
            Some(period)
        }
    }

    fn period_to_frequency(period: u16) -> f32 {
        PSG_MASTER_CLOCK_HZ / (16.0 * period as f32)
    }

    fn frequency_to_note_label(freq: f32) -> Option<String> {
        if !(freq.is_finite()) || freq <= 0.0 {
            return None;
        }
        let midi = 69.0 + 12.0 * (freq / 440.0).log2();
        let midi_rounded = midi.round();
        if !(0.0..=127.0).contains(&midi_rounded) {
            return None;
        }
        let midi_int = midi_rounded as i32;
        let note_index = ((midi_int % 12) + 12) % 12;
        let octave = (midi_int / 12) - 1;
        Some(format!("{}{}", NOTE_NAMES[note_index as usize], octave))
    }

    fn format_channel_highlight(
        period: Option<u16>,
        env_enabled: bool,
        sid_enabled: bool,
        drum_enabled: bool,
    ) -> String {
        match period {
            Some(period) => {
                let freq = period_to_frequency(period);
                let note = frequency_to_note_label(freq).unwrap_or_default();
                let mut parts = vec![format!("{freq:>7.1}Hz")];
                if !note.is_empty() {
                    parts.push(note);
                }
                if env_enabled {
                    parts.push("ENV".into());
                }
                if sid_enabled {
                    parts.push("SID".into());
                }
                if drum_enabled {
                    parts.push("DRUM".into());
                }
                parts.join(" ")
            }
            None => {
                let mut labels: Vec<String> = Vec::new();
                if env_enabled {
                    labels.push("ENV".into());
                }
                if sid_enabled {
                    labels.push("SID".into());
                }
                if drum_enabled {
                    labels.push("DRUM".into());
                }
                if labels.is_empty() {
                    "--".to_string()
                } else {
                    labels.join(" ")
                }
            }
        }
    }

    #[derive(Clone, Copy)]
    struct VisualSnapshot {
        registers: [u8; 16],
        sync_buzzer: bool,
        sid_active: [bool; 3],
        drum_active: [bool; 3],
    }

    trait RealtimeChip: PlaybackController + Send {
        fn generate_samples(&mut self, count: usize) -> Vec<f32>;
        fn visual_snapshot(&self) -> VisualSnapshot;
        fn set_color_filter(&mut self, enabled: bool);
        fn set_channel_mute(&mut self, channel: usize, mute: bool);
        fn is_channel_muted(&self, channel: usize) -> bool;
        fn get_playback_position(&self) -> f32;
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum ChipChoice {
        Ym2149,
        #[cfg(feature = "softsynth")]
        SoftSynth,
    }

    impl ChipChoice {
        fn from_str(value: &str) -> Option<Self> {
            match value.to_ascii_lowercase().as_str() {
                "ym2149" => Some(ChipChoice::Ym2149),
                #[cfg(feature = "softsynth")]
                "softsynth" => Some(ChipChoice::SoftSynth),
                _ => None,
            }
        }

        fn as_str(&self) -> &'static str {
            match self {
                ChipChoice::Ym2149 => "ym2149",
                #[cfg(feature = "softsynth")]
                ChipChoice::SoftSynth => "softsynth",
            }
        }
    }

    impl fmt::Display for ChipChoice {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(self.as_str())
        }
    }

    impl RealtimeChip for Ym6Player {
        fn generate_samples(&mut self, count: usize) -> Vec<f32> {
            Ym6Player::generate_samples(self, count)
        }

        fn visual_snapshot(&self) -> VisualSnapshot {
            let regs = self.get_chip().dump_registers();
            let (sync, sid, drum) = self.get_active_effects();
            VisualSnapshot {
                registers: regs,
                sync_buzzer: sync,
                sid_active: sid,
                drum_active: drum,
            }
        }

        fn set_color_filter(&mut self, enabled: bool) {
            self.get_chip_mut().set_color_filter(enabled);
        }

        fn set_channel_mute(&mut self, channel: usize, mute: bool) {
            self.set_channel_mute(channel, mute);
        }

        fn is_channel_muted(&self, channel: usize) -> bool {
            self.is_channel_muted(channel)
        }

        fn get_playback_position(&self) -> f32 {
            Ym6Player::get_playback_position(self)
        }
    }

    #[cfg(feature = "softsynth")]
    impl RealtimeChip for SoftPlayer {
        fn generate_samples(&mut self, count: usize) -> Vec<f32> {
            SoftPlayer::generate_samples(self, count)
        }

        fn visual_snapshot(&self) -> VisualSnapshot {
            let regs = self.visual_registers();
            let (sync, sid, drum) = self.get_active_effects();
            VisualSnapshot {
                registers: regs,
                sync_buzzer: sync,
                sid_active: sid,
                drum_active: drum,
            }
        }

        fn set_color_filter(&mut self, _enabled: bool) {}

        fn set_channel_mute(&mut self, channel: usize, mute: bool) {
            self.set_channel_mute(channel, mute);
        }

        fn is_channel_muted(&self, channel: usize) -> bool {
            self.is_channel_muted(channel)
        }

        fn get_playback_position(&self) -> f32 {
            SoftPlayer::get_playback_position(self)
        }
    }

    #[cfg(unix)]
    fn restore_terminal_mode() {
        let _ = std::process::Command::new("stty")
            .arg("echo")
            .arg("-raw")
            .status();
    }

    #[cfg(not(unix))]
    fn restore_terminal_mode() {}

    pub fn run() -> ym2149::Result<()> {
        println!("YM2149 PSG Emulator - Real-time Streaming Playback");
        println!("===================================================\n");

        let mut color_filter_override: Option<bool> = None;
        let mut file_arg: Option<String> = None;
        let mut show_help = false;
        let mut chip_choice = ChipChoice::Ym2149;

        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--no-color-filter" => {
                    color_filter_override = Some(false);
                }
                "--help" | "-h" => {
                    show_help = true;
                }
                "--chip" => {
                    if let Some(value) = args.next() {
                        if let Some(choice) = ChipChoice::from_str(&value) {
                            chip_choice = choice;
                        } else {
                            eprintln!("Unknown chip type: {}", value);
                            show_help = true;
                        }
                    } else {
                        eprintln!(
                            "--chip requires an argument (ym2149{})",
                            if cfg!(feature = "softsynth") {
                                "|softsynth"
                            } else {
                                ""
                            }
                        );
                        show_help = true;
                    }
                }
                _ if arg.starts_with("--chip=") => {
                    let value = &arg[7..];
                    if let Some(choice) = ChipChoice::from_str(value) {
                        chip_choice = choice;
                    } else {
                        eprintln!("Unknown chip type: {}", value);
                        show_help = true;
                    }
                }
                _ if arg.starts_with('-') => {
                    eprintln!("Unknown flag: {}", arg);
                    show_help = true;
                }
                _ => {
                    file_arg = Some(arg);
                }
            }
        }

        if show_help || file_arg.is_none() {
            eprintln!(
                "Usage:\n  ym2149 [--no-color-filter] [--chip <mode>] <file.ym>\n\nFlags:\n  --no-color-filter    Disable ST-style color filter globally (default enabled)\n  --chip <mode>        Select synthesis engine:\n                         - ym2149 (default){}\n  -h, --help           Show this help\n\nExamples:\n  ym2149 examples/Scaven6.ym{}\n",
                if cfg!(feature = "softsynth") {
                    "\n                         - softsynth (software synthesizer)"
                } else {
                    ""
                },
                if cfg!(feature = "softsynth") {
                    "\n  ym2149 --chip softsynth examples/Ashtray.ym"
                } else {
                    ""
                }
            );
            if file_arg.is_none() {
                return Ok(());
            }
        }

        let (player_box, total_samples, song_info) = match file_arg {
            Some(file_path) => {
                println!("Loading file: {}\n", file_path);
                let file_data = fs::read(&file_path)
                    .map_err(|e| format!("Failed to read file '{}': {}", file_path, e))?;
                let (mut ym_player, summary) = load_song(&file_data)?;
                println!("Detected format: {}\n", summary.format);

                match chip_choice {
                    ChipChoice::Ym2149 => {
                        if let Some(cf) = color_filter_override {
                            ym_player.get_chip_mut().set_color_filter(cf);
                        }
                        let info_str = format!(
                            "File: {} ({})\n{}",
                            file_path,
                            summary.format,
                            ym_player.format_info()
                        );
                        let total_samples = summary.total_samples();
                        (
                            Box::new(ym_player) as Box<dyn RealtimeChip>,
                            total_samples,
                            info_str,
                        )
                    }
                    #[cfg(feature = "softsynth")]
                    ChipChoice::SoftSynth => {
                        if matches!(summary.format, YmFileFormat::Ymt1 | YmFileFormat::Ymt2) {
                            return Err(
                                "Softsynth backend does not yet support tracker formats".into()
                            );
                        }
                        let mut soft_player = SoftPlayer::from_ym_player(&ym_player)?;
                        drop(ym_player);
                        if let Some(cf) = color_filter_override {
                            soft_player.set_color_filter(cf);
                        }
                        let info_str = format!(
                            "File: {} ({})\n{}",
                            file_path,
                            summary.format,
                            soft_player.format_info()
                        );
                        let duration_secs = soft_player.duration_seconds();
                        let total_samples = (duration_secs * DEFAULT_SAMPLE_RATE as f32) as usize;
                        (
                            Box::new(soft_player) as Box<dyn RealtimeChip>,
                            total_samples,
                            info_str,
                        )
                    }
                }
            }
            None => {
                println!("No YM file specified. Running in demo mode (5 seconds).");
                println!(
                    "Usage: {} <path/to/song.ym6>\n",
                    env::current_exe()
                        .ok()
                        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                        .unwrap_or_else(|| "ym2149".to_string())
                );

                match chip_choice {
                    ChipChoice::Ym2149 => {
                        let mut demo_player = Player::new();
                        let frames = vec![[0u8; 16]; 250];
                        demo_player.load_frames(frames);
                        let duration_secs = demo_player.get_duration_seconds();
                        let total_samples = (duration_secs * DEFAULT_SAMPLE_RATE as f32) as usize;
                        let info_str =
                            format!("Demo Mode: {:.2} seconds of silence", duration_secs);
                        (
                            Box::new(demo_player) as Box<dyn RealtimeChip>,
                            total_samples,
                            info_str,
                        )
                    }
                    #[cfg(feature = "softsynth")]
                    ChipChoice::SoftSynth => {
                        let mut demo_player = SoftPlayer::new();
                        let frames = vec![[0u8; 16]; 250];
                        let info = Ym6Info {
                            song_name: "Demo".into(),
                            author: "SoftSynth".into(),
                            comment: "Generated silence".into(),
                            frame_count: 250,
                            frame_rate: 50,
                            loop_frame: 0,
                            master_clock: 2_000_000,
                        };
                        demo_player.load_frames(frames, 50, None, info);
                        let duration_secs = demo_player.duration_seconds();
                        let total_samples = (duration_secs * DEFAULT_SAMPLE_RATE as f32) as usize;
                        let info_str = format!(
                            "Demo Mode (SoftSynth): {:.2} seconds of silence",
                            duration_secs
                        );
                        (
                            Box::new(demo_player) as Box<dyn RealtimeChip>,
                            total_samples,
                            info_str,
                        )
                    }
                }
            }
        };

        let player: Arc<parking_lot::Mutex<Box<dyn RealtimeChip>>> =
            Arc::new(parking_lot::Mutex::new(player_box));

        println!("File Information:");
        println!("{}\n", song_info);
        println!("Selected Chip: {}\n", chip_choice);

        let config = StreamConfig::low_latency(DEFAULT_SAMPLE_RATE);
        println!("Streaming Configuration:");
        println!("  Sample rate: {} Hz", config.sample_rate);
        println!(
            "  Buffer size: {} samples ({:.1}ms latency)",
            config.ring_buffer_size,
            config.latency_ms()
        );
        println!("  Total samples: {}\n", total_samples);

        let streamer = Arc::new(RealtimePlayer::new(config)?);
        let audio_device =
            AudioDevice::new(config.sample_rate, config.channels, streamer.get_buffer())?;

        println!("Audio device initialized - playing to speakers\n");

        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);
        let player_clone = Arc::clone(&player);
        let streamer_clone = Arc::clone(&streamer);

        let producer_thread = std::thread::spawn(move || {
            let mut sample_buffer = [0.0f32; 4096];
            {
                let mut player = player_clone.lock();
                if let Err(e) = player.play() {
                    eprintln!("Failed to start playback: {}", e);
                    return;
                }
            }

            while running_clone.load(Ordering::Relaxed) {
                let batch_size = sample_buffer.len();
                {
                    let mut player = player_clone.lock();
                    if player.state() == ym2149::replayer::PlaybackState::Stopped {
                        let _ = player.stop();
                        let _ = player.play();
                    }
                    let samples = player.generate_samples(batch_size);
                    for (i, &sample) in samples.iter().enumerate() {
                        sample_buffer[i] = sample;
                    }
                }

                let written = streamer_clone.write_blocking(&sample_buffer[..batch_size]);
                if written < batch_size {
                    std::thread::sleep(std::time::Duration::from_micros(BUFFER_BACKOFF_MICROS));
                }
            }
        });

        println!("Playback running — keys: [1/2/3]=mute A/B/C, [space]=pause/resume, [q]=quit\n");
        let playback_start = Instant::now();

        print!("\x1B[?25l");
        for _ in 0..4 {
            println!();
        }

        let (tx, rx) = std::sync::mpsc::channel::<u8>();
        let input_running = Arc::new(AtomicBool::new(true));
        let input_running_clone = Arc::clone(&input_running);
        std::thread::spawn(move || {
            #[cfg(unix)]
            let _ = std::process::Command::new("stty")
                .arg("-echo")
                .arg("raw")
                .status();
            let mut stdin = io::stdin();
            let mut buf = [0u8; 1];
            while input_running_clone.load(Ordering::Relaxed) {
                if stdin.read_exact(&mut buf).is_ok() {
                    let _ = tx.send(buf[0]);
                    if buf[0] == b'\x03' {
                        break;
                    }
                }
            }
            #[cfg(unix)]
            let _ = std::process::Command::new("stty")
                .arg("echo")
                .arg("-raw")
                .status();
        });

        loop {
            std::thread::sleep(std::time::Duration::from_millis(VISUALIZATION_UPDATE_MS));

            while let Ok(key) = rx.try_recv() {
                match key {
                    b'1' | b'2' | b'3' => {
                        let ch = (key - b'1') as usize;
                        let mut guard = player.lock();
                        let muted = guard.is_channel_muted(ch);
                        guard.set_channel_mute(ch, !muted);
                    }
                    b' ' => {
                        let mut guard = player.lock();
                        use ym2149::replayer::PlaybackState;
                        match guard.state() {
                            PlaybackState::Playing => {
                                let _ = guard.pause();
                            }
                            PlaybackState::Paused | PlaybackState::Stopped => {
                                let _ = guard.play();
                            }
                        }
                    }
                    b'q' | b'Q' => {
                        running.store(false, Ordering::Relaxed);
                    }
                    _ => {}
                }
            }

            let stats = streamer.get_stats();
            let elapsed = playback_start.elapsed().as_secs_f32();

            let snapshot = {
                let guard = player.lock();
                guard.visual_snapshot()
            };
            let regs = snapshot.registers;
            let mixer_r7 = regs[7];
            let envelope_shape_r15 = regs[15];

            let period_a = channel_period(regs[0], regs[1]);
            let period_b = channel_period(regs[2], regs[3]);
            let period_c = channel_period(regs[4], regs[5]);

            let tone_a = (mixer_r7 & 0x01) == 0;
            let tone_b = (mixer_r7 & 0x02) == 0;
            let tone_c = (mixer_r7 & 0x04) == 0;

            let noise_a = (mixer_r7 & 0x08) == 0;
            let noise_b = (mixer_r7 & 0x10) == 0;
            let noise_c = (mixer_r7 & 0x20) == 0;

            let amp_a = regs[8] & 0x0F;
            let amp_b = regs[9] & 0x0F;
            let amp_c = regs[10] & 0x0F;

            let env_a = (regs[8] & 0x10) != 0;
            let env_b = (regs[9] & 0x10) != 0;
            let env_c = (regs[10] & 0x10) != 0;

            let env_shape = get_envelope_shape_name(envelope_shape_r15);

            let bar_len = 10;
            let bar_a = create_volume_bar(amp_a as f32 / 15.0, bar_len);
            let bar_b = create_volume_bar(amp_b as f32 / 15.0, bar_len);
            let bar_c = create_volume_bar(amp_c as f32 / 15.0, bar_len);

            let sync_buzzer_active = snapshot.sync_buzzer;
            let sid_active = snapshot.sid_active;
            let drum_active = snapshot.drum_active;

            let highlight_a =
                format_channel_highlight(period_a, env_a, sid_active[0], drum_active[0]);
            let highlight_b =
                format_channel_highlight(period_b, env_b, sid_active[1], drum_active[1]);
            let highlight_c =
                format_channel_highlight(period_c, env_c, sid_active[2], drum_active[2]);

            let status_a = create_channel_status(
                tone_a,
                noise_a,
                amp_a,
                env_a,
                env_shape,
                sid_active[0],
                drum_active[0],
                sync_buzzer_active,
            );
            let status_b = create_channel_status(
                tone_b,
                noise_b,
                amp_b,
                env_b,
                env_shape,
                sid_active[1],
                drum_active[1],
                sync_buzzer_active,
            );
            let status_c = create_channel_status(
                tone_c,
                noise_c,
                amp_c,
                env_c,
                env_shape,
                sid_active[2],
                drum_active[2],
                sync_buzzer_active,
            );

            let (muted_a, muted_b, muted_c) = {
                let guard = player.lock();
                (
                    guard.is_channel_muted(0),
                    guard.is_channel_muted(1),
                    guard.is_channel_muted(2),
                )
            };

            let pos_pct = {
                let guard = player.lock();
                (guard.get_playback_position() * 100.0).clamp(0.0, 100.0)
            };

            print!("\x1B[4A");
            print!(
                "\x1B[2K\r[{:.1}s] Progress: {:>5.1}% | Buffer: {:.1}%b | Overruns: {}\n",
                elapsed,
                pos_pct,
                streamer.fill_percentage() * 100.0,
                stats.overrun_count,
            );
            print!(
                "\x1B[2K\rA{} {:<18} | B{} {:<18} | C{} {:<18}\n",
                if muted_a { "(M)" } else { "  " },
                bar_a,
                if muted_b { "(M)" } else { "  " },
                bar_b,
                if muted_c { "(M)" } else { "  " },
                bar_c,
            );
            print!(
                "\x1B[2K\r{:<22} | {:<22} | {:<22}\n",
                status_a, status_b, status_c
            );
            print!(
                "\x1B[2K\r{:<22} | {:<22} | {:<22}\n",
                highlight_a, highlight_b, highlight_c
            );
            io::stdout().flush().ok();

            if !running.load(Ordering::Relaxed) {
                break;
            }
        }

        restore_terminal_mode();
        println!("\x1B[?25h");
        io::stdout().flush().ok();

        running.store(false, Ordering::Relaxed);
        input_running.store(false, Ordering::Relaxed);
        producer_thread
            .join()
            .expect("Producer thread panicked during shutdown");

        audio_device.finish();

        let total_time = playback_start.elapsed();
        let final_stats = streamer.get_stats();

        println!("\n=== Playback Statistics ===");
        println!("Duration:          {:.2} seconds", total_time.as_secs_f32());
        println!("Samples played:    {}", final_stats.samples_played);
        println!("Overrun events:    {}", final_stats.overrun_count);
        println!("Buffer latency:    {:.1} ms", config.latency_ms());
        println!(
            "Memory used:       {} bytes (ring buffer)",
            config.ring_buffer_size * std::mem::size_of::<f32>()
        );
        println!("\nPlayback complete!");

        Ok(())
    }
}

#[cfg(feature = "streaming")]
fn main() -> ym2149::Result<()> {
    cli::run()
}
