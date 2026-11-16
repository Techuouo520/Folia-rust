// Song credit links preserve platform artist identities and unresolved legacy credits.

use folia_core::{OnlineId, Track};

use crate::catalog::Message;

pub(crate) fn artists(track: &Track, enabled: bool) -> Vec<(String, Option<Message>)> {
    if let (Some(song), Some(info)) = (&track.online, &track.online_info)
        && !info.artists.is_empty()
        && info.artists.iter().all(|artist| !artist.id.is_empty())
    {
        return info
            .artists
            .iter()
            .map(|artist| {
                (
                    artist.name.clone(),
                    enabled.then(|| {
                        Message::OpenArtist(OnlineId { provider: song.provider.clone(), id: artist.id.clone() })
                    }),
                )
            })
            .collect();
    }
    track
        .artist
        .as_ref()
        .filter(|name| !name.is_empty())
        .map(|name| {
            vec![(
                name.clone(),
                track.online.as_ref().filter(|_| enabled).map(|_| Message::TrackArtists(Box::new(track.clone()))),
            )]
        })
        .unwrap_or_default()
}

pub(crate) fn album(track: &Track, enabled: bool) -> Option<Message> {
    track.online.as_ref().filter(|_| enabled).map(|_| Message::TrackAlbum(Box::new(track.clone())))
}
