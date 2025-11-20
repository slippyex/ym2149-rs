use std::path::PathBuf;
use ym2149_ay_replayer::AyPlayer;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from(std::env::args().nth(1).expect("usage: log_ports <file.ay>"));
    let data = std::fs::read(&path)?;
    let (mut player, meta) = AyPlayer::load_from_bytes(&data, 0)?;
    println!("Loaded: {}", meta.song_name);
    player.play()?;
    let mut buffer = vec![0.0f32; 10 * 882];
    player.generate_samples_into(&mut buffer);
    println!("Generated {} samples", buffer.len());
    #[cfg(feature = "trace-ports")]
    {
        let log = player.take_port_log();
        for entry in log.iter().take(256) {
            println!("{entry}");
        }
        println!("... total {} port events", log.len());
    }
    Ok(())
}
