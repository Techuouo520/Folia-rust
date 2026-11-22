//! macOS Now Playing integration: Control Center, media keys and notch apps read this.
//!
//! Kept in its own crate because the workspace forbids `unsafe`, and the Objective-C
//! bindings here need it. Non-macOS targets compile as no-ops so workspace checks stay portable.

use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Play,
    Pause,
    TogglePlayPause,
    Next,
    Previous,
}

#[derive(Debug, Clone)]
pub struct Artwork {
    pub id: u64,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, Default)]
pub struct Info {
    pub title: String,
    pub artist: String,
    pub album: String,
    /// Seconds.
    pub duration: f64,
    /// Seconds.
    pub position: f64,
    pub playing: bool,
    /// Cover pixels, already decoded; keyed by `id` so it is only re-encoded on change.
    pub artwork: Option<Arc<Artwork>>,
}

/// Registers where remote commands go. Call once, before the first `publish`.
pub fn install_sink(sink: impl Fn(Command) + Send + Sync + 'static) {
    #[cfg(target_os = "macos")]
    imp::install_sink(sink);
    #[cfg(not(target_os = "macos"))]
    let _ = sink;
}

pub fn publish(info: &Info) {
    #[cfg(target_os = "macos")]
    imp::publish(info);
    #[cfg(not(target_os = "macos"))]
    let _ = info;
}

pub fn clear() {
    #[cfg(target_os = "macos")]
    imp::clear();
}

#[cfg(target_os = "macos")]
mod imp {
    use std::io::Cursor;
    use std::ptr::NonNull;
    use std::sync::{Mutex, Once, OnceLock};

