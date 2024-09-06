use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub address: usize,
    pub file_name: String,
    pub line_no: u32,
    pub col_no: u32,
}

#[derive(Serialize, Deserialize)]
pub struct Block {
    pub size: usize,
    pub frames: Vec<Symbol>,
}

#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
mod alloc {

    use std::{
        alloc::{GlobalAlloc, Layout, System},
        cell::UnsafeCell,
        ffi::{c_void, CStr},
        mem::MaybeUninit,
        ptr::{self, null_mut},
        sync::atomic::{AtomicUsize, Ordering},
    };

    use leveldb_sys::{
        leveldb_close, leveldb_delete, leveldb_open, leveldb_options_create,
        leveldb_options_destroy, leveldb_options_set_create_if_missing, leveldb_put, leveldb_t,
        leveldb_writeoptions_create, leveldb_writeoptions_destroy,
    };

    use crate::helper::{backtrace_lock, helper_println, Reentrancy};

    use super::*;

    fn backtrace() -> Vec<Symbol> {
        let _guard = backtrace_lock();

        let mut symbols = vec![];

        // get frame symbol object.
        unsafe {
            backtrace::trace_unsynchronized(|frame| {
                let addr = frame.symbol_address() as usize;

                let mut proto_symbol = None;

                backtrace::resolve_unsynchronized(addr as *mut c_void, |symbol| {
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

                if let Some(symbol) = proto_symbol {
                    symbols.push(symbol);
                }

                true
            });
        }

        symbols
    }

    pub struct HeapProfiler {
        /// the snapshot database storage.
        db: *mut leveldb_t,
    }

    impl HeapProfiler {
        /// Create a  new `HeapProfiler` instance.
        fn new() -> Option<Self> {
            let db = unsafe {
                let mut error = ptr::null_mut();
                let options = leveldb_options_create();

                leveldb_options_set_create_if_missing(options, 1);

                let db = leveldb_open(options, c"./memory.pprof".as_ptr(), &mut error);

                leveldb_options_destroy(options);

                if error == ptr::null_mut() {
                    db
                } else {
                    helper_println(error);
                    return None;
                }
            };

            Some(Self { db })
        }

        fn register(&self, ptr: *mut u8, layout: Layout) {
            let frames = backtrace();

            let block = Block {
                size: layout.size(),
                frames,
            };

            let value = bson::to_vec(&block).unwrap();

            unsafe {
                let ops = leveldb_writeoptions_create();

                let key = ptr as u64;

                let mut error = null_mut();

                leveldb_put(
                    self.db,
                    ops,
                    key.to_be_bytes().as_ptr() as *const i8,
                    8,
                    value.as_ptr() as *const i8,
                    value.len(),
                    &mut error,
                );

                leveldb_writeoptions_destroy(ops);

                if error != ptr::null_mut() {
                    helper_println(error);
                    panic!("write frame failed.");
                }
            }
        }

        fn unregister(&self, ptr: *mut u8, _layout: Layout) {
            unsafe {
                let ops = leveldb_writeoptions_create();

                let key = ptr as u64;

                let mut error = null_mut();

                leveldb_delete(
                    self.db,
                    ops,
                    key.to_be_bytes().as_ptr() as *const i8,
                    8,
                    &mut error,
                );

                leveldb_writeoptions_destroy(ops);

                if error != ptr::null_mut() {
                    eprintln!("{:?}", CStr::from_ptr(error));
                    panic!("remove frame failed.");
                }
            }
        }
    }

    impl Drop for HeapProfiler {
        fn drop(&mut self) {
            unsafe {
                leveldb_close(self.db);
            }
        }
    }

    pub struct GLobalHeapProfiler {
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

        fn get(&self) -> Option<&HeapProfiler> {
            while self.initialized.load(Ordering::Acquire) < 2 {
                if self
                    .initialized
                    .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Relaxed)
                    .is_ok()
                {
                    let profiler = match HeapProfiler::new() {
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

    fn global_heap_profiler() -> Option<&'static HeapProfiler> {
        static PROFILER: GLobalHeapProfiler = GLobalHeapProfiler::new();

        PROFILER.get()
    }

    pub struct PprofAlloc;

    unsafe impl GlobalAlloc for PprofAlloc {
        #[inline]
        unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
            let ptr = System.alloc(layout);

            let guard = Reentrancy::new();

            if !guard.is_ok() {
                return ptr;
            }

            if let Some(profiler) = global_heap_profiler() {
                profiler.register(ptr, layout);
            }

            ptr
        }

        #[inline]
        unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
            let guard = Reentrancy::new();

            if guard.is_ok() {
                if let Some(profiler) = global_heap_profiler() {
                    profiler.unregister(ptr, layout);
                }
            }

            System.dealloc(ptr, layout);
        }
    }
}

#[cfg(feature = "alloc")]
pub use alloc::*;
