use hala_pprof_memory::{snapshot, PprofAlloc};

#[global_allocator]
static ALLOC: PprofAlloc = PprofAlloc(10);

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