    use block2::RcBlock;
    use objc2::AllocAnyThread;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, ProtocolObject};
    use objc2_app_kit::NSImage;
    use objc2_foundation::{NSData, NSMutableDictionary, NSNumber, NSString};
    use objc2_media_player::{
        MPMediaItemArtwork, MPNowPlayingInfoCenter, MPNowPlayingPlaybackState, MPRemoteCommand, MPRemoteCommandCenter,
        MPRemoteCommandEvent, MPRemoteCommandHandlerStatus,
    };

    use super::{Artwork, Command, Info};

    type Sink = Box<dyn Fn(Command) + Send + Sync>;

    static SINK: OnceLock<Sink> = OnceLock::new();
    /// Remote-command targets must outlive registration or macOS drops the handlers.
    struct Targets(Mutex<Vec<Retained<AnyObject>>>);
    unsafe impl Sync for Targets {}
    static TARGETS: Targets = Targets(Mutex::new(Vec::new()));
    static CLAIMED: Once = Once::new();
    /// The artwork object is only touched from the app's main thread.
    struct ArtCache(Mutex<Option<(u64, Retained<MPMediaItemArtwork>)>>);
    unsafe impl Sync for ArtCache {}
    static ARTWORK: ArtCache = ArtCache(Mutex::new(None));

    pub(super) fn install_sink(sink: impl Fn(Command) + Send + Sync + 'static) {
        let _ = SINK.set(Box::new(sink));
    }

    fn key(name: &str) -> Retained<NSString> {
        NSString::from_str(name)
    }

    fn set(dict: &NSMutableDictionary<NSString, AnyObject>, name: &str, value: &AnyObject) {
        let key = key(name);
        unsafe { dict.setObject_forKey(value, ProtocolObject::from_ref(&*key)) };
    }

    fn set_string(dict: &NSMutableDictionary<NSString, AnyObject>, name: &str, value: &str) {
        let value = NSString::from_str(value);
        set(dict, name, unsafe { &*(&*value as *const NSString).cast::<AnyObject>() });
    }

    fn set_number(dict: &NSMutableDictionary<NSString, AnyObject>, name: &str, value: f64) {
        let value = NSNumber::new_f64(value);
        set(dict, name, unsafe { &*(&*value as *const NSNumber).cast::<AnyObject>() });
    }

    /// Registering remote-command handlers is what makes macOS treat this process as the
    /// current Now Playing app; without it the info dictionary alone is not surfaced.
    fn claim() {
        CLAIMED.call_once(|| unsafe {
            let center = MPRemoteCommandCenter::sharedCommandCenter();
            let commands: [Retained<MPRemoteCommand>; 5] = [
                center.playCommand(),
                center.pauseCommand(),
                center.togglePlayPauseCommand(),
                center.nextTrackCommand(),
                center.previousTrackCommand(),
            ];
            let commands_sent =
                [Command::Play, Command::Pause, Command::TogglePlayPause, Command::Next, Command::Previous];
            for (command, sent) in commands.into_iter().zip(commands_sent) {
                let handler = RcBlock::new(move |_event: NonNull<MPRemoteCommandEvent>| {
                    if let Some(sink) = SINK.get() {
                        sink(sent);
                    }
                    MPRemoteCommandHandlerStatus::Success
                });
                let target = command.addTargetWithHandler(&handler);
                if let Ok(mut targets) = TARGETS.0.lock() {
                    targets.push(target);
                }
            }
        });
    }

    fn artwork(art: &Artwork) -> Option<Retained<MPMediaItemArtwork>> {
        let mut cache = match ARTWORK.0.lock() {
            Ok(cache) => cache,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some((cached, artwork)) = cache.as_ref()
            && *cached == art.id
        {
            return Some(artwork.clone());
        }
        let image = image::RgbaImage::from_raw(art.width, art.height, art.rgba.clone())?;
        let mut png = Cursor::new(Vec::new());
        image.write_to(&mut png, image::ImageFormat::Png).ok()?;
        unsafe {
            let data = NSData::with_bytes(&png.into_inner());
            let image = NSImage::initWithData(NSImage::alloc(), &data)?;
            let handler = RcBlock::new(move |_size: objc2_core_foundation::CGSize| {
                // The artwork keeps this block (and therefore the image) alive.
                NonNull::from(&*image)
            });
            let bounds = objc2_core_foundation::CGSize { width: f64::from(art.width), height: f64::from(art.height) };
            let artwork =
                MPMediaItemArtwork::initWithBoundsSize_requestHandler(MPMediaItemArtwork::alloc(), bounds, &handler);
            *cache = Some((art.id, artwork.clone()));
            Some(artwork)
        }
    }

    pub(super) fn publish(info: &Info) {
        claim();
        unsafe {
            let center = MPNowPlayingInfoCenter::defaultCenter();
            let dict = NSMutableDictionary::<NSString, AnyObject>::new();
            set_string(&dict, "title", &info.title);
            if !info.artist.is_empty() {
                set_string(&dict, "artist", &info.artist);
            }
            if !info.album.is_empty() {
                set_string(&dict, "albumTitle", &info.album);
            }
            if info.duration > 0.0 {
                set_number(&dict, "playbackDuration", info.duration);
            }
            set_number(&dict, "elapsedPlaybackTime", info.position.max(0.0));
            set_number(&dict, "playbackRate", if info.playing { 1.0 } else { 0.0 });
            if let Some(art) = &info.artwork
                && let Some(artwork) = artwork(art)
            {
                set(&dict, "artwork", &*(&*artwork as *const MPMediaItemArtwork).cast::<AnyObject>());
            }
            center.setNowPlayingInfo(Some(&dict));
            center.setPlaybackState(if info.playing {
                MPNowPlayingPlaybackState::Playing
            } else {
                MPNowPlayingPlaybackState::Paused
            });
        }
    }

    pub(super) fn clear() {
        unsafe {
            let center = MPNowPlayingInfoCenter::defaultCenter();
            center.setNowPlayingInfo(None);
            center.setPlaybackState(MPNowPlayingPlaybackState::Stopped);
        }
        if let Ok(mut cache) = ARTWORK.0.lock() {
            *cache = None;
        }
    }
}
