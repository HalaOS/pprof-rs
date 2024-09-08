//! An implementation of [`GlobalAlloc`] that supports memory profiling
//! and whose output reports are compatible with [`pprof`].
//!
//! The below code enables memory profiling in rust programs.
//!
//! ```no_run
//! use hala_pprof_memory::PprofAlloc;
//!
//! #[global_allocator]
//! static ALLOC: PprofAlloc = PprofAlloc(10);
//! ```
//!
//! `PprofAlloc` does not automatically generate memory profiling reports,
//! the developers need to manually call [`snapshot`] function to generate them.
//!
//! [`GlobalAlloc`]: std::alloc::GlobalAlloc
//! [`pprof`]: https://github.com/google/pprof
//!
//! ```no_run
//! use hala_pprof_memory::{PprofAlloc,snapshot};
//!
//! #[global_allocator]
//! static ALLOC: PprofAlloc = PprofAlloc(10);
//!
//! fn main() {
//!     loop {
//!         // working...
//!         // generate report.
//!         snapshot();
//!     }
//! }
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]

mod helper;
mod proto;

mod profiler;
pub use profiler::*;

#[cfg(feature = "report")]
#[cfg_attr(docsrs, doc(cfg(feature = "report")))]
mod report;

#[cfg(feature = "report")]
pub use report::*;
