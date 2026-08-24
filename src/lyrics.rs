use std::io::Read;
use std::time::Duration;

use reqwest::blocking::{Client, Response};
use reqwest::{StatusCode, Url};
use serde::Deserialize;

use crate::meta::TrackInfo;

const LRCLIB_GET_ENDPOINT: &str = "https://lrclib.net/api/get";
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_SYNCED_LYRICS_BYTES: usize = 512 * 1024;
const MAX_CUES: usize = 4096;
const MAX_CUE_TEXT_BYTES: usize = 8192;
const MAX_TOTAL_TEXT_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricsRequest {
    pub title: String,
    pub primary_artist: String,
    pub album: String,
    pub duration_secs: u32,
}

impl LyricsRequest {
    pub fn from_track(track: &TrackInfo) -> Self {
        Self {
            title: track.title.clone(),
            primary_artist: track.lyrics_artist().to_owned(),
            album: track.album.clone(),
            duration_secs: track.duration_secs,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricCue {
    pub start_ms: u64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LyricsResult {
    Found(Vec<LyricCue>),
    Missing,
    Error(String),
}

/// Fetch synchronized lyrics from LRCLIB.
///
/// This function is blocking and must be called away from the GTK main thread.
pub fn fetch(request: &LyricsRequest) -> LyricsResult {
    match fetch_inner(request) {
        Ok(result) => result,
        Err(error) => LyricsResult::Error(error),
    }
}

/// Return the cue at or immediately before `elapsed_ms`.
pub fn active_cue_index(cues: &[LyricCue], elapsed_ms: u64) -> Option<usize> {
    cues.partition_point(|cue| cue.start_ms <= elapsed_ms)
        .checked_sub(1)
}

fn fetch_inner(request: &LyricsRequest) -> Result<LyricsResult, String> {
    if request.title.trim().is_empty() || request.primary_artist.trim().is_empty() {
        return Ok(LyricsResult::Missing);
    }

    let user_agent = format!(
        "{}/{} (+{})",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_REPOSITORY")
    );
    let client = Client::builder()
        .user_agent(user_agent)
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|error| format!("could not create lyrics client: {error}"))?;

    let include_album = !request.album.trim().is_empty();
    match fetch_attempt(&client, LRCLIB_GET_ENDPOINT, request, include_album)? {
        FetchAttempt::Found(body) => parse_response(&body),
        FetchAttempt::NotFound if include_album => {
            match fetch_attempt(&client, LRCLIB_GET_ENDPOINT, request, false)? {
                FetchAttempt::Found(body) => parse_response(&body),
                FetchAttempt::NotFound => Ok(LyricsResult::Missing),
            }
        }
        FetchAttempt::NotFound => Ok(LyricsResult::Missing),
    }
}

enum FetchAttempt {
    Found(Vec<u8>),
    NotFound,
}

fn fetch_attempt(
    client: &Client,
    endpoint: &str,
    request: &LyricsRequest,
    include_album: bool,
) -> Result<FetchAttempt, String> {
    let query = query_parameters(request, include_album);
    let mut url = Url::parse(endpoint).map_err(|error| format!("invalid lyrics URL: {error}"))?;
    {
        let mut query_pairs = url.query_pairs_mut();
        for (key, value) in &query {
            query_pairs.append_pair(key, value);
        }
    }
    let response = client
        .get(url)
        .send()
        .map_err(|error| format!("lyrics request failed: {error}"))?;

    if response.status() == StatusCode::NOT_FOUND {
        return Ok(FetchAttempt::NotFound);
    }
    if !response.status().is_success() {
        return Err(format!(
            "lyrics service returned HTTP {}",
            response.status()
        ));
    }

    read_bounded(response).map(FetchAttempt::Found)
}

fn query_parameters(request: &LyricsRequest, include_album: bool) -> Vec<(&'static str, String)> {
    let mut query = vec![
        ("track_name", request.title.clone()),
        ("artist_name", request.primary_artist.clone()),
    ];
    if include_album {
        query.push(("album_name", request.album.clone()));
    }
    if request.duration_secs > 0 {
        query.push(("duration", request.duration_secs.to_string()));
    }
    query
}

fn read_bounded(mut response: Response) -> Result<Vec<u8>, String> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err("lyrics response exceeded the size limit".to_owned());
    }

    let mut body = Vec::new();
    response
        .by_ref()
        .take((MAX_RESPONSE_BYTES + 1) as u64)
        .read_to_end(&mut body)
        .map_err(|error| format!("could not read lyrics response: {error}"))?;
    if body.len() > MAX_RESPONSE_BYTES {
        return Err("lyrics response exceeded the size limit".to_owned());
    }
    Ok(body)
}

#[derive(Deserialize)]
struct LrclibResponse {
    #[serde(default)]
    instrumental: bool,
    #[serde(rename = "syncedLyrics", default)]
    synced_lyrics: Option<String>,
}

fn parse_response(body: &[u8]) -> Result<LyricsResult, String> {
    let response: LrclibResponse = serde_json::from_slice(body)
        .map_err(|error| format!("invalid lyrics response: {error}"))?;
    if response.instrumental {
        return Ok(LyricsResult::Missing);
    }

    let Some(synced_lyrics) = response.synced_lyrics else {
        return Ok(LyricsResult::Missing);
    };
    if synced_lyrics.trim().is_empty() {
        return Ok(LyricsResult::Missing);
    }

    let cues = parse_lrc(&synced_lyrics)?;
    if cues.is_empty() {
        Ok(LyricsResult::Missing)
    } else {
        Ok(LyricsResult::Found(cues))
    }
}

fn parse_lrc(source: &str) -> Result<Vec<LyricCue>, String> {
    if source.len() > MAX_SYNCED_LYRICS_BYTES {
        return Err("synchronized lyrics exceeded the size limit".to_owned());
    }

    let offset_ms = lrc_offset(source);
    let mut entries = Vec::<(u64, String)>::new();
    let mut total_text_bytes = 0usize;

    for line in source.lines() {
        let (timestamps, text) = line_timestamps_and_text(line);
        if timestamps.is_empty() {
            continue;
        }

        let text = text.trim().to_owned();
        if text.len() > MAX_CUE_TEXT_BYTES {
            return Err("a synchronized lyric line exceeded the size limit".to_owned());
        }

        let added_text_bytes = text
            .len()
            .checked_mul(timestamps.len())
            .ok_or_else(|| "synchronized lyrics exceeded the size limit".to_owned())?;
        total_text_bytes = total_text_bytes
            .checked_add(added_text_bytes)
            .ok_or_else(|| "synchronized lyrics exceeded the size limit".to_owned())?;
        if total_text_bytes > MAX_TOTAL_TEXT_BYTES {
            return Err("synchronized lyrics text exceeded the size limit".to_owned());
        }
        if entries.len().saturating_add(timestamps.len()) > MAX_CUES {
            return Err("synchronized lyrics contained too many cues".to_owned());
        }

        for timestamp_ms in timestamps {
            entries.push((apply_offset(timestamp_ms, offset_ms), text.clone()));
        }
    }

    entries.sort_by_key(|(timestamp_ms, _)| *timestamp_ms);
    let mut cues: Vec<LyricCue> = Vec::with_capacity(entries.len());
    for (start_ms, text) in entries {
        if let Some(previous) = cues.last_mut().filter(|cue| cue.start_ms == start_ms) {
            merge_cue_text(&mut previous.text, &text)?;
        } else {
            cues.push(LyricCue { start_ms, text });
        }
    }

    Ok(cues)
}

fn lrc_offset(source: &str) -> i64 {
    let mut offset = 0;
    for line in source.lines() {
        for_each_leading_tag(line, |tag| {
            let Some((key, value)) = tag.split_once(':') else {
                return;
            };
            if key.trim().eq_ignore_ascii_case("offset") {
                if let Ok(parsed) = value.trim().parse::<i64>() {
                    offset = parsed;
                }
            }
        });
    }
    offset
}

fn line_timestamps_and_text(line: &str) -> (Vec<u64>, &str) {
    let mut timestamps = Vec::new();
    let mut rest = line;

    while let Some(after_open) = rest.strip_prefix('[') {
        let Some(close) = after_open.find(']') else {
            break;
        };
        let tag = &after_open[..close];
        if let Some(timestamp_ms) = parse_timestamp(tag.trim()) {
            timestamps.push(timestamp_ms);
        }
        rest = &after_open[close + 1..];
    }

    (timestamps, rest)
}

fn for_each_leading_tag(mut line: &str, mut visit: impl FnMut(&str)) {
    while let Some(after_open) = line.strip_prefix('[') {
        let Some(close) = after_open.find(']') else {
            return;
        };
        visit(&after_open[..close]);
        line = &after_open[close + 1..];
    }
}

fn parse_timestamp(tag: &str) -> Option<u64> {
    if tag.len() > 32 {
        return None;
    }
    let (minutes, seconds_and_fraction) = tag.split_once(':')?;
    if minutes.is_empty() || !minutes.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    let (seconds, fraction) = match seconds_and_fraction
        .split_once('.')
        .or_else(|| seconds_and_fraction.split_once(','))
    {
        Some((seconds, fraction)) => (seconds, Some(fraction)),
        None => (seconds_and_fraction, None),
    };
    if seconds.is_empty() || !seconds.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    let minutes = minutes.parse::<u64>().ok()?;
    let seconds = seconds.parse::<u64>().ok()?;
    if seconds >= 60 {
        return None;
    }

    let fraction_ms = match fraction {
        None => 0,
        Some(value) => fraction_to_ms(value)?,
    };
    minutes
        .checked_mul(60_000)?
        .checked_add(seconds.checked_mul(1000)?)?
        .checked_add(fraction_ms)
}

fn fraction_to_ms(fraction: &str) -> Option<u64> {
    if fraction.is_empty() || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    let digits = fraction.as_bytes();
    let first = u64::from(digits[0] - b'0');
    let second = digits.get(1).map_or(0, |digit| u64::from(*digit - b'0'));
    let third = digits.get(2).map_or(0, |digit| u64::from(*digit - b'0'));
    Some(first * 100 + second * 10 + third)
}

fn apply_offset(timestamp_ms: u64, offset_ms: i64) -> u64 {
    let adjusted = i128::from(timestamp_ms) + i128::from(offset_ms);
    adjusted.clamp(0, i128::from(u64::MAX)) as u64
}

fn merge_cue_text(existing: &mut String, new: &str) -> Result<(), String> {
    if new.is_empty() || existing.split('\n').any(|line| line == new) {
        return Ok(());
    }
    if existing.is_empty() {
        existing.push_str(new);
    } else {
        if existing.len().saturating_add(new.len()).saturating_add(1) > MAX_CUE_TEXT_BYTES {
            return Err("a grouped synchronized lyric cue exceeded the size limit".to_owned());
        }
        existing.push('\n');
        existing.push_str(new);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        active_cue_index, parse_lrc, parse_response, query_parameters, LyricCue, LyricsRequest,
        LyricsResult, MAX_CUES,
    };

    #[test]
    fn parses_precision_multiple_timestamps_and_offset() {
        let cues =
            parse_lrc("[offset:+250]\n[00:01.2]One\n[00:02.34][00:03.4567]Two\n[bad]Ignored")
                .expect("lyrics should parse");

        assert_eq!(
            cues,
            vec![
                LyricCue {
                    start_ms: 1450,
                    text: "One".to_owned(),
                },
                LyricCue {
                    start_ms: 2590,
                    text: "Two".to_owned(),
                },
                LyricCue {
                    start_ms: 3706,
                    text: "Two".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn sorts_groups_deduplicates_and_preserves_empty_cues() {
        let cues = parse_lrc(
            "[00:03.000]Later\n[00:01.000]\n[00:02.000]First\n[00:02.000]First\n[00:02.000]Second",
        )
        .expect("lyrics should parse");

        assert_eq!(cues[0].start_ms, 1000);
        assert!(cues[0].text.is_empty());
        assert_eq!(cues[1].text, "First\nSecond");
        assert_eq!(cues[2].text, "Later");
    }

    #[test]
    fn negative_offset_clamps_to_zero() {
        let cues = parse_lrc("[offset:-1500]\n[00:01.000]Line").expect("lyrics should parse");
        assert_eq!(cues[0].start_ms, 0);
    }

    #[test]
    fn offset_is_global_even_when_declared_after_a_cue() {
        let cues = parse_lrc("[00:01,5]Line\n[offset:+250]").expect("lyrics should parse");
        assert_eq!(cues[0].start_ms, 1750);
    }

    #[test]
    fn ignores_malformed_lines_and_rejects_excessive_cues() {
        assert!(parse_lrc("[00:61.00]No\n[xx:01]No").unwrap().is_empty());

        let source = format!("{}Text", "[00:01.00]".repeat(MAX_CUES + 1));
        assert!(parse_lrc(&source).is_err());
    }

    #[test]
    fn response_requires_synchronized_non_instrumental_lyrics() {
        assert_eq!(
            parse_response(br#"{"instrumental":true,"syncedLyrics":"[00:01]Line"}"#).unwrap(),
            LyricsResult::Missing
        );
        assert_eq!(
            parse_response(br#"{"instrumental":false,"plainLyrics":"Only plain"}"#).unwrap(),
            LyricsResult::Missing
        );
        assert!(matches!(
            parse_response(br#"{"syncedLyrics":"[00:01]Line"}"#).unwrap(),
            LyricsResult::Found(_)
        ));
    }

    #[test]
    fn query_omits_empty_optional_values() {
        let request = LyricsRequest {
            title: "Test title".to_owned(),
            primary_artist: "Test artist".to_owned(),
            album: String::new(),
            duration_secs: 0,
        };
        let query = query_parameters(&request, false);
        assert_eq!(query.len(), 2);
        assert!(query.iter().all(|(key, _)| *key != "album_name"));
        assert!(query.iter().all(|(key, _)| *key != "duration"));
    }

    #[test]
    fn active_cue_uses_last_cue_at_or_before_elapsed_time() {
        let cues = vec![
            LyricCue {
                start_ms: 1000,
                text: "One".to_owned(),
            },
            LyricCue {
                start_ms: 2000,
                text: "Two".to_owned(),
            },
        ];

        assert_eq!(active_cue_index(&cues, 999), None);
        assert_eq!(active_cue_index(&cues, 1000), Some(0));
        assert_eq!(active_cue_index(&cues, 2500), Some(1));
    }
}
