use crate::{
    error::{AppError, Result},
    models::RawLyrics,
};
use amll_lyric::{eqrc::decrypt_qrc_hex, lrc::parse_lrc, qrc::parse_qrc, LyricLineOwned};
use base64::{engine::general_purpose::STANDARD, Engine};
use regex::Regex;
use serde::Serialize;
use std::sync::LazyLock;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeLyrics {
    lines: Vec<LyricLineOwned>,
    warnings: Vec<String>,
    offset: i64,
    word_timed: bool,
}

fn decode(raw: &str) -> Result<String> {
    static XML: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(?s)\bLyricContent\s*=\s*"(.*?)"\s*/?\s*>"#).unwrap());
    static ENTITIES: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"&(?:amp|lt|gt|quot|apos|nbsp|#x[0-9a-fA-F]+|#[0-9]+);").unwrap()
    });
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(String::new());
    }
    let invalid = || AppError::new("LYRICS_FORMAT", "QQ 音乐歌词格式无法解析。");
    let text = if raw.len() % 16 == 0 && raw.bytes().all(|b| b.is_ascii_hexdigit()) {
        if raw.len() > 1024 * 1024 {
            return Err(invalid());
        }
        let decoded = decrypt_qrc_hex(raw);
        if decoded.is_empty() {
            return Err(invalid());
        }
        decoded
    } else if !raw.contains('[')
        && raw.len() % 4 == 0
        && raw
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b))
    {
        String::from_utf8(STANDARD.decode(raw).map_err(|_| invalid())?).map_err(|_| invalid())?
    } else {
        raw.to_owned()
    };
    let content = XML
        .captures(&text)
        .map_or(text.as_str(), |c| c.get(1).unwrap().as_str());
    Ok(ENTITIES
        .replace_all(content, |c: &regex::Captures<'_>| {
            let entity = &c[0];
            match entity {
                "&amp;" => "&".into(),
                "&lt;" => "<".into(),
                "&gt;" => ">".into(),
                "&quot;" => "\"".into(),
                "&apos;" => "'".into(),
                "&nbsp;" => " ".into(),
                _ => {
                    let number = &entity[2..entity.len() - 1];
                    let code = match number.strip_prefix('x') {
                        Some(hex) => u32::from_str_radix(hex, 16),
                        None => number.parse(),
                    };
                    code.ok()
                        .and_then(char::from_u32)
                        .map_or_else(|| entity.to_owned(), |ch| ch.to_string())
                }
            }
        })
        .trim_start_matches('\u{feff}')
        .replace("\r\n", "\n"))
}

fn parse(raw: &str) -> Result<Vec<LyricLineOwned>> {
    parse_with_timing(raw).map(|(lines, _)| lines)
}

fn parse_with_timing(raw: &str) -> Result<(Vec<LyricLineOwned>, bool)> {
    let text = decode(raw)?;
    let timed = parse_qrc(&text);
    let word_timed = !timed.is_empty();
    let lines = if !word_timed {
        parse_lrc(&text)
    } else {
        timed
    };
    Ok((lines
        .into_iter()
        .filter(|line| !line.is_empty())
        .map(Into::into)
        .collect(), word_timed))
}

