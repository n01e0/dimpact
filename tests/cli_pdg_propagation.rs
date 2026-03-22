#![allow(deprecated)]
use predicates::prelude::*;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn git(cwd: &std::path::Path, args: &[&str]) -> std::process::Output {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(cwd);
    let out = cmd.output().expect("git command failed to spawn");
    if !out.status.success() {
        panic!(
            "git {:?} failed: status {:?}\nstdout:{}\nstderr:{}",
            args,
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    out
}

fn setup_repo() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    let src = r#"fn callee(a: i32) -> i32 { a + 1 }
fn caller() {
    let x = 1;
    let y = callee(x);
    println!("{}", y);
}
"#;
    fs::write(path.join("f.rs"), src).unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    // change x assignment to force a diff in caller
    let src2 = r#"fn callee(a: i32) -> i32 { a + 1 }
fn caller() {
    let x = 2;
    let y = callee(x);
    println!("{}", y);
}
"#;
    fs::write(path.join("f.rs"), src2).unwrap();

    (dir, path)
}

fn setup_two_arg_repo() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    let src = r#"fn callee(a: i32, b: i32) -> i32 { b + 1 }
fn caller() {
    let x = 1;
    let y = 2;
    let out = callee(x, y);
    println!("{}", out);
}
"#;
    fs::write(path.join("f.rs"), src).unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    let src2 = r#"fn callee(a: i32, b: i32) -> i32 { b + 1 }
fn caller() {
    let x = 3;
    let y = 2;
    let out = callee(x, y);
    println!("{}", out);
}
"#;
    fs::write(path.join("f.rs"), src2).unwrap();

    (dir, path)
}

fn setup_repo_with_file(
    rel_path: &str,
    src: &str,
    before: &str,
    after: &str,
) -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    let file_path = path.join(rel_path);
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&file_path, src).unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    let updated = src.replacen(before, after, 1);
    assert_ne!(updated, src, "expected fixture mutation to change source");
    fs::write(&file_path, updated).unwrap();

    (dir, path)
}

fn setup_cross_file_callsite_repo() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::write(
        path.join("callee.rs"),
        "pub fn callee(a: i32) -> i32 { a + 1 }\n",
    )
    .unwrap();
    fs::write(
        path.join("main.rs"),
        r#"mod callee;
fn caller() {
    let x = 1;
    let y = callee::callee(x);
    println!("{}", y);
}
"#,
    )
    .unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("main.rs"),
        r#"mod callee;
fn caller() {
    let x = 2;
    let y = callee::callee(x);
    println!("{}", y);
}
"#,
    )
    .unwrap();

    (dir, path)
}

fn setup_cross_file_callers_repo() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::write(
        path.join("callee.rs"),
        "pub fn callee(a: i32) -> i32 { a + 1 }\n",
    )
    .unwrap();
    fs::write(
        path.join("main.rs"),
        r#"mod callee;
fn caller() {
    let x = 1;
    let y = callee::callee(x);
    println!("{}", y);
}
"#,
    )
    .unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("callee.rs"),
        "pub fn callee(a: i32) -> i32 { a + 2 }\n",
    )
    .unwrap();

    (dir, path)
}

fn setup_cross_file_wrapper_two_arg_repo() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::write(
        path.join("leaf.rs"),
        r#"pub fn source(v: i32) -> i32 {
    v + 1
}
"#,
    )
    .unwrap();
    fs::write(
        path.join("wrapper.rs"),
        r#"use crate::leaf;

pub fn wrap(left: i32, right: i32) -> i32 {
    let mid = leaf::source(right);
    mid
}
"#,
    )
    .unwrap();
    fs::write(
        path.join("main.rs"),
        r#"mod leaf;
mod wrapper;

fn caller() {
    let x = 1;
    let y = 2;
    let out = wrapper::wrap(x, y);
    println!("{}", out);
}
"#,
    )
    .unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("main.rs"),
        r#"mod leaf;
mod wrapper;

fn caller() {
    let x = 3;
    let y = 2;
    let out = wrapper::wrap(x, y);
    println!("{}", out);
}
"#,
    )
    .unwrap();

    (dir, path)
}

fn setup_cross_file_wrapper_noise_repo() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::write(
        path.join("aaa_helper.rs"),
        r#"pub fn noise(v: i32) -> i32 {
    v - 1
}
"#,
    )
    .unwrap();
    fs::write(
        path.join("leaf.rs"),
        r#"pub fn source(v: i32) -> i32 {
    v + 1
}
"#,
    )
    .unwrap();
    fs::write(
        path.join("wrapper.rs"),
        r#"use crate::aaa_helper;
use crate::leaf;

pub fn wrap(left: i32, right: i32) -> i32 {
    let _noise = aaa_helper::noise(left);
    let mid = leaf::source(right);
    mid
}
"#,
    )
    .unwrap();
    fs::write(
        path.join("main.rs"),
        r#"mod aaa_helper;
mod leaf;
mod wrapper;

fn caller() {
    let x = 1;
    let y = 2;
    let out = wrapper::wrap(x, y);
    println!("{}", out);
}
"#,
    )
    .unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("main.rs"),
        r#"mod aaa_helper;
mod leaf;
mod wrapper;

fn caller() {
    let x = 3;
    let y = 2;
    let out = wrapper::wrap(x, y);
    println!("{}", out);
}
"#,
    )
    .unwrap();

    (dir, path)
}

fn setup_cross_file_returnish_helper_noise_repo() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::write(
        path.join("leaf.rs"),
        r#"pub fn source(v: i32) -> i32 {
    v + 1
}
"#,
    )
    .unwrap();
    fs::write(
        path.join("zzz_final_helper.rs"),
        r#"pub fn final_helper(v: i32) -> i32 {
    v - 1
}
"#,
    )
    .unwrap();
    fs::write(
        path.join("wrapper.rs"),
        r#"use crate::leaf;
use crate::zzz_final_helper;

pub fn wrap(left: i32, right: i32) -> i32 {
    let mid = leaf::source(right);
    let _side = zzz_final_helper::final_helper(left);
    mid
}
"#,
    )
    .unwrap();
    fs::write(
        path.join("main.rs"),
        r#"mod leaf;
mod wrapper;
mod zzz_final_helper;

fn caller() {
    let x = 1;
    let y = 2;
    let out = wrapper::wrap(x, y);
    println!("{}", out);
}
"#,
    )
    .unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("main.rs"),
        r#"mod leaf;
mod wrapper;
mod zzz_final_helper;

fn caller() {
    let x = 3;
    let y = 2;
    let out = wrapper::wrap(x, y);
    println!("{}", out);
}
"#,
    )
    .unwrap();

    (dir, path)
}

fn setup_cross_file_dual_wrapper_repo() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::write(
        path.join("left_leaf.rs"),
        r#"pub fn source_left(shared: i32) -> i32 {
    shared + 1
}
"#,
    )
    .unwrap();
    fs::write(
        path.join("right_leaf.rs"),
        r#"pub fn source_right(shared: i32) -> i32 {
    shared * 2
}
"#,
    )
    .unwrap();
    fs::write(
        path.join("left_wrapper.rs"),
        r#"use crate::left_leaf;

pub fn wrap_left(shared: i32) -> i32 {
    let mid = left_leaf::source_left(shared);
    mid
}
"#,
    )
    .unwrap();
    fs::write(
        path.join("right_wrapper.rs"),
        r#"use crate::right_leaf;

pub fn wrap_right(shared: i32) -> i32 {
    let mid = right_leaf::source_right(shared);
    mid
}
"#,
    )
    .unwrap();
    fs::write(
        path.join("main.rs"),
        r#"mod left_leaf;
mod left_wrapper;
mod right_leaf;
mod right_wrapper;

fn caller() {
    let shared = 1;
    let left = left_wrapper::wrap_left(shared);
    let right = right_wrapper::wrap_right(shared);
    let out = left + right;
    println!("{}", out);
}
"#,
    )
    .unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("main.rs"),
        r#"mod left_leaf;
mod left_wrapper;
mod right_leaf;
mod right_wrapper;

fn caller() {
    let shared = 2;
    let left = left_wrapper::wrap_left(shared);
    let right = right_wrapper::wrap_right(shared);
    let out = left + right;
    println!("{}", out);
}
"#,
    )
    .unwrap();

    (dir, path)
}

fn setup_cross_file_imported_result_alias_repo() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::write(
        path.join("value.rs"),
        r#"pub fn make(a: i32) -> i32 {
    a + 1
}
"#,
    )
    .unwrap();
    fs::write(
        path.join("adapter.rs"),
        r#"use crate::value;

pub fn wrap(a: i32) -> i32 {
    value::make(a)
}
"#,
    )
    .unwrap();
    fs::write(
        path.join("main.rs"),
        r#"mod value;
mod adapter;

fn caller() {
    let x = 1;
    let y = adapter::wrap(x);
    let alias = y;
    let out = alias;
    println!("{}", out);
}
"#,
    )
    .unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("main.rs"),
        r#"mod value;
mod adapter;

fn caller() {
    let x = 2;
    let y = adapter::wrap(x);
    let alias = y;
    let out = alias;
    println!("{}", out);
}
"#,
    )
    .unwrap();

    (dir, path)
}

fn setup_cross_file_imported_result_alias_competition_repo() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::write(
        path.join("value.rs"),
        r#"pub fn make(a: i32) -> i32 {
    a + 1
}
"#,
    )
    .unwrap();
    fs::write(
        path.join("zzz_helper.rs"),
        r#"pub fn noise(v: i32) -> i32 {
    v - 1
}
"#,
    )
    .unwrap();
    fs::write(
        path.join("adapter.rs"),
        r#"use crate::value;
use crate::zzz_helper;

pub fn wrap(a: i32) -> i32 {
    let mid = value::make(a);
    let _noise = zzz_helper::noise(a);
    mid
}
"#,
    )
    .unwrap();
    fs::write(
        path.join("main.rs"),
        r#"mod value;
mod zzz_helper;
mod adapter;

fn caller() {
    let x = 1;
    let y = adapter::wrap(x);
    let alias = y;
    let out = alias;
    println!("{}", out);
}
"#,
    )
    .unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("main.rs"),
        r#"mod value;
mod zzz_helper;
mod adapter;

fn caller() {
    let x = 2;
    let y = adapter::wrap(x);
    let alias = y;
    let out = alias;
    println!("{}", out);
}
"#,
    )
    .unwrap();

    (dir, path)
}

fn setup_cross_file_param_passthrough_competition_repo() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::write(
        path.join("step.rs"),
        r#"pub fn step(input: i32) -> i32 {
    let forwarded = input;
    forwarded
}
"#,
    )
    .unwrap();
    fs::write(
        path.join("later.rs"),
        r#"pub fn later(drop: i32) -> i32 {
    let shadow = drop;
    41
}
"#,
    )
    .unwrap();
    fs::write(
        path.join("wrapper.rs"),
        r#"use crate::later;
use crate::step;

pub fn wrap(a: i32) -> i32 {
    let keep = step::step(a);
    let _side = later::later(a);
    keep
}
"#,
    )
    .unwrap();
    fs::write(
        path.join("main.rs"),
        r#"mod later;
mod step;
mod wrapper;

fn caller() {
    let input = 1;
    let out = wrapper::wrap(input);
    println!("{}", out);
}
"#,
    )
    .unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("main.rs"),
        r#"mod later;
mod step;
mod wrapper;

fn caller() {
    let input = 2;
    let out = wrapper::wrap(input);
    println!("{}", out);
}
"#,
    )
    .unwrap();

    (dir, path)
}

fn setup_cross_file_two_hop_wrapper_return_repo() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::write(
        path.join("leaf.rs"),
        r#"pub fn leaf(a: i32) -> i32 {
    a + 1
}
"#,
    )
    .unwrap();
    fs::write(
        path.join("step.rs"),
        r#"use crate::leaf;

pub fn step(a: i32) -> i32 {
    let v = leaf::leaf(a);
    v + 1
}
"#,
    )
    .unwrap();
    fs::write(
        path.join("wrap.rs"),
        r#"use crate::step;

pub fn wrap(a: i32) -> i32 {
    step::step(a)
}
"#,
    )
    .unwrap();
    fs::write(
        path.join("main.rs"),
        r#"mod leaf;
mod step;
mod wrap;

fn caller() {
    let x = 1;
    let y = wrap::wrap(x);
    println!("{}", y);
}
"#,
    )
    .unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("main.rs"),
        r#"mod leaf;
mod step;
mod wrap;

fn caller() {
    let x = 2;
    let y = wrap::wrap(x);
    println!("{}", y);
}
"#,
    )
    .unwrap();

    (dir, path)
}

fn setup_cross_file_multiline_wrapper_return_repo() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::write(
        path.join("leaf.rs"),
        r#"pub fn source(v: i32) -> i32 { v + 1 }
"#,
    )
    .unwrap();
    fs::write(
        path.join("wrapper.rs"),
        r#"use crate::leaf;

pub fn wrap(v: i32) -> i32 {
    leaf::source(v)
}
"#,
    )
    .unwrap();
    fs::write(
        path.join("main.rs"),
        r#"mod leaf;
mod wrapper;

fn caller() {
    let x = 1;
    let out = wrapper::wrap(
        x,
    );
    println!("{}", out);
}
"#,
    )
    .unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("main.rs"),
        r#"mod leaf;
mod wrapper;

fn caller() {
    let x = 2;
    let out = wrapper::wrap(
        x,
    );
    println!("{}", out);
}
"#,
    )
    .unwrap();

    (dir, path)
}

fn setup_cross_file_semantic_support_competition_repo() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::write(
        path.join("steady.rs"),
        r#"pub fn carry(input: i32) -> i32 {
    let forwarded = input;
    let settled = forwarded;
    settled + forwarded
}
"#,
    )
    .unwrap();
    fs::write(
        path.join("plain.rs"),
        r#"pub fn carry(input: i32) -> i32 {
    input
}
"#,
    )
    .unwrap();
    fs::write(
        path.join("wrapper.rs"),
        r#"use crate::plain;
use crate::steady;

pub fn wrap(a: i32) -> i32 {
    let keep = steady::carry(a);
    let _later = plain::carry(a);
    keep
}
"#,
    )
    .unwrap();
    fs::write(
        path.join("main.rs"),
        r#"mod plain;
mod steady;
mod wrapper;

fn caller() {
    let input = 1;
    let out = wrapper::wrap(input);
    println!("{}", out);
}
"#,
    )
    .unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("main.rs"),
        r#"mod plain;
mod steady;
mod wrapper;

