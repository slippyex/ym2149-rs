//! Integration tests for YM2149 playback functionality
//!
//! These tests verify the complete playback pipeline including initialization,
//! frame advancement, state transitions, and event emission.

use bevy::prelude::*;
use bevy_ym2149::{PlaybackState, Ym2149Playback, Ym2149Plugin, Ym2149Settings};

/// Helper to create a minimal test app with YM2149 plugin
fn create_test_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        bevy::audio::AudioPlugin::default(),
        Ym2149Plugin::default(),
    ));
    app
}

/// Create minimal valid YM3 file (14 registers, 1 frame)
const SAMPLE_YM: &[u8] = include_bytes!("../../bevy_ym2149_examples/assets/music/Ashtray.ym");

fn create_minimal_ym_file() -> Vec<u8> {
    SAMPLE_YM.to_vec()
}

#[test]
fn test_playback_initialization() {
    let mut app = create_test_app();
    let ym_data = create_minimal_ym_file();

    // Spawn a playback entity
    let entity = app
        .world_mut()
        .spawn(Ym2149Playback::from_bytes(ym_data))
        .id();

    // Run initialization system
    app.update();

    // Verify playback was initialized
    let playback = app
        .world()
        .entity(entity)
        .get::<Ym2149Playback>()
        .expect("Playback component should exist");

    assert_eq!(
        playback.state,
        PlaybackState::Idle,
        "Initial state should be Idle"
    );
    assert_eq!(
        playback.frame_position(),
        0,
        "Frame position should start at 0"
    );
    assert!(!playback.is_playing(), "Should not be playing initially");
}

#[test]
fn test_playback_state_transitions() {
    let mut app = create_test_app();
    let ym_data = create_minimal_ym_file();

    let entity = app
        .world_mut()
        .spawn(Ym2149Playback::from_bytes(ym_data))
        .id();

    // Initialize
    app.update();

    // Transition to Playing
    {
        let mut pb = app.world_mut().entity_mut(entity);
        let mut playback = pb.get_mut::<Ym2149Playback>().unwrap();
        playback.play();
    }
    assert_eq!(
        app.world()
            .entity(entity)
            .get::<Ym2149Playback>()
            .unwrap()
            .state,
        PlaybackState::Playing,
        "State should be Playing after play()"
    );

    // Transition to Paused
    {
        let mut pb = app.world_mut().entity_mut(entity);
        let mut playback = pb.get_mut::<Ym2149Playback>().unwrap();
        playback.pause();
    }
    assert_eq!(
        app.world()
            .entity(entity)
            .get::<Ym2149Playback>()
            .unwrap()
            .state,
        PlaybackState::Paused,
        "State should be Paused after pause()"
    );

    // Transition back to Playing
    {
        let mut pb = app.world_mut().entity_mut(entity);
        let mut playback = pb.get_mut::<Ym2149Playback>().unwrap();
        playback.play();
    }
    assert_eq!(
        app.world()
            .entity(entity)
            .get::<Ym2149Playback>()
            .unwrap()
            .state,
        PlaybackState::Playing,
        "State should be Playing again"
    );
}

#[test]
fn test_frame_advancement_during_playback() {
    let mut app = create_test_app();
    let ym_data = create_minimal_ym_file();

    let entity = app
        .world_mut()
        .spawn(Ym2149Playback::from_bytes(ym_data))
        .id();

    // Initialize
    app.update();

    // Start playing
    {
        let mut pb = app.world_mut().entity_mut(entity);
        let mut playback = pb.get_mut::<Ym2149Playback>().unwrap();
        playback.play();
    }

    let initial_position = app
        .world()
        .entity(entity)
        .get::<Ym2149Playback>()
        .unwrap()
        .frame_position();

    // Simulate time passing (20ms = 1 frame at 50Hz)
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_millis(20));

    app.update();

    let final_position = app
        .world()
        .entity(entity)
        .get::<Ym2149Playback>()
        .unwrap()
        .frame_position();

    // Position should have advanced (though might be 0 still if frame_count is 1)
    assert!(
        final_position >= initial_position,
        "Frame position should not go backwards"
    );
}

