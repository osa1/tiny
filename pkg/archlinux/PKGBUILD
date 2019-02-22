# Maintainer: Nick Econopouly <wry at mm dot st>
pkgname=tiny-irc-client-git
pkgver="0.4.3"
pkgrel=1
pkgdesc="A console IRC client"
arch=('x86_64')
provides=('tiny')
url="https://github.com/osa1/tiny"
license=('MIT')
depends=('openssl' 'dbus')
makedepends=('git' 'rust-nightly')
source=(git+$url)
sha512sums=(SKIP)

build() {
           #  Installs the Rust nightly toolchain to a temporary
           #  directory. If you already have the toolchain installed,
           #  e.g. via the script at https://rustup.rs/ or another
           #  package, you can remove the rust-nightly dependancy and
           #  comment out the following three commands.
          
          mkdir nightly
          export RUSTUP_HOME=$(pwd)/nightly
          rustup toolchain install nightly

          # build tiny
          cd tiny
          cargo +nightly build --release
}

package() {
          cd tiny
          install -Dm755 target/release/tiny "$pkgdir/usr/bin/tiny"
          install -Dm644 LICENSE "$pkgdir/usr/share/licenses/tiny/LICENSE"
}
