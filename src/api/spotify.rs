use anyhow::Result;
use crate::env::Env;
use serde::Deserialize;
use log::debug;
use std::sync::{Arc, Mutex};
use ratelimit_meter::{DirectRateLimiter, LeakyBucket};
use crate::try_rl;

lazy_static! {
    static ref CLIENT: reqwest::blocking::Client = reqwest::blocking::Client::new();
    static ref BUCKET: Arc<Mutex<DirectRateLimiter>> = Arc::new(Mutex::new(DirectRateLimiter::<LeakyBucket>::per_second(nonzero_ext::nonzero!(10u32))));
}

#[derive(Deserialize)]
struct LoginResponse {
    #[serde(rename(serialize= "accessToken"))]
    access_token: String
}

fn get_login_token(env: &Env) -> Result<String> {
    debug!("Requesting Spotify login token");
    let auth_string = format!("{}:{}", env.spotify_client_id, env.spotify_client_secret);
    let auth_string = base64::encode(auth_string);
    debug!("Using Authorization: {:?}", &auth_string);

    let response: LoginResponse = try_rl!(BUCKET, CLIENT
        .post("https://accounts.spotify.com/api/token")
        .body("grant_type=client_credentials")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Authorization", format!("Basic {}", auth_string))
        .send()?
        .json()?);

    Ok(response.access_token)
}

#[derive(Deserialize)]
struct TrackResponse {
    name: String,
    artists: Vec<Artist>
}

#[derive(Deserialize)]
struct Artist {
    name: String
}

#[derive(Deserialize)]
struct PlaylistResponse {
    name: String
}

#[derive(Deserialize)]
struct PlaylistTracksResponse {
    next: Option<String>,
    items: Vec<PlaylistItems>
}

#[derive(Deserialize)]
struct PlaylistItems {
    track: TrackResponse
}

pub struct Playlist {
    pub tracks: Vec<PlaylistTrack>,
    pub name: String
}

pub struct PlaylistTrack {
    pub name: String,
    pub artist: String
}

pub fn get_playlist(env: &Env, id: &str) -> Result<Playlist> {
    let login_token = get_login_token(env)?;
    let header = format!("Bearer {}", login_token);
    debug!("Using Authorization header: {}", &header);

    let res = try_rl!(BUCKET, CLIENT
        .get(format!("https://api.spotify.com/v1/playlists/{}", id))
        .header("Authorization", &header)
        .send()?);

    debug!("Got HTTP status {}", &res.status());
    let response: PlaylistResponse = res.json()?;

    let tracks = get_playlist_next(&format!("https://api.spotify.com/v1/playlists/{}/tracks?offset=0&limit=100", id), &header)?;
    let tracks: Vec<_> = tracks.into_iter()
        .map(|f| PlaylistTrack {
            name: f.name,
            artist: f.artists.get(0).unwrap_or(&Artist {
                name: String::default()
            }).name.clone()
        })
        .collect();

    Ok(Playlist {
        name: response.name,
        tracks
    })
}

fn get_playlist_next(next: &str, auth_header_value: &str) -> Result<Vec<TrackResponse>> {
    let resp: PlaylistTracksResponse = try_rl!(BUCKET, CLIENT
        .get(next)
        .header("Authorization", auth_header_value)
        .send()?
        .json()?);

    let mut tracks: Vec<_> = resp.items
        .into_iter()
        .map(|f| f.track)
        .collect();

    if let Some(ref next) = resp.next {
        tracks.append(&mut get_playlist_next(next, auth_header_value)?);
    }

    Ok(tracks)
}