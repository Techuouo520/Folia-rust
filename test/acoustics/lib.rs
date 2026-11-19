// Room calibration contracts and an opt-in execution check against the pinned native Steam Audio SDK.
#![cfg(test)]

#[test]
fn default_seat_calibration_preserves_identity_and_physical_gain() {
    for rate in [44100, 48000, 96000] {
        for gain in [0.25, 0.5, 1.0, 2.0] {
            let channels = [vec![gain], vec![0.0], vec![0.0], vec![gain]];
            let correction = folia_acoustics::loudness_gain(&channels, rate);
            assert!((correction * gain - 1.0).abs() < 1e-6);
        }
    }
    assert_eq!(folia_acoustics::loudness_gain(&std::array::from_fn(|_| vec![0.0]), 48000), 1.0);
}

#[test]
fn room_cache_relocation_releases_the_database_before_copying() {
    use folia_core::{AcousticWorld, RoomListener, RoomOutput, RoomRequest};
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let target = temp.path().join("target");
    let store = folia_store::Store::at(temp.path().join("config"));
    let engine =
        folia_acoustics::Engine::new(temp.path().join("missing-sdk"), source.join("room_sound.sqlite")).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
    let listener = RoomListener { position: [0.0; 3], forward: [0.0, 0.0, -1.0], up: [0.0, 1.0, 0.0] };
    let request = RoomRequest {
        world: std::sync::Arc::new(AcousticWorld { meshes: vec![], materials: vec![] }),
        speakers: [[0.0; 3]; 2],
        listener,
        output: RoomOutput::Speakers,
        sample_rate: 48000,
    };
    assert!(runtime.block_on(engine.render(request, listener)).is_err());
    assert!(source.join("room_sound.sqlite").is_file());
    assert!(folia_store::StorageSettings::migrate(&store, &source, Some(target.clone()), &source, |_, _| {}).is_err());
    runtime.block_on(engine.set_cache_path(source.join("room_sound.sqlite"))).unwrap();
    let moved =
        folia_store::StorageSettings::migrate(&store, &source, Some(target.clone()), &source, |_, _| {}).unwrap();
    assert!(!moved.cleanup_pending);
    assert!(target.join("room_sound.sqlite").is_file());
    assert!(!source.join("room_sound.sqlite").exists());
    runtime.block_on(engine.set_cache_path(target.join("room_sound.sqlite"))).unwrap();
}

#[test]
#[cfg(feature = "stress")]
#[ignore = "requires the pinned Steam Audio native library"]
fn native_room_returns_finite_stereo_reflections_for_both_outputs() {
    use folia_core::{AcousticMaterial, AcousticMesh, AcousticWorld, RoomListener, RoomOutput, RoomRequest};
    let path = std::env::var_os("FOLIA_STEAM_AUDIO_LIBRARY").expect("set the temporary SDK library path");
    let mut simulator = folia_acoustics::Simulator::open(std::path::Path::new(&path)).unwrap();
    let mesh = AcousticMesh {
        positions: vec![
            [-4.0, 0.0, -2.0],
            [4.0, 0.0, -2.0],
            [4.0, 4.0, -2.0],
            [-4.0, 4.0, -2.0],
            [-4.0, 0.0, 8.0],
            [4.0, 0.0, 8.0],
            [4.0, 4.0, 8.0],
            [-4.0, 4.0, 8.0],
        ],
        triangles: vec![
            [0, 1, 2],
            [0, 2, 3],
            [1, 5, 6],
            [1, 6, 2],
            [5, 4, 7],
            [5, 7, 6],
            [4, 0, 3],
            [4, 3, 7],
            [3, 2, 6],
            [3, 6, 7],
            [4, 5, 1],
            [4, 1, 0],
        ],
        triangle_materials: vec![0; 12],
    };
    let mut request = RoomRequest {
        world: std::sync::Arc::new(AcousticWorld {
            meshes: vec![mesh],
            materials: vec![AcousticMaterial {
                absorption: [0.25, 0.35, 0.45],
                scattering: 0.2,
                transmission: [0.0; 3],
            }],
        }),
        speakers: [[-1.0, 1.5, 0.0], [1.0, 1.5, 0.0]],
        listener: RoomListener { position: [0.0, 1.2, 4.0], forward: [0.0, 0.0, -1.0], up: [0.0, 1.0, 0.0] },
        output: RoomOutput::Speakers,
        sample_rate: 48000,
    };
    for output in [RoomOutput::Speakers, RoomOutput::Headphones] {
        request.output = output;
        let impulse = simulator.compute(&request).unwrap();
        assert_eq!(impulse.channels.each_ref().map(Vec::len), [96000; 4]);
        assert!(impulse.channels.iter().flatten().all(|value| value.is_finite()));
        let direct: f64 =
            impulse.channels.iter().flat_map(|channel| &channel[..1024]).map(|value| f64::from(*value).powi(2)).sum();
        let tail: f64 =
            impulse.channels.iter().flat_map(|channel| &channel[1024..]).map(|value| f64::from(*value).powi(2)).sum();
        assert!(direct > 1e-6, "{output:?} direct {direct}");
        assert!(tail > 1e-8, "{output:?} tail {tail}");
        let gain = folia_acoustics::loudness_gain(&impulse.channels, 48000);
        assert!(gain.is_finite() && gain > 0.0);
        println!("{output:?} direct={direct} tail={tail} calibration={gain}");
    }
    request.listener.up = request.listener.forward;
    assert!(simulator.compute(&request).is_err());
    drop(simulator);
    request.listener.up = [0.0, 1.0, 0.0];
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("room.sqlite");
    let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
    let engine = folia_acoustics::Engine::new(path.into(), database.clone()).unwrap();
    let response = runtime.block_on(engine.render(request.clone(), request.listener)).unwrap();
    drop(engine);
    let cached = folia_acoustics::Engine::new(temp.path().join("missing-native-library"), database).unwrap();
    let restored = runtime.block_on(cached.render(request.clone(), request.listener)).unwrap();
    assert_eq!(restored.channels, response.channels, "hot cache must work without loading a native library");
    assert_eq!(restored.gain, response.gain);
}