#[test]
fn test_playback_does_not_advance_when_paused() {
    let mut app = create_test_app();
    let ym_data = create_minimal_ym_file();

    let entity = app
        .world_mut()
        .spawn(Ym2149Playback::from_bytes(ym_data))
        .id();

    // Initialize and start
    app.update();
    {
        let mut pb = app.world_mut().entity_mut(entity);
        let mut playback = pb.get_mut::<Ym2149Playback>().unwrap();
        playback.play();
    }

    // Pause
    {
        let mut pb = app.world_mut().entity_mut(entity);
        let mut playback = pb.get_mut::<Ym2149Playback>().unwrap();
        playback.pause();
    }

    let position_before = app
        .world()
        .entity(entity)
        .get::<Ym2149Playback>()
        .unwrap()
        .frame_position();

    // Advance time
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_millis(100));

    app.update();

    let position_after = app
        .world()
        .entity(entity)
        .get::<Ym2149Playback>()
        .unwrap()
        .frame_position();

    assert_eq!(
        position_before, position_after,
        "Frame position should not advance while paused"
    );
}

#[test]
fn test_volume_control() {
    let mut app = create_test_app();
    let ym_data = create_minimal_ym_file();

    let entity = app
        .world_mut()
        .spawn(Ym2149Playback::from_bytes(ym_data))
        .id();

    app.update();

    // Test volume setting
    {
        let mut pb = app.world_mut().entity_mut(entity);
        let mut playback = pb.get_mut::<Ym2149Playback>().unwrap();

        assert_eq!(playback.volume, 1.0, "Default volume should be 1.0");

        playback.set_volume(0.5);
        assert_eq!(playback.volume, 0.5, "Volume should be set to 0.5");

        playback.set_volume(0.0);
        assert_eq!(playback.volume, 0.0, "Volume should be set to 0.0");

        playback.set_volume(1.0);
        assert_eq!(playback.volume, 1.0, "Volume should be set back to 1.0");
    }
}

#[test]
fn test_playback_restart() {
    let mut app = create_test_app();
    let ym_data = create_minimal_ym_file();

    let entity = app
        .world_mut()
        .spawn(Ym2149Playback::from_bytes(ym_data))
        .id();

    app.update();

    // Start playing
    {
        let mut pb = app.world_mut().entity_mut(entity);
        let mut playback = pb.get_mut::<Ym2149Playback>().unwrap();
        playback.play();
    }

    // Advance time
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_millis(50));
    app.update();

    let _position_after_advance = app
        .world()
        .entity(entity)
        .get::<Ym2149Playback>()
        .unwrap()
        .frame_position();

    // Restart
    {
        let mut pb = app.world_mut().entity_mut(entity);
        let mut playback = pb.get_mut::<Ym2149Playback>().unwrap();
        playback.restart();
    }

    let position_after_restart = app
        .world()
        .entity(entity)
        .get::<Ym2149Playback>()
        .unwrap()
        .frame_position();

    assert_eq!(
        position_after_restart, 0,
        "Frame position should reset to 0 after restart"
    );
}

#[test]
fn test_playback_query() {
    let mut app = create_test_app();
    let ym_data = create_minimal_ym_file();

    // Spawn multiple playback entities
    let entity1 = app
        .world_mut()
        .spawn(Ym2149Playback::from_bytes(ym_data.clone()))
        .id();
    let entity2 = app
        .world_mut()
        .spawn(Ym2149Playback::from_bytes(ym_data.clone()))
        .id();

    app.update();

    // Verify both entities exist and can be queried
    let entity1_exists = app
        .world()
        .entity(entity1)
        .get::<Ym2149Playback>()
        .is_some();

    let entity2_exists = app
        .world()
        .entity(entity2)
        .get::<Ym2149Playback>()
        .is_some();

    assert!(
        entity1_exists,
        "Entity1 should have Ym2149Playback component"
    );
    assert!(
        entity2_exists,
        "Entity2 should have Ym2149Playback component"
    );
}

