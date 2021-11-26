use anyhow::Result;
use serde::{Serialize, Deserialize};
use log::debug;
use std::sync::{Arc, Mutex};
use ratelimit_meter::{DirectRateLimiter, LeakyBucket};
use reqwest::StatusCode;
use crate::try_rl;

lazy_static! {
    static ref CLIENT: reqwest::blocking::Client = reqwest::blocking::Client::new();
    static ref BUCKET: Arc<Mutex<DirectRateLimiter>> = Arc::new(Mutex::new(DirectRateLimiter::<LeakyBucket>::per_second(nonzero_ext::nonzero!(10u32))));
    static ref SEARCH_BUCKET: Arc<Mutex<DirectRateLimiter>> = Arc::new(Mutex::new(DirectRateLimiter::<LeakyBucket>::per_second(nonzero_ext::nonzero!(1u32))));
}

#[derive(Serialize, Deserialize)]
struct Playlist {
    snippet: PlaylistSnippet,
    id: Option<String>
}

#[derive(Serialize, Deserialize)]
struct PlaylistSnippet {
    title: String
}

pub fn create_playlist(name: &str, auth: &str) -> Result<String> {
    let req = Playlist {
        snippet: PlaylistSnippet {
            title: name.to_string()
        },
        id: None
    };

    let res = try_rl!(BUCKET, CLIENT
        .post("https://www.googleapis.com/youtube/v3/playlists?part=snippet")
        .header("Authorization", &format!("Bearer {}", auth))
        .json(&req)
        .send()?);

    debug!("Created playlist, got response code {}", res.status());
    let res: Playlist = res.json()?;

    // Unwrap is safe, the Google API guarantees it to be present
    Ok(res.id.unwrap())
}

#[derive(Serialize)]
struct PlaylistItem {
    snippet: PlaylistItemSnippet
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlaylistItemSnippet {
    playlist_id: String,
    resource_id: ResourceId
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceId {
    kind: &'static str,
    video_id: String
}

pub fn insert_track(playlist_id: &str, resource_id: &str, auth: &str) -> Result<()> {
    let req = PlaylistItem {
        snippet: PlaylistItemSnippet {
            resource_id: ResourceId {
                kind: "youtube#video",
                video_id: resource_id.to_string()
            },
            playlist_id: playlist_id.to_string()
        }
    };

    let res = try_rl!(BUCKET, CLIENT
        .post("https://www.googleapis.com/youtube/v3/playlistItems?part=snippet")
        .header("Authorization", &format!("Bearer {}", auth))
        .json(&req)
        .send()?);

    debug!("Inserted item into playlist, got status: {}", res.status());
    if res.status() != StatusCode::OK {
        debug!("{}", res.text()?);
    }

    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Search {
    contents: Contents
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Contents {
    tabbed_search_results_renderer: TabbedSearchResultsRenderer
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TabbedSearchResultsRenderer {
    tabs: Vec<Tab>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Tab {
    tab_renderer: TabRenderer
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TabRenderer {
    content: Content1
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Content1 {
    section_list_renderer: SectionListRenderer
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SectionListRenderer {
    contents: Vec<Content2>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Content2 {
    music_shelf_renderer: MusicShelfRenderer
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MusicShelfRenderer {
    contents: Vec<Content3>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Content3 {
    music_responsive_list_item_renderer: MusicResponsiveListItemRenderer
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MusicResponsiveListItemRenderer {
    flex_columns: Vec<FlexColumn>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FlexColumn {
    music_responsive_list_item_flex_column_renderer: MusicResponsiveListItemFlexColumnRenderer
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MusicResponsiveListItemFlexColumnRenderer {
    text: Text
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Text {
    runs: Vec<Run>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Run {
    navigation_endpoint: Option<NavigationEndpoint>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NavigationEndpoint {
    watch_endpoint: Option<WatchEndpoint>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WatchEndpoint {
    video_id: String
}

pub fn search(terms: &str) -> Result<Option<String>> {
    let res = try_rl!(SEARCH_BUCKET, CLIENT
        .get(format!("https://music.youtube.com/search?q={}", terms))
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/87.0.4280.88 Safari/537.36 Edg/87.0.664.66")
        .send()?);

    debug!("Got search response status: {}", res.status());
    let res = res.text()?;

    log::trace!("Got response: {}", &res);

    debug!("Extracting JSON from HTML response");
    let data = res
        .split("initialData.push({path: '\\/search',")
        .collect::<Vec<_>>()
        .get(1)
        .expect("Missing 1st data element")
        .split("), data: '")
        .collect::<Vec<_>>()
        .get(1)
        .expect("Missing 2nd data element")
        .split("'});ytcfg.set({'YTMUSIC_INITIAL_DATA'")
        .collect::<Vec<_>>()
        .get(0)
        .expect("Missing 3rd data element")
        .to_string();

    debug!("Unescaping Unicode encoding");
    let unicode = data
        .replace(r#"\x"#, r#"\u00"#)
        .replace(r#"\u0022"#, r#"""#)
        .replace(r#"\u003d"#, "=")
        .replace(r#"\u005d"#, "]")
        .replace(r#"\u005b"#, "[")
        .replace(r#"\u007b"#, "{")
        .replace(r#"\u007d"#, "}")
        .replace(r#"\\"#, r#"\"#);

    debug!("Deserializing JSON");
    log::trace!("{}", &unicode);
    let data: Search = match serde_json::from_str(&unicode) {
        Ok(d) => d,
        Err(e) => {
            log::warn!("Failed to deserialize JSON: {:?}", e);
            log::debug!("Received JSON: {}", &unicode);
            return Err(e.into())
        }
    };
    let ct = data.contents;

    let video_id = get_video_id(ct);
    debug!("Found video ID: {}", video_id.as_ref().unwrap_or(&"<unknown>".to_string()));

    Ok(video_id)
}

fn get_video_id(contents: Contents) -> Option<String> {
    let fcs = &contents
        .tabbed_search_results_renderer
        .tabs
        .get(0)
        .expect("Missing tab 0")
        .tab_renderer
        .content
        .section_list_renderer
        .contents
        .get(1)
        .expect("Missing contents 1")
        .music_shelf_renderer
        .contents
        .get(0)
        .expect("Missing contents 0")
        .music_responsive_list_item_renderer
        .flex_columns;

    for fc in fcs {
        let rs = &fc.music_responsive_list_item_flex_column_renderer.text.runs;
        for r in rs {
            if let Some(ne) = &r.navigation_endpoint {
                if let Some(we) = &ne.watch_endpoint {
                    return Some(we.video_id.clone())
                }
            }
        }
    }

    return None
}