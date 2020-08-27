extern crate syscall;

use syscall::{
    Event as SysEvent,
    EventFlags,
};

use std::{
    collections::BTreeMap,
    fs::File,
    io::{
        prelude::*,
        Result as IOResult,
        Error as IOError
    },
    os::unix::io::{AsRawFd, RawFd},
    slice,
    mem
};

#[derive(Debug, Clone, Copy)]
pub struct Event {
    pub fd: RawFd,
    pub flags: EventFlags,
}

/// Subscribe `EvenQueue` to events produced by `fd`.
pub fn subscribe_to_fd(onto: RawFd, fd: RawFd, id: usize) -> IOResult<()> {
    syscall::write(onto as usize, &SysEvent {
        id: fd as usize,
        flags: syscall::EVENT_READ,
        data: id
    })
    .map_err(|x| IOError::from_raw_os_error(x.errno))
    .map(|_| ())
}

/// Unsubscribe `EvenQueue` from events produced by `fd`.
pub fn unsubscribe_from_fd(onto: RawFd, fd: RawFd, id: usize) -> IOResult<()> {
    syscall::write(onto as usize, &SysEvent {
        id: fd as usize,
        flags: syscall::EventFlags::empty(),
        data: id
    })
    .map_err(|x| IOError::from_raw_os_error(x.errno))
    .map(|_| ())
}


pub struct EventQueue<R, E = IOError> {
    /// The file to read events from
    pub file: File,
    /// A map of registered file descriptors to their handler callbacks
    callbacks: BTreeMap<usize, (RawFd, Box<dyn FnMut(Event) -> Result<Option<R>, E>>)>,
    /// An ID counter, ensuring each registered fd gets a unique ID
    /// (which means the same fd can be registered multiple times)
    next_id: usize,
    /// The default callback to call for not-registered FD
    default_callback: Option<Box<dyn FnMut(Event) -> Result<Option<R>, E>>>,
}

impl<R, E> EventQueue<R, E>
where
    E: From<IOError>,
{
    /// Create a new event queue
    pub fn new() -> IOResult<EventQueue<R, E>> {
        Ok(EventQueue {
            file: File::open("event:")?,
            callbacks: BTreeMap::new(),
            next_id: 0,
            default_callback: None,
        })
    }

    /// Set the default callback to be called if an event is produced
    /// by a FD not registered with `add`.
    pub fn set_default_callback<F>(&mut self, callback: F)
    where
        F: FnMut(Event) -> Result<Option<R>, E> + 'static,
    {
        self.default_callback = Some(Box::new(callback));
    }

    /// Add a file to the event queue, calling a callback when an event occurs.
    /// Returns the event id it got, which can be used to remove or trigger this event.
    ///
    /// The callback returns Ok(None) if it wishes to continue the event loop,
    /// or Ok(Some(R)) to break the event loop and return the value.
    /// Err can be used to allow the callback to return an error, and break the
    /// event loop.
    pub fn add<F: FnMut(Event) -> Result<Option<R>, E> + 'static>(
        &mut self,
        fd: RawFd,
        callback: F,
    ) -> IOResult<usize> {
        self.next_id += 1;

        self.callbacks.insert(self.next_id, (fd, Box::new(callback)));
        subscribe_to_fd(self.file.as_raw_fd(), fd, self.next_id)?;

        Ok(self.next_id)
    }

    /// Remove a file from the event queue, returning its callback if found
    pub fn remove(
        &mut self,
        id: usize
    ) -> IOResult<Option<Box<dyn FnMut(Event) -> Result<Option<R>, E>>>> {
        if let Some((fd, callback)) = self.callbacks.remove(&id) {
            unsubscribe_from_fd(self.file.as_raw_fd(), fd, id)?;
            Ok(Some(callback))
        } else {
            Ok(None)
        }
    }

    /// Send an event to a descriptor callback
    pub fn trigger(&mut self, id: usize, event: Event) -> Result<Option<R>, E> {
        if let Some((_fd, callback)) = self.callbacks.get_mut(&id) {
            callback(event)
        } else if let Some(ref mut callback) = self.default_callback {
            callback(event)
        } else {
            Ok(None)
        }
    }

    /// Send an event to all descriptor callbacks, useful for cleaning out buffers after init
    pub fn trigger_all(&mut self, event: Event) -> Result<Vec<R>, E> {
        let mut rets = Vec::new();
        for &mut (_fd, ref mut callback) in self.callbacks.values_mut() {
            if let Some(ret) = callback(event)? {
                rets.push(ret);
            }
        }
        Ok(rets)
    }

    /// Process the event queue until a callback returns Some(R)
    pub fn run(&mut self) -> Result<R, E> {
        loop {
            let mut events = [SysEvent::default(); 16];
            let mut events_buf = unsafe {
                slice::from_raw_parts_mut(
                    events.as_mut_ptr() as *mut u8,
                    events.len() * mem::size_of::<SysEvent>()
                )
            };
            let n = self.file.read(&mut events_buf).map_err(E::from)? / mem::size_of::<SysEvent>();
            for sysevent in &events[..n] {
                let event = Event {
                    fd: sysevent.id as RawFd,
                    flags: sysevent.flags,
                };
                if let Some(ret) = self.trigger(sysevent.data, event)? {
                    return Ok(ret);
                }
            }
        }
    }
}
