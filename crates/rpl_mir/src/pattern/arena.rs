use rustc_middle::ty::TyCtxt;
use rustc_span::Symbol;

pub trait IntoArena<'tcx, T: ?Sized> {
    fn into_arena(self) -> &'tcx T;
}

impl<'tcx, T: ?Sized> IntoArena<'tcx, T> for &'tcx T {
    fn into_arena(self) -> &'tcx T {
        self
    }
}

impl<'tcx, T, const N: usize> IntoArena<'tcx, [T]> for &'tcx [T; N] {
    fn into_arena(self) -> &'tcx [T] {
        self
    }
}

impl<'tcx> IntoArena<'tcx, [Symbol]> for (TyCtxt<'tcx>, &[&str]) {
    fn into_arena(self) -> &'tcx [Symbol] {
        let (tcx, syms) = self;
        tcx.arena
            .dropless
            .alloc_from_iter(syms.iter().map(|sym| Symbol::intern(sym)))
    }
}

impl<'tcx, const N: usize> IntoArena<'tcx, [Symbol]> for (TyCtxt<'tcx>, &[&str; N]) {
    fn into_arena(self) -> &'tcx [Symbol] {
        let (tcx, syms) = self;
        (tcx, &syms[..]).into_arena()
    }
}

impl<'tcx, T: Copy> IntoArena<'tcx, T> for (TyCtxt<'tcx>, T) {
    fn into_arena(self) -> &'tcx T {
        let (tcx, value) = self;
        tcx.arena.dropless.alloc(value)
    }
}

impl<'tcx, T: Copy> IntoArena<'tcx, [T]> for (TyCtxt<'tcx>, &[T]) {
    fn into_arena(self) -> &'tcx [T] {
        let (tcx, slice) = self;
        if slice.is_empty() {
            return &[];
        }
        tcx.arena.dropless.alloc_slice(slice)
    }
}

impl<'tcx, T: Copy, const N: usize> IntoArena<'tcx, [T]> for (TyCtxt<'tcx>, &[T; N]) {
    fn into_arena(self) -> &'tcx [T] {
        let (tcx, slice) = self;
        (tcx, &slice[..]).into_arena()
    }
}
