test:
	# rm -rf tests/cycle
	mkdir -p tests/cycle
	cargo run tests/fixtures/cycle.yaml tests/cycle
	cargo test -- --nocapture

watch-test:
	cargo watch -s 'make test' -i 'tests/cycle'

build:
	cargo run tests/fixtures/cycle.yaml tests/cycle

watch-build:
	cargo watch -s 'make build' -i 'tests/cycle'
