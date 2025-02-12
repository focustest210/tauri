// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0, MIT

//! Tauri CI Benchmark Tool
//!
//! This binary runs in a CI environment, collecting performance metrics for Tauri.
//! See [benchmark_results](https://github.com/tauri-apps/benchmark_results) for details.
//!
//! ***_Internal use only_***

#![doc(
  html_logo_url = "https://github.com/tauri-apps/tauri/raw/dev/.github/icon.png",
  html_favicon_url = "https://github.com/tauri-apps/tauri/raw/dev/.github/icon.png"
)]

use anyhow::{Context, Result};
use log::{error, info, warn};
use std::{
  collections::{HashMap, HashSet},
  env,
  fs,
  path::{Path, PathBuf},
  process::{Command, Stdio},
  thread,
  time::Duration,
};

mod utils;

const BENCHMARKS: &[(&str, &str)] = &[
  ("tauri_hello_world", "bench_helloworld"),
  ("tauri_cpu_intensive", "bench_cpu_intensive"),
  ("tauri_3mb_transfer", "bench_files_transfer"),
];

fn get_all_benchmarks() -> Vec<(String, String)> {
  BENCHMARKS
    .iter()
    .map(|(name, exe)| {
      (
        name.to_string(),
        format!("../target/{}/release/{}", utils::get_target(), exe),
      )
    })
    .collect()
}

fn run_strace_benchmarks(new_data: &mut utils::BenchResult) -> Result<()> {
  use std::io::Read;

  let mut thread_count = HashMap::new();
  let mut syscall_count = HashMap::new();

  for (name, example_exe) in get_all_benchmarks() {
    let mut file = tempfile::NamedTempFile::new()
      .context("Failed to create temp file for strace output")?;

    Command::new("strace")
      .args(["-c", "-f", "-o", file.path().to_str().unwrap(), &example_exe])
      .stdout(Stdio::inherit())
      .spawn()
      .context("Failed to spawn strace process")?
      .wait()?;

    let mut output = String::new();
    file.as_file_mut().read_to_string(&mut output)?;

    let strace_result = utils::parse_strace_output(&output);
    let clone_count = 1 + strace_result.get("clone").map_or(0, |d| d.calls)
      + strace_result.get("clone3").map_or(0, |d| d.calls);
    let total_syscalls = strace_result.get("total").map_or(0, |d| d.calls);

    thread_count.insert(name.clone(), clone_count);
    syscall_count.insert(name, total_syscalls);
  }

  new_data.thread_count = thread_count;
  new_data.syscall_count = syscall_count;

  Ok(())
}

fn run_max_mem_benchmark() -> Result<HashMap<String, u64>> {
  let mut results = HashMap::new();

  for (name, example_exe) in get_all_benchmarks() {
    let benchmark_file = utils::target_dir().join(format!("mprof_{}.dat", name));

    Command::new("mprof")
      .args(["run", "-C", "-o", benchmark_file.to_str().unwrap(), &example_exe])
      .stdout(Stdio::null())
      .stderr(Stdio::piped())
      .spawn()
      .context("Failed to run mprof benchmark")?
      .wait()?;

    results.insert(
      name,
      utils::parse_max_mem(benchmark_file.to_str().unwrap()).unwrap_or(0),
    );
  }

  Ok(results)
}

fn rlib_size(target_dir: &Path, prefix: &str) -> u64 {
  fs::read_dir(target_dir.join("deps"))
    .unwrap_or_else(|_| panic!("Failed to read target directory: {:?}", target_dir))
    .filter_map(|e| e.ok())
    .filter(|e| e.file_name().to_string_lossy().starts_with(prefix) && e.file_name().to_string_lossy().ends_with(".rlib"))
    .map(|e| e.metadata().unwrap().len())
    .sum()
}

fn get_binary_sizes(target_dir: &Path) -> Result<HashMap<String, u64>> {
  let mut sizes = HashMap::new();
  sizes.insert("wry_rlib".to_string(), rlib_size(target_dir, "libwry"));

  for (name, example_exe) in get_all_benchmarks() {
    let meta = fs::metadata(&example_exe)?;
    sizes.insert(name, meta.len());
  }

  Ok(sizes)
}

fn get_cargo_deps() -> HashMap<String, usize> {
  let targets = [
    ("Windows", &["x86_64-pc-windows-msvc"]),
    ("Linux", &["x86_64-unknown-linux-gnu"]),
    ("macOS", &["x86_64-apple-darwin"]),
  ];
  let mut results = HashMap::new();

  for (os, targets) in targets {
    let max_count = targets.iter().map(|t| {
      utils::run_collect(&["cargo", "tree", "--target", t]).0.lines().count()
    }).max().unwrap_or(0);

    results.insert(os.to_string(), max_count);
  }

  results
}

fn run_exec_time(target_dir: &Path) -> Result<HashMap<String, HashMap<String, f64>>> {
  let benchmark_file = target_dir.join("hyperfine_results.json");

  let mut command = vec![
    "hyperfine", "--export-json", benchmark_file.to_str().unwrap(), "--warmup", "3",
  ];
  command.extend(get_all_benchmarks().iter().map(|(_, exe)| exe.clone()));

  utils::run(&command.iter().map(AsRef::as_ref).collect::<Vec<_>>());

  utils::read_json(benchmark_file.to_str().unwrap()).map(|results| {
    results["results"]
      .as_array()
      .unwrap()
      .iter()
      .enumerate()
      .map(|(i, data)| (BENCHMARKS[i].0.to_string(), utils::extract_hyperfine_metrics(data)))
      .collect()
  })
}

fn main() -> Result<()> {
  env_logger::init();

  let json_3mb = utils::home_path().join(".tauri_3mb.json");
  if !json_3mb.exists() {
    utils::download_file(
      "https://github.com/lemarier/tauri-test/releases/download/v2.0.0/json_3mb.json",
      json_3mb,
    );
  }

  info!("Starting tauri benchmark");

  let target_dir = utils::target_dir();
  env::set_current_dir(utils::bench_root_path()).context("Failed to set working directory")?;

  let now = time::OffsetDateTime::now_utc();
  let mut new_data = utils::BenchResult {
    created_at: now.format(&time::format_description::well_known::Iso8601)?,
    sha1: utils::run_collect(&["git", "rev-parse", "HEAD"]).0.trim().to_string(),
    exec_time: run_exec_time(&target_dir)?,
    binary_size: get_binary_sizes(&target_dir)?,
    cargo_deps: get_cargo_deps(),
    ..Default::default()
  };

  if cfg!(target_os = "linux") {
    run_strace_benchmarks(&mut new_data)?;
    new_data.max_memory = run_max_mem_benchmark()?;
  }

  info!("Benchmark completed successfully.");
  serde_json::to_writer_pretty(std::io::stdout(), &new_data)?;

  utils::write_json(target_dir.join("bench.json"), &serde_json::to_value(&new_data)?)?;

  Ok(())
}
