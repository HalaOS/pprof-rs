#![cfg_attr(docsrs, feature(doc_cfg))]

mod helper;
mod proto;

mod profiler;
pub use profiler::*;

#[cfg(feature = "report")]
#[cfg_attr(docsrs, doc(cfg(feature = "report")))]
pub mod report;
