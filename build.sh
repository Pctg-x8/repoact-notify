#!/bin/bash

cargo lambda build --release --arm64 --compiler cargo --output-format zip