fn caller() {
    let input = 2;
    let out = wrapper::wrap(input);
    println!("{}", out);
}
"#,
    )
    .unwrap();

    (dir, path)
}

fn setup_two_seed_shared_callee_repo() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::write(
        path.join("lib.rs"),
        "pub mod shared;\npub mod left;\npub mod right;\n",
    )
    .unwrap();
    fs::write(path.join("shared.rs"), "pub fn sink() {}\n").unwrap();
    fs::write(
        path.join("left.rs"),
        "pub fn left() { crate::shared::sink(); }\n",
    )
    .unwrap();
    fs::write(
        path.join("right.rs"),
        "pub fn right() { crate::shared::sink(); }\n",
    )
    .unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    (dir, path)
}

fn setup_ruby_require_relative_alias_return_repo() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::create_dir_all(path.join("lib")).unwrap();
    fs::create_dir_all(path.join("app")).unwrap();
    fs::write(
        path.join("lib/service.rb"),
        r#"def bounce(value)
  alias_value = value
  return alias_value
end
"#,
    )
    .unwrap();
    fs::write(
        path.join("app/runner.rb"),
        r#"require_relative '../lib/service'

def entry(seed)
  reply = bounce(seed)
  return reply
end
"#,
    )
    .unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("lib/service.rb"),
        r#"def bounce(value)
  alias_value = value
  return alias_value.to_s
end
"#,
    )
    .unwrap();

    (dir, path)
}

fn setup_ruby_require_relative_no_paren_wrapper_repo() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::create_dir_all(path.join("lib")).unwrap();
    fs::create_dir_all(path.join("app")).unwrap();
    fs::write(
        path.join("lib/leaf.rb"),
        r#"def finish(value)
  alias_value = value
  return alias_value
end
"#,
    )
    .unwrap();
    fs::write(
        path.join("lib/service.rb"),
        r#"require_relative 'leaf'

def helper_noise
  1
end

def bounce value
  noise = helper_noise
  alias_value = value
  wrapped = finish(alias_value)
  return wrapped
end
"#,
    )
    .unwrap();
    fs::write(
        path.join("app/runner.rb"),
        r#"require_relative '../lib/service'

def entry seed
  reply = bounce(seed)
  return reply
end
"#,
    )
    .unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("lib/leaf.rb"),
        r#"def finish(value)
  alias_value = value
  return alias_value.to_s
end
"#,
    )
    .unwrap();

    (dir, path)
}

fn setup_ruby_two_hop_require_relative_return_repo() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::create_dir_all(path.join("lib")).unwrap();
    fs::write(
        path.join("main.rb"),
        r#"require_relative "lib/wrap"

def entry
  x = 1
  y = Wrap.wrap(x)
  puts y
end
"#,
    )
    .unwrap();
    fs::write(
        path.join("lib/wrap.rb"),
        r#"require_relative "step"

module Wrap
  def self.wrap(a)
    Step.step(a)
  end
end
"#,
    )
    .unwrap();
    fs::write(
        path.join("lib/step.rb"),
        r#"require_relative "leaf"

module Step
  def self.step(a)
    v = Leaf.leaf(a)
    v + 1
  end
end
"#,
    )
    .unwrap();
    fs::write(
        path.join("lib/leaf.rb"),
        r#"module Leaf
  def self.leaf(a)
    a + 1
  end
end
"#,
    )
    .unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("main.rb"),
        r#"require_relative "lib/wrap"

def entry
  x = 2
  y = Wrap.wrap(x)
  puts y
end
"#,
    )
    .unwrap();

    (dir, path)
}

fn setup_ruby_require_relative_competing_leaf_repo() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::create_dir_all(path.join("lib")).unwrap();
    fs::create_dir_all(path.join("app")).unwrap();
    fs::write(
        path.join("lib/leaf.rb"),
        r#"def finish(value)
  alias_value = value
  return alias_value
end
"#,
    )
    .unwrap();
    fs::write(
        path.join("lib/zzz_helper.rb"),
        r#"def helper_noise(value)
  debug_value = value.inspect
  return debug_value
end
"#,
    )
    .unwrap();
    fs::write(
        path.join("lib/service.rb"),
        r#"require_relative 'leaf'
require_relative 'zzz_helper'

def bounce(value)
  alias_value = value
  wrapped = finish(alias_value)
  helper = helper_noise(value)
  return wrapped
end
"#,
    )
    .unwrap();
    fs::write(
        path.join("app/runner.rb"),
        r#"require_relative '../lib/service'

def entry(seed)
  prepared = seed + 1
  reply = bounce(prepared)
  return reply
end
"#,
    )
    .unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("app/runner.rb"),
        r#"require_relative '../lib/service'

def entry(seed)
  prepared = seed + 2
  reply = bounce(prepared)
  return reply
end
"#,
    )
    .unwrap();

    (dir, path)
}

fn setup_ruby_dynamic_send_runtime_noise_repo() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::create_dir_all(path.join("lib")).unwrap();
    fs::create_dir_all(path.join("app")).unwrap();
    fs::write(
        path.join("lib/aaa_runtime.rb"),
        r#"class GenericRuntime
  def method_missing(name, *args)
    return args.first if name.to_s.start_with?("route_")
    super
  end

  def respond_to_missing?(name, include_private = false)
    name.to_s.start_with?("route_") || super
  end
end
"#,
    )
    .unwrap();
    fs::write(
        path.join("lib/route_runtime.rb"),
        r#"class RuntimeProxy
  def method_missing(name, *args)
    return args.first if name.to_s.start_with?("route_")
    super
  end

  def respond_to_missing?(name, include_private = false)
    name.to_s.start_with?("route_") || super
  end
end
"#,
    )
    .unwrap();
    fs::write(
        path.join("lib/service.rb"),
        r#"require_relative 'aaa_runtime'
require_relative 'route_runtime'

def bounce(payload)
  runtime = Object.const_get("RuntimeProxy").new
  runtime.public_send("route_created", payload)
end
"#,
    )
    .unwrap();
    fs::write(
        path.join("app/runner.rb"),
        r##"require_relative '../lib/service'

def entry(seed)
  prepared = "#{seed}-v1"
  reply = bounce(prepared)
  return reply
end
"##,
    )
    .unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("app/runner.rb"),
        r##"require_relative '../lib/service'

def entry(seed)
  prepared = "#{seed}-v2"
  reply = bounce(prepared)
  return reply
end
"##,
    )
    .unwrap();

    (dir, path)
}

fn diff_text(repo: &std::path::Path) -> String {
    let diff_out = git(repo, &["diff", "--no-ext-diff", "--unified=0"]);
    String::from_utf8(diff_out.stdout).unwrap()
}

fn run_impact_json(repo: &std::path::Path, diff: &str, args: &[&str]) -> serde_json::Value {
    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(repo)
        .arg("impact")
        .args(args)
        .write_stdin(diff.to_string())
        .assert()
        .success();
    serde_json::from_slice(&assert.get_output().stdout).expect("json output")
}

fn run_impact_yaml(repo: &std::path::Path, diff: &str, args: &[&str]) -> serde_json::Value {
    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(repo)
        .arg("impact")
        .args(args)
        .write_stdin(diff.to_string())
        .assert()
        .success();
    let value: serde_yaml::Value =
        serde_yaml::from_slice(&assert.get_output().stdout).expect("yaml output");
    serde_json::to_value(value).expect("yaml->json value")
}

fn slice_selection_file<'a>(
    slice_selection: &'a serde_json::Value,
    path: &str,
) -> &'a serde_json::Value {
    slice_selection["files"]
        .as_array()
        .expect("slice_selection.files array")
        .iter()
        .find(|file| file["path"] == path)
        .unwrap_or_else(|| {
            panic!("missing slice_selection file metadata for {path}: {slice_selection:#}")
        })
}

fn witness_slice_file<'a>(witness: &'a serde_json::Value, path: &str) -> &'a serde_json::Value {
    witness["slice_context"]["selected_files_on_path"]
        .as_array()
        .expect("witness slice_context.selected_files_on_path array")
        .iter()
        .find(|file| file["path"] == path)
        .unwrap_or_else(|| panic!("missing witness slice_context file for {path}: {witness:#}"))
}

fn witness_slice_paths<'a>(witness: &'a serde_json::Value) -> Vec<&'a str> {
    witness["slice_context"]["selected_files_on_path"]
        .as_array()
        .expect("witness slice_context.selected_files_on_path array")
        .iter()
        .filter_map(|file| file["path"].as_str())
        .collect()
}

fn run_impact_dot(repo: &std::path::Path, diff: &str, args: &[&str]) -> String {
    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(repo)
        .arg("impact")
        .args(args)
        .write_stdin(diff.to_string())
        .assert()
        .success();
    String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 output")
}

#[test]
fn pdg_diff_mode_respects_strict_lsp_engine_selection() {
    let (_tmp, repo) = setup_repo();
    let diff = diff_text(&repo);

    let mut ts_cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    ts_cmd
        .current_dir(&repo)
        .env("DIMPACT_DISABLE_REAL_LSP", "1")
        .arg("impact")
        .args([
            "--engine",
            "ts",
            "--with-pdg",
            "--format",
            "json",
            "--direction",
            "callers",
        ])
        .write_stdin(diff.clone())
        .assert()
        .success();

    for pdg_flag in ["--with-pdg", "--with-propagation"] {
        let mut base_cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
        base_cmd
            .current_dir(&repo)
            .env("DIMPACT_DISABLE_REAL_LSP", "1")
            .arg("impact")
            .args([
                "--engine",
                "lsp",
                "--engine-lsp-strict",
                "--format",
                "json",
                "--direction",
                "callers",
            ])
            .write_stdin(diff.clone())
            .assert()
            .failure()
            .stderr(predicate::str::contains(
                "real LSP disabled by DIMPACT_DISABLE_REAL_LSP=1",
            ));

        let mut pdg_cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
        pdg_cmd
            .current_dir(&repo)
            .env("DIMPACT_DISABLE_REAL_LSP", "1")
            .arg("impact")
            .args([
                "--engine",
                "lsp",
                "--engine-lsp-strict",
                pdg_flag,
                "--format",
                "json",
                "--direction",
                "callers",
            ])
            .write_stdin(diff.clone())
            .assert()
            .failure()
            .stderr(predicate::str::contains(
                "real LSP disabled by DIMPACT_DISABLE_REAL_LSP=1",
            ));
    }
}

#[test]
fn pdg_propagation_adds_var_to_callee_edge() {
    let (_tmp, repo) = setup_repo();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(&repo)
        .arg("impact")
        .arg("--with-pdg")
        .arg("--with-propagation")
        .arg("--format")
        .arg("dot")
        .write_stdin(diff)
        .assert()
        .success()
        .stdout(predicate::str::contains("rust:f.rs:fn:callee:1"));

    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    // Roughly ensure there's an edge into the callee symbol ID
    assert!(stdout.contains("\"rust:f.rs:fn:callee:1\""));
}

#[test]
fn pdg_path_assigns_confirmed_or_inferred_confidence_only() {
    let (_tmp, repo) = setup_repo();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(&repo)
        .arg("impact")
        .arg("--with-pdg")
        .arg("--with-propagation")
        .arg("--with-edges")
        .arg("--format")
        .arg("json")
        .write_stdin(diff)
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 output");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("json output");
    let edges = v["edges"].as_array().expect("edges array");
    assert!(!edges.is_empty(), "expected non-empty edges");

    let certainties: std::collections::BTreeSet<String> = edges
        .iter()
        .filter_map(|e| e["certainty"].as_str().map(|s| s.to_string()))
        .collect();
    let kinds: std::collections::BTreeSet<String> = edges
        .iter()
        .filter_map(|e| e["kind"].as_str().map(|s| s.to_string()))
        .collect();
    let provenances: std::collections::BTreeSet<String> = edges
        .iter()
        .filter_map(|e| e["provenance"].as_str().map(|s| s.to_string()))
        .collect();

    assert!(
        certainties
            .iter()
            .all(|c| c == "confirmed" || c == "inferred"),
        "unexpected certainty values: {:?}",
        certainties
    );
    assert!(
        !certainties.contains("dynamic_fallback"),
        "PDG path should not emit dynamic_fallback certainty"
    );
    assert!(
        kinds.contains("call"),
        "expected merged call edges: {:?}",
        kinds
    );
    assert!(
        kinds.contains("data"),
        "expected merged data edges: {:?}",
        kinds
    );
    assert!(
        provenances.contains("call_graph"),
        "expected call_graph provenance: {:?}",
        provenances
    );
    assert!(
        provenances.contains("symbolic_propagation"),
        "expected symbolic_propagation provenance: {:?}",
        provenances
    );
}

#[test]
fn pdg_propagation_adds_direct_summary_bridge_for_single_line_callee() {
    let (_tmp, repo) = setup_repo();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(&repo)
        .arg("impact")
        .arg("--with-propagation")
        .arg("--format")
        .arg("dot")
        .write_stdin(diff)
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    assert!(
        stdout.contains("\"f.rs:use:x:4\" -> \"f.rs:def:y:4\""),
        "expected summary bridge from callsite arg use into assigned def, got:\n{}",
        stdout
    );
}

#[test]
fn pdg_propagation_adds_cross_file_summary_bridge_for_direct_callee() {
    let (_tmp, repo) = setup_cross_file_callsite_repo();
    let diff = diff_text(&repo);

    let pdg = run_impact_dot(
        &repo,
        &diff,
        &["--direction", "callees", "--with-pdg", "--format", "dot"],
    );
    let prop = run_impact_dot(
        &repo,
        &diff,
        &[
            "--direction",
            "callees",
            "--with-propagation",
            "--format",
            "dot",
        ],
    );

    assert!(
        !pdg.contains("\"main.rs:use:x:4\" -> \"main.rs:def:y:4\""),
        "plain PDG should not synthesize the cross-file summary bridge, got:\n{}",
        pdg
    );
    assert!(
        prop.contains("\"main.rs:use:x:4\" -> \"main.rs:def:y:4\""),
        "expected propagation to recover the cross-file summary bridge, got:\n{}",
        prop
    );
    assert!(
        prop.contains("\"callee.rs:def:a:1\""),
        "expected callee-file DFG nodes to be present after related-file expansion, got:\n{}",
        prop
    );
}

