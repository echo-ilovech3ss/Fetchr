pub mod db;
pub mod sanitizer;
pub mod yt_dlp;
pub mod queue;
pub mod capabilities;
pub mod presets;
pub mod verification;
pub mod logger;

pub fn get_core_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
