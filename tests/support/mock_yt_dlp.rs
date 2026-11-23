// tests/support/mock_yt_dlp.rs
// Mock binary for yt-dlp and gallery-dl execution testing.

use std::env;
use std::thread;
use std::time::Duration;

fn main() {
    let args: Vec<String> = env::args().collect();
    let args_str = args.join(" ");

    // Match based on URL passed in args (isolated per test thread)
    if args_str.contains("mock-error") {
        eprintln!("ERROR: Video unavailable in your country");
        std::process::exit(1);
    } else if args_str.contains("mock-warning") {
        eprintln!("WARNING: Audio stream missing");
        println!("[download] 100% of 20.00MiB");
        std::process::exit(0);
    } else if args_str.contains("mock-hang") {
        // Sleep so cancellation logic can terminate the process
        thread::sleep(Duration::from_secs(15));
        std::process::exit(0);
    } else {
        // Default: success
        println!("[download]   0.0% of ~50.00MiB");
        println!("[download]  50.0% of ~50.00MiB at 10.00MiB/s ETA 00:05");
        println!("[Merger] Merging formats into \"video.mp4\"");
        println!("[download] 100% of 50.00MiB in 00:10 at 5.00MiB/s");
        std::process::exit(0);
    }
}
