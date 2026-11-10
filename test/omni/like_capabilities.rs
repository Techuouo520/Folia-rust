// The hearts and playlist writes each platform offers, as the original's capabilities and mutations declare them,
// against what this port implements.

use folia_omni::Omni;

#[test]
fn capabilities_match_the_ported_platforms() {
    let dir = tempfile::tempdir().unwrap();
    let omni = Omni::new(dir.path().to_path_buf(), "Folia-rust-test".to_owned()).unwrap();
    // provider, likes, playlistTrackMutations, createPlaylist
    // The original declares likes and playlist track mutations on all seven; this port has direct write routes
    // only for NetEase, QQ Music and Spotify, and YouTube Music has no signed-in session at all.
    let table = [
        ("netease", true, true, false),
        ("qq", true, true, true),
        ("spotify", true, true, true),
        ("kugou", false, false, false),
        ("bodian", false, false, false),
        ("qishui", false, false, false),
        ("ytm", false, false, false),
    ];
    for (provider, likes, tracks, create) in table {
        let capabilities = omni.capabilities(provider);
        assert_eq!(capabilities.likes, likes, "{provider} likes");
        assert_eq!(capabilities.playlist_track_mutations, tracks, "{provider} playlist tracks");
        assert_eq!(capabilities.create_playlist, create, "{provider} create playlist");
    }
}

#[test]
fn song_capability_helpers_follow_the_platform() {
    let dir = tempfile::tempdir().unwrap();
    let omni = Omni::new(dir.path().to_path_buf(), "Folia-rust-test".to_owned()).unwrap();
    let netease = folia_core::OnlineId { provider: "netease".to_owned(), id: "1".to_owned() };
    let kugou = folia_core::OnlineId { provider: "kugou".to_owned(), id: "HASH".to_owned() };
    assert!(omni.can_like(&netease));
    assert!(!omni.can_create_playlist(&netease));
    assert!(omni.can_add_to_playlist(&netease));
    assert!(!omni.can_like(&kugou));
    assert!(!omni.can_add_to_playlist(&kugou));
}
