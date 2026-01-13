CARGO := cargo
RELEASE := --release

HOST_ARCH := $(shell uname -m)
HOST_OS := $(shell uname -s)

MAC_TARGET := x86_64-apple-darwin
LINUX_TARGET := x86_64-unknown-linux-gnu

ifeq ($(HOST_ARCH),arm64)
	MAC_TARGET := aarch64-apple-darwin
endif
ifeq ($(HOST_ARCH),aarch64)
	MAC_TARGET := aarch64-apple-darwin
	LINUX_TARGET := aarch64-unknown-linux-gnu
endif

.PHONY: build build-mac build-linux build-all clean

build:
	$(CARGO) build $(RELEASE)

build-mac:
	$(CARGO) build $(RELEASE) --target $(MAC_TARGET)

build-linux:
	$(CARGO) build $(RELEASE) --target $(LINUX_TARGET)

build-all: build-mac build-linux

jsminify:
	rm -rf static/core_minify/catpaw.core.min.js
	rm -rf static/core_minify/catpaw.worker.min.js
	rm -rf static/core_minify/meta.min.js
	bunx esbuild static/core/catpaw.core.js --minify --outfile=static/core_minify/catpaw.core.min.js
	bunx esbuild static/core/catpaw.worker.js --minify --outfile=static/core_minify/catpaw.worker.min.js
	bunx esbuild static/core/meta.js --minify --outfile=static/core_minify/meta.min.js

clean:
	$(CARGO) clean
