use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;

use ym2149_ay_replayer::load_ay;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "../../ProjectAY".to_string()),
    );
    let mut total_songs = 0usize;
    let mut points_missing = 0usize;
    let mut init_zero = 0usize;
    let mut interrupt_zero = 0usize;

    let mut queue = VecDeque::new();
    queue.push_back(root);

    while let Some(dir) = queue.pop_front() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                queue.push_back(path);
                continue;
            }
            if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("ay"))
            {
                let data = fs::read(&path)?;
                match load_ay(&data) {
                    Ok(file) => {
                        for song in file.songs {
                            total_songs += 1;
                            if let Some(pts) = song.data.points.as_ref() {
                                if pts.init == 0 {
                                    init_zero += 1;
                                }
                                if pts.interrupt == 0 {
                                    interrupt_zero += 1;
                                }
                            } else {
                                points_missing += 1;
                            }
                        }
                    }
                    Err(err) => {
                        eprintln!("Failed to parse {}: {err}", path.display());
                    }
                }
            }
        }
    }

    println!(
        "Songs: {total_songs}, points missing: {points_missing}, init zero: {init_zero}, interrupt zero: {interrupt_zero}"
    );
    Ok(())
}
