#![feature(rustc_private)]
#![feature(const_refs_to_static)]
#![feature(let_chains)]
#![feature(decl_macro)]

extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_driver_impl;
extern crate rustc_errors;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

mod callbacks;
// pub mod interface;
// mod passes;

pub use callbacks::{DefaultCallbacks, RplCallbacks, RustcCallbacks, RPL_ARGS_ENV};

static RPL_LOCALE_RESOURCES: &[&str] = &[
    rpl_driver::DEFAULT_LOCALE_RESOURCE,
    rpl_patterns::DEFAULT_LOCALE_RESOURCE,
];

pub static DEFAULT_LOCALE_RESOURCES: [&str;
    rustc_driver_impl::DEFAULT_LOCALE_RESOURCES.len() + RPL_LOCALE_RESOURCES.len()] = {
    let mut resources = ["str"; rustc_driver_impl::DEFAULT_LOCALE_RESOURCES.len() + RPL_LOCALE_RESOURCES.len()];
    let mut i = 0;
    while i < rustc_driver_impl::DEFAULT_LOCALE_RESOURCES.len() {
        resources[i] = rustc_driver_impl::DEFAULT_LOCALE_RESOURCES[i];
        i += 1;
    }
    let mut j = 0;
    while i < resources.len() {
        resources[i] = RPL_LOCALE_RESOURCES[j];
        i += 1;
        j += 1;
    }
    resources
};