#[test]
fn pdg_propagation_does_not_leak_irrelevant_two_arg_bridge() {
    let (_tmp, repo) = setup_two_arg_repo();
    let diff = diff_text(&repo);

    let stdout = run_impact_dot(&repo, &diff, &["--with-propagation", "--format", "dot"]);
    assert!(
        stdout.contains("\"f.rs:use:y:5\" -> \"f.rs:def:out:5\""),
        "expected later arg to keep its summary bridge, got:\n{}",
        stdout
    );
    assert!(
        !stdout.contains("\"f.rs:use:x:5\" -> \"f.rs:def:out:5\""),
        "unexpected irrelevant arg bridge leaked into output, got:\n{}",
        stdout
    );
}

#[test]
fn pdg_propagation_maps_multi_file_wrapper_return_without_leaking_irrelevant_arg() {
    let (_tmp, repo) = setup_cross_file_wrapper_two_arg_repo();
    let diff = diff_text(&repo);

    let pdg = run_impact_dot(
        &repo,
        &diff,
        &["--direction", "callees", "--with-pdg", "--format", "dot"],
    );
    let prop = run_impact_dot(
        &repo,
        &diff,
        &[
            "--direction",
            "callees",
            "--with-propagation",
            "--format",
            "dot",
        ],
    );

    assert!(
        pdg.contains("\"wrapper.rs:def:right:3\""),
        "expected shared bounded-slice scope to include wrapper-file DFG nodes for plain PDG, got:\n{}",
        pdg
    );
    assert!(
        pdg.contains("\"leaf.rs:def:v:1\""),
        "expected bounded slice builder to pull the third-file leaf DFG nodes into plain PDG scope, got:\n{}",
        pdg
    );
    assert!(
        !pdg.contains("\"main.rs:use:y:7\" -> \"main.rs:def:out:7\""),
        "plain PDG should still avoid synthesizing the wrapper return bridge without propagation, got:\n{}",
        pdg
    );
    assert!(
        prop.contains("\"wrapper.rs:def:right:3\""),
        "expected propagation to keep wrapper-file DFG nodes in scope, got:\n{}",
        prop
    );
    assert!(
        prop.contains("\"leaf.rs:def:v:1\""),
        "expected propagation to keep the third-file leaf DFG nodes in scope, got:\n{}",
        prop
    );
    assert!(
        prop.contains("\"main.rs:use:y:7\" -> \"main.rs:def:out:7\""),
        "expected propagation to bridge the return flow from the relevant arg, got:\n{}",
        prop
    );
    assert!(
        !prop.contains("\"main.rs:use:x:7\" -> \"main.rs:def:out:7\""),
        "unexpected irrelevant arg bridge leaked through wrapper summary, got:\n{}",
        prop
    );
}

#[test]
fn pdg_propagation_extends_two_hop_wrapper_return_through_rust_bridge_continuation_scope() {
    let (_tmp, repo) = setup_cross_file_two_hop_wrapper_return_repo();
    let diff = diff_text(&repo);

    let pdg = run_impact_dot(
        &repo,
        &diff,
        &["--direction", "callees", "--with-pdg", "--format", "dot"],
    );
    let prop = run_impact_dot(
        &repo,
        &diff,
        &[
            "--direction",
            "callees",
            "--with-propagation",
            "--format",
            "dot",
        ],
    );
    let prop_json = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callees",
            "--with-propagation",
            "--format",
            "json",
        ],
    );

    assert!(
        pdg.contains("\"leaf.rs:def:a:1\""),
        "expected bounded slice scope to include continuation leaf DFG nodes in plain PDG, got:\n{}",
        pdg
    );
    assert!(
        !pdg.contains("\"main.rs:use:x:7\" -> \"main.rs:def:y:7\""),
        "plain PDG should still avoid synthesizing the two-hop wrapper return bridge without propagation, got:\n{}",
        pdg
    );
    assert!(
        prop.contains("\"leaf.rs:use:a:2\" -> \"main.rs:def:y:7\""),
        "expected propagation to carry the nested leaf return back into the caller result, got:\n{}",
        prop
    );
    assert!(
        prop.contains("\"main.rs:use:x:7\" -> \"main.rs:def:y:7\""),
        "expected propagation to recover the caller-side two-hop wrapper return bridge, got:\n{}",
        prop
    );

    let slice_selection = &prop_json["summary"]["slice_selection"];
    let paths: Vec<&str> = slice_selection["files"]
        .as_array()
        .expect("slice_selection.files array")
        .iter()
        .filter_map(|file| file["path"].as_str())
        .collect();
    assert_eq!(paths, vec!["leaf.rs", "main.rs", "step.rs", "wrap.rs"]);

    let leaf = slice_selection_file(slice_selection, "leaf.rs");
    assert!(
        leaf["reasons"]
            .as_array()
            .is_some_and(|reasons| reasons.iter().any(|reason| {
                reason["tier"] == 3
                    && reason["kind"] == "bridge_continuation_file"
                    && reason["via_symbol_id"] == "rust:step.rs:fn:step:3"
                    && reason["via_path"] == "step.rs"
                    && reason["bridge_kind"] == "wrapper_return"
            })),
        "expected continuation leaf to be explained from the selected step bridge-completion anchor: {prop_json:#}"
    );
}

#[test]
fn pdg_propagation_recovers_multiline_rust_wrapper_callsite_attachment() {
    let (_tmp, repo) = setup_cross_file_multiline_wrapper_return_repo();
    let diff = diff_text(&repo);

    let pdg = run_impact_dot(
        &repo,
        &diff,
        &["--direction", "callees", "--with-pdg", "--format", "dot"],
    );
    let prop = run_impact_dot(
        &repo,
        &diff,
        &[
            "--direction",
            "callees",
            "--with-propagation",
            "--format",
            "dot",
        ],
    );

    assert!(
        pdg.contains("\"wrapper.rs:def:v:3\""),
        "expected plain PDG to keep the wrapper param node in scope, got:\n{}",
        pdg
    );
    assert!(
        !pdg.contains("\"main.rs:use:x:7\" -> \"main.rs:def:out:6\""),
        "plain PDG should still avoid synthesizing the multiline callsite bridge without propagation, got:\n{}",
        pdg
    );
    assert!(
        prop.contains("\"main.rs:use:x:7\" -> \"rust:wrapper.rs:fn:wrap:3\""),
        "expected propagation to attach the off-line caller arg use to the callee symbol, got:\n{}",
        prop
    );
    assert!(
        prop.contains("\"main.rs:use:x:7\" -> \"main.rs:def:out:6\""),
        "expected propagation to recover the caller-side multiline wrapper bridge, got:\n{}",
        prop
    );
    assert!(
        prop.contains("\"leaf.rs:use:v:1\" -> \"main.rs:def:out:6\""),
        "expected propagation to keep the downstream return flow attached to the multiline caller def, got:\n{}",
        prop
    );
}

#[test]
fn pdg_slice_selection_prefers_wrapper_return_leaf_over_earlier_noise_candidate() {
    let (_tmp, repo) = setup_cross_file_wrapper_noise_repo();
    let diff = diff_text(&repo);

    let pdg = run_impact_dot(
        &repo,
        &diff,
        &["--direction", "callees", "--with-pdg", "--format", "dot"],
    );
    let prop = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callees",
            "--with-propagation",
            "--format",
            "json",
        ],
    );

    assert!(
        pdg.contains("\"leaf.rs:def:v:1\""),
        "expected bounded slice scope to keep the later wrapper-return leaf DFG nodes, got:\n{}",
        pdg
    );
    assert!(
        !pdg.contains("\"aaa_helper.rs:def:v:1\""),
        "unexpected noise-side helper DFG nodes entered the bounded slice scope, got:\n{}",
        pdg
    );

    let slice_selection = &prop["summary"]["slice_selection"];
    let paths: Vec<&str> = slice_selection["files"]
        .as_array()
        .expect("slice_selection.files array")
        .iter()
        .filter_map(|file| file["path"].as_str())
        .collect();
    assert_eq!(paths, vec!["leaf.rs", "main.rs", "wrapper.rs"]);

    let leaf = slice_selection_file(slice_selection, "leaf.rs");
    assert!(
        leaf["reasons"]
            .as_array()
            .is_some_and(|reasons| reasons.iter().any(|reason| {
                reason["tier"] == 2
                    && reason["kind"] == "bridge_completion_file"
                    && reason["via_symbol_id"] == "rust:wrapper.rs:fn:wrap:4"
                    && reason["via_path"] == "wrapper.rs"
                    && reason["bridge_kind"] == "wrapper_return"
                    && reason["scoring"]
                        == serde_json::json!({
                            "source_kind": "graph_second_hop",
                            "lane": "return_continuation",
                            "primary_evidence_kinds": [
                                "assigned_result",
                                "return_flow"
                            ],
                            "secondary_evidence_kinds": [
                                "callsite_position_hint",
                                "name_path_hint"
                            ],
                            "score_tuple": {
                                "source_rank": 0,
                                "lane_rank": 0,
                                "primary_evidence_count": 2,
                                "secondary_evidence_count": 2,
                                "call_position_rank": 6,
                                "lexical_tiebreak": "leaf.rs"
                            }
                        })
            })),
        "expected leaf to carry wrapper_return bridge metadata: {prop:#}"
    );
    assert!(
        slice_selection["pruned_candidates"]
            .as_array()
            .is_some_and(|candidates| candidates.iter().any(|candidate| {
                candidate["path"] == "aaa_helper.rs"
                    && candidate["prune_reason"] == "suppressed_before_admit"
                    && candidate["compact_explanation"]
                        == "suppressed_before_admit=helper_noise_suppressor"
                    && candidate["via_symbol_id"] == "rust:wrapper.rs:fn:wrap:4"
                    && candidate["bridge_kind"] == "boundary_alias_continuation"
                    && candidate["scoring"]
                        == serde_json::json!({
                            "source_kind": "graph_second_hop",
                            "lane": "alias_continuation",
                            "primary_evidence_kinds": [
                                "assigned_result"
                            ],
                            "secondary_evidence_kinds": [
                                "name_path_hint"
                            ],
                            "score_tuple": {
                                "source_rank": 0,
                                "lane_rank": 1,
                                "primary_evidence_count": 1,
                                "secondary_evidence_count": 1,
                                "call_position_rank": 5,
                                "lexical_tiebreak": "aaa_helper.rs"
                            }
                        })
            })),
        "expected helper candidate to be kept only as suppressed-before-admit metadata: {prop:#}"
    );
    let witness = &prop["impacted_witnesses"]["rust:leaf.rs:fn:source:1"];
    let leaf_context = witness_slice_file(witness, "leaf.rs");
    assert_eq!(
        leaf_context["selected_vs_pruned_reasons"],
        serde_json::json!([{
            "via_symbol_id": "rust:wrapper.rs:fn:wrap:4",
            "via_path": "wrapper.rs",
            "selected_bridge_kind": "wrapper_return",
            "pruned_path": "aaa_helper.rs",
            "prune_reason": "suppressed_before_admit",
            "pruned_bridge_kind": "boundary_alias_continuation",
            "selected_better_by": "lane",
            "winning_primary_evidence_kinds": [
                "return_flow"
            ],
            "compact_explanation": "suppressed_before_admit=helper_noise_suppressor",
            "summary": "selected over aaa_helper.rs because return_continuation outranked alias_continuation; winning primary evidence: return_flow",
        }])
    );

    let prop_dot = run_impact_dot(
        &repo,
        &diff,
        &[
            "--direction",
            "callees",
            "--with-propagation",
            "--format",
            "dot",
        ],
    );
    assert!(
        prop_dot.contains("\"main.rs:use:y:8\" -> \"main.rs:def:out:8\""),
        "expected propagation to recover the leaf-backed wrapper-return bridge, got:\n{}",
        prop_dot
    );
}

#[test]
fn pdg_slice_selection_penalizes_returnish_helper_noise_after_later_callsite() {
    let (_tmp, repo) = setup_cross_file_returnish_helper_noise_repo();
    let diff = diff_text(&repo);

    let pdg = run_impact_dot(
        &repo,
        &diff,
        &["--direction", "callees", "--with-pdg", "--format", "dot"],
    );
    let prop = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callees",
            "--with-propagation",
            "--format",
            "json",
        ],
    );

    assert!(
        pdg.contains("\"leaf.rs:def:v:1\""),
        "expected bounded slice scope to keep the real leaf return nodes, got:\n{}",
        pdg
    );
    assert!(
        !pdg.contains("\"zzz_final_helper.rs:def:v:1\""),
        "unexpected return-ish helper noise entered the bounded slice scope, got:\n{}",
        pdg
    );

    let slice_selection = &prop["summary"]["slice_selection"];
    let paths: Vec<&str> = slice_selection["files"]
        .as_array()
        .expect("slice_selection.files array")
        .iter()
        .filter_map(|file| file["path"].as_str())
        .collect();
    assert_eq!(paths, vec!["leaf.rs", "main.rs", "wrapper.rs"]);

    let leaf = slice_selection_file(slice_selection, "leaf.rs");
    assert!(
        leaf["reasons"]
            .as_array()
            .is_some_and(|reasons| reasons.iter().any(|reason| {
                reason["tier"] == 2
                    && reason["kind"] == "bridge_completion_file"
                    && reason["via_symbol_id"] == "rust:wrapper.rs:fn:wrap:4"
                    && reason["via_path"] == "wrapper.rs"
                    && reason["bridge_kind"] == "wrapper_return"
                    && reason["scoring"]
                        == serde_json::json!({
                            "source_kind": "graph_second_hop",
                            "lane": "return_continuation",
                            "primary_evidence_kinds": [
                                "assigned_result",
                                "return_flow"
                            ],
                            "secondary_evidence_kinds": [
                                "name_path_hint"
                            ],
                            "score_tuple": {
                                "source_rank": 0,
                                "lane_rank": 0,
                                "primary_evidence_count": 2,
                                "secondary_evidence_count": 1,
                                "call_position_rank": 5,
                                "lexical_tiebreak": "leaf.rs"
                            }
                        })
            })),
        "expected leaf to keep the clean wrapper-return score: {prop:#}"
    );
    assert!(
        slice_selection["pruned_candidates"]
            .as_array()
            .is_some_and(|candidates| candidates.iter().any(|candidate| {
                candidate["path"] == "zzz_final_helper.rs"
                    && candidate["prune_reason"] == "ranked_out"
                    && candidate["via_symbol_id"] == "rust:wrapper.rs:fn:wrap:4"
                    && candidate["bridge_kind"] == "wrapper_return"
                    && candidate["scoring"]
                        == serde_json::json!({
                            "source_kind": "graph_second_hop",
                            "lane": "return_continuation",
                            "primary_evidence_kinds": [
                                "assigned_result",
                                "return_flow"
                            ],
                            "secondary_evidence_kinds": [
                                "callsite_position_hint",
                                "name_path_hint"
                            ],
                            "negative_evidence_kinds": [
                                "noisy_return_hint"
                            ],
                            "score_tuple": {
                                "source_rank": 0,
                                "lane_rank": 0,
                                "primary_evidence_count": 2,
                                "secondary_evidence_count": 2,
                                "negative_evidence_count": 1,
                                "call_position_rank": 6,
                                "lexical_tiebreak": "zzz_final_helper.rs"
                            }
                        })
            })),
        "expected return-ish helper noise to remain only as ranked-out negative evidence metadata: {prop:#}"
    );

    let witness = &prop["impacted_witnesses"]["rust:leaf.rs:fn:source:1"];
    let leaf_context = witness_slice_file(witness, "leaf.rs");
    assert_eq!(
        leaf_context["selected_vs_pruned_reasons"],
        serde_json::json!([{
            "via_symbol_id": "rust:wrapper.rs:fn:wrap:4",
            "via_path": "wrapper.rs",
            "selected_bridge_kind": "wrapper_return",
            "pruned_path": "zzz_final_helper.rs",
            "prune_reason": "ranked_out",
            "pruned_bridge_kind": "wrapper_return",
            "selected_better_by": "negative_evidence_count",
            "losing_side_reason": "negative_evidence=noisy_return_hint",
            "summary": "selected over zzz_final_helper.rs because it had less negative evidence (0 < 1); losing side: negative_evidence=noisy_return_hint",
        }])
    );

    let helper_witness = &prop["impacted_witnesses"]["rust:zzz_final_helper.rs:fn:final_helper:1"];
    let helper_paths = witness_slice_paths(helper_witness);
    assert_eq!(
        helper_paths,
        vec!["main.rs", "wrapper.rs"],
        "expected ranked-out helper witness to stay compact and exclude zzz_final_helper.rs from the explanation slice: {prop:#}"
    );
}

