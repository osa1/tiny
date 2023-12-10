# Maintainer: Ralph Torres <mail at ralphptorr dot es>
# Contributor: Jonathan Kirszling <jonathan dot kirszling at runbox dot com>
# Contributor: Nick Econopouly <wry at mm dot st>

_pkgname=tiny
pkgname=$_pkgname-git
pkgver=0.11.0.r21.65f367e
pkgrel=2
pkgdesc='A terminal IRC client written in Rust'
arch=(x86_64)
url=https://github.com/osa1/tiny
license=(MIT)

provides=($_pkgname)
conflicts=($_pkgname)
replaces=(tiny-irc-client-git)
depends=(dbus)
makedepends=(git cargo)
source=(git+$url)
sha512sums=(SKIP)
options=(!lto)

pkgver() {
    cd "$srcdir"/$_pkgname
    git describe --tags --long --abbrev=7 |\
        sed 's/\([^-]*-\)g/r\1/;s/-/./g;s/^v//'
}

prepare() {
    cd "$srcdir"/$_pkgname
    cargo update
    cargo fetch --locked --target $CARCH-unknown-linux-gnu
}

build() {
    cd "$srcdir"/$_pkgname
    cargo build --frozen --release --features desktop-notifications
}

check() {
    cd "$srcdir"/$_pkgname
    cargo test --frozen --workspace --features desktop-notifications
}

package() {
    cd "$srcdir"/$_pkgname
    install -Dm755 -t "$pkgdir"/usr/bin target/release/$_pkgname
    install -Dm644 -t "$pkgdir"/usr/share/licenses/$_pkgname LICENSE
    install -Dm644 -t "$pkgdir"/usr/share/doc/$_pkgname \
        ARCHITECTURE.md CHANGELOG.md README.md
    install -Dm644 -t "$pkgdir"/usr/share/$_pkgname crates/$_pkgname/config.yml
}
