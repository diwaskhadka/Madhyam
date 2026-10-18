//! How much heap a catalog takes: `cargo run --release --example memory -- catalog.jsonl [guide.xml ...]`.
//!
//! Loads a snapshot (and guides) with a counting allocator and prints the live heap after each
//! step, the way a TV with little RAM sees it. Build for a 32-bit target to see ARMv7 sizes.

use madhyam_core::Catalog;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, AtomicUsize, Ordering};
use std::time::Instant;

struct Counting;
static LIVE: AtomicIsize = AtomicIsize::new(0);
static PEAK: AtomicIsize = AtomicIsize::new(0);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let n = LIVE.fetch_add(l.size() as isize, Ordering::Relaxed) + l.size() as isize;
        PEAK.fetch_max(n, Ordering::Relaxed);
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        LIVE.fetch_sub(l.size() as isize, Ordering::Relaxed);
        ALLOCS.fetch_sub(1, Ordering::Relaxed);
        unsafe { System.dealloc(p, l) }
    }
}

#[global_allocator]
static A: Counting = Counting;

/// Resident memory of the process (Linux and Android), as the system sees it.
fn rss() -> String {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .map(|l| l[6..].trim().to_string())
        })
        .map(|kb| format!("  rss {kb}"))
        .unwrap_or_default()
}

fn report(step: &str, t: Instant) {
    println!(
        "{step:<28} live {:>7.1} MB  peak {:>7.1} MB  blocks {:>9}  {:>6} ms{}",
        LIVE.load(Ordering::Relaxed) as f64 / 1048576.0,
        PEAK.load(Ordering::Relaxed) as f64 / 1048576.0,
        ALLOCS.load(Ordering::Relaxed),
        t.elapsed().as_millis(),
        rss()
    );
    PEAK.store(LIVE.load(Ordering::Relaxed), Ordering::Relaxed);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(snapshot) = args.first() else {
        eprintln!("usage: memory <catalog.jsonl> [guide.xml ...]");
        std::process::exit(2);
    };
    let t = Instant::now();
    let f = std::io::BufReader::new(std::fs::File::open(snapshot).expect("snapshot"));
    let mut c = Catalog::load_snapshot(f).expect("load");
    report(&format!("snapshot ({} items)", c.len()), t);
    for g in &args[1..] {
        let t = Instant::now();
        let f = std::io::BufReader::new(std::fs::File::open(g).expect("guide"));
        let now = madhyam_core::time::now();
        // As the Android app imports it: a day back, three days ahead.
        let s = c
            .import_xmltv_window(f, now - 86_400, now + 3 * 86_400)
            .expect("guide");
        report(&format!("guide ({} programmes)", s.programmes), t);
    }
    let t = Instant::now();
    let mut out = Vec::new();
    c.save_snapshot(&mut out).expect("save");
    let n = out.len();
    // MADHYAM_SAVE_TO=file keeps the re-saved snapshot, to compare two engine versions.
    if let Ok(path) = std::env::var("MADHYAM_SAVE_TO") {
        std::fs::write(path, &out).expect("write");
    }
    drop(out);
    report(&format!("save ({} MB)", n / 1048576), t);
}
