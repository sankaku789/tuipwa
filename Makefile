# TuiPWA Ghostty native host のビルド。
#
#   make        Ghostty ホストと tuipwa を dist/ へ作る
#   make install   ビルド後、Cargo CLI とユーザー領域のホストを導入
#   make test   Rust と配布物検査の単体テスト
#   make lint   cargo clippy
#   make check-dist   システム GTK 非同梱・動的リンク・通知の検査
#
# キャッシュ先は TUIPWA_BUILD_CACHE か CACHE で変更できる。

SHELL := /bin/bash
.SHELLFLAGS := -eu -o pipefail -c
.ONESHELL:
.DEFAULT_GOAL := all
.PHONY: all install test lint check-dist

ROOT := $(patsubst %/,%,$(dir $(abspath $(lastword $(MAKEFILE_LIST)))))
PREFIX := $(ROOT)/dist
CACHE ?= $(if $(TUIPWA_BUILD_CACHE),$(TUIPWA_BUILD_CACHE),$(if $(XDG_CACHE_HOME),$(XDG_CACHE_HOME),$(HOME)/.cache)/tuipwa-native)

GHOSTTY_VERSION := 1.4.0
# v1.4.0 tag resolves to 83fb95624b0439ee660c2679b43b38c8b63769cd.
# Official release archive includes generated GTK resources; GitHub auto-tar does not.
GHOSTTY_URL := https://release.files.ghostty.org/$(GHOSTTY_VERSION)/ghostty-$(GHOSTTY_VERSION).tar.gz
GHOSTTY_SHA := cb188ac6c3a8278d5fd7bfef15d8c017104c87f638b5560eb38024a71ae0f005
ZIG_VERSION := 0.17.0
ZIG_URL := https://ziglang.org/download/$(ZIG_VERSION)/zig-x86_64-linux-$(ZIG_VERSION).tar.xz
ZIG_SHA := 1cbe9df9f27e6b78d14ccbca43b6703a404ef79ef1c463de901d7f088d4e2026

# 上流ソースに同梱されたライセンス通知を dist/ へ集める。
define UPSTREAM_LICENSES_PY
import pathlib, shutil, sys
src, dst = map(pathlib.Path, sys.argv[1:])
for package in (src / 'zig-pkg').iterdir():
    if not package.is_dir():
        continue
    for f in package.rglob('*'):
        if f.is_file() and (f.name.upper().startswith(('LICENSE', 'COPYING', 'NOTICE')) or f.name.upper() == 'OFL.TXT'):
            out = dst / 'upstream' / package.name / f.relative_to(package)
            out.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(f, out)
endef
export UPSTREAM_LICENSES_PY

# Rust 依存のバージョン・ライセンス宣言とライセンスファイルを集める。
define RUST_LICENSES_PY
import json, pathlib, shutil, sys
out = pathlib.Path(sys.argv[1])
data = json.load(sys.stdin)
with (out / 'rust-dependencies.tsv').open('w') as inventory:
    for p in data['packages']:
        if not p['source']:
            continue
        inventory.write('\t'.join([p['name'], p['version'], p['license'] or 'UNKNOWN']) + '\n')
        src = pathlib.Path(p['manifest_path']).parent
        for f in src.iterdir():
            if f.is_file() and f.name.upper().startswith(('LICENSE', 'COPYING', 'NOTICE')):
                dest = out / 'rust' / (p['name'] + '-' + p['version'])
                dest.mkdir(parents=True, exist_ok=True)
                shutil.copy2(f, dest / f.name)
endef
export RUST_LICENSES_PY

