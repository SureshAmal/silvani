pub mod browse;
pub mod details;
pub mod history_view;
pub mod settings_view;

pub use browse::BrowseView;
pub use details::DetailsView;
pub use history_view::HistoryView;
pub use settings_view::SettingsView;

pub mod player_view;
pub use player_view::EmbeddedPlayerView;
