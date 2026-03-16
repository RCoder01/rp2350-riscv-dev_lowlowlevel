#!/usr/bin/env -S cargo +nightly -Zscript --quiet --config build.target=\"x86_64-unknown-linux-gnu\"
---cargo
[package]
edition = "2024"

[profile.dev]
opt-level = 3

[dependencies]
clap = { version = "4.5.60", features = ["derive"] }
anyhow = "1.0"
---
use anyhow::{Context, Result};
use clap::Parser;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;

/// Read a file, pad to multiple of 8 bytes with zeros, append a fixed trailer,
/// then pad to a multiple of 256 bytes with zeros, writing output to a file.
#[derive(Parser)]
struct Args {
    /// Input file path
    #[arg(short, long)]
    input: PathBuf,

    /// Output file path (if omitted, overwrites input)
    #[arg(short, long)]
    output: Option<PathBuf>,
}

fn pad_to_multiple(buf: &mut Vec<u8>, multiple: usize) {
    let rem = buf.len() % multiple;
    if rem != 0 {
        buf.extend(std::iter::repeat(0).take(multiple - rem));
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    let out_path = args.output.unwrap_or_else(|| args.input.clone());

    let mut input_file = File::open(&args.input)
        .with_context(|| format!("Failed to open input file {:?}", args.input))?;
    let mut data = Vec::new();
    input_file
        .read_to_end(&mut data)
        .with_context(|| "Failed to read input file")?;

    pad_to_multiple(&mut data, 8);
    let image_def: [u8; 20] = [
        0xD3, 0xDE, 0xFF, 0xFF, 0x42, 0x01, 0x01, 0x11, 0xFF, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x79, 0x35, 0x12, 0xAB,
    ];
    data.extend_from_slice(&image_def);

    pad_to_multiple(&mut data, 256);

    let tmp_path = out_path.with_extension("tmp");
    {
        let mut tmp = File::create(&tmp_path)
            .with_context(|| format!("Failed to create temp file {:?}", tmp_path))?;
        tmp.write_all(&data)
            .with_context(|| "Failed to write output data")?;
        tmp.flush().with_context(|| "Failed to flush temp file")?;
    }
    std::fs::rename(&tmp_path, &out_path)
        .with_context(|| format!("Failed to rename temp file to {:?}", out_path))?;

    println!("Wrote {} bytes to {:?}", data.len(), out_path);
    Ok(())
}
