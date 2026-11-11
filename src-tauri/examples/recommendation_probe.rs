//! Read-only live recommendation check. Never prints credentials or playback URLs.
use listener_lib::{auth, models::AudioQuality, qq::QqMusic};
use serde_json::json;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("Loading the existing account from macOS Keychain");
    let auth = auth::load()?.ok_or("No saved account")?;
    let qq = QqMusic::new()?;
    qq.profile(&auth).await?;
    eprintln!("Authenticated account verified");
    let daily = qq.daily_recommendation(&auth).await?;
    assert_eq!(daily.playlist.count, daily.tracks.len() as u64);
    let radio = qq.recommendation_radio(&auth).await?;
    let next_radio = qq.recommendation_radio(&auth).await?;
    let source = qq
        .source(&auth, &daily.tracks[0], AudioQuality::Auto)
        .await?;
    assert!(source.url.starts_with("https://"));
    println!(
        "{}",
        json!({
            "dailyCount":daily.tracks.len(), "dailyDay":daily.day,
            "radioCount":radio.len(), "nextRadioCount":next_radio.len(),
            "radioBatchChanged":radio.iter().map(|t| &t.mid).collect::<Vec<_>>() != next_radio.iter().map(|t| &t.mid).collect::<Vec<_>>(),
            "dailySourceAvailable":true,
            "catalogLinksComplete":daily.tracks.iter().chain(radio.iter()).all(|t| !t.album_mid.is_empty() && !t.artists.is_empty())
        })
    );
    Ok(())
}
