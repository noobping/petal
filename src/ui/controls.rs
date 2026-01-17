use mpris_server::{Metadata, PlaybackStatus, Player, TrackId};
use std::{cell::RefCell, rc::Rc, sync::mpsc};

#[cfg(any(debug_assertions, feature = "beta"))]
const APP_ID: &str = "io.github.noobping.listenmoe.beta";
#[cfg(not(any(debug_assertions, feature = "beta")))]
const APP_ID: &str = "io.github.noobping.listenmoe";

pub struct MprisControls {
    player: Rc<Player>,
    track_counter: Rc<RefCell<u64>>,
}

impl MprisControls {
    pub fn set_playback(&self, status: PlaybackStatus) {
        let player = self.player.clone();
        glib::MainContext::default().spawn_local(async move {
            let _ = player.set_playback_status(status).await;
        });
    }

    pub fn set_metadata(&self, title: String, artist: String, album: String, art_url: Option<String>) {
        let player = self.player.clone();
        let track_counter = self.track_counter.clone();

        glib::MainContext::default().spawn_local(async move {
            // Object path: /io/github/noobping/listenmoe/track/NNN
            let mut n = track_counter.borrow_mut();
            *n += 1;
            let track_id = TrackId::try_from(format!("/io/github/noobping/listenmoe/track/{}", *n))
                .unwrap_or(TrackId::NO_TRACK);

            let mut b = Metadata::builder()
                .trackid(track_id)
                .title(title)
                .artist([artist])
                .album(album);

            if let Some(url) = art_url {
                b = b.art_url(url);
            }

            let _ = player.set_metadata(b.build()).await;
        });
    }
}

pub fn build_controls_mpris(
    // you can keep your original args if you want; not all are needed here
) -> (Rc<MprisControls>, mpsc::Receiver<MediaControlEvent>) {
    let (ctrl_tx, ctrl_rx) = mpsc::channel::<MediaControlEvent>();

    // IMPORTANT:
    // - bus_name_suffix: can be whatever unique-ish (I avoid dots to keep it simple)
    // - desktop_entry: MUST match the installed .desktop basename (no ".desktop")
    let ctx = glib::MainContext::default();
    let player = ctx.block_on(async {
        Player::builder(env!("CARGO_PKG_NAME")) // org.mpris.MediaPlayer2.listenmoe
            .identity("Listen Moe")
            .desktop_entry(APP_ID)
            .can_control(true)
            .can_play(true)
            .can_pause(true)
            .can_go_next(true)
            .can_go_previous(false)
            .build()
            .await
            .expect("Failed to init MPRIS player")
    });

    let player = Rc::new(player);

    // Must run the server task ASAP after creating the player. :contentReference[oaicite:3]{index=3}
    ctx.spawn_local(player.run());

    // Wire MPRIS method calls -> your existing event channel
    {
        let tx = ctrl_tx.clone();
        player.connect_play(move |_| { let _ = tx.send(MediaControlEvent::Play); });
    }
    {
        let tx = ctrl_tx.clone();
        player.connect_pause(move |_| { let _ = tx.send(MediaControlEvent::Pause); });
    }
    {
        let tx = ctrl_tx.clone();
        player.connect_stop(move |_| { let _ = tx.send(MediaControlEvent::Stop); });
    }
    {
        let tx = ctrl_tx.clone();
        player.connect_play_pause(move |_| { let _ = tx.send(MediaControlEvent::Toggle); });
    }
    {
        let tx = ctrl_tx.clone();
        player.connect_next(move |_| { let _ = tx.send(MediaControlEvent::Next); });
    }

    let controls = Rc::new(MprisControls {
        player,
        track_counter: Rc::new(RefCell::new(0)),
    });

    (controls, ctrl_rx)
}
