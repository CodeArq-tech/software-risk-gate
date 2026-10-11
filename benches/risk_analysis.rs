use criterion::{Criterion, criterion_group, criterion_main};
use risk_gate::ast::parse_project;
use std::fs;
use tempfile::tempdir;

fn parse_fixture(c: &mut Criterion) {
    let root = tempdir().expect("temp dir");
    for index in 0..100 {
        fs::write(
            root.path().join(format!("module_{index}.py")),
            "class Payment:\n    pass\n",
        )
        .expect("fixture");
    }
    c.bench_function("parse_100_python_files", |b| {
        b.iter(|| parse_project(root.path()).expect("parse"));
    });
}

criterion_group!(benches, parse_fixture);
criterion_main!(benches);