pub fn normalize(raw: RawLyrics, duration: u64, offset: i64) -> Result<NativeLyrics> {
    let mut warnings = raw.warnings;
    let (mut lines, mut word_timed) = match parse_with_timing(&raw.word) {
        Ok(value) => value,
        Err(error) => {
            warnings.push(error.message);
            (Vec::new(), false)
        }
    };
    let line_source = parse_with_timing(&raw.line);
    let use_line_source = match &line_source {
        Ok((candidate, _)) => {
            lines.is_empty()
                || (candidate.len() > lines.len() + 2
                    && candidate.last().zip(lines.last()).is_some_and(|(end, partial)| {
                        end.start_time > partial.start_time.saturating_add(2000)
                    }))
        }
        Err(_) => lines.is_empty(),
    };
    if use_line_source {
        (lines, word_timed) = line_source?;
        if lines.is_empty() && !warnings.is_empty() {
            return Err(AppError::new("LYRICS_UNAVAILABLE", warnings.join("\n")));
        }
    }
    if !word_timed {
        for i in 0..lines.len() {
            let end = lines
                .get(i + 1)
                .map_or(duration.saturating_mul(1000), |line| line.start_time);
            let line = &mut lines[i];
            line.end_time = end.max(line.start_time);
            for word in &mut line.words {
                word.end_time = line.end_time;
            }
        }
    }
    for (translation, payload) in [(true, raw.translation), (false, raw.romanization)] {
        match parse(&payload) {
            Ok(extra) => {
                for line in &mut lines {
                    if let Some(other) = extra
                        .iter()
                        .find(|other| line.start_time.abs_diff(other.start_time) <= 80)
                    {
                        let text = other
                            .words
                            .iter()
                            .map(|word| word.word.as_str())
                            .collect::<String>();
                        if translation {
                            if text.trim() != "//" {
                                line.translated_lyric = text;
                            }
                        } else {
                            line.roman_lyric = text;
                        }
                    }
                }
            }
            Err(error) => warnings.push(error.message),
        }
    }
    Ok(NativeLyrics {
        lines,
        warnings,
        offset,
        word_timed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_word_times_and_discards_translation_placeholders() {
        let lyrics = normalize(RawLyrics {
            word: concat!(
                "0c8d67dd3e549974b64ed2680459f138553fe2b7cdeb268e33263ce3ef746059a2b99f78e6c007503dc411c4bbec6964de68e",
                "aa80c5a40da58034f7b420d28a610ca884da2ecb1ad6b4bccf283afead6979c1960f179e7dc5f853329941f992fab89d707d080e740",
            ).into(),
            line: "".into(), translation: "[00:00.50]//".into(), romanization: "".into(),
            warnings: vec![],
        }, 2, 120).unwrap();
        assert_eq!(lyrics.offset, 120);
        assert!(lyrics.word_timed);
        assert_eq!(lyrics.lines[0].words[1].start_time, 750);
        assert!(lyrics.lines[0].translated_lyric.is_empty());
    }

    #[test]
    fn does_not_advertise_line_timing_as_word_timing() {
        let lyrics = normalize(RawLyrics {
            word: String::new(),
            line: "[00:01.00]一行\n[00:02.50]下一行".into(),
            translation: String::new(),
            romanization: String::new(),
            warnings: vec![],
        }, 10, 0).unwrap();
        assert!(!lyrics.word_timed);
        assert_eq!(lyrics.lines[0].end_time, 2500);
    }

    #[test]
    fn quotes_inside_qq_xml_do_not_truncate_the_lyrics() {
        let lyrics = normalize(RawLyrics {
            word: "<LyricInfo><Lyric_1 LyricContent=\"[0,1000]Credit: Name \"Alias\"(0,1000)\n[2000,900]一(2000,400)句(2400,500)\"/></LyricInfo>".into(),
            line: String::new(),
            translation: String::new(),
            romanization: String::new(),
            warnings: vec![],
        }, 10, 0).unwrap();
        assert!(lyrics.word_timed);
        assert_eq!(lyrics.lines.len(), 2);
        assert_eq!(lyrics.lines[1].words[1].start_time, 2400);
    }

    #[test]
    fn prefers_complete_line_lyrics_over_a_truncated_word_payload() {
        let lyrics = normalize(RawLyrics {
            word: "[0,500]片段(0,500)".into(),
            line: "[00:00.00]片段\n[00:03.00]一句\n[00:05.00]两句\n[00:07.00]三句".into(),
            translation: String::new(),
            romanization: String::new(),
            warnings: vec![],
        }, 10, 0).unwrap();
        assert!(!lyrics.word_timed);
        assert_eq!(lyrics.lines.len(), 4);
        assert_eq!(lyrics.lines[3].start_time, 7000);
    }
}