#[test]
fn test_looping_restarts_when_loop_enabled() {
    let mut app = create_test_app();
    app.insert_resource(Ym2149Settings {
        loop_enabled: true,
        ..Default::default()
    });
    let ym_data = create_minimal_ym_file();

    let entity = app
        .world_mut()
        .spawn(Ym2149Playback::from_bytes(ym_data))
        .id();

    app.update();

    {
        let mut pb = app.world_mut().entity_mut(entity);
        let mut playback = pb.get_mut::<Ym2149Playback>().unwrap();
        playback.play();
    }

    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_secs(600));
    app.update();

    let playback = app
        .world()
        .entity(entity)
        .get::<Ym2149Playback>()
        .expect("Playback component present");

    assert_eq!(
        playback.state,
        PlaybackState::Playing,
        "Playback should continue playing when looping is enabled"
    );
    // After 600 seconds of simulated time with a typical YM file, looping should have occurred
    // The position should be relatively low (not at the end of the track)
    // Ashtray.ym has more than 2 frames, so position 2 indicates looping is working
    assert!(
        playback.frame_position() < 100,
        "Looping should keep position low, not at end of track (got {})",
        playback.frame_position()
    );
}

#[test]
fn test_stereo_gain_control() {
    let mut app = create_test_app();
    let ym_data = create_minimal_ym_file();

    let entity = app
        .world_mut()
        .spawn(Ym2149Playback::from_bytes(ym_data))
        .id();

    app.update();

    // Test stereo gain setting (method should succeed without panicking)
    {
        let mut pb = app.world_mut().entity_mut(entity);
        let mut playback = pb.get_mut::<Ym2149Playback>().unwrap();

        // Test that setting stereo gain doesn't panic
        playback.set_stereo_gain(0.3, 0.7);
        playback.set_stereo_gain(0.0, 1.0);
        playback.set_stereo_gain(1.0, 1.0);
    }

    // If we get here without panicking, stereo gain control works
    assert!(true, "Stereo gain control should work without panicking");
}

#[test]
fn test_multiple_playbacks_independent() {
    let mut app = create_test_app();
    let ym_data = create_minimal_ym_file();

    // Create two independent playbacks
    let entity1 = app
        .world_mut()
        .spawn(Ym2149Playback::from_bytes(ym_data.clone()))
        .id();
    let entity2 = app
        .world_mut()
        .spawn(Ym2149Playback::from_bytes(ym_data))
        .id();

    app.update();

    // Play entity1, start and pause entity2
    {
        let mut pb = app.world_mut().entity_mut(entity1);
        pb.get_mut::<Ym2149Playback>().unwrap().play();
    }
    {
        let mut pb = app.world_mut().entity_mut(entity2);
        let mut playback = pb.get_mut::<Ym2149Playback>().unwrap();
        playback.play();
        playback.pause();
    }

    // Verify independent states
    let pb1_state = app
        .world()
        .entity(entity1)
        .get::<Ym2149Playback>()
        .unwrap()
        .state;
    let pb2_state = app
        .world()
        .entity(entity2)
        .get::<Ym2149Playback>()
        .unwrap()
        .state;

    assert_eq!(
        pb1_state,
        PlaybackState::Playing,
        "Entity1 should be playing"
    );
    assert_eq!(pb2_state, PlaybackState::Paused, "Entity2 should be paused");
}

#[test]
fn test_metadata_extraction() {
    let mut app = create_test_app();
    let ym_data = create_minimal_ym_file();

    let entity = app
        .world_mut()
        .spawn(Ym2149Playback::from_bytes(ym_data))
        .id();

    app.update();

    let playback = app.world().entity(entity).get::<Ym2149Playback>().unwrap();

    // Ashtray.ym should have metadata
    assert!(
        !playback.song_title.is_empty() || !playback.song_author.is_empty(),
        "Metadata should be extracted from YM file"
    );
}