#[test]
fn pdg_slice_selection_prefers_alias_continuation_value_over_later_adapter_helper() {
    let (_tmp, repo) = setup_cross_file_imported_result_alias_competition_repo();
    let diff = diff_text(&repo);

    let pdg = run_impact_dot(
        &repo,
        &diff,
        &["--direction", "callees", "--with-pdg", "--format", "dot"],
    );
    let prop = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callees",
            "--with-propagation",
            "--format",
            "json",
        ],
    );

    assert!(
        pdg.contains("\"value.rs:def:a:1\""),
        "expected bounded slice scope to keep the alias continuation value file, got:\n{}",
        pdg
    );
    assert!(
        !pdg.contains("\"zzz_helper.rs:def:v:1\""),
        "unexpected helper noise file entered the bounded slice scope, got:\n{}",
        pdg
    );

    let slice_selection = &prop["summary"]["slice_selection"];
    let paths: Vec<&str> = slice_selection["files"]
        .as_array()
        .expect("slice_selection.files array")
        .iter()
        .filter_map(|file| file["path"].as_str())
        .collect();
    assert_eq!(paths, vec!["adapter.rs", "main.rs", "value.rs"]);

    let value = slice_selection_file(slice_selection, "value.rs");
    assert!(
        value["reasons"]
            .as_array()
            .is_some_and(|reasons| reasons.iter().any(|reason| {
                reason["tier"] == 2
                    && reason["kind"] == "bridge_completion_file"
                    && reason["via_symbol_id"] == "rust:adapter.rs:fn:wrap:4"
                    && reason["via_path"] == "adapter.rs"
                    && reason["bridge_kind"] == "boundary_alias_continuation"
                    && reason["scoring"]
                        == serde_json::json!({
                            "source_kind": "graph_second_hop",
                            "lane": "alias_continuation",
                            "primary_evidence_kinds": [
                                "alias_chain",
                                "assigned_result"
                            ],
                            "secondary_evidence_kinds": [
                                "name_path_hint"
                            ],
                            "score_tuple": {
                                "source_rank": 0,
                                "lane_rank": 1,
                                "primary_evidence_count": 2,
                                "secondary_evidence_count": 1,
                                "call_position_rank": 5,
                                "lexical_tiebreak": "value.rs"
                            }
                        })
            })),
        "expected value.rs to win the alias-continuation competition with scoring metadata: {prop:#}"
    );
    assert!(
        slice_selection["pruned_candidates"]
            .as_array()
            .is_some_and(|candidates| candidates.iter().any(|candidate| {
                candidate["path"] == "zzz_helper.rs"
                    && candidate["prune_reason"] == "suppressed_before_admit"
                    && candidate["compact_explanation"]
                        == "suppressed_before_admit=helper_noise_suppressor"
                    && candidate["via_symbol_id"] == "rust:adapter.rs:fn:wrap:4"
                    && candidate["bridge_kind"] == "boundary_alias_continuation"
                    && candidate["scoring"]
                        == serde_json::json!({
                            "source_kind": "graph_second_hop",
                            "lane": "alias_continuation",
                            "primary_evidence_kinds": [
                                "assigned_result"
                            ],
                            "secondary_evidence_kinds": [
                                "callsite_position_hint",
                                "name_path_hint"
                            ],
                            "score_tuple": {
                                "source_rank": 0,
                                "lane_rank": 1,
                                "primary_evidence_count": 1,
                                "secondary_evidence_count": 2,
                                "call_position_rank": 6,
                                "lexical_tiebreak": "zzz_helper.rs"
                            }
                        })
            })),
        "expected later helper noise to be preserved only as suppressed-before-admit metadata: {prop:#}"
    );

    let witness = &prop["impacted_witnesses"]["rust:value.rs:fn:make:1"];
    let value_context = witness_slice_file(witness, "value.rs");
    assert_eq!(
        value_context["selected_vs_pruned_reasons"],
        serde_json::json!([{
            "via_symbol_id": "rust:adapter.rs:fn:wrap:4",
            "via_path": "adapter.rs",
            "selected_bridge_kind": "boundary_alias_continuation",
            "pruned_path": "zzz_helper.rs",
            "prune_reason": "suppressed_before_admit",
            "pruned_bridge_kind": "boundary_alias_continuation",
            "selected_better_by": "primary_evidence_count",
            "winning_primary_evidence_kinds": [
                "alias_chain"
            ],
            "compact_explanation": "suppressed_before_admit=helper_noise_suppressor",
            "summary": "selected over zzz_helper.rs because it had more primary evidence (2 > 1); winning primary evidence: alias_chain",
        }])
    );

    let helper_witness = &prop["impacted_witnesses"]["rust:zzz_helper.rs:fn:noise:1"];
    let helper_paths = witness_slice_paths(helper_witness);
    assert_eq!(
        helper_paths,
        vec!["main.rs", "adapter.rs"],
        "expected helper noise to stay outside the selected explanation slice even if it remains reachable: {prop:#}"
    );
}

#[test]
fn pdg_slice_selection_prefers_param_passthrough_leaf_over_later_neutral_helper() {
    let (_tmp, repo) = setup_cross_file_param_passthrough_competition_repo();
    let diff = diff_text(&repo);

    let pdg = run_impact_dot(
        &repo,
        &diff,
        &["--direction", "callees", "--with-pdg", "--format", "dot"],
    );
    let prop = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callees",
            "--with-propagation",
            "--format",
            "json",
        ],
    );

    assert!(
        pdg.contains("\"step.rs:def:forwarded:2\""),
        "expected bounded slice scope to keep the param-passthrough leaf DFG nodes, got:\n{}",
        pdg
    );
    assert!(
        !pdg.contains("\"later.rs:def:shadow:2\""),
        "unexpected later helper noise entered the bounded slice scope, got:\n{}",
        pdg
    );

    let slice_selection = &prop["summary"]["slice_selection"];
    let paths: Vec<&str> = slice_selection["files"]
        .as_array()
        .expect("slice_selection.files array")
        .iter()
        .filter_map(|file| file["path"].as_str())
        .collect();
    assert_eq!(paths, vec!["main.rs", "step.rs", "wrapper.rs"]);
    assert!(
        !serde_json::to_string(&slice_selection["files"])
            .expect("serialize slice_selection.files")
            .contains("later.rs"),
        "unexpected later.rs leaked into slice_selection.files explanation scope: {prop:#}"
    );

    let step = slice_selection_file(slice_selection, "step.rs");
    assert!(
        step["reasons"]
            .as_array()
            .is_some_and(|reasons| reasons.iter().any(|reason| {
                reason["tier"] == 2
                    && reason["kind"] == "bridge_completion_file"
                    && reason["via_symbol_id"] == "rust:wrapper.rs:fn:wrap:4"
                    && reason["via_path"] == "wrapper.rs"
                    && reason["bridge_kind"] == "wrapper_return"
                    && reason["scoring"]
                        == serde_json::json!({
                            "source_kind": "graph_second_hop",
                            "lane": "return_continuation",
                            "primary_evidence_kinds": [
                                "assigned_result",
                                "param_to_return_flow",
                                "return_flow"
                            ],
                            "secondary_evidence_kinds": [
                                "name_path_hint"
                            ],
                            "score_tuple": {
                                "source_rank": 0,
                                "lane_rank": 0,
                                "primary_evidence_count": 3,
                                "secondary_evidence_count": 1,
                                "semantic_support_rank": 2,
                                "call_position_rank": 5,
                                "lexical_tiebreak": "step.rs"
                            },
                            "support": {
                                "local_dfg_support": true
                            }
                        })
            })),
        "expected step.rs to carry param-to-return scoring metadata: {prop:#}"
    );
    assert!(
        slice_selection["pruned_candidates"]
            .as_array()
            .is_some_and(|candidates| candidates.iter().any(|candidate| {
                candidate["path"] == "later.rs"
                    && candidate["prune_reason"] == "weaker_same_family_sibling"
                    && candidate["via_symbol_id"] == "rust:wrapper.rs:fn:wrap:4"
                    && candidate["bridge_kind"] == "wrapper_return"
                    && candidate["scoring"]
                        == serde_json::json!({
                            "source_kind": "graph_second_hop",
                            "lane": "return_continuation",
                            "primary_evidence_kinds": [
                                "assigned_result",
                                "return_flow"
                            ],
                            "secondary_evidence_kinds": [
                                "callsite_position_hint",
                                "name_path_hint"
                            ],
                            "score_tuple": {
                                "source_rank": 0,
                                "lane_rank": 0,
                                "primary_evidence_count": 2,
                                "secondary_evidence_count": 2,
                                "call_position_rank": 6,
                                "lexical_tiebreak": "later.rs"
                            }
                        })
            })),
        "expected later.rs to remain only as weaker same-family sibling metadata once param flow is observed: {prop:#}"
    );

    let witness = &prop["impacted_witnesses"]["rust:step.rs:fn:step:1"];
    let step_context = witness_slice_file(witness, "step.rs");
    assert_eq!(
        step_context["selected_vs_pruned_reasons"],
        serde_json::json!([{
            "via_symbol_id": "rust:wrapper.rs:fn:wrap:4",
            "via_path": "wrapper.rs",
            "selected_bridge_kind": "wrapper_return",
            "pruned_path": "later.rs",
            "prune_reason": "weaker_same_family_sibling",
            "pruned_bridge_kind": "wrapper_return",
            "selected_better_by": "primary_evidence_count",
            "winning_primary_evidence_kinds": [
                "param_to_return_flow"
            ],
            "winning_support": {
                "local_dfg_support": true
            },
            "summary": "selected over later.rs because it had more primary evidence (3 > 2); winning primary evidence: param_to_return_flow; winning support: local_dfg_support",
        }])
    );

    let later_witness = &prop["impacted_witnesses"]["rust:later.rs:fn:later:1"];
    let later_paths = witness_slice_paths(later_witness);
    assert_eq!(
        later_paths,
        vec!["main.rs", "wrapper.rs"],
        "expected weaker same-family sibling later.rs to stay outside the selected explanation slice even if it remains reachable: {prop:#}"
    );
}

#[test]
fn pdg_slice_selection_prefers_stronger_rust_semantic_support_over_later_callsite_hint() {
    let (_tmp, repo) = setup_cross_file_semantic_support_competition_repo();
    let diff = diff_text(&repo);

    let pdg = run_impact_dot(
        &repo,
        &diff,
        &["--direction", "callees", "--with-pdg", "--format", "dot"],
    );
    let prop = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callees",
            "--with-propagation",
            "--format",
            "json",
        ],
    );

    assert!(
        pdg.contains("\"steady.rs:use:settled:4\""),
        "expected bounded slice scope to keep the stronger semantic Rust leaf nodes, got:\n{}",
        pdg
    );
    assert!(
        !pdg.contains("\"plain.rs:use:input:2\""),
        "unexpected later plain passthrough helper entered the bounded slice scope, got:\n{}",
        pdg
    );

    let slice_selection = &prop["summary"]["slice_selection"];
    let paths: Vec<&str> = slice_selection["files"]
        .as_array()
        .expect("slice_selection.files array")
        .iter()
        .filter_map(|file| file["path"].as_str())
        .collect();
    assert_eq!(paths, vec!["main.rs", "steady.rs", "wrapper.rs"]);

    let steady = slice_selection_file(slice_selection, "steady.rs");
    assert!(
        steady["reasons"]
            .as_array()
            .is_some_and(|reasons| reasons.iter().any(|reason| {
                reason["tier"] == 2
                    && reason["kind"] == "bridge_completion_file"
                    && reason["via_symbol_id"] == "rust:wrapper.rs:fn:wrap:4"
                    && reason["via_path"] == "wrapper.rs"
                    && reason["bridge_kind"] == "wrapper_return"
                    && reason["scoring"]
                        == serde_json::json!({
                            "source_kind": "graph_second_hop",
                            "lane": "return_continuation",
                            "primary_evidence_kinds": [
                                "assigned_result",
                                "param_to_return_flow",
                                "return_flow"
                            ],
                            "secondary_evidence_kinds": [
                                "name_path_hint"
                            ],
                            "score_tuple": {
                                "source_rank": 0,
                                "lane_rank": 0,
                                "primary_evidence_count": 3,
                                "secondary_evidence_count": 1,
                                "semantic_support_rank": 3,
                                "call_position_rank": 5,
                                "lexical_tiebreak": "steady.rs"
                            },
                            "support": {
                                "local_dfg_support": true
                            }
                        })
            })),
        "expected steady.rs to win on stronger Rust semantic support: {prop:#}"
    );
    assert!(
        slice_selection["pruned_candidates"]
            .as_array()
            .is_some_and(|candidates| candidates.iter().any(|candidate| {
                candidate["path"] == "plain.rs"
                    && candidate["prune_reason"] == "ranked_out"
                    && candidate["via_symbol_id"] == "rust:wrapper.rs:fn:wrap:4"
                    && candidate["bridge_kind"] == "wrapper_return"
                    && candidate["scoring"]
                        == serde_json::json!({
                            "source_kind": "graph_second_hop",
                            "lane": "return_continuation",
                            "primary_evidence_kinds": [
                                "assigned_result",
                                "param_to_return_flow",
                                "return_flow"
                            ],
                            "secondary_evidence_kinds": [
                                "callsite_position_hint",
                                "name_path_hint"
                            ],
                            "score_tuple": {
                                "source_rank": 0,
                                "lane_rank": 0,
                                "primary_evidence_count": 3,
                                "secondary_evidence_count": 2,
                                "semantic_support_rank": 2,
                                "call_position_rank": 6,
                                "lexical_tiebreak": "plain.rs"
                            },
                            "support": {
                                "local_dfg_support": true
                            }
                        })
            })),
        "expected plain.rs to remain only as ranked-out metadata despite the later callsite hint: {prop:#}"
    );

    let witness = &prop["impacted_witnesses"]["rust:steady.rs:fn:carry:1"];
    let steady_context = witness_slice_file(witness, "steady.rs");
    assert_eq!(
        steady_context["selected_vs_pruned_reasons"],
        serde_json::json!([{
            "via_symbol_id": "rust:wrapper.rs:fn:wrap:4",
            "via_path": "wrapper.rs",
            "selected_bridge_kind": "wrapper_return",
            "pruned_path": "plain.rs",
            "prune_reason": "ranked_out",
            "pruned_bridge_kind": "wrapper_return",
            "selected_better_by": "semantic_support_rank",
            "summary": "selected over plain.rs because it had stronger semantic support (3 > 2)",
        }])
    );

    let plain_witness = &prop["impacted_witnesses"]["rust:plain.rs:fn:carry:1"];
    let plain_paths = witness_slice_paths(plain_witness);
    assert_eq!(
        plain_paths,
        vec!["main.rs", "wrapper.rs"],
        "expected ranked-out plain.rs witness to stay compact and exclude plain.rs from the explanation slice: {prop:#}"
    );
}

