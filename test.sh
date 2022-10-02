#!/bin/bash

set -euo pipefail

cargo build
cd ../craftinginterpreters
dart ./tool/bin/test.dart clox chap14_chunks --interpreter ../clox-rs/target/debug/clox-rs
