use crate::{
    error::{AppError, Result},
    models::{Download, PlaybackState, Settings, Track},
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    collections::HashSet,
    path::Path,
    sync::{Mutex, MutexGuard},
    time::{SystemTime, UNIX_EPOCH},
};

pub struct Database(Mutex<Connection>);

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        let db = Connection::open(path)?;
        db.execute_batch("PRAGMA journal_mode=WAL;
            CREATE TABLE IF NOT EXISTS cache (
                owner TEXT NOT NULL, key TEXT NOT NULL, payload TEXT NOT NULL,
                updated_at INTEGER NOT NULL, PRIMARY KEY(owner, key));
            CREATE TABLE IF NOT EXISTS history (
                owner TEXT NOT NULL, mid TEXT NOT NULL, track TEXT NOT NULL,
                played_at INTEGER NOT NULL, PRIMARY KEY(owner, mid));
            CREATE TABLE IF NOT EXISTS settings (id INTEGER PRIMARY KEY CHECK(id=1), payload TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS playback (owner TEXT PRIMARY KEY NOT NULL, payload TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS downloads (owner TEXT NOT NULL, mid TEXT NOT NULL, payload TEXT NOT NULL, PRIMARY KEY(owner,mid));
            CREATE TABLE IF NOT EXISTS search_history (owner TEXT NOT NULL, keyword TEXT NOT NULL, searched_at INTEGER NOT NULL, PRIMARY KEY(owner,keyword));
            CREATE TABLE IF NOT EXISTS saved_catalog (owner TEXT NOT NULL, kind TEXT NOT NULL CHECK(kind IN ('artist','album')), mid TEXT NOT NULL, payload TEXT NOT NULL, saved_at INTEGER NOT NULL, PRIMARY KEY(owner,kind,mid));
            CREATE TABLE IF NOT EXISTS lyric_offsets (owner TEXT NOT NULL, mid TEXT NOT NULL, offset_ms INTEGER NOT NULL, PRIMARY KEY(owner,mid));")?;
        Ok(Self(Mutex::new(db)))
    }
    pub(crate) fn connection(&self) -> Result<MutexGuard<'_, Connection>> {
        self.0
            .lock()
            .map_err(|_| AppError::new("DATABASE_ERROR", "本地资料库锁异常，请重新启动应用。"))
    }
    pub fn cached<T: DeserializeOwned>(
        &self,
        owner: &str,
        key: &str,
        max_age: u64,
    ) -> Result<Option<T>> {
        let row: Option<(String, u64)> = self
            .connection()?
            .query_row(
                "SELECT payload, updated_at FROM cache WHERE owner=?1 AND key=?2",
                params![owner, key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        match row {
            Some((payload, updated_at)) if now().saturating_sub(updated_at) < max_age => {
                Ok(Some(serde_json::from_str(&payload)?))
            }
            _ => Ok(None),
        }
    }
    pub fn cache<T: Serialize>(&self, owner: &str, key: &str, value: &T) -> Result<()> {
        self.connection()?.execute(
            "INSERT INTO cache(owner,key,payload,updated_at) VALUES(?1,?2,?3,?4)
            ON CONFLICT(owner,key) DO UPDATE SET payload=excluded.payload,updated_at=excluded.updated_at",
            params![owner, key, serde_json::to_string(value)?, now()],
        )?;
        if key.starts_with("lyrics:") {
            self.prune_cache(
                owner,
                u64::from(self.settings()?.cache_limit_mb) * 1024 * 1024,
            )?;
        }
        Ok(())
    }
    pub fn record(&self, owner: &str, track: &Track) -> Result<()> {
        let mut db = self.connection()?;
        let tx = db.transaction()?;
        tx.execute(
            "INSERT INTO history(owner,mid,track,played_at) VALUES(?1,?2,?3,?4)
            ON CONFLICT(owner,mid) DO UPDATE SET track=excluded.track,played_at=excluded.played_at",
            params![owner, track.mid, serde_json::to_string(track)?, now()],
        )?;
        tx.execute(
            "DELETE FROM history WHERE owner=?1 AND mid NOT IN
            (SELECT mid FROM history WHERE owner=?1 ORDER BY played_at DESC LIMIT 200)",
            [owner],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn history(&self, owner: &str) -> Result<Vec<Track>> {
        let db = self.connection()?;
        let mut stmt = db.prepare(
            "SELECT track FROM history WHERE owner=?1 ORDER BY played_at DESC LIMIT 200",
        )?;
        let rows = stmt.query_map([owner], |r| r.get::<_, String>(0))?;
        rows.map(|row| Ok(serde_json::from_str(&row?)?)).collect()
    }
    pub fn downloads(&self, owner: &str) -> Result<Vec<Download>> {
        let db = self.connection()?;
        let mut stmt =
            db.prepare("SELECT payload FROM downloads WHERE owner=?1 ORDER BY rowid DESC")?;
        let rows = stmt.query_map([owner], |r| r.get::<_, String>(0))?;
        rows.map(|row| Ok(serde_json::from_str(&row?)?)).collect()
    }
    pub fn save_download(&self, owner: &str, download: &Download) -> Result<()> {
        self.connection()?.execute(
            "INSERT INTO downloads(owner,mid,payload) VALUES(?1,?2,?3)",
            params![owner, download.track.mid, serde_json::to_string(download)?],
        )?;
        Ok(())
    }
    pub fn remove_download(&self, owner: &str, mid: &str) -> Result<()> {
        self.connection()?.execute(
            "DELETE FROM downloads WHERE owner=?1 AND mid=?2",
            params![owner, mid],
        )?;
        Ok(())
    }
    pub fn settings(&self) -> Result<Settings> {
        let raw: Option<String> = self
            .connection()?
            .query_row("SELECT payload FROM settings WHERE id=1", [], |r| r.get(0))
            .optional()?;
        match raw {
            Some(raw) => Ok(serde_json::from_str(&raw)?),
            None => Ok(Settings::default()),
        }
    }
    pub fn playback(&self, owner: &str) -> Result<Option<PlaybackState>> {
        let raw: Option<String> = self
            .connection()?
            .query_row(
                "SELECT payload FROM playback WHERE owner=?1",
                [owner],
                |r| r.get(0),
            )
            .optional()?;
        match raw {
            Some(raw) => {
                let playback = serde_json::from_str(&raw)?;
                validate_playback(&playback)?;
                Ok(Some(playback))
            }
            None => Ok(None),
        }
    }
    pub fn save_playback(&self, owner: &str, playback: &PlaybackState) -> Result<()> {
        validate_playback(playback)?;
        self.connection()?.execute(
            "INSERT INTO playback(owner,payload) VALUES(?1,?2)
            ON CONFLICT(owner) DO UPDATE SET payload=excluded.payload",
            params![owner, serde_json::to_string(playback)?],
        )?;
        Ok(())
    }
    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        if !settings.volume.is_finite()
            || !(0.0..=1.0).contains(&settings.volume)
            || !(-5000..=5000).contains(&settings.lyric_offset)
            || !(70..=150).contains(&settings.lyrics.font_scale)
            || !settings.lyrics.line_height.is_finite()
            || !(1.2..=2.2).contains(&settings.lyrics.line_height)
            || !(16..=512).contains(&settings.cache_limit_mb)
        {
            return Err(AppError::new(
                "INVALID_SETTINGS",
                "音量或歌词偏移超出有效范围。",
            ));
        }
        self.connection()?.execute(
            "INSERT INTO settings(id,payload) VALUES(1,?1)
            ON CONFLICT(id) DO UPDATE SET payload=excluded.payload",
            [serde_json::to_string(settings)?],
        )?;
        Ok(())
    }
}

fn validate_playback(playback: &PlaybackState) -> Result<()> {
    let invalid = || AppError::new("INVALID_PLAYBACK", "播放队列或进度无效，无法保存或恢复。");
    if !playback.position.is_finite() || playback.position < 0.0 {
        return Err(invalid());
    }
    let mut mids = HashSet::new();
    for track in &playback.queue {
        crate::qq::validate_track(track)?;
        if !mids.insert(track.mid.as_str()) {
            return Err(invalid());
        }
    }
    match &playback.current_mid {
        Some(mid) if mids.contains(mid.as_str()) => {}
        None if playback.queue.is_empty() && playback.position == 0.0 => {}
        _ => return Err(invalid()),
    }
    if let Some(mid) = &playback.next_mid {
        if !mids.contains(mid.as_str()) || playback.current_mid.as_ref() == Some(mid) {
            return Err(invalid());
        }
    }
    Ok(())
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before 1970")
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playback() -> PlaybackState {
        let track = |id| Track {
            artists: Vec::new(),
            album_mid: String::new(),
            id,
            mid: format!("mid{id}"),
            media_mid: format!("media{id}"),
            title: format!("Track {id}"),
            artist: "Artist".into(),
            album: "Album".into(),
            cover: String::new(),
            duration: 120,
            vip: false,
        };
        PlaybackState {
            queue: vec![track(1), track(2), track(3)],
            current_mid: Some("mid2".into()),
            next_mid: Some("mid3".into()),
            position: 42.5,
            repeat: crate::models::RepeatMode::All,
            shuffle: true,
        }
    }

    #[test]
    fn downloads_are_account_scoped_and_duplicate_records_are_rejected() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        let record = Download {
            track: playback().queue.remove(0),
            quality: "无损 FLAC".into(),
            filename: "test.flac".into(),
            bytes: 100,
        };
        db.save_download("account-a", &record).unwrap();
        assert!(db.downloads("account-b").unwrap().is_empty());
        assert_eq!(db.downloads("account-a").unwrap().len(), 1);
        assert!(db.save_download("account-a", &record).is_err());
        db.remove_download("account-b", &record.track.mid).unwrap();
        assert_eq!(db.downloads("account-a").unwrap().len(), 1);
    }

    #[test]
    fn playback_round_trips_without_crossing_accounts() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        let saved = playback();
        db.save_playback("account-a", &saved).unwrap();
        assert_eq!(
            serde_json::to_value(db.playback("account-a").unwrap().unwrap()).unwrap(),
            serde_json::to_value(&saved).unwrap()
        );
        assert!(db.playback("account-b").unwrap().is_none());
    }

    #[test]
    fn invalid_playback_does_not_replace_a_valid_queue() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        let saved = playback();
        db.save_playback("account-a", &saved).unwrap();
        for case in 0..8 {
            let mut invalid = saved.clone();
            match case {
                0 => invalid.position = -1.0,
                1 => invalid.position = f64::NAN,
                2 => invalid.queue.push(invalid.queue[0].clone()),
                3 => invalid.current_mid = Some("missing".into()),
                4 => invalid.current_mid = None,
                5 => invalid.next_mid = Some("missing".into()),
                6 => invalid.next_mid = invalid.current_mid.clone(),
                7 => invalid.queue[0].id = 0,
                _ => unreachable!(),
            }
            assert!(
                db.save_playback("account-a", &invalid).is_err(),
                "case {case}"
            );
        }
        assert_eq!(db.playback("account-a").unwrap().unwrap().position, 42.5);
    }

    #[test]
    fn an_empty_queue_can_replace_an_old_queue() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        let mut saved = playback();
        db.save_playback("account-a", &saved).unwrap();
        saved.queue.clear();
        saved.current_mid = None;
        saved.next_mid = None;
        saved.position = 0.0;
        db.save_playback("account-a", &saved).unwrap();
        assert!(db.playback("account-a").unwrap().unwrap().queue.is_empty());
    }

    #[test]
    fn corrupt_playback_is_an_error_instead_of_an_empty_queue() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        let mut invalid = playback();
        invalid.current_mid = Some("missing".into());
        db.connection()
            .unwrap()
            .execute(
                "INSERT INTO playback(owner,payload) VALUES(?1,?2)",
                params!["account-a", serde_json::to_string(&invalid).unwrap()],
            )
            .unwrap();
        assert_eq!(
            db.playback("account-a").unwrap_err().code,
            "INVALID_PLAYBACK"
        );
    }

    #[test]
    fn reopening_an_existing_library_preserves_its_settings_and_queue() {
        let path = std::env::temp_dir().join(format!(
            "listener-playback-{}.sqlite3",
            uuid::Uuid::new_v4()
        ));
        {
            let db = Connection::open(&path).unwrap();
            db.execute_batch("CREATE TABLE settings (id INTEGER PRIMARY KEY CHECK(id=1), payload TEXT NOT NULL);").unwrap();
            db.execute(
                "INSERT INTO settings(id,payload) VALUES(1,?1)",
                [serde_json::to_string(&Settings::default()).unwrap()],
            )
            .unwrap();
        }
        {
            let db = Database::open(&path).unwrap();
            assert_eq!(db.settings().unwrap().volume, 0.7);
            db.save_playback("account-a", &playback()).unwrap();
        }
        {
            let db = Database::open(&path).unwrap();
            assert_eq!(db.playback("account-a").unwrap().unwrap().position, 42.5);
            assert_eq!(db.settings().unwrap().volume, 0.7);
        }
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn cache_is_scoped_to_its_account() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        db.cache("account-a", "lyrics:1", &vec!["line"]).unwrap();
        assert!(db
            .cached::<Vec<String>>("account-b", "lyrics:1", 60)
            .unwrap()
            .is_none());
        assert_eq!(
            db.cached::<Vec<String>>("account-a", "lyrics:1", 60)
                .unwrap()
                .unwrap(),
            vec!["line"]
        );
    }
    #[test]
    fn settings_reject_invalid_volume() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        assert!(db
            .save_settings(&Settings {
                volume: 2.0,
                ..Settings::default()
            })
            .is_err());
    }
    #[test]
    fn previous_settings_without_quality_keep_the_original_mp3_policy() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        db.connection()
            .unwrap()
            .execute(
                "INSERT INTO settings(id,payload) VALUES(1,?1)",
                [r#"{"volume":0.4,"animatedBackground":false,"lyricOffset":100}"#],
            )
            .unwrap();
        let settings = db.settings().unwrap();
        assert_eq!(settings.volume, 0.4);
        assert!(matches!(
            settings.audio_quality,
            crate::models::AudioQuality::Auto
        ));
    }
}
