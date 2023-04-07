use std::{hint::black_box, path::Path};

use criterion::{criterion_group, criterion_main, Criterion, Throughput};

use hashbrown::HashSet;
use preproc::{exp::Exp, parse_file, Config, DefaultFileLoader, PreProcessor};

pub fn criterion_benchmark(c: &mut Criterion) {
    // expressions
    let test_cases = [
        ("noop", "ENABLE_SHADOWS"),
        ("neg", "!ENABLE_SHADOWS"),
        ("realistic", "!EDITOR && (IOS || ANDROID)"),
        (
            "long",
            "(!a && b) || (!c && d && e) && (f || (!h && !c) || !(a && b))",
        ),
        ("bexpr", "Origin && Country && Value && Adults"),
    ];

    for (name, exp) in test_cases {
        let mut group = c.benchmark_group("Exp::from_str");
        group.throughput(Throughput::Bytes(exp.len() as u64));
        group.sample_size(150);
        group.bench_function(name, |c| {
            c.iter(|| black_box(Exp::from_str(&exp)));
        });
    }

    // parse files
    let files = [
        Path::new("benches/files/Native.g.cs"),
        Path::new("benches/files/shader.wgsl"),
        Path::new("benches/files/bevy/pbr/pbr.wgsl"),
    ];

    let names = files.iter().map(|path| {
        path.file_name()
            .expect("path has no filename")
            .to_str()
            .expect("failed to convert the path file into a string")
    });

    let data = files
        .iter()
        .map(|path| std::fs::read_to_string(path).expect("file not found"));

    let config = Config::default();

    for (name, data) in names.zip(data) {
        let mut group = c.benchmark_group("parse_file");
        group.throughput(Throughput::Bytes(data.len() as u64));
        group.sample_size(150);
        group.bench_function(name, |c| {
            c.iter(|| {
                parse_file(&data, &config, |line| {
                    black_box(line);
                })
            });
        });
    }

    // process

    let mut pre_processor = PreProcessor::with_loader({
        let mut file_loader = DefaultFileLoader::default();
        file_loader.search_paths.push("benches/files/bevy".into());
        file_loader
    });

    // pre-load files
    let mut defines = HashSet::with_capacity(32);
    pre_processor.find_defines_of("pbr/pbr.wgsl", &mut defines);

    {
        let mut group = c.benchmark_group("PreProcessor::find_defines_of");
        group.sample_size(150);
        group.bench_function("pbr.wgsl", |c| {
            c.iter(|| {
                defines.clear();
                pre_processor.find_defines_of("pbr/pbr.wgsl", &mut defines);
            });
        });
    }

    {
        let mut group = c.benchmark_group("PreProcessor::process");
        group.sample_size(150);
        group.bench_function("pbr.wgsl", |c| {
            c.iter(|| {
                pre_processor.process("pbr/pbr.wgsl", |line| {
                    black_box(line);
                });
            });
        });
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