#[test]
fn pdg_propagation_extends_two_hop_require_relative_wrapper_return_scope() {
    let (_tmp, repo) = setup_ruby_two_hop_require_relative_return_repo();
    let diff = diff_text(&repo);

    let pdg = run_impact_dot(
        &repo,
        &diff,
        &[
            "--direction",
            "callees",
            "--lang",
            "ruby",
            "--with-pdg",
            "--format",
            "dot",
        ],
    );
    let prop = run_impact_dot(
        &repo,
        &diff,
        &[
            "--direction",
            "callees",
            "--lang",
            "ruby",
            "--with-propagation",
            "--format",
            "dot",
        ],
    );
    let prop_json = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callees",
            "--lang",
            "ruby",
            "--with-propagation",
            "--format",
            "json",
        ],
    );

    assert!(
        pdg.contains("\"lib/leaf.rb:def:a:2\""),
        "expected bounded slice scope to include the two-hop Ruby leaf DFG nodes in plain PDG, got:\n{}",
        pdg
    );
    assert!(
        !pdg.contains("\"lib/leaf.rb:use:a:3\" -> \"main.rb:def:y:5\""),
        "plain PDG should still avoid synthesizing the two-hop Ruby wrapper return bridge without propagation, got:\n{}",
        pdg
    );
    assert!(
        prop.contains("\"lib/leaf.rb:use:a:3\" -> \"main.rb:def:y:5\""),
        "expected propagation to carry the nested Ruby leaf return back into the caller result, got:\n{}",
        prop
    );
    assert!(
        prop.contains("\"lib/step.rb:use:a:5\" -> \"main.rb:def:y:5\""),
        "expected propagation to keep the intermediate Ruby step continuation connected to the caller result, got:\n{}",
        prop
    );

    let slice_selection = &prop_json["summary"]["slice_selection"];
    let paths: Vec<&str> = slice_selection["files"]
        .as_array()
        .expect("slice_selection.files array")
        .iter()
        .filter_map(|file| file["path"].as_str())
        .collect();
    assert_eq!(
        paths,
        vec!["lib/leaf.rb", "lib/step.rb", "lib/wrap.rb", "main.rb"]
    );

    let leaf = slice_selection_file(slice_selection, "lib/leaf.rb");
    assert!(
        leaf["reasons"]
            .as_array()
            .is_some_and(|reasons| reasons.iter().any(|reason| {
                reason["tier"] == 3
                    && reason["kind"] == "bridge_continuation_file"
                    && reason["via_symbol_id"] == "ruby:lib/step.rb:method:step:4"
                    && reason["via_path"] == "lib/step.rb"
                    && reason["bridge_kind"] == "wrapper_return"
            })),
        "expected continuation leaf to be explained from the selected Ruby step bridge-completion anchor: {prop_json:#}"
    );
}

#[test]
fn pdg_slice_selection_prefers_ruby_require_relative_leaf_over_later_helper_noise() {
    let (_tmp, repo) = setup_ruby_require_relative_competing_leaf_repo();
    let diff = diff_text(&repo);

    let pdg = run_impact_dot(
        &repo,
        &diff,
        &[
            "--direction",
            "callees",
            "--lang",
            "ruby",
            "--with-pdg",
            "--format",
            "dot",
        ],
    );
    let prop = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callees",
            "--lang",
            "ruby",
            "--with-propagation",
            "--format",
            "json",
        ],
    );

    assert!(
        pdg.contains("\"lib/leaf.rb:def:alias_value:2\""),
        "expected bounded slice scope to keep the semantic Ruby leaf file, got:\n{}",
        pdg
    );
    assert!(
        !pdg.contains("\"lib/zzz_helper.rb:def:debug_value:2\""),
        "unexpected later Ruby helper noise entered the bounded slice scope, got:\n{}",
        pdg
    );

    let slice_selection = &prop["summary"]["slice_selection"];
    let paths: Vec<&str> = slice_selection["files"]
        .as_array()
        .expect("slice_selection.files array")
        .iter()
        .filter_map(|file| file["path"].as_str())
        .collect();
    assert_eq!(
        paths,
        vec!["app/runner.rb", "lib/leaf.rb", "lib/service.rb"]
    );

    let leaf = slice_selection_file(slice_selection, "lib/leaf.rb");
    assert!(
        leaf["reasons"]
            .as_array()
            .is_some_and(|reasons| reasons.iter().any(|reason| {
                reason["tier"] == 2
                    && reason["kind"] == "bridge_completion_file"
                    && reason["via_symbol_id"] == "ruby:lib/service.rb:method:bounce:4"
                    && reason["via_path"] == "lib/service.rb"
                    && reason["bridge_kind"] == "wrapper_return"
                    && reason["scoring"]
                        == serde_json::json!({
                            "source_kind": "graph_second_hop",
                            "lane": "return_continuation",
                            "primary_evidence_kinds": [
                                "assigned_result",
                                "return_flow"
                            ],
                            "secondary_evidence_kinds": [
                                "name_path_hint"
                            ],
                            "score_tuple": {
                                "source_rank": 0,
                                "lane_rank": 0,
                                "primary_evidence_count": 2,
                                "secondary_evidence_count": 1,
                                "call_position_rank": 6,
                                "lexical_tiebreak": "lib/leaf.rb"
                            }
                        })
            })),
        "expected Ruby leaf to carry wrapper_return bridge metadata: {prop:#}"
    );
    assert!(
        slice_selection["pruned_candidates"]
            .as_array()
            .is_some_and(|candidates| candidates.iter().any(|candidate| {
                candidate["path"] == "lib/zzz_helper.rb"
                    && candidate["prune_reason"] == "suppressed_before_admit"
                    && candidate["compact_explanation"]
                        == "suppressed_before_admit=fallback_only_suppressor"
                    && candidate["via_symbol_id"] == "ruby:lib/service.rb:method:bounce:4"
                    && candidate["bridge_kind"] == "require_relative_chain"
                    && candidate["scoring"]
                        == serde_json::json!({
                            "source_kind": "graph_second_hop",
                            "lane": "require_relative_continuation",
                            "primary_evidence_kinds": [
                                "require_relative_edge"
                            ],
                            "secondary_evidence_kinds": [
                                "callsite_position_hint"
                            ],
                            "score_tuple": {
                                "source_rank": 0,
                                "lane_rank": 2,
                                "primary_evidence_count": 1,
                                "secondary_evidence_count": 1,
                                "call_position_rank": 9,
                                "lexical_tiebreak": "lib/zzz_helper.rb"
                            }
                        })
            })),
        "expected later Ruby helper noise to be preserved only as require_relative suppressed metadata: {prop:#}"
    );
    let witness = &prop["impacted_witnesses"]["ruby:lib/leaf.rb:method:finish:1"];
    let leaf_context = witness_slice_file(witness, "lib/leaf.rb");
    assert_eq!(
        leaf_context["selected_vs_pruned_reasons"],
        serde_json::json!([{
            "via_symbol_id": "ruby:lib/service.rb:method:bounce:4",
            "via_path": "lib/service.rb",
            "selected_bridge_kind": "wrapper_return",
            "pruned_path": "lib/zzz_helper.rb",
            "prune_reason": "suppressed_before_admit",
            "pruned_bridge_kind": "require_relative_chain",
            "selected_better_by": "lane",
            "winning_primary_evidence_kinds": [
                "assigned_result",
                "return_flow"
            ],
            "compact_explanation": "suppressed_before_admit=fallback_only_suppressor",
            "summary": "selected over lib/zzz_helper.rb because return_continuation outranked require_relative_continuation; winning primary evidence: assigned_result + return_flow",
        }])
    );
    let helper_witness =
        &prop["impacted_witnesses"]["ruby:lib/zzz_helper.rb:method:helper_noise:1"];
    let helper_paths = witness_slice_paths(helper_witness);
    assert_eq!(
        helper_paths,
        vec!["app/runner.rb", "lib/service.rb"],
        "expected Ruby helper noise to stay outside the bounded explanation slice even if it remains reachable: {prop:#}"
    );

    let prop_dot = run_impact_dot(
        &repo,
        &diff,
        &[
            "--direction",
            "callees",
            "--lang",
            "ruby",
            "--with-propagation",
            "--format",
            "dot",
        ],
    );
    assert!(
        prop_dot.contains("\"app/runner.rb:use:prepared:5\" -> \"app/runner.rb:def:reply:5\""),
        "expected propagation to recover the Ruby leaf-backed summary bridge, got:\n{}",
        prop_dot
    );
}

#[test]
fn pdg_slice_selection_filters_generic_ruby_dynamic_runtime_noise() {
    let (_tmp, repo) = setup_ruby_dynamic_send_runtime_noise_repo();
    let diff = diff_text(&repo);

    let prop = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callees",
            "--lang",
            "ruby",
            "--with-propagation",
            "--format",
            "json",
        ],
    );

    let slice_selection = &prop["summary"]["slice_selection"];
    let paths: Vec<&str> = slice_selection["files"]
        .as_array()
        .expect("slice_selection.files array")
        .iter()
        .filter_map(|file| file["path"].as_str())
        .collect();
    assert_eq!(
        paths,
        vec!["app/runner.rb", "lib/route_runtime.rb", "lib/service.rb"]
    );
    assert!(
        slice_selection["pruned_candidates"]
            .as_array()
            .is_some_and(|candidates| {
                !candidates.is_empty()
                    && candidates.iter().all(|candidate| {
                        candidate["path"] == "lib/route_runtime.rb"
                            && candidate["prune_reason"] == "weaker_same_path_duplicate"
                            && candidate["compact_explanation"]
                                == "suppressed_before_admit=weaker_same_path_duplicate"
                    })
            }),
        "expected generic runtime noise to stay filtered while same-path route_runtime losers are recorded explicitly: {prop:#}"
    );

    let route_runtime = slice_selection_file(slice_selection, "lib/route_runtime.rb");
    assert!(
        route_runtime["reasons"]
            .as_array()
            .is_some_and(|reasons| reasons.iter().any(|reason| {
                reason["via_symbol_id"] == "ruby:lib/service.rb:method:bounce:4"
                    && reason["via_path"] == "lib/service.rb"
            })),
        "expected family-specific runtime to remain selected through lib/service.rb after filtering generic runtime noise: {prop:#}"
    );
    assert!(
        !serde_json::to_string(&slice_selection["files"])
            .expect("serialize slice_selection.files")
            .contains("weaker_same_path_duplicate"),
        "unexpected same-path duplicate prune metadata widened slice_selection.files explanation scope: {prop:#}"
    );
    assert!(
        !prop["impacted_files"]
            .as_array()
            .is_some_and(|files| files.iter().any(|path| path == "lib/aaa_runtime.rb")),
        "unexpected generic dynamic runtime survived into impacted_files: {prop:#}"
    );
    let impacted_witnesses = prop["impacted_witnesses"]
        .as_object()
        .expect("impacted_witnesses object");
    assert!(
        !impacted_witnesses
            .keys()
            .any(|symbol_id| symbol_id.contains("aaa_runtime.rb")),
        "unexpected generic dynamic runtime survived into impacted_witnesses: {prop:#}"
    );
    assert!(
        !impacted_witnesses
            .values()
            .any(|witness| witness_slice_paths(witness).contains(&"lib/aaa_runtime.rb")),
        "unexpected generic dynamic runtime entered witness slice_context.selected_files_on_path: {prop:#}"
    );
    assert!(
        !serde_json::to_string(slice_selection)
            .expect("serialize slice_selection")
            .contains("lib/aaa_runtime.rb"),
        "unexpected generic dynamic runtime leaked into slice_selection explanation context: {prop:#}"
    );

    let runtime_witness =
        &prop["impacted_witnesses"]["ruby:lib/route_runtime.rb:method:method_missing:2"];
    assert_eq!(
        witness_slice_paths(runtime_witness),
        vec!["app/runner.rb", "lib/service.rb", "lib/route_runtime.rb"],
        "expected duplicate runtime losers to keep witness scope pinned to the selected runtime path: {prop:#}"
    );
    let route_runtime_context = witness_slice_file(runtime_witness, "lib/route_runtime.rb");
    assert!(
        route_runtime_context
            .as_object()
            .is_some_and(|file| !file.contains_key("selected_vs_pruned_reasons")),
        "expected same-path duplicate losers to stay folded into the selected runtime path instead of widening witness reasoning: {prop:#}"
    );

    let service_witness = &prop["impacted_witnesses"]["ruby:lib/service.rb:method:bounce:4"];
    assert_eq!(
        witness_slice_paths(service_witness),
        vec!["app/runner.rb", "lib/service.rb"],
        "expected same-path duplicate runtime losers to stay out of upstream witness scope: {prop:#}"
    );
    assert!(
        !serde_json::to_string(&service_witness["slice_context"])
            .expect("serialize service witness slice_context")
            .contains("lib/route_runtime.rb"),
        "unexpected selected runtime path leaked into upstream witness explanation scope: {prop:#}"
    );
}

