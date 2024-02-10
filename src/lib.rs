#![no_std]

#[cfg(feature = "wrappers")]
pub extern crate libredox;

#[cfg(feature = "redox_syscall")]
mod sc {
    use crate::raw::EventFlags;

    const _: () = {
        if EventFlags::READ.bits() as usize != syscall::EventFlags::EVENT_READ.bits() {
            panic!();
        }
        if EventFlags::WRITE.bits() as usize != syscall::EventFlags::EVENT_WRITE.bits() {
            panic!();
        }
    };
    impl From<syscall::flag::EventFlags> for EventFlags {
        fn from(value: syscall::flag::EventFlags) -> Self {
            let mut this = Self::empty();
            this.set(Self::READ, value.contains(syscall::EventFlags::EVENT_READ));
            this.set(
                Self::WRITE,
                value.contains(syscall::EventFlags::EVENT_WRITE),
            );
            this
        }
    }
    impl From<EventFlags> for syscall::flag::EventFlags {
        fn from(value: EventFlags) -> Self {
            let mut this = Self::empty();
            this.set(Self::EVENT_READ, value.contains(EventFlags::READ));
            this.set(Self::EVENT_WRITE, value.contains(EventFlags::WRITE));
            this
        }
    }
}
pub mod raw;

#[cfg(feature = "wrappers")]
mod wrappers;
#[cfg(feature = "wrappers")]
pub use wrappers::*;
