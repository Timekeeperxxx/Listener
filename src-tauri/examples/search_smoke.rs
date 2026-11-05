use listener_lib::qq::QqMusic;
use serde_json::json;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let qq = QqMusic::new()?;
    let songs = qq.search_tracks_page(None, "周杰伦", 0).await?;
    let artists = qq.catalog_artists(None, "周杰伦", 0, -100).await?;
    let albums = qq.catalog_albums(None, "周杰伦", 0, 1).await?;
    let playlists = qq.search_playlists(None, "周杰伦", 0).await?;
    assert!(!songs.items.is_empty());
    assert!(!artists.items.is_empty());
    assert!(!albums.items.is_empty());
    assert!(!playlists.items.is_empty());
    println!("{}", json!({"songs":songs.total,"artists":artists.total,"albums":albums.total,"playlists":playlists.total,"firstPlaylist":playlists.items[0].name}));
    Ok(())
}