#[test]
fn pdg_slice_selection_keeps_two_bridge_completions_for_distinct_boundaries() {
    let (_tmp, repo) = setup_cross_file_dual_wrapper_repo();
    let diff = diff_text(&repo);

    let pdg = run_impact_dot(
        &repo,
        &diff,
        &["--direction", "callees", "--with-pdg", "--format", "dot"],
    );
    let prop = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callees",
            "--with-propagation",
            "--format",
            "json",
        ],
    );

    assert!(
        pdg.contains("\"left_leaf.rs:def:shared:1\""),
        "expected bounded slice scope to keep the left third-file leaf DFG nodes, got:\n{}",
        pdg
    );
    assert!(
        pdg.contains("\"right_leaf.rs:def:shared:1\""),
        "expected bounded slice scope to keep the right third-file leaf DFG nodes, got:\n{}",
        pdg
    );

    let slice_selection = &prop["summary"]["slice_selection"];
    assert_eq!(slice_selection["planner"], "bounded_slice");
    assert_eq!(slice_selection["pruned_candidates"], serde_json::json!([]));
    let paths: Vec<&str> = slice_selection["files"]
        .as_array()
        .expect("slice_selection.files array")
        .iter()
        .filter_map(|file| file["path"].as_str())
        .collect();
    assert_eq!(
        paths,
        vec![
            "left_leaf.rs",
            "left_wrapper.rs",
            "main.rs",
            "right_leaf.rs",
            "right_wrapper.rs",
        ]
    );

    let left_leaf = slice_selection_file(slice_selection, "left_leaf.rs");
    assert!(
        left_leaf["reasons"]
            .as_array()
            .is_some_and(|reasons| reasons.iter().any(|reason| {
                reason["tier"] == 2
                    && reason["kind"] == "bridge_completion_file"
                    && reason["via_symbol_id"] == "rust:left_wrapper.rs:fn:wrap_left:3"
                    && reason["via_path"] == "left_wrapper.rs"
                    && reason["bridge_kind"] == "wrapper_return"
            })),
        "expected left leaf to carry wrapper_return bridge metadata: {prop:#}"
    );

    let right_leaf = slice_selection_file(slice_selection, "right_leaf.rs");
    assert!(
        right_leaf["reasons"]
            .as_array()
            .is_some_and(|reasons| reasons.iter().any(|reason| {
                reason["tier"] == 2
                    && reason["kind"] == "bridge_completion_file"
                    && reason["via_symbol_id"] == "rust:right_wrapper.rs:fn:wrap_right:3"
                    && reason["via_path"] == "right_wrapper.rs"
                    && reason["bridge_kind"] == "wrapper_return"
            })),
        "expected right leaf to carry wrapper_return bridge metadata: {prop:#}"
    );
}

#[test]
fn pdg_propagation_extends_imported_result_into_caller_alias_chain() {
    let (_tmp, repo) = setup_cross_file_imported_result_alias_repo();
    let diff = diff_text(&repo);

    let pdg = run_impact_dot(
        &repo,
        &diff,
        &["--direction", "callees", "--with-pdg", "--format", "dot"],
    );
    let prop = run_impact_dot(
        &repo,
        &diff,
        &[
            "--direction",
            "callees",
            "--with-propagation",
            "--format",
            "dot",
        ],
    );

    assert!(
        pdg.contains("\"value.rs:def:a:1\""),
        "expected bounded slice builder to include third-file callee DFG nodes in plain PDG scope, got:\n{}",
        pdg
    );
    assert!(
        !pdg.contains("\"value.rs:use:a:2\" -> \"main.rs:def:y:6\""),
        "plain PDG should not synthesize the imported-result bridge into the caller alias chain, got:\n{}",
        pdg
    );
    assert!(
        prop.contains("\"adapter.rs:use:a:4\" -> \"value.rs:def:a:1\""),
        "expected propagation to bridge the boundary callsite into the completion callee input, got:\n{}",
        prop
    );
    assert!(
        prop.contains("\"value.rs:use:a:2\" -> \"main.rs:def:y:6\""),
        "expected propagation to carry the imported result back into caller def y, got:\n{}",
        prop
    );
    assert!(
        prop.contains("\"main.rs:def:y:6\" -> \"main.rs:def:alias:7\""),
        "expected caller-side alias chain to remain connected after the imported result bridge, got:\n{}",
        prop
    );
    assert!(
        prop.contains("\"main.rs:def:alias:7\" -> \"main.rs:def:out:8\""),
        "expected caller-side alias continuation to reach the final output def, got:\n{}",
        prop
    );
}

#[test]
fn propagation_callers_edges_keep_cross_file_callsite_bridges_but_drop_irrelevant_symbol_fanout() {
    let (_tmp, repo) = setup_cross_file_callers_repo();
    let diff = diff_text(&repo);

    let prop = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callers",
            "--with-propagation",
            "--format",
            "json",
            "--with-edges",
        ],
    );
    let edges = prop["edges"].as_array().expect("edges array");
    let pairs: std::collections::BTreeSet<(String, String)> = edges
        .iter()
        .map(|e| {
            (
                e["from"].as_str().unwrap().to_string(),
                e["to"].as_str().unwrap().to_string(),
            )
        })
        .collect();

    assert!(
        pairs.contains(&(
            "rust:main.rs:fn:caller:2".to_string(),
            "main.rs:use:x:4".to_string(),
        )),
        "expected impacted caller to retain the callsite-use bridge: {prop:#}"
    );
    assert!(
        !pairs.contains(&(
            "rust:main.rs:fn:caller:2".to_string(),
            "main.rs:def:x:3".to_string(),
        )),
        "unexpected non-callsite fanout into caller-local seed def leaked into edges: {prop:#}"
    );
    assert!(
        !pairs.contains(&(
            "rust:main.rs:fn:caller:2".to_string(),
            "main.rs:use:y:5".to_string(),
        )),
        "unexpected post-call use fanout leaked into edges: {prop:#}"
    );
}

#[test]
fn pdg_keeps_latest_def_and_avoids_stale_reassignment_edge() {
    let src = r#"fn demo() {
    let mut a = 1;
    let b = a;
    a = 2;
    let c = a;
    let d = b;
    println!("{} {}", c, d);
}
"#;
    let (_tmp, repo) = setup_repo_with_file(
        "f.rs",
        src,
        "println!(\"{} {}\", c, d);",
        "println!(\"{} {}!\", c, d);",
    );
    let diff = diff_text(&repo);

    let pdg = run_impact_dot(
        &repo,
        &diff,
        &["--direction", "callees", "--with-pdg", "--format", "dot"],
    );
    let prop = run_impact_dot(
        &repo,
        &diff,
        &[
            "--direction",
            "callees",
            "--with-propagation",
            "--format",
            "dot",
        ],
    );

    for out in [&pdg, &prop] {
        assert!(
            out.contains("\"f.rs:def:a:2\" -> \"f.rs:def:b:3\""),
            "expected alias edge a:2 -> b:3, got:\n{}",
            out
        );
        assert!(
            out.contains("\"f.rs:def:a:4\" -> \"f.rs:def:c:5\""),
            "expected latest-def edge a:4 -> c:5, got:\n{}",
            out
        );
        assert!(
            out.contains("\"f.rs:def:b:3\" -> \"f.rs:def:d:6\""),
            "expected alias chain edge b:3 -> d:6, got:\n{}",
            out
        );
        assert!(
            !out.contains("\"f.rs:def:a:2\" -> \"f.rs:def:c:5\""),
            "stale a:2 -> c:5 edge should not reappear, got:\n{}",
            out
        );
    }
}

#[test]
fn ruby_chain_fixture_only_gains_symbolic_edges_with_propagation() {
    let src = include_str!("fixtures/ruby/analyzer_hard_cases_callees_chain_alias_return.rb");
    let (_tmp, repo) = setup_repo_with_file("demo/test.rb", src, "v + inc", "(v + inc) + 1");
    let diff = diff_text(&repo);

    let baseline = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callers",
            "--lang",
            "ruby",
            "--format",
            "json",
            "--with-edges",
        ],
    );
    let pdg = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callers",
            "--lang",
            "ruby",
            "--with-pdg",
            "--format",
            "json",
            "--with-edges",
        ],
    );
    let prop = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callers",
            "--lang",
            "ruby",
            "--with-propagation",
            "--format",
            "json",
            "--with-edges",
        ],
    );

    assert_eq!(baseline["edges"].as_array().unwrap().len(), 0);
    assert_eq!(pdg["edges"].as_array().unwrap().len(), 0);

    let prop_edges = prop["edges"].as_array().expect("edges array");
    assert_eq!(
        prop_edges.len(),
        2,
        "expected fixed pair of symbolic edges: {prop:#}"
    );
    assert!(prop_edges.iter().all(|e| e["kind"] == "data"));
    assert!(
        prop_edges
            .iter()
            .all(|e| e["provenance"] == "symbolic_propagation")
    );
    assert!(
        prop_edges
            .iter()
            .any(|e| e["to"] == "demo/test.rb:def:v:14")
    );
    assert!(
        prop_edges
            .iter()
            .any(|e| e["to"] == "demo/test.rb:use:v:16")
    );
}

#[test]
fn ruby_require_relative_alias_return_only_gains_symbolic_edges_with_propagation() {
    let (_tmp, repo) = setup_ruby_require_relative_alias_return_repo();
    let diff = diff_text(&repo);

    let baseline = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callers",
            "--lang",
            "ruby",
            "--format",
            "json",
            "--with-edges",
        ],
    );
    let pdg = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callers",
            "--lang",
            "ruby",
            "--with-pdg",
            "--format",
            "json",
            "--with-edges",
        ],
    );
    let prop = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callers",
            "--lang",
            "ruby",
            "--with-propagation",
            "--format",
            "json",
            "--with-edges",
        ],
    );

    let data_pairs = |value: &serde_json::Value| -> std::collections::BTreeSet<(String, String)> {
        value["edges"]
            .as_array()
            .expect("edges array")
            .iter()
            .filter(|e| e["kind"] == "data")
            .map(|e| {
                (
                    e["from"].as_str().unwrap().to_string(),
                    e["to"].as_str().unwrap().to_string(),
                )
            })
            .collect()
    };

    assert!(
        data_pairs(&baseline).is_empty(),
        "baseline should keep pure call edges"
    );
    assert!(
        data_pairs(&pdg).is_empty(),
        "plain PDG should not add cross-file alias bridges"
    );

    let prop_data = data_pairs(&prop);
    assert!(
        prop_data.contains(&(
            "app/runner.rb:use:seed:4".to_string(),
            "ruby:lib/service.rb:method:bounce:1".to_string(),
        )),
        "expected propagation to connect caller arg use into required callee: {prop:#}"
    );
    assert!(
        prop_data.contains(&(
            "ruby:lib/service.rb:method:bounce:1".to_string(),
            "app/runner.rb:def:reply:4".to_string(),
        )),
        "expected propagation to connect callee return back into caller def: {prop:#}"
    );
    assert!(
        prop["edges"].as_array().is_some_and(|edges| edges
            .iter()
            .filter(|e| e["kind"] == "data")
            .all(|e| e["provenance"] == "symbolic_propagation")),
        "expected propagation-only data edges to stay tagged as symbolic_propagation: {prop:#}"
    );
}

#[test]
fn ruby_require_relative_no_paren_wrapper_recovers_caller_arg_and_callee_param_scope() {
    let (_tmp, repo) = setup_ruby_require_relative_no_paren_wrapper_repo();
    let diff = diff_text(&repo);

    let baseline = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callers",
            "--lang",
            "ruby",
            "--format",
            "json",
            "--with-edges",
        ],
    );
    let pdg = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callers",
            "--lang",
            "ruby",
            "--with-pdg",
            "--format",
            "json",
            "--with-edges",
        ],
    );
    let prop = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callers",
            "--lang",
            "ruby",
            "--with-propagation",
            "--format",
            "json",
            "--with-edges",
        ],
    );

    let data_pairs = |value: &serde_json::Value| -> std::collections::BTreeSet<(String, String)> {
        value["edges"]
            .as_array()
            .expect("edges array")
            .iter()
            .filter(|e| e["kind"] == "data")
            .map(|e| {
                (
                    e["from"].as_str().unwrap().to_string(),
                    e["to"].as_str().unwrap().to_string(),
                )
            })
            .collect()
    };

    let baseline_data = data_pairs(&baseline);
    let pdg_data = data_pairs(&pdg);
    let prop_data = data_pairs(&prop);

    let caller_arg_bridge = (
        "app/runner.rb:use:seed:4".to_string(),
        "ruby:lib/service.rb:method:bounce:7".to_string(),
    );
    let callee_param_scope = (
        "ruby:lib/service.rb:method:bounce:7".to_string(),
        "lib/service.rb:def:value:7".to_string(),
    );

    assert!(
        !baseline_data.contains(&caller_arg_bridge),
        "baseline should not synthesize the no-paren wrapper caller-arg bridge: {baseline:#}"
    );
    assert!(
        !pdg_data.contains(&caller_arg_bridge),
        "plain PDG should not synthesize the no-paren wrapper caller-arg bridge: {pdg:#}"
    );
    assert!(
        !baseline_data.contains(&callee_param_scope),
        "baseline should not surface the no-paren callee param def: {baseline:#}"
    );
    assert!(
        !pdg_data.contains(&callee_param_scope),
        "plain PDG should not surface the no-paren callee param def: {pdg:#}"
    );
    assert!(
        prop_data.contains(&caller_arg_bridge),
        "expected propagation to recover the caller arg bridge through the no-paren wrapper: {prop:#}"
    );
    assert!(
        prop_data.contains(&callee_param_scope),
        "expected propagation to keep the no-paren callee param in symbolic scope: {prop:#}"
    );
}

