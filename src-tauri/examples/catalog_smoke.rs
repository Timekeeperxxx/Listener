use listener_lib::qq::QqMusic;
use serde_json::json;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let qq = QqMusic::new()?;
    eprintln!("Checking first");
    let first = qq.catalog_artists(None, "", 0, -100).await?;
    eprintln!("Checking second");
    let second = qq.catalog_artists(None, "", 80, -100).await?;
    assert_ne!(first.items[0].mid, second.items[0].mid);
    eprintln!("Checking found");
    let found = qq.catalog_artists(None, "孙燕姿", 0, -100).await?;
    assert!(found.items.iter().any(|a| a.mid == "001pWERg3vFgg8"));
    eprintln!("Checking detail");
    let detail = qq.artist_detail(None, "0025NhlN2yWrP4").await?;
    eprintln!("Checking songs");
    let songs = qq.artist_songs(None, "0025NhlN2yWrP4", 0).await?;
    eprintln!("Checking songs2");
    let songs2 = qq
        .artist_songs(None, "0025NhlN2yWrP4", songs.next_offset)
        .await?;
    assert_ne!(songs.items[0].mid, songs2.items[0].mid);
    eprintln!("Checking albums");
    let albums = qq.artist_albums(None, "0025NhlN2yWrP4", 0).await?;
    eprintln!("Checking albums2");
    let albums2 = qq
        .artist_albums(None, "0025NhlN2yWrP4", albums.next_offset)
        .await?;
    assert_eq!(
        albums.items.len() + albums2.items.len(),
        albums.total as usize
    );
    eprintln!("Checking album");
    let album = qq.album_detail(None, "000MkMni19ClKG").await?;
    assert_eq!(album.tracks.len(), 11);
    eprintln!("Checking track");
    let track = qq.catalog_track(None, "0039MnYb0qxYhV").await?;
    assert_eq!(track.album_mid, "000MkMni19ClKG");
    assert_eq!(track.artists[0].mid, "0025NhlN2yWrP4");
    eprintln!("Checking releases");
    let releases = qq.catalog_albums(None, "", 0, 1).await?;
    eprintln!("Checking releases2");
    let releases2 = qq.catalog_albums(None, "", releases.next_offset, 1).await?;
    assert_ne!(releases.items[0].mid, releases2.items[0].mid);
    eprintln!("Checking album_search");
    let album_search = qq.catalog_albums(None, "叶惠美", 0, 1).await?;
    assert!(album_search.items.iter().any(|a| a.mid == "000MkMni19ClKG"));
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "artistDirectory":first.total,"directoryPagesDistinct":true,"artistSearch":found.items[0],
            "artist":detail,"songsTotal":songs.total,"songsLoaded":songs.items.len()+songs2.items.len(),
            "albumsTotal":albums.total,"albumsLoaded":albums.items.len()+albums2.items.len(),
            "album":album.album,"albumTracks":album.tracks.len(),"albumDescriptionCharacters":album.description.chars().count(),
            "newAlbums":releases.total,"newAlbumPagesDistinct":true,"albumSearchTotal":album_search.total
        }))?
    );
    Ok(())
}
