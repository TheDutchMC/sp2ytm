mod oauth2;
mod env;
mod api;
mod clap;

#[macro_use]
extern crate lazy_static;

use log::{error, warn, info, debug};

fn main() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "sp2ytm=INFO");
    }

    env_logger::init();

    let matches = clap::clap().get_matches();
    let env = env::Env {
        google_client_id: matches.value_of("google-client-id").expect("Missing required 'google-client-id'").to_string(),
        google_client_secret: matches.value_of("google-client-secret").expect("Missing required 'google-client-secret'").to_string(),
        spotify_client_id: matches.value_of("spotify-client-id").expect("Missing required 'spotify-client-id'").to_string(),
        spotify_client_secret: matches.value_of("spotify-client-secret").expect("Missing required 'spotify-client-secret'").to_string()
    };
    let playlist = matches.value_of("playlist-url").expect("Missing required 'playlist-url'");
    let playlist_id_regex = regex::Regex::new(r#"(.*playlist/)(.*)(\?.*)"#).expect("Invalid playlist_id_regex");

    if let Some(captures) = playlist_id_regex.captures(playlist) {
        debug!("Performing authhentication with Google");
        let google_access_token = oauth2::do_oauth(env.clone()).expect("Unable to complete Oauth process");

        let sp_playlist_id = captures.get(2)
            .expect("Missing playlist ID: Capturing group 2")
            .as_str();
        debug!("Found playlist ID: {}", sp_playlist_id);

        let sp_playlist = api::spotify::get_playlist(&env, sp_playlist_id).expect("Failed to fetch Playlist information from Spotify");
        debug!("Got {} tracks for '{}'", sp_playlist.tracks.len(), sp_playlist.name);

        debug!("Creating YouTube playlist");
        let yt_playlist_id = api::youtube::create_playlist(&sp_playlist.name, &google_access_token).expect("Failed to create YouTube playlist");

        debug!("Resolving all Spotify tracks to YouTube IDs and adding them to the playlist");
        for t in sp_playlist.tracks {
            let search = api::youtube::search(&format!("{} {}", t.name, t.artist)).expect("Failed to perform search");
            if let Some(id) = search {
                debug!("Adding track {} to playlist", id);
                api::youtube::insert_track(&yt_playlist_id, &id, &google_access_token).expect("Failed to insert track");
            } else {
                warn!("Unable to find track with search terms: '{} {}'", t.name, t.artist);
                continue;
            }
        }

        info!("Done");
    } else {
        error!("Invalid Spotify playlist url. Regex did not match");
    }

}

