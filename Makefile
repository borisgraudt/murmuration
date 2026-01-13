.PHONY: install build test clean demo help

help:
	@echo "Elysium Build Commands:"
	@echo ""
	@echo "  make install    Install 'ely' binary to ~/.cargo/bin"
	@echo "  make build      Build release binaries"
	@echo "  make test       Run all tests"
	@echo "  make clean      Clean build artifacts"
	@echo "  make demo       Run quick 2-node demo"
	@echo ""

install:
	@echo "Installing ely to ~/.cargo/bin..."
	cd core && cargo install --path . --bin ely
	@echo ""
	@echo "✓ Done! Now you can run: ely start 8080"
	@echo ""

build:
	@echo "Building release binaries..."
	cd core && cargo build --release --bins
	@echo ""
	@echo "✓ Binaries in: core/target/release/"
	@echo ""

test:
	@echo "Running tests..."
	cd core && cargo test --release
	@echo ""
	@echo "✓ All tests passed!"
	@echo ""

clean:
	@echo "Cleaning build artifacts..."
	cd core && cargo clean
	rm -rf .ely/
	@echo ""
	@echo "✓ Clean!"
	@echo ""

demo:
	@echo "Starting 2-node demo..."
	@echo ""
	@echo "Terminal 1: Starting node on 8080..."
	@echo "Terminal 2: Starting node on 8081..."
	@echo ""
	@echo "Run these commands in separate terminals:"
	@echo ""
	@echo "  Terminal 1:  ely start 8080"
	@echo "  Terminal 2:  ely start 8081 127.0.0.1:8080"
	@echo "  Terminal 3:  ely broadcast \"Hello Elysium!\""
	@echo "  Terminal 3:  ely inbox"
	@echo ""

