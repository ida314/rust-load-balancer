// examples/chaos_test.rs
use tokio::time::{sleep, Duration};
use rand::Rng;

#[tokio::main]
async fn main() {
    // Randomly kill and restart backends
    loop {
        let port = rand::thread_rng().gen_range(8001..=8003);
        println!("Killing backend {}", port);
        
        // Kill backend
        std::process::Command::new("kill")
            .args(&["-9", &format!("{}", get_pid_for_port(port))])
            .output()
            .expect("Failed to kill process");
        
        // Random sleep
        sleep(Duration::from_secs(rand::thread_rng().gen_range(5..30))).await;
        
        // Restart backend
        println!("Restarting backend {}", port);
        std::process::Command::new("cargo")
            .args(&["run", "--example", "test_backend", "--", &port.to_string()])
            .spawn()
            .expect("Failed to start backend");
    }
}