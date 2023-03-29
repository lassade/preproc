use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion, Throughput};

use preproc::exp::Exp;

pub fn criterion_benchmark(c: &mut Criterion) {
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
        let mut group = c.benchmark_group("from_str");
        group.throughput(Throughput::Bytes(exp.len() as u64));
        group.sample_size(150);
        group.bench_function(name, |c| {
            c.iter(|| black_box(Exp::from_str(&exp)));
        });
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
