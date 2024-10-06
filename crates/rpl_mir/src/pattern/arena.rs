use rustc_arena::DroplessArena;
use rustc_span::Symbol;

pub trait AllocInArena<'tcx, T: ?Sized> {
    fn alloc_in_arena(self) -> &'tcx T;
}

impl<'tcx, T: ?Sized> AllocInArena<'tcx, T> for &'tcx T {
    fn alloc_in_arena(self) -> &'tcx T {
        self
    }
}

impl<'tcx, T, const N: usize> AllocInArena<'tcx, [T]> for &'tcx [T; N] {
    fn alloc_in_arena(self) -> &'tcx [T] {
        self
    }
}

impl<'tcx> AllocInArena<'tcx, [Symbol]> for (&'tcx DroplessArena, &[&str]) {
    fn alloc_in_arena(self) -> &'tcx [Symbol] {
        let (arena, syms) = self;
        arena.alloc_from_iter(syms.iter().map(|sym| Symbol::intern(sym)))
    }
}

impl<'tcx, const N: usize> AllocInArena<'tcx, [Symbol]> for (&'tcx DroplessArena, &[&str; N]) {
    fn alloc_in_arena(self) -> &'tcx [Symbol] {
        let (arena, syms) = self;
        (arena, &syms[..]).alloc_in_arena()
    }
}

impl<'tcx, T: Copy> AllocInArena<'tcx, T> for (&'tcx DroplessArena, T) {
    fn alloc_in_arena(self) -> &'tcx T {
        let (arena, value) = self;
        arena.alloc(value)
    }
}

impl<'tcx, T: Copy> AllocInArena<'tcx, [T]> for (&'tcx DroplessArena, &[T]) {
    fn alloc_in_arena(self) -> &'tcx [T] {
        let (arena, slice) = self;
        if slice.is_empty() {
            return &[];
        }
        arena.alloc_slice(slice)
    }
}

impl<'tcx, T: Copy, const N: usize> AllocInArena<'tcx, [T]> for (&'tcx DroplessArena, &[T; N]) {
    fn alloc_in_arena(self) -> &'tcx [T] {
        let (arena, slice) = self;
        (arena, &slice[..]).alloc_in_arena()
    }
}
