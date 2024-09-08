use serde::{Deserialize, Serialize};

use crate::helper::{backtrace_lock, Reentrancy};

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::UnsafeCell,
    collections::HashMap,
    ffi::c_void,
    mem::MaybeUninit,
    ptr::null_mut,
    sync::atomic::{AtomicUsize, Ordering},
};

#[derive(Serialize, Deserialize)]
pub(crate) struct Symbol {
    pub name: String,
    pub address: usize,
    pub file_name: String,
    pub line_no: u32,
    pub col_no: u32,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Block {
    pub size: usize,
    pub frames: Vec<usize>,
}

/// Call this fn to get callstack, not via [`backtrace::trace`].
///
/// The [``backtrace``] standard api, which uses thread-local keys, may not use in GlobalAlloc.
#[allow(unused)]
pub(super) fn get_backtrace(max_frames: usize) -> Vec<usize> {
    let mut stack = vec![];

    let mut skips = 0;

    unsafe {
        backtrace::trace_unsynchronized(|frame| {
            if skips < 4 {
                skips += 1;
                return true;
            }

            stack.push(frame.symbol_address() as usize);

            if stack.len() >= max_frames {
                false
            } else {
                true
            }
        });
    }

    stack
}

/// Call this fn to convert frame symbol address to frame symbol, not via [`backtrace::resolve`]
///
/// The [``backtrace``] standard api, which uses thread-local keys, may not use in GlobalAlloc.
#[allow(unused)]
pub(super) fn frames_to_symbols(frames: &[usize]) -> Vec<Symbol> {
    let mut symbols = vec![];

    for addr in frames {
        let mut proto_symbol = None;

        // get frame symbol object.
        unsafe {
            // Safety: we provide frame to resolve symbol.
            // let _guard = backtrace_lock();

            backtrace::resolve_unsynchronized((*addr) as *mut c_void, |symbol| {
                if proto_symbol.is_none() {
                    proto_symbol = Some(Symbol {
                        name: symbol.name().map(|s| s.to_string()).unwrap_or_default(),
                        address: symbol.addr().unwrap_or(null_mut()) as usize,
                        file_name: symbol
                            .filename()
                            .map(|path| path.to_str().unwrap().to_string())
                            .unwrap_or_default(),
                        line_no: symbol.lineno().unwrap_or_default(),
                        col_no: symbol.colno().unwrap_or_default(),
                    });
                }
            });
        };

        if let Some(symbol) = proto_symbol {
            symbols.push(symbol);
        }
    }

    symbols
}

pub(crate) struct HeapProfiler {
    max_frames: usize,
    blocks: UnsafeCell<HashMap<usize, Block>>,
}

impl HeapProfiler {
    /// Create a  new `HeapProfiler` instance.
    fn new(max_frames: usize) -> Option<Self> {
        Some(Self {
            max_frames,
            blocks: Default::default(),
        })
    }

    fn register(&self, ptr: *mut u8, layout: Layout) {
        let _locker = backtrace_lock();

        let frames = get_backtrace(self.max_frames);

        let block = Block {
            size: layout.size(),
            frames,
        };

        unsafe { &mut *self.blocks.get() }.insert(ptr as usize, block);
    }

    fn unregister(&self, ptr: *mut u8, _layout: Layout) {
        let _locker = backtrace_lock();

        unsafe { &mut *self.blocks.get() }.remove(&(ptr as usize));
    }

    #[cfg(feature = "report")]
    pub fn report(&self) -> crate::proto::gperf::Profile {
        let _locker = backtrace_lock();

        use crate::report::GperfHeapProfilerReport;

        let mut reporter = GperfHeapProfilerReport::new();

        for (ptr, block) in unsafe { &mut *self.blocks.get() }.iter() {
            reporter.report_block_info(
                (*ptr) as *mut u8,
                block.size,
                &frames_to_symbols(&block.frames),
            );
        }

        reporter.build()
    }
}

pub(crate) struct GLobalHeapProfiler {
    initialized: AtomicUsize,
    profiler: UnsafeCell<MaybeUninit<HeapProfiler>>,
}

impl GLobalHeapProfiler {
    const fn new() -> Self {
        GLobalHeapProfiler {
            initialized: AtomicUsize::new(0),
            profiler: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    fn get(&self, max_frames: usize) -> Option<&HeapProfiler> {
        while self.initialized.load(Ordering::Acquire) < 2 {
            if self
                .initialized
                .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                let profiler = match HeapProfiler::new(max_frames) {
                    Some(profiler) => profiler,
                    None => {
                        assert!(self
                            .initialized
                            .compare_exchange(1, 3, Ordering::AcqRel, Ordering::Relaxed)
                            .is_ok());
                        return None;
                    }
                };

                unsafe { (&mut *self.profiler.get()).write(profiler) };

                self.initialized.fetch_add(1, Ordering::Release);
            }
        }

        if self.initialized.load(Ordering::Acquire) == 2 {
            Some(unsafe { (&*self.profiler.get()).assume_init_ref() })
        } else {
            None
        }
    }
}

unsafe impl Sync for GLobalHeapProfiler {}
unsafe impl Send for GLobalHeapProfiler {}

pub(crate) fn global_heap_profiler(max_frames: usize) -> Option<&'static HeapProfiler> {
    static PROFILER: GLobalHeapProfiler = GLobalHeapProfiler::new();

    PROFILER.get(max_frames)
}

/// An implementation of [`GlobalAlloc`] that supports memory profiling.
pub struct PprofAlloc(pub usize);

unsafe impl GlobalAlloc for PprofAlloc {
    #[inline]
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        let ptr = System.alloc(layout);

        let guard = Reentrancy::new();

        if !guard.is_ok() {
            return ptr;
        }

        if let Some(profiler) = global_heap_profiler(self.0) {
            profiler.register(ptr, layout);
        }

        ptr
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        System.dealloc(ptr, layout);

        let guard = Reentrancy::new();

        if guard.is_ok() {
            if let Some(profiler) = global_heap_profiler(self.0) {
                profiler.unregister(ptr, layout);
            }
        }
    }
}
