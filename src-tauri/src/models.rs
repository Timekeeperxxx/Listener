use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub uin: String,
    pub nickname: String,
    pub avatar: String,
    #[serde(default)]
    pub offline: bool,
    #[serde(default)]
    pub renewable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: u64,
    pub mid: String,
    pub media_mid: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub cover: String,
    pub duration: u64,
    pub vip: bool,
    // Playback queues and downloads saved before 0.2.4 contain display names only.
    #[serde(default)]
    pub artists: Vec<ArtistRef>,
    #[serde(default)]
    pub album_mid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtistRef {
    pub mid: String,
    pub name: String,
    pub image: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogAlbum {
    pub mid: String,
    pub name: String,
    pub artists: Vec<ArtistRef>,
    pub credit: String,
    pub cover: String,
    pub publish_date: String,
    pub album_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistDetail {
    pub artist: ArtistRef,
    pub description: String,
    pub total_songs: u64,
    pub total_albums: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlbumDetail {
    pub album: CatalogAlbum,
    pub description: String,
    pub language: String,
    pub genre: String,
    pub company: String,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPage<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub next_offset: u64,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub cover: String,
    pub count: u64,
    pub kind: String,
    #[serde(rename = "dirId")]
    pub dir_id: Option<u64>,
    #[serde(default)]
    pub creator: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistPage {
    pub tracks: Vec<Track>,
    pub total: u64,
    pub next_offset: u64,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library {
    pub playlists: Vec<Playlist>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackSource {
    pub url: String,
    pub quality: String,
    pub local: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Download {
    pub track: Track,
    pub quality: String,
    pub filename: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawLyrics {
    pub word: String,
    pub line: String,
    pub translation: String,
    pub romanization: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepeatMode {
    Off,
    All,
    One,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackState {
    pub queue: Vec<Track>,
    pub current_mid: Option<String>,
    pub next_mid: Option<String>,
    pub position: f64,
    pub repeat: RepeatMode,
    pub shuffle: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub volume: f64,
    pub animated_background: bool,
    pub lyric_offset: i64,
    #[serde(default)]
    pub audio_quality: AudioQuality,
    #[serde(default)]
    pub lyrics: LyricSettings,
    #[serde(default = "enabled")]
    pub preload_next: bool,
    #[serde(default = "enabled")]
    pub close_to_tray: bool,
    #[serde(default)]
    pub shortcuts: ShortcutSettings,
    #[serde(default = "cache_limit")]
    pub cache_limit_mb: u32,
}

fn enabled() -> bool {
    true
}
fn cache_limit() -> u32 {
    64
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricSettings {
    pub font_scale: u16,
    pub line_height: f64,
    pub show_translation: bool,
    pub show_romanization: bool,
}
impl Default for LyricSettings {
    fn default() -> Self {
        Self {
            font_scale: 100,
            line_height: 1.5,
            show_translation: true,
            show_romanization: false,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutSettings {
    pub enabled: bool,
    pub toggle: String,
    pub next: String,
    pub previous: String,
    pub main: String,
}
impl Default for ShortcutSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            toggle: "Control+Alt+Space".into(),
            next: "Control+Alt+ArrowRight".into(),
            previous: "Control+Alt+ArrowLeft".into(),
            main: "Control+Alt+KeyM".into(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedCatalog {
    pub artists: Vec<ArtistRef>,
    pub albums: Vec<CatalogAlbum>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageUsage {
    pub cache_bytes: u64,
    pub cache_count: u64,
    pub download_bytes: u64,
    pub download_count: u64,
    pub database_bytes: u64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioQuality {
    #[default]
    Auto,
    Standard,
    High,
    Lossless,
    Hires,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            volume: 0.7,
            animated_background: true,
            lyric_offset: 0,
            audio_quality: AudioQuality::Auto,
            lyrics: LyricSettings::default(),
            preload_next: true,
            close_to_tray: true,
            shortcuts: ShortcutSettings::default(),
            cache_limit_mb: cache_limit(),
        }
    }
}
