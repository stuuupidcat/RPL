#![feature(rustc_private)]
#![feature(let_chains)]
#![feature(decl_macro)]
#![recursion_limit = "1024"]

extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_driver_impl;
extern crate rustc_errors;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

mod callbacks;

pub use callbacks::{DefaultCallbacks, RPL_ARGS_ENV, RplCallbacks, RustcCallbacks};

static RPL_LOCALE_RESOURCES: &[&str] = &[
    rpl_driver::DEFAULT_LOCALE_RESOURCE,
    // rpl_patterns::DEFAULT_LOCALE_RESOURCE,
    rpl_utils::DEFAULT_LOCALE_RESOURCE,
];

pub fn default_locale_resources() -> Vec<&'static str> {
    [rustc_driver_impl::DEFAULT_LOCALE_RESOURCES, RPL_LOCALE_RESOURCES]
        .into_iter()
        .flatten()
        .copied()
        .collect()
}
