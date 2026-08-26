//! Handler modules for processing QDM application messages.
//!
//! Each submodule handles a specific category of messages, keeping the
//! main update dispatcher in [`super::QdmApp::update`] thin and organized.

pub(crate) mod dialogs;
pub(crate) mod downloads;
pub(crate) mod engine;
pub(crate) mod navigation;
pub(crate) mod network;
pub(crate) mod queue;
