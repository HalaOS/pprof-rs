use hala_pprof_memory::{report::snapshot, PprofAlloc};

#[global_allocator]
static ALLOC: PprofAlloc = PprofAlloc;

#[test]
fn alloc_string() {
    for i in 0..1000 {
        _ = format!("hello world {}", "===");

        if i == 50 {
            snapshot();
        }
    }

    snapshot();
}
