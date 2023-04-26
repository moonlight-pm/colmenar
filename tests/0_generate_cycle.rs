use colmenar::Workload;

#[test]
fn test_generate_cycle() {
    Workload::new("tests/fixtures/cycle.yaml", "tests/cycle")
        .unwrap()
        .generate()
        .unwrap();
}
