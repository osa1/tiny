# Maintainer: Nick Econopouly <wry at mm dot st>
pkgname=tiny-irc-client-git
pkgver="0.10.0"
pkgrel=1
pkgdesc="A console IRC client"
arch=('x86_64')
provides=('tiny-irc-client')
conflicts=('tiny-irc-client')
url="https://github.com/osa1/tiny"
license=('MIT')
depends=('openssl' 'dbus')
makedepends=('git' 'rust')
source=(git+$url)
sha512sums=(SKIP)

build() {

    # build tiny
    cd tiny
    cargo install --path crates/tiny --features=desktop-notifications
}

package() {
    cd tiny
    install -Dm755 target/release/tiny "$pkgdir/usr/bin/tiny"
    install -Dm644 LICENSE "$pkgdir/usr/share/licenses/tiny/LICENSE"
}
