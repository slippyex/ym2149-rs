//! Unit tests for the AKS parser.

use super::*;

#[test]
fn test_parse_format_3_metadata() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<song xmlns:aks="https://www.julien-nevo.com/arkostracker/ArkosTrackerSong">
  <formatVersion>3.0</formatVersion>
  <title>Test Song</title>
  <author>Test Author</author>
  <composer>Test Composer</composer>
  <comment>Test Comment</comment>
</song>"#;

    let song = load_aks(xml.as_bytes()).unwrap();
    assert_eq!(song.metadata.title, "Test Song");
    assert_eq!(song.metadata.author, "Test Author");
    assert_eq!(song.metadata.composer, "Test Composer");
}

#[test]
fn test_parse_subsong_with_psg() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<song>
  <title>Test</title>
  <subsongs>
    <subsong>
      <title>Main</title>
      <initialSpeed>6</initialSpeed>
      <replayFrequencyHz>50</replayFrequencyHz>
      <psgs>
        <psg>
          <type>ym</type>
          <frequencyHz>2000000</frequencyHz>
          <referenceFrequencyHz>440</referenceFrequencyHz>
          <samplePlayerFrequencyHz>11025</samplePlayerFrequencyHz>
          <mixingOutput>ABC</mixingOutput>
        </psg>
      </psgs>
    </subsong>
  </subsongs>
</song>"#;

    let song = load_aks(xml.as_bytes()).unwrap();
    assert_eq!(song.subsongs.len(), 1);
    assert_eq!(song.subsongs[0].title, "Main");
    assert_eq!(song.subsongs[0].psgs.len(), 1);
    assert_eq!(song.subsongs[0].psgs[0].psg_frequency, 2_000_000);
}

#[cfg(feature = "extended-tests")]
#[test]
fn test_parse_patterns() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<song>
  <title>Test</title>
  <subsongs>
    <subsong>
      <title>Main</title>
      <positions>
        <position>
          <patternIndex>0</patternIndex>
          <height>32</height>
          <markerName>Start</markerName>
          <markerColor>4282400896</markerColor>
          <transpositions/>
        </position>
        <position>
          <patternIndex>1</patternIndex>
          <height>64</height>
          <markerName></markerName>
          <markerColor>4282400896</markerColor>
          <transpositions/>
        </position>
      </positions>
      <patterns>
        <pattern>
          <trackIndexes>
            <trackIndex>0</trackIndex>
          </trackIndexes>
          <trackIndexes>
            <trackIndex>1</trackIndex>
          </trackIndexes>
          <trackIndexes>
            <trackIndex>2</trackIndex>
          </trackIndexes>
          <speedTrackIndex>
            <trackIndex>0</trackIndex>
          </speedTrackIndex>
          <eventTrackIndex>
            <trackIndex>0</trackIndex>
          </eventTrackIndex>
          <colorArgb>4286611584</colorArgb>
        </pattern>
        <pattern>
          <trackIndexes>
            <trackIndex>3</trackIndex>
          </trackIndexes>
          <trackIndexes>
            <trackIndex>4</trackIndex>
          </trackIndexes>
          <trackIndexes>
            <trackIndex>5</trackIndex>
          </trackIndexes>
          <speedTrackIndex>
            <trackIndex>1</trackIndex>
          </speedTrackIndex>
          <eventTrackIndex>
            <trackIndex>1</trackIndex>
          </eventTrackIndex>
          <colorArgb>4286611584</colorArgb>
        </pattern>
      </patterns>
      <psgs>
        <psg>
          <type>ym</type>
        </psg>
      </psgs>
    </subsong>
  </subsongs>
</song>"#;

    let song = load_aks(xml.as_bytes()).unwrap();
    assert_eq!(song.subsongs.len(), 1);

    let subsong = &song.subsongs[0];

    // Check positions
    assert_eq!(subsong.positions.len(), 2);
    assert_eq!(subsong.positions[0].pattern_index, 0);
    assert_eq!(subsong.positions[0].height, 32);
    assert_eq!(subsong.positions[0].marker_name, "Start");
    assert_eq!(subsong.positions[1].pattern_index, 1);
    assert_eq!(subsong.positions[1].height, 64);

    // Check patterns
    assert_eq!(subsong.patterns.len(), 2);

    let pattern0 = &subsong.patterns[0];
    assert_eq!(pattern0.index, 0);
    assert_eq!(pattern0.track_indexes, vec![0, 1, 2]);
    assert_eq!(pattern0.speed_track_index, 0);
    assert_eq!(pattern0.event_track_index, 0);
    assert_eq!(pattern0.color_argb, 4286611584);

    let pattern1 = &subsong.patterns[1];
    assert_eq!(pattern1.index, 1);
    assert_eq!(pattern1.track_indexes, vec![3, 4, 5]);
    assert_eq!(pattern1.speed_track_index, 1);
    assert_eq!(pattern1.event_track_index, 1);
}

