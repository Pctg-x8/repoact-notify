.PHONY: all serialize-pem

CARGO_TARGET_DIR = target/x86_64-unknown-linux-musl/release

all:
	$(MAKE) package.zip

$(CARGO_TARGET_DIR)/bootstrap: src/*.rs Cargo.toml
	cargo build --release --target x86_64-unknown-linux-musl

package.zip: $(CARGO_TARGET_DIR)/bootstrap
	cd $(CARGO_TARGET_DIR) && zip ../../../package.zip bootstrap

serialize-pem: pkey.pem
	sed -z -e 's/\n/\\n/g' pkey.pem
