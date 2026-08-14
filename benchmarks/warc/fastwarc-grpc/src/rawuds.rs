// Raw UDS throughput floor: sender reads the file and writes it to a Unix
// socket; receiver reads and discards. No framing, no serde, no parse.
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args[1].clone();
    let chunk: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(64 << 10);
    let sock = std::env::temp_dir().join(format!("rawuds-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&sock);
    let listener = UnixListener::bind(&sock).unwrap();

    let sock2 = sock.clone();
    let sender = std::thread::spawn(move || {
        let mut file = std::fs::File::open(&path).unwrap();
        let mut conn = UnixStream::connect(&sock2).unwrap();
        let mut buf = vec![0u8; chunk];
        loop {
            match file.read(&mut buf).unwrap() {
                0 => break,
                n => conn.write_all(&buf[..n]).unwrap(),
            }
        }
    });

    let (mut conn, _) = listener.accept().unwrap();
    let start = Instant::now();
    let mut buf = vec![0u8; chunk];
    let mut total = 0u64;
    loop {
        match conn.read(&mut buf).unwrap() {
            0 => break,
            n => total += n as u64,
        }
    }
    let secs = start.elapsed().as_secs_f64();
    sender.join().unwrap();
    let _ = std::fs::remove_file(&sock);
    println!(
        "raw UDS {} KiB writes: {:.1} MiB/s ({:.1} MiB in {:.2}s)",
        chunk / 1024,
        total as f64 / secs / 1024.0 / 1024.0,
        total as f64 / 1024.0 / 1024.0,
        secs
    );
}
