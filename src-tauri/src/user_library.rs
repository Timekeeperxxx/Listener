use crate::{
    catalog::validate_mid,
    db::Database,
    error::{AppError, Result},
    models::*,
};
use rusqlite::{params, OptionalExtension};
use std::time::{SystemTime, UNIX_EPOCH};

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before 1970")
        .as_secs()
}

impl Database {
    pub fn search_history(&self, owner: &str) -> Result<Vec<String>> {
        let db = self.connection()?;
        let mut statement = db.prepare("SELECT keyword FROM search_history WHERE owner=?1 ORDER BY searched_at DESC,rowid DESC LIMIT 20")?;
        let rows = statement.query_map([owner], |r| r.get(0))?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }
    pub fn record_search(&self, owner: &str, keyword: &str) -> Result<()> {
        let keyword = keyword.trim();
        if keyword.is_empty() || keyword.chars().count() > 100 {
            return Err(AppError::new("INVALID_QUERY", "搜索内容需要 1–100 个字。"));
        }
        let mut db = self.connection()?;
        let tx = db.transaction()?;
        tx.execute(
            "DELETE FROM search_history WHERE owner=?1 AND keyword=?2",
            params![owner, keyword],
        )?;
        tx.execute(
            "INSERT INTO search_history(owner,keyword,searched_at) VALUES(?1,?2,?3)",
            params![owner, keyword, now()],
        )?;
        tx.execute("DELETE FROM search_history WHERE owner=?1 AND keyword NOT IN (SELECT keyword FROM search_history WHERE owner=?1 ORDER BY searched_at DESC,rowid DESC LIMIT 20)",[owner])?;
        tx.commit()?;
        Ok(())
    }
    pub fn clear_search_history(&self, owner: &str) -> Result<()> {
        self.connection()?
            .execute("DELETE FROM search_history WHERE owner=?1", [owner])?;
        Ok(())
    }
    #[cfg(test)]
    pub fn saved_catalog(&self, owner: &str) -> Result<SavedCatalog> {
        let db = self.connection()?;
        let mut statement=db.prepare("SELECT kind,payload FROM saved_catalog WHERE owner=?1 ORDER BY saved_at DESC,rowid DESC")?;
        let mut result = SavedCatalog::default();
        for row in statement.query_map([owner], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })? {
            let (kind, payload) = row?;
            match kind.as_str() {
                "artist" => result.artists.push(serde_json::from_str(&payload)?),
                "album" => result.albums.push(serde_json::from_str(&payload)?),
                _ => return Err(AppError::protocol("saved_catalog.kind")),
            }
        }
        Ok(result)
    }
    pub fn replace_saved_catalog(&self, owner: &str, catalog: &SavedCatalog) -> Result<()> {
        let mut db = self.connection()?;
        let tx = db.transaction()?;
        tx.execute("DELETE FROM saved_catalog WHERE owner=?1", [owner])?;
        let timestamp = now();
        for (index, artist) in catalog.artists.iter().enumerate() {
            validate_mid(&artist.mid)?;
            tx.execute(
                "INSERT INTO saved_catalog(owner,kind,mid,payload,saved_at) VALUES(?1,'artist',?2,?3,?4)",
                params![
                    owner,
                    artist.mid,
                    serde_json::to_string(artist)?,
                    timestamp.saturating_sub(index as u64)
                ],
            )?;
        }
        for (index, album) in catalog.albums.iter().enumerate() {
            validate_mid(&album.mid)?;
            tx.execute(
                "INSERT INTO saved_catalog(owner,kind,mid,payload,saved_at) VALUES(?1,'album',?2,?3,?4)",
                params![
                    owner,
                    album.mid,
                    serde_json::to_string(album)?,
                    timestamp.saturating_sub(index as u64)
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn lyric_offset(&self, owner: &str, mid: &str) -> Result<i64> {
        validate_mid(mid)?;
        Ok(self
            .connection()?
            .query_row(
                "SELECT offset_ms FROM lyric_offsets WHERE owner=?1 AND mid=?2",
                params![owner, mid],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(0))
    }
    pub fn save_lyric_offset(&self, owner: &str, mid: &str, offset: i64) -> Result<()> {
        validate_mid(mid)?;
        if !(-5000..=5000).contains(&offset) {
            return Err(AppError::new(
                "INVALID_OFFSET",
                "歌词偏移需要在 -5000–5000 ms 之间。",
            ));
        }
        self.connection()?.execute("INSERT INTO lyric_offsets(owner,mid,offset_ms) VALUES(?1,?2,?3) ON CONFLICT(owner,mid) DO UPDATE SET offset_ms=excluded.offset_ms",params![owner,mid,offset])?;
        Ok(())
    }
    pub fn prune_cache(&self, owner: &str, limit: u64) -> Result<()> {
        let mut db = self.connection()?;
        let tx = db.transaction()?;
        let mut used:u64=tx.query_row("SELECT COALESCE(SUM(LENGTH(CAST(payload AS BLOB))),0) FROM cache WHERE owner=?1 AND key LIKE 'lyrics:%'",[owner],|r|r.get(0))?;
        while used > limit {
            let (key,bytes):(String,u64)=tx.query_row("SELECT key,LENGTH(CAST(payload AS BLOB)) FROM cache WHERE owner=?1 AND key LIKE 'lyrics:%' ORDER BY updated_at,rowid LIMIT 1",[owner],|r|Ok((r.get(0)?,r.get(1)?)))?;
            tx.execute(
                "DELETE FROM cache WHERE owner=?1 AND key=?2",
                params![owner, key],
            )?;
            used -= bytes;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn storage_usage(&self, owner: &str, database_bytes: u64) -> Result<StorageUsage> {
        let (cache_bytes,cache_count)=self.connection()?.query_row("SELECT COALESCE(SUM(LENGTH(CAST(payload AS BLOB))),0),COUNT(*) FROM cache WHERE owner=?1 AND key LIKE 'lyrics:%'",[owner],|r|Ok((r.get(0)?,r.get(1)?)))?;
        let downloads = self.downloads(owner)?;
        Ok(StorageUsage {
            cache_bytes,
            cache_count,
            download_bytes: downloads.iter().map(|d| d.bytes).sum(),
            download_count: downloads.len() as u64,
            database_bytes,
        })
    }
    pub fn clear_lyric_cache(&self, owner: &str) -> Result<()> {
        let db = self.connection()?;
        db.execute(
            "DELETE FROM cache WHERE owner=?1 AND key LIKE 'lyrics:%'",
            [owner],
        )?;
        db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); VACUUM;")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_library_is_account_scoped_and_survives_cache_cleanup() {
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        let artist = ArtistRef {
            mid: "realid".into(),
            name: "Artist".into(),
            image: String::new(),
        };
        db.replace_saved_catalog(
            "a",
            &SavedCatalog {
                artists: vec![artist.clone()],
                albums: vec![],
                ..SavedCatalog::default()
            },
        )
        .unwrap();
        db.record_search("a", "Artist").unwrap();
        db.save_lyric_offset("a", "song", 150).unwrap();
        assert!(db.saved_catalog("b").unwrap().artists.is_empty());
        assert!(db.search_history("b").unwrap().is_empty());
        assert_eq!(db.lyric_offset("b", "song").unwrap(), 0);
        db.cache("a", "profile", &artist).unwrap();
        db.cache("a", "lyrics:1", &"lyrics").unwrap();
        db.clear_lyric_cache("a").unwrap();
        assert_eq!(db.saved_catalog("a").unwrap().artists.len(), 1);
        assert_eq!(db.lyric_offset("a", "song").unwrap(), 150);
        assert!(db
            .cached::<ArtistRef>("a", "profile", 60)
            .unwrap()
            .is_some());
    }
    #[test]
    fn cloud_favorites_replace_local_cache_in_order() {
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        let first = ArtistRef {
            mid: "first".into(),
            name: "First".into(),
            image: String::new(),
        };
        let second = ArtistRef {
            mid: "second".into(),
            name: "Second".into(),
            image: String::new(),
        };
        let album = CatalogAlbum {
            mid: "album".into(),
            name: "Album".into(),
            artists: vec![first.clone()],
            credit: "First".into(),
            cover: String::new(),
            publish_date: String::new(),
            album_type: String::new(),
        };
        db.replace_saved_catalog(
            "a",
            &SavedCatalog {
                artists: vec![first.clone(), second.clone()],
                albums: vec![album],
                ..SavedCatalog::default()
            },
        )
        .unwrap();
        let loaded = db.saved_catalog("a").unwrap();
        assert_eq!(loaded.artists[0].mid, "first");
        assert_eq!(loaded.artists[1].mid, "second");
        assert_eq!(loaded.albums[0].mid, "album");
        assert!(db.saved_catalog("b").unwrap().artists.is_empty());

        db.replace_saved_catalog(
            "a",
            &SavedCatalog {
                artists: vec![second],
                albums: vec![],
                ..SavedCatalog::default()
            },
        )
        .unwrap();
        let loaded = db.saved_catalog("a").unwrap();
        assert_eq!(loaded.artists.len(), 1);
        assert!(loaded.albums.is_empty());
    }
    #[test]
    fn search_history_is_recent_first_unique_and_bounded() {
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        db.record_search("a", "A").unwrap();
        db.record_search("a", "B").unwrap();
        db.record_search("a", "A").unwrap();
        assert_eq!(db.search_history("a").unwrap(), vec!["A", "B"]);
        for index in 0..25 {
            db.record_search("a", &format!("term{index}")).unwrap();
        }
        let history = db.search_history("a").unwrap();
        assert_eq!(history.len(), 20);
        assert_eq!(history[0], "term24");
        db.clear_search_history("a").unwrap();
        assert!(db.search_history("a").unwrap().is_empty());
    }
    #[test]
    fn cache_budget_removes_only_old_lyrics_for_the_selected_account() {
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        db.cache("a", "lyrics:1", &"one").unwrap();
        db.cache("a", "lyrics:2", &"two").unwrap();
        db.cache("b", "lyrics:1", &"other").unwrap();
        db.prune_cache("a", 5).unwrap();
        assert!(db.cached::<String>("a", "lyrics:1", 60).unwrap().is_none());
        assert!(db.cached::<String>("a", "lyrics:2", 60).unwrap().is_some());
        assert!(db.cached::<String>("b", "lyrics:1", 60).unwrap().is_some());
    }
}
