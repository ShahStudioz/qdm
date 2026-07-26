use iced::{window, Size};

mod app;
mod icons;
mod models;
mod theme;
mod views;

use app::QdmApp;

fn main() -> iced::Result {
    iced::application("Quick Download Manager", QdmApp::update, QdmApp::view)
        .subscription(QdmApp::subscription)
        .theme(QdmApp::theme)
        .window(window::Settings {
            size: Size::new(1200.0, 760.0),
            min_size: Some(Size::new(900.0, 600.0)),
            position: window::Position::Centered,
            ..Default::default()
        })
        .font(icons::FONTAWESOME_BYTES)
        .run()
}
