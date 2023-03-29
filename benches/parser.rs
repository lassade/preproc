use std::{hint::black_box, io::Write, path::Path};

use criterion::{criterion_group, criterion_main, Criterion, Throughput};

use preproc::{sse2::parse_file, Config};

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
        let file = parse_file(&data, &config);
        let mut output = std::fs::File::create(format!("benches/files/{}.dbg", name))
            .expect("failed to create output file");
        for line in &file.lines {
            writeln!(output, "{:?}", &line).unwrap();
        }

        let mut group = c.benchmark_group("sse2::parse_file");
        group.throughput(Throughput::Bytes(data.len() as u64));
        group.sample_size(150);
        group.bench_function(name, |c| {
            c.iter(|| black_box(parse_file(&data, &config)));
        });
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