#[cfg(feature = "extended-tests")]
#[test]
fn test_load_real_aks_file() {
    // Load a real AKS file to ensure everything parses correctly
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/arkos/Doclands - Pong Cracktro (YM).aks");
    let data = std::fs::read(&path).expect("Doclands test AKS file missing");
    let song = load_aks(&data).expect("failed to parse Doclands AKS file");

    assert!(!song.subsongs.is_empty(), "expected subsongs in {:?}", path);
    let subsong = &song.subsongs[0];
    eprintln!(
        "doclands subsong debug: positions {} patterns {} tracks {}",
        subsong.positions.len(),
        subsong.patterns.len(),
        subsong.tracks.len()
    );
    assert!(!subsong.psgs.is_empty());
    assert!(!subsong.positions.is_empty());
    assert!(!subsong.patterns.is_empty());
}

#[cfg(feature = "extended-tests")]
#[test]
fn test_load_real_at2_file() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/arkos/Excellence in Art 2018 - Just add cream.aks");
    let data = std::fs::read(&path).expect("AT2 test AKS file missing");
    let song = load_aks(&data).expect("failed to parse AT2 AKS file");

    assert!(!song.subsongs.is_empty(), "expected subsongs in AT2 song");
    let subsong = &song.subsongs[0];
    assert!(!subsong.psgs.is_empty());
    assert!(!subsong.positions.is_empty());
    assert!(!subsong.patterns.is_empty());
    assert!(
        subsong.speed_tracks.contains_key(&0),
        "expected speed track 0 in AT2 song"
    );
}

#[cfg(feature = "extended-tests")]
#[test]
fn test_at2_patterns_cover_all_channels() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/arkos/Excellence in Art 2018 - Just add cream.aks");
    let data = std::fs::read(&path).expect("AT2 test AKS file missing");
    let song = load_aks(&data).expect("failed to parse AT2 AKS file");

    let subsong = &song.subsongs[0];
    let channel_count = subsong.psgs.len() * 3;
    assert!(channel_count > 0, "song should define at least one PSG");

    for pattern in &subsong.patterns {
        assert_eq!(
            pattern.track_indexes.len(),
            channel_count,
            "pattern {} should provide a track index per channel",
            pattern.index
        );
    }
}

#[cfg(feature = "extended-tests")]
#[test]
fn test_at2_track_effects_have_names() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/arkos/Excellence in Art 2018 - Just add cream.aks");
    let data = std::fs::read(&path).expect("AT2 test AKS file missing");
    let song = load_aks(&data).expect("failed to parse AT2 AKS file");

    let subsong = &song.subsongs[0];
    let track = subsong.tracks.get(&1).expect("missing track 1");
    let cell = track
        .cells
        .iter()
        .find(|c| c.index == 0)
        .expect("missing cell 0");
    println!(
        "track1 cell0 instrument_present={} instrument={} format instruments={}",
        cell.instrument_present,
        cell.instrument,
        song.instruments.len()
    );
    assert!(
        !cell.effects.is_empty(),
        "expected effects parsed for track 1 cell 0"
    );
    let effect = &cell.effects[0];
    assert_eq!(effect.name, "volume");
    assert_eq!(effect.logical_value, 0xF);

    let arp_track = subsong.tracks.get(&22).expect("missing track 22");
    let arp_cell = arp_track
        .cells
        .iter()
        .find(|c| c.index == 0)
        .expect("missing track 22 cell 0");
    if let Some(vol_effect) = arp_cell.effects.iter().find(|eff| eff.name == "volume") {
        println!("track22 volume effect {}", vol_effect.logical_value);
    }
    let arp_effect = arp_cell
        .effects
        .iter()
        .find(|eff| eff.name == "arpeggioTable")
        .expect("missing arpeggioTable effect");
    assert_eq!(arp_effect.logical_value, 2);
}
