use std::{
    cell::{Cell, UnsafeCell},
    fmt,
    mem::MaybeUninit,
};

/// A `Lazy<T>` is a single-threaded lazy initialised container.
///
/// It can be initialised by calling `get_or_create` with a function which will provide the
/// lazily computed value.
///
/// ```compile_fail,E0515
/// # fn test() {
/// #     let _: impl Sync = Lazy::<i32>::new();
/// # }
/// ```
pub struct Lazy<T> {
    // SAFETY (racing): we're !Sync so only a single thread can do this at a time
    t: UnsafeCell<MaybeUninit<T>>,
    init: Cell<bool>, // also prevents Sync + Send
}

impl<T> Lazy<T> {
    pub const fn new() -> Self {
        Self {
            t: UnsafeCell::new(MaybeUninit::uninit()),
            init: Cell::new(false),
        }
    }

    pub fn into_inner(self) -> Option<T> {
        if self.init.get() {
            // SAFETY (initialisation): we've just checked self.init
            // SAFETY (mutability): r/o operations only here
            unsafe { Some(self.minit().assume_init_read()) }
        } else {
            None
        }
    }

    pub fn get(&self) -> Option<&T> {
        if self.init.get() {
            // SAFETY (initialisation): we've just checked self.init
            // SAFETY (mutability): r/o operations only here
            unsafe { Some(self.minit().assume_init_ref()) }
        } else {
            None
        }
    }

    unsafe fn minit(&self) -> &mut MaybeUninit<T> {
        &mut *self.t.get()
    }

    pub fn get_or_create<F>(&self, f: F) -> &T
    where
        F: FnOnce() -> T,
    {
        if !self.init.get() {
            let t = f();

            if self.init.get() {
                // f() modified self
                panic!("recursive modification of Lazy<T>");
            }

            // SAFETY (initialisation): we're uninitialised from the self.init check
            // SAFETY (mutability): no possibility of other mutable references (&self)
            //                      and other shared references can't see the change
            //                      because we're single threaded
            unsafe {
                self.minit().write(t);
            }

            self.init.set(true);
        }

        unsafe { self.minit().assume_init_ref() }
    }
}

impl<T> fmt::Debug for Lazy<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Lazy({:?})", self.get())
    }
}

impl<T: Clone> Clone for Lazy<T> {
    fn clone(&self) -> Self {
        match self.get() {
            Some(t) => Self {
                t: UnsafeCell::new(MaybeUninit::new(t.clone())),
                init: Cell::new(true),
            },
            None => Self::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    struct NoCopy(i32);

    #[test]
    fn get_or_create() {
        let l = Lazy::new();
        assert!(l.get().is_none());

        let got = l.get_or_create(|| NoCopy(3));
        assert_eq!(got, &NoCopy(3));
    }

    #[test]
    fn double_drop() {
        let l = Lazy::new();

        l.get_or_create(|| String::from("hi"));
        let l2 = l.clone();
        drop(l);
        drop(l2);
    }

    #[test]
    #[should_panic]
    fn recursive_init() {
        let l = Lazy::new();

        l.get_or_create(|| {
            l.get_or_create(|| 0);
            0
        });
    }
}
