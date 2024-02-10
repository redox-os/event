#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct RawEventV1 {
    // NOTE: This field will likely be removed soon (v2), as user_data can already uniquely
    // identify the origin of any event.
    pub fd: usize,

    pub user_data: usize,
    pub flags: u32,
}

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
    pub struct EventQueueCreateFlagsV1: usize {
        const NONE = 0;
    }
    #[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
    pub struct EventQueueGetEventsFlagsV1: usize {
        const NONE = 0;
        // TODO: const NONBLOCK = 1;
        // TODO? const RESTART = 2;
    }
}
type RawResult = usize;
extern "C" {
    pub fn redox_event_queue_create_v1(flags: u32) -> RawResult;

    pub fn redox_event_queue_get_events_v1(
        queue: usize,
        buf: *mut RawEventV1,
        buf_count: usize,
        flags: u32,
        timeout: *const libredox::data::TimeSpec,
        sigset: *const libredox::data::SigSet,
    ) -> RawResult;
    pub fn redox_event_queue_ctl_v1(
        queue: usize,
        fd: usize,
        flags: u32,
        user_data: usize,
    ) -> RawResult;

    // An event queue is currently simply a file descriptor. It would need some new flag to be
    // allowed not to be one, but keep it opaque anyway, as this will be called from a library.
    pub fn redox_event_queue_destroy_v1(queue: usize) -> RawResult;
}
bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
    pub struct EventFlags: u32 {
        const READ = 1;
        const WRITE = 2;
    }
}
