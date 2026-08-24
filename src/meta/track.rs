use serde::{Deserialize, Serialize};

pub const ALBUM_COVER_BASE: &str = "https://cdn.listen.moe/covers/";
pub const ARTIST_IMAGE_BASE: &str = "https://cdn.listen.moe/artists/";

/// Track info sent to the UI thread.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackInfo {
    pub artist: String,
    /// The first credited artist, used for services that expect one artist
    /// rather than the UI's comma-separated display value.
    #[serde(default)]
    pub primary_artist: String,
    pub title: String,
    pub album: String,
    pub album_cover: Option<String>,
    pub artist_image: Option<String>,
    pub start_time_ms: u64,
    pub duration_secs: u32,
}

impl TrackInfo {
    pub fn lyrics_artist(&self) -> &str {
        if self.primary_artist.trim().is_empty() {
            &self.artist
        } else {
            &self.primary_artist
        }
    }

    pub fn end_time_ms(&self) -> u64 {
        self.start_time_ms
            .saturating_add(u64::from(self.duration_secs).saturating_mul(1000))
    }

    pub fn contains_timestamp_ms(&self, timestamp_ms: u64) -> bool {
        if self.duration_secs == 0 {
            return timestamp_ms >= self.start_time_ms;
        }

        timestamp_ms >= self.start_time_ms && timestamp_ms < self.end_time_ms()
    }
}

#[cfg(test)]
mod tests {
    use super::TrackInfo;

    #[test]
    fn old_serialized_tracks_fall_back_to_the_display_artist() {
        let track: TrackInfo = serde_json::from_str(
            r#"{
                "artist":"First, Second",
                "title":"Test title",
                "album":"Test album",
                "album_cover":null,
                "artist_image":null,
                "start_time_ms":1000,
                "duration_secs":120
            }"#,
        )
        .expect("legacy track should deserialize");

        assert!(track.primary_artist.is_empty());
        assert_eq!(track.lyrics_artist(), "First, Second");
    }

    #[test]
    fn primary_artist_is_preferred_for_lyrics() {
        let track: TrackInfo = serde_json::from_str(
            r#"{
                "artist":"First, Second",
                "primary_artist":"First",
                "title":"Test title",
                "album":"Test album",
                "album_cover":null,
                "artist_image":null,
                "start_time_ms":1000,
                "duration_secs":120
            }"#,
        )
        .expect("track should deserialize");

        assert_eq!(track.lyrics_artist(), "First");
    }
}
