# Maintainer: Jonathan Kirszling <jonathan.kirszling at runbox dot com>
# Maintainer: Ralph Torres <mail at ralphptorr dot es>
# Contributor: Nick Econopouly <wry at mm dot st>

pkgname=tiny-irc-client-git
pkgver=0.11.0.r18.e125c77
pkgrel=1
pkgdesc='A terminal IRC client written in Rust'
arch=(x86_64)
url=https://github.com/osa1/tiny
license=(MIT)

depends=(dbus)
makedepends=(git cargo)
provides=(${pkgname%-git})
conflicts=(${pkgname%-git})
source=(git+$url)
sha512sums=(SKIP)

_pkgname=${pkgname%-irc-client-git}

pkgver() {
    cd $_pkgname
    git describe --tags --long | \
        sed -e 's/\([^-]*-\)g/r\1/' -e 's/-/./g' -e 's/^v//'
}

build() {
    cd $_pkgname
    cargo install --path crates/$_pkgname --features=desktop-notifications
}

package() {
    cd $_pkgname
    install -Dm755 target/release/$_pkgname "$pkgdir"/usr/bin/$_pkgname
    install -Dm644 LICENSE "$pkgdir"/usr/share/licenses/$_pkgname/LICENSE
    install -Dm644 crates/$_pkgname/config.yml \
        "$pkgdir"/usr/share/$_pkgname/config.yml
    mkdir -p "$pkgdir"/usr/share/doc/$_pkgname
    install -Dm644 ARCHITECTURE.md CHANGELOG.md README.md \
        "$pkgdir"/usr/share/doc/$_pkgname
}
