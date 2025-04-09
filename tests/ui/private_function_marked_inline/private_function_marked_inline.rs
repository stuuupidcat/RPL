#[inline]
fn foo11() {}
//~^ERROR: it usually isn’t necessary to apply #[inline] to private functions

#[inline]
pub(crate) fn foo12() {}
//~^ERROR: it usually isn’t necessary to apply #[inline] to private functions
#[inline]
pub fn foo13() {}

#[inline(always)]
fn foo21() {}
//~^ERROR: it usually isn’t necessary to apply #[inline] to private functions

#[inline(always)]
pub(crate) fn foo22() {}
//~^ERROR: it usually isn’t necessary to apply #[inline] to private functions

#[inline(always)]
pub fn foo23() {}

#[inline(never)]
fn foo31() {}

#[inline(never)]
pub(crate) fn foo32() {}

#[inline(never)]
pub fn foo33() {}

trait Trait {
    #[inline]
    fn foo_in_trait() {}
    //~^ERROR: it usually isn’t necessary to apply #[inline] to private functions
}

struct Struct;

impl Struct {
    #[inline]
    fn foo_in_impl() {}
    //~^ERROR: it usually isn’t necessary to apply #[inline] to private functions
}

fn main() {}
