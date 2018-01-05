#!/bin/bash
# TODO:
# cross-platform dependancy check: clang, perl, ca-certificates (?) others?
openssl_release="OpenSSL_1_1_0g"


wget https://github.com/openssl/openssl/archive/$openssl_release.tar.gz &&
tar -xf $openssl_release.tar.gz &&
cd openssl-$openssl_release &&
CC=clang ./config no-async &&
make &&
mkdir lib &&
cp libcrypto.a libssl.a lib/ &&
cd ../../.. &&
OPENSSL_DIR=$PWD/pkg/bin/openssl-$openssl_release/ cargo +nightly build --release

