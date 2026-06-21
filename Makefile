all:
	apt update && apt install -y curl build-essential
	curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
	. $$HOME/.cargo/env && cargo build --release -j 1
	cp target/release/project5 crawler
