#![no_std]

use core::marker::PhantomData;
use core::mem::size_of;

use libredox::{Fd, flag, errno};
pub use libredox::error::{self, Result};

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
    pub struct EventFlags: usize {
        const READ = 1;
        const WRITE = 2;
    }
}
const _: () = {
    if EventFlags::READ.bits() != syscall::EventFlags::EVENT_READ.bits() {
        panic!();
    }
    if EventFlags::WRITE.bits() != syscall::EventFlags::EVENT_WRITE.bits() {
        panic!();
    }
};
impl From<syscall::flag::EventFlags> for EventFlags {
    fn from(value: syscall::flag::EventFlags) -> Self {
        let mut this = Self::empty();
        this.set(Self::READ, value.contains(syscall::EventFlags::EVENT_READ));
        this.set(Self::WRITE, value.contains(syscall::EventFlags::EVENT_WRITE));
        this
    }
}
impl From<EventFlags> for syscall::flag::EventFlags {
    fn from(value: EventFlags) -> Self {
        let mut this = Self::empty();
        this.set(Self::EVENT_READ, value.contains(EventFlags::READ));
        this.set(Self::EVENT_WRITE, value.contains(EventFlags::READ));
        this
    }
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
pub struct RawEvent {
    pub fd: usize,
    pub flags: EventFlags,
    pub user_data: usize,
}

pub struct RawEventQueue {
    inner: Fd,
}
impl RawEventQueue {
    pub fn new() -> Result<Self> {
        Ok(Self {
            inner: Fd::open("event:", flag::O_CREAT | flag::O_RDWR | flag::O_CLOEXEC, 0o700)?,
        })
    }
    /// Subscribe to events produced by `fd`
    pub fn subscribe(&self, fd: usize, user_data: usize, flags: EventFlags) -> Result<()> {
        self.inner.write(&syscall::data::Event {
            id: fd,
            data: user_data,
            flags: flags.into(),
        })?;
        Ok(())
    }
    /// Unsubscribe from events produced by `fd`
    pub fn unsubscribe(&self, fd: usize, flags: EventFlags) -> Result<()> {
        self.inner.write(&syscall::data::Event {
            id: fd,
            flags: flags.into(),
            data: 0,
        })?;
        Ok(())
    }
}
impl Iterator for RawEventQueue {
    type Item = Result<RawEvent>;

    // TODO: next_chunk
    fn next(&mut self) -> Option<Self::Item> {
        let mut event = syscall::data::Event::default();

        loop {
            match self.inner.read(&mut event) {
                Ok(0) => return None,
                Ok(n) => {
                    debug_assert_eq!(n, size_of::<syscall::data::Event>());
                    return Some(Ok(RawEvent {
                        fd: event.id,
                        flags: event.flags.into(),
                        user_data: event.data,
                    }));
                }
                Err(error::Error { errno: errno::EINTR }) => continue,
                Err(err) => return Some(Err(err)),
            }
        }
    }
}

#[macro_export]
macro_rules! user_data {
    {
        $vis:vis enum $name:ident {
            $($variant:ident),*$(,)?
        }
    } => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[repr(usize)]
        $vis enum $name {
            $($variant),*
        }

        impl $crate::UserData for $name {
            fn into_user_data(self) -> usize {
                self as usize
            }
            fn from_user_data(raw: usize) -> Self {
                assert!(raw < [$(Self::$variant),*].len());
                //assert!(raw < core::mem::variant_count::<$name>());

                unsafe { ::core::mem::transmute(raw) }
            }
        }
    };
}

pub trait UserData: Clone + Copy {
    fn into_user_data(self) -> usize;
    fn from_user_data(user_data: usize) -> Self;
}
impl UserData for usize {
    fn into_user_data(self) -> usize {
        self
    }
    // TODO: make unsafe fn
    fn from_user_data(user_data: usize) -> Self {
        user_data
    }
}
/*
unsafe impl<'a, T> UserData for &'a T {
    fn into_user_data(self) -> usize {
        self as *const T as usize
    }
    unsafe fn from_user_data(user_data: usize) -> Self {
        &*(user_data as *const T)
    }
}
*/

#[non_exhaustive]
pub struct Event<U: UserData> {
    pub user_data: U,
    pub flags: EventFlags,
    pub fd: usize,
}

pub struct EventQueue<U: UserData> {
    inner: RawEventQueue,

    // We'll be casting user_data to and from U, so ensure it's invariant.
    _marker: PhantomData<*mut U>,
}

impl<U: UserData> EventQueue<U> {
    /// Create a new event queue
    pub fn new() -> Result<Self> {
        Ok(EventQueue {
            inner: RawEventQueue::new()?,
            _marker: PhantomData,
        })
    }
    pub fn subscribe(&self, fd: usize, data: U, flags: EventFlags) -> Result<()> {
        self.inner.subscribe(fd, data.into_user_data(), flags)
    }
    pub fn unsubscribe(&self, fd: usize, flags: EventFlags) -> Result<()> {
        self.inner.unsubscribe(fd, flags)
    }
}
impl<U: UserData> Iterator for EventQueue<U> {
    type Item = Result<Event<U>>;

    // TODO: next_chunk
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|res| res.map(|raw| Event {
            user_data: U::from_user_data(raw.user_data),
            fd: raw.fd,
            flags: raw.flags,
        }))
    }
}