all:
	cd "$(ROOT)"
	mkdir -p "$(CACHE)"
	fetch() {
	  local url="$$1" name="$$2" hash="$$3"
	  if [[ ! -f "$(CACHE)/$$name" ]]; then
	    curl --fail --location "$$url" -o "$(CACHE)/$$name"
	  fi
	  printf '%s  %s\n' "$$hash" "$(CACHE)/$$name" | sha256sum --check --status
	}
	fetch "$(GHOSTTY_URL)" ghostty.tar.gz "$(GHOSTTY_SHA)"
	fetch "$(ZIG_URL)" zig.tar.xz "$(ZIG_SHA)"
	if [[ ! -d "$(CACHE)/zig-x86_64-linux-$(ZIG_VERSION)" ]]; then
	  tar -xf "$(CACHE)/zig.tar.xz" -C "$(CACHE)"
	fi
	# パッチの内容が変わったら別のソースツリーを使い、既存の作業を壊さない。
	PATCH_HASH=$$(sha256sum native/linux/patches/0001-window-identity.patch | cut -d' ' -f1)
	SOURCE="$(CACHE)/ghostty-tuipwa-$(GHOSTTY_VERSION)-$$PATCH_HASH"
	if [[ ! -f "$$SOURCE/.applied" ]]; then
	  rm -rf "$$SOURCE"
	  mkdir -p "$$SOURCE"
	  tar -xf "$(CACHE)/ghostty.tar.gz" -C "$$SOURCE" --strip-components=1
	  ( cd "$$SOURCE" && patch -p1 < "$(ROOT)/native/linux/patches/0001-window-identity.patch" )
	  touch "$$SOURCE/.applied"
	fi
	( cd "$$SOURCE" && "$(CACHE)/zig-x86_64-linux-$(ZIG_VERSION)/zig" build \
	    -Dversion-string="$(GHOSTTY_VERSION)" -Doptimize=ReleaseSmall -Dcpu=baseline -Demit-docs=false \
	    -fsys=spirv-cross -fsys=fontconfig -fsys=freetype \
	    -fsys=libpng -fsys=zlib -fsys=oniguruma -j1 --prefix "$(PREFIX)" )
	cargo build --release --locked
	cp target/release/tuipwa "$(PREFIX)/bin/tuipwa"
	mv "$(PREFIX)/bin/ghostty" "$(PREFIX)/bin/ghostty-host"
	mkdir -p "$(PREFIX)/share/licenses/ghostty"
	cp "$$SOURCE/LICENSE" "$(PREFIX)/share/licenses/ghostty/LICENSE"
	python3 -c "$$UPSTREAM_LICENSES_PY" "$$SOURCE" "$(PREFIX)/share/licenses"
	cargo metadata --locked --format-version 1 | python3 -c "$$RUST_LICENSES_PY" "$(PREFIX)/share/licenses"
	cp LICENSE THIRD_PARTY_NOTICES.md "$(PREFIX)/"
	mkdir -p "$(PREFIX)/share/licenses/system"
	cp licenses/LGPL-2.1.txt "$(PREFIX)/share/licenses/system/"
	python3 native/linux/check-dist.py "$(PREFIX)"
	printf 'Built: %s/bin/tuipwa\nHost: %s/bin/ghostty-host\n' "$(PREFIX)" "$(PREFIX)"

install: all
	cd "$(ROOT)"
	data_home="$${XDG_DATA_HOME:-}"
	if [[ "$$data_home" != /* ]]; then
	  data_home="$${HOME:?HOME が必要です}/.local/share"
	fi
	host_dir="$$data_home/tuipwa/native"
	mkdir -p "$$host_dir/bin" "$$host_dir/share"
	install -m 755 "$(PREFIX)/bin/ghostty-host" "$$host_dir/bin/ghostty-host"
	cp -a "$(PREFIX)/share/." "$$host_dir/share/"
	cp "$(PREFIX)/LICENSE" "$(PREFIX)/THIRD_PARTY_NOTICES.md" "$$host_dir/"
	cargo install --path "$(ROOT)" --locked
	printf 'Host installed: %s/bin/ghostty-host\nCLI installed with cargo install (normally ~/.cargo/bin/tuipwa)\n' "$$host_dir"

check-dist:
	cd "$(ROOT)" && python3 native/linux/check-dist.py "$(PREFIX)"

test:
	cd "$(ROOT)" && cargo test --locked
	python3 -m unittest discover -s tests/linux -p 'test_*.py'

lint:
	cd "$(ROOT)" && cargo clippy --locked --all-targets -- -D warnings
