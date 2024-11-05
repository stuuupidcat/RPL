#![feature(stmt_expr_attributes)]

#[rpl::print_hir] //~ HELP: remove this attribute
//~^ ERROR: abort due to debugging
//~| NOTE: `#[rpl::dump_hir]`, `#[rpl::print_hir]` and `#[rpl::dump_mir]` are only used for debugging
//~| NOTE: this error is to remind you removing these attributes
use std::sync::Arc; //~ NOTE: use std::sync::Arc;

#[rpl::print_hir] //~ HELP: remove this attribute
mod m {
    //~^ NOTE: mod m {
    pub fn foo() {}
}

#[rpl::print_hir] //~ HELP: remove this attribute
trait Foo {
    //~^ NOTE: trait Foo {
    #[rpl::print_hir] //~ HELP: remove this attribute
    const N: usize; //~ NOTE: const N: usize;
}

#[rpl::print_hir] //~ HELP: remove this attribute
impl Foo for () {
    //~^ NOTE: impl Foo for () {
    #[rpl::print_hir] //~ HELP: remove this attribute
    const N: usize = 0_usize; //~ NOTE: const N: usize = 0usize;
}

#[rpl::print_hir] //~ HELP: remove this attribute
fn main() {
    //~^ NOTE: fn main() {
    #[rpl::print_hir] //~ HELP: remove this attribute
    let x = Arc::new(0_usize); //~ NOTE: let x = Arc::new(0usize);

    #[rpl::print_hir] //~ HELP: remove this attribute
    fn foo() {
        //~^ NOTE: fn foo() {
        #[rpl::print_hir] //~ HELP: remove this attribute
        {} //~ NOTE: { }
    }

    #[rpl::print_hir] //~ HELP: remove this attribute
    if true {
        //~^ NOTE: if true {
    } else {
    }

    #[rpl::print_hir] //~ HELP: remove this attribute
    std::thread::spawn(move || {
        //~^ NOTE: std::thread::spawn(move ||
        println!("{x}");
    });

    #[rpl::print_hir] //~ HELP: remove this attribute
    macro_rules! mac {
        //~^ NOTE: macro_rules! mac {
        () => {
            #[rpl::print_hir] // No effect after expansion.
            println!("test");
        };
    }

    #[rpl::print_hir] // No effect after expansion.
    mac!();
}
