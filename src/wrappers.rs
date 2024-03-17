use core::marker::PhantomData;
use core::mem::MaybeUninit;

use libredox::error::{Error, Result};

use crate::raw;
pub use crate::raw::EventFlags;

pub struct RawEventQueue {
    inner: usize,
}
pub type RawEvent = raw::RawEventV1;
impl RawEventQueue {
    pub fn new() -> Result<Self> {
        Ok(Self {
            inner: Error::demux(unsafe { raw::redox_event_queue_create_v1(0) })?,
        })
    }
    /// Subscribe to events produced by `fd`
    pub fn subscribe(&self, fd: usize, user_data: usize, flags: EventFlags) -> Result<()> {
        let _ = Error::demux(unsafe {
            raw::redox_event_queue_ctl_v1(self.inner, fd, flags.bits(), user_data)
        })?;
        Ok(())
    }
    /// Unsubscribe from events produced by `fd`
    pub fn unsubscribe(&self, fd: usize) -> Result<()> {
        // TODO: Will user_data be needed?
        self.subscribe(fd, 0, EventFlags::empty())
    }
    // TODO: next_events
    pub fn next_event(&self) -> Result<RawEvent> {
        let mut event = MaybeUninit::uninit();

        unsafe {
            let res = Error::demux(raw::redox_event_queue_get_events_v1(
                self.inner,
                event.as_mut_ptr(),
                1,
                0,
                core::ptr::null(),
                core::ptr::null(),
            ))?;
            assert_eq!(res, 1, "EOF is not yet well defined for event queues");
            Ok(event.assume_init())
        }
    }
    // TODO: "next_event_nonblock"?
}
impl Drop for RawEventQueue {
    fn drop(&mut self) {
        unsafe {
            let _ = Error::demux(raw::redox_event_queue_destroy_v1(self.inner));
        }
    }
}
impl Iterator for RawEventQueue {
    type Item = Result<RawEvent>;

    // TODO: next_chunk
    fn next(&mut self) -> Option<Self::Item> {
        Some(self.next_event())
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
    pub fn unsubscribe(&self, fd: usize) -> Result<()> {
        self.inner.unsubscribe(fd)
    }
}
impl<U: UserData> Iterator for EventQueue<U> {
    type Item = Result<Event<U>>;

    // TODO: next_chunk
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|res| {
            res.map(|raw| Event {
                user_data: U::from_user_data(raw.user_data),
                fd: raw.fd,
                flags: EventFlags::from_bits_retain(raw.flags),
            })
        })
    }
}
