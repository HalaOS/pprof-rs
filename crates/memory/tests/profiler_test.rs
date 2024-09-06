use hala_pprof_memory::PprofAlloc;

#[global_allocator]
static ALLOC: PprofAlloc = PprofAlloc;

#[test]
fn alloc_string() {
    for _ in 0..1000 {
        _ = format!("hello world {}", "===");
    }
}