#[test]
fn ruby_alias_define_fixture_keeps_defined_sym_without_leaking_defined_only() {
    let src = include_str!("fixtures/ruby/analyzer_hard_cases_dynamic_alias_define_method.rb");
    let (_tmp, repo) = setup_repo_with_file("demo/test.rb", src, ":ok", ":ko");
    let diff = diff_text(&repo);

    let prop = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callers",
            "--lang",
            "ruby",
            "--with-propagation",
            "--format",
            "json",
            "--with-edges",
        ],
    );
    let edges = prop["edges"].as_array().expect("edges array");

    assert!(
        edges
            .iter()
            .any(|e| e["to"] == "ruby:demo/test.rb:method:defined_sym:9")
    );
    assert!(
        !edges
            .iter()
            .any(|e| e["to"] == "ruby:demo/test.rb:method:defined_only:17"),
        "unexpected leak into defined_only: {prop:#}"
    );
}

#[test]
fn ruby_send_fixture_keeps_target_separation_under_propagation() {
    let src = include_str!("fixtures/ruby/analyzer_hard_cases_dynamic_send_public_send.rb");
    let (_tmp, repo) = setup_repo_with_file(
        "demo/test.rb",
        src,
        "send(:target_sym)",
        "send(:target_sym).to_s",
    );
    let diff = diff_text(&repo);

    let prop = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callers",
            "--lang",
            "ruby",
            "--with-propagation",
            "--format",
            "json",
            "--with-edges",
        ],
    );
    let targets: std::collections::BTreeSet<String> = prop["edges"]
        .as_array()
        .expect("edges array")
        .iter()
        .filter_map(|e| e["to"].as_str().map(|s| s.to_string()))
        .collect();

    assert_eq!(
        targets,
        std::collections::BTreeSet::from([
            "ruby:demo/test.rb:method:target_sym:2".to_string(),
            "ruby:demo/test.rb:method:target_str:6".to_string(),
        ]),
        "propagation should keep dynamic target separation: {prop:#}"
    );
}

#[test]
fn pdg_json_reports_slice_selection_summary() {
    let (_tmp, repo) = setup_cross_file_callsite_repo();
    let diff = diff_text(&repo);

    let output = run_impact_json(
        &repo,
        &diff,
        &["--direction", "callees", "--with-pdg", "--format", "json"],
    );

    let slice_selection = &output["summary"]["slice_selection"];
    assert_eq!(slice_selection["planner"], "bounded_slice");
    assert_eq!(slice_selection["pruned_candidates"], serde_json::json!([]));

    let paths: Vec<&str> = slice_selection["files"]
        .as_array()
        .expect("slice_selection.files array")
        .iter()
        .filter_map(|file| file["path"].as_str())
        .collect();
    assert_eq!(paths, vec!["callee.rs", "main.rs"]);

    let main = slice_selection_file(slice_selection, "main.rs");
    assert_eq!(
        main["scopes"],
        serde_json::json!({
            "cache_update": true,
            "local_dfg": true,
            "explanation": true,
        })
    );
    assert_eq!(
        main["reasons"],
        serde_json::json!([{
            "seed_symbol_id": "rust:main.rs:fn:caller:2",
            "tier": 0,
            "kind": "changed_file",
        }])
    );

    let callee = slice_selection_file(slice_selection, "callee.rs");
    assert_eq!(
        callee["scopes"],
        serde_json::json!({
            "cache_update": true,
            "local_dfg": true,
            "explanation": true,
        })
    );
    assert_eq!(
        callee["reasons"],
        serde_json::json!([{
            "seed_symbol_id": "rust:main.rs:fn:caller:2",
            "tier": 1,
            "kind": "direct_callee_file",
            "via_symbol_id": "rust:callee.rs:fn:callee:1",
        }])
    );

    let witness = &output["impacted_witnesses"]["rust:callee.rs:fn:callee:1"];
    assert_eq!(
        witness["slice_context"],
        serde_json::json!({
            "seed_symbol_id": "rust:main.rs:fn:caller:2",
            "selected_files_on_path": [
                {
                    "path": "main.rs",
                    "witness_hops": [0],
                    "selection_reasons": [{
                        "seed_symbol_id": "rust:main.rs:fn:caller:2",
                        "tier": 0,
                        "kind": "changed_file",
                    }],
                    "seed_reasons": [{
                        "seed_symbol_id": "rust:main.rs:fn:caller:2",
                        "tier": 0,
                        "kind": "changed_file",
                    }],
                },
                {
                    "path": "callee.rs",
                    "witness_hops": [0],
                    "selection_reasons": [{
                        "seed_symbol_id": "rust:main.rs:fn:caller:2",
                        "tier": 1,
                        "kind": "direct_callee_file",
                        "via_symbol_id": "rust:callee.rs:fn:callee:1",
                    }],
                    "seed_reasons": [{
                        "seed_symbol_id": "rust:main.rs:fn:caller:2",
                        "tier": 1,
                        "kind": "direct_callee_file",
                        "via_symbol_id": "rust:callee.rs:fn:callee:1",
                    }],
                },
            ],
        })
    );
}

#[test]
fn propagation_yaml_reports_slice_selection_summary() {
    let (_tmp, repo) = setup_cross_file_callsite_repo();
    let diff = diff_text(&repo);

    let output = run_impact_yaml(
        &repo,
        &diff,
        &[
            "--direction",
            "callees",
            "--with-propagation",
            "--format",
            "yaml",
        ],
    );

    let slice_selection = &output["summary"]["slice_selection"];
    assert_eq!(slice_selection["planner"], "bounded_slice");
    assert_eq!(slice_selection["pruned_candidates"], serde_json::json!([]));
    assert_eq!(
        slice_selection["files"]
            .as_array()
            .expect("slice_selection.files array")
            .len(),
        2
    );
    assert_eq!(
        slice_selection_file(slice_selection, "main.rs")["reasons"],
        serde_json::json!([{
            "seed_symbol_id": "rust:main.rs:fn:caller:2",
            "tier": 0,
            "kind": "changed_file",
        }])
    );
    assert_eq!(
        slice_selection_file(slice_selection, "callee.rs")["reasons"],
        serde_json::json!([{
            "seed_symbol_id": "rust:main.rs:fn:caller:2",
            "tier": 1,
            "kind": "direct_callee_file",
            "via_symbol_id": "rust:callee.rs:fn:callee:1",
        }])
    );
}

#[test]
fn per_seed_seed_mode_pdg_keeps_two_hop_wrapper_witness_compact() {
    let (_tmp, repo) = setup_cross_file_two_hop_wrapper_return_repo();

    let grouped = run_impact_json(
        &repo,
        "",
        &[
            "--direction",
            "callers",
            "--seed-symbol",
            "rust:leaf.rs:fn:leaf:1",
            "--with-pdg",
            "--with-edges",
            "--per-seed",
            "--format",
            "json",
        ],
    );

    let grouped = grouped.as_array().expect("per-seed top-level array");
    assert_eq!(grouped.len(), 1);
    assert_eq!(
        grouped[0]["changed_symbol"]["id"].as_str(),
        Some("rust:leaf.rs:fn:leaf:1")
    );
    let output = &grouped[0]["impacts"][0]["output"];
    assert!(
        output["impacted_symbols"]
            .as_array()
            .is_some_and(|syms| syms
                .iter()
                .any(|sym| sym["id"] == "rust:main.rs:fn:caller:5")),
        "expected caller to stay impacted through the bounded four-file slice: {grouped:#?}"
    );
    let witness = &output["impacted_witnesses"]["rust:main.rs:fn:caller:5"];
    assert_eq!(witness["depth"].as_u64(), Some(3));
    assert_eq!(
        witness["via_symbol_id"].as_str(),
        Some("rust:wrap.rs:fn:wrap:3")
    );
    assert_eq!(witness["path"].as_array().map(|v| v.len()), Some(3));
    assert_eq!(witness["path_compact"].as_array().map(|v| v.len()), Some(3));
    assert_eq!(
        witness["path_compact"][0]["from_symbol_id"].as_str(),
        Some("rust:leaf.rs:fn:leaf:1")
    );
    assert_eq!(
        witness["path_compact"][2]["to_symbol_id"].as_str(),
        Some("rust:main.rs:fn:caller:5")
    );
    assert_eq!(
        witness["provenance_chain_compact"],
        serde_json::json!(["call_graph", "call_graph", "call_graph"])
    );
    assert_eq!(
        witness["kind_chain_compact"],
        serde_json::json!(["call", "call", "call"])
    );
    assert_eq!(
        witness_slice_paths(witness),
        vec!["leaf.rs", "step.rs", "wrap.rs", "main.rs"]
    );
    let main_file = witness_slice_file(witness, "main.rs");
    assert!(
        main_file["selection_reasons"]
            .as_array()
            .is_some_and(|reasons| reasons.iter().any(|reason| {
                reason["tier"] == 3
                    && reason["kind"] == "bridge_continuation_file"
                    && reason["via_symbol_id"] == "rust:wrap.rs:fn:wrap:3"
                    && reason["bridge_kind"] == "wrapper_return"
            })),
        "expected caller witness slice context to retain the tier-3 wrapper-return continuation into main.rs: {grouped:#?}"
    );
}

#[test]
fn per_seed_seed_mode_pdg_keeps_three_file_wrapper_witness_compact() {
    let (_tmp, repo) = setup_cross_file_wrapper_two_arg_repo();

    let grouped = run_impact_json(
        &repo,
        "",
        &[
            "--direction",
            "callers",
            "--seed-symbol",
            "rust:leaf.rs:fn:source:1",
            "--with-pdg",
            "--with-edges",
            "--per-seed",
            "--format",
            "json",
        ],
    );

    let grouped = grouped.as_array().expect("per-seed top-level array");
    assert_eq!(grouped.len(), 1);
    assert_eq!(
        grouped[0]["changed_symbol"]["id"].as_str(),
        Some("rust:leaf.rs:fn:source:1")
    );
    let output = &grouped[0]["impacts"][0]["output"];
    assert!(
        output["impacted_symbols"]
            .as_array()
            .is_some_and(|syms| syms
                .iter()
                .any(|sym| sym["id"] == "rust:main.rs:fn:caller:4")),
        "expected caller to stay impacted through the bounded three-file slice: {grouped:#?}"
    );
    let witness = &output["impacted_witnesses"]["rust:main.rs:fn:caller:4"];
    assert_eq!(witness["depth"].as_u64(), Some(2));
    assert_eq!(
        witness["via_symbol_id"].as_str(),
        Some("rust:wrapper.rs:fn:wrap:3")
    );
    assert_eq!(witness["path"].as_array().map(|v| v.len()), Some(2));
    assert_eq!(witness["path_compact"].as_array().map(|v| v.len()), Some(2));
    assert_eq!(
        witness["path_compact"][0]["from_symbol_id"].as_str(),
        Some("rust:leaf.rs:fn:source:1")
    );
    assert_eq!(
        witness["path_compact"][1]["to_symbol_id"].as_str(),
        Some("rust:main.rs:fn:caller:4")
    );
    assert_eq!(
        witness["provenance_chain_compact"],
        serde_json::json!(["call_graph", "call_graph"])
    );
    assert_eq!(
        witness["kind_chain_compact"],
        serde_json::json!(["call", "call"])
    );
}

#[test]
fn per_seed_diff_mode_propagation_keeps_two_hop_wrapper_witness_compact() {
    let (_tmp, repo) = setup_cross_file_two_hop_wrapper_return_repo();
    let diff = diff_text(&repo);

    let grouped = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callees",
            "--with-propagation",
            "--with-edges",
            "--per-seed",
            "--format",
            "json",
        ],
    );

    let grouped = grouped.as_array().expect("per-seed top-level array");
    assert_eq!(grouped.len(), 1);
    let output = &grouped[0]["impacts"][0]["output"];
    assert!(
        output["impacted_symbols"]
            .as_array()
            .is_some_and(|syms| syms.iter().any(|sym| sym["id"] == "rust:leaf.rs:fn:leaf:1")),
        "expected leaf to remain impacted in the two-hop propagation output: {grouped:#?}"
    );
    let witness = &output["impacted_witnesses"]["rust:leaf.rs:fn:leaf:1"];
    assert_eq!(witness["depth"].as_u64(), Some(3));
    assert_eq!(
        witness["root_symbol_id"].as_str(),
        Some("rust:main.rs:fn:caller:5")
    );
    assert_eq!(
        witness["via_symbol_id"].as_str(),
        Some("rust:step.rs:fn:step:3")
    );
    assert_eq!(witness["path"].as_array().map(|v| v.len()), Some(3));
    assert_eq!(witness["path_compact"].as_array().map(|v| v.len()), Some(3));
    assert_eq!(
        witness["path_compact"][0]["to_symbol_id"].as_str(),
        Some("rust:wrap.rs:fn:wrap:3")
    );
    assert_eq!(
        witness["path_compact"][2]["to_symbol_id"].as_str(),
        Some("rust:leaf.rs:fn:leaf:1")
    );
    assert_eq!(
        witness["provenance_chain_compact"],
        serde_json::json!(["call_graph", "call_graph", "call_graph"])
    );
    assert_eq!(
        witness_slice_paths(witness),
        vec!["main.rs", "wrap.rs", "step.rs", "leaf.rs"]
    );
    let leaf_file = witness_slice_file(witness, "leaf.rs");
    assert!(
        leaf_file["selection_reasons"]
            .as_array()
            .is_some_and(|reasons| reasons.iter().any(|reason| {
                reason["tier"] == 3
                    && reason["kind"] == "bridge_continuation_file"
                    && reason["via_symbol_id"] == "rust:step.rs:fn:step:3"
                    && reason["via_path"] == "step.rs"
                    && reason["bridge_kind"] == "wrapper_return"
            })),
        "expected leaf witness slice context to keep the continuation-tier wrapper-return reason: {grouped:#?}"
    );
}

