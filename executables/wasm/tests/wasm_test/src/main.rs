use std::io::{Read, Write};

#[unsafe(export_name = "gcd")]
pub fn gcd(mut a: i32, mut b: i32) -> i32 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

fn test_stdio() -> Result<(), ()> {
    println!("Test stdout");
    eprintln!("Test stderr");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    println!("Input: {}", input);
    Ok(())
}

fn test_file() {
    println!("Testing file operations...");
    {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open("/test.txt")
            .unwrap();
        file.write_all(b"Hello World from WASM!").unwrap();
    }
    {
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .open("/test.txt")
            .unwrap();
        let mut string = String::new();
        file.read_to_string(&mut string).unwrap();
        assert_eq!(string, "Hello World from WASM!");
    }
}

fn test_environment_variables() {
    println!("Environment variables:");
    for (key, value) in std::env::vars() {
        println!("{}: {}", key, value);
    }
}

fn test_directory() {
    println!("Testing directory operations...");
    for entry in std::fs::read_dir("/").unwrap() {
        let entry = entry.unwrap();
        let kind = entry.file_type().unwrap();
        let kind = if kind.is_dir() {
            "Directory"
        } else if kind.is_file() {
            "File"
        } else {
            "Other"
        };
        println!("{:?} - {}", entry.file_name(), kind);
    }
}

fn main() -> Result<(), ()> {
    test_stdio()?;
    test_file();
    test_directory();
    test_environment_variables();
    Ok(())
}
