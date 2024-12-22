mod app_cfg;
mod app_fns;
mod arg_parse;
mod disjoint_set;
mod errors;
mod match_db;
mod match_group_ext;

mod search_output;

pub(crate) use app_cfg::*;
pub(crate) use errors::*;

use match_db::MatchDb;
use search_output::SearchOutput;

pub use app_fns::run_app;

#[cfg(all(target_family = "unix", feature = "gui_slint"))]
mod resolution_thunk;
#[cfg(all(target_family = "unix", feature = "gui_slint"))]
pub(crate) use resolution_thunk::*;

#[cfg(all(target_family = "unix", feature = "gui_slint"))]
mod gui_slint;
#[cfg(all(target_family = "unix", feature = "gui_slint"))]
use gui_slint::run_gui_slint;
