//! Overlay abstraction.
//!
//! Default build: no overlay, just structured `tracing` logs.
//!
//! `overlay` feature: hooks DX11 Present via `hudhook` and draws an
//! ImGui panel with session / rollback / hook diagnostics.
//!
//! The two implementations have identical public APIs so
//! [`super::on_attach`] can call the same function regardless.

#[cfg(not(feature = "overlay"))]
pub mod imp {
    use tracing::info;

    pub fn install() {
        info!("overlay disabled (build without --features overlay to enable hudhook)");
    }

    pub fn uninstall() {}
}

#[cfg(feature = "overlay")]
pub mod imp {
    use tracing::info;

    pub fn install() {
        // `hudhook::Hudhook::builder().apply()` hooks DX11 Present on
        // this process and routes every frame into our render loop.
        // The render loop reads live state out of the `MOD` singleton.
        info!("overlay enabled; installing hudhook");
        match hudhook::Hudhook::builder()
            .with::<hudhook::hooks::dx11::ImguiDx11Hooks>(OverlayState::new())
            .build()
            .apply()
        {
            Ok(_) => info!("hudhook installed"),
            Err(e) => tracing::error!(%e, "hudhook install failed"),
        }
    }

    pub fn uninstall() {
        hudhook::Hudhook::eject();
    }

    struct OverlayState;

    impl OverlayState {
        fn new() -> Self {
            Self
        }
    }

    impl hudhook::ImguiRenderLoop for OverlayState {
        fn render(&mut self, ui: &mut hudhook::imgui::Ui) {
            let m = super::super::global();
            ui.window("sekiro-coop")
                .size([340.0, 260.0], hudhook::imgui::Condition::FirstUseEver)
                .position([16.0, 16.0], hudhook::imgui::Condition::FirstUseEver)
                .build(|| {
                    ui.text(format!(
                        "version: {}",
                        env!("CARGO_PKG_VERSION")
                    ));
                    match m {
                        None => ui.text("mod not initialised"),
                        Some(m) => {
                            ui.text(format!("game: {}", m.version));
                            ui.text(format!("frame: {}", m.current_frame()));
                            ui.text(format!("peer: {:?}", m.authority.peer));
                            ui.text(format!("band: {} entities", m.band.lock().len()));
                            ui.text(format!("snapshots: {}", m.snapshots.lock().len()));
                            let hook_count = sekiro_sdk_core::hook::list().len();
                            ui.text(format!("hooks: {}", hook_count));
                            match m.session.lock().as_ref() {
                                Some(s) => {
                                    let stats = s.reliability.stats.lock();
                                    ui.text(format!(
                                        "rel sent/ack/retx: {} / {} / {}",
                                        stats.sent,
                                        stats.acked,
                                        stats.retransmitted
                                    ));
                                }
                                None => ui.text("session: none"),
                            }
                        }
                    }
                });
        }
    }
}

pub use imp::{install, uninstall};
