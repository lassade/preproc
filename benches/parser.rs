use std::{hint::black_box, path::Path};

use criterion::{criterion_group, criterion_main, Criterion, Throughput};

use preproc::{sse2, Config};

pub fn criterion_benchmark(c: &mut Criterion) {
    let files = [
        Path::new("benches/files/Native.g.cs"),
        Path::new("benches/files/shader.wgsl"),
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
        let mut group = c.benchmark_group("sse2::parse_file");
        group.throughput(Throughput::Bytes(data.len() as u64));
        group.sample_size(150);
        group.bench_function(name, |c| {
            c.iter(|| {
                sse2::parse_file(&data, &config, |line| {
                    black_box(line);
                })
            });
        });
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
