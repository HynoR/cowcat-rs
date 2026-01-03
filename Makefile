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

clean:
	$(CARGO) clean
