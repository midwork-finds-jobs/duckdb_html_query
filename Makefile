.PHONY: clean clean_all

PROJ_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))

EXTENSION_NAME=html_query

# Stable V1 C API (C_STRUCT): loads on DuckDB >= TARGET_DUCKDB_VERSION incl. v2.0.x.
# Unstable API would require the v2 entrypoint (_init_c_api_v2), which duckdb-rs classic API does not emit.
USE_UNSTABLE_C_API=0

# Target DuckDB V1 C API version
TARGET_DUCKDB_VERSION=v1.5.6

# DuckDB version used by the python test runner
DUCKDB_TEST_VERSION?=2.0.0

# Build with duckdb feature for extension
CARGO_FEATURES=--features duckdb --no-default-features

all: configure debug

# Include makefiles from DuckDB
include extension-ci-tools/makefiles/c_api_extensions/base.Makefile
include extension-ci-tools/makefiles/c_api_extensions/rust.Makefile

configure: venv platform extension_version

debug: build_extension_library_debug build_extension_with_metadata_debug
release: build_extension_library_release build_extension_with_metadata_release

test: test_debug
test_debug: test_extension_debug
test_release: test_extension_release

clean: clean_build clean_rust
clean_all: clean_configure clean

# Override rust build to add features
build_extension_library_debug: check_configure
	DUCKDB_EXTENSION_NAME=$(EXTENSION_NAME) DUCKDB_EXTENSION_MIN_DUCKDB_VERSION=$(TARGET_DUCKDB_VERSION) cargo build $(CARGO_OVERRIDE_DUCKDB_RS_FLAG) $(TARGET_INFO) $(CARGO_FEATURES)
	$(PYTHON_VENV_BIN) -c "from pathlib import Path;Path('$(EXTENSION_BUILD_PATH)/debug/extension/$(EXTENSION_NAME)').mkdir(parents=True, exist_ok=True)"
	$(PYTHON_VENV_BIN) -c "import shutil;shutil.copyfile('$(TARGET_PATH)/debug$(IS_EXAMPLE)/$(RUST_LIBNAME)', '$(EXTENSION_BUILD_PATH)/debug/$(EXTENSION_LIB_FILENAME)')"

build_extension_library_release: check_configure
	DUCKDB_EXTENSION_NAME=$(EXTENSION_NAME) DUCKDB_EXTENSION_MIN_DUCKDB_VERSION=$(TARGET_DUCKDB_VERSION) cargo build $(CARGO_OVERRIDE_DUCKDB_RS_FLAG) --release $(TARGET_INFO) $(CARGO_FEATURES)
	$(PYTHON_VENV_BIN) -c "from pathlib import Path;Path('$(EXTENSION_BUILD_PATH)/release/extension/$(EXTENSION_NAME)').mkdir(parents=True, exist_ok=True)"
	$(PYTHON_VENV_BIN) -c "import shutil;shutil.copyfile('$(TARGET_PATH)/release$(IS_EXAMPLE)/$(RUST_LIBNAME)', '$(EXTENSION_BUILD_PATH)/release/$(EXTENSION_LIB_FILENAME)')"

# Code quality checks (called by extension-ci-tools _extension_code_quality.yml)
.PHONY: format-check tidy-check
format-check:
	cargo fmt --all -- --check

tidy-check:
	cargo clippy --all-targets $(CARGO_FEATURES) -- -D warnings
