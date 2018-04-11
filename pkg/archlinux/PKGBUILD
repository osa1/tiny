# Maintainer: Nick Econopouly <wry at mm dot st>
pkgname=tiny-irc-client-git
pkgver="0.4.0"
pkgrel=1
pkgdesc="A console IRC client"
arch=('x86_64')
provides=('tiny')
url="https://github.com/osa1/tiny"
license=('MIT')
depends=('openssl' 'dbus')
makedepends=('git' 'rust-nightly')

build() {
        return 0
}

package() {
          git clone "$url.git"
          cd tiny
          cargo +nightly build --release
          install -Dm755 target/release/tiny "$pkgdir/usr/bin/tiny"
          install -Dm644 LICENSE "$pkgdir/usr/share/licenses/tiny/LICENSE"

}