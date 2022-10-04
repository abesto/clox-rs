#!/bin/bash

set -euo pipefail

cargo build
cd ../craftinginterpreters
dart ./tool/bin/test.dart clox chap18_types --interpreter ../clox-rs/target/debug/clox-rs
