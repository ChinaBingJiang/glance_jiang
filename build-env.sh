#!/usr/bin/env bash
# MSVC build environment for Glance, without vcvars64.bat (it shells out to reg.exe,
# which some sandboxes block). Source this before any cargo command:
#   source ./build-env.sh && cargo check
#
# Rust lives OUTSIDE the user profile: RUSTUP_HOME=D:\Tools\rustup, CARGO_HOME=D:\Tools\cargo
# (both are also set as user environment variables, so a normal terminal needs none of this).
# Change the three version numbers below if VS / MSVC / SDK is upgraded.

export V="C:/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools"
export MSVCVER="14.44.35207"
export SDKVER="10.0.26100.0"
export KITS="C:/Program Files (x86)/Windows Kits/10"

export RUSTUP_HOME="D:\\Tools\\rustup"
export CARGO_HOME="D:\\Tools\\cargo"

export PATH="$V/VC/Tools/MSVC/$MSVCVER/bin/Hostx64/x64:$KITS/bin/$SDKVER/x64:/d/Tools/cargo/bin:$PATH"
export LIB="C:\\Program Files (x86)\\Microsoft Visual Studio\\2022\\BuildTools\\VC\\Tools\\MSVC\\$MSVCVER\\lib\\x64;C:\\Program Files (x86)\\Windows Kits\\10\\Lib\\$SDKVER\\um\\x64;C:\\Program Files (x86)\\Windows Kits\\10\\Lib\\$SDKVER\\ucrt\\x64"
export INCLUDE="C:\\Program Files (x86)\\Microsoft Visual Studio\\2022\\BuildTools\\VC\\Tools\\MSVC\\$MSVCVER\\include;C:\\Program Files (x86)\\Windows Kits\\10\\Include\\$SDKVER\\ucrt;C:\\Program Files (x86)\\Windows Kits\\10\\Include\\$SDKVER\\um;C:\\Program Files (x86)\\Windows Kits\\10\\Include\\$SDKVER\\shared"
export RUSTUP_DIST_SERVER=https://rsproxy.cn
export RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup
