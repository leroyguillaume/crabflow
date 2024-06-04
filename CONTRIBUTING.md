# Contributing

## Prerequisites
- [pre-commit](https://pre-commit.com/)
- [Rust](https://rustup.rs/)

## Getting started

```bash
git clone https://github.com/leroyguillaume/crabflow
cd crabflow
pre-commit install
```

## How to build

```bash
cargo build --all-features
```

## How to build Python bindings

```bash
python -m venv venv
./venv/bin/activate
pip install -r requirements.txt
maturin build -F py
```

## How to execute Python example

See [**How to build Python bindings**](#how-to-build-python-bindings) to see how to install `maturin`.

```bash
maturin develop -F py
python examples/python.py
```

## How to test

```bash
cargo test --all-features
```