#[test]
fn per_seed_diff_mode_propagation_keeps_imported_result_witness_compact() {
    let (_tmp, repo) = setup_cross_file_imported_result_alias_repo();
    let diff = diff_text(&repo);

    let grouped = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callees",
            "--with-propagation",
            "--with-edges",
            "--per-seed",
            "--format",
            "json",
        ],
    );

    let grouped = grouped.as_array().expect("per-seed top-level array");
    assert_eq!(grouped.len(), 1);
    let output = &grouped[0]["impacts"][0]["output"];
    assert!(
        output["impacted_symbols"]
            .as_array()
            .is_some_and(|syms| syms
                .iter()
                .any(|sym| sym["id"] == "rust:value.rs:fn:make:1")),
        "expected imported result callee to remain impacted in propagation output: {grouped:#?}"
    );
    let witness = &output["impacted_witnesses"]["rust:value.rs:fn:make:1"];
    assert_eq!(witness["depth"].as_u64(), Some(2));
    assert_eq!(
        witness["root_symbol_id"].as_str(),
        Some("rust:main.rs:fn:caller:4")
    );
    assert_eq!(
        witness["via_symbol_id"].as_str(),
        Some("rust:adapter.rs:fn:wrap:3")
    );
    assert_eq!(witness["path"].as_array().map(|v| v.len()), Some(2));
    assert_eq!(witness["path_compact"].as_array().map(|v| v.len()), Some(2));
    assert_eq!(
        witness["path_compact"][0]["to_symbol_id"].as_str(),
        Some("rust:adapter.rs:fn:wrap:3")
    );
    assert_eq!(
        witness["path_compact"][1]["to_symbol_id"].as_str(),
        Some("rust:value.rs:fn:make:1")
    );
    assert_eq!(
        witness["provenance_chain_compact"],
        serde_json::json!(["call_graph", "call_graph"])
    );
    assert_eq!(
        witness["kind_chain_compact"],
        serde_json::json!(["call", "call"])
    );
}

#[test]
fn per_seed_diff_mode_ruby_two_hop_require_relative_propagation_keeps_compact_witness() {
    let (_tmp, repo) = setup_ruby_two_hop_require_relative_return_repo();
    let diff = diff_text(&repo);

    let grouped = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callees",
            "--lang",
            "ruby",
            "--with-propagation",
            "--with-edges",
            "--per-seed",
            "--format",
            "json",
        ],
    );

    let grouped = grouped.as_array().expect("per-seed top-level array");
    assert_eq!(grouped.len(), 1);
    let output = &grouped[0]["impacts"][0]["output"];
    let witness = &output["impacted_witnesses"]["ruby:lib/leaf.rb:method:leaf:2"];
    assert_eq!(witness["depth"].as_u64(), Some(3));
    assert_eq!(
        witness["root_symbol_id"].as_str(),
        Some("ruby:main.rb:method:entry:3")
    );
    assert_eq!(
        witness["via_symbol_id"].as_str(),
        Some("ruby:lib/step.rb:method:step:4")
    );
    assert_eq!(witness["path_compact"].as_array().map(|v| v.len()), Some(3));
    assert_eq!(
        witness["path_compact"][0]["to_symbol_id"].as_str(),
        Some("ruby:lib/wrap.rb:method:wrap:4")
    );
    assert_eq!(
        witness["path_compact"][2]["to_symbol_id"].as_str(),
        Some("ruby:lib/leaf.rb:method:leaf:2")
    );
    assert_eq!(
        witness["provenance_chain_compact"],
        serde_json::json!(["call_graph", "call_graph", "call_graph"])
    );
    assert_eq!(
        witness["kind_chain_compact"],
        serde_json::json!(["call", "call", "call"])
    );
    assert_eq!(
        witness_slice_paths(witness),
        vec!["main.rb", "lib/wrap.rb", "lib/step.rb", "lib/leaf.rb"]
    );
    let leaf_file = witness_slice_file(witness, "lib/leaf.rb");
    assert!(
        leaf_file["selection_reasons"]
            .as_array()
            .is_some_and(|reasons| reasons.iter().any(|reason| {
                reason["tier"] == 3
                    && reason["kind"] == "bridge_continuation_file"
                    && reason["via_symbol_id"] == "ruby:lib/step.rb:method:step:4"
                    && reason["via_path"] == "lib/step.rb"
                    && reason["bridge_kind"] == "wrapper_return"
            })),
        "expected Ruby leaf witness slice context to retain the tier-3 wrapper-return reason: {grouped:#?}"
    );
}

#[test]
fn per_seed_diff_mode_ruby_require_relative_propagation_keeps_compact_witness() {
    let (_tmp, repo) = setup_ruby_require_relative_alias_return_repo();
    let diff = diff_text(&repo);

    let grouped = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callers",
            "--lang",
            "ruby",
            "--with-propagation",
            "--with-edges",
            "--per-seed",
            "--format",
            "json",
        ],
    );

    let grouped = grouped.as_array().expect("per-seed top-level array");
    assert_eq!(grouped.len(), 1);
    let output = &grouped[0]["impacts"][0]["output"];
    let witness = &output["impacted_witnesses"]["ruby:app/runner.rb:method:entry:3"];
    assert_eq!(witness["depth"].as_u64(), Some(1));
    assert_eq!(
        witness["root_symbol_id"].as_str(),
        Some("ruby:lib/service.rb:method:bounce:1")
    );
    assert_eq!(
        witness["path_compact"][0]["from_symbol_id"].as_str(),
        Some("ruby:lib/service.rb:method:bounce:1")
    );
    assert_eq!(
        witness["path_compact"][0]["to_symbol_id"].as_str(),
        Some("ruby:app/runner.rb:method:entry:3")
    );
    assert_eq!(
        witness["path_compact"][0]["collapsed_hops"].as_u64(),
        Some(1)
    );
    assert_eq!(
        witness["provenance_chain_compact"],
        serde_json::json!(["call_graph"])
    );
    assert_eq!(witness["kind_chain_compact"], serde_json::json!(["call"]));
}

#[test]
fn per_seed_diff_mode_supports_propagation() {
    let (_tmp, repo) = setup_repo();
    let diff = diff_text(&repo);

    let grouped = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callees",
            "--with-propagation",
            "--with-edges",
            "--per-seed",
            "--format",
            "json",
        ],
    );

    let grouped = grouped.as_array().expect("per-seed top-level array");
    assert_eq!(grouped.len(), 1);
    assert_eq!(
        grouped[0]["changed_symbol"]["name"].as_str(),
        Some("caller")
    );
    let output = &grouped[0]["impacts"][0]["output"];
    assert!(
        output["impacted_symbols"]
            .as_array()
            .is_some_and(|syms| !syms.is_empty()),
        "expected impacted symbols in per-seed propagation output: {grouped:#?}"
    );
    assert!(
        output["edges"].as_array().is_some_and(|edges| edges
            .iter()
            .any(|e| { e["provenance"] == "call_graph" && e["kind"] == "call" })),
        "expected merged call edge in per-seed output: {grouped:#?}"
    );
    assert!(
        output["impacted_witnesses"]
            .as_object()
            .is_some_and(|w| w.contains_key("rust:f.rs:fn:callee:1")),
        "expected per-seed witness nesting for impacted callee: {grouped:#?}"
    );
    let witness = &output["impacted_witnesses"]["rust:f.rs:fn:callee:1"];
    assert_eq!(
        witness["path"][0]["from_symbol_id"].as_str(),
        Some("rust:f.rs:fn:caller:2")
    );
    assert_eq!(
        witness["path"][0]["to_symbol_id"].as_str(),
        Some("rust:f.rs:fn:callee:1")
    );
    assert_eq!(witness["provenance_chain"][0].as_str(), Some("call_graph"));
    assert_eq!(witness["kind_chain"][0].as_str(), Some("call"));
    assert_eq!(
        witness["path_compact"][0]["from_symbol_id"].as_str(),
        Some("rust:f.rs:fn:caller:2")
    );
    assert_eq!(
        witness["path_compact"][0]["to_symbol_id"].as_str(),
        Some("rust:f.rs:fn:callee:1")
    );
    assert_eq!(
        witness["path_compact"][0]["collapsed_hops"].as_u64(),
        Some(1)
    );
    assert_eq!(
        witness["provenance_chain_compact"][0].as_str(),
        Some("call_graph")
    );
    assert_eq!(witness["kind_chain_compact"][0].as_str(), Some("call"));
}

#[test]
fn per_seed_seed_mode_supports_pdg() {
    let (_tmp, repo) = setup_repo();

    let grouped = run_impact_json(
        &repo,
        "",
        &[
            "--direction",
            "callees",
            "--seed-symbol",
            "rust:f.rs:fn:caller:2",
            "--with-pdg",
            "--with-edges",
            "--per-seed",
            "--format",
            "json",
        ],
    );

    let grouped = grouped.as_array().expect("per-seed top-level array");
    assert_eq!(grouped.len(), 1);
    assert_eq!(
        grouped[0]["changed_symbol"]["id"].as_str(),
        Some("rust:f.rs:fn:caller:2")
    );
    let output = &grouped[0]["impacts"][0]["output"];
    assert!(
        output["impacted_symbols"]
            .as_array()
            .is_some_and(|syms| !syms.is_empty()),
        "expected impacted symbols in seed-based per-seed PDG output: {grouped:#?}"
    );
    assert!(
        output["edges"].as_array().is_some_and(|edges| edges
            .iter()
            .any(|e| { e["provenance"] == "call_graph" && e["kind"] == "call" })),
        "expected call graph edges in seed-based per-seed PDG output: {grouped:#?}"
    );
}

#[test]
fn per_seed_pdg_keeps_slice_selection_attribution_per_seed() {
    let (_tmp, repo) = setup_two_seed_shared_callee_repo();

    let grouped = run_impact_json(
        &repo,
        "",
        &[
            "--direction",
            "callees",
            "--seed-symbol",
            "rust:left.rs:fn:left:1",
            "--seed-symbol",
            "rust:right.rs:fn:right:1",
            "--with-pdg",
            "--per-seed",
            "--format",
            "json",
        ],
    );

    let grouped = grouped.as_array().expect("per-seed top-level array");
    assert_eq!(grouped.len(), 2);

    let left = grouped
        .iter()
        .find(|entry| entry["changed_symbol"]["id"] == "rust:left.rs:fn:left:1")
        .expect("left seed output");
    let left_slice = &left["impacts"][0]["output"]["summary"]["slice_selection"];
    assert_eq!(left_slice["planner"], "bounded_slice");
    assert_eq!(left_slice["pruned_candidates"], serde_json::json!([]));
    let left_paths: Vec<&str> = left_slice["files"]
        .as_array()
        .expect("left slice files")
        .iter()
        .filter_map(|file| file["path"].as_str())
        .collect();
    assert_eq!(left_paths, vec!["left.rs", "shared.rs"]);
    assert_eq!(
        slice_selection_file(left_slice, "left.rs")["reasons"],
        serde_json::json!([{
            "seed_symbol_id": "rust:left.rs:fn:left:1",
            "tier": 0,
            "kind": "seed_file",
        }])
    );
    assert_eq!(
        slice_selection_file(left_slice, "shared.rs")["reasons"],
        serde_json::json!([{
            "seed_symbol_id": "rust:left.rs:fn:left:1",
            "tier": 1,
            "kind": "direct_callee_file",
            "via_symbol_id": "rust:shared.rs:fn:sink:1",
        }])
    );
    let left_witness =
        &left["impacts"][0]["output"]["impacted_witnesses"]["rust:shared.rs:fn:sink:1"];
    assert_eq!(
        left_witness["slice_context"],
        serde_json::json!({
            "seed_symbol_id": "rust:left.rs:fn:left:1",
            "selected_files_on_path": [
                {
                    "path": "left.rs",
                    "witness_hops": [0],
                    "selection_reasons": [{
                        "seed_symbol_id": "rust:left.rs:fn:left:1",
                        "tier": 0,
                        "kind": "seed_file",
                    }],
                    "seed_reasons": [{
                        "seed_symbol_id": "rust:left.rs:fn:left:1",
                        "tier": 0,
                        "kind": "seed_file",
                    }],
                },
                {
                    "path": "shared.rs",
                    "witness_hops": [0],
                    "selection_reasons": [{
                        "seed_symbol_id": "rust:left.rs:fn:left:1",
                        "tier": 1,
                        "kind": "direct_callee_file",
                        "via_symbol_id": "rust:shared.rs:fn:sink:1",
                    }],
                    "seed_reasons": [{
                        "seed_symbol_id": "rust:left.rs:fn:left:1",
                        "tier": 1,
                        "kind": "direct_callee_file",
                        "via_symbol_id": "rust:shared.rs:fn:sink:1",
                    }],
                },
            ],
        })
    );

    let right = grouped
        .iter()
        .find(|entry| entry["changed_symbol"]["id"] == "rust:right.rs:fn:right:1")
        .expect("right seed output");
    let right_slice = &right["impacts"][0]["output"]["summary"]["slice_selection"];
    assert_eq!(right_slice["planner"], "bounded_slice");
    assert_eq!(right_slice["pruned_candidates"], serde_json::json!([]));
    let right_paths: Vec<&str> = right_slice["files"]
        .as_array()
        .expect("right slice files")
        .iter()
        .filter_map(|file| file["path"].as_str())
        .collect();
    assert_eq!(right_paths, vec!["right.rs", "shared.rs"]);
    assert_eq!(
        slice_selection_file(right_slice, "right.rs")["reasons"],
        serde_json::json!([{
            "seed_symbol_id": "rust:right.rs:fn:right:1",
            "tier": 0,
            "kind": "seed_file",
        }])
    );
    assert_eq!(
        slice_selection_file(right_slice, "shared.rs")["reasons"],
        serde_json::json!([{
            "seed_symbol_id": "rust:right.rs:fn:right:1",
            "tier": 1,
            "kind": "direct_callee_file",
            "via_symbol_id": "rust:shared.rs:fn:sink:1",
        }])
    );
    let right_witness =
        &right["impacts"][0]["output"]["impacted_witnesses"]["rust:shared.rs:fn:sink:1"];
    assert_eq!(
        right_witness["slice_context"],
        serde_json::json!({
            "seed_symbol_id": "rust:right.rs:fn:right:1",
            "selected_files_on_path": [
                {
                    "path": "right.rs",
                    "witness_hops": [0],
                    "selection_reasons": [{
                        "seed_symbol_id": "rust:right.rs:fn:right:1",
                        "tier": 0,
                        "kind": "seed_file",
                    }],
                    "seed_reasons": [{
                        "seed_symbol_id": "rust:right.rs:fn:right:1",
                        "tier": 0,
                        "kind": "seed_file",
                    }],
                },
                {
                    "path": "shared.rs",
                    "witness_hops": [0],
                    "selection_reasons": [{
                        "seed_symbol_id": "rust:right.rs:fn:right:1",
                        "tier": 1,
                        "kind": "direct_callee_file",
                        "via_symbol_id": "rust:shared.rs:fn:sink:1",
                    }],
                    "seed_reasons": [{
                        "seed_symbol_id": "rust:right.rs:fn:right:1",
                        "tier": 1,
                        "kind": "direct_callee_file",
                        "via_symbol_id": "rust:shared.rs:fn:sink:1",
                    }],
                },
            ],
        })
    );
}
