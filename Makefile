test:
	cargo test -- --test-threads 1 --nocapture

watch-test:
	cargo watch -s 'make test' -i 'tests/cycle'
