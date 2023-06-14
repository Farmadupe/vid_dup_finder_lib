mod app_cfg;
mod app_fns;
mod arg_parse;
mod errors;
mod match_db;
mod match_group_ext;

mod search_output;

pub(crate) use app_cfg::*;
pub(crate) use errors::*;

use match_db::MatchDb;
use search_output::SearchOutput;

pub use app_fns::run_app;

#[cfg(all(target_family = "unix", feature = "gui"))]
mod gui;
#[cfg(all(target_family = "unix", feature = "gui"))]
mod resolution_thunk;
#[cfg(all(target_family = "unix", feature = "gui"))]
use gui::run_gui;
#[cfg(all(target_family = "unix", feature = "gui"))]
pub(crate) use resolution_thunk::*;
